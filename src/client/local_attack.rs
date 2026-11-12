//! Input-frame presentation of the local player's own attacks.
//!
//! The server remains the only combat authority: it alone accepts attacks, spends ammunition,
//! spawns deliveries, and resolves hits. On the local input tick the client evaluates the same
//! fire gate against its replicated (delayed) weapon, ability, and status state and, when the
//! gate passes, announces a [`LocalAttackPresented`] so presentation producers can start the
//! shoot animation, muzzle effect, fire sound, recoil, and cosmetic projectile immediately.
//!
//! [`LocalAttackLedger`] pairs each locally presented shot with the later authoritative
//! `AttackAccepted` cue for the same fighter (within a small simulation-tick tolerance). Matched
//! attack ids become echoes that presentation producers skip, so the server cue never replays
//! the animation, effect, or sound. A shot the server never confirms is rejected once the
//! owner's confirmed authoritative tick has passed it by a margin (the cue for tick T travels
//! with the state for T), which lets cosmetic projectiles fade within a bounded time. Rejected
//! shots stay matchable so a late cue still cannot double-play. Nothing here is sent to the
//! server or mutates authoritative state.

use crate::builds::{AbilityPhase, AbilityState, ResolvedMatchLoadout, UltimateKind};
use crate::client::prediction::{OwnerPrediction, OwnerPredictionSet};
use crate::combat::{
    ActiveEffects, AttackId, CombatCue, DeduplicatedCombatCue, TeamId, WeaponPhase, WeaponState,
};
use crate::protocol::{Fighter, FighterInput, NetworkEntityId};
use bevy::prelude::*;
use lightyear::prelude::input::native::ActionState;
use lightyear::prelude::{Controlled, LocalTimeline, Tick};
use std::collections::VecDeque;

/// Locally presented shots retained for matching (several seconds of the fastest weapon).
pub const MAX_LOCAL_SHOTS: usize = 32;
/// Simulation ticks by which a server-accepted attack may precede the local estimate.
pub const MATCH_EARLY_TOLERANCE_TICKS: u64 = 2;
/// Simulation ticks by which a server-accepted attack may follow the local estimate: an input
/// that reaches the server late is applied on a later tick (measured: four ticks for the first
/// press at a simulated 120 ms round trip).
pub const MATCH_LATE_TOLERANCE_TICKS: u64 = 6;
/// Confirmed authoritative ticks past a shot's input tick after which a missing cue is a
/// rejection. It exceeds the late match tolerance, so a late-applied input is never rejected
/// before its cue (which travels with the state for its tick) can arrive.
pub const CONFIRMATION_MARGIN_TICKS: i32 = 8;
/// Local ticks after which an unconfirmed shot is rejected even without new authoritative state.
pub const ABSOLUTE_CONFIRMATION_TIMEOUT_TICKS: i32 = 90;

/// Client-local identity of one locally presented primary-fire shot. Never sent on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalShotId(pub u64);

/// What the local player started on this input tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalAttackKind {
    PrimaryFire(LocalShotId),
    Ultimate(UltimateKind),
}

/// The controlled fighter started an attack on this input tick (presentation only).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct LocalAttackPresented {
    /// The client entity of the controlled fighter.
    pub fighter: Entity,
    pub owner: NetworkEntityId,
    pub team: TeamId,
    pub kind: LocalAttackKind,
    /// Presented fighter position when the attack started.
    pub origin: Vec2,
    pub facing: f32,
    /// Estimated server simulation tick at which the server applies this input.
    pub simulation_tick: u64,
}

/// Server agreement with one locally presented shot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalShotStatus {
    Pending,
    Confirmed(AttackId),
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LocalShot {
    id: LocalShotId,
    local_tick: Tick,
    simulation_tick: u64,
    status: LocalShotStatus,
}

/// How one authoritative own-source `AttackAccepted` related to local presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OwnAttackObservation {
    /// The attack echoes a locally presented shot; presentation must skip the cue.
    Matched(LocalShotId),
    /// The server fired without a local presentation; the cue presents it as before.
    Unmatched,
    /// The cue was already observed.
    Repeated,
}

