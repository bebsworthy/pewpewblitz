//! Typed worker-control sequencing and dispatch inside the owner loop.

use std::time::Instant;

use crate::{
    AllocationId, ControlBody, ControlFrame, LifecycleEvent, MatchId, RequestId,
    RoutingErrorCategory, WorkerId,
};

use super::{
    RuntimeError, RuntimePollReport, SupervisorRuntime, WorkerControlDisposition,
    runtime_worker_failure_category, worker_control_failure_category,
};

impl SupervisorRuntime {
    pub(super) fn handle_control(
        &mut self,
        worker_id: WorkerId,
        readable: bool,
        writable: bool,
        report: &mut RuntimePollReport,
    ) -> Result<(), RuntimeError> {
        let mut failed = false;
        if readable {
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
                    .ok_or(RuntimeError::Routing(
                        RoutingErrorCategory::IpcControlClosed,
                    ))?
                    .control_read_ready(self.config.control_burst)
            };
            match result {
                Ok(progress) => {
                    self.metrics
                        .ipc_from_worker
                        .observe_ipc_read(progress.bytes_read, progress.records.len());
                    failed = progress.eof && !lifecycle_owned;
                    let mut received_exit = false;
                    for raw in progress.records {
                        let Some(frame) = self.decode_worker_control(worker_id, &raw) else {
                            self.core.note_error(RoutingErrorCategory::IpcMalformed);
                            failed = true;
                            break;
                        };
                        match self.process_worker_control(worker_id, frame, report)? {
                            WorkerControlDisposition::Continue => {}
                            WorkerControlDisposition::ExitReceived => received_exit = true,
                            WorkerControlDisposition::Failed => {
                                failed = true;
                                break;
                            }
                        }
                    }
                    // Keep the channels registered for one more owner turn after a valid Exit.
                    // ProcessSupervisor reconciles the already-observed body with try_wait; an
                    // EOF without Exit remains an immediate IPC failure.
                    if received_exit {
                        failed = false;
                    }
                }
                Err(error) => {
                    let _ = error;
                    failed = true;
                }
            }
        }
        if writable && !failed {
            let result = self
                .workers
                .get_mut(&worker_id)
                .expect("token worker exists")
                .channels
                .as_mut()
                .expect("control channel exists")
                .flush_control(self.config.control_burst);
            if result.is_err() {
                failed = true;
            }
        }
        if failed {
            self.cleanup_worker(worker_id);
        } else {
            self.update_worker_interest(worker_id)?;
        }
        report.controls_to_workers += 0;
        Ok(())
    }

    fn decode_worker_control(&self, worker_id: WorkerId, raw: &[u8]) -> Option<ControlFrame> {
        let worker = self.workers.get(&worker_id).expect("worker exists");
        ControlFrame::decode_for(raw, worker.registration.process_id, worker_id).ok()
    }

    fn process_worker_control(
        &mut self,
        worker_id: WorkerId,
        frame: ControlFrame,
        report: &mut RuntimePollReport,
    ) -> Result<WorkerControlDisposition, RuntimeError> {
        let body = frame.body.clone();
        let received_exit = match self.observe_worker_control(worker_id, frame, report)? {
            WorkerControlDisposition::Continue => false,
            WorkerControlDisposition::ExitReceived => true,
            WorkerControlDisposition::Failed => return Ok(WorkerControlDisposition::Failed),
        };
        if self.dispatch_worker_control_body(worker_id, body, report)?
            == WorkerControlDisposition::Failed
        {
            return Ok(WorkerControlDisposition::Failed);
        }
        Ok(if received_exit {
            WorkerControlDisposition::ExitReceived
        } else {
            WorkerControlDisposition::Continue
        })
    }

    fn observe_worker_control(
        &mut self,
        worker_id: WorkerId,
        frame: ControlFrame,
        report: &mut RuntimePollReport,
    ) -> Result<WorkerControlDisposition, RuntimeError> {
        let Some(processes) = self.processes.as_mut() else {
            let worker = self.workers.get_mut(&worker_id).expect("worker exists");
            if worker.control_sequences.observe(frame).is_err() {
                self.core.note_error(RoutingErrorCategory::IpcMalformed);
                return Ok(WorkerControlDisposition::Failed);
            }
            return Ok(WorkerControlDisposition::Continue);
        };
        match processes.observe_control_frame(worker_id, &frame, Instant::now()) {
            Ok(events) => {
                let received_exit = events
                    .iter()
                    .any(|event| matches!(event, LifecycleEvent::ExitReceived { .. }));
                report.lifecycle_events.extend(events);
                Ok(if received_exit {
                    WorkerControlDisposition::ExitReceived
                } else {
                    WorkerControlDisposition::Continue
                })
            }
            Err(error) => {
                let Some(category) = worker_control_failure_category(&error) else {
                    return Err(RuntimeError::Lifecycle(error));
                };
                self.fail_worker_control(worker_id, category, report)?;
                Ok(WorkerControlDisposition::Failed)
            }
        }
    }

    fn dispatch_worker_control_body(
        &mut self,
        worker_id: WorkerId,
        body: ControlBody,
        report: &mut RuntimePollReport,
    ) -> Result<WorkerControlDisposition, RuntimeError> {
        let result = match body {
            ControlBody::AllocateRequest(request) if !self.shutting_down => self
                .accept_allocation_request(worker_id, request, report)
                .map(|()| true),
            ControlBody::Result(result) => {
                self.complete_match_result(worker_id, result).map(|()| true)
            }
            ControlBody::PeerClose(close) => self
                .handle_peer_close(worker_id, close, report)
                .map(|()| true),
            ControlBody::LobbyAuthenticated(fact) => {
                self.promote_lobby_source(worker_id, fact).map(|()| true)
            }
            ControlBody::LobbyNetcodeAuthenticated(fact) => self
                .promote_lobby_netcode_source(worker_id, fact)
                .map(|()| true),
            ControlBody::CancelActivation(fact) => self.forward_cancel_activation(worker_id, fact),
            ControlBody::Activated(fact) => self.forward_activated(worker_id, fact),
            ControlBody::StartFailed(fact) => self.forward_start_failed(worker_id, fact),
            _ => Ok(true),
        };
        match result {
            Ok(true) => Ok(WorkerControlDisposition::Continue),
            Ok(false) => Ok(WorkerControlDisposition::Failed),
            Err(error) => {
                let Some(category) = runtime_worker_failure_category(&error) else {
                    return Err(error);
                };
                self.fail_worker_control(worker_id, category, report)?;
                Ok(WorkerControlDisposition::Failed)
            }
        }
    }

    fn handle_peer_close(
        &mut self,
        worker_id: WorkerId,
        close: crate::PeerCloseBody,
        report: &mut RuntimePollReport,
    ) -> Result<(), RuntimeError> {
        if self
            .core
            .close_route_from_worker(worker_id, close.route_id, close.peer_id)
            .map_err(RuntimeError::Routing)?
            .is_some()
        {
            report.routes_torn_down += 1;
        }
        Ok(())
    }

    fn promote_lobby_source(
        &mut self,
        worker_id: WorkerId,
        fact: crate::LobbyAuthenticatedBody,
    ) -> Result<(), RuntimeError> {
        let source = self
            .core
            .authenticated_lobby_source(worker_id, fact)
            .map_err(RuntimeError::Routing)?;
        self.ingress.promote_authenticated(source, self.now());
        Ok(())
    }

    fn promote_lobby_netcode_source(
        &mut self,
        worker_id: WorkerId,
        fact: crate::LobbyNetcodeAuthenticatedBody,
    ) -> Result<(), RuntimeError> {
        let source = self
            .core
            .authenticated_lobby_netcode_source(worker_id, fact)
            .map_err(RuntimeError::Routing)?;
        self.ingress.promote_authenticated(source, self.now());
        Ok(())
    }

    fn forward_cancel_activation(
        &mut self,
        worker_id: WorkerId,
        fact: crate::ActivationBody,
    ) -> Result<bool, RuntimeError> {
        let allocation = self.allocations.get(&fact.request_id).and_then(|record| {
            (record.allocation_id == Some(fact.allocation_id)
                && record.match_id == Some(fact.match_id)
                && (record.lobby_worker_id == worker_id
                    || record.match_worker_id == Some(worker_id)))
            .then(|| {
                record
                    .match_worker_id
                    .map(|match_worker_id| (record.lobby_worker_id, match_worker_id))
            })
            .flatten()
        });
        let Some((lobby_worker_id, match_worker_id)) = allocation else {
            return Err(RuntimeError::Routing(
                RoutingErrorCategory::WorkerProtocolConflict,
            ));
        };
        let dissolved = ControlBody::ActivationDissolved(fact);
        Ok(self.queue_control_body(lobby_worker_id, dissolved.clone())
            && self.queue_control_body(match_worker_id, dissolved))
    }

    fn allocation_lobby_for_match_fact(
        &self,
        worker_id: WorkerId,
        request_id: RequestId,
        allocation_id: AllocationId,
        match_id: MatchId,
    ) -> Result<WorkerId, RuntimeError> {
        self.allocations
            .get(&request_id)
            .and_then(|record| {
                (record.match_worker_id == Some(worker_id)
                    && record.allocation_id == Some(allocation_id)
                    && record.match_id == Some(match_id))
                .then_some(record.lobby_worker_id)
            })
            .ok_or(RuntimeError::Routing(
                RoutingErrorCategory::WorkerProtocolConflict,
            ))
    }

    fn forward_activated(
        &mut self,
        worker_id: WorkerId,
        fact: crate::ActivationBody,
    ) -> Result<bool, RuntimeError> {
        let lobby_worker_id = self.allocation_lobby_for_match_fact(
            worker_id,
            fact.request_id,
            fact.allocation_id,
            fact.match_id,
        )?;
        Ok(self.queue_control_body(lobby_worker_id, ControlBody::Activated(fact)))
    }

    fn forward_start_failed(
        &mut self,
        worker_id: WorkerId,
        fact: crate::ActivationBody,
    ) -> Result<bool, RuntimeError> {
        let lobby_worker_id = self.allocation_lobby_for_match_fact(
            worker_id,
            fact.request_id,
            fact.allocation_id,
            fact.match_id,
        )?;
        let dissolved = ControlBody::ActivationDissolved(fact);
        Ok(self.queue_control_body(lobby_worker_id, dissolved.clone())
            && self.queue_control_body(worker_id, dissolved))
    }
}
