//! Held match seats for routed participants whose session ended mid-match (BRL-yyeg).
//!
//! A routed participant's fighter has a server-owned lifetime. When its controlling link is lost
//! while the match is `Active`, the seat is *held* for [`SEAT_HOLD_DURATION`]: the fighter stays,
//! counts for its team, and the server-hosted Practice bot policy drives it through the ordinary
//! authoritative `ActionState` path. A supervisor rebind names the fresh routed peer and Netcode
//! client that may reclaim the seat; admission then re-attaches control to the same fighter. At
//! expiry, or for a departure outside `Active`, the fighter is removed and today's
//! departure/forfeit rules apply.
//!
//! A player who leaves on purpose (`MatchCommand::LeaveMatch`, BRL-8y1f) is not a lost session:
//! the seat is marked [`LeftSeat`], released to the supervisor and lobby at once, and the end of
//! that session removes the fighter in every phase, exactly like a departure before BRL-yyeg.
//!
//! A match that completes while a seat is held (BRL-jxfy) defers its `Result` (see
//! [`CompletedSeats`]): the held player may still rejoin, in `Completed`, to receive the completed
//! snapshot and so its Results. Every seat that can no longer be rejoined is released at
//! completion, and a held seat once it is reclaimed or expires.

use crate::{
    matchplay::{MatchParticipant, MatchPhase, MatchRoot, MatchState},
    protocol::{Fighter, FighterInput, NetworkEntityId, PlayerId},
    timing::SimulationTick,
};
use bevy::{ecs::system::SystemParam, prelude::*};
use brawler_routing::{
    MAX_PARTICIPANTS, MAX_REJOINS_PER_SEAT, ParticipantRebindBody, ParticipantReboundBody,
    RejoinReason, SeatReleasedBody,
};
use lightyear::prelude::input::native::{ActionState, NativeBuffer};
use lightyear::prelude::{ControlledBy, ControlledSend, Disconnected, Lifetime, LinkOf};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// How long a lost routed participant's seat is held while the match is Active (owner decision
/// Q1). This is code-owned session policy beside the 10 s routed session bound, not balance.
pub(crate) const SEAT_HOLD_DURATION_SECONDS: u64 = 60;

/// [`SEAT_HOLD_DURATION_SECONDS`] in simulation ticks.
pub(crate) const SEAT_HOLD_TICKS: u64 =
    match crate::timing::simulation_ticks_from_seconds(SEAT_HOLD_DURATION_SECONDS) {
        Some(ticks) => ticks,
        None => panic!("seat hold duration fits the tick range"),
    };

/// How long a seat reclaimed after completion may keep the Result deferred while its session is
/// connected: the routed session bound, so the rejoined client receives the completed snapshot
/// before the supervisor's drain closes the routes (BRL-jxfy).
const COMPLETED_REJOIN_LINGER_SECONDS: u64 = 10;

/// [`COMPLETED_REJOIN_LINGER_SECONDS`] in simulation ticks.
const COMPLETED_REJOIN_LINGER_TICKS: u64 =
    match crate::timing::simulation_ticks_from_seconds(COMPLETED_REJOIN_LINGER_SECONDS) {
        Some(ticks) => ticks,
        None => panic!("completed rejoin linger fits the tick range"),
    };

/// Whether a match in this phase admits the rebind and rejoin of a held seat: while it runs,
/// and after it completed so the returning player still sees its Results (BRL-jxfy).
pub(crate) const fn phase_admits_rejoin(phase: &MatchPhase) -> bool {
    matches!(
        phase,
        MatchPhase::Active { .. } | MatchPhase::Completed { .. }
    )
}

/// Marks a fighter assembled for a routed manifest participant. Its `ControlledBy` lifetime is
/// `Persistent`, so this module, not Lightyear, decides what a lost session means.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct RoutedSeat;

/// A held seat. The fighter has no controlling link and is driven by a Practice bot until the
/// player rejoins or `expires_at_tick` passes.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SeatHold {
    pub(crate) expires_at_tick: u64,
}

/// The fighter's player chose to leave the match (BRL-8y1f). Inserted by the accepted
/// `MatchCommand::LeaveMatch`; the end of its session is then a departure, never a held seat.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct LeftSeat;

/// The routed identity that may reclaim one participant's seat, overriding the immutable
/// manifest identity once the supervisor has rebound the seat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SeatBinding {
    pub(crate) peer_id: brawler_routing::PeerId,
    pub(crate) netcode_client_id: brawler_routing::NetcodeClientId,
}

