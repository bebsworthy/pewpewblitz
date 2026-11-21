//! Shard Rush authority: source spawns, collection, carrier drops, the hold countdown, and
//! the restart/completion lifecycle. Every mutation is server-owned; clients only see the
//! replicated shard entities, carrier counts, and root state.

use super::{
    SHARD_BODY_RADIUS, SHARD_NETWORK_NAMESPACE, SHARD_PICKUP_RADIUS, Shard, ShardCarrier,
    ShardIdentity, ShardOrigin, ShardRushRules, ShardRushState, ShardRushSummary, leading_team,
    next_countdown, shard_fan_offset, timeout_result,
};
use crate::{
    combat::{
        CombatOutcomeFacts, CombatOutcomeKind, CombatTargetKind, CurrentHealth, Defeated, TeamId,
    },
    map::{MapCatalogResource, MapDynamicState, MapRoot, ModeAnchorId, ResolvedMap},
    matchplay::{
        ActiveCombatant, ConnectedMatchRoster, MatchId, MatchOutcomeDiagnostics, MatchParticipant,
        MatchPhase, MatchRestartSet, MatchRoot, MatchSet, MatchState, ModeOutcomeCause,
        ModeRuleOutcome, PendingMatchRestart, PendingModeRuleOutcome, offer_mode_rule_outcome,
        prepare_mode_rule_facts,
    },
    protocol::{Fighter, NetworkEntityId},
    server::SeatHold,
    timing::SimulationTick,
};
use avian2d::prelude::Position;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use lightyear::prelude::{NetworkTarget, Replicate};
use std::collections::BTreeMap;

/// Server-cached source anchor resolved from the installed map.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ShardRushSource {
    pub anchor_id: ModeAnchorId,
    pub position: Vec2,
}

#[derive(Resource, Default)]
struct NextShardId(u32);

/// One carrier that left the world (seat removal or hold expiry) while holding shards; the drop
/// transaction scatters them at its last position.
#[derive(Clone, Copy, Debug, PartialEq)]
struct DepartedCarrier {
    match_id: MatchId,
    network_id: u64,
    position: Vec2,
    carried: u8,
}

#[derive(Resource, Default)]
struct DepartedCarriers(Vec<DepartedCarrier>);

/// A fighter entity removed while carrying shards would otherwise take them out of play.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy removal observers receive On as an owned system parameter"
)]
fn record_departed_carrier(
    trigger: On<Remove, ShardCarrier>,
    carriers: Query<(
        &ShardCarrier,
        &Position,
        &MatchParticipant,
        &NetworkEntityId,
    )>,
    mut departed: ResMut<DepartedCarriers>,
) {
    let Ok((carrier, position, participant, network_id)) = carriers.get(trigger.entity) else {
        return;
    };
    if carrier.0 > 0 {
        departed.0.push(DepartedCarrier {
            match_id: participant.match_id,
            network_id: network_id.0,
            position: position.0,
            carried: carrier.0,
        });
    }
}

impl NextShardId {
    fn next(&mut self) -> Option<u32> {
        self.0 = self.0.checked_add(1)?;
        Some(self.0)
    }
}

/// Live Shard Rush telemetry accumulator, reset by the mode restart reset.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ShardRushTelemetry {
    match_id: Option<MatchId>,
    shards_spawned: u16,
    shards_collected: u16,
    shards_dropped: u16,
    countdown_starts_by_team: [u16; 2],
    countdown_resets: u16,
}

impl ShardRushTelemetry {
    fn begin_match(&mut self, match_id: MatchId) {
        if self.match_id != Some(match_id) {
            *self = Self {
                match_id: Some(match_id),
                ..Self::default()
            };
        }
    }

    pub(crate) fn summary(&self, state: &ShardRushState) -> ShardRushSummary {
        ShardRushSummary {
            final_team_shards: state.team_shards,
            target_shards: state.target_shards,
            shards_spawned: self.shards_spawned,
            shards_collected: self.shards_collected,
            shards_dropped: self.shards_dropped,
            countdown_starts_by_team: self.countdown_starts_by_team,
            countdown_resets: self.countdown_resets,
        }
    }
}

pub struct ShardRushModePlugin;

