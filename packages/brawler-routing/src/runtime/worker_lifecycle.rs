//! Worker channel attachment, process supervision, readiness, terminal drain, and cleanup policy.

use std::{
    io,
    time::{Duration, Instant},
};

use mio::Interest;

use crate::{
    AllocationRejectedBody, ControlBody, IpcChannel, LifecycleEvent, PeerId, RequestId,
    RouteRegistration, RoutingErrorCategory, StopId, UnixWorkerChannels, UnixWorkerListeners,
    WorkerId, WorkerKind, WorkerLaunchSpec, WorkerRegistration,
};

use super::{
    ALLOCATION_REJECT_INTERNAL, RESULT_PACKET_DRAIN_TIMEOUT, ReadyTarget, RuntimeError,
    RuntimePollReport, SupervisorRuntime, random_u64, report_lifecycle_event,
    report_runtime_observations,
};

impl SupervisorRuntime {
    pub fn register_worker_listener(
        &mut self,
        registration: WorkerRegistration,
        mut listeners: UnixWorkerListeners,
    ) -> Result<(), RuntimeError> {
        self.register_worker(registration)?;
        let packet_token =
            self.allocate_target(ReadyTarget::PacketListener(registration.worker_id));
        let control_token =
            self.allocate_target(ReadyTarget::ControlListener(registration.worker_id));
        self.poll.registry().register(
            listeners.packet_listener_mut(),
            packet_token,
            Interest::READABLE,
        )?;
        self.poll.registry().register(
            listeners.control_listener_mut(),
            control_token,
            Interest::READABLE,
        )?;
        let worker = self
            .workers
            .get_mut(&registration.worker_id)
            .expect("worker inserted above");
        worker.packet_listener_token = Some(packet_token);
        worker.control_listener_token = Some(control_token);
        worker.listeners = Some(listeners);
        Ok(())
    }

    pub fn attach_worker_channels(
        &mut self,
        worker_id: WorkerId,
        mut channels: UnixWorkerChannels,
    ) -> Result<(), RuntimeError> {
        let packet_token = self.allocate_target(ReadyTarget::Packet(worker_id));
        let control_token = self.allocate_target(ReadyTarget::Control(worker_id));
        let (packet_token, control_token) = {
            let worker = self
                .workers
                .get_mut(&worker_id)
                .ok_or(RuntimeError::Routing(
                    RoutingErrorCategory::ManifestIdentity,
                ))?;
            if worker.channels.is_some() {
                return Err(RuntimeError::Routing(
                    RoutingErrorCategory::WorkerProtocolConflict,
                ));
            }
            self.poll.registry().register(
                channels.packet_source_mut(),
                packet_token,
                Interest::READABLE,
            )?;
            self.poll.registry().register(
                channels.control_source_mut(),
                control_token,
                Interest::READABLE,
            )?;
            worker.packet_token = Some(packet_token);
            worker.control_token = Some(control_token);
            worker.channels = Some(channels);
            (packet_token, control_token)
        };
        let _ = (packet_token, control_token);
        Ok(())
    }

    /// Spawn one worker through the process lifecycle contract while retaining sole ownership of
    /// its listeners and accepted streams in this Mio runtime.
    pub fn spawn_worker(
        &mut self,
        spec: WorkerLaunchSpec,
    ) -> Result<Vec<LifecycleEvent>, RuntimeError> {
        let worker_id = spec.registration.worker_id;
        let registration = spec.registration;
        let manifest_body = spec.manifest.clone();
        let (pending_default_route, match_slot_limit) = if registration.kind == WorkerKind::Lobby {
            let manifest = crate::LobbyManifest::decode(&manifest_body.manifest)
                .map_err(|_| RuntimeError::Routing(RoutingErrorCategory::ManifestMalformed))?;
            (
                Some(RouteRegistration {
                    route_id: manifest.default_route_id,
                    worker_id,
                    peer_id: PeerId::new(manifest.default_route_id.get()).ok_or(
                        RuntimeError::Routing(RoutingErrorCategory::ManifestIdentity),
                    )?,
                    is_default_lobby: true,
                }),
                Some(manifest.active_matches),
            )
        } else {
            (None, None)
        };
        let runtime = self.runtime_dir.as_ref().ok_or(RuntimeError::Routing(
            RoutingErrorCategory::SupervisorShutdown,
        ))?;
        let listeners = UnixWorkerListeners::bind(runtime, worker_id)?;
        let processes = self.processes.as_mut().ok_or(RuntimeError::Routing(
            RoutingErrorCategory::SupervisorInternal,
        ))?;
        let events = processes
            .spawn_with_listeners(spec, listeners)
            .map_err(RuntimeError::Lifecycle)?;
        let listeners = processes
            .take_worker_listeners(worker_id)
            .map_err(RuntimeError::Lifecycle)?;
        self.register_worker_listener(registration, listeners)?;
        if let Some(worker) = self.workers.get_mut(&worker_id) {
            worker.pending_default_route = pending_default_route;
            worker.match_slot_limit = match_slot_limit;
        }
        Ok(events)
    }

