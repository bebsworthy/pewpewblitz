//! Allocation admission, match-worker launch, grant commit, and rejection reclamation.

use std::path::PathBuf;

use crate::{
    AllocateRequestBody, AllocationGrant, AllocationGrantedBody, AllocationId,
    AllocationRejectedBody, CONTROL_VERSION_CURRENT, Capability, CapabilityBinding, ControlBody,
    ControlFrame, Generation, LifecycleEvent, ManifestBody, ManifestCommon, MatchId,
    MatchManifestBot, MatchManifestParticipant, MatchManifestV1, PACKET_VERSION_V1, PeerId,
    ProcessId, ROUTE_VERSION_V1, RequestId, RouteId, RouteRegistration, RoutingErrorCategory,
    SeedPolicy, WorkerId, WorkerKind, WorkerLaunchSpec, WorkerRegistration, WorkerRole,
};

use super::{
    ALLOCATION_REJECT_CAPACITY, ALLOCATION_REJECT_CONFLICT, ALLOCATION_REJECT_INTERNAL,
    ALLOCATION_REJECT_INVALID, AllocationParticipant, AllocationRecord, MAX_TRACKED_ALLOCATIONS,
    RuntimeError, RuntimePollReport, RuntimeTimingEvent, SupervisorRuntime,
    allocation_rejection_for, random_id128, random_u64, random_u128, unix_now_millis,
};

struct AllocationLaunchContext {
    common: ManifestCommon,
    executable: PathBuf,
    allocation_id: AllocationId,
    match_id: MatchId,
    match_worker_id: WorkerId,
    seed: u64,
    heartbeat_ms: u32,
}

struct AllocationIdentities {
    allocation_id: AllocationId,
    match_id: MatchId,
    match_worker_id: WorkerId,
    process_id: ProcessId,
    generation: Generation,
    seed: u64,
}

struct PreparedAllocation {
    request_id: RequestId,
    match_worker_id: WorkerId,
    record: AllocationRecord,
    spec: WorkerLaunchSpec,
}

impl SupervisorRuntime {
    pub(super) fn accept_allocation_request(
        &mut self,
        lobby_worker_id: WorkerId,
        request: AllocateRequestBody,
        report: &mut RuntimePollReport,
    ) -> Result<(), RuntimeError> {
        if !self.admit_allocation_request(lobby_worker_id, &request) {
            return Ok(());
        }
        let Some(context) = self.resolve_allocation_launch_context(lobby_worker_id, &request)?
        else {
            return Ok(());
        };
        let Some(prepared) = self.prepare_allocation(lobby_worker_id, request, context)? else {
            return Ok(());
        };
        self.commit_allocation(prepared, report);
        Ok(())
    }

    fn admit_allocation_request(
        &mut self,
        lobby_worker_id: WorkerId,
        request: &AllocateRequestBody,
    ) -> bool {
        if self.shutting_down {
            return false;
        }
        if let Some(existing) = self.allocations.get(&request.request_id) {
            if existing.request == *request && existing.lobby_worker_id == lobby_worker_id {
                return false;
            }
            self.queue_control_body(
                lobby_worker_id,
                ControlBody::AllocationRejected(AllocationRejectedBody {
                    request_id: request.request_id,
                    reason: ALLOCATION_REJECT_CONFLICT,
                    retry_after_ms: 0,
                }),
            );
            return false;
        }
        if self
            .workers
            .get(&lobby_worker_id)
            .is_none_or(|worker| worker.registration.kind != WorkerKind::Lobby)
        {
            return false;
        }
        if self.allocations.len() >= MAX_TRACKED_ALLOCATIONS {
            self.queue_control_body(
                lobby_worker_id,
                ControlBody::AllocationRejected(AllocationRejectedBody {
                    request_id: request.request_id,
                    reason: ALLOCATION_REJECT_CAPACITY,
                    retry_after_ms: 1_000,
                }),
            );
            return false;
        }
        if crate::validate_product_request(request).is_err() {
            self.insert_rejected_allocation(
                request.clone(),
                lobby_worker_id,
                ALLOCATION_REJECT_INVALID,
            );
            return false;
        }
        true
    }