impl Plugin for ShardRushModePlugin {
    fn build(&self, app: &mut App) {
        let setup = app
            .world()
            .get_resource::<crate::matchplay::MatchModeSetup>()
            .copied()
            .unwrap_or_default();
        assert_eq!(
            setup.mode_definition_id,
            crate::map::SHARD_RUSH_MODE_DEFINITION,
            "ShardRushModePlugin requires a Shard Rush match mode setup"
        );
        let rules = app
            .world()
            .get_resource::<ShardRushRules>()
            .copied()
            .expect("Shard Rush installs its validated rules before the plugin")
            .validate()
            .expect("Shard Rush rules must be valid");
        app.insert_resource(rules)
            .init_resource::<NextShardId>()
            .init_resource::<ShardRushTelemetry>()
            .init_resource::<DepartedCarriers>()
            .add_observer(record_departed_carrier)
            .add_systems(
                Startup,
                initialize_shard_rush_state
                    .after(crate::matchplay::server::initialize_match_root)
                    .after(crate::map::MapStartupSet::Instantiate),
            )
            .add_systems(
                FixedUpdate,
                resolve_shard_rush_deadline.in_set(MatchSet::DeadlineRules),
            )
            .add_systems(
                FixedUpdate,
                reset_shard_rush_on_restart.in_set(MatchRestartSet::ModeReset),
            )
            .add_systems(
                FixedUpdate,
                (spawn_source_shards, collect_shards)
                    .chain()
                    .after(MatchSet::FighterLifecycle),
            )
            .add_systems(
                FixedUpdate,
                cleanup_completed_shards.after(MatchSet::PreGameOutcomes),
            )
            .add_systems(
                FixedPostUpdate,
                (drop_carried_shards, evaluate_shard_rush_hold)
                    .chain()
                    .in_set(MatchSet::ModeRules)
                    .after(prepare_mode_rule_facts),
            );
    }
}

#[allow(clippy::needless_pass_by_value)]
fn initialize_shard_rush_state(
    mut commands: Commands,
    rules: Res<ShardRushRules>,
    resolved: Res<ResolvedMap>,
    roots: Query<(Entity, &MatchState), With<MatchRoot>>,
) {
    let Ok((root, state)) = roots.single() else {
        return;
    };
    assert_eq!(
        state.mode_definition_id,
        crate::map::SHARD_RUSH_MODE_DEFINITION,
        "Shard Rush mode requires a Shard Rush map"
    );
    let source = resolved
        .shard_source
        .expect("validated Shard Rush maps resolve exactly one source");
    commands.entity(root).insert((
        initial_state(state.match_id, source.anchor_id, *rules),
        crate::matchplay::BotObjectiveView::CollectAndHold {
            source: source.position,
        },
    ));
    commands.insert_resource(ShardRushSource {
        anchor_id: source.anchor_id,
        position: source.position,
    });
}

fn initial_state(
    match_id: MatchId,
    anchor_id: ModeAnchorId,
    rules: ShardRushRules,
) -> ShardRushState {
    ShardRushState {
        match_id,
        source_anchor_id: anchor_id,
        team_shards: [0, 0],
        target_shards: rules.target_shards,
        hold_ticks: rules.policy.hold_ticks,
        countdown: None,
        ground_shards: 0,
        next_spawn_at_tick: ShardRushState::UNSCHEDULED_SPAWN_TICK,
    }
}

/// Map geometry used to keep spawned and dropped shards on reachable floor.
#[derive(SystemParam)]
struct ShardPlacement<'w, 's> {
    resolved: Res<'w, ResolvedMap>,
    states: Query<'w, 's, &'static MapDynamicState, With<MapRoot>>,
    catalog: Res<'w, MapCatalogResource>,
}

impl ShardPlacement<'_, '_> {
    /// Resolve a candidate against bounds and blocking geometry; fall back to the anchor point
    /// (a source centre or a fighter's own position, both proven clear) when it stays blocked.
    fn resolve(&self, candidate: Vec2, fallback: Vec2) -> Vec2 {
        let Ok(map_state) = self.states.single() else {
            return fallback;
        };
        let snapshot = &self.resolved.snapshot;
        let resolved = crate::map::resolve_circle_against_blocking_map(
            candidate,
            SHARD_BODY_RADIUS,
            snapshot,
            map_state,
            &self.catalog.0,
        );
        let inside = snapshot
            .dimensions
            .bounds()
            .contains_with_inset(resolved, SHARD_BODY_RADIUS);
        if resolved.is_finite()
            && inside
            && !crate::map::circle_overlaps_blocking_map(
                resolved,
                SHARD_BODY_RADIUS,
                snapshot,
                map_state,
                &self.catalog.0,
            )
        {
            resolved
        } else {
            fallback
        }
    }
}

/// The shard entity ledger: stable shard IDs, spawned entities, and their telemetry counts.
/// This tick's reasons a carrier loses its shards: defeat facts, roster departures, and carriers
/// whose entity was removed since the last drop transaction.
#[derive(SystemParam)]
struct DropSignals<'w> {
    tick: Res<'w, SimulationTick>,
    roster: Res<'w, ConnectedMatchRoster>,
    facts: Res<'w, CombatOutcomeFacts>,
    departed: ResMut<'w, DepartedCarriers>,
}

