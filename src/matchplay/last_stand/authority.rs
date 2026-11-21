//! Last Stand authority: the boundary schedule, outside-zone damage through the combat
//! environment transaction, elimination ranking, death and chest drops, the deadline, and the
//! restart/completion lifecycle. Every mutation is server-owned; clients see only the replicated
//! root state, fighters, and boost pickups.

use super::{
    LAST_STAND_FIGHTERS, LastStandBoundary, LastStandRules, LastStandState, LastStandSummary,
    boundary_at, death_drop_count, elimination_result, rank_eliminations,
};
use crate::{
    combat::{CombatOutcomeFacts, CombatOutcomeKind, CombatTargetKind, Defeated, TeamId},
    map::{MapCatalogResource, MapDynamicState, MapRoot, ResolvedMap},
    matchplay::{
        ActiveCombatant, BoostGrantScope, BoostPickup, BoostPickupSource, BoostStacks,
        EliminationLifecycle, MAX_LIVE_BOOST_PICKUPS, MAX_MATCH_TEAMS, MatchId,
        MatchOutcomeDiagnostics, MatchParticipant, MatchPhase, MatchRestartSet, MatchRoot,
        MatchSet, MatchState, ModeOutcomeCause, ModeRuleOutcome, PendingMatchRestart,
        PendingModeRuleOutcome,
        boost_pickups::{
            BoostPickupRules, NextBoostPickupId, ProcessedChests, collect_boost_pickups,
            spawn_boost_pickup,
        },
        offer_mode_rule_outcome, prepare_mode_rule_facts,
    },
    protocol::{Fighter, NetworkEntityId},
    timing::SimulationTick,
};
use avian2d::prelude::Position;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

/// Body radius a dropped pickup must clear blocking geometry by.
const DROP_BODY_RADIUS: f32 = 10.0;

/// The round's start, length, and the fighters still standing, keyed by network ID.
#[derive(Resource, Default, Debug)]
pub(super) struct LastStandRoster {
    match_id: Option<MatchId>,
    started_at_tick: u64,
    active_ticks: u64,
    alive: BTreeMap<u64, TeamId>,
}

/// Live Last Stand telemetry accumulator, reset when a new round begins.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LastStandTelemetry {
    match_id: Option<MatchId>,
    boundary_defeats: u16,
    chests_opened: u16,
    drops_spawned: u16,
    drops_skipped: u16,
}

impl LastStandTelemetry {
    fn begin_match(&mut self, match_id: MatchId) {
        if self.match_id != Some(match_id) {
            *self = Self {
                match_id: Some(match_id),
                ..Self::default()
            };
        }
    }

    pub(crate) fn summary(&self, state: &LastStandState) -> LastStandSummary {
        LastStandSummary {
            fighters: state.fighters,
            survivors: state.alive,
            boundary_phase_reached: state.boundary.phase,
            boundary_defeats: self.boundary_defeats,
            chests_opened: self.chests_opened,
            drops_spawned: self.drops_spawned,
            drops_skipped: self.drops_skipped,
        }
    }
}

pub struct LastStandModePlugin;

