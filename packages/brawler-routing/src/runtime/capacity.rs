//! Runtime-owned expiry, capacity refresh, and bounded external-control intake.

use crate::{ControlBody, ControlFrame, WorkerKind};

use super::{RuntimePollReport, SupervisorRuntime};

impl SupervisorRuntime {
    pub(super) fn queue_external_controls(&mut self) {
        if self.processes.is_none() {
            return;
        }
        let worker_ids = self.workers.keys().copied().collect::<Vec<_>>();
        for worker_id in worker_ids {
            if self
                .workers
                .get(&worker_id)
                .is_none_or(|worker| worker.channels.is_none())
            {
                continue;
            }
            let records = self
                .processes
                .as_mut()
                .expect("process supervisor exists")
                .take_external_control_records(worker_id);
            for record in records {
                let Some(worker) = self.workers.get_mut(&worker_id) else {
                    continue;
                };
                let Some(channels) = worker.channels.as_mut() else {
                    continue;
                };
                if channels.enqueue_control(&record).is_err() {
                    self.cleanup_worker(worker_id);
                    break;
                }
                self.metrics
                    .ipc_to_worker
                    .observe_ipc_frame(record.len().saturating_add(4));
            }
        }
    }

    /// Publish only the scalar capacity needed by the lobby. Match workers remain counted until
    /// their runtime entry is removed after the lifecycle owner observes actual child reap.
    pub(super) fn refresh_lobby_capacity(&mut self) {
        let match_workers = self.processes.as_ref().map_or_else(
            || {
                self.workers
                    .values()
                    .filter(|worker| worker.registration.kind == WorkerKind::Match)
                    .count()
            },
            |processes| processes.worker_count().saturating_sub(1),
        );
        let Some((lobby_worker_id, manifest_limit, previous, ready)) =
            self.workers.iter().find_map(|(worker_id, worker)| {
                if worker.registration.kind != WorkerKind::Lobby {
                    return None;
                }
                Some((
                    *worker_id,
                    usize::from(worker.match_slot_limit?),
                    worker.last_free_match_slots,
                    worker.pending_default_route.is_none(),
                ))
            })
        else {
            return;
        };
        if !ready {
            return;
        }
        let host_limit = self.config.core.max_workers.saturating_sub(1);
        let free = u8::try_from(
            manifest_limit
                .min(host_limit)
                .saturating_sub(match_workers)
                .min(usize::from(u8::MAX)),
        )
        .expect("free slot value was clamped to u8");
        if previous == Some(free) {
            return;
        }
        if self.queue_control_body(
            lobby_worker_id,
            ControlBody::LobbyCapacity(crate::LobbyCapacityBody {
                free_match_slots: free,
            }),
        ) && let Some(worker) = self.workers.get_mut(&lobby_worker_id)
        {
            worker.last_free_match_slots = Some(free);
        }
    }

    pub(super) fn expire(&mut self, report: &mut RuntimePollReport) {
        let now = self.now();
        self.ingress.expire(now);
        let teardowns = self.core.expire(now);
        if self.shutting_down {
            // Route expiry still revokes routes/capabilities above, but its PeerClose controls
            // must not be appended after lifecycle-owned Stop during global shutdown.
            report.routes_torn_down += teardowns.len();
            return;
        }
        for teardown in teardowns {
            report.routes_torn_down += 1;
            let mut failed = false;
            if let Some(worker) = self.workers.get_mut(&teardown.worker_id)
                && let Some(channels) = worker.channels.as_mut()
            {
                let sequence = worker.next_control_sequence;
                worker.next_control_sequence = worker.next_control_sequence.saturating_add(1);
                let frame = ControlFrame::from_raw_sequence(
                    sequence,
                    worker.registration.process_id,
                    teardown.worker_id,
                    ControlBody::PeerClose(crate::PeerCloseBody {
                        route_id: teardown.route_id,
                        peer_id: teardown.peer_id,
                        reason: 1,
                    }),
                )
                .and_then(|frame| frame.encode());
                match frame {
                    Ok(frame) => {
                        failed = channels.enqueue_control(&frame).is_err();
                        if !failed {
                            self.metrics
                                .ipc_to_worker
                                .observe_ipc_frame(frame.len().saturating_add(4));
                        }
                    }
                    Err(_) => failed = true,
                }
            }
            if failed {
                self.cleanup_worker(teardown.worker_id);
            }
        }
    }
}