#[derive(SystemParam)]
struct ShardLedger<'w, 's> {
    commands: Commands<'w, 's>,
    ids: ResMut<'w, NextShardId>,
    telemetry: ResMut<'w, ShardRushTelemetry>,
}

impl ShardLedger<'_, '_> {
    /// Spawn one replicated shard; false once the match exhausted its shard ID space.
    fn spawn(&mut self, match_id: MatchId, origin: ShardOrigin, position: Vec2) -> bool {
        let Some(shard_id) = self.ids.next() else {
            return false;
        };
        self.commands.spawn((
            Shard,
            ShardIdentity { match_id, shard_id },
            origin,
            NetworkEntityId(SHARD_NETWORK_NAMESPACE | u64::from(shard_id)),
            Position::from_xy(position.x, position.y),
            Transform::from_translation(position.extend(0.0)),
            Replicate::to_clients(NetworkTarget::All),
        ));
        let count = match origin {
            ShardOrigin::Source => &mut self.telemetry.shards_spawned,
            ShardOrigin::Dropped => &mut self.telemetry.shards_dropped,
        };
        *count = count.saturating_add(1);
        true
    }
}

/// Source cadence: the first shard on the first active tick, then one per interval while the
/// uncarried ground count is below the authored ceiling. A full ground skips that deadline.
#[allow(clippy::needless_pass_by_value)]
fn spawn_source_shards(
    tick: Res<SimulationTick>,
    rules: Res<ShardRushRules>,
    source: Option<Res<ShardRushSource>>,
    mut ledger: ShardLedger,
    mut roots: Query<(&MatchState, &mut ShardRushState), With<MatchRoot>>,
    shards: Query<&ShardIdentity, With<Shard>>,
    placement: ShardPlacement,
) {
    let Some(source) = source else { return };
    let Ok((state, mut shard_rush)) = roots.single_mut() else {
        return;
    };
    if !matches!(state.phase, MatchPhase::Active { .. }) || state.match_id != shard_rush.match_id {
        return;
    }
    ledger.telemetry.begin_match(state.match_id);
    let ground = shards
        .iter()
        .filter(|identity| identity.match_id == state.match_id)
        .count();
    let mut ground = u8::try_from(ground).unwrap_or(u8::MAX);
    if shard_rush.next_spawn_at_tick == ShardRushState::UNSCHEDULED_SPAWN_TICK
        || tick.0 >= shard_rush.next_spawn_at_tick
    {
        if ground < rules.policy.live_shard_ceiling {
            // The opening shard sits on the source itself, equidistant from mirrored spawns;
            // later shards fan out around it.
            let offset = match ledger.telemetry.shards_spawned {
                0 => Vec2::ZERO,
                spawned => shard_fan_offset(spawned, 20.0, 6.0),
            };
            let position = placement.resolve(source.position + offset, source.position);
            if ledger.spawn(state.match_id, ShardOrigin::Source, position) {
                ground = ground.saturating_add(1);
            }
        }
        shard_rush.next_spawn_at_tick = tick
            .0
            .saturating_add(u64::from(rules.policy.spawn_interval_ticks));
    }
    shard_rush.ground_shards = ground;
}

fn eligible_carrier(
    state: &MatchState,
    roster: &ConnectedMatchRoster,
    network_id: NetworkEntityId,
    participant: &MatchParticipant,
    team: TeamId,
    health: CurrentHealth,
    position: Vec2,
) -> bool {
    participant.match_id == state.match_id
        && team.0 <= 1
        && health.0 > 0
        && position.is_finite()
        && roster.match_id == Some(state.match_id)
        && roster.connected_network_ids.contains(&network_id.0)
}