/// Runtime rebind overlay on top of the immutable manifest, keyed by manifest player ID, plus
/// each seat's accepted rebind count. Bounded by the manifest participant count.
#[derive(Resource, Default, Debug)]
pub(crate) struct ParticipantRebinds {
    bindings: BTreeMap<u64, SeatBinding>,
    accepted: BTreeMap<u64, u8>,
    /// Seats whose players left on purpose; they never accept a rebind again.
    left: BTreeSet<u64>,
}

impl ParticipantRebinds {
    #[must_use]
    pub(crate) fn binding(&self, player_id: u64) -> Option<SeatBinding> {
        self.bindings.get(&player_id).copied()
    }

    /// The worker enforces the per-seat rejoin limit too, so the seat it owns stays bounded
    /// even if a supervisor fact were replayed.
    fn limit_reached(&self, player_id: u64) -> bool {
        self.accepted.get(&player_id).copied().unwrap_or(0) >= MAX_REJOINS_PER_SEAT
    }

    fn rebind(&mut self, player_id: u64, binding: SeatBinding) {
        self.bindings.insert(player_id, binding);
        let accepted = self.accepted.entry(player_id).or_default();
        *accepted = accepted.saturating_add(1);
    }

    fn release(&mut self, player_id: u64) {
        self.bindings.remove(&player_id);
    }

    /// Record a voluntary leave; `false` when the seat was already left.
    fn leave(&mut self, player_id: u64) -> bool {
        self.release(player_id);
        self.left.insert(player_id)
    }

    fn has_left(&self, player_id: u64) -> bool {
        self.left.contains(&player_id)
    }
}

/// Bounded supervisor rebind requests and the worker's replies. The worker control owner pushes
/// requests and drains replies; [`answer_participant_rebinds`] is the only consumer/producer.
#[derive(Resource, Default, Debug)]
pub(crate) struct SeatRebindQueue {
    requests: VecDeque<ParticipantRebindBody>,
    replies: VecDeque<ParticipantReboundBody>,
    /// One release fact per seat that can no longer be rejoined (left, BRL-8y1f, or released by
    /// completion, BRL-jxfy); each seat is released once, so this is bounded by the participant
    /// count.
    releases: VecDeque<SeatReleasedBody>,
}

impl SeatRebindQueue {
    /// Accept one rebind request; `false` when the bound is exceeded (a supervisor protocol
    /// violation: it keeps at most one rebind in flight per seat).
    pub(crate) fn push_request(&mut self, request: ParticipantRebindBody) -> bool {
        if self.requests.len() >= MAX_PARTICIPANTS {
            return false;
        }
        self.requests.push_back(request);
        true
    }

    pub(crate) fn front_reply(&self) -> Option<ParticipantReboundBody> {
        self.replies.front().copied()
    }

    pub(crate) fn pop_reply(&mut self) {
        let _ = self.replies.pop_front();
    }

    pub(crate) fn front_release(&self) -> Option<SeatReleasedBody> {
        self.releases.front().copied()
    }

    pub(crate) fn pop_release(&mut self) {
        let _ = self.releases.pop_front();
    }

    /// Queue one release fact for a manifest participant whose seat can no longer be rebound.
    fn push_release(&mut self, manifest: &brawler_routing::MatchManifestV1, player_id: u64) {
        let Some(player_id) = brawler_routing::PlayerId::new(player_id) else {
            return;
        };
        if self.releases.len() < MAX_PARTICIPANTS {
            self.releases.push_back(SeatReleasedBody {
                request_id: manifest.request_id,
                allocation_id: manifest.allocation_id,
                match_id: manifest.match_id,
                player_id,
            });
        }
    }
}

/// The match worker's Result gate after completion (BRL-jxfy). The worker sends its `Result`,
/// which ends the match for the supervisor and lobby, only once no seat can still rejoin:
/// - a seat held at completion blocks it until it is reclaimed or its hold expires;
/// - a seat reclaimed in `Completed` blocks it while that session stays connected, for at most
///   [`COMPLETED_REJOIN_LINGER_SECONDS`].
///
/// With no held seat at completion the Result goes out on the completion frame, as before. The
/// first completed state is latched: the Result describes it even if the root later restarts
/// (an empty roster unlocks a restart once its last fighter is gone).
#[derive(Resource, Default, Debug)]
pub(crate) struct CompletedSeats {
    /// The first completed root state; once set, the seats that cannot rejoin were released.
    completed: Option<MatchState>,
    /// Seats held at completion that are still held.
    held: BTreeSet<u64>,
    /// Seats reclaimed in `Completed`, with the tick their linger ends.
    lingering: BTreeMap<u64, u64>,
}

