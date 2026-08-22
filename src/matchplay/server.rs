//! Server-owned fixed-tick common match lifecycle shared by every installed game mode.
//!
//! This plugin owns the match root, roster snapshot, phase machine, forfeit precedence,
//! restart transaction, cleanup, common telemetry, and the bounded mode-rule outcome handoff.
//! Installed mode plugins own only their scoring/progress rules through `MatchSet::DeadlineRules`
//! and `MatchSet::ModeRules` plus their in-place restart reset.

use super::{
    ActiveCombatant, AuthoritativeFighterLifecyclePlugin, FighterLifecycleConfig, FighterReset,
    MatchClock, MatchId, MatchMember, MatchOutcomeDiagnostics, MatchParticipant,
    MatchParticipantSummary, MatchPhase, MatchRestartSet, MatchResult, MatchRoot, MatchSet,
    MatchState, MatchTelemetry, MatchTelemetryContext, ModeSummary, ResolvedMatchCapacity,
    RespawnState, SpawnCandidate, SpawnProtection, WipeoutState, WipeoutSummary,
    complete_fighter_lifecycle, configure_match_schedule, fighter_runtime_values,
    reset_fighter_runtime, select_spawn,
};
use crate::{
    combat::{
        ActiveAttackTrackers, CombatOutbox, CombatOutcomeFacts, Defeated, FighterDefinitions,
        MeleeAttack, PendingDelivery, PendingPayload, SpawnState, TeamId, WeaponDefinitions,
        WeaponTelemetry,
    },
    map::{MapStartupSet, ResolvedMap, WIPEOUT_MODE_DEFINITION},
    protocol::{Fighter, FighterInput, NetworkEntityId, PlayerId},
    timing::SimulationTick,
};
use avian2d::prelude::Position;
use bevy::prelude::*;
use lightyear::prelude::input::native::{ActionState, NativeBuffer};
use lightyear::prelude::{ControlledBy, Disconnected, LinkOf, NetworkTarget, Replicate};
use std::collections::{BTreeMap, BTreeSet};

/// Revision of the common lifecycle rules fragment on its own.
pub const MATCH_LIFECYCLE_RULES_REVISION: u16 = 1;

/// Common, mode-neutral match lifecycle rules. The installed mode layers its own scoring
/// rules on top; `MatchState::rules_revision` describes the validated composition.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct MatchLifecycleRules {
    pub team_count: u8,
    pub minimum_participants_per_team: u8,
    pub maximum_participants_per_team: u8,
    pub countdown_ticks: u64,
    pub active_limit_ticks: u64,
    pub respawn_delay_ticks: u64,
    pub spawn_protection_ticks: u64,
    pub completed_input_lock_ticks: u64,
    pub movement_displacement_epsilon: f32,
    pub retained_match_summaries: usize,
    pub retained_match_records: usize,
}

impl Default for MatchLifecycleRules {
    fn default() -> Self {
        Self {
            team_count: 2,
            minimum_participants_per_team: 1,
            maximum_participants_per_team: 3,
            countdown_ticks: 180,
            active_limit_ticks: 10_800,
            respawn_delay_ticks: 180,
            spawn_protection_ticks: 90,
            completed_input_lock_ticks: 60,
            movement_displacement_epsilon: 0.25,
            retained_match_summaries: 32,
            retained_match_records: 2_048,
        }
    }
}

impl MatchLifecycleRules {
    pub fn validate(self) -> Result<Self, &'static str> {
        if self.team_count != 2 {
            return Err("match lifecycle requires exactly two teams");
        }
        if self.minimum_participants_per_team == 0
            || self.minimum_participants_per_team > self.maximum_participants_per_team
            || self.maximum_participants_per_team > 3
        {
            return Err("invalid match team capacity");
        }
        if self.countdown_ticks == 0
            || self.active_limit_ticks == 0
            || self.respawn_delay_ticks == 0
            || self.spawn_protection_ticks == 0
            || self.completed_input_lock_ticks == 0
        {
            return Err("match lifecycle deadlines must be nonzero");
        }
        if self
            .countdown_ticks
            .checked_add(self.active_limit_ticks)
            .is_none()
            || self
                .respawn_delay_ticks
                .checked_add(self.spawn_protection_ticks)
                .is_none()
            || self
                .active_limit_ticks
                .checked_add(self.completed_input_lock_ticks)
                .is_none()
        {
            return Err("match lifecycle deadline combination overflows");
        }
        if !self.movement_displacement_epsilon.is_finite()
            || self.movement_displacement_epsilon < 0.0
            || self.retained_match_summaries == 0
            || self.retained_match_records == 0
        {
            return Err("invalid match telemetry limits");
        }
        Ok(self)
    }
}

/// Code-owned selection of which mode's rules the server composes onto the common lifecycle.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchModeSetup {
    pub mode_definition_id: crate::map::ModeDefinitionId,
    pub rules_revision: u16,
}

impl Default for MatchModeSetup {
    fn default() -> Self {
        Self {
            mode_definition_id: WIPEOUT_MODE_DEFINITION,
            rules_revision: super::WIPEOUT_RULES_REVISION,
        }
    }
}

/// Why an installed mode resolved one completion result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModeOutcomeCause {
    Threshold,
    Timeout,
}