/// Server-owned overlap collection: ascending shard ID, each to the nearest-ID eligible fighter
/// in range that still has carry room.
#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
fn collect_shards(
    mut commands: Commands,
    rules: Res<ShardRushRules>,
    roster: Res<ConnectedMatchRoster>,
    mut telemetry: ResMut<ShardRushTelemetry>,
    mut roots: Query<(&MatchState, &mut ShardRushState), With<MatchRoot>>,
    shards: Query<(Entity, &ShardIdentity, &Position), With<Shard>>,
    mut fighters: Query<
        (
            &NetworkEntityId,
            &MatchParticipant,
            &TeamId,
            &CurrentHealth,
            &Position,
            &mut ShardCarrier,
        ),
        (
            With<Fighter>,
            With<ActiveCombatant>,
            Without<Defeated>,
            Without<SeatHold>,
        ),
    >,
) {
    let Ok((state, mut shard_rush)) = roots.single_mut() else {
        return;
    };
    if !matches!(state.phase, MatchPhase::Active { .. }) || state.match_id != shard_rush.match_id {
        return;
    }
    let maximum = rules.maximum_carried();
    let mut carriers = fighters
        .iter_mut()
        .filter(|(network_id, participant, team, health, position, _)| {
            eligible_carrier(
                state,
                &roster,
                **network_id,
                participant,
                **team,
                **health,
                position.0,
            )
        })
        .map(|(network_id, _, _, _, position, carrier)| (network_id.0, (position.0, carrier)))
        .collect::<BTreeMap<_, _>>();
    let mut ordered = shards
        .iter()
        .filter(|(_, identity, _)| identity.match_id == state.match_id)
        .collect::<Vec<_>>();
    ordered.sort_by_key(|(_, identity, _)| identity.shard_id);
    let mut collected = 0_u8;
    for (entity, _, shard_position) in ordered {
        let Some((_, carrier)) = carriers.values_mut().find(|(position, carrier)| {
            carrier.0 < maximum
                && position.distance_squared(shard_position.0) <= SHARD_PICKUP_RADIUS.powi(2)
        }) else {
            continue;
        };
        carrier.0 = carrier.0.saturating_add(1);
        commands.entity(entity).despawn();
        collected = collected.saturating_add(1);
    }
    shard_rush.ground_shards = shard_rush.ground_shards.saturating_sub(collected);
    telemetry.shards_collected = telemetry
        .shards_collected
        .saturating_add(u16::from(collected));
}

/// Defeat and disconnect outcomes: a defeated carrier scatters every carried shard around its
/// defeat position; a carrier that left the connected roster, or whose routed seat went on hold
/// (a lost link the bot now drives), scatters at its last position.
#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
fn drop_carried_shards(
    mut signals: DropSignals,
    mut ledger: ShardLedger,
    mut roots: Query<(&MatchState, &mut ShardRushState), With<MatchRoot>>,
    mut fighters: Query<
        (
            &NetworkEntityId,
            &MatchParticipant,
            &Position,
            &mut ShardCarrier,
            Has<SeatHold>,
        ),
        With<Fighter>,
    >,
    placement: ShardPlacement,
) {
    let departed = std::mem::take(&mut signals.departed.0);
    let DropSignals {
        tick,
        roster,
        facts,
        ..
    } = signals;
    let Ok((state, mut shard_rush)) = roots.single_mut() else {
        return;
    };
    if !matches!(state.phase, MatchPhase::Active { .. }) || state.match_id != shard_rush.match_id {
        return;
    }
    let defeated = facts
        .0
        .iter()
        .filter(|fact| {
            fact.tick == tick.0
                && matches!(fact.kind, CombatOutcomeKind::Defeat)
                && fact.target_kind == CombatTargetKind::Fighter
        })
        .map(|fact| fact.target_network_id.0)
        .collect::<std::collections::BTreeSet<_>>();
    let roster_current = roster.match_id == Some(state.match_id);
    let mut dropping = fighters
        .iter_mut()
        .filter(|(network_id, participant, _, carrier, held)| {
            carrier.0 > 0
                && participant.match_id == state.match_id
                && (*held
                    || defeated.contains(&network_id.0)
                    || (roster_current && !roster.connected_network_ids.contains(&network_id.0)))
        })
        .collect::<Vec<_>>();
    dropping.sort_by_key(|(network_id, ..)| network_id.0);
    let mut scatter = |origin: Vec2, carried: u8| {
        for index in 0..carried {
            let offset = shard_fan_offset(u16::from(index), 18.0, 4.0);
            let point = placement.resolve(origin + offset, origin);
            if ledger.spawn(state.match_id, ShardOrigin::Dropped, point) {
                shard_rush.ground_shards = shard_rush.ground_shards.saturating_add(1);
            }
        }
    };
    for (_, _, position, mut carrier, _) in dropping {
        scatter(position.0, carrier.0);
        carrier.0 = 0;
    }
    let mut departed = departed
        .into_iter()
        .filter(|carrier| carrier.match_id == state.match_id)
        .collect::<Vec<_>>();
    departed.sort_by_key(|carrier| carrier.network_id);
    for carrier in departed {
        scatter(carrier.position, carrier.carried);
    }
}

