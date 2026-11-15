//! Held-seat rejoin admission, minting, and match-end notification (BRL-yyeg).
//!
//! The lobby asks for a fresh route to a seat it handed off earlier; the supervisor checks the
//! seat's bounded limits, mints a fresh capability on a fresh route and peer, and lets the match
//! worker accept the rebind before the grant reaches the lobby. Earlier capabilities are never
//! revived: they stay revoked negative-cache entries until their hard expiry.

use crate::{
    ActivationBody, AllocationGrant, Capability, CapabilityBinding, ControlBody, LobbySessionId,
    MAX_REJOINS_PER_SEAT, ParticipantRebindBody, ParticipantReboundBody, PeerId, RejoinGrantedBody,
    RejoinReason, RejoinRejectedBody, RejoinRequestBody, RequestId, RouteId, RouteRegistration,
    RoutingErrorCategory, WorkerId, WorkerKind,
};

use super::{SupervisorRuntime, random_id128, unix_now_millis};

/// One minted-but-not-yet-confirmed rejoin for a seat.
#[derive(Clone)]
struct PendingRejoin {
    rejoin_id: RequestId,
    route_id: RouteId,
    peer_id: PeerId,
    capability: Capability,
    lobby_session_id: LobbySessionId,
}

/// Bounded rejoin state of one manifest participant.
#[derive(Clone, Default)]
pub(super) struct SeatRejoins {
    granted: u8,
    /// Routes granted by earlier rejoins; with the original allocation route these are every
    /// route that may still carry a live capability for this seat. At most
    /// `MAX_REJOINS_PER_SEAT` entries.
    routes: Vec<RouteId>,
    pending: Option<PendingRejoin>,
}

impl SeatRejoins {
    /// Apply the per-seat limits: no rebind in flight, fewer than the maximum granted rejoins,
    /// and no live capability on any of the seat's routes (at most one live grant per seat).
    fn admit(
        &self,
        original_route: RouteId,
        route_is_live: impl Fn(RouteId) -> bool,
    ) -> Result<(), RejoinReason> {
        if self.pending.is_some() {
            return Err(RejoinReason::Busy);
        }
        if self.granted >= MAX_REJOINS_PER_SEAT {
            return Err(RejoinReason::LimitReached);
        }
        if std::iter::once(original_route)
            .chain(self.routes.iter().copied())
            .any(route_is_live)
        {
            return Err(RejoinReason::SeatConnected);
        }
        Ok(())
    }

    fn record_grant(&mut self, route_id: RouteId) {
        self.granted = self.granted.saturating_add(1);
        self.routes.push(route_id);
    }

    #[cfg(test)]
    pub(super) const fn granted(&self) -> u8 {
        self.granted
    }
}

/// Where a validated rejoin request points.
#[derive(Clone, Copy)]
struct SeatAddress {
    request_id: RequestId,
    participant_index: usize,
    match_worker_id: WorkerId,
}

impl SupervisorRuntime {
    /// Validate one lobby rejoin request and, when the seat may rejoin, mint a fresh route,
    /// peer and capability and ask the match worker to rebind the seat. Every refusal is
    /// answered to the lobby with a bounded reason; nothing here can fail the lobby worker.
    pub(super) fn accept_rejoin_request(
        &mut self,
        lobby_worker_id: WorkerId,
        request: RejoinRequestBody,
    ) {
        match self.admit_rejoin_request(lobby_worker_id, &request) {
            Ok(address) => {
                if let Err(reason) = self.mint_rejoin(address, &request) {
                    self.reject_rejoin(lobby_worker_id, request.rejoin_id, reason);
                }
            }
            Err(reason) => self.reject_rejoin(lobby_worker_id, request.rejoin_id, reason),
        }
    }

    fn admit_rejoin_request(
        &self,
        lobby_worker_id: WorkerId,
        request: &RejoinRequestBody,
    ) -> Result<SeatAddress, RejoinReason> {
        if self.shutting_down {
            return Err(RejoinReason::Internal);
        }
        if self
            .workers
            .get(&lobby_worker_id)
            .is_none_or(|worker| worker.registration.kind != WorkerKind::Lobby)
        {
            return Err(RejoinReason::UnknownSeat);
        }
        let record = self
            .allocations
            .get(&request.request_id)
            .filter(|record| {
                record.lobby_worker_id == lobby_worker_id
                    && record.allocation_id == Some(request.allocation_id)
                    && record.match_id == Some(request.match_id)
                    && matches!(record.response, Some(ControlBody::AllocationGranted(_)))
            })
            .ok_or(RejoinReason::UnknownSeat)?;
        let match_worker_id = record.match_worker_id.ok_or(RejoinReason::MatchEnded)?;
        if record.result.is_some()
            || self
                .workers
                .get(&match_worker_id)
                .is_none_or(|worker| worker.result_received)
        {
            return Err(RejoinReason::MatchEnded);
        }
        let participant_index = record
            .participants
            .iter()
            .position(|participant| participant.source.player_id == request.player_id)
            .ok_or(RejoinReason::UnknownSeat)?;
        let participant = &record.participants[participant_index];
        participant
            .rejoins
            .admit(participant.route_id, |route_id| {
                self.core.has_live_capability_for_route(route_id)
            })?;
        Ok(SeatAddress {
            request_id: request.request_id,
            participant_index,
            match_worker_id,
        })
    }