/// Bounded pairing between locally presented shots and authoritative attacks.
#[derive(Resource, Debug, Default)]
pub struct LocalAttackLedger {
    fighter: Option<Entity>,
    shots: VecDeque<LocalShot>,
    next_id: u64,
    last_shot_simulation_tick: Option<u64>,
    ultimate_was_held: bool,
}

impl LocalAttackLedger {
    /// Forget everything when the controlled fighter changes (new match, rejoin).
    fn track(&mut self, fighter: Entity) {
        if self.fighter != Some(fighter) {
            *self = Self {
                fighter: Some(fighter),
                next_id: self.next_id,
                ..Self::default()
            };
        }
    }

    /// Shots presented locally that the server has neither confirmed nor rejected yet.
    #[must_use]
    pub fn unconfirmed(&self) -> usize {
        self.shots
            .iter()
            .filter(|shot| shot.status == LocalShotStatus::Pending)
            .count()
    }

    #[must_use]
    pub fn status(&self, shot: LocalShotId) -> Option<LocalShotStatus> {
        self.shots
            .iter()
            .find(|candidate| candidate.id == shot)
            .map(|shot| shot.status)
    }

    /// The local shot an authoritative attack echoes, if any.
    #[must_use]
    pub fn echo_shot(&self, attack_id: AttackId) -> Option<LocalShotId> {
        self.shots
            .iter()
            .find(|shot| shot.status == LocalShotStatus::Confirmed(attack_id))
            .map(|shot| shot.id)
    }

    /// Whether presentation must skip this authoritative attack because it already played.
    #[must_use]
    pub fn is_echo(&self, attack_id: AttackId) -> bool {
        self.echo_shot(attack_id).is_some()
    }

    /// Record one locally presented shot.
    pub(crate) fn record(&mut self, local_tick: Tick, simulation_tick: u64) -> LocalShotId {
        let id = LocalShotId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.shots.push_back(LocalShot {
            id,
            local_tick,
            simulation_tick,
            status: LocalShotStatus::Pending,
        });
        while self.shots.len() > MAX_LOCAL_SHOTS {
            self.shots.pop_front();
        }
        self.last_shot_simulation_tick = Some(
            self.last_shot_simulation_tick
                .map_or(simulation_tick, |last| last.max(simulation_tick)),
        );
        id
    }

    /// Fold one authoritative own-source attack into the ledger.
    pub(crate) fn observe_own_attack(
        &mut self,
        attack_id: AttackId,
        tick: u64,
    ) -> OwnAttackObservation {
        if self.is_echo(attack_id) {
            return OwnAttackObservation::Repeated;
        }
        self.last_shot_simulation_tick = Some(
            self.last_shot_simulation_tick
                .map_or(tick, |last| last.max(tick)),
        );
        let matched = self.shots.iter_mut().find(|shot| {
            matches!(
                shot.status,
                LocalShotStatus::Pending | LocalShotStatus::Rejected
            ) && tick + MATCH_EARLY_TOLERANCE_TICKS >= shot.simulation_tick
                && tick <= shot.simulation_tick + MATCH_LATE_TOLERANCE_TICKS
        });
        let Some(shot) = matched else {
            return OwnAttackObservation::Unmatched;
        };
        shot.status = LocalShotStatus::Confirmed(attack_id);
        let matched = shot.id;
        // The server fires at most once per tick, in order: older local shots it skipped are
        // rejections.
        for older in &mut self.shots {
            if older.id == matched {
                break;
            }
            if older.status == LocalShotStatus::Pending
                && older.simulation_tick + MATCH_LATE_TOLERANCE_TICKS < tick
            {
                older.status = LocalShotStatus::Rejected;
            }
        }
        OwnAttackObservation::Matched(matched)
    }

    /// Reject pending shots the server has had time to confirm. Returns how many changed.
    pub(crate) fn expire(&mut self, confirmed_tick: Option<Tick>, now: Tick) -> u64 {
        let mut rejected = 0;
        for shot in &mut self.shots {
            if shot.status != LocalShotStatus::Pending {
                continue;
            }
            let confirmed_past = confirmed_tick
                .is_some_and(|confirmed| confirmed - shot.local_tick >= CONFIRMATION_MARGIN_TICKS);
            let timed_out = now - shot.local_tick >= ABSOLUTE_CONFIRMATION_TIMEOUT_TICKS;
            if confirmed_past || timed_out {
                shot.status = LocalShotStatus::Rejected;
                rejected += 1;
            }
        }
        rejected
    }
}

