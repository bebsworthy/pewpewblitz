//! Lobby-owned match seats and automatic rejoin forwarding (BRL-yyeg).
//!
//! At handoff the lobby remembers which account holds which match seat. The account never leaves
//! the lobby: when the same `AccountId` completes a later lobby admission while its match still
//! runs, the lobby tells the client it is rejoining, asks the supervisor for a fresh route, and
//! forwards the resulting grant as an ordinary `MatchRouteGrant`. The table is bounded (one seat
//! per account) and a seat is released when its match ends or when the supervisor refuses it for
//! a terminal reason.

use crate::{
    lobby::MatchRejoinStatus,
    profiles::AccountId,
    protocol::{MatchRouteGrant, RouteCapability, SessionChannel},
};
use bevy::prelude::*;
use brawler_routing::{
    ActivationBody, ControlBody, LobbySessionId, MAX_PARTICIPANTS, MAX_WORKERS, NetcodeClientId,
    RejoinGrantedBody, RejoinRejectedBody, RejoinRequestBody, RequestId,
};
use lightyear::prelude::MessageSender;
use std::{
    collections::{BTreeMap, VecDeque},
    time::Duration,
};

use super::LobbyClient;

/// Every live match worker can hold at most `MAX_PARTICIPANTS` routed seats.
const MAX_MATCH_SEATS: usize = MAX_WORKERS * MAX_PARTICIPANTS;

/// A rejoin that finds the seat not yet released (the earlier route still live, or the match
/// worker not yet holding the seat) is retried for this long. The earlier session is released by
/// the 10 s routed idle bound on every side at about the time the client notices the loss, and a
/// relaunched app may log in before that; the window covers both without changing the bound.
pub(super) const REJOIN_RETRY_WINDOW: Duration =
    Duration::from_millis(brawler_routing::CAPABILITY_IDLE_MILLIS + 2_000);
/// Spacing of those retries; at most `REJOIN_RETRY_WINDOW / REJOIN_RETRY_INTERVAL` requests per
/// lobby admission.
pub(super) const REJOIN_RETRY_INTERVAL: Duration = Duration::from_millis(500);

/// One account's handed-off match seat, named exactly as its original grant named it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MatchSeat {
    grant: MatchRouteGrant,
    player_id: brawler_routing::PlayerId,
    pending: Option<PendingRejoin>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingRejoin {
    /// The request in flight, or `None` while a transient refusal waits for `retry_at`.
    rejoin_id: Option<RequestId>,
    lobby_session_id: LobbySessionId,
    netcode_client_id: NetcodeClientId,
    retry_until: Duration,
    retry_at: Duration,
}

/// What to tell one lobby session about its rejoin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RejoinDelivery {
    Granted(MatchRouteGrant),
    Unavailable,
}

/// Bounded account -> seat table plus the rejoin outcomes waiting for delivery.
#[derive(Resource)]
pub(super) struct MatchSeatTable {
    seats: BTreeMap<AccountId, MatchSeat>,
    next_rejoin_id: u64,
    deliveries: VecDeque<(LobbySessionId, RejoinDelivery)>,
}

impl core::fmt::Debug for MatchSeatTable {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MatchSeatTable")
            .field("seat_count", &self.seats.len())
            .field("pending_deliveries", &self.deliveries.len())
            .finish_non_exhaustive()
    }
}

impl Default for MatchSeatTable {
    fn default() -> Self {
        Self {
            seats: BTreeMap::new(),
            next_rejoin_id: 1,
            deliveries: VecDeque::new(),
        }
    }
}