impl CompletedSeats {
    /// Whether the match has completed and no seat can still rejoin it.
    #[must_use]
    pub(crate) fn result_ready(&self) -> bool {
        self.ready_result().is_some()
    }

    /// The completed state the Result describes, once no seat can still rejoin the match.
    #[must_use]
    pub(crate) fn ready_result(&self) -> Option<&MatchState> {
        self.completed
            .as_ref()
            .filter(|_| self.held.is_empty() && self.lingering.is_empty())
    }
}

/// Read-only view of the held seats used by match admission.
#[derive(SystemParam)]
pub(crate) struct HeldSeats<'w, 's> {
    rebinds: Res<'w, ParticipantRebinds>,
    held: Query<'w, 's, (Entity, &'static PlayerId, &'static NetworkEntityId), With<SeatHold>>,
}

impl HeldSeats<'_, '_> {
    /// Rebound identities of the seats held right now; only these may be admitted mid-match.
    pub(crate) fn rejoinable(&self) -> BTreeMap<u64, SeatBinding> {
        self.held
            .iter()
            .filter_map(|(_, player, _)| {
                self.rebinds
                    .binding(player.0)
                    .map(|binding| (player.0, binding))
            })
            .collect()
    }

    /// The held fighter of a manifest participant, if its seat is held right now.
    pub(crate) fn held_fighter(
        &self,
        player_id: u64,
    ) -> Option<(Entity, PlayerId, NetworkEntityId)> {
        self.held
            .iter()
            .find(|(_, player, _)| player.0 == player_id)
            .map(|(entity, player, network)| (entity, *player, *network))
    }
}

pub(crate) struct SeatHoldPlugin;

impl Plugin for SeatHoldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ParticipantRebinds>()
            .init_resource::<SeatRebindQueue>()
            .init_resource::<CompletedSeats>()
            .add_systems(
                FixedUpdate,
                (hold_released_seats, expire_seat_holds, ApplyDeferred)
                    .chain()
                    .in_set(crate::gameplay::GameplaySet::Lifecycle)
                    .before(crate::matchplay::MatchSet::Lifecycle),
            )
            .add_systems(
                Update,
                (
                    release_left_seats.after(super::ServerSessionSet::CommitCommands),
                    answer_participant_rebinds,
                    gate_completed_result,
                )
                    .chain(),
            );
    }
}

/// Whether a routed fighter's controlling link is gone or disconnected.
fn owner_lost(
    controlled: Option<&ControlledBy>,
    links: &Query<Has<Disconnected>, With<LinkOf>>,
) -> bool {
    controlled.is_none_or(|controlled| !links.get(controlled.owner).is_ok_and(|lost| !lost))
}

/// Releases seats for good: a released seat refuses every later rebind, and the supervisor and
/// lobby learn it through one `SeatReleased` fact, so the lobby never announces a rejoin to the
/// player's next lobby session.
#[derive(SystemParam)]
struct SeatReleases<'w> {
    role: Res<'w, super::ServerRoleResource>,
    rebinds: ResMut<'w, ParticipantRebinds>,
    queue: ResMut<'w, SeatRebindQueue>,
}

impl SeatReleases<'_> {
    /// Release one manifest participant's seat; `false` when it was already released.
    fn release(&mut self, player_id: u64) -> bool {
        if !self.rebinds.leave(player_id) {
            return false;
        }
        if let Some(manifest) = self.role.manifest() {
            self.queue.push_release(manifest, player_id);
        }
        true
    }

    fn participant_ids(&self) -> Vec<u64> {
        self.role.manifest().map_or_else(Vec::new, |manifest| {
            manifest
                .participants
                .iter()
                .map(|participant| participant.player_id.get())
                .collect()
        })
    }
}

/// Release the seat of every routed participant whose leave was just accepted (BRL-8y1f).
#[allow(
    clippy::type_complexity,
    reason = "Bevy system parameters are owned by the schedule"
)]
fn release_left_seats(
    mut releases: SeatReleases,
    left: Query<&PlayerId, (With<Fighter>, With<RoutedSeat>, Added<LeftSeat>)>,
) {
    for player in &left {
        if releases.release(player.0) {
            info!(
                player_id = player.0,
                "brawler match participant left; its seat is released"
            );
        }
    }
}