impl Plugin for LastStandModePlugin {
    fn build(&self, app: &mut App) {
        let setup = app
            .world()
            .get_resource::<crate::matchplay::MatchModeSetup>()
            .copied()
            .unwrap_or_default();
        assert_eq!(
            setup.mode_definition_id,
            crate::map::LAST_STAND_MODE_DEFINITION,
            "LastStandModePlugin requires a Last Stand match mode setup"
        );
        let rules = app
            .world()
            .get_resource::<LastStandRules>()
            .copied()
            .unwrap_or_default()
            .validate()
            .expect("Last Stand rules must be valid");
        app.insert_resource(rules)
            .insert_resource(EliminationLifecycle)
            .insert_resource(BoostPickupRules {
                pickup_radius: rules.pickup_radius,
                stack_limit: rules.boost_stack_limit,
            })
            .init_resource::<NextBoostPickupId>()
            .init_resource::<ProcessedChests>()
            .init_resource::<LastStandRoster>()
            .init_resource::<LastStandTelemetry>()
            .add_systems(
                Startup,
                initialize_last_stand_state
                    .after(crate::matchplay::server::initialize_match_root)
                    .after(crate::map::MapStartupSet::Instantiate),
            )
            .add_systems(
                FixedUpdate,
                resolve_last_stand_deadline.in_set(MatchSet::DeadlineRules),
            )
            .add_systems(
                FixedUpdate,
                reset_last_stand_on_restart.in_set(MatchRestartSet::ModeReset),
            )
            .add_systems(
                FixedUpdate,
                (begin_round_and_advance_boundary, collect_boost_pickups)
                    .chain()
                    .after(MatchSet::FighterLifecycle),
            )
            .add_systems(
                FixedUpdate,
                cleanup_completed_pickups.after(MatchSet::PreGameOutcomes),
            )
            .add_systems(
                FixedPostUpdate,
                apply_boundary_damage
                    .in_set(crate::combat::CombatDamageSet::EnvironmentReactions)
                    .after(crate::map::runtime::effect_tiles::apply_damage_tile_pulses),
            )
            .add_systems(
                FixedPostUpdate,
                (spawn_chest_pickups, resolve_eliminations)
                    .chain()
                    .in_set(MatchSet::ModeRules)
                    .after(prepare_mode_rule_facts),
            );
    }
}

/// The ring starts at the map's half-diagonal around its centre, so every spawn begins safe.
fn opening_ring(resolved: &ResolvedMap) -> (Vec2, f32) {
    let bounds = resolved.snapshot.dimensions.bounds();
    (bounds.center(), (bounds.max - bounds.min).length() * 0.5)
}

fn initial_state(
    match_id: MatchId,
    rules: &LastStandRules,
    resolved: &ResolvedMap,
    active_ticks: u64,
) -> LastStandState {
    let (center, radius) = opening_ring(resolved);
    LastStandState {
        match_id,
        fighters: LAST_STAND_FIGHTERS,
        alive: LAST_STAND_FIGHTERS,
        boundary: boundary_at(rules, 0, active_ticks, center, radius, 0),
        placements: [0; MAX_MATCH_TEAMS],
    }
}

fn bot_view(boundary: &LastStandBoundary, tick: u64) -> crate::matchplay::BotObjectiveView {
    crate::matchplay::BotObjectiveView::SurviveBoundary {
        center: boundary.center,
        radius: boundary.radius_at(tick),
        next_radius: boundary.radius_to,
    }
}

#[allow(clippy::needless_pass_by_value)]
fn initialize_last_stand_state(
    mut commands: Commands,
    rules: Res<LastStandRules>,
    lifecycle: Res<crate::matchplay::MatchLifecycleRules>,
    resolved: Res<ResolvedMap>,
    roots: Query<(Entity, &MatchState), With<MatchRoot>>,
) {
    let Ok((root, state)) = roots.single() else {
        return;
    };
    assert_eq!(
        state.mode_definition_id,
        crate::map::LAST_STAND_MODE_DEFINITION,
        "Last Stand mode requires a Last Stand map"
    );
    let initial = initial_state(
        state.match_id,
        &rules,
        &resolved,
        lifecycle.active_limit_ticks,
    );
    commands
        .entity(root)
        .insert((initial, bot_view(&initial.boundary, 0)));
}