impl MatchSeatTable {
    /// Remember the seat an account was handed off to. A newer seat replaces the account's
    /// older one; a full table records nothing (that player then cannot rejoin).
    pub(super) fn record(
        &mut self,
        account: AccountId,
        player_id: brawler_routing::PlayerId,
        grant: MatchRouteGrant,
    ) -> bool {
        if !self.seats.contains_key(&account) && self.seats.len() >= MAX_MATCH_SEATS {
            warn!("brawler lobby match-seat table is full; the seat cannot be rejoined");
            return false;
        }
        self.seats.insert(
            account,
            MatchSeat {
                grant,
                player_id,
                pending: None,
            },
        );
        info!(
            match_id = grant.match_id.get(),
            "brawler lobby recorded a handed-off match seat"
        );
        true
    }

    /// Begin a rejoin for an account that just completed lobby admission, if it holds a seat.
    /// The returned request is the only one in flight for the seat; a later admission replaces
    /// it, so a stale grant can never reach the newer session.
    pub(super) fn begin_rejoin(
        &mut self,
        account: AccountId,
        lobby_session_id: LobbySessionId,
        netcode_client_id: NetcodeClientId,
        now: Duration,
    ) -> Option<RejoinRequestBody> {
        if !self.seats.contains_key(&account) {
            return None;
        }
        let rejoin_id = self.fresh_rejoin_id()?;
        let seat = self.seats.get_mut(&account)?;
        seat.pending = Some(PendingRejoin {
            rejoin_id: Some(rejoin_id),
            lobby_session_id,
            netcode_client_id,
            retry_until: now.saturating_add(REJOIN_RETRY_WINDOW),
            retry_at: now,
        });
        Some(rejoin_request(
            seat,
            rejoin_id,
            lobby_session_id,
            netcode_client_id,
        ))
    }

    fn fresh_rejoin_id(&mut self) -> Option<RequestId> {
        let rejoin_id = RequestId::new(self.next_rejoin_id)?;
        self.next_rejoin_id = self.next_rejoin_id.checked_add(1)?;
        Some(rejoin_id)
    }

    /// Re-issue every transiently refused rejoin whose retry is due and whose lobby session is
    /// still connected; a rejoin whose session is gone is dropped.
    pub(super) fn due_retries(
        &mut self,
        now: Duration,
        session_connected: impl Fn(LobbySessionId) -> bool,
    ) -> Vec<RejoinRequestBody> {
        // A request still unanswered well after its window (a stuck match worker) is reported as
        // unavailable so the client never waits without bound.
        let overdue: Vec<_> = self
            .seats
            .iter()
            .filter_map(|(account, seat)| {
                seat.pending
                    .filter(|pending| {
                        pending.rejoin_id.is_some()
                            && now >= pending.retry_until.saturating_add(REJOIN_RETRY_INTERVAL)
                    })
                    .map(|pending| (*account, pending.lobby_session_id))
            })
            .collect();
        for (account, lobby_session_id) in overdue {
            if let Some(seat) = self.seats.get_mut(&account) {
                seat.pending = None;
            }
            self.push_delivery(lobby_session_id, RejoinDelivery::Unavailable);
        }
        let due: Vec<_> = self
            .seats
            .iter()
            .filter_map(|(account, seat)| {
                seat.pending
                    .filter(|pending| pending.rejoin_id.is_none() && now >= pending.retry_at)
                    .map(|_| *account)
            })
            .collect();
        let mut requests = Vec::with_capacity(due.len());
        for account in due {
            let connected = self.seats[&account]
                .pending
                .is_some_and(|pending| session_connected(pending.lobby_session_id));
            if !connected {
                if let Some(seat) = self.seats.get_mut(&account) {
                    seat.pending = None;
                }
                continue;
            }
            let Some(rejoin_id) = self.fresh_rejoin_id() else {
                break;
            };
            let seat = self.seats.get_mut(&account).expect("due seat exists");
            let pending = seat.pending.as_mut().expect("due seat is pending");
            pending.rejoin_id = Some(rejoin_id);
            let (lobby_session_id, netcode_client_id) =
                (pending.lobby_session_id, pending.netcode_client_id);
            requests.push(rejoin_request(
                seat,
                rejoin_id,
                lobby_session_id,
                netcode_client_id,
            ));
        }
        requests
    }

