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
//!
//! Ultimate casts follow the same timeline. A non-Dash cast is announced on the ULTIMATE rising
//! edge with full replicated charge; presentation plays the shoot motion, a short launch flash at
//! the caster, and, for ultimates whose activation sound is a launch
//! ([`presents_activation_sound_locally`]), the activation sound. The ledger pairs the cast with
//! the authoritative activation signal of the same kind (its activation cue or the spawned
//! sentry/concealment field, within the same tick tolerance) and records that signal as an
//! activation echo, so the authoritative activation sound does not replay. Area outcomes (fields,
//! scans, strikes, sentry placement, cloak state) and their effects stay authoritative.

use crate::prediction::{OwnerPrediction, OwnerPredictionSet};
use bevy::prelude::*;
use brawler::builds::{AbilityPhase, AbilityState, ResolvedMatchLoadout, UltimateKind};
use brawler::combat::{
    ActiveEffects, AttackId, CombatCue, CombatEventId, DeduplicatedCombatCue, TeamId, WeaponPhase,
    WeaponState,
};
use brawler::protocol::{Fighter, FighterInput, NetworkEntityId};
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
/// Locally presented ultimate casts retained for matching (one charge allows one at a time).
pub const MAX_LOCAL_ULTIMATE_CASTS: usize = 4;
/// Authoritative activation signals retained as echoes for the presentation producers.
pub const MAX_ULTIMATE_ACTIVATION_ECHOES: usize = 8;

/// Client-local identity of one locally presented primary-fire shot. Never sent on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalShotId(pub u64);

/// Client-local identity of one locally presented ultimate cast. Never sent on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalCastId(pub u64);

/// What the local player started on this input tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalAttackKind {
    PrimaryFire(LocalShotId),
    Ultimate {
        kind: UltimateKind,
        cast: LocalCastId,
    },
}

/// Whether an ultimate's activation sound is a launch the client may play on the input frame.
///
/// Self Cloak, Reveal Scan, the elemental fields, Sentry, and Concealment Field play their
/// activation sound when the cast starts. Demolition Strike's sound is its explosion at the
/// target (an area outcome) and Big Blob has no activation sound, so both stay authoritative.
/// Dash is not cast locally at all: its sound accompanies the authoritative dash motion.
#[must_use]
pub const fn presents_activation_sound_locally(kind: UltimateKind) -> bool {
    match kind {
        UltimateKind::SelfCloak
        | UltimateKind::RevealScan
        | UltimateKind::CryogenicField
        | UltimateKind::FireField
        | UltimateKind::PoisonField
        | UltimateKind::RestorationField
        | UltimateKind::Sentry
        | UltimateKind::ConcealmentField => true,
        UltimateKind::Dash | UltimateKind::DemolitionStrike | UltimateKind::BigBlob => false,
    }
}

/// Whether the server announces this ultimate's activation with a cue or a replicated entity the
/// ledger can pair (Big Blob has neither; Dash is never cast locally).
#[must_use]
pub(crate) const fn has_activation_signal(kind: UltimateKind) -> bool {
    !matches!(kind, UltimateKind::BigBlob | UltimateKind::Dash)
}

/// One authoritative activation signal of the local player's ultimate whose sound already
/// played on the input frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UltimateActivationEcho {
    /// An activation combat cue (Self Cloak, Reveal Scan, elemental field).
    Cue(CombatEventId),
    /// A replicated activation entity (Sentry, Concealment Field).
    Spawned(Entity),
}

/// Server agreement with one locally presented ultimate cast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalCastStatus {
    Pending,
    Confirmed,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LocalUltimateCast {
    id: LocalCastId,
    kind: UltimateKind,
    local_tick: Tick,
    simulation_tick: u64,
    status: LocalCastStatus,
    /// The authoritative activation signal was already paired with this cast.
    echoed: bool,
    /// Replicated ability state has not yet shown the charge spent since this cast.
    awaiting_spent: bool,
}

/// How one authoritative own activation signal related to local presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OwnActivationObservation {
    /// The signal echoes a locally presented cast; its activation sound must not replay.
    Matched(LocalCastId),
    /// The server activated without a matching local cast; the signal presents it as before.
    Unmatched,
    /// The signal was already observed.
    Repeated,
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
    casts: VecDeque<LocalUltimateCast>,
    ultimate_echoes: VecDeque<UltimateActivationEcho>,
    casts_confirmed: u64,
}

