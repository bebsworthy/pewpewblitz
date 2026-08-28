//! Bounded public UDP and worker packet transport for the single Mio owner loop.

use std::{io, net::SocketAddr, time::Instant};

use mio::Interest;

use crate::{
    PacketDirection, PacketRecord, PublicEnvelope, RouteId, RoutingErrorCategory,
    UnixWorkerChannels, WorkerId, WorkerKind,
};

use super::{
    PUBLIC_TOKEN, PendingPublicDatagram, RuntimeError, RuntimePollReport, SupervisorRuntime,
};

impl SupervisorRuntime {
    pub(super) fn receive_public(
        &mut self,
        report: &mut RuntimePollReport,
    ) -> Result<(), RuntimeError> {
        for _ in 0..self.config.udp_burst {
            let (length, source) = match self.public.recv_from(&mut self.incoming) {
                Ok(value) => value,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(RuntimeError::Io(error)),
            };
            report.public_received += 1;
            let public_received_at = Instant::now();
            self.metrics.public_ingress.observe_datagram(length);
            let now = self.now();
            if self.ingress.is_suppressed(source, now) {
                report.public_dropped += 1;
                self.core.note_error(RoutingErrorCategory::SourceLimited);
                continue;
            }
            let envelope = match PublicEnvelope::decode(&self.incoming[..length]) {
                Ok(envelope) => envelope,
                Err(error) => {
                    report.public_dropped += 1;
                    let category = match error {
                        crate::CodecError::Oversize => RoutingErrorCategory::PublicOversize,
                        crate::CodecError::UnsupportedVersion(_)
                        | crate::CodecError::UnsupportedType(_) => {
                            RoutingErrorCategory::PublicUnsupported
                        }
                        _ => RoutingErrorCategory::PublicMalformed,
                    };
                    self.core.note_error(category);
                    if self.ingress.record_malformed(source, now)
                        == crate::IngressDecision::Suppressed
                    {
                        self.core.note_error(RoutingErrorCategory::SourceLimited);
                    }
                    continue;
                }
            };
            self.metrics.public_ingress.observe_frame();
            self.metrics
                .inner_ingress
                .observe_datagram(envelope.payload().len());
            self.metrics.inner_ingress.observe_frame();
            // The lobby route is published only after the worker's Ready handshake.  A client
            // may legitimately send Netcode handshake retries in that startup window; dropping
            // them here keeps the pre-auth limiter reserved for packets that can actually reach
            // an admitted route, while retaining the exact 8-datagram/9-KiB budget thereafter.
            // Boundary counters still include these valid envelopes so public/inner accounting
            // remains an exact per-datagram relation even when readiness races the first retry.
            if matches!(envelope.selector(), crate::RouteSelector::DefaultLobby)
                && !self.core.default_lobby_ready()
            {
                report.public_dropped += 1;
                continue;
            }
            if matches!(envelope.selector(), crate::RouteSelector::DefaultLobby)
                && self.ingress.admit_default(source, length, now)
                    != crate::IngressDecision::Allowed
            {
                report.public_dropped += 1;
                self.core.note_error(RoutingErrorCategory::SourceLimited);
                continue;
            }
            let capability_selector =
                matches!(envelope.selector(), crate::RouteSelector::Capability(_));
            let routed = self.core.route_public(&envelope, source, now);
            if let Ok(route) = routed {
                if self
                    .workers
                    .get(&route.worker_id)
                    .is_some_and(|worker| worker.registration.kind == WorkerKind::Match)
                {
                    self.metrics
                        .match_inner_ingress
                        .observe_datagram(envelope.payload().len());
                    self.metrics.match_inner_ingress.observe_frame();
                }
                if capability_selector {
                    self.ingress.promote_authenticated(source, now);
                }
                self.packet_enqueue_started
                    .entry(route.route_id)
                    .or_default()
                    .push_back(public_received_at);
            } else {
                report.public_dropped += 1;
            }
        }
        Ok(())
    }