    fn find_pending(&mut self, rejoin_id: RequestId) -> Option<(AccountId, &mut MatchSeat)> {
        self.seats
            .iter_mut()
            .find(|(_, seat)| {
                seat.pending
                    .is_some_and(|pending| pending.rejoin_id == Some(rejoin_id))
            })
            .map(|(account, seat)| (*account, seat))
    }

    fn take_pending(
        &mut self,
        rejoin_id: RequestId,
    ) -> Option<(AccountId, MatchSeat, LobbySessionId)> {
        let (account, seat) = self.find_pending(rejoin_id)?;
        let pending = seat.pending.take()?;
        Some((account, *seat, pending.lobby_session_id))
    }

    /// Accept the supervisor's grant for the in-flight rejoin it answers.
    pub(super) fn apply_granted(&mut self, body: RejoinGrantedBody) {
        let Some((_, seat, lobby_session_id)) = self.take_pending(body.rejoin_id) else {
            return;
        };
        let Some(capability) = RouteCapability::from_bytes(body.grant.capability.into_bytes())
        else {
            return;
        };
        if body.request_id != seat.grant.request_id
            || body.allocation_id != seat.grant.allocation_id
            || body.match_id != seat.grant.match_id
            || body.player_id != seat.player_id
            || body.grant.lobby_session_id != lobby_session_id
        {
            self.push_delivery(lobby_session_id, RejoinDelivery::Unavailable);
            return;
        }
        let grant = MatchRouteGrant {
            route_id: body.grant.route_id,
            peer_id: body.grant.peer_id,
            capability,
            activation_expiry_unix_ms: body.grant.activation_expiry_unix_ms,
            route_expiry_unix_ms: body.grant.route_expiry_unix_ms,
            ..seat.grant
        };
        self.push_delivery(lobby_session_id, RejoinDelivery::Granted(grant));
    }

    /// Accept the supervisor's refusal. A transient refusal is retried while the retry window
    /// lasts; a terminal reason releases the seat.
    pub(super) fn apply_rejected(&mut self, body: RejoinRejectedBody, now: Duration) {
        if !body.reason.is_terminal()
            && let Some((_, seat)) = self.find_pending(body.rejoin_id)
            && let Some(pending) = seat.pending.as_mut()
            && now.saturating_add(REJOIN_RETRY_INTERVAL) <= pending.retry_until
        {
            pending.rejoin_id = None;
            pending.retry_at = now.saturating_add(REJOIN_RETRY_INTERVAL);
            return;
        }
        let Some((account, _, lobby_session_id)) = self.take_pending(body.rejoin_id) else {
            return;
        };
        info!(reason = ?body.reason, "brawler lobby rejoin was refused");
        if body.reason.is_terminal() {
            self.seats.remove(&account);
        }
        self.push_delivery(lobby_session_id, RejoinDelivery::Unavailable);
    }

    /// Release every seat of an ended allocation; an in-flight rejoin for it is
    /// answered as unavailable.
    pub(super) fn release_allocation(&mut self, ended: ActivationBody) {
        let mut refused = Vec::new();
        self.seats.retain(|_, seat| {
            let ended_seat = seat.grant.request_id == ended.request_id
                && seat.grant.allocation_id == ended.allocation_id
                && seat.grant.match_id == ended.match_id;
            if ended_seat && let Some(pending) = seat.pending {
                // A rejoin waiting for its retry is answered too.
                refused.push(pending.lobby_session_id);
            }
            if ended_seat {
                info!(
                    match_id = ended.match_id.get(),
                    "brawler lobby released a match seat"
                );
            }
            !ended_seat
        });
        for lobby_session_id in refused {
            self.push_delivery(lobby_session_id, RejoinDelivery::Unavailable);
        }
    }