/// Replicated weapon facts the local fire gate evaluates for one input tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LocalFireGate {
    pub simulation_tick: u64,
    pub weapon: WeaponState,
    pub capacity: u8,
    pub fire_cooldown_ticks: u64,
    pub unconfirmed_shots: usize,
    pub last_shot_simulation_tick: Option<u64>,
}

/// Whether the server would accept a primary-fire input at the estimated tick, as far as the
/// client can tell from replicated state and its own unconfirmed shots.
#[must_use]
pub(crate) fn local_fire_ready(gate: LocalFireGate) -> bool {
    let recovered = gate.weapon.ammo_recovery.is_some_and(|recovery| {
        gate.simulation_tick >= recovery.ready_at_tick && gate.weapon.ammo < gate.capacity
    });
    let ammo = usize::from(gate.weapon.ammo) + usize::from(recovered);
    if ammo <= gate.unconfirmed_shots {
        return false;
    }
    let replicated_ready = match gate.weapon.phase {
        WeaponPhase::Ready => true,
        WeaponPhase::Cooldown { ready_at_tick } => gate.simulation_tick >= ready_at_tick,
    };
    let local_ready = gate
        .last_shot_simulation_tick
        .is_none_or(|last| gate.simulation_tick >= last.saturating_add(gate.fire_cooldown_ticks));
    replicated_ready && local_ready
}

/// Whether a replicated ultimate is ready to activate (full charge, ready phase).
#[must_use]
pub(crate) fn local_ultimate_ready(state: AbilityState, maximum_charge: u16) -> bool {
    state.charge == maximum_charge && matches!(state.phase, AbilityPhase::Ready)
}

/// Bounded measured facts for tests and diagnostics.
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct LocalAttackStats {
    /// Primary-fire shots presented on their input tick.
    pub presented: u64,
    /// Ultimate casts presented on their input tick.
    pub ultimates_presented: u64,
    /// Shots the server confirmed (their cue was suppressed as an echo).
    pub confirmed: u64,
    /// Shots the server never confirmed.
    pub rejected: u64,
    /// Own attacks the server accepted without a local presentation (presented from the cue).
    pub unmatched_own_attacks: u64,
    /// Local input tick of the newest presented shot.
    pub last_presented_tick: Option<u32>,
}

/// Ordering of local attack presentation.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalAttackSet {
    /// `FixedUpdate`: evaluate the local gate on the input tick.
    Present,
    /// `Update`: pair authoritative cues with local shots before any cue presentation reads them.
    Reconcile,
}

/// Presents the controlled fighter's attacks on the input frame and suppresses their echoes.
pub struct LocalAttackPlugin;

impl Plugin for LocalAttackPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LocalAttackLedger>()
            .init_resource::<LocalAttackStats>()
            .add_message::<LocalAttackPresented>()
            .configure_sets(
                FixedUpdate,
                LocalAttackSet::Present
                    .after(OwnerPredictionSet::Predict)
                    .in_set(crate::gameplay::GameplaySet::Fire),
            )
            .configure_sets(
                Update,
                LocalAttackSet::Reconcile
                    .after(crate::combat::client::CombatClientSet::Ingest)
                    .before(crate::combat::client::CombatClientSet::Sync),
            )
            .add_systems(
                FixedUpdate,
                present_local_attacks.in_set(LocalAttackSet::Present),
            )
            .add_systems(
                Update,
                reconcile_local_attacks.in_set(LocalAttackSet::Reconcile),
            );
    }
}