/// Gate the Result on the seats that may still rejoin a completed match (BRL-jxfy). On the first
/// completed frame every routed seat that is not held is released: its player is either
/// connected and sees Results now, or gone for good, so its lobby must not announce a rejoin
/// while the Result is deferred. A held seat is released when it is reclaimed (the rejoined
/// session sees Results and its next lobby welcome is ordinary) or when its hold expires.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy system parameters are owned by the schedule"
)]
fn gate_completed_result(
    tick: Res<SimulationTick>,
    mut gate: ResMut<CompletedSeats>,
    mut releases: SeatReleases,
    roots: Query<&MatchState, With<MatchRoot>>,
    links: Query<Has<Disconnected>, With<LinkOf>>,
    seats: Query<
        (&PlayerId, Option<&ControlledBy>, Option<&SeatHold>),
        (With<Fighter>, With<RoutedSeat>),
    >,
) {
    let first = gate.completed.is_none();
    if first {
        let Some(state) = roots
            .single()
            .ok()
            .filter(|state| matches!(state.phase, MatchPhase::Completed { .. }))
        else {
            return;
        };
        gate.completed = Some(*state);
        gate.held = seats
            .iter()
            .filter(|(_, _, hold)| hold.is_some())
            .map(|(player, _, _)| player.0)
            .collect();
        for player_id in releases.participant_ids() {
            if !gate.held.contains(&player_id) {
                releases.release(player_id);
            }
        }
        if !gate.held.is_empty() {
            let deadline = seats
                .iter()
                .filter_map(|(_, _, hold)| hold.map(|hold| hold.expires_at_tick))
                .max();
            info!(
                held = gate.held.len(),
                deadline_tick = deadline,
                "brawler match completed; result deferred for held seats"
            );
        }
    } else if gate.result_ready() {
        return;
    }
    let now = tick.0;
    let seat_of = |player_id: u64| seats.iter().find(|(player, _, _)| player.0 == player_id);
    let held: Vec<u64> = gate.held.iter().copied().collect();
    for player_id in held {
        match seat_of(player_id) {
            Some((_, _, Some(_))) => {}
            Some(_) => {
                gate.held.remove(&player_id);
                gate.lingering
                    .insert(player_id, now.saturating_add(COMPLETED_REJOIN_LINGER_TICKS));
                releases.release(player_id);
                info!(
                    player_id,
                    "brawler match seat reclaimed after completion; its seat is released"
                );
            }
            None => {
                gate.held.remove(&player_id);
                releases.release(player_id);
            }
        }
    }
    gate.lingering.retain(|player_id, deadline| {
        now < *deadline
            && seat_of(*player_id).is_some_and(|(_, controlled, _)| !owner_lost(controlled, &links))
    });
}

/// Begin a hold for every routed fighter whose session ended while the match is Active, and
/// remove the fighter of a session lost in any other phase (today's departure behavior). A
/// fighter whose player left on purpose is always removed: leaving is a departure.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy system parameters are owned by the schedule"
)]
fn hold_released_seats(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    roots: Query<&MatchState, With<MatchRoot>>,
    links: Query<Has<Disconnected>, With<LinkOf>>,
    fighters: Query<
        (
            Entity,
            &PlayerId,
            &MatchParticipant,
            Option<&ControlledBy>,
            Has<LeftSeat>,
        ),
        (With<Fighter>, With<RoutedSeat>, Without<SeatHold>),
    >,
) {
    let Ok(state) = roots.single() else {
        return;
    };
    for (entity, player, participant, controlled, left) in &fighters {
        if !owner_lost(controlled, &links) {
            continue;
        }
        if left {
            info!(
                player_id = player.0,
                "brawler match participant departed after leaving; the fighter is removed"
            );
            commands.entity(entity).try_despawn();
        } else if matches!(state.phase, MatchPhase::Active { .. })
            && participant.match_id == state.match_id
        {
            info!(
                player_id = player.0,
                hold_seconds = SEAT_HOLD_DURATION_SECONDS,
                "brawler match seat held; a server bot controls the fighter"
            );
            // `ControlledBy` requires the replicated `ControlledSend` marker, and removing the
            // relationship leaves that marker behind. Lightyear hides it per link only for
            // entities that still have a `ControlledBy`, so a held fighter that kept it would
            // arrive as `Controlled` on every session that connects later, including the
            // rejoining player's (BRL-4zx2). Reclaiming inserts `ControlledBy`, which brings the
            // marker back visible to its new owner only.
            commands
                .entity(entity)
                .remove::<(ControlledBy, ControlledSend, NativeBuffer<FighterInput>)>()
                .insert((
                    SeatHold {
                        expires_at_tick: tick.0.saturating_add(SEAT_HOLD_TICKS),
                    },
                    ActionState::<FighterInput>::default(),
                    crate::bots::PracticeBotController::new(player.0),
                ));
        } else {
            commands.entity(entity).try_despawn();
        }
    }
}