/// One validated mode-rule completion result offered by the installed mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PendingModeRuleOutcome {
    pub match_id: MatchId,
    pub evaluated_tick: u64,
    pub cause: ModeOutcomeCause,
    pub result: MatchResult,
}

/// Bounded server-only common<->mode completion-result handoff. The installed mode may write
/// the empty slot from its deadline system or its post-damage rules system; the common
/// consumers always `take()` it, so no outcome survives into gameplay or the next tick.
#[derive(Resource, Default, Debug)]
pub(crate) struct ModeRuleOutcome {
    pending: Option<PendingModeRuleOutcome>,
}

impl ModeRuleOutcome {
    /// Offer one outcome into the empty slot. A second write in one tick is rejected.
    pub(crate) fn offer(&mut self, pending: PendingModeRuleOutcome) -> bool {
        if self.pending.is_some() {
            return false;
        }
        self.pending = Some(pending);
        true
    }

    pub(crate) fn take(&mut self) -> Option<PendingModeRuleOutcome> {
        self.pending.take()
    }

    pub(crate) fn clear(&mut self) {
        self.pending = None;
    }
}

/// Allocated by the common restart prepare system and consumed by the common commit system,
/// with the installed mode's in-place state reset running between them.
#[derive(Resource, Default, Debug)]
pub(crate) struct PendingMatchRestart(Option<PendingMatchRestartSlot>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PendingMatchRestartSlot {
    pub previous_id: MatchId,
    pub next_id: MatchId,
    pub restart_tick: u64,
}

impl PendingMatchRestart {
    pub(crate) fn slot(&self) -> Option<PendingMatchRestartSlot> {
        self.0
    }

    #[cfg(feature = "balance-lab")]
    pub(crate) fn stage(&mut self, slot: PendingMatchRestartSlot) -> bool {
        if self.0.is_some() {
            return false;
        }
        self.0 = Some(slot);
        true
    }

    /// In-crate test staging for the environment-reset transaction.
    #[cfg(test)]
    pub(crate) fn stage_for_test(&mut self, slot: PendingMatchRestartSlot) {
        self.0 = Some(slot);
    }
}

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum RestartBuildPolicy {
    #[default]
    Reconfirm,
    Retain,
}

#[derive(Resource, Debug)]
pub(crate) struct NextMatchId(u128);

#[derive(Resource, Default, Debug)]
struct RespawnOrdinals(BTreeMap<u64, u64>);

#[derive(Resource, Default, Debug)]
struct PriorMatchPositions(BTreeMap<u64, Vec2>);

#[derive(Resource, Default, Debug)]
struct KnownMatchRoster {
    match_id: Option<MatchId>,
    players: BTreeSet<u64>,
}

#[derive(Resource, Default, Debug)]
struct RestartCleanupEpoch(Option<MatchId>);

/// Fixed-tick snapshot of the accepted, currently connected participant set for the root
/// match. Forfeit resolution and Hot Zone occupancy both read this same snapshot.
#[derive(Resource, Default, Debug)]
pub(crate) struct ConnectedMatchRoster {
    pub(crate) match_id: Option<MatchId>,
    pub(crate) counts: [u8; 2],
    pub(crate) ready: bool,
    pub(crate) participants: Vec<MatchParticipantSummary>,
    pub(crate) participant_departed: bool,
    pub(crate) connected_network_ids: BTreeSet<u64>,
}

#[derive(Resource, Default, Debug)]
struct PendingMatchActivation(Option<MatchId>);

impl Default for NextMatchId {
    fn default() -> Self {
        Self(1)
    }
}

impl NextMatchId {
    /// Keep local restart allocation strictly after an externally assigned worker identity.
    ///
    /// A routed worker starts from its manifest's `u128` `MatchId` rather than this local counter.
    /// Advancing the counter here prevents the first restart from reusing a low default value.
    fn observe(&mut self, match_id: MatchId) {
        let next = match_id.0.saturating_add(1);
        self.0 = self.0.max(next);
    }

    pub(crate) fn allocate(&mut self) -> MatchId {
        let id = MatchId(self.0.max(1));
        self.0 =
            id.0.checked_add(1)
                .expect("match identifier space exhausted");
        id
    }
}

/// Operational connection limits may not under-provision the selected gameplay profile.
fn reject_under_provisioned_connection_capacity(app: &App, capacity: &ResolvedMatchCapacity) {
    if let Some(config) = app
        .world()
        .get_resource::<crate::config::ServerNetworkConfig>()
        && config.max_clients < usize::from(capacity.maximum_active_fighters)
    {
        panic!(
            "server connection capacity {} under-provisions the selected profile's {} active fighters",
            config.max_clients, capacity.maximum_active_fighters
        );
    }
}

/// Map validation proves the map satisfies its mode definition; this composition step
/// proves it satisfies the resolved rules: identical team slots and enough
/// spawn points for every simultaneous participant. Runs once the selected map is
/// installed, before the match root exists.
#[allow(clippy::needless_pass_by_value)]
pub(super) fn validate_capacity_against_selected_map(
    capacity: Res<ResolvedMatchCapacity>,
    resolved: Option<Res<ResolvedMap>>,
) {
    let Some(resolved) = resolved else {
        return;
    };
    if let Err(reason) = capacity.validate_against_map(&resolved.snapshot) {
        panic!("resolved match capacity does not satisfy the selected map: {reason}");
    }
}

/// Ordered post-update pipeline shared by every mode: consume the mode outcome, resolve
/// respawns, record telemetry, and capture the match summary with deferred boundaries.
fn register_match_outcome_pipeline(app: &mut App) {
    app.add_systems(
        FixedPostUpdate,
        (
            consume_mode_rule_outcome,
            ApplyDeferred,
            handle_defeated_respawns,
            record_match_telemetry,
            clear_combat_facts,
            ApplyDeferred,
            crate::abilities::request_sentry_lifecycle_cleanup,
            crate::abilities::cleanup_requested_sentries,
            ApplyDeferred,
            capture_match_summary,
        )
            .chain()
            .in_set(MatchSet::Outcomes),
    );
}

/// The common authoritative match lifecycle plugin. Compose with exactly one installed mode
/// plugin (`WipeoutModePlugin` or `HotZoneModePlugin`).
pub struct AuthoritativeMatchPlugin;

impl Plugin for AuthoritativeMatchPlugin {
    fn build(&self, app: &mut App) {
        let configured = app
            .world()
            .get_resource::<MatchLifecycleRules>()
            .copied()
            .unwrap_or_default();
        let rules = configured
            .validate()
            .expect("match lifecycle rules must be valid");
        let capacity = ResolvedMatchCapacity::from_rules(&rules)
            .expect("validated match rules resolve a checked capacity");
        reject_under_provisioned_connection_capacity(app, &capacity);
        if app.world().get_resource::<MatchModeSetup>().is_none() {
            app.insert_resource(MatchModeSetup::default());
        }
        app.insert_resource(capacity);
        configure_match_schedule(app);
        app.add_plugins(AuthoritativeFighterLifecyclePlugin)
            .insert_resource(rules)
            .insert_resource(FighterLifecycleConfig {
                spawn_protection_ticks: rules.spawn_protection_ticks,
            })
            .init_resource::<NextMatchId>()
            .init_resource::<RespawnOrdinals>()
            .init_resource::<MatchTelemetry>()
            .init_resource::<MatchOutcomeDiagnostics>()
            .init_resource::<ModeRuleOutcome>()
            .init_resource::<PendingMatchRestart>()
            .init_resource::<RestartBuildPolicy>()
            .init_resource::<PriorMatchPositions>()
            .init_resource::<KnownMatchRoster>()
            .init_resource::<RestartCleanupEpoch>()
            .init_resource::<ConnectedMatchRoster>()
            .init_resource::<PendingMatchActivation>()
            .add_systems(
                Startup,
                (
                    validate_capacity_against_selected_map,
                    initialize_match_root,
                )
                    .chain()
                    .after(MapStartupSet::Instantiate),
            )
            .add_systems(
                FixedUpdate,
                (
                    refresh_match_roster,
                    advance_waiting_and_countdown,
                    activate_started_match,
                )
                    .chain()
                    .in_set(MatchSet::Lifecycle),
            )
            .add_systems(
                FixedUpdate,
                prepare_match_restart.in_set(MatchRestartSet::Prepare),
            )
            .add_systems(
                FixedUpdate,
                commit_match_restart.in_set(MatchRestartSet::Commit),
            )
            .add_systems(
                FixedUpdate,
                (cleanup_restarted_match, select_due_respawn_spawns)
                    .chain()
                    .in_set(MatchSet::Lifecycle)
                    .after(MatchRestartSet::Commit),
            )
            .add_systems(
                FixedUpdate,
                resolve_pregame_outcomes.in_set(MatchSet::PreGameOutcomes),
            )
            .add_systems(
                FixedPostUpdate,
                prepare_mode_rule_facts.in_set(MatchSet::ModeRules),
            );
        register_match_outcome_pipeline(app);
        app.add_systems(
            FixedPostUpdate,
            publish_match_clock
                .in_set(crate::combat::CombatSet::Finalize)
                .before(crate::gameplay::advance_simulation_tick),
        )
        .add_systems(
            FixedPostUpdate,
            record_match_movement
                .after(avian2d::prelude::PhysicsSystems::StepSimulation)
                .before(crate::combat::CombatSet::ProjectileSweep),
        );
    }
}

/// Offer one mode outcome, counting rejected duplicate writes under the common diagnostics.
pub(crate) fn offer_mode_rule_outcome(
    outcomes: &mut ModeRuleOutcome,
    diagnostics: &mut MatchOutcomeDiagnostics,
    pending: PendingModeRuleOutcome,
) {
    if !outcomes.offer(pending) {
        diagnostics.duplicate_mode_outcome = diagnostics.duplicate_mode_outcome.saturating_add(1);
    }
}

#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
fn record_match_movement(
    rules: Res<MatchLifecycleRules>,
    mut prior: ResMut<PriorMatchPositions>,
    mut telemetry: ResMut<MatchTelemetry>,
    fighters: Query<
        (&PlayerId, &Position),
        (With<Fighter>, With<ActiveCombatant>, Without<Defeated>),
    >,
) {
    let active_ids: BTreeSet<_> = fighters.iter().map(|(player, _)| player.0).collect();
    prior.0.retain(|player, _| active_ids.contains(player));
    for (player, position) in &fighters {
        let moved = prior.0.get(&player.0).is_some_and(|previous| {
            previous.distance(position.0) > rules.movement_displacement_epsilon
        });
        telemetry.record_movement(player.0, moved);
        prior.0.insert(player.0, position.0);
    }
}

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn initialize_match_root(
    mut commands: Commands,
    mut ids: ResMut<NextMatchId>,
    setup: Res<MatchModeSetup>,
    role: Option<Res<crate::server::ServerRoleResource>>,
    roots: Query<(), With<MatchRoot>>,
) {
    if roots.is_empty() {
        let manifest_match_id = role
            .as_deref()
            .and_then(crate::server::ServerRoleResource::match_worker_match_id);
        let match_id = if let Some(match_id) = manifest_match_id {
            ids.observe(match_id);
            match_id
        } else {
            ids.allocate()
        };
        commands.spawn((
            MatchRoot,
            MatchState {
                match_id,
                mode_definition_id: setup.mode_definition_id,
                phase: MatchPhase::Waiting,
                rules_revision: setup.rules_revision,
            },
            MatchClock {
                match_id,
                completed_tick: 0,
            },
            Replicate::to_clients(NetworkTarget::All),
        ));
    }
}

/// A participant belongs to the connected roster only while its owning session is accepted,
/// its controlling link is present and not disconnected, and its match membership is current.
/// Fighters without a controlling link (server-local test entities) count as connected.
fn participant_is_connected(
    controlled: Option<&ControlledBy>,
    links: &Query<(Entity, Has<Disconnected>), With<LinkOf>>,
) -> bool {
    let Some(controlled) = controlled else {
        return true;
    };
    links
        .get(controlled.owner)
        .is_ok_and(|(_, disconnected)| !disconnected)
}

#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
fn refresh_match_roster(
    rules: Res<MatchLifecycleRules>,
    roots: Query<&MatchState, With<MatchRoot>>,
    links: Query<(Entity, Has<Disconnected>), With<LinkOf>>,
    mut known: ResMut<KnownMatchRoster>,
    mut current: ResMut<ConnectedMatchRoster>,
    mut telemetry: ResMut<MatchTelemetry>,
    participants: Query<
        (
            &PlayerId,
            &NetworkEntityId,
            &MatchParticipant,
            &TeamId,
            &crate::builds::SelectedBuild,
            Option<&crate::builds::ResolvedMatchLoadout>,
            Option<&ControlledBy>,
        ),
        With<Fighter>,
    >,
) {
    let Ok(state) = roots.single() else { return };
    let mut counts = [0_u8; 2];
    let mut players = BTreeSet::new();
    let mut connected_network_ids = BTreeSet::new();
    let mut summaries = Vec::new();
    let mut ready = true;
    for (player, network_id, participant, team, build, selected, controlled) in &participants {
        if participant.match_id != state.match_id
            || team.0 > 1
            || !participant_is_connected(controlled, &links)
        {
            continue;
        }
        ready &= participant.ready && selected.is_some();
        players.insert(player.0);
        connected_network_ids.insert(network_id.0);
        counts[usize::from(team.0)] = counts[usize::from(team.0)].saturating_add(1);
        summaries.push(MatchParticipantSummary {
            player_id: player.0,
            network_entity_id: network_id.0,
            team: *team,
            selected_build: *build,
            weapon_preset: selected.and_then(|loadout| loadout.primary_weapon.source_preset_id),
            total_points: selected.map(|loadout| loadout.total_points),
            ultimate_id: selected.map(|loadout| loadout.ultimate.id),
            passive_ids: selected.map(|loadout| loadout.passives.map(|passive| passive.id)),
        });
        if matches!(state.phase, MatchPhase::Active { .. }) {
            telemetry.record_participant_active_tick(
                *team,
                selected.and_then(|loadout| loadout.primary_weapon.source_preset_id),
            );
        }
    }
    summaries.sort_by_key(|participant| participant.player_id);
    ready &= counts
        .into_iter()
        .all(|count| count >= rules.minimum_participants_per_team);
    let departed = known.match_id == Some(state.match_id)
        && known.players.difference(&players).next().is_some();
    if departed && matches!(state.phase, MatchPhase::Active { .. }) {
        telemetry.record_disconnects(known.players.difference(&players).count());
    }
    known.match_id = Some(state.match_id);
    known.players.clone_from(&players);
    *current = ConnectedMatchRoster {
        match_id: Some(state.match_id),
        counts,
        ready,
        participants: summaries,
        participant_departed: departed,
        connected_network_ids,
    };
}

#[allow(clippy::needless_pass_by_value)]
fn advance_waiting_and_countdown(
    tick: Res<SimulationTick>,
    rules: Res<MatchLifecycleRules>,
    roster: Res<ConnectedMatchRoster>,
    mut activation: ResMut<PendingMatchActivation>,
    mut roots: Query<&mut MatchState, With<MatchRoot>>,
    mut participants: Query<&mut MatchParticipant, With<Fighter>>,
) {
    let Ok(mut state) = roots.single_mut() else {
        return;
    };
    if roster.match_id != Some(state.match_id) {
        return;
    }
    match state.phase {
        MatchPhase::Waiting if roster.ready => {
            if let Some(starts_at_tick) = tick.0.checked_add(rules.countdown_ticks) {
                state.phase = MatchPhase::Countdown { starts_at_tick };
            }
        }
        MatchPhase::Countdown { .. } if roster.participant_departed || !roster.ready => {
            state.phase = MatchPhase::Waiting;
            for mut participant in &mut participants {
                if participant.match_id == state.match_id {
                    participant.ready = false;
                }
            }
        }
        MatchPhase::Countdown { starts_at_tick } if tick.0 >= starts_at_tick => {
            let Some(ends_at_tick) = tick.0.checked_add(rules.active_limit_ticks) else {
                return;
            };
            state.phase = MatchPhase::Active { ends_at_tick };
            activation.0 = Some(state.match_id);
        }
        _ => {}
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
fn activate_started_match(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    rules: Res<MatchLifecycleRules>,
    fighter_definitions: Res<FighterDefinitions>,
    weapon_definitions: Res<WeaponDefinitions>,
    weapon_telemetry: Res<WeaponTelemetry>,
    ability_telemetry: Res<crate::abilities::AbilityTelemetry>,
    resolved_map: Res<ResolvedMap>,
    content_fingerprint: Res<crate::content::GameplayContentFingerprint>,
    roster: Res<ConnectedMatchRoster>,
    mut activation: ResMut<PendingMatchActivation>,
    mut telemetry: ResMut<MatchTelemetry>,
    mut outcomes: ResMut<ModeRuleOutcome>,
    roots: Query<&MatchState, With<MatchRoot>>,
    fighters: Query<(
        Entity,
        &MatchParticipant,
        &crate::combat::FighterDefinitionId,
        &crate::builds::SelectedBuild,
        Option<&crate::builds::ResolvedMatchLoadout>,
        &SpawnState,
    )>,
) {
    let Some(match_id) = activation.0.take() else {
        return;
    };
    let Ok(state) = roots.single() else { return };
    if state.match_id != match_id || !matches!(state.phase, MatchPhase::Active { .. }) {
        return;
    }
    outcomes.clear();
    telemetry.begin_with_sources(match_id, tick.0, &weapon_telemetry, &ability_telemetry);
    telemetry.set_context(MatchTelemetryContext {
        map_identity: resolved_map.snapshot.identity,
        content_fingerprint: *content_fingerprint,
        rules_revision: state.rules_revision,
        participants: roster.participants.clone(),
    });
    for (entity, participant, fighter_id, build, loadout, spawn) in &fighters {
        if participant.match_id != match_id {
            continue;
        }
        let Some((maximum_health, ammunition)) = loadout.map_or_else(
            || {
                fighter_runtime_values(
                    *fighter_id,
                    build,
                    &fighter_definitions,
                    &weapon_definitions,
                )
            },
            |loadout| {
                Some((
                    loadout.fighter_stats.maximum_health,
                    loadout.primary_weapon.recipe.economy.capacity(),
                ))
            },
        ) else {
            continue;
        };
        reset_fighter_runtime(
            &mut commands,
            entity,
            FighterReset {
                maximum_health,
                ammunition,
                position: spawn.position,
                facing: spawn.facing,
                collision_mask: crate::movement::INDESTRUCTIBLE_TERRAIN_LAYER
                    | crate::movement::DESTRUCTIBLE_TERRAIN_LAYER,
                protection_until: Some(tick.0.saturating_add(rules.spawn_protection_ticks)),
                active: true,
            },
        );
        commands.entity(entity).insert((
            crate::builds::AbilityState::default(),
            crate::builds::PassiveRuntimeState::default(),
        ));
    }
}

#[must_use]
pub(crate) fn forfeit_result(counts: [u8; 2]) -> Option<MatchResult> {
    match (counts[0] == 0, counts[1] == 0) {
        (false, false) => None,
        (true, true) => Some(MatchResult::Draw),
        (true, false) => Some(MatchResult::Forfeit {
            winner: TeamId(1),
            departed_team: TeamId(0),
        }),
        (false, true) => Some(MatchResult::Forfeit {
            winner: TeamId(0),
            departed_team: TeamId(1),
        }),
    }
}

/// Commit one resolved result: transition the phase and lock every current fighter.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn commit_match_result<'a>(
    commands: &mut Commands,
    state: &mut MatchState,
    tick: u64,
    rules: &MatchLifecycleRules,
    result: MatchResult,
    participants: impl Iterator<Item = (Entity, &'a MatchParticipant)>,
) {
    state.phase = MatchPhase::Completed {
        completed_at_tick: tick,
        restart_unlocked_at_tick: tick
            .checked_add(rules.completed_input_lock_ticks)
            .expect("validated completion deadline cannot overflow"),
        result,
    };
    for (entity, participant) in participants {
        if participant.match_id == state.match_id {
            complete_fighter_lifecycle(commands, entity);
        }
    }
}

/// Pre-game outcome resolution: common forfeit precedence first, then the installed mode's
/// deadline outcome, both before any fighter lifecycle, input, movement, or combat runs.
#[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
fn resolve_pregame_outcomes(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    rules: Res<MatchLifecycleRules>,
    roster: Res<ConnectedMatchRoster>,
    mut outcomes: ResMut<ModeRuleOutcome>,
    mut diagnostics: ResMut<MatchOutcomeDiagnostics>,
    mut roots: Query<&mut MatchState, With<MatchRoot>>,
    participants: Query<(Entity, &MatchParticipant), With<Fighter>>,
) {
    let Ok(mut state) = roots.single_mut() else {
        return;
    };
    if !matches!(state.phase, MatchPhase::Active { .. }) {
        if outcomes.take().is_some() {
            diagnostics.stale_mode_outcome = diagnostics.stale_mode_outcome.saturating_add(1);
        }
        return;
    }
    if let Some(result) = forfeit_result(roster.counts) {
        // Forfeit has precedence and still takes/discards any same-tick deadline outcome.
        outcomes.clear();
        commit_match_result(
            &mut commands,
            &mut state,
            tick.0,
            &rules,
            result,
            participants.iter(),
        );
        return;
    }
    let Some(pending) = outcomes.take() else {
        return;
    };
    if pending.match_id != state.match_id {
        diagnostics.wrong_match_outcome = diagnostics.wrong_match_outcome.saturating_add(1);
        return;
    }
    if pending.evaluated_tick != tick.0 {
        diagnostics.wrong_tick_outcome = diagnostics.wrong_tick_outcome.saturating_add(1);
        return;
    }
    commit_match_result(
        &mut commands,
        &mut state,
        tick.0,
        &rules,
        pending.result,
        participants.iter(),
    );
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
pub(crate) fn prepare_match_restart(
    tick: Res<SimulationTick>,
    mut ids: ResMut<NextMatchId>,
    mut restart: ResMut<PendingMatchRestart>,
    roots: Query<&MatchState, With<MatchRoot>>,
    participants: Query<&MatchParticipant, With<Fighter>>,
) {
    let Ok(state) = roots.single() else {
        return;
    };
    let MatchPhase::Completed {
        restart_unlocked_at_tick,
        ..
    } = state.phase
    else {
        return;
    };
    if restart.slot().is_some()
        || tick.0 < restart_unlocked_at_tick
        || !participants
            .iter()
            .filter(|participant| participant.match_id == state.match_id)
            .all(|participant| participant.restart_ready)
    {
        return;
    }
    restart.0 = Some(PendingMatchRestartSlot {
        previous_id: state.match_id,
        next_id: ids.allocate(),
        restart_tick: tick.0,
    });
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
fn commit_match_restart(
    mut commands: Commands,
    mut restart: ResMut<PendingMatchRestart>,
    mut outcomes: ResMut<ModeRuleOutcome>,
    mut build_policy: ResMut<RestartBuildPolicy>,
    fighter_definitions: Res<FighterDefinitions>,
    weapon_definitions: Res<WeaponDefinitions>,
    mut roots: Query<(&mut MatchState, &mut MatchClock), With<MatchRoot>>,
    mut participants: Query<(Entity, &mut MatchParticipant), With<Fighter>>,
    fighters: Query<(
        Entity,
        &crate::combat::FighterDefinitionId,
        &crate::builds::SelectedBuild,
        Option<&crate::builds::ResolvedMatchLoadout>,
        &SpawnState,
    )>,
) {
    let Some(slot) = restart.0.take() else {
        return;
    };
    let Ok((mut state, mut clock)) = roots.single_mut() else {
        return;
    };
    if state.match_id != slot.previous_id {
        return;
    }
    let retain_builds = *build_policy == RestartBuildPolicy::Retain;
    *build_policy = RestartBuildPolicy::Reconfirm;
    for (entity, mut participant) in &mut participants {
        participant.match_id = slot.next_id;
        participant.ready = retain_builds;
        participant.restart_ready = false;
        commands.entity(entity).insert(MatchMember(slot.next_id));
        complete_fighter_lifecycle(&mut commands, entity);
    }
    for (entity, fighter_id, build, loadout, spawn) in &fighters {
        let Some((maximum_health, ammunition)) = loadout.map_or_else(
            || {
                fighter_runtime_values(
                    *fighter_id,
                    build,
                    &fighter_definitions,
                    &weapon_definitions,
                )
            },
            |loadout| {
                Some((
                    loadout.fighter_stats.maximum_health,
                    loadout.primary_weapon.recipe.economy.capacity(),
                ))
            },
        ) else {
            continue;
        };
        reset_fighter_runtime(
            &mut commands,
            entity,
            FighterReset {
                maximum_health,
                ammunition,
                position: spawn.position,
                facing: spawn.facing,
                collision_mask: avian2d::prelude::LayerMask::NONE,
                protection_until: None,
                active: false,
            },
        );
        commands.entity(entity).insert((
            crate::builds::AbilityState::default(),
            crate::builds::PassiveRuntimeState::default(),
        ));
        if retain_builds {
            commands
                .entity(entity)
                .remove::<crate::builds::SelectingBuild>();
        } else {
            commands
                .entity(entity)
                .insert(crate::builds::SelectingBuild);
        }
    }
    state.match_id = slot.next_id;
    state.phase = MatchPhase::Waiting;
    clock.match_id = slot.next_id;
    clock.completed_tick = slot.restart_tick;
    outcomes.clear();
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
fn cleanup_restarted_match(
    mut commands: Commands,
    roots: Query<&MatchState, With<MatchRoot>>,
    mut epoch: ResMut<RestartCleanupEpoch>,
    mut ordinals: ResMut<RespawnOrdinals>,
    mut prior_positions: ResMut<PriorMatchPositions>,
    mut facts: ResMut<CombatOutcomeFacts>,
    mut trackers: ResMut<ActiveAttackTrackers>,
    mut outbox: ResMut<CombatOutbox>,
    mut pending_payloads: ResMut<Messages<PendingPayload>>,
    mut pending_deliveries: ResMut<Messages<PendingDelivery>>,
    mut melee_attacks: ResMut<Messages<MeleeAttack>>,
    projectiles: Query<(Entity, Option<&MatchMember>), With<crate::combat::Projectile>>,
    mut inputs: Query<(
        Option<&mut NativeBuffer<FighterInput>>,
        Option<&mut ActionState<FighterInput>>,
    )>,
) {
    let Ok(state) = roots.single() else {
        return;
    };
    let Some(previous) = epoch.0 else {
        epoch.0 = Some(state.match_id);
        return;
    };
    if previous == state.match_id || !matches!(state.phase, MatchPhase::Waiting) {
        return;
    }
    epoch.0 = Some(state.match_id);
    ordinals.0.clear();
    prior_positions.0.clear();
    facts.0.clear();
    trackers.active.clear();
    trackers.completed.clear();
    outbox.0.clear();
    pending_payloads.clear();
    pending_deliveries.clear();
    melee_attacks.clear();
    for (entity, member) in &projectiles {
        if member.is_some_and(|member| member.0 == previous) {
            commands.entity(entity).try_despawn();
        }
    }
    for (buffer, actions) in &mut inputs {
        if let Some(mut buffer) = buffer {
            *buffer = NativeBuffer::default();
        }
        if let Some(mut actions) = actions {
            *actions = ActionState::default();
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
fn select_due_respawn_spawns(
    tick: Res<SimulationTick>,
    roots: Query<&MatchState, With<MatchRoot>>,
    spawn_points: Res<crate::map::SpawnPointCatalog>,
    tuning: Res<crate::movement::MovementTuning>,
    mut ordinals: ResMut<RespawnOrdinals>,
    living: Query<(&TeamId, &Position, Option<&Defeated>), With<Fighter>>,
    mut due: Query<(
        &PlayerId,
        &TeamId,
        &MatchParticipant,
        &RespawnState,
        &mut SpawnState,
        &mut crate::map::SpawnAssignment,
    )>,
) {
    let Ok(state) = roots.single() else { return };
    if !matches!(state.phase, MatchPhase::Active { .. }) {
        return;
    }
    for (player_id, team, participant, respawn, mut spawn, mut assignment) in &mut due {
        if participant.match_id != state.match_id || tick.0 < respawn.respawn_at_tick {
            continue;
        }
        let living: Vec<_> = living
            .iter()
            .filter(|(_, _, defeated)| defeated.is_none())
            .map(|(team, position, _)| (*team, position.0))
            .collect();
        let ordinal = ordinals.0.entry(player_id.0).or_default();
        let candidates = spawn_points
            .0
            .get(&team.0)
            .into_iter()
            .flatten()
            .map(|point| SpawnCandidate {
                id: point.spawn_point_id,
                position: point.position,
                facing: point.facing,
            })
            .collect();
        let selected = select_spawn(
            candidates,
            &living,
            *team,
            tuning.radius * 2.0 + tuning.skin_width,
            participant.match_id,
            *player_id,
            *ordinal,
        )
        .unwrap_or(SpawnCandidate {
            id: assignment.spawn_point_id,
            position: spawn.position,
            facing: spawn.facing,
        });
        *ordinal = ordinal.saturating_add(1);
        assignment.spawn_point_id = selected.id;
        spawn.position = selected.position;
        spawn.facing = selected.facing;
    }
}

/// Common preparation inside the mode-rule phase: sort the current-tick fact buffer once
/// without draining it and clear any outcome that survived the pre-game consumer.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn prepare_mode_rule_facts(
    mut facts: ResMut<CombatOutcomeFacts>,
    mut outcomes: ResMut<ModeRuleOutcome>,
    mut diagnostics: ResMut<MatchOutcomeDiagnostics>,
) {
    facts.0.sort_by_key(|fact| fact.event_id.0);
    if outcomes.take().is_some() {
        diagnostics.stale_mode_outcome = diagnostics.stale_mode_outcome.saturating_add(1);
    }
}

/// Post-damage outcome consumption: take the mode's eligible-tick outcome and commit it
/// through the same helper used by pre-game completion.
#[allow(clippy::needless_pass_by_value)]
fn consume_mode_rule_outcome(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    rules: Res<MatchLifecycleRules>,
    mut outcomes: ResMut<ModeRuleOutcome>,
    mut diagnostics: ResMut<MatchOutcomeDiagnostics>,
    mut roots: Query<&mut MatchState, With<MatchRoot>>,
    participants: Query<(Entity, &MatchParticipant), With<Fighter>>,
) {
    let Some(pending) = outcomes.take() else {
        return;
    };
    let Ok(mut state) = roots.single_mut() else {
        diagnostics.stale_mode_outcome = diagnostics.stale_mode_outcome.saturating_add(1);
        return;
    };
    if pending.match_id != state.match_id {
        diagnostics.wrong_match_outcome = diagnostics.wrong_match_outcome.saturating_add(1);
        return;
    }
    if pending.evaluated_tick != tick.0 {
        diagnostics.wrong_tick_outcome = diagnostics.wrong_tick_outcome.saturating_add(1);
        return;
    }
    if !matches!(state.phase, MatchPhase::Active { .. }) {
        diagnostics.stale_mode_outcome = diagnostics.stale_mode_outcome.saturating_add(1);
        return;
    }
    commit_match_result(
        &mut commands,
        &mut state,
        tick.0,
        &rules,
        pending.result,
        participants.iter(),
    );
}

/// Common defeat lifecycle: once deferred `Defeated` markers are visible, schedule respawns
/// for newly defeated fighters of the still-active current match. Mode plugins never do this.
#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
fn handle_defeated_respawns(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    rules: Res<MatchLifecycleRules>,
    roots: Query<&MatchState, With<MatchRoot>>,
    defeated: Query<
        (Entity, &MatchParticipant),
        (
            With<Fighter>,
            With<Defeated>,
            Without<RespawnState>,
            Without<SpawnProtection>,
        ),
    >,
) {
    let Ok(state) = roots.single() else { return };
    if !matches!(state.phase, MatchPhase::Active { .. }) {
        return;
    }
    for (entity, participant) in &defeated {
        if participant.match_id != state.match_id {
            continue;
        }
        commands.entity(entity).insert(RespawnState {
            respawn_at_tick: tick.0.saturating_add(rules.respawn_delay_ticks),
        });
    }
}

/// Common telemetry reads the current-tick fact buffer without draining it.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn record_match_telemetry(
    tick: Res<SimulationTick>,
    rules: Res<MatchLifecycleRules>,
    roots: Query<&MatchState, With<MatchRoot>>,
    facts: Res<CombatOutcomeFacts>,
    mut telemetry: ResMut<MatchTelemetry>,
) {
    let Ok(state) = roots.single() else { return };
    if matches!(state.phase, MatchPhase::Active { .. }) {
        telemetry.begin(state.match_id, tick.0);
    }
    for fact in &facts.0 {
        telemetry.record(*fact, rules.retained_match_records);
    }
}

/// The one ordered clear of the current-tick combat fact buffer, after every registered
/// reader has run, including when no match is active or the root is missing.
pub(crate) fn clear_combat_facts(
    mut facts: ResMut<CombatOutcomeFacts>,
    mut world_effect_facts: ResMut<crate::combat::CombatWorldEffectFacts>,
) {
    facts.0.clear();
    // Terrain drains world-effect facts earlier in the fixed-post chain; this clear is the
    // safety net for compositions without an authoritative terrain plugin.
    world_effect_facts.0.clear();
}

/// Publish the generation-tagged match clock in fixed finalize before the simulation tick
/// advances, so clients can derive deadlines without a client-local tick.
#[allow(clippy::needless_pass_by_value)]
fn publish_match_clock(
    tick: Res<SimulationTick>,
    mut roots: Query<(&MatchState, &mut MatchClock), With<MatchRoot>>,
) {
    let Ok((state, mut clock)) = roots.single_mut() else {
        return;
    };
    if clock.match_id == state.match_id {
        clock.completed_tick = tick.0;
    }
}

#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
fn capture_match_summary(
    tick: Res<SimulationTick>,
    rules: Res<MatchLifecycleRules>,
    roots: Query<
        (
            &MatchState,
            Option<&WipeoutState>,
            Option<&super::HotZoneState>,
        ),
        With<MatchRoot>,
    >,
    hot_zone_telemetry: Option<ResMut<super::hot_zone::HotZoneTelemetry>>,
    mut telemetry: ResMut<MatchTelemetry>,
    weapons: Res<WeaponTelemetry>,
    abilities: Res<crate::abilities::AbilityTelemetry>,
) {
    let Ok((state, wipeout, hot_zone)) = roots.single() else {
        return;
    };
    match state.phase {
        MatchPhase::Active { .. } => telemetry.begin(state.match_id, tick.0),
        MatchPhase::Completed { result, .. } => {
            let mode_summary = match (wipeout, hot_zone) {
                (Some(wipeout), _) => ModeSummary::Wipeout(WipeoutSummary {
                    final_scores: wipeout.team_scores,
                    target_score: wipeout.target_score,
                    score_margin: wipeout.team_scores[0].abs_diff(wipeout.team_scores[1]),
                }),
                (None, Some(hot_zone)) => hot_zone_telemetry
                    .map(|telemetry| ModeSummary::HotZone(telemetry.summary(hot_zone)))
                    .expect("an installed Hot Zone match carries its telemetry"),
                (None, None) => panic!("an installed mode carries its state on the root"),
            };
            telemetry.complete_with_mode(
                tick.0,
                state.mode_definition_id,
                mode_summary,
                result,
                rules.retained_match_summaries,
                &weapons,
                &abilities,
            );
        }
        MatchPhase::Waiting | MatchPhase::Countdown { .. } => {}
    }
}