    fn push_delivery(&mut self, lobby_session_id: LobbySessionId, delivery: RejoinDelivery) {
        // At most one rejoin is in flight per seat, so this stays within the seat bound.
        if self.deliveries.len() < MAX_MATCH_SEATS {
            self.deliveries.push_back((lobby_session_id, delivery));
        }
    }

    /// Apply one supervisor control body this module owns; other bodies are left untouched.
    pub(super) fn apply_control(&mut self, body: &ControlBody, now: Duration) {
        match body {
            ControlBody::RejoinGranted(granted) => self.apply_granted(granted.clone()),
            ControlBody::RejoinRejected(rejected) => self.apply_rejected(*rejected, now),
            // Only the supervisor's end fact releases seats. `ActivationDissolved` is not one: a
            // Practice lobby session that closes at its own handoff still cancels the
            // activation while its match runs on; a start that really fails ends the worker,
            // and that end is announced as `MatchEnded`.
            ControlBody::MatchEnded(ended) => self.release_allocation(*ended),
            _ => {}
        }
    }

    #[cfg(test)]
    fn seat_count(&self) -> usize {
        self.seats.len()
    }
}

fn rejoin_request(
    seat: &MatchSeat,
    rejoin_id: RequestId,
    lobby_session_id: LobbySessionId,
    netcode_client_id: NetcodeClientId,
) -> RejoinRequestBody {
    RejoinRequestBody {
        rejoin_id,
        request_id: seat.grant.request_id,
        allocation_id: seat.grant.allocation_id,
        match_id: seat.grant.match_id,
        player_id: seat.player_id,
        lobby_session_id,
        netcode_client_id,
    }
}

/// Re-issue due rejoin retries through the lobby control outbox.
#[allow(clippy::needless_pass_by_value)]
pub(super) fn retry_pending_rejoins(
    time: Res<Time<Real>>,
    mut seats: ResMut<MatchSeatTable>,
    mut outbox: ResMut<crate::server::LobbyControlOutbox>,
    clients: Query<&LobbyClient>,
) {
    let requests = seats.due_retries(time.elapsed(), |session| {
        clients
            .iter()
            .any(|client| client.lobby_session_id == session)
    });
    for request in requests {
        let _ = outbox.push_rejoin(request);
    }
}

/// Whether a supervisor body belongs to the rejoin seat table rather than the allocation state.
pub(super) const fn is_rejoin_control(body: &ControlBody) -> bool {
    matches!(
        body,
        ControlBody::RejoinGranted(_) | ControlBody::RejoinRejected(_) | ControlBody::MatchEnded(_)
    )
}