/// Team totals, the reset-on-loss hold countdown, and the threshold outcome. Runs after drops
/// so a defeat that breaks the lead resets the countdown on the same tick.
#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
fn evaluate_shard_rush_hold(
    tick: Res<SimulationTick>,
    roster: Res<ConnectedMatchRoster>,
    mut telemetry: ResMut<ShardRushTelemetry>,
    mut outcomes: ResMut<ModeRuleOutcome>,
    mut diagnostics: ResMut<MatchOutcomeDiagnostics>,
    mut roots: Query<(&MatchState, &mut ShardRushState), With<MatchRoot>>,
    fighters: Query<
        (&NetworkEntityId, &MatchParticipant, &TeamId, &ShardCarrier),
        (With<Fighter>, Without<SeatHold>),
    >,
) {
    let Ok((state, mut shard_rush)) = roots.single_mut() else {
        return;
    };
    if !matches!(state.phase, MatchPhase::Active { .. }) || state.match_id != shard_rush.match_id {
        return;
    }
    if roster.match_id != Some(state.match_id) {
        return;
    }
    telemetry.begin_match(state.match_id);
    let mut totals = [0_u8; 2];
    for (network_id, participant, team, carrier) in &fighters {
        if participant.match_id == state.match_id
            && team.0 <= 1
            && roster.connected_network_ids.contains(&network_id.0)
        {
            let index = usize::from(team.0);
            totals[index] = totals[index].saturating_add(carrier.0);
        }
    }
    let previous = shard_rush.countdown;
    let next = next_countdown(
        previous,
        leading_team(totals, shard_rush.target_shards),
        tick.0,
        shard_rush.hold_ticks,
    );
    if let Some(started) = next.filter(|next| previous.is_none_or(|old| old.team != next.team)) {
        let index = usize::from(started.team.0);
        telemetry.countdown_starts_by_team[index] =
            telemetry.countdown_starts_by_team[index].saturating_add(1);
    }
    if previous.is_some()
        && next.is_none_or(|next| previous.is_some_and(|old| old.team != next.team))
    {
        telemetry.countdown_resets = telemetry.countdown_resets.saturating_add(1);
    }
    shard_rush.team_shards = totals;
    shard_rush.countdown = next;
    if let Some(countdown) = next.filter(|countdown| tick.0 >= countdown.ends_at_tick) {
        offer_mode_rule_outcome(
            &mut outcomes,
            &mut diagnostics,
            PendingModeRuleOutcome {
                match_id: state.match_id,
                evaluated_tick: tick.0,
                cause: ModeOutcomeCause::Threshold,
                result: crate::matchplay::MatchResult::TeamVictory {
                    team: countdown.team,
                },
            },
        );
    }
}

/// Deadline rule: a countdown already due wins; otherwise the held totals decide or draw.
#[allow(clippy::needless_pass_by_value)]
fn resolve_shard_rush_deadline(
    tick: Res<SimulationTick>,
    roots: Query<(&MatchState, &ShardRushState), With<MatchRoot>>,
    mut outcomes: ResMut<ModeRuleOutcome>,
    mut diagnostics: ResMut<MatchOutcomeDiagnostics>,
) {
    let Ok((state, shard_rush)) = roots.single() else {
        return;
    };
    let MatchPhase::Active { ends_at_tick } = state.phase else {
        return;
    };
    if tick.0 < ends_at_tick || state.match_id != shard_rush.match_id {
        return;
    }
    let (cause, result) = match shard_rush
        .countdown
        .filter(|countdown| tick.0 >= countdown.ends_at_tick)
    {
        Some(countdown) => (
            ModeOutcomeCause::Threshold,
            crate::matchplay::MatchResult::TeamVictory {
                team: countdown.team,
            },
        ),
        None => (
            ModeOutcomeCause::Timeout,
            timeout_result(shard_rush.team_shards),
        ),
    };
    offer_mode_rule_outcome(
        &mut outcomes,
        &mut diagnostics,
        PendingModeRuleOutcome {
            match_id: state.match_id,
            evaluated_tick: tick.0,
            cause,
            result,
        },
    );
}

#[allow(clippy::needless_pass_by_value)]
fn reset_shard_rush_on_restart(
    mut ledger: ShardLedger,
    mut departed: ResMut<DepartedCarriers>,
    pending: Res<PendingMatchRestart>,
    rules: Res<ShardRushRules>,
    mut roots: Query<&mut ShardRushState, With<MatchRoot>>,
    shards: Query<Entity, With<Shard>>,
    mut carriers: Query<&mut ShardCarrier, With<Fighter>>,
) {
    let Some(slot) = pending.slot() else { return };
    for entity in &shards {
        ledger.commands.entity(entity).despawn();
    }
    for mut carrier in &mut carriers {
        carrier.0 = 0;
    }
    *ledger.ids = NextShardId::default();
    departed.0.clear();
    ledger.telemetry.begin_match(slot.next_id);
    if let Ok(mut state) = roots.single_mut() {
        *state = initial_state(slot.next_id, state.source_anchor_id, *rules);
    }
}