    fn resolve_allocation_launch_context(
        &mut self,
        lobby_worker_id: WorkerId,
        request: &AllocateRequestBody,
    ) -> Result<Option<AllocationLaunchContext>, RuntimeError> {
        let Some(policy) = self.config.allocation_policy else {
            return Ok(self.reject_allocation_preparation(
                request,
                lobby_worker_id,
                ALLOCATION_REJECT_INTERNAL,
            ));
        };
        let Some(executable) = self.config.worker_executable.clone() else {
            return Ok(self.reject_allocation_preparation(
                request,
                lobby_worker_id,
                ALLOCATION_REJECT_INTERNAL,
            ));
        };
        let Some(protocol_registry_fingerprint) = self.config.protocol_registry_fingerprint else {
            return Ok(self.reject_allocation_preparation(
                request,
                lobby_worker_id,
                ALLOCATION_REJECT_INTERNAL,
            ));
        };
        let Some(processes) = self.processes.as_ref() else {
            return Ok(self.reject_allocation_preparation(
                request,
                lobby_worker_id,
                ALLOCATION_REJECT_INTERNAL,
            ));
        };
        if processes.worker_count() >= self.config.core.max_workers {
            return Ok(self.reject_allocation_preparation(
                request,
                lobby_worker_id,
                ALLOCATION_REJECT_CAPACITY,
            ));
        }
        let logical_server_id = self
            .config
            .core
            .logical_server_id
            .ok_or(RuntimeError::Routing(
                RoutingErrorCategory::ManifestIncompatible,
            ))?;
        let network_protocol = self
            .config
            .core
            .network_protocol
            .ok_or(RuntimeError::Routing(
                RoutingErrorCategory::ManifestIncompatible,
            ))?;
        let content_fingerprint =
            self.config
                .core
                .content_fingerprint
                .ok_or(RuntimeError::Routing(
                    RoutingErrorCategory::ManifestIncompatible,
                ))?;
        let Some(identities) =
            self.generate_allocation_identities(request, lobby_worker_id, policy.seed_policy)
        else {
            return Ok(None);
        };
        Ok(Some(AllocationLaunchContext {
            common: ManifestCommon {
                manifest_version: 3,
                role: WorkerRole::Match,
                logical_server_id,
                process_id: identities.process_id,
                worker_id: identities.match_worker_id,
                generation: identities.generation,
                network_protocol,
                protocol_registry_fingerprint,
                content_fingerprint,
                route_version: ROUTE_VERSION_V1,
                packet_version: PACKET_VERSION_V1,
                control_version: CONTROL_VERSION_CURRENT,
                flags: 0,
            },
            executable,
            allocation_id: identities.allocation_id,
            match_id: identities.match_id,
            match_worker_id: identities.match_worker_id,
            seed: identities.seed,
            heartbeat_ms: policy.heartbeat_ms,
        }))
    }

    fn generate_allocation_identities(
        &mut self,
        request: &AllocateRequestBody,
        lobby_worker_id: WorkerId,
        seed_policy: SeedPolicy,
    ) -> Option<AllocationIdentities> {
        let Ok(allocation_id) = random_id128(AllocationId::new) else {
            return self.reject_allocation_preparation(
                request,
                lobby_worker_id,
                ALLOCATION_REJECT_INTERNAL,
            );
        };
        let Ok(match_id) = random_id128(MatchId::new) else {
            return self.reject_allocation_preparation(
                request,
                lobby_worker_id,
                ALLOCATION_REJECT_INTERNAL,
            );
        };
        let Ok(match_worker_id) = self.fresh_worker_id() else {
            return self.reject_allocation_preparation(
                request,
                lobby_worker_id,
                ALLOCATION_REJECT_INTERNAL,
            );
        };
        let Ok(process_id) = random_id128(ProcessId::new) else {
            return self.reject_allocation_preparation(
                request,
                lobby_worker_id,
                ALLOCATION_REJECT_INTERNAL,
            );
        };
        let generation = Generation::new(1).expect("constant generation is nonzero");
        let seed = match seed_policy {
            SeedPolicy::OsRandom => {
                let Ok(seed) = random_u64() else {
                    return self.reject_allocation_preparation(
                        request,
                        lobby_worker_id,
                        ALLOCATION_REJECT_INTERNAL,
                    );
                };
                seed
            }
        };
        Some(AllocationIdentities {
            allocation_id,
            match_id,
            match_worker_id,
            process_id,
            generation,
            seed,
        })
    }

    fn reject_allocation_preparation<T>(
        &mut self,
        request: &AllocateRequestBody,
        lobby_worker_id: WorkerId,
        reason: u16,
    ) -> Option<T> {
        self.insert_rejected_allocation(request.clone(), lobby_worker_id, reason);
        None
    }