/// Send each pending rejoin outcome to the lobby session it belongs to. A grant travels as the
/// ordinary `MatchRouteGrant`, so the client's existing grant/unlink/match-connect path carries
/// it; a refusal is `MatchRejoinStatus::Unavailable`.
#[allow(
    clippy::type_complexity,
    reason = "one query names the two lobby senders"
)]
pub(super) fn deliver_rejoin_outcomes(
    mut seats: ResMut<MatchSeatTable>,
    mut clients: Query<(
        &LobbyClient,
        &mut MessageSender<MatchRouteGrant>,
        &mut MessageSender<MatchRejoinStatus>,
    )>,
) {
    while let Some((lobby_session_id, delivery)) = seats.deliveries.pop_front() {
        let Some((_, mut grants, mut statuses)) = clients
            .iter_mut()
            .find(|(client, _, _)| client.lobby_session_id == lobby_session_id)
        else {
            continue;
        };
        match delivery {
            RejoinDelivery::Granted(grant) => {
                info!(
                    request_id = grant.request_id.get(),
                    match_id = grant.match_id.get(),
                    "brawler lobby delivered a match rejoin grant"
                );
                grants.send::<SessionChannel>(grant);
            }
            RejoinDelivery::Unavailable => {
                statuses.send::<SessionChannel>(MatchRejoinStatus::Unavailable);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brawler_routing::{
        AllocationGrant, AllocationId, Capability, MatchId, PeerId, RejoinReason, RouteId,
    };

    const NOW: Duration = Duration::from_secs(100);

    fn grant(request: u64) -> MatchRouteGrant {
        MatchRouteGrant {
            request_id: RequestId::new(request).unwrap(),
            allocation_id: AllocationId::new(u128::from(request) + 100).unwrap(),
            match_id: MatchId::new(u128::from(request) + 200).unwrap(),
            route_id: RouteId::new(1).unwrap(),
            peer_id: PeerId::new(2).unwrap(),
            game_mode: crate::config::GameMode::Wipeout,
            capability: RouteCapability::from_bytes([1; 32]).unwrap(),
            activation_expiry_unix_ms: 1,
            route_expiry_unix_ms: 2,
        }
    }

    fn account(value: u128) -> AccountId {
        AccountId::new(value).unwrap()
    }

    fn player(value: u64) -> brawler_routing::PlayerId {
        brawler_routing::PlayerId::new(value).unwrap()
    }

    fn session(value: u128) -> LobbySessionId {
        LobbySessionId::new(value).unwrap()
    }

    fn client(value: u64) -> NetcodeClientId {
        NetcodeClientId::new(value).unwrap()
    }

    fn granted_for(request: &RejoinRequestBody) -> RejoinGrantedBody {
        RejoinGrantedBody {
            rejoin_id: request.rejoin_id,
            request_id: request.request_id,
            allocation_id: request.allocation_id,
            match_id: request.match_id,
            player_id: request.player_id,
            grant: AllocationGrant {
                lobby_session_id: request.lobby_session_id,
                route_id: RouteId::new(9).unwrap(),
                peer_id: PeerId::new(10).unwrap(),
                capability: Capability::from_bytes([7; 32]).unwrap(),
                activation_expiry_unix_ms: 3,
                route_expiry_unix_ms: 4,
            },
        }
    }

    #[test]
    fn only_the_same_account_starts_a_rejoin_and_the_grant_reaches_its_session() {
        let mut table = MatchSeatTable::default();
        assert!(table.record(account(1), player(5), grant(7)));
        assert!(
            table
                .begin_rejoin(account(2), session(20), client(21), NOW)
                .is_none()
        );
        let request = table
            .begin_rejoin(account(1), session(30), client(31), NOW)
            .unwrap();
        assert_eq!(request.request_id.get(), 7);
        assert_eq!(request.player_id.get(), 5);
        assert_eq!(request.lobby_session_id, session(30));
        assert_eq!(request.netcode_client_id, client(31));

        table.apply_granted(granted_for(&request));
        let (session_id, delivery) = table.deliveries.pop_front().unwrap();
        assert_eq!(session_id, session(30));
        let RejoinDelivery::Granted(delivered) = delivery else {
            panic!("the grant is delivered");
        };
        assert_eq!(delivered.request_id.get(), 7);
        assert_eq!(delivered.route_id.get(), 9);
        assert_eq!(
            delivered.capability,
            RouteCapability::from_bytes([7; 32]).unwrap()
        );
        // A duplicate answer matches no in-flight rejoin.
        table.apply_granted(granted_for(&request));
        assert!(table.deliveries.is_empty());
        // The seat stays for a later rejoin.
        assert_eq!(table.seat_count(), 1);
    }

    #[test]
    fn a_newer_admission_supersedes_an_older_pending_rejoin() {
        let mut table = MatchSeatTable::default();
        table.record(account(1), player(5), grant(7));
        let first = table
            .begin_rejoin(account(1), session(30), client(31), NOW)
            .unwrap();
        let second = table
            .begin_rejoin(account(1), session(40), client(41), NOW)
            .unwrap();
        table.apply_granted(granted_for(&first));
        assert!(
            table.deliveries.is_empty(),
            "a stale grant reaches no session"
        );
        table.apply_granted(granted_for(&second));
        assert_eq!(table.deliveries.pop_front().unwrap().0, session(40));
    }

    #[test]
    fn terminal_refusals_and_match_end_release_the_seat() {
        let mut table = MatchSeatTable::default();
        table.record(account(1), player(5), grant(7));
        table.record(account(2), player(6), grant(7));
        table.record(account(3), player(7), grant(8));

        // A not-yet-released seat is retried until the window closes, then reported.
        let mut transient = table
            .begin_rejoin(account(1), session(30), client(31), NOW)
            .unwrap();
        let mut now = NOW;
        let mut retries = 0;
        loop {
            table.apply_rejected(
                RejoinRejectedBody {
                    rejoin_id: transient.rejoin_id,
                    reason: RejoinReason::SeatConnected,
                },
                now,
            );
            if !table.deliveries.is_empty() {
                break;
            }
            assert!(table.due_retries(now, |_| true).is_empty(), "not due yet");
            now += REJOIN_RETRY_INTERVAL;
            let mut due = table.due_retries(now, |_| true);
            assert_eq!(due.len(), 1);
            let retry = due.remove(0);
            assert_ne!(
                retry.rejoin_id, transient.rejoin_id,
                "each retry is a fresh request"
            );
            assert_eq!(retry.lobby_session_id, session(30));
            assert_eq!(retry.netcode_client_id, client(31));
            transient = retry;
            retries += 1;
        }
        assert!(now + REJOIN_RETRY_INTERVAL >= NOW + REJOIN_RETRY_WINDOW);
        assert_eq!(
            u32::try_from(retries).unwrap(),
            u32::try_from(REJOIN_RETRY_WINDOW.as_millis() / REJOIN_RETRY_INTERVAL.as_millis())
                .unwrap()
        );
        assert_eq!(table.seat_count(), 3);
        assert_eq!(
            table.deliveries.pop_front(),
            Some((session(30), RejoinDelivery::Unavailable))
        );
        // A retry whose lobby session has gone is dropped.
        let gone = table
            .begin_rejoin(account(1), session(31), client(31), NOW)
            .unwrap();
        table.apply_rejected(
            RejoinRejectedBody {
                rejoin_id: gone.rejoin_id,
                reason: RejoinReason::SeatConnected,
            },
            NOW,
        );
        assert!(
            table
                .due_retries(NOW + REJOIN_RETRY_INTERVAL, |_| false)
                .is_empty()
        );
        assert!(table.deliveries.is_empty());
        let terminal = table
            .begin_rejoin(account(1), session(32), client(33), NOW)
            .unwrap();
        table.apply_rejected(
            RejoinRejectedBody {
                rejoin_id: terminal.rejoin_id,
                reason: RejoinReason::LimitReached,
            },
            NOW,
        );
        assert_eq!(table.seat_count(), 2);
        table.deliveries.clear();

        let in_flight = table
            .begin_rejoin(account(2), session(34), client(35), NOW)
            .unwrap();
        table.apply_control(
            &ControlBody::MatchEnded(ActivationBody {
                request_id: in_flight.request_id,
                allocation_id: in_flight.allocation_id,
                match_id: in_flight.match_id,
            }),
            NOW,
        );
        assert_eq!(table.seat_count(), 1, "only the other match's seat remains");
        assert_eq!(
            table.deliveries.pop_front(),
            Some((session(34), RejoinDelivery::Unavailable))
        );
        assert!(
            table
                .begin_rejoin(account(2), session(36), client(37), NOW)
                .is_none()
        );
    }

    #[test]
    fn the_seat_table_is_bounded() {
        let mut table = MatchSeatTable::default();
        for index in 0..MAX_MATCH_SEATS {
            let index = u128::try_from(index).unwrap() + 1;
            assert!(table.record(account(index), player(1), grant(1)));
        }
        assert!(!table.record(account(10_000), player(1), grant(1)));
        // An existing account may still replace its own seat.
        assert!(table.record(account(1), player(2), grant(2)));
        assert_eq!(table.seat_count(), MAX_MATCH_SEATS);
    }
}
