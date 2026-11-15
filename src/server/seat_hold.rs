//! Held match seats for routed participants whose session ended mid-match (BRL-yyeg).
//!
//! A routed participant's fighter has a server-owned lifetime. When its controlling link is lost
//! while the match is `Active`, the seat is *held* for [`SEAT_HOLD_DURATION`]: the fighter stays,
//! counts for its team, and the server-hosted Practice bot policy drives it through the ordinary
//! authoritative `ActionState` path. A supervisor rebind names the fresh routed peer and Netcode
//! client that may reclaim the seat; admission then re-attaches control to the same fighter. At
//! expiry, or for a departure outside `Active`, the fighter is removed and today's
//! departure/forfeit rules apply.

use crate::{
    matchplay::{MatchParticipant, MatchPhase, MatchRoot, MatchState},
    protocol::{Fighter, FighterInput, NetworkEntityId, PlayerId},
    timing::SimulationTick,
};
use bevy::{ecs::system::SystemParam, prelude::*};
use brawler_routing::{
    MAX_PARTICIPANTS, MAX_REJOINS_PER_SEAT, ParticipantRebindBody, ParticipantReboundBody,
    RejoinReason,
};
use lightyear::prelude::input::native::{ActionState, NativeBuffer};
use lightyear::prelude::{ControlledBy, Disconnected, Lifetime, LinkOf};
use std::collections::{BTreeMap, VecDeque};

/// How long a lost routed participant's seat is held while the match is Active (owner decision
/// Q1). This is code-owned session policy beside the 10 s routed session bound, not balance.
pub(crate) const SEAT_HOLD_DURATION_SECONDS: u64 = 60;

/// [`SEAT_HOLD_DURATION_SECONDS`] in simulation ticks.
pub(crate) const SEAT_HOLD_TICKS: u64 =
    match crate::timing::simulation_ticks_from_seconds(SEAT_HOLD_DURATION_SECONDS) {
        Some(ticks) => ticks,
        None => panic!("seat hold duration fits the tick range"),
    };

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
}

/// Bounded supervisor rebind requests and the worker's replies. The worker control owner pushes
/// requests and drains replies; [`answer_participant_rebinds`] is the only consumer/producer.
#[derive(Resource, Default, Debug)]
pub(crate) struct SeatRebindQueue {
    requests: VecDeque<ParticipantRebindBody>,
    replies: VecDeque<ParticipantReboundBody>,
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
            .add_systems(
                FixedUpdate,
                (hold_released_seats, expire_seat_holds, ApplyDeferred)
                    .chain()
                    .in_set(crate::gameplay::GameplaySet::Lifecycle)
                    .before(crate::matchplay::MatchSet::Lifecycle),
            )
            .add_systems(Update, answer_participant_rebinds);
    }
}

/// Whether a routed fighter's controlling link is gone or disconnected.
fn owner_lost(
    controlled: Option<&ControlledBy>,
    links: &Query<Has<Disconnected>, With<LinkOf>>,
) -> bool {
    controlled.is_none_or(|controlled| !links.get(controlled.owner).is_ok_and(|lost| !lost))
}

/// Begin a hold for every routed fighter whose session ended while the match is Active, and
/// remove the fighter of a session lost in any other phase (today's departure behavior).
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
        (Entity, &PlayerId, &MatchParticipant, Option<&ControlledBy>),
        (With<Fighter>, With<RoutedSeat>, Without<SeatHold>),
    >,
) {
    let Ok(state) = roots.single() else {
        return;
    };
    for (entity, player, participant, controlled) in &fighters {
        if !owner_lost(controlled, &links) {
            continue;
        }
        if matches!(state.phase, MatchPhase::Active { .. })
            && participant.match_id == state.match_id
        {
            info!(
                player_id = player.0,
                hold_seconds = SEAT_HOLD_DURATION_SECONDS,
                "brawler match seat held; a server bot controls the fighter"
            );
            commands
                .entity(entity)
                .remove::<(ControlledBy, NativeBuffer<FighterInput>)>()
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

/// Decide one supervisor rebind: only a held seat of an Active match accepts a fresh identity.
fn rebind_refusal(
    phase_active: bool,
    seat: SeatState,
    limit_reached: bool,
) -> Option<RejoinReason> {
    match seat {
        _ if !phase_active => Some(RejoinReason::SeatExpired),
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
    let phase_active = roots
        .single()
        .is_ok_and(|state| matches!(state.phase, MatchPhase::Active { .. }));
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
            phase_active,
            seat,
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
    fn only_a_held_seat_of_an_active_match_accepts_a_rebind() {
        assert_eq!(rebind_refusal(true, SeatState::Held, false), None);
        assert_eq!(
            rebind_refusal(true, SeatState::Connected, false),
            Some(RejoinReason::SeatConnected)
        );
        assert_eq!(
            rebind_refusal(true, SeatState::Gone, false),
            Some(RejoinReason::SeatExpired)
        );
        assert_eq!(
            rebind_refusal(false, SeatState::Held, false),
            Some(RejoinReason::SeatExpired)
        );
        assert_eq!(
            rebind_refusal(true, SeatState::Held, true),
            Some(RejoinReason::LimitReached)
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