/// On the first active tick, record the starting fighters and the round's clock; on every
/// active tick, publish the boundary only when its phase changes.
#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
fn begin_round_and_advance_boundary(
    tick: Res<SimulationTick>,
    rules: Res<LastStandRules>,
    resolved: Res<ResolvedMap>,
    mut roster: ResMut<LastStandRoster>,
    mut telemetry: ResMut<LastStandTelemetry>,
    mut roots: Query<
        (
            &MatchState,
            &mut LastStandState,
            &mut crate::matchplay::BotObjectiveView,
        ),
        With<MatchRoot>,
    >,
    fighters: Query<(&NetworkEntityId, &TeamId, &MatchParticipant), With<Fighter>>,
) {
    let Ok((state, mut mode, mut view)) = roots.single_mut() else {
        return;
    };
    let MatchPhase::Active { ends_at_tick } = state.phase else {
        return;
    };
    if roster.match_id != Some(state.match_id) {
        let alive: BTreeMap<_, _> = fighters
            .iter()
            .filter(|(_, team, participant)| {
                participant.match_id == state.match_id && usize::from(team.0) < MAX_MATCH_TEAMS
            })
            .map(|(network_id, team, _)| (network_id.0, *team))
            .collect();
        *roster = LastStandRoster {
            match_id: Some(state.match_id),
            started_at_tick: tick.0,
            active_ticks: ends_at_tick.saturating_sub(tick.0),
            alive,
        };
        telemetry.begin_match(state.match_id);
        let fighters = u8::try_from(roster.alive.len()).unwrap_or(u8::MAX);
        *mode = LastStandState {
            match_id: state.match_id,
            fighters,
            alive: fighters,
            placements: [0; MAX_MATCH_TEAMS],
            ..*mode
        };
    }
    let (center, radius) = opening_ring(&resolved);
    let boundary = boundary_at(
        &rules,
        roster.started_at_tick,
        roster.active_ticks,
        center,
        radius,
        tick.0,
    );
    if mode.boundary != boundary {
        mode.boundary = boundary;
    }
    let next_view = bot_view(&boundary, tick.0);
    if *view != next_view {
        *view = next_view;
    }
}

/// Every pulse interval, damage each living fighter outside the safe radius through the combat
/// environment transaction, which alone commits health, defeats, facts, and cues.
fn apply_boundary_damage(world: &mut World) {
    let tick = world.resource::<SimulationTick>().0;
    let interval = u64::from(world.resource::<LastStandRules>().pulse_interval_ticks);
    let Some((match_id, boundary)) = world
        .query_filtered::<(&MatchState, &LastStandState), With<MatchRoot>>()
        .iter(world)
        .next()
        .filter(|(state, mode)| {
            matches!(state.phase, MatchPhase::Active { .. }) && mode.match_id == state.match_id
        })
        .map(|(state, mode)| (state.match_id, mode.boundary))
    else {
        return;
    };
    let roster = world.resource::<LastStandRoster>();
    if roster.match_id != Some(match_id)
        || !tick
            .saturating_sub(roster.started_at_tick)
            .is_multiple_of(interval)
    {
        return;
    }
    let Some(generation) = world
        .query_filtered::<&MapDynamicState, With<MapRoot>>()
        .iter(world)
        .next()
        .map(MapDynamicState::generation_id)
    else {
        return;
    };
    let mut outside = world
        .query_filtered::<(Entity, &NetworkEntityId, &Position, &MatchParticipant), (
            With<Fighter>,
            With<ActiveCombatant>,
            Without<Defeated>,
        )>()
        .iter(world)
        .filter(|(_, _, position, participant)| {
            participant.match_id == match_id && !boundary.contains(position.0, tick)
        })
        .map(|(entity, network_id, _, _)| (network_id.0, entity))
        .collect::<Vec<_>>();
    if outside.is_empty() {
        return;
    }
    outside.sort_by_key(|(network_id, _)| *network_id);
    let targets: Vec<_> = outside.iter().map(|(_, entity)| *entity).collect();
    match crate::combat::environment::apply_environment_damage_batch(
        world,
        crate::combat::environment::EnvironmentDamageBatch {
            targets: &targets,
            generation,
            placement_id: crate::map::MapPlacementId(0),
            damage: boundary.damage_per_pulse,
            tick,
            origin: Some(boundary.center),
            attack: crate::combat::environment::EnvironmentAttack::Neutral,
            protection: crate::combat::environment::EnvironmentProtection::IgnoreSpawnProtection,
        },
    ) {
        Ok(_) => {
            let defeated = targets
                .iter()
                .filter(|entity| world.get::<Defeated>(**entity).is_some())
                .count();
            let mut telemetry = world.resource_mut::<LastStandTelemetry>();
            telemetry.boundary_defeats = telemetry
                .boundary_defeats
                .saturating_add(u16::try_from(defeated).unwrap_or(u16::MAX));
        }
        Err(error) => error!(?error, "Last Stand boundary combat transaction failed"),
    }
}