    pub(super) fn handle_packet(
        &mut self,
        worker_id: WorkerId,
        readable: bool,
        writable: bool,
        report: &mut RuntimePollReport,
    ) -> Result<(), RuntimeError> {
        let (mut failed, packet_eof) = if readable {
            self.read_worker_packets(worker_id, report)?
        } else {
            (false, false)
        };
        if packet_eof && !failed {
            if let Some(worker) = self.workers.get_mut(&worker_id) {
                worker.packet_eof = true;
            }
            self.maybe_complete_match_result(worker_id)?;
        }
        if writable && !failed {
            let result = self
                .workers
                .get_mut(&worker_id)
                .expect("token worker exists")
                .channels
                .as_mut()
                .expect("packet channel exists")
                .flush_packet(self.config.packet_burst);
            if result.is_err() {
                failed = true;
            }
        }
        if failed {
            self.cleanup_worker(worker_id);
        } else {
            self.update_worker_interest(worker_id)?;
        }
        Ok(())
    }
    fn read_worker_packets(
        &mut self,
        worker_id: WorkerId,
        report: &mut RuntimePollReport,
    ) -> Result<(bool, bool), RuntimeError> {
        let mut failed;
        let mut packet_eof = false;
        let match_worker = self
            .workers
            .get(&worker_id)
            .is_some_and(|worker| worker.registration.kind == WorkerKind::Match);
        let lifecycle_owned = self
            .processes
            .as_ref()
            .is_some_and(|processes| processes.worker_phase(worker_id).is_some());
        let result = {
            let worker = self
                .workers
                .get_mut(&worker_id)
                .expect("token worker exists");
            worker
                .channels
                .as_mut()
                .ok_or(RuntimeError::Routing(RoutingErrorCategory::IpcPacketClosed))?
                .packet_read_ready(self.config.packet_burst)
        };
        match result {
            Ok(progress) => {
                self.metrics
                    .ipc_from_worker
                    .observe_ipc_read(progress.bytes_read, progress.records.len());
                // For lifecycle-owned workers, process reconciliation is the authority for
                // EOF versus a valid Exit. Do not kill a child merely because its packet
                // half closes in the same turn as its typed Exit control frame.
                failed = progress.eof && !lifecycle_owned;
                for raw in progress.records {
                    let worker_packet_started = Instant::now();
                    let packet =
                        match PacketRecord::decode(&raw, PacketDirection::WorkerToSupervisor) {
                            Ok(packet) => packet,
                            Err(error) => {
                                self.core.note_error(RoutingErrorCategory::IpcMalformed);
                                let _ = error;
                                failed = true;
                                break;
                            }
                        };
                    if packet.worker_id != worker_id {
                        self.core.note_error(RoutingErrorCategory::ManifestIdentity);
                        failed = true;
                        break;
                    }
                    let Ok(destination) = self.core.accept_worker_packet(&packet) else {
                        failed = true;
                        break;
                    };
                    let Some(selector) = self.core.public_selector_for_route(packet.route_id)
                    else {
                        self.core.note_error(RoutingErrorCategory::Binding);
                        failed = true;
                        break;
                    };
                    let inner_bytes = packet.payload.len();
                    let envelope = PublicEnvelope::new(selector, packet.payload)
                        .map_err(|_| RuntimeError::Routing(RoutingErrorCategory::IpcMalformed))?
                        .encode()
                        .map_err(|_| RuntimeError::Routing(RoutingErrorCategory::IpcMalformed))?;
                    if self.enqueue_public_datagram_with_metadata(
                        envelope,
                        destination,
                        inner_bytes,
                        match_worker,
                        worker_packet_started,
                    ) {
                        report.packets_to_public += 1;
                    }
                }
                if progress.eof {
                    // EOF is the explicit worker-side packet drain barrier.  A partial
                    // framed record cannot be silently accepted as a terminal success.
                    let buffered = self
                        .workers
                        .get(&worker_id)
                        .and_then(|worker| worker.channels.as_ref())
                        .map_or(0, UnixWorkerChannels::packet_buffered_bytes);
                    if buffered != 0 {
                        self.core.note_error(RoutingErrorCategory::IpcMalformed);
                        failed = true;
                    } else {
                        packet_eof = true;
                    }
                }
            }
            Err(error) => {
                let _ = error;
                failed = true;
            }
        }
        Ok((failed, packet_eof))
    }
    pub(super) fn dispatch_queues(
        &mut self,
        report: &mut RuntimePollReport,
    ) -> Result<(), RuntimeError> {
        for packet in self.core.drain_packets(self.config.packet_burst) {
            let worker_id = packet.worker_id;
            let public_received_at = self.take_packet_enqueue_started(packet.route_id);
            if self
                .workers
                .get(&worker_id)
                .is_some_and(|worker| worker.result_received)
            {
                // A terminal Result stops new client intent immediately, but worker-to-public
                // packets remain routable until the packet EOF drain barrier is observed.
                continue;
            }
            if self.core.is_default_template(packet.route_id) {
                self.core.note_error(RoutingErrorCategory::Binding);
                continue;
            }
            let encoded = packet
                .encode()
                .map_err(|_| RuntimeError::Routing(RoutingErrorCategory::IpcMalformed))?;
            let Some(channels) = self
                .workers
                .get_mut(&worker_id)
                .and_then(|worker| worker.channels.as_mut())
            else {
                // Route publication is readiness-gated, but keep this owner loop fail-closed if
                // a future route transition races cleanup: an early packet is dropped rather
                // than turning a missing IPC attachment into a supervisor-wide poll failure.
                self.core.note_error(RoutingErrorCategory::IpcPacketClosed);
                continue;
            };
            let result = channels.enqueue_packet(&encoded);
            if result.is_err() {
                self.cleanup_worker(worker_id);
            } else {
                self.metrics
                    .ipc_to_worker
                    .observe_ipc_frame(encoded.len().saturating_add(4));
                if let Some(public_received_at) = public_received_at {
                    self.metrics
                        .public_receive_to_packet_ipc_enqueue
                        .observe(public_received_at.elapsed());
                }
                report.packets_to_workers += 1;
            }
        }
        for (worker_id, record) in self.core.drain_controls(self.config.control_burst) {
            let Some(channels) = self
                .workers
                .get_mut(&worker_id)
                .and_then(|worker| worker.channels.as_mut())
            else {
                self.core.note_error(RoutingErrorCategory::IpcControlClosed);
                continue;
            };
            let result = channels.enqueue_control(&record);
            if result.is_err() {
                self.cleanup_worker(worker_id);
            } else {
                self.metrics
                    .ipc_to_worker
                    .observe_ipc_frame(record.len().saturating_add(4));
                report.controls_to_workers += 1;
            }
        }
        Ok(())
    }