    fn mint_rejoin(
        &mut self,
        address: SeatAddress,
        request: &RejoinRequestBody,
    ) -> Result<(), RejoinReason> {
        // 128-bit random identities; a route collision is still refused by `register_route`.
        let route_id = random_id128(RouteId::new).map_err(|_| RejoinReason::Internal)?;
        let peer_id = random_id128(PeerId::new).map_err(|_| RejoinReason::Internal)?;
        let capability = Capability::generate().map_err(|_| RejoinReason::Internal)?;
        let record = self
            .allocations
            .get_mut(&address.request_id)
            .ok_or(RejoinReason::UnknownSeat)?;
        record.participants[address.participant_index]
            .rejoins
            .pending = Some(PendingRejoin {
            rejoin_id: request.rejoin_id,
            route_id,
            peer_id,
            capability,
            lobby_session_id: request.lobby_session_id,
        });
        let rebind = ControlBody::ParticipantRebind(ParticipantRebindBody {
            rejoin_id: request.rejoin_id,
            player_id: request.player_id,
            peer_id,
            netcode_client_id: request.netcode_client_id,
        });
        if self.queue_control_body(address.match_worker_id, rebind) {
            return Ok(());
        }
        if let Some(record) = self.allocations.get_mut(&address.request_id) {
            record.participants[address.participant_index]
                .rejoins
                .pending = None;
        }
        Err(RejoinReason::Internal)
    }

    /// Complete one rebind the match worker answered. A reply that matches no in-flight rejoin
    /// of this worker is a stale or duplicate fact and is ignored.
    pub(super) fn complete_participant_rebind(
        &mut self,
        match_worker_id: WorkerId,
        rebound: ParticipantReboundBody,
    ) {
        let Some((request_id, index)) = self.allocations.iter().find_map(|(request_id, record)| {
            (record.match_worker_id == Some(match_worker_id))
                .then(|| {
                    record.participants.iter().position(|participant| {
                        participant.source.player_id == rebound.player_id
                            && participant.rejoins.pending.as_ref().is_some_and(|pending| {
                                pending.rejoin_id == rebound.rejoin_id
                                    && pending.peer_id == rebound.peer_id
                            })
                    })
                })
                .flatten()
                .map(|index| (*request_id, index))
        }) else {
            return;
        };
        let record = self
            .allocations
            .get_mut(&request_id)
            .expect("allocation was just found");
        let lobby_worker_id = record.lobby_worker_id;
        let pending = record.participants[index]
            .rejoins
            .pending
            .take()
            .expect("pending rejoin was just matched");
        if let Some(reason) = rebound.refusal {
            self.reject_rejoin(lobby_worker_id, pending.rejoin_id, reason);
            return;
        }
        match self.bind_rejoin_route(request_id, match_worker_id, &pending) {
            Ok(grant) => {
                let record = self
                    .allocations
                    .get_mut(&request_id)
                    .expect("allocation was just found");
                record.participants[index]
                    .rejoins
                    .record_grant(pending.route_id);
                let body = ControlBody::RejoinGranted(RejoinGrantedBody {
                    rejoin_id: pending.rejoin_id,
                    request_id,
                    allocation_id: record.allocation_id.expect("granted allocation has an ID"),
                    match_id: record.match_id.expect("granted allocation has a match"),
                    player_id: rebound.player_id,
                    grant,
                });
                if !self.queue_control_body(lobby_worker_id, body) {
                    // The lobby never learns this capability; revoke it so it cannot linger.
                    let _ = self
                        .core
                        .revoke_capability_with_teardown(&pending.capability);
                }
            }
            Err(_category) => {
                self.reject_rejoin(lobby_worker_id, pending.rejoin_id, RejoinReason::Internal);
            }
        }
    }