/// Map geometry and live pickups used to place boost drops on reachable floor.
#[derive(SystemParam)]
struct DropPlacement<'w, 's> {
    resolved: Res<'w, ResolvedMap>,
    states: Query<'w, 's, &'static MapDynamicState, With<MapRoot>>,
    catalog: Res<'w, MapCatalogResource>,
    pickups: Query<'w, 's, &'static Position, With<BoostPickup>>,
}

impl DropPlacement<'_, '_> {
    /// A cell is free when a pickup body at its centre needs no push out of blocking geometry.
    fn cell_is_free(&self, center: Vec2) -> bool {
        self.states.single().is_ok_and(|state| {
            crate::map::resolve_circle_against_blocking_map(
                center,
                DROP_BODY_RADIUS,
                &self.resolved.snapshot,
                state,
                &self.catalog.0,
            )
            .distance_squared(center)
                < 0.01
        })
    }

    /// Distinct free cells for `count` drops: at or next to `position` when it is inside the
    /// ring, otherwise the nearest cells inside the ring. Fewer cells means skipped drops.
    fn drop_positions(
        &self,
        position: Vec2,
        boundary: &LastStandBoundary,
        tick: u64,
        count: u8,
        taken: &mut Vec<Vec2>,
    ) -> Vec<Vec2> {
        let dimensions = self.resolved.snapshot.dimensions;
        let inside = boundary.contains(position, tick);
        let target = if inside {
            position
        } else {
            let radius = (boundary.radius_at(tick) - crate::map::MAP_CELL_SIZE_WORLD).max(0.0);
            boundary.center + (position - boundary.center).normalize_or_zero() * radius
        };
        let reach = if inside {
            1
        } else {
            i32::from(dimensions.width.max(dimensions.height))
        };
        let Some(origin) = dimensions.world_cell(target.clamp(
            dimensions.bounds().min,
            dimensions.bounds().max - Vec2::splat(0.01),
        )) else {
            return Vec::new();
        };
        let mut candidates = Vec::new();
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                let (Ok(x), Ok(y)) = (
                    u16::try_from(i32::from(origin.x) + dx),
                    u16::try_from(i32::from(origin.y) + dy),
                ) else {
                    continue;
                };
                let cell = crate::map::MapCell::new(x, y);
                if !dimensions.contains(cell) {
                    continue;
                }
                let center = dimensions.cell_center(cell);
                candidates.push((center.distance_squared(target), y, x, center));
            }
        }
        candidates.sort_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then(left.1.cmp(&right.1))
                .then(left.2.cmp(&right.2))
        });
        let minimum_gap = crate::map::MAP_CELL_SIZE_WORLD * 0.5;
        let mut chosen = Vec::new();
        for (_, _, _, center) in candidates {
            if chosen.len() == usize::from(count) {
                break;
            }
            if boundary.contains(center, tick)
                && taken
                    .iter()
                    .all(|other| other.distance(center) >= minimum_gap)
                && self.cell_is_free(center)
            {
                taken.push(center);
                chosen.push(center);
            }
        }
        chosen
    }
}

/// Destroyed treasure chests each drop one public individual boost (shared Zombie path).
#[allow(clippy::needless_pass_by_value)]
fn spawn_chest_pickups(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    mut ids: ResMut<NextBoostPickupId>,
    mut processed: ResMut<ProcessedChests>,
    mut telemetry: ResMut<LastStandTelemetry>,
    roots: Query<&MatchState, With<MatchRoot>>,
    placement: DropPlacement,
) {
    let Ok(state) = roots.single() else { return };
    if !matches!(state.phase, MatchPhase::Active { .. }) {
        return;
    }
    let mut live = placement.pickups.iter().count();
    for (_, position) in processed.newly_destroyed(
        state.match_id,
        &placement.resolved,
        placement.states.single().ok(),
    ) {
        if live >= MAX_LIVE_BOOST_PICKUPS {
            telemetry.drops_skipped = telemetry.drops_skipped.saturating_add(1);
            continue;
        }
        if spawn_boost_pickup(
            &mut commands,
            &mut ids,
            state.match_id,
            BoostPickupSource::Chest,
            BoostGrantScope::Individual,
            position,
            tick.0,
        ) {
            live += 1;
            telemetry.chests_opened = telemetry.chests_opened.saturating_add(1);
        }
    }
}