    pub fn request_stop(&self) -> io::Result<()> {
        self.waker.wake()
    }

    /// Queue one idempotent worker Stop through the lifecycle owner. The actual control record is
    /// delivered on the next bounded owner turn, preserving the single Mio stream owner.
    pub fn stop_worker(
        &mut self,
        worker_id: WorkerId,
        stop_id: StopId,
        reason: u16,
    ) -> Result<bool, RuntimeError> {
        let next_sequence = self
            .workers
            .get(&worker_id)
            .map(|worker| worker.next_control_sequence)
            .ok_or(RuntimeError::Routing(
                RoutingErrorCategory::SupervisorInternal,
            ))?;
        if let Some(processes) = self.processes.as_mut() {
            processes
                .sync_external_next_sequence(worker_id, next_sequence)
                .map_err(RuntimeError::Lifecycle)?;
        }
        self.processes
            .as_mut()
            .ok_or(RuntimeError::Routing(
                RoutingErrorCategory::SupervisorInternal,
            ))?
            .stop_worker(worker_id, stop_id, reason)
            .inspect(|&queued| {
                if queued && let Some(worker) = self.workers.get_mut(&worker_id) {
                    // The lifecycle owner consumed this sequence for Stop.  Keep the runtime
                    // cursor past it so any later owner-side inspection cannot reuse the frame.
                    worker.next_control_sequence = worker.next_control_sequence.saturating_add(1);
                }
            })
            .map_err(RuntimeError::Lifecycle)
    }

    pub(super) fn shutdown_processes(&mut self) -> Result<(), RuntimeError> {
        if self.processes.is_none() {
            return Ok(());
        }
        self.shutting_down = true;
        // Runtime-owned lobby controls and lifecycle-owned Stop share one BRCT sequence space.
        // Synchronize every live worker before ProcessSupervisor allocates shutdown frames.
        let already_stopping = self
            .workers
            .keys()
            .copied()
            .filter(|worker_id| {
                self.processes
                    .as_ref()
                    .is_some_and(|processes| processes.worker_is_stopping(*worker_id))
            })
            .collect::<std::collections::HashSet<_>>();
        let sequence_cursors = self
            .workers
            .iter()
            .map(|(worker_id, worker)| (*worker_id, worker.next_control_sequence))
            .collect::<Vec<_>>();
        if let Some(processes) = self.processes.as_mut() {
            for (worker_id, next_sequence) in sequence_cursors {
                processes
                    .sync_external_next_sequence(worker_id, next_sequence)
                    .map_err(RuntimeError::Lifecycle)?;
            }
        }
        let initial_events = self
            .processes
            .as_mut()
            .expect("process supervisor exists")
            .begin_shutdown_at(Instant::now())
            .map_err(RuntimeError::Lifecycle)?;
        for event in &initial_events {
            report_lifecycle_event(event, self.started.elapsed());
        }
        // `begin_shutdown_at` queues Stop through ProcessSupervisor directly, rather than via
        // `stop_worker`, so advance the runtime cursor for each newly requested lifecycle Stop.
        // This is intentionally event-based: a worker that was already stopping did not consume
        // another sequence during this shutdown pass.
        for worker_id in initial_events.iter().filter_map(|event| match event {
            LifecycleEvent::StopRequested { worker_id, .. } => Some(*worker_id),
            _ => None,
        }) {
            if !already_stopping.contains(&worker_id)
                && let Some(worker) = self.workers.get_mut(&worker_id)
            {
                worker.next_control_sequence = worker.next_control_sequence.saturating_add(1);
            }
        }
        let started = Instant::now();
        while self
            .processes
            .as_ref()
            .is_some_and(|processes| processes.worker_count() > 0)
            && started.elapsed() < Duration::from_secs(5)
        {
            let report = self.poll_once(Some(self.config.poll_interval))?;
            report_runtime_observations(&report, self.started.elapsed());
        }
        Ok(())
    }