/// Remove the fighter of a seat whose hold expired; the roster then applies today's
/// departure/forfeit rules.
#[allow(clippy::needless_pass_by_value)]
fn expire_seat_holds(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    mut rebinds: ResMut<ParticipantRebinds>,
    held: Query<(Entity, &PlayerId, &SeatHold), With<Fighter>>,
) {
    for (entity, player, hold) in &held {
        if tick.0 >= hold.expires_at_tick {
            info!(player_id = player.0, "brawler match seat hold expired");
            rebinds.release(player.0);
            commands.entity(entity).try_despawn();
        }
    }
}

/// The worker's view of one seat when a rebind arrives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SeatState {
    /// No fighter exists for the participant (never joined, departed, or hold expired).
    Gone,
    /// The participant's session is still connected.
    Connected,
    Held,
}

/// Decide one supervisor rebind: only a held seat of an Active or Completed match accepts a fresh
/// identity. At completion every seat that is not held is released, so in `Completed` only a
/// seat that was held then can still be rebound.
fn rebind_refusal(
    phase: Option<&MatchPhase>,
    seat: SeatState,
    left: bool,
    limit_reached: bool,
) -> Option<RejoinReason> {
    match seat {
        _ if !phase.is_some_and(phase_admits_rejoin) || left => Some(RejoinReason::SeatExpired),
        SeatState::Gone => Some(RejoinReason::SeatExpired),
        _ if limit_reached => Some(RejoinReason::LimitReached),
        SeatState::Connected => Some(RejoinReason::SeatConnected),
        SeatState::Held => None,
    }
}

/// Answer queued supervisor rebinds. An accepted rebind replaces the seat's admitted identity;
/// the earlier one can no longer reclaim it.
#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
fn answer_participant_rebinds(
    mut queue: ResMut<SeatRebindQueue>,
    mut rebinds: ResMut<ParticipantRebinds>,
    roots: Query<&MatchState, With<MatchRoot>>,
    seats: Query<(&PlayerId, Has<SeatHold>), (With<Fighter>, With<RoutedSeat>)>,
) {
    let phase = roots.single().ok().map(|state| state.phase);
    while let Some(request) = queue.requests.pop_front() {
        let seat = seats
            .iter()
            .find(|(player, _)| player.0 == request.player_id.get())
            .map_or(SeatState::Gone, |(_, held)| {
                if held {
                    SeatState::Held
                } else {
                    SeatState::Connected
                }
            });
        let refusal = rebind_refusal(
            phase.as_ref(),
            seat,
            rebinds.has_left(request.player_id.get()),
            rebinds.limit_reached(request.player_id.get()),
        );
        if refusal.is_none() {
            rebinds.rebind(
                request.player_id.get(),
                SeatBinding {
                    peer_id: request.peer_id,
                    netcode_client_id: request.netcode_client_id,
                },
            );
        }
        info!(
            player_id = request.player_id.get(),
            ?refusal,
            "brawler match worker answered a seat rebind"
        );
        if queue.replies.len() < MAX_PARTICIPANTS {
            queue.replies.push_back(ParticipantReboundBody {
                rejoin_id: request.rejoin_id,
                player_id: request.player_id,
                peer_id: request.peer_id,
                refusal,
            });
        }
    }
}