/// One tick's eliminations: who left the standing set and which of them were defeated here.
struct Eliminations {
    teams: Vec<TeamId>,
    defeated: BTreeSet<u64>,
}

fn collect_eliminations(
    roster: &LastStandRoster,
    facts: &CombatOutcomeFacts,
    standing: &BTreeSet<u64>,
) -> Eliminations {
    let defeated: BTreeSet<u64> = facts
        .0
        .iter()
        .filter(|fact| {
            matches!(fact.kind, CombatOutcomeKind::Defeat)
                && matches!(fact.target_kind, CombatTargetKind::Fighter)
        })
        .map(|fact| fact.target_network_id.0)
        .filter(|network_id| roster.alive.contains_key(network_id))
        .collect();
    let mut teams: Vec<TeamId> = roster
        .alive
        .iter()
        .filter(|(network_id, _)| defeated.contains(network_id) || !standing.contains(network_id))
        .map(|(_, team)| *team)
        .collect();
    teams.sort_unstable_by_key(|team| team.0);
    Eliminations { teams, defeated }
}

#[derive(SystemParam)]
struct EliminationOutcome<'w> {
    outcomes: ResMut<'w, ModeRuleOutcome>,
    diagnostics: ResMut<'w, MatchOutcomeDiagnostics>,
}

/// Rank this tick's eliminations, drop each defeated fighter's boosts, and offer the round
/// outcome once at most one fighter stands. Defeat facts make same-tick defeats visible before
/// their deferred `Defeated` markers; a fighter whose entity left the match is eliminated too.
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
fn resolve_eliminations(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    facts: Res<CombatOutcomeFacts>,
    mut roster: ResMut<LastStandRoster>,
    mut ids: ResMut<NextBoostPickupId>,
    mut telemetry: ResMut<LastStandTelemetry>,
    mut outcome: EliminationOutcome,
    mut roots: Query<(&MatchState, &mut LastStandState), With<MatchRoot>>,
    mut fighters: Query<
        (
            &NetworkEntityId,
            &Position,
            &MatchParticipant,
            Option<&mut BoostStacks>,
            Has<Defeated>,
            Has<ActiveCombatant>,
        ),
        With<Fighter>,
    >,
    placement: DropPlacement,
) {
    let Ok((state, mut mode)) = roots.single_mut() else {
        return;
    };
    if !matches!(state.phase, MatchPhase::Active { .. })
        || roster.match_id != Some(state.match_id)
        || mode.match_id != state.match_id
    {
        return;
    }
    let standing: BTreeSet<u64> = fighters
        .iter()
        .filter(|(_, _, participant, _, defeated, active)| {
            participant.match_id == state.match_id && *active && !*defeated
        })
        .map(|(network_id, ..)| network_id.0)
        .collect();
    let eliminations = collect_eliminations(&roster, &facts, &standing);
    if eliminations.teams.is_empty() {
        return;
    }
    let mut live = placement.pickups.iter().count();
    let mut taken = placement
        .pickups
        .iter()
        .map(|position| position.0)
        .collect::<Vec<_>>();
    for (network_id, position, participant, mut stacks, defeated, _) in &mut fighters {
        let eliminated_by_defeat = eliminations.defeated.contains(&network_id.0)
            || (defeated && roster.alive.contains_key(&network_id.0));
        if participant.match_id != state.match_id || !eliminated_by_defeat {
            continue;
        }
        let carried = stacks
            .as_mut()
            .map_or(0, |stacks| std::mem::take(&mut stacks.0));
        let count = death_drop_count(carried);
        let positions =
            placement.drop_positions(position.0, &mode.boundary, tick.0, count, &mut taken);
        let skipped = usize::from(count).saturating_sub(positions.len());
        for drop in positions {
            if live < MAX_LIVE_BOOST_PICKUPS
                && spawn_boost_pickup(
                    &mut commands,
                    &mut ids,
                    state.match_id,
                    BoostPickupSource::PlayerDeath,
                    BoostGrantScope::Individual,
                    drop,
                    tick.0,
                )
            {
                live += 1;
                telemetry.drops_spawned = telemetry.drops_spawned.saturating_add(1);
            } else {
                telemetry.drops_skipped = telemetry.drops_skipped.saturating_add(1);
            }
        }
        if skipped > 0 {
            telemetry.drops_skipped = telemetry
                .drops_skipped
                .saturating_add(u16::try_from(skipped).unwrap_or(u16::MAX));
            debug!(
                network_id = network_id.0,
                skipped, "Last Stand death drop found no free cell"
            );
        }
    }
    let mut placements = mode.placements;
    let alive = rank_eliminations(&mut placements, mode.alive, &eliminations.teams);
    roster
        .alive
        .retain(|_, team| !eliminations.teams.contains(team));
    let survivors: Vec<TeamId> = roster.alive.values().copied().collect();
    if let Some(result) = elimination_result(&survivors) {
        for team in &survivors {
            placements[usize::from(team.0)] = 1;
        }
        offer_mode_rule_outcome(
            &mut outcome.outcomes,
            &mut outcome.diagnostics,
            PendingModeRuleOutcome {
                match_id: state.match_id,
                evaluated_tick: tick.0,
                cause: ModeOutcomeCause::Threshold,
                result,
            },
        );
    }
    mode.placements = placements;
    mode.alive = alive;
}