/// Completion freezes the root state for Results and removes the ground shards.
fn cleanup_completed_shards(
    mut commands: Commands,
    roots: Query<&MatchState, With<MatchRoot>>,
    shards: Query<Entity, With<Shard>>,
) {
    let Ok(state) = roots.single() else { return };
    if !matches!(state.phase, MatchPhase::Completed { .. }) {
        return;
    }
    for entity in &shards {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{CombatEventId, CombatOutcomeFact, WorldPoint};
    use crate::matchplay::{MatchResult, ShardRushPolicy};

    const MATCH: MatchId = MatchId(7);
    const ENDS_AT: u64 = 10_000;

    fn rules() -> ShardRushRules {
        ShardRushRules {
            target_shards: 3,
            policy: ShardRushPolicy::PROCESS_VERIFICATION,
        }
    }

    /// Shard Rush rule systems over a real Prism Quarry resolution, without the network graph.
    fn app() -> (App, Entity) {
        let catalog = crate::map::MapContentCatalog::embedded().unwrap();
        let resolved = catalog
            .resolve_preset(
                crate::map::PRISM_QUARRY_PRESET,
                crate::map::MapInstanceId(1),
            )
            .unwrap();
        let source = resolved.shard_source.unwrap();
        let mut app = App::new();
        app.insert_resource(SimulationTick(1))
            .insert_resource(rules())
            .insert_resource(ShardRushSource {
                anchor_id: source.anchor_id,
                position: source.position,
            })
            .insert_resource(resolved)
            .insert_resource(MapCatalogResource(catalog))
            .insert_resource(ConnectedMatchRoster {
                match_id: Some(MATCH),
                ..Default::default()
            })
            .init_resource::<NextShardId>()
            .init_resource::<ShardRushTelemetry>()
            .init_resource::<DepartedCarriers>()
            .add_observer(record_departed_carrier)
            .init_resource::<ModeRuleOutcome>()
            .init_resource::<MatchOutcomeDiagnostics>()
            .init_resource::<CombatOutcomeFacts>()
            .add_systems(
                Update,
                (
                    spawn_source_shards,
                    collect_shards,
                    drop_carried_shards,
                    evaluate_shard_rush_hold,
                )
                    .chain(),
            )
            .add_systems(PostUpdate, resolve_shard_rush_deadline);
        let root = app
            .world_mut()
            .spawn((
                MatchRoot,
                MatchState {
                    match_id: MATCH,
                    mode_definition_id: crate::map::SHARD_RUSH_MODE_DEFINITION,
                    phase: MatchPhase::Active {
                        ends_at_tick: ENDS_AT,
                    },
                    rules_revision: crate::matchplay::SHARD_RUSH_RULES_REVISION,
                },
                initial_state(MATCH, source.anchor_id, rules()),
            ))
            .id();
        (app, root)
    }

    fn fighter(app: &mut App, network_id: u64, team: u8, position: Vec2) -> Entity {
        app.world_mut()
            .resource_mut::<ConnectedMatchRoster>()
            .connected_network_ids
            .insert(network_id);
        app.world_mut()
            .spawn((
                Fighter,
                ActiveCombatant,
                NetworkEntityId(network_id),
                MatchParticipant {
                    match_id: MATCH,
                    ready: true,
                    restart_ready: false,
                },
                TeamId(team),
                CurrentHealth(100),
                Position(position),
                ShardCarrier::default(),
            ))
            .id()
    }

    fn step(app: &mut App) {
        app.update();
        app.world_mut().resource_mut::<SimulationTick>().0 += 1;
    }

    fn state(app: &App, root: Entity) -> ShardRushState {
        *app.world().get::<ShardRushState>(root).unwrap()
    }

    fn carried(app: &App, fighter: Entity) -> u8 {
        app.world().get::<ShardCarrier>(fighter).unwrap().0
    }

    fn ground(app: &mut App) -> Vec<Vec2> {
        let world = app.world_mut();
        world
            .query_filtered::<&Position, With<Shard>>()
            .iter(world)
            .map(|position| position.0)
            .collect()
    }

    fn set_carried(app: &mut App, fighter: Entity, count: u8) {
        app.world_mut().get_mut::<ShardCarrier>(fighter).unwrap().0 = count;
    }

    fn defeat(app: &mut App, network_id: u64) {
        let tick = app.world().resource::<SimulationTick>().0;
        app.world_mut()
            .resource_mut::<CombatOutcomeFacts>()
            .0
            .push(CombatOutcomeFact {
                event_id: CombatEventId(tick),
                tick,
                attack_id: crate::combat::AttackId(tick),
                source_kind: crate::combat::CombatSourceKind::PrimaryWeapon,
                source_player: None,
                source_network_id: None,
                source_team: None,
                target_network_id: NetworkEntityId(network_id),
                target_kind: CombatTargetKind::Fighter,
                target_team: TeamId(0),
                preset_id: None,
                recipe_fingerprint: None,
                position: WorldPoint { x: 0.0, y: 0.0 },
                engagement_distance: 0.0,
                kind: CombatOutcomeKind::Defeat,
            });
    }

    fn outcome(app: &mut App) -> Option<PendingModeRuleOutcome> {
        app.world_mut().resource_mut::<ModeRuleOutcome>().take()
    }

    #[test]
    fn source_spawns_on_its_cadence_up_to_the_live_ceiling_and_fighters_collect() {
        let (mut app, root) = app();
        let source = app.world().resource::<ShardRushSource>().position;
        step(&mut app);
        let first = ground(&mut app);
        assert_eq!(first.len(), 1, "the first active tick spawns one shard");
        assert!(
            first[0].distance(source) < 1.0,
            "the opening shard sits on the source, equidistant from mirrored spawns"
        );
        for _ in 0..29 {
            step(&mut app);
        }
        assert_eq!(
            ground(&mut app).len(),
            1,
            "the next spawn waits a full interval"
        );
        step(&mut app);
        assert_eq!(ground(&mut app).len(), 2);
        assert_eq!(state(&app, root).ground_shards, 2);

        let collector = fighter(&mut app, 1, 0, source);
        step(&mut app);
        assert_eq!(carried(&app, collector), 2);
        assert!(ground(&mut app).is_empty());
        assert_eq!(state(&app, root).team_shards, [2, 0]);

        // A full ground skips deadlines instead of queueing spawns.
        app.world_mut().entity_mut(collector).despawn();
        for _ in 0..(30 * 20) {
            step(&mut app);
        }
        assert_eq!(
            ground(&mut app).len(),
            usize::from(ShardRushPolicy::PROCESS_VERIFICATION.live_shard_ceiling)
        );
        let telemetry = *app.world().resource::<ShardRushTelemetry>();
        assert_eq!(telemetry.shards_collected, 2);
    }

    #[test]
    fn a_strict_lead_at_the_target_counts_down_and_resets_when_lost() {
        let (mut app, root) = app();
        let far = Vec2::new(-10_000.0, -10_000.0);
        let leader = fighter(&mut app, 1, 0, far);
        let rival = fighter(&mut app, 2, 1, far);
        set_carried(&mut app, leader, 3);
        step(&mut app);
        let countdown = state(&app, root).countdown.unwrap();
        assert_eq!(countdown.team, TeamId(0));
        assert_eq!(countdown.ends_at_tick, countdown.started_at_tick + 60);

        // An equal total is no strict lead: the countdown resets.
        set_carried(&mut app, rival, 3);
        step(&mut app);
        assert_eq!(state(&app, root).countdown, None);

        // The other team takes the lead and starts again from the full duration.
        set_carried(&mut app, rival, 4);
        let restarted_at = app.world().resource::<SimulationTick>().0;
        step(&mut app);
        let countdown = state(&app, root).countdown.unwrap();
        assert_eq!(countdown.team, TeamId(1));
        assert_eq!(countdown.ends_at_tick, restarted_at + 60);
        for _ in 0..59 {
            step(&mut app);
            assert_eq!(outcome(&mut app), None);
        }
        step(&mut app);
        let won = outcome(&mut app).unwrap();
        assert_eq!(won.cause, ModeOutcomeCause::Threshold);
        assert_eq!(won.result, MatchResult::TeamVictory { team: TeamId(1) });
        let telemetry = *app.world().resource::<ShardRushTelemetry>();
        assert_eq!(telemetry.countdown_starts_by_team, [1, 1]);
        assert_eq!(telemetry.countdown_resets, 1);
    }

    #[test]
    fn defeated_and_disconnected_carriers_scatter_every_carried_shard() {
        let (mut app, root) = app();
        let origin = app.world().resource::<ShardRushSource>().position + Vec2::new(0.0, 200.0);
        let defeated = fighter(&mut app, 1, 0, origin);
        let leaver = fighter(&mut app, 2, 1, origin + Vec2::new(300.0, 0.0));
        set_carried(&mut app, defeated, 3);
        set_carried(&mut app, leaver, 2);
        // Keep collection out of range of the drop by removing both from the active roster.
        app.world_mut()
            .entity_mut(defeated)
            .remove::<ActiveCombatant>();
        app.world_mut()
            .resource_mut::<ConnectedMatchRoster>()
            .connected_network_ids
            .remove(&2);
        defeat(&mut app, 1);
        step(&mut app);
        assert_eq!(carried(&app, defeated), 0);
        assert_eq!(carried(&app, leaver), 0);
        let dropped = {
            let world = app.world_mut();
            world
                .query::<(&ShardOrigin, &Position)>()
                .iter(world)
                .filter(|(origin, _)| **origin == ShardOrigin::Dropped)
                .map(|(_, position)| position.0)
                .collect::<Vec<_>>()
        };
        assert_eq!(dropped.len(), 5);
        let near_defeat = dropped.iter().filter(|p| p.distance(origin) < 60.0).count();
        assert_eq!(near_defeat, 3, "the fan stays around the defeat position");
        assert_eq!(state(&app, root).team_shards, [0, 0]);
        assert_eq!(
            app.world().resource::<ShardRushTelemetry>().shards_dropped,
            5
        );

        // A carrier whose entity is removed (seat departure) scatters at its last position.
        let departing = fighter(&mut app, 3, 1, origin - Vec2::new(300.0, 0.0));
        set_carried(&mut app, departing, 2);
        app.world_mut().entity_mut(departing).despawn();
        step(&mut app);
        assert_eq!(
            app.world().resource::<ShardRushTelemetry>().shards_dropped,
            7
        );
    }

    #[test]
    fn a_held_seat_scatters_its_shards_and_neither_collects_nor_counts_while_held() {
        let (mut app, root) = app();
        let source = app.world().resource::<ShardRushSource>().position;
        let origin = source + Vec2::new(0.0, 200.0);
        // A held seat stays in the connected roster (BRL-yyeg) but its player is gone.
        let held = fighter(&mut app, 1, 0, origin);
        set_carried(&mut app, held, 3);
        app.world_mut().entity_mut(held).insert(SeatHold {
            expires_at_tick: 10_000,
        });
        step(&mut app);
        assert_eq!(
            carried(&app, held),
            0,
            "the hold scatters every carried shard"
        );
        assert_eq!(
            app.world().resource::<ShardRushTelemetry>().shards_dropped,
            3
        );
        assert_eq!(state(&app, root).team_shards, [0, 0]);
        assert_eq!(state(&app, root).countdown, None);

        // The bot-driven held fighter walks over shards without collecting them.
        app.world_mut().get_mut::<Position>(held).unwrap().0 = origin;
        for _ in 0..5 {
            step(&mut app);
        }
        assert_eq!(carried(&app, held), 0);

        // A carried count on a held seat (never expected) still cannot win the countdown.
        app.world_mut().get_mut::<Position>(held).unwrap().0 = Vec2::new(-10_000.0, -10_000.0);
        set_carried(&mut app, held, 3);
        step(&mut app);
        assert_eq!(carried(&app, held), 0);
        assert_eq!(state(&app, root).countdown, None);
    }

    #[test]
    fn the_deadline_awards_a_finished_hold_then_the_higher_total_or_a_draw() {
        let (mut app, root) = app();
        app.world_mut().resource_mut::<SimulationTick>().0 = ENDS_AT;
        let set = |app: &mut App, shards: [u8; 2]| {
            let mut state = app.world_mut().get_mut::<ShardRushState>(root).unwrap();
            state.team_shards = shards;
            state.countdown = None;
        };
        set(&mut app, [2, 2]);
        app.world_mut().run_schedule(PostUpdate);
        let draw = outcome(&mut app).unwrap();
        assert_eq!(
            (draw.cause, draw.result),
            (ModeOutcomeCause::Timeout, MatchResult::Draw)
        );

        set(&mut app, [1, 2]);
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(
            outcome(&mut app).unwrap().result,
            MatchResult::TeamVictory { team: TeamId(1) }
        );

        app.world_mut()
            .get_mut::<ShardRushState>(root)
            .unwrap()
            .countdown = Some(crate::matchplay::ShardRushCountdown {
            team: TeamId(0),
            started_at_tick: ENDS_AT - 60,
            ends_at_tick: ENDS_AT,
        });
        app.world_mut().run_schedule(PostUpdate);
        let hold = outcome(&mut app).unwrap();
        assert_eq!(hold.cause, ModeOutcomeCause::Threshold);
        assert_eq!(hold.result, MatchResult::TeamVictory { team: TeamId(0) });
    }
}