    fn prepare_allocation(
        &mut self,
        lobby_worker_id: WorkerId,
        request: AllocateRequestBody,
        context: AllocationLaunchContext,
    ) -> Result<Option<PreparedAllocation>, RuntimeError> {
        let mut participants = Vec::with_capacity(request.participants.len());
        let mut manifest_participants = Vec::with_capacity(request.participants.len());
        for source in &request.participants {
            let route_id = Self::fresh_route_id(&participants)?;
            let peer_id = Self::fresh_peer_id(&participants)?;
            let Ok(capability) = Capability::generate() else {
                self.insert_rejected_allocation(
                    request.clone(),
                    lobby_worker_id,
                    ALLOCATION_REJECT_INTERNAL,
                );
                return Ok(None);
            };
            manifest_participants.push(MatchManifestParticipant {
                lobby_session_id: source.lobby_session_id,
                player_id: source.player_id,
                netcode_client_id: source.netcode_client_id,
                peer_id,
                team: source.team,
                display_name: source.display_name,
                recipe_fingerprint: source.recipe_fingerprint,
                revision: source.build_revision,
                build_snapshot: source.build_snapshot,
            });
            participants.push(AllocationParticipant {
                source: *source,
                route_id,
                peer_id,
                capability,
            });
        }
        let manifest_bots = request
            .bots
            .iter()
            .map(|source| MatchManifestBot {
                player_id: source.player_id,
                team: source.team,
                display_name: source.display_name,
                recipe_fingerprint: source.recipe_fingerprint,
                revision: source.build_revision,
                build_snapshot: source.build_snapshot,
            })
            .collect();
        let manifest = MatchManifestV1 {
            common: context.common,
            request_id: request.request_id,
            match_id: context.match_id,
            allocation_id: context.allocation_id,
            mode: request.mode,
            map_preset: request.map_preset,
            map_revision: request.map_revision,
            rules_profile: request.rules_profile,
            objective_target: request.objective_target,
            match_duration_ticks: request.match_duration_ticks,
            countdown_ticks: request.countdown_ticks,
            respawn_ticks: request.respawn_ticks,
            reserved: 0,
            seed: context.seed,
            participants: manifest_participants,
            bots: manifest_bots,
            heartbeat_ms: context.heartbeat_ms,
            nonce: random_u128()
                .map_err(|_| RuntimeError::Routing(RoutingErrorCategory::SupervisorInternal))?,
            digest: [0; 32],
        };
        let mut spec = WorkerLaunchSpec::new(
            context.executable,
            WorkerRegistration {
                worker_id: context.match_worker_id,
                process_id: context.common.process_id,
                generation: context.common.generation,
                kind: WorkerKind::Match,
            },
            ManifestBody::from_match(&manifest)
                .map_err(|_| RuntimeError::Routing(RoutingErrorCategory::ManifestMalformed))?,
        );
        // The paired evidence launcher opts into one role-local authoritative window marker.
        // Keep this as a narrow environment seam: the worker manifest and argv remain unchanged,
        // while the match worker writes its first Active->Completed interval to a unique path.
        if let Some(window_dir) = std::env::var_os("BRAWLER_ROUTED_WINDOW_DIR") {
            let window_path = PathBuf::from(window_dir).join("match.window");
            spec = spec
                .with_environment("BRAWLER_DIAGNOSTICS_WINDOW_FILE", window_path)
                .with_environment("BRAWLER_DIAGNOSTICS_ROLE", "match");
        }
        Ok(Some(PreparedAllocation {
            request_id: request.request_id,
            match_worker_id: context.match_worker_id,
            record: AllocationRecord {
                request,
                lobby_worker_id,
                allocation_id: Some(context.allocation_id),
                match_id: Some(context.match_id),
                match_worker_id: Some(context.match_worker_id),
                participants,
                response: None,
                response_queued: false,
                result: None,
            },
            spec,
        }))
    }

    fn commit_allocation(&mut self, prepared: PreparedAllocation, report: &mut RuntimePollReport) {
        self.allocations
            .insert(prepared.request_id, prepared.record);
        // This is the handoff gate's origin: the request has passed every policy/capacity check,
        // its immutable record is committed, and the cold match-worker spawn has not begun yet.
        // Emitting at grant delivery would incorrectly exclude spawn and validated Ready latency.
        report
            .timing_events
            .push(RuntimeTimingEvent::AllocationAccepted {
                request_id: prepared.request_id,
                worker_id: prepared.match_worker_id,
            });
        if let Ok(events) = self.spawn_worker(prepared.spec) {
            report.lifecycle_events.extend(events);
        } else {
            self.cleanup_worker(prepared.match_worker_id);
            self.reject_allocation(prepared.request_id, ALLOCATION_REJECT_INTERNAL);
        }
    }