/// At the deadline every fighter still standing shares first place and the round draws.
#[allow(clippy::needless_pass_by_value)]
fn resolve_last_stand_deadline(
    tick: Res<SimulationTick>,
    roster: Res<LastStandRoster>,
    mut roots: Query<(&MatchState, &mut LastStandState), With<MatchRoot>>,
    mut outcomes: ResMut<ModeRuleOutcome>,
    mut diagnostics: ResMut<MatchOutcomeDiagnostics>,
) {
    let Ok((state, mut mode)) = roots.single_mut() else {
        return;
    };
    let MatchPhase::Active { ends_at_tick } = state.phase else {
        return;
    };
    if tick.0 < ends_at_tick || state.match_id != mode.match_id {
        return;
    }
    let survivors: Vec<TeamId> = roster.alive.values().copied().collect();
    for team in &survivors {
        mode.placements[usize::from(team.0)] = 1;
    }
    let result = elimination_result(&survivors).unwrap_or(crate::matchplay::MatchResult::Draw);
    offer_mode_rule_outcome(
        &mut outcomes,
        &mut diagnostics,
        PendingModeRuleOutcome {
            match_id: state.match_id,
            evaluated_tick: tick.0,
            cause: ModeOutcomeCause::Timeout,
            result,
        },
    );
}

#[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
fn reset_last_stand_on_restart(
    mut commands: Commands,
    pending: Res<PendingMatchRestart>,
    rules: Res<LastStandRules>,
    lifecycle: Res<crate::matchplay::MatchLifecycleRules>,
    resolved: Res<ResolvedMap>,
    mut ids: ResMut<NextBoostPickupId>,
    mut roster: ResMut<LastStandRoster>,
    mut roots: Query<&mut LastStandState, With<MatchRoot>>,
    pickups: Query<Entity, With<BoostPickup>>,
    mut fighters: Query<&mut BoostStacks, With<Fighter>>,
) {
    let Some(slot) = pending.slot() else { return };
    for entity in &pickups {
        commands.entity(entity).despawn();
    }
    for mut stacks in &mut fighters {
        stacks.0 = 0;
    }
    ids.clear();
    *roster = LastStandRoster::default();
    if let Ok(mut state) = roots.single_mut() {
        *state = initial_state(
            slot.next_id,
            &rules,
            &resolved,
            lifecycle.active_limit_ticks,
        );
    }
}

/// Completion freezes the root state for Results and removes the ground pickups.
fn cleanup_completed_pickups(
    mut commands: Commands,
    roots: Query<&MatchState, With<MatchRoot>>,
    pickups: Query<Entity, With<BoostPickup>>,
) {
    let Ok(state) = roots.single() else { return };
    if !matches!(state.phase, MatchPhase::Completed { .. }) {
        return;
    }
    for entity in &pickups {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests;