    pub(super) fn take_packet_enqueue_started(&mut self, route_id: RouteId) -> Option<Instant> {
        let started = self
            .packet_enqueue_started
            .get_mut(&route_id)
            .and_then(std::collections::VecDeque::pop_front);
        if self
            .packet_enqueue_started
            .get(&route_id)
            .is_some_and(std::collections::VecDeque::is_empty)
        {
            self.packet_enqueue_started.remove(&route_id);
        }
        started
    }

    pub(super) fn flush_workers(
        &mut self,
        _report: &mut RuntimePollReport,
    ) -> Result<(), RuntimeError> {
        let worker_ids = self.workers.keys().copied().collect::<Vec<_>>();
        for worker_id in worker_ids {
            let (failed, control_drained) = {
                let Some(worker) = self.workers.get_mut(&worker_id) else {
                    continue;
                };
                let Some(channels) = worker.channels.as_mut() else {
                    continue;
                };
                let failed = channels.flush_packet(self.config.packet_burst).is_err()
                    || channels.flush_control(self.config.control_burst).is_err();
                (failed, !channels.control_pending())
            };
            if failed {
                self.cleanup_worker(worker_id);
            } else if self.workers.contains_key(&worker_id) {
                if control_drained
                    && let Some(processes) = self.processes.as_mut()
                    && processes
                        .mark_external_stop_sent(worker_id)
                        .map_err(RuntimeError::Lifecycle)?
                {
                    // The lifecycle owner queues StopSent behind StopRequested and publishes both
                    // on its next poll, preserving the actual causal order in evidence logs.
                }
                self.update_worker_interest(worker_id)?;
            }
        }
        Ok(())
    }