    pub(super) fn poll_processes(
        &mut self,
        report: &mut RuntimePollReport,
    ) -> Result<(), RuntimeError> {
        let now = Instant::now();
        if self
            .last_process_poll
            .is_some_and(|last| now.duration_since(last) < Duration::from_millis(100))
        {
            return Ok(());
        }
        self.last_process_poll = Some(now);
        if self.processes.is_none() {
            return Ok(());
        }
        let events = self
            .processes
            .as_mut()
            .expect("process supervisor exists")
            .poll_at(now)
            .map_err(RuntimeError::Lifecycle)?;
        for event in events {
            let terminal = matches!(
                &event,
                LifecycleEvent::Failed { .. }
                    | LifecycleEvent::Stopped { .. }
                    | LifecycleEvent::ChildReaped { .. }
                    | LifecycleEvent::Cleaned { .. }
            );
            let worker_id = match &event {
                LifecycleEvent::Spawned { worker_id, .. }
                | LifecycleEvent::ManifestSent { worker_id }
                | LifecycleEvent::Ready { worker_id }
                | LifecycleEvent::HeartbeatSuspect { worker_id }
                | LifecycleEvent::HeartbeatRecovered { worker_id }
                | LifecycleEvent::ExitReceived { worker_id, .. }
                | LifecycleEvent::ChildReaped { worker_id, .. }
                | LifecycleEvent::Failed { worker_id, .. }
                | LifecycleEvent::StopRequested { worker_id, .. }
                | LifecycleEvent::StopSent { worker_id, .. }
                | LifecycleEvent::ForcedStop { worker_id }
                | LifecycleEvent::Stopped { worker_id, .. }
                | LifecycleEvent::RestartScheduled { worker_id, .. }
                | LifecycleEvent::RestartExhausted { worker_id }
                | LifecycleEvent::Cleaned { worker_id }
                | LifecycleEvent::Control { worker_id, .. }
                | LifecycleEvent::ResultReceived { worker_id, .. } => *worker_id,
            };
            report.lifecycle_events.push(event);
            if terminal {
                self.cleanup_worker(worker_id);
                self.reclaim_terminal_allocations(worker_id);
            }
        }
        if !self.shutting_down {
            let external_restart_ids = self
                .processes
                .as_ref()
                .expect("process supervisor exists")
                .due_external_restart_ids(now);
            for worker_id in external_restart_ids {
                let spec = self
                    .processes
                    .as_mut()
                    .expect("process supervisor exists")
                    .take_due_external_restart(worker_id, now)
                    .map_err(RuntimeError::Lifecycle)?;
                if let Some(spec) = spec {
                    let events = self.spawn_worker(spec)?;
                    report.lifecycle_events.extend(events);
                }
            }
        }
        Ok(())
    }

    /// Fail a worker that announced a terminal Result but never supplied the packet-stream EOF
    /// barrier.  Normal completion never waits on a sleep: `handle_packet` invokes
    /// `maybe_complete_match_result` as soon as EOF is observed.  This bounded deadline only
    /// prevents a stuck worker from retaining routes/capabilities indefinitely.
    pub(super) fn expire_result_packet_drains(&mut self) {
        let now = Instant::now();
        let expired = self
            .workers
            .iter()
            .filter_map(|(worker_id, worker)| {
                worker
                    .result_drain_deadline
                    .is_some_and(|deadline| now >= deadline && !worker.packet_eof)
                    .then_some(*worker_id)
            })
            .collect::<Vec<_>>();
        for worker_id in expired {
            self.core.note_error(RoutingErrorCategory::IpcPacketClosed);
            self.cleanup_worker(worker_id);
        }
    }

    /// Release request records once their worker lifecycle is terminal. Rejected records are
    /// reclaimed earlier, after their response enters the bounded control queue; successful
    /// records remain only until the match Result/Exit handshake has been reconciled.
    pub(super) fn reclaim_terminal_allocations(&mut self, worker_id: WorkerId) {
        let lobby_requests = self
            .allocations
            .iter()
            .filter_map(|(request_id, record)| {
                (record.lobby_worker_id == worker_id)
                    .then_some((*request_id, record.match_worker_id))
            })
            .collect::<Vec<_>>();
        if !lobby_requests.is_empty() {
            // A dead lobby cannot receive a pending response. Any match it launched is orphaned
            // and is stopped through the same bounded cleanup path before its record is dropped.
            let orphaned_matches = lobby_requests
                .iter()
                .filter_map(|(_, match_worker_id)| *match_worker_id)
                .collect::<Vec<_>>();
            for (request_id, _) in lobby_requests {
                self.allocations.remove(&request_id);
            }
            for match_worker_id in orphaned_matches {
                self.cleanup_worker(match_worker_id);
            }
        }

        let match_requests = self
            .allocations
            .iter()
            .filter_map(|(request_id, record)| {
                (record.match_worker_id == Some(worker_id)).then_some(*request_id)
            })
            .collect::<Vec<_>>();
        for request_id in match_requests {
            let Some(record) = self.allocations.get(&request_id) else {
                continue;
            };
            if record.result.is_some() || record.response_queued {
                self.allocations.remove(&request_id);
            } else {
                self.reject_allocation_terminal(request_id, ALLOCATION_REJECT_INTERNAL);
            }
        }
    }