/// Re-attach control of a held fighter to the rejoined session's link. The bot controller and
/// hold are removed and intent resets to neutral, so the next player input is the only intent.
pub(crate) fn reclaim_held_seat(commands: &mut Commands, fighter: Entity, connection: Entity) {
    commands
        .entity(fighter)
        .remove::<(SeatHold, crate::bots::PracticeBotController)>()
        .insert((
            ActionState::<FighterInput>::default(),
            ControlledBy {
                owner: connection,
                lifetime: Lifetime::Persistent,
            },
        ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seat_hold_is_sixty_seconds_of_ticks() {
        assert_eq!(SEAT_HOLD_TICKS, 60 * crate::timing::SIMULATION_TICK_HZ);
    }

    #[test]
    fn only_a_held_seat_of_an_active_or_completed_match_accepts_a_rebind() {
        let active = MatchPhase::Active { ends_at_tick: 100 };
        let completed = MatchPhase::Completed {
            completed_at_tick: 50,
            restart_unlocked_at_tick: 60,
            result: crate::matchplay::MatchResult::Draw,
        };
        for phase in [active, completed] {
            let phase = Some(&phase);
            assert_eq!(rebind_refusal(phase, SeatState::Held, false, false), None);
            assert_eq!(
                rebind_refusal(phase, SeatState::Connected, false, false),
                Some(RejoinReason::SeatConnected)
            );
            assert_eq!(
                rebind_refusal(phase, SeatState::Gone, false, false),
                Some(RejoinReason::SeatExpired)
            );
            assert_eq!(
                rebind_refusal(phase, SeatState::Held, false, true),
                Some(RejoinReason::LimitReached)
            );
            // A seat its player left (or that completion released) is never rebound, whether or
            // not the session has ended yet.
            for seat in [SeatState::Held, SeatState::Connected, SeatState::Gone] {
                assert_eq!(
                    rebind_refusal(phase, seat, true, false),
                    Some(RejoinReason::SeatExpired)
                );
            }
        }
        for phase in [
            Some(MatchPhase::Waiting),
            Some(MatchPhase::Countdown { starts_at_tick: 9 }),
            None,
        ] {
            assert_eq!(
                rebind_refusal(phase.as_ref(), SeatState::Held, false, false),
                Some(RejoinReason::SeatExpired)
            );
        }
    }

    #[test]
    fn the_result_waits_for_held_and_lingering_seats_only() {
        let mut gate = CompletedSeats::default();
        assert!(
            !gate.result_ready(),
            "nothing is released before completion"
        );
        gate.completed = Some(MatchState {
            match_id: crate::matchplay::MatchId(4),
            mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
            phase: MatchPhase::Completed {
                completed_at_tick: 50,
                restart_unlocked_at_tick: 60,
                result: crate::matchplay::MatchResult::Draw,
            },
            rules_revision: 1,
        });
        assert!(
            gate.result_ready(),
            "no held seat: the result goes out at completion"
        );
        gate.held.insert(7);
        assert!(!gate.result_ready(), "a held seat defers the result");
        gate.held.remove(&7);
        gate.lingering.insert(7, 600);
        assert!(
            !gate.result_ready(),
            "a reclaimed seat still watching defers the result"
        );
        gate.lingering.clear();
        assert!(gate.result_ready());
        assert_eq!(
            COMPLETED_REJOIN_LINGER_TICKS,
            10 * crate::timing::SIMULATION_TICK_HZ
        );
    }

    #[test]
    fn accepted_rebinds_replace_the_identity_and_count_toward_the_limit() {
        let mut rebinds = ParticipantRebinds::default();
        for index in 1..=u64::from(MAX_REJOINS_PER_SEAT) {
            assert!(!rebinds.limit_reached(7));
            let binding = SeatBinding {
                peer_id: brawler_routing::PeerId::new(u128::from(index)).unwrap(),
                netcode_client_id: brawler_routing::NetcodeClientId::new(index).unwrap(),
            };
            rebinds.rebind(7, binding);
            assert_eq!(rebinds.binding(7), Some(binding));
        }
        assert!(rebinds.limit_reached(7));
        assert!(!rebinds.limit_reached(8));
        rebinds.release(7);
        assert_eq!(rebinds.binding(7), None);
        assert!(rebinds.limit_reached(7), "expiry does not reset the count");
    }

    #[test]
    fn the_rebind_queue_is_bounded_by_the_participant_count() {
        let mut queue = SeatRebindQueue::default();
        let request = ParticipantRebindBody {
            rejoin_id: brawler_routing::RequestId::new(1).unwrap(),
            player_id: brawler_routing::PlayerId::new(2).unwrap(),
            peer_id: brawler_routing::PeerId::new(3).unwrap(),
            netcode_client_id: brawler_routing::NetcodeClientId::new(4).unwrap(),
        };
        for _ in 0..MAX_PARTICIPANTS {
            assert!(queue.push_request(request));
        }
        assert!(!queue.push_request(request));
    }
}