    fn insert_rejected_allocation(
        &mut self,
        request: AllocateRequestBody,
        lobby_worker_id: WorkerId,
        reason: u16,
    ) {
        let request_id = request.request_id;
        self.allocations.insert(
            request_id,
            AllocationRecord {
                request,
                lobby_worker_id,
                allocation_id: None,
                match_id: None,
                match_worker_id: None,
                participants: Vec::new(),
                response: Some(ControlBody::AllocationRejected(AllocationRejectedBody {
                    request_id,
                    reason,
                    retry_after_ms: 0,
                })),
                response_queued: false,
                result: None,
            },
        );
    }

    pub(super) fn reject_allocation(&mut self, request_id: RequestId, reason: u16) {
        let Some(record) = self.allocations.get_mut(&request_id) else {
            return;
        };
        if record.response.is_some() {
            return;
        }
        record.response = Some(ControlBody::AllocationRejected(AllocationRejectedBody {
            request_id,
            reason,
            retry_after_ms: 0,
        }));
        record.response_queued = false;
        record.match_worker_id = None;
    }

    pub(super) fn reject_allocation_for_worker(&mut self, worker_id: WorkerId, reason: u16) {
        let ids = self
            .allocations
            .iter()
            .filter_map(|(request_id, record)| {
                (record.match_worker_id == Some(worker_id)
                    && record.response.is_none()
                    && record.result.is_none())
                .then_some(*request_id)
            })
            .collect::<Vec<_>>();
        for request_id in ids {
            self.reject_allocation(request_id, reason);
        }
    }