    pub(super) fn reject_allocation_terminal(&mut self, request_id: RequestId, reason: u16) {
        let Some(record) = self.allocations.get_mut(&request_id) else {
            return;
        };
        if record.response_queued || record.result.is_some() {
            return;
        }
        record.response = Some(ControlBody::AllocationRejected(AllocationRejectedBody {
            request_id,
            reason,
            retry_after_ms: 0,
        }));
        record.response_queued = false;
        record.match_worker_id = None;
        record.allocation_id = None;
        record.match_id = None;
    }

    /// Publish the default lobby route only after the process supervisor has accepted Ready.
    /// `Ready` can only be observed through the attached control stream, so this also establishes
    /// the IPC-attachment half of the admission invariant. Early public datagrams therefore fail
    /// closed in `SupervisorCore` instead of reaching `dispatch_queues` with missing channels.
    pub(super) fn activate_ready_routes(&mut self, report: &mut RuntimePollReport) {
        let ready_workers = report
            .lifecycle_events
            .iter()
            .filter_map(|event| match event {
                LifecycleEvent::Ready { worker_id } => Some(*worker_id),
                _ => None,
            })
            .collect::<Vec<_>>();
        for worker_id in ready_workers {
            let Some(route) = self
                .workers
                .get_mut(&worker_id)
                .and_then(|worker| worker.pending_default_route.take())
            else {
                continue;
            };
            if let Err(category) = self.core.register_route(route) {
                self.core.note_error(category);
                self.cleanup_worker(worker_id);
            }
        }
    }

    pub(super) fn accept_stream(
        &mut self,
        worker_id: WorkerId,
        packet: bool,
    ) -> Result<(), RuntimeError> {
        let accepted = {
            let worker = self
                .workers
                .get_mut(&worker_id)
                .ok_or(RuntimeError::Routing(
                    RoutingErrorCategory::ManifestIdentity,
                ))?;
            let Some(listeners) = worker.listeners.as_mut() else {
                return Ok(());
            };
            let listener = if packet {
                listeners.packet_listener_mut()
            } else {
                listeners.control_listener_mut()
            };
            match listener.accept() {
                Ok((stream, _)) => Some(stream),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => None,
                Err(error) => return Err(RuntimeError::Io(error)),
            }
        };
        if let Some(stream) = accepted {
            let pair = {
                let worker = self.workers.get_mut(&worker_id).expect("worker exists");
                if packet {
                    worker.pending_packet = Some(stream);
                } else {
                    worker.pending_control = Some(stream);
                }
                worker
                    .pending_packet
                    .as_ref()
                    .zip(worker.pending_control.as_ref())
                    .is_some()
            };
            if pair {
                let (packet_stream, control_stream) = {
                    let worker = self.workers.get_mut(&worker_id).expect("worker exists");
                    (
                        worker.pending_packet.take().expect("pair checked"),
                        worker.pending_control.take().expect("pair checked"),
                    )
                };
                self.attach_worker_channels(
                    worker_id,
                    UnixWorkerChannels::new(packet_stream, control_stream),
                )?;
                self.queue_worker_manifest(worker_id)?;
            }
        }
        Ok(())
    }