/// Evaluate the local fire and ultimate gates for the controlled fighter on this input tick.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "the query declares this system's complete world view inline at its schedule boundary"
)]
fn present_local_attacks(
    timeline: Res<LocalTimeline>,
    mut ledger: ResMut<LocalAttackLedger>,
    mut stats: ResMut<LocalAttackStats>,
    mut presented: MessageWriter<LocalAttackPresented>,
    owners: Query<
        (
            (Entity, &NetworkEntityId, &TeamId),
            &OwnerPrediction,
            Option<&ActionState<FighterInput>>,
            Option<&ResolvedMatchLoadout>,
            Option<&WeaponState>,
            Option<&AbilityState>,
            Option<&ActiveEffects>,
        ),
        (With<Fighter>, With<Controlled>),
    >,
) {
    let tick = timeline.tick();
    let Some(((fighter, owner, team), prediction, action, loadout, weapon, ability, effects)) =
        owners.iter().next()
    else {
        return;
    };
    ledger.track(fighter);
    let input = action
        .map(|action| action.0)
        .filter(|input| input.is_valid())
        .unwrap_or_default();
    let ultimate_held = input.gameplay_buttons & FighterInput::ULTIMATE != 0;
    let ultimate_pressed = ultimate_held && !ledger.ultimate_was_held;
    ledger.ultimate_was_held = ultimate_held;
    let (Some(simulation_tick), Some(loadout)) = (prediction.simulation_tick_at(tick), loadout)
    else {
        return;
    };
    let suspended = prediction.is_following_authority()
        || effects.is_some_and(|effects| effects.is_frozen(simulation_tick));
    if suspended {
        return;
    }
    let mut announce = |kind| {
        presented.write(LocalAttackPresented {
            fighter,
            owner: *owner,
            team: *team,
            kind,
            origin: prediction.presented_position(),
            facing: prediction.facing,
            simulation_tick,
        });
    };
    let ultimate = loadout.ultimate;
    if ultimate_pressed
        && ultimate.kind != UltimateKind::Dash
        && ability.is_some_and(|state| local_ultimate_ready(*state, ultimate.charge_policy.maximum))
    {
        announce(LocalAttackKind::Ultimate(ultimate.kind));
        stats.ultimates_presented = stats.ultimates_presented.saturating_add(1);
    }
    let targeted_ultimate_held = ultimate_held
        && ultimate.kind.activation_style() == crate::builds::UltimateActivationStyle::Targeted;
    let fire_held = input.gameplay_buttons & FighterInput::PRIMARY_FIRE != 0;
    let Some(weapon) = weapon else {
        return;
    };
    let recipe = &loadout.primary_weapon.recipe;
    if !fire_held
        || targeted_ultimate_held
        || !local_fire_ready(LocalFireGate {
            simulation_tick,
            weapon: *weapon,
            capacity: recipe.economy.capacity(),
            fire_cooldown_ticks: recipe.fire_cooldown_ticks,
            unconfirmed_shots: ledger.unconfirmed(),
            last_shot_simulation_tick: ledger.last_shot_simulation_tick,
        })
    {
        return;
    }
    let shot = ledger.record(tick, simulation_tick);
    announce(LocalAttackKind::PrimaryFire(shot));
    stats.presented = stats.presented.saturating_add(1);
    stats.last_presented_tick = Some(tick.0);
}

/// Pair authoritative own-source attacks with local shots and reject unconfirmed ones.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn reconcile_local_attacks(
    timeline: Res<LocalTimeline>,
    mut ledger: ResMut<LocalAttackLedger>,
    mut stats: ResMut<LocalAttackStats>,
    mut cues: MessageReader<DeduplicatedCombatCue>,
    owners: Query<(&NetworkEntityId, &OwnerPrediction), (With<Fighter>, With<Controlled>)>,
) {
    let Some((owner, prediction)) = owners.iter().next() else {
        cues.clear();
        return;
    };
    for DeduplicatedCombatCue(cue) in cues.read() {
        let CombatCue::AttackAccepted {
            attack_id,
            source,
            tick,
            ..
        } = cue
        else {
            continue;
        };
        if source != owner {
            continue;
        }
        match ledger.observe_own_attack(*attack_id, *tick) {
            OwnAttackObservation::Matched(_) => {
                stats.confirmed = stats.confirmed.saturating_add(1);
            }
            OwnAttackObservation::Unmatched => {
                stats.unmatched_own_attacks = stats.unmatched_own_attacks.saturating_add(1);
            }
            OwnAttackObservation::Repeated => {}
        }
    }
    let rejected = ledger.expire(prediction.last_confirmed_tick(), timeline.tick());
    stats.rejected = stats.rejected.saturating_add(rejected);
}

#[cfg(test)]
mod tests;