    pub(super) fn finalize_ready_allocations(
        &mut self,
        report: &mut RuntimePollReport,
    ) -> Result<(), RuntimeError> {
        let ready_workers = report
            .lifecycle_events
            .iter()
            .filter_map(|event| match event {
                LifecycleEvent::Ready { worker_id } => Some(*worker_id),
                _ => None,
            })
            .collect::<Vec<_>>();
        for worker_id in ready_workers {
            let Some(request_id) = self.allocations.iter().find_map(|(request_id, record)| {
                (record.match_worker_id == Some(worker_id) && record.response.is_none())
                    .then_some(*request_id)
            }) else {
                continue;
            };
            self.finalize_allocation(request_id, worker_id)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn finalize_allocation(
        &mut self,
        request_id: RequestId,
        match_worker_id: WorkerId,
    ) -> Result<(), RuntimeError> {
        let Some(record) = self.allocations.get(&request_id) else {
            return Ok(());
        };
        let Some(allocation_id) = record.allocation_id else {
            return Ok(());
        };
        let Some(match_id) = record.match_id else {
            return Ok(());
        };
        let Some(registration) = self
            .processes
            .as_ref()
            .and_then(|processes| processes.worker_registration(match_worker_id))
        else {
            self.reject_allocation(request_id, ALLOCATION_REJECT_INTERNAL);
            return Ok(());
        };
        let logical_server_id = self
            .config
            .core
            .logical_server_id
            .ok_or(RuntimeError::Routing(
                RoutingErrorCategory::ManifestIncompatible,
            ))?;
        let supervisor_generation =
            self.config
                .core
                .supervisor_generation
                .ok_or(RuntimeError::Routing(
                    RoutingErrorCategory::ManifestIncompatible,
                ))?;
        let network_protocol = self
            .config
            .core
            .network_protocol
            .ok_or(RuntimeError::Routing(
                RoutingErrorCategory::ManifestIncompatible,
            ))?;
        let content_fingerprint =
            self.config
                .core
                .content_fingerprint
                .ok_or(RuntimeError::Routing(
                    RoutingErrorCategory::ManifestIncompatible,
                ))?;
        let now = self.now();
        let expiry = unix_now_millis().saturating_add(crate::CAPABILITY_HARD_LIFETIME_MILLIS);
        let activation_expiry = unix_now_millis().saturating_add(crate::CAPABILITY_PENDING_MILLIS);
        let participants = record
            .participants
            .iter()
            .map(|participant| {
                (
                    participant.source,
                    participant.route_id,
                    participant.peer_id,
                    participant.capability.clone(),
                    AllocationGrant {
                        lobby_session_id: participant.source.lobby_session_id,
                        route_id: participant.route_id,
                        peer_id: participant.peer_id,
                        capability: participant.capability.clone(),
                        activation_expiry_unix_ms: activation_expiry,
                        route_expiry_unix_ms: expiry,
                    },
                )
            })
            .collect::<Vec<_>>();
        for (_, route_id, peer_id, _, _) in &participants {
            if let Err(category) = self.core.register_route(RouteRegistration {
                route_id: *route_id,
                worker_id: match_worker_id,
                peer_id: *peer_id,
                is_default_lobby: false,
            }) {
                self.cleanup_worker(match_worker_id);
                self.reject_allocation(request_id, allocation_rejection_for(category));
                return Ok(());
            }
        }
        for (source, route_id, peer_id, capability, _) in &participants {
            let binding = CapabilityBinding {
                logical_server_id,
                supervisor_generation,
                worker_id: match_worker_id,
                worker_generation: registration.generation,
                route_id: *route_id,
                peer_id: *peer_id,
                lobby_session_id: source.lobby_session_id,
                allocation_id,
                match_id,
                network_protocol,
                content_fingerprint,
            };
            if let Err(category) = self.core.bind_capability(capability.clone(), binding, now) {
                self.cleanup_worker(match_worker_id);
                self.reject_allocation(request_id, allocation_rejection_for(category));
                return Ok(());
            }
        }
        let grants = participants
            .into_iter()
            .map(|(_, _, _, _, grant)| grant)
            .collect();
        if let Some(record) = self.allocations.get_mut(&request_id) {
            record.response = Some(ControlBody::AllocationGranted(AllocationGrantedBody {
                request_id,
                allocation_id,
                match_id,
                worker_id: match_worker_id,
                grants,
            }));
            record.response_queued = false;
        }
        Ok(())
    }

    pub(super) fn queue_allocation_responses(&mut self) {
        let mut ids = self.allocations.keys().copied().collect::<Vec<_>>();
        ids.sort_by_key(|id| id.get());
        for request_id in ids {
            let Some((worker_id, response)) =
                self.allocations.get(&request_id).and_then(|record| {
                    (!record.response_queued)
                        .then(|| {
                            record
                                .response
                                .clone()
                                .map(|response| (record.lobby_worker_id, response))
                        })
                        .flatten()
                })
            else {
                continue;
            };
            if self.queue_control_body(worker_id, response)
                && let Some(record) = self.allocations.get_mut(&request_id)
            {
                record.response_queued = true;
            }
        }
        // Rejections have no match worker whose Result still needs to be correlated. Once the
        // response is safely in the bounded supervisor queue, release the request record so the
        // bound describes concurrent work rather than the process's entire request history.
        self.allocations
            .retain(|_, record| record.match_worker_id.is_some() || !record.response_queued);
    }

    pub(super) fn queue_control_body(&mut self, worker_id: WorkerId, body: ControlBody) -> bool {
        // ProcessSupervisor owns the shutdown Stop frame.  A runtime response queued after it
        // would be physically ordered after Stop while still carrying the pre-Stop runtime
        // cursor, so the worker would reject it as stale (and a later retry could duplicate the
        // sequence).  Shutdown deliberately drops these non-lifecycle controls.
        if self.shutting_down {
            return false;
        }
        let Some(worker) = self.workers.get_mut(&worker_id) else {
            return false;
        };
        let sequence = worker.next_control_sequence;
        worker.next_control_sequence = worker.next_control_sequence.saturating_add(1);
        let Ok(frame) = ControlFrame::from_raw_sequence(
            sequence,
            worker.registration.process_id,
            worker_id,
            body,
        ) else {
            return false;
        };
        let Ok(record) = frame.encode() else {
            return false;
        };
        self.core.enqueue_control(worker_id, record).is_ok()
    }

    fn fresh_worker_id(&self) -> Result<WorkerId, RuntimeError> {
        for _ in 0..16 {
            let id = random_id128(WorkerId::new)?;
            if self.workers.contains_key(&id)
                || self
                    .allocations
                    .values()
                    .any(|record| record.match_worker_id == Some(id))
            {
                continue;
            }
            return Ok(id);
        }
        Err(RuntimeError::Routing(
            RoutingErrorCategory::SupervisorInternal,
        ))
    }

    fn fresh_route_id(participants: &[AllocationParticipant]) -> Result<RouteId, RuntimeError> {
        for _ in 0..16 {
            let id = random_id128(RouteId::new)?;
            if participants
                .iter()
                .any(|participant| participant.route_id == id)
            {
                continue;
            }
            return Ok(id);
        }
        Err(RuntimeError::Routing(
            RoutingErrorCategory::SupervisorInternal,
        ))
    }

    fn fresh_peer_id(participants: &[AllocationParticipant]) -> Result<PeerId, RuntimeError> {
        for _ in 0..16 {
            let id = random_id128(PeerId::new)?;
            if participants
                .iter()
                .any(|participant| participant.peer_id == id)
            {
                continue;
            }
            return Ok(id);
        }
        Err(RuntimeError::Routing(
            RoutingErrorCategory::SupervisorInternal,
        ))
    }
}