impl LocalAttackLedger {
    /// Forget everything when the controlled fighter changes (new match, rejoin).
    fn track(&mut self, fighter: Entity) {
        if self.fighter != Some(fighter) {
            *self = Self {
                fighter: Some(fighter),
                next_id: self.next_id,
                casts_confirmed: self.casts_confirmed,
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

    #[must_use]
    pub fn cast_status(&self, cast: LocalCastId) -> Option<LocalCastStatus> {
        self.casts
            .iter()
            .find(|candidate| candidate.id == cast)
            .map(|cast| cast.status)
    }

    /// Whether a previous cast still occupies the charge: the server has not answered it yet, or
    /// it confirmed the activation while the replicated charge still reads full.
    #[must_use]
    pub fn ultimate_outstanding(&self) -> bool {
        self.casts.iter().any(|cast| {
            cast.status == LocalCastStatus::Pending
                || (cast.status == LocalCastStatus::Confirmed && cast.awaiting_spent)
        })
    }

    /// Whether an authoritative activation cue's sound already played on the input frame.
    #[must_use]
    pub fn is_activation_echo_cue(&self, event_id: CombatEventId) -> bool {
        self.ultimate_echoes
            .contains(&UltimateActivationEcho::Cue(event_id))
    }

    /// Whether a replicated activation entity's spawn sound already played on the input frame.
    #[must_use]
    pub fn is_activation_echo_entity(&self, entity: Entity) -> bool {
        self.ultimate_echoes
            .contains(&UltimateActivationEcho::Spawned(entity))
    }

    /// Record one locally presented ultimate cast.
    pub(crate) fn record_cast(
        &mut self,
        kind: UltimateKind,
        local_tick: Tick,
        simulation_tick: u64,
    ) -> LocalCastId {
        let id = LocalCastId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.casts.push_back(LocalUltimateCast {
            id,
            kind,
            local_tick,
            simulation_tick,
            status: LocalCastStatus::Pending,
            echoed: false,
            awaiting_spent: true,
        });
        while self.casts.len() > MAX_LOCAL_ULTIMATE_CASTS {
            self.casts.pop_front();
        }
        id
    }

    /// Fold one authoritative activation signal of the local player's ultimate into the ledger.
    /// `tick` is the authoritative activation tick; `echo` is the signal presentation skips.
    pub(crate) fn observe_own_activation(
        &mut self,
        kind: UltimateKind,
        tick: u64,
        echo: Option<UltimateActivationEcho>,
    ) -> OwnActivationObservation {
        if echo.is_some_and(|echo| self.ultimate_echoes.contains(&echo)) {
            return OwnActivationObservation::Repeated;
        }
        let matched = self.casts.iter_mut().find(|cast| {
            cast.kind == kind
                && !cast.echoed
                && tick + MATCH_EARLY_TOLERANCE_TICKS >= cast.simulation_tick
                && tick <= cast.simulation_tick + MATCH_LATE_TOLERANCE_TICKS
        });
        let Some(cast) = matched else {
            return OwnActivationObservation::Unmatched;
        };
        cast.echoed = true;
        if cast.status != LocalCastStatus::Confirmed {
            cast.status = LocalCastStatus::Confirmed;
            self.casts_confirmed = self.casts_confirmed.saturating_add(1);
        }
        let matched = cast.id;
        if let Some(echo) = echo.filter(|_| presents_activation_sound_locally(kind)) {
            self.ultimate_echoes.push_back(echo);
            while self.ultimate_echoes.len() > MAX_ULTIMATE_ACTIVATION_ECHOES {
                self.ultimate_echoes.pop_front();
            }
        }
        OwnActivationObservation::Matched(matched)
    }

    /// Replicated ability state no longer reads a full, ready charge: no cast blocks a new one.
    /// A pending cast of an ultimate without its own activation signal (Big Blob) is confirmed by
    /// the spent charge; every other kind is confirmed only by its activation signal, so a charge
    /// spent another way never confirms a rejected cast. Returns how many casts this confirmed.
    pub(crate) fn observe_charge_spent(&mut self) -> u64 {
        let mut confirmed = 0;
        for cast in &mut self.casts {
            cast.awaiting_spent = false;
            if cast.status == LocalCastStatus::Pending && !has_activation_signal(cast.kind) {
                cast.status = LocalCastStatus::Confirmed;
                confirmed += 1;
            }
        }
        self.casts_confirmed = self.casts_confirmed.saturating_add(confirmed);
        confirmed
    }

    /// Reject pending casts the server has had time to confirm. Returns how many changed.
    pub(crate) fn expire_casts(&mut self, confirmed_tick: Option<Tick>, now: Tick) -> u64 {
        let mut rejected = 0;
        for cast in &mut self.casts {
            if cast.status != LocalCastStatus::Pending {
                continue;
            }
            let confirmed_past = confirmed_tick
                .is_some_and(|confirmed| confirmed - cast.local_tick >= CONFIRMATION_MARGIN_TICKS);
            let timed_out = now - cast.local_tick >= ABSOLUTE_CONFIRMATION_TIMEOUT_TICKS;
            if confirmed_past || timed_out {
                cast.status = LocalCastStatus::Rejected;
                rejected += 1;
            }
        }
        rejected
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
    /// Ultimate casts the server activated.
    pub ultimates_confirmed: u64,
    /// Ultimate casts the server never activated.
    pub ultimates_rejected: u64,
    /// Authoritative activation signals paired with a local cast (their sound was suppressed
    /// when the ultimate presents its activation sound locally).
    pub ultimate_activation_echoes: u64,
    /// Own ultimate activations observed without a matching local cast (presented as before).
    pub unmatched_own_activations: u64,
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
                    .in_set(brawler::gameplay::GameplaySet::Fire),
            )
            .configure_sets(
                Update,
                LocalAttackSet::Reconcile
                    .after(brawler::combat::client::CombatClientSet::Ingest)
                    .before(brawler::combat::client::CombatClientSet::Sync),
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
        && !ledger.ultimate_outstanding()
        && ability.is_some_and(|state| local_ultimate_ready(*state, ultimate.charge_policy.maximum))
    {
        let cast = ledger.record_cast(ultimate.kind, tick, simulation_tick);
        announce(LocalAttackKind::Ultimate {
            kind: ultimate.kind,
            cast,
        });
        stats.ultimates_presented = stats.ultimates_presented.saturating_add(1);
    }
    let targeted_ultimate_held = ultimate_held
        && ultimate.kind.activation_style() == brawler::builds::UltimateActivationStyle::Targeted;
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

/// The controlled fighter's identity and replicated ultimate facts reconciliation reads.
type ReconciledOwner<'a> = (
    &'a NetworkEntityId,
    &'a TeamId,
    &'a OwnerPrediction,
    Option<&'a ResolvedMatchLoadout>,
    Option<&'a AbilityState>,
);

/// Authoritative activation entities spawned since the previous reconciliation.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct SpawnedActivations<'w, 's> {
    sentries: Query<
        'w,
        's,
        (
            Entity,
            &'static brawler::abilities::SentryIdentity,
            &'static brawler::abilities::SentryDeadline,
        ),
        Added<brawler::abilities::Sentry>,
    >,
    concealment_fields: Query<
        'w,
        's,
        (Entity, &'static brawler::concealment::ConcealmentFieldState),
        Added<brawler::concealment::ConcealmentFieldState>,
    >,
}

/// Pair authoritative own-source attacks and ultimate activations with local presentations and
/// reject unconfirmed ones.
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
    spawned: SpawnedActivations,
    owners: Query<ReconciledOwner, (With<Fighter>, With<Controlled>)>,
) {
    let Some((owner, team, prediction, loadout, ability)) = owners.iter().next() else {
        cues.clear();
        return;
    };
    let ultimate = loadout.map(|loadout| loadout.ultimate);
    let mut activations = Vec::new();
    for DeduplicatedCombatCue(cue) in cues.read() {
        match cue {
            CombatCue::AttackAccepted {
                attack_id,
                source,
                tick,
                ..
            } if source == owner => match ledger.observe_own_attack(*attack_id, *tick) {
                OwnAttackObservation::Matched(_) => {
                    stats.confirmed = stats.confirmed.saturating_add(1);
                }
                OwnAttackObservation::Unmatched => {
                    stats.unmatched_own_attacks = stats.unmatched_own_attacks.saturating_add(1);
                }
                OwnAttackObservation::Repeated => {}
            },
            _ => {
                if let Some(activation) = own_activation_cue(cue, *owner, *team) {
                    activations.push(activation);
                }
            }
        }
    }
    if let Some(ultimate) = ultimate {
        for (entity, identity, deadline) in &spawned.sentries {
            if identity.owner_network_id != *owner {
                continue;
            }
            let brawler::builds::UltimateParameters::Sentry { lifetime_ticks, .. } =
                ultimate.parameters
            else {
                continue;
            };
            activations.push((
                UltimateKind::Sentry,
                deadline.expires_at_tick.saturating_sub(lifetime_ticks),
                Some(UltimateActivationEcho::Spawned(entity)),
            ));
        }
        for (entity, field) in &spawned.concealment_fields {
            if field.team == *team {
                activations.push((
                    UltimateKind::ConcealmentField,
                    field.activated_at_tick,
                    Some(UltimateActivationEcho::Spawned(entity)),
                ));
            }
        }
    }
    for (kind, tick, echo) in activations {
        // Teammates share team-scoped signals; only the local fighter's own kind can pair.
        if ultimate.is_none_or(|ultimate| ultimate.kind != kind) {
            continue;
        }
        match ledger.observe_own_activation(kind, tick, echo) {
            OwnActivationObservation::Matched(_) => {
                stats.ultimate_activation_echoes =
                    stats.ultimate_activation_echoes.saturating_add(1);
            }
            OwnActivationObservation::Unmatched => {
                stats.unmatched_own_activations = stats.unmatched_own_activations.saturating_add(1);
            }
            OwnActivationObservation::Repeated => {}
        }
    }
    if let (Some(ultimate), Some(ability)) = (ultimate, ability)
        && !local_ultimate_ready(*ability, ultimate.charge_policy.maximum)
    {
        ledger.observe_charge_spent();
    }
    let rejected = ledger.expire(prediction.last_confirmed_tick(), timeline.tick());
    stats.rejected = stats.rejected.saturating_add(rejected);
    let rejected_casts = ledger.expire_casts(prediction.last_confirmed_tick(), timeline.tick());
    stats.ultimates_rejected = stats.ultimates_rejected.saturating_add(rejected_casts);
    stats.ultimates_confirmed = ledger.casts_confirmed;
}

/// The ultimate kind, authoritative tick, and echo identity of an activation cue that may be the
/// local fighter's own. Reveal Scan cues carry only the revealing team.
fn own_activation_cue(
    cue: &CombatCue,
    owner: NetworkEntityId,
    team: TeamId,
) -> Option<(UltimateKind, u64, Option<UltimateActivationEcho>)> {
    let echo = |event_id: &CombatEventId| Some(UltimateActivationEcho::Cue(*event_id));
    match cue {
        CombatCue::SelfCloakActivated {
            event_id,
            tick,
            source,
            ..
        } if *source == owner => Some((UltimateKind::SelfCloak, *tick, echo(event_id))),
        CombatCue::RevealScanActivated {
            event_id,
            tick,
            revealing_team,
            ..
        } if *revealing_team == team => Some((UltimateKind::RevealScan, *tick, echo(event_id))),
        CombatCue::ElementalFieldActivated {
            event_id,
            tick,
            source,
            kind,
            ..
        } if *source == owner => Some((elemental_ultimate_kind(*kind), *tick, echo(event_id))),
        CombatCue::DemolitionStrikeActivated { tick, source, .. } if *source == owner => {
            Some((UltimateKind::DemolitionStrike, *tick, None))
        }
        _ => None,
    }
}

const fn elemental_ultimate_kind(kind: brawler::combat::ElementalFieldKind) -> UltimateKind {
    use brawler::combat::ElementalFieldKind as K;
    match kind {
        K::Cryogenic => UltimateKind::CryogenicField,
        K::Fire => UltimateKind::FireField,
        K::Poison => UltimateKind::PoisonField,
        K::Restoration => UltimateKind::RestorationField,
    }
}

#[cfg(test)]
mod tests;