    fn bind_rejoin_route(
        &mut self,
        request_id: RequestId,
        match_worker_id: WorkerId,
        pending: &PendingRejoin,
    ) -> Result<AllocationGrant, RoutingErrorCategory> {
        let record = self
            .allocations
            .get(&request_id)
            .ok_or(RoutingErrorCategory::ManifestIdentity)?;
        let allocation_id = record
            .allocation_id
            .ok_or(RoutingErrorCategory::ManifestIdentity)?;
        let match_id = record
            .match_id
            .ok_or(RoutingErrorCategory::ManifestIdentity)?;
        let worker_generation = self
            .workers
            .get(&match_worker_id)
            .ok_or(RoutingErrorCategory::ManifestIdentity)?
            .registration
            .generation;
        let core = self.config.core;
        let binding = CapabilityBinding {
            logical_server_id: core
                .logical_server_id
                .ok_or(RoutingErrorCategory::ManifestIncompatible)?,
            supervisor_generation: core
                .supervisor_generation
                .ok_or(RoutingErrorCategory::ManifestIncompatible)?,
            worker_id: match_worker_id,
            worker_generation,
            route_id: pending.route_id,
            peer_id: pending.peer_id,
            lobby_session_id: pending.lobby_session_id,
            allocation_id,
            match_id,
            network_protocol: core
                .network_protocol
                .ok_or(RoutingErrorCategory::ManifestIncompatible)?,
            content_fingerprint: core
                .content_fingerprint
                .ok_or(RoutingErrorCategory::ManifestIncompatible)?,
        };
        self.core.register_route(RouteRegistration {
            route_id: pending.route_id,
            worker_id: match_worker_id,
            peer_id: pending.peer_id,
            is_default_lobby: false,
        })?;
        if let Err(category) =
            self.core
                .bind_capability(pending.capability.clone(), binding, self.now())
        {
            let _ = self.core.teardown_route(pending.route_id);
            return Err(category);
        }
        let now = unix_now_millis();
        Ok(AllocationGrant {
            lobby_session_id: pending.lobby_session_id,
            route_id: pending.route_id,
            peer_id: pending.peer_id,
            capability: pending.capability.clone(),
            activation_expiry_unix_ms: now.saturating_add(crate::CAPABILITY_PENDING_MILLIS),
            route_expiry_unix_ms: now.saturating_add(crate::CAPABILITY_HARD_LIFETIME_MILLIS),
        })
    }

    fn reject_rejoin(
        &mut self,
        lobby_worker_id: WorkerId,
        rejoin_id: RequestId,
        reason: RejoinReason,
    ) {
        let _ = self.queue_control_body(
            lobby_worker_id,
            ControlBody::RejoinRejected(RejoinRejectedBody { rejoin_id, reason }),
        );
    }

    /// Tell the lobby, once, that an activated allocation's match has ended, and refuse any
    /// rejoin still in flight for it. Called when the worker's Result arrives and again when its
    /// allocation record is reclaimed, so a worker that dies without a Result still releases its
    /// seats.
    pub(super) fn notify_match_ended(&mut self, request_id: RequestId) {
        let Some(record) = self.allocations.get_mut(&request_id) else {
            return;
        };
        if record.ended_notified
            || !matches!(record.response, Some(ControlBody::AllocationGranted(_)))
        {
            return;
        }
        let (Some(allocation_id), Some(match_id)) = (record.allocation_id, record.match_id) else {
            return;
        };
        record.ended_notified = true;
        let lobby_worker_id = record.lobby_worker_id;
        let refused: Vec<_> = record
            .participants
            .iter_mut()
            .filter_map(|participant| participant.rejoins.pending.take())
            .map(|pending| pending.rejoin_id)
            .collect();
        for rejoin_id in refused {
            self.reject_rejoin(lobby_worker_id, rejoin_id, RejoinReason::MatchEnded);
        }
        let _ = self.queue_control_body(
            lobby_worker_id,
            ControlBody::MatchEnded(ActivationBody {
                request_id,
                allocation_id,
                match_id,
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(value: u128) -> RouteId {
        RouteId::new(value).unwrap()
    }

    #[test]
    fn seat_admission_enforces_one_live_grant_and_the_rejoin_limit() {
        let mut seat = SeatRejoins::default();
        let original = route(1);
        assert_eq!(seat.admit(original, |_| false), Ok(()));
        assert_eq!(
            seat.admit(original, |route_id| route_id == original),
            Err(RejoinReason::SeatConnected)
        );
        for granted in 1..=MAX_REJOINS_PER_SEAT {
            let rejoin_route = route(10 + u128::from(granted));
            seat.record_grant(rejoin_route);
            assert_eq!(
                seat.admit(original, |route_id| route_id == rejoin_route),
                if granted < MAX_REJOINS_PER_SEAT {
                    Err(RejoinReason::SeatConnected)
                } else {
                    Err(RejoinReason::LimitReached)
                }
            );
        }
        assert_eq!(
            seat.admit(original, |_| false),
            Err(RejoinReason::LimitReached)
        );
        assert_eq!(seat.granted(), MAX_REJOINS_PER_SEAT);
    }

    #[test]
    fn a_rebind_in_flight_blocks_a_second_request() {
        let seat = SeatRejoins {
            pending: Some(PendingRejoin {
                rejoin_id: RequestId::new(1).unwrap(),
                route_id: route(2),
                peer_id: PeerId::new(3).unwrap(),
                capability: Capability::from_bytes([9; 32]).unwrap(),
                lobby_session_id: LobbySessionId::new(4).unwrap(),
            }),
            ..SeatRejoins::default()
        };
        assert_eq!(seat.admit(route(1), |_| false), Err(RejoinReason::Busy));
    }
}