    pub(super) fn flush_public(
        &mut self,
        _report: &mut RuntimePollReport,
    ) -> Result<(), RuntimeError> {
        for _ in 0..self.config.udp_burst {
            let Some(pending) = self.outgoing.front() else {
                break;
            };
            match self.public.send_to(&pending.bytes, pending.destination) {
                Ok(_) => {
                    if let Some(pending) = self.outgoing.pop_front() {
                        self.outgoing_bytes =
                            self.outgoing_bytes.saturating_sub(pending.bytes.len());
                        self.metrics
                            .public_egress
                            .observe_datagram(pending.bytes.len());
                        self.metrics.public_egress.observe_frame();
                        self.metrics
                            .inner_egress
                            .observe_datagram(pending.inner_bytes);
                        self.metrics.inner_egress.observe_frame();
                        if pending.match_worker {
                            self.metrics
                                .match_inner_egress
                                .observe_datagram(pending.inner_bytes);
                            self.metrics.match_inner_egress.observe_frame();
                        }
                        self.metrics
                            .worker_packet_to_public_send
                            .observe(pending.worker_packet_started.elapsed());
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(RuntimeError::Io(error)),
            }
        }
        let interest = if self.outgoing.is_empty() {
            Interest::READABLE
        } else {
            Interest::READABLE.add(Interest::WRITABLE)
        };
        self.poll
            .registry()
            .reregister(&mut self.public, PUBLIC_TOKEN, interest)?;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn enqueue_public_datagram(
        &mut self,
        bytes: Vec<u8>,
        destination: SocketAddr,
    ) -> bool {
        self.enqueue_public_datagram_with_metadata(bytes, destination, 0, false, Instant::now())
    }

    pub(super) fn enqueue_public_datagram_with_metadata(
        &mut self,
        bytes: Vec<u8>,
        destination: SocketAddr,
        inner_bytes: usize,
        match_worker: bool,
        worker_packet_started: Instant,
    ) -> bool {
        if self.outgoing.len() >= crate::GLOBAL_PACKET_QUEUE_FRAMES
            || self.outgoing_bytes.saturating_add(bytes.len()) > crate::GLOBAL_PACKET_QUEUE_BYTES
        {
            self.core.note_error(RoutingErrorCategory::PacketQueueFull);
            return false;
        }
        self.outgoing_bytes = self.outgoing_bytes.saturating_add(bytes.len());
        self.outgoing.push_back(PendingPublicDatagram {
            bytes,
            destination,
            inner_bytes,
            match_worker,
            worker_packet_started,
        });
        true
    }

    pub(super) fn update_worker_interest(
        &mut self,
        worker_id: WorkerId,
    ) -> Result<(), RuntimeError> {
        let Some(worker) = self.workers.get_mut(&worker_id) else {
            return Ok(());
        };
        let Some(channels) = worker.channels.as_mut() else {
            return Ok(());
        };
        if let Some(token) = worker.packet_token {
            let interest = if channels.packet_pending() {
                Interest::READABLE.add(Interest::WRITABLE)
            } else {
                Interest::READABLE
            };
            self.poll
                .registry()
                .reregister(channels.packet_source_mut(), token, interest)?;
        }
        if let Some(token) = worker.control_token {
            let interest = if channels.control_pending() {
                Interest::READABLE.add(Interest::WRITABLE)
            } else {
                Interest::READABLE
            };
            self.poll
                .registry()
                .reregister(channels.control_source_mut(), token, interest)?;
        }
        Ok(())
    }
}