    pub(super) fn queue_worker_manifest(
        &mut self,
        worker_id: WorkerId,
    ) -> Result<(), RuntimeError> {
        let Some(manifest) = self
            .processes
            .as_ref()
            .and_then(|processes| processes.worker_manifest(worker_id))
        else {
            return Ok(());
        };
        let Some(worker) = self.workers.get_mut(&worker_id) else {
            return Err(RuntimeError::Routing(
                RoutingErrorCategory::ManifestIdentity,
            ));
        };
        let Some(channels) = worker.channels.as_mut() else {
            return Err(RuntimeError::Routing(
                RoutingErrorCategory::IpcControlClosed,
            ));
        };
        channels
            .enqueue_control(&manifest)
            .map_err(|error| RuntimeError::Ipc {
                worker_id,
                channel: IpcChannel::Control,
                error,
            })?;
        self.metrics
            .ipc_to_worker
            .observe_ipc_frame(manifest.len().saturating_add(4));
        // ProcessSupervisor's immutable Manifest always owns sequence 1. Runtime-generated
        // controls share that same supervisor-to-worker sequence space, so the first grant,
        // rejection, peer close, or stop must begin at 2.
        worker.next_control_sequence = worker.next_control_sequence.max(2);
        self.processes
            .as_mut()
            .expect("process supervisor exists for worker manifest")
            .mark_manifest_sent(worker_id)
            .map_err(RuntimeError::Lifecycle)?;
        Ok(())
    }

    /// A validated match Result is the worker's terminal gameplay fact. Tear down only that
    /// match's routes/capabilities, then send one graceful Stop through the lifecycle owner so
    /// the worker can emit Exit after Result. The lobby and any unrelated worker stay live.
    pub(super) fn complete_match_result(
        &mut self,
        worker_id: WorkerId,
        result: crate::ResultBody,
    ) -> Result<(), RuntimeError> {
        let Some(request_id) = self.allocations.iter().find_map(|(request_id, record)| {
            (record.match_worker_id == Some(worker_id)
                && record.match_id == Some(result.match_id)
                && record.allocation_id == Some(result.allocation_id))
            .then_some(*request_id)
        }) else {
            return Err(RuntimeError::Routing(
                RoutingErrorCategory::WorkerProtocolConflict,
            ));
        };
        let record = self
            .allocations
            .get_mut(&request_id)
            .expect("allocation request was just found");
        if record.result.is_some() {
            return Err(RuntimeError::Routing(
                RoutingErrorCategory::WorkerProtocolConflict,
            ));
        }
        record.result = Some(result);
        let worker = self
            .workers
            .get_mut(&worker_id)
            .ok_or(RuntimeError::Routing(
                RoutingErrorCategory::WorkerProtocolConflict,
            ))?;
        worker.result_received = true;
        worker.result_drain_deadline = Some(Instant::now() + RESULT_PACKET_DRAIN_TIMEOUT);
        // The Result control stream and gameplay packet stream are independent. Keep the route
        // and capability registry alive until the worker packet write half closes; otherwise a
        // control frame that overtakes a final BRPK frame would discard that gameplay packet.
        self.maybe_complete_match_result(worker_id)
    }

    pub(super) fn maybe_complete_match_result(
        &mut self,
        worker_id: WorkerId,
    ) -> Result<(), RuntimeError> {
        let should_teardown = self.workers.get(&worker_id).is_some_and(|worker| {
            worker.packet_eof && worker.result_received && !worker.result_teardown_started
        });
        if !should_teardown {
            return Ok(());
        }
        let worker = self
            .workers
            .get_mut(&worker_id)
            .expect("result worker exists");
        worker.result_teardown_started = true;
        // EOF is the explicit drain barrier: every complete BRPK frame written by the worker is
        // already readable and has been routed before this registry cleanup drops the route.
        let _ = self.core.cleanup_worker(worker_id);
        // Bevy-free in-memory runtimes have no lifecycle owner to receive Stop. They still need
        // the same route teardown semantics for deterministic transport tests.
        if self.processes.is_none() || self.shutting_down {
            return Ok(());
        }
        let stop_id = StopId::new(random_u64()?).ok_or(RuntimeError::Routing(
            RoutingErrorCategory::SupervisorInternal,
        ))?;
        self.stop_worker(worker_id, stop_id, 0)?;
        Ok(())
    }

    /// Isolate a decoded worker fact whose semantics do not match this worker's admitted
    /// identity.  The process lifecycle owner still receives the exact failure category so a
    /// lobby can follow its bounded restart policy; route and capability cleanup is restricted to
    /// this worker.  Supervisor invariants intentionally do not use this path.
    pub(super) fn fail_worker_control(
        &mut self,
        worker_id: WorkerId,
        category: RoutingErrorCategory,
        report: &mut RuntimePollReport,
    ) -> Result<(), RuntimeError> {
        self.core.note_error(category);
        if let Some(processes) = self.processes.as_mut()
            && processes.worker_phase(worker_id).is_some()
        {
            let events = processes
                .fail_worker(worker_id, category)
                .map_err(RuntimeError::Lifecycle)?;
            report.lifecycle_events.extend(events);
        }
        self.cleanup_worker(worker_id);
        Ok(())
    }
}
