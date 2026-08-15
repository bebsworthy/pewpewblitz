//! Server-owned fixed-tick Wipeout lifecycle and scoring.
#![allow(clippy::needless_pass_by_value)]

use super::{
    ActiveCombatant, AuthoritativeFighterLifecyclePlugin, FighterLifecycleConfig, FighterReset,
    MatchId, MatchMember, MatchParticipant, MatchParticipantSummary, MatchPhase, MatchResult,
    MatchRoot, MatchState, MatchTelemetry, MatchTelemetryContext, RespawnState, SpawnCandidate,
    SpawnProtection, WIPEOUT_RULES_REVISION, WipeoutRules, complete_fighter_lifecycle,
    complete_phase, reset_fighter_runtime, score_result, select_spawn, timeout_result,
};
use crate::{
    combat::{
        ActiveAttackTrackers, CombatOutbox, CombatOutcomeFacts, CombatOutcomeKind, Defeated,
        FighterDefinitions, MeleeAttack, PendingDelivery, PendingPayload, SelectedBuild,
        SelectedWeapon, SpawnState, TeamId, WeaponDefinitions, WeaponTelemetry,
    },
    gameplay::GameplaySet,
    map::{MapStartupSet, ResolvedMap, WIPEOUT_MODE_DEFINITION},
    protocol::{Fighter, FighterInput, NetworkEntityId, PlayerId},
    timing::SimulationTick,
};
use avian2d::prelude::{LayerMask, Position};
use bevy::prelude::*;
use lightyear::prelude::input::native::{ActionState, NativeBuffer};
use lightyear::prelude::{NetworkTarget, Replicate};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Resource, Debug)]
struct NextMatchId(u64);

#[derive(Resource, Default, Debug)]
struct RespawnOrdinals(BTreeMap<u64, u64>);

#[derive(Resource, Default, Debug)]
struct ScoredCombatEvents {
    match_id: Option<MatchId>,
    ids: BTreeSet<u64>,
}

#[derive(Resource, Default, Debug)]
struct PriorMatchPositions(BTreeMap<u64, Vec2>);

#[derive(Resource, Default, Debug)]
struct KnownMatchRoster {
    match_id: Option<MatchId>,
    players: BTreeSet<u64>,
}

#[derive(Resource, Default, Debug)]
struct RestartCleanupEpoch(Option<MatchId>);

impl Default for NextMatchId {
    fn default() -> Self {
        Self(1)
    }
}

impl NextMatchId {
    fn allocate(&mut self) -> MatchId {
        let id = MatchId(self.0.max(1));
        self.0 =
            id.0.checked_add(1)
                .expect("match identifier space exhausted");
        id
    }
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MatchSet {
    Lifecycle,
    FighterLifecycle,
    Outcomes,
}

pub struct WipeoutPlugin;

fn configure_match_schedule(app: &mut App) {
    app.configure_sets(
        FixedUpdate,
        (MatchSet::Lifecycle, MatchSet::FighterLifecycle).chain(),
    );
    app.configure_sets(
        FixedPostUpdate,
        MatchSet::Outcomes
            .after(crate::combat::CombatSet::Damage)
            .before(crate::combat::CombatSet::Lifecycle),
    );
}

impl Plugin for WipeoutPlugin {
    fn build(&self, app: &mut App) {
        let configured = app
            .world()
            .get_resource::<WipeoutRules>()
            .copied()
            .unwrap_or_default();
        let rules = if std::env::var("BRAWLER_NETWORK_ASSERT_MATCH").as_deref() == Ok("1") {
            WipeoutRules {
                target_score: 3,
                countdown_ticks: 30,
                active_limit_ticks: 1_200,
                respawn_delay_ticks: 30,
                spawn_protection_ticks: 10,
                completed_input_lock_ticks: 10,
                ..configured
            }
        } else {
            configured
        }
        .validate()
        .expect("Wipeout rules must be valid");
        configure_match_schedule(app);
        app.add_plugins(AuthoritativeFighterLifecyclePlugin)
            .insert_resource(rules)
            .insert_resource(FighterLifecycleConfig {
                spawn_protection_ticks: rules.spawn_protection_ticks,
            })
            .init_resource::<NextMatchId>()
            .init_resource::<RespawnOrdinals>()
            .init_resource::<ScoredCombatEvents>()
            .init_resource::<MatchTelemetry>()
            .init_resource::<PriorMatchPositions>()
            .init_resource::<KnownMatchRoster>()
            .init_resource::<RestartCleanupEpoch>()
            .add_systems(
                Startup,
                initialize_match_root.after(MapStartupSet::Instantiate),
            )
            .add_systems(
                FixedUpdate,
                (
                    advance_match_phase,
                    cleanup_restarted_match,
                    select_due_respawn_spawns,
                )
                    .chain()
                    .in_set(GameplaySet::Lifecycle)
                    .in_set(MatchSet::Lifecycle),
            )
            .add_systems(
                FixedPostUpdate,
                (resolve_match_outcomes, ApplyDeferred, capture_match_summary)
                    .chain()
                    .in_set(MatchSet::Outcomes),
            )
            .add_systems(
                FixedPostUpdate,
                record_match_movement
                    .after(avian2d::prelude::PhysicsSystems::StepSimulation)
                    .before(crate::combat::CombatSet::ProjectileSweep),
            );
    }
}

#[allow(clippy::type_complexity)]
fn record_match_movement(
    rules: Res<WipeoutRules>,
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

fn initialize_match_root(
    mut commands: Commands,
    mut ids: ResMut<NextMatchId>,
    rules: Res<WipeoutRules>,
    roots: Query<(), With<MatchRoot>>,
) {
    if roots.is_empty() {
        commands.spawn((
            MatchRoot,
            MatchState {
                match_id: ids.allocate(),
                mode_definition_id: WIPEOUT_MODE_DEFINITION,
                phase: MatchPhase::Waiting,
                team_scores: [0, 0],
                target_score: rules.target_score,
                rules_revision: WIPEOUT_RULES_REVISION,
            },
            Replicate::to_clients(NetworkTarget::All),
        ));
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity
)]
fn advance_match_phase(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    rules: Res<WipeoutRules>,
    mut ids: ResMut<NextMatchId>,
    mut roots: Query<&mut MatchState, With<MatchRoot>>,
    fighter_definitions: Res<FighterDefinitions>,
    weapon_definitions: Res<WeaponDefinitions>,
    weapon_telemetry: Res<WeaponTelemetry>,
    resolved_map: Res<ResolvedMap>,
    content_fingerprint: Res<crate::content::GameplayContentFingerprint>,
    mut known_roster: ResMut<KnownMatchRoster>,
    mut telemetry: ResMut<MatchTelemetry>,
    mut participants: Query<
        (
            Entity,
            &PlayerId,
            &NetworkEntityId,
            &mut MatchParticipant,
            &TeamId,
            &SelectedBuild,
            Option<&SelectedWeapon>,
        ),
        With<Fighter>,
    >,
    lifecycle_fighters: Query<(
        Entity,
        &crate::combat::FighterDefinitionId,
        &SelectedBuild,
        Option<&crate::combat::ResolvedWeapon>,
        &SpawnState,
    )>,
) {
    let Ok(mut state) = roots.single_mut() else {
        return;
    };
    let phase = state.phase;
    let mut counts = [0_u8; 2];
    let mut current_players = BTreeSet::new();
    let mut participant_summaries = Vec::new();
    let mut ready = true;
    for (_, player, network_id, participant, team, build, selected) in &participants {
        if participant.match_id != state.match_id || team.0 > 1 {
            continue;
        }
        ready &= participant.ready && selected.is_some();
        current_players.insert(player.0);
        counts[usize::from(team.0)] = counts[usize::from(team.0)].saturating_add(1);
        participant_summaries.push(MatchParticipantSummary {
            player_id: player.0,
            network_entity_id: network_id.0,
            team: *team,
            selected_build: *build,
        });
        if matches!(phase, MatchPhase::Active { .. }) {
            telemetry.record_participant_active_tick(*team, build.source_preset_id);
        }
    }
    participant_summaries.sort_by_key(|participant| participant.player_id);
    ready &= counts
        .into_iter()
        .all(|count| count >= rules.minimum_participants_per_team);
    if known_roster.match_id == Some(state.match_id) {
        if matches!(phase, MatchPhase::Active { .. }) {
            telemetry.record_disconnects(known_roster.players.difference(&current_players).count());
        }
        known_roster.players = current_players;
    } else {
        known_roster.match_id = Some(state.match_id);
        known_roster.players.clone_from(&current_players);
    }
    match phase {
        MatchPhase::Waiting if ready => {
            if let Some(starts_at_tick) = tick.0.checked_add(rules.countdown_ticks) {
                state.phase = MatchPhase::Countdown { starts_at_tick };
            }
        }
        MatchPhase::Countdown { .. } if !ready => {
            state.phase = MatchPhase::Waiting;
            for (_, _, _, mut participant, _, _, _) in &mut participants {
                participant.ready = false;
            }
        }
        MatchPhase::Countdown { starts_at_tick } if tick.0 >= starts_at_tick => {
            let Some(ends_at_tick) = tick.0.checked_add(rules.active_limit_ticks) else {
                return;
            };
            state.phase = MatchPhase::Active { ends_at_tick };
            telemetry.begin_with_weapons(state.match_id, tick.0, &weapon_telemetry);
            telemetry.set_context(MatchTelemetryContext {
                map_identity: resolved_map.snapshot.identity,
                content_fingerprint: *content_fingerprint,
                rules_revision: state.rules_revision,
                participants: participant_summaries,
            });
            for (entity, _, _, participant, _, _, _) in &mut participants {
                if participant.match_id == state.match_id {
                    let Ok((_, fighter_id, build, resolved, spawn)) =
                        lifecycle_fighters.get(entity)
                    else {
                        continue;
                    };
                    let Some(fighter) = fighter_definitions.get(*fighter_id) else {
                        continue;
                    };
                    let capacity = resolved.map_or_else(
                        || {
                            weapon_definitions
                                .get(build.primary_weapon)
                                .map_or(0, |weapon| weapon.magazine_capacity)
                        },
                        |weapon| weapon.recipe.economy.capacity(),
                    );
                    reset_fighter_runtime(
                        &mut commands,
                        entity,
                        FighterReset {
                            maximum_health: fighter.maximum_health,
                            ammunition: capacity,
                            position: spawn.position,
                            facing: spawn.facing,
                            collision_mask: crate::movement::INDESTRUCTIBLE_TERRAIN_LAYER
                                | crate::movement::DESTRUCTIBLE_TERRAIN_LAYER,
                            protection_until: Some(
                                tick.0.saturating_add(rules.spawn_protection_ticks),
                            ),
                            active: true,
                        },
                    );
                }
            }
        }
        MatchPhase::Active { .. } if counts[0] == 0 || counts[1] == 0 => {
            let result = match (counts[0] == 0, counts[1] == 0) {
                (true, true) => MatchResult::Draw,
                (true, false) => MatchResult::Forfeit {
                    winner: TeamId(1),
                    departed_team: TeamId(0),
                },
                (false, true) => MatchResult::Forfeit {
                    winner: TeamId(0),
                    departed_team: TeamId(1),
                },
                (false, false) => unreachable!(),
            };
            state.phase = complete_phase(tick.0, rules.completed_input_lock_ticks, result)
                .expect("validated completion deadline cannot overflow");
            complete_match_fighters(
                &mut commands,
                participants
                    .iter()
                    .filter_map(|(entity, _, _, participant, _, _, _)| {
                        (participant.match_id == state.match_id).then_some(entity)
                    }),
            );
        }
        MatchPhase::Active { ends_at_tick } if tick.0 >= ends_at_tick => {
            state.phase = complete_phase(
                tick.0,
                rules.completed_input_lock_ticks,
                timeout_result(state.team_scores),
            )
            .expect("validated match deadline cannot overflow");
            complete_match_fighters(
                &mut commands,
                participants
                    .iter()
                    .filter_map(|(entity, _, _, participant, _, _, _)| {
                        (participant.match_id == state.match_id).then_some(entity)
                    }),
            );
        }
        MatchPhase::Completed {
            restart_unlocked_at_tick,
            ..
        } if tick.0 >= restart_unlocked_at_tick
            && participants
                .iter()
                .filter(|(_, _, _, participant, _, _, _)| participant.match_id == state.match_id)
                .all(|(_, _, _, participant, _, _, _)| participant.restart_ready) =>
        {
            let new_id = ids.allocate();
            for (entity, _, _, mut participant, _, _, _) in &mut participants {
                participant.match_id = new_id;
                participant.ready = false;
                participant.restart_ready = false;
                commands
                    .entity(entity)
                    .insert(MatchMember(new_id))
                    .remove::<ActiveCombatant>()
                    .remove::<RespawnState>()
                    .remove::<SpawnProtection>();
            }
            for (entity, fighter_id, build, resolved, spawn) in &lifecycle_fighters {
                let Some(fighter) = fighter_definitions.get(*fighter_id) else {
                    continue;
                };
                let capacity = resolved.map_or_else(
                    || {
                        weapon_definitions
                            .get(build.primary_weapon)
                            .map_or(0, |weapon| weapon.magazine_capacity)
                    },
                    |weapon| weapon.recipe.economy.capacity(),
                );
                reset_fighter_runtime(
                    &mut commands,
                    entity,
                    FighterReset {
                        maximum_health: fighter.maximum_health,
                        ammunition: capacity,
                        position: spawn.position,
                        facing: spawn.facing,
                        collision_mask: LayerMask::NONE,
                        protection_until: None,
                        active: false,
                    },
                );
            }
            state.match_id = new_id;
            state.phase = MatchPhase::Waiting;
            state.team_scores = [0, 0];
        }
        _ => {}
    }
}

fn complete_match_fighters(commands: &mut Commands, entities: impl IntoIterator<Item = Entity>) {
    for entity in entities {
        complete_fighter_lifecycle(commands, entity);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn cleanup_restarted_match(
    mut commands: Commands,
    roots: Query<&MatchState, With<MatchRoot>>,
    mut epoch: ResMut<RestartCleanupEpoch>,
    mut ordinals: ResMut<RespawnOrdinals>,
    mut scored: ResMut<ScoredCombatEvents>,
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
    scored.match_id = Some(state.match_id);
    scored.ids.clear();
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
            commands.entity(entity).despawn();
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

#[allow(clippy::too_many_arguments)]
fn resolve_match_outcomes(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    rules: Res<WipeoutRules>,
    mut facts: ResMut<CombatOutcomeFacts>,
    mut scored_events: ResMut<ScoredCombatEvents>,
    mut telemetry: ResMut<MatchTelemetry>,
    mut roots: Query<&mut MatchState, With<MatchRoot>>,
    participants: Query<(Entity, &NetworkEntityId, &TeamId, &MatchParticipant), With<Fighter>>,
) {
    let Ok(mut state) = roots.single_mut() else {
        facts.0.clear();
        return;
    };
    if !matches!(state.phase, MatchPhase::Active { .. }) {
        facts.0.clear();
        return;
    }
    telemetry.begin(state.match_id, tick.0);
    if scored_events.match_id != Some(state.match_id) {
        scored_events.match_id = Some(state.match_id);
        scored_events.ids.clear();
    }
    facts.0.sort_by_key(|fact| fact.event_id.0);
    let mut defeated_entities = Vec::new();
    for fact in facts.0.drain(..) {
        if fact.tick != tick.0 {
            continue;
        }
        telemetry.record(fact, rules.retained_match_records);
        if !matches!(fact.kind, CombatOutcomeKind::Defeat)
            || !scored_events.ids.insert(fact.event_id.0)
        {
            continue;
        }
        let Some((target_entity, _, target_team, _)) =
            participants.iter().find(|(_, network_id, _, participant)| {
                **network_id == fact.target_network_id && participant.match_id == state.match_id
            })
        else {
            continue;
        };
        defeated_entities.push(target_entity);
        if let Some(source_team) = credited_defeat_team(&fact, *target_team) {
            let index = usize::from(source_team.0);
            increment_score(&mut state.team_scores[index]);
        }
    }
    if let Some(result) = score_result(state.team_scores, state.target_score) {
        state.phase = complete_phase(tick.0, rules.completed_input_lock_ticks, result)
            .expect("validated completion deadline cannot overflow");
        complete_match_fighters(
            &mut commands,
            participants
                .iter()
                .filter_map(|(entity, _, _, participant)| {
                    (participant.match_id == state.match_id).then_some(entity)
                }),
        );
    } else {
        for entity in defeated_entities {
            commands.entity(entity).insert(RespawnState {
                respawn_at_tick: tick.0.saturating_add(rules.respawn_delay_ticks),
            });
        }
    }
}

pub(crate) fn increment_score(score: &mut u16) {
    *score = score.saturating_add(1);
}

pub(crate) fn credited_defeat_team(
    fact: &crate::combat::CombatOutcomeFact,
    target_team: TeamId,
) -> Option<TeamId> {
    let source_team = fact.source_team?;
    (source_team.0 <= 1
        && source_team != target_team
        && fact.source_network_id != Some(fact.target_network_id))
    .then_some(source_team)
}

fn capture_match_summary(
    tick: Res<SimulationTick>,
    rules: Res<WipeoutRules>,
    roots: Query<&MatchState, With<MatchRoot>>,
    mut telemetry: ResMut<MatchTelemetry>,
    weapons: Res<WeaponTelemetry>,
) {
    let Ok(state) = roots.single() else {
        return;
    };
    match state.phase {
        MatchPhase::Active { .. } => telemetry.begin(state.match_id, tick.0),
        MatchPhase::Completed { result, .. } => telemetry.complete(
            tick.0,
            state.team_scores,
            result,
            rules.retained_match_summaries,
            &weapons,
        ),
        MatchPhase::Waiting | MatchPhase::Countdown { .. } => {}
    }
}

#[cfg(test)]
mod schedule_tests {
    use super::*;
    use crate::{combat::CombatSet, gameplay::GameplayPlugin};
    use bevy::time::TimeUpdateStrategy;

    #[derive(Resource, Default)]
    struct ScheduleTrace(Vec<&'static str>);

    #[derive(Component)]
    struct DeferredLifecycleMarker;

    fn lifecycle(mut commands: Commands, mut trace: ResMut<ScheduleTrace>) {
        trace.0.push("match lifecycle");
        commands.spawn(DeferredLifecycleMarker);
    }

    fn fire(markers: Query<(), With<DeferredLifecycleMarker>>, mut trace: ResMut<ScheduleTrace>) {
        assert_eq!(markers.iter().count(), 1);
        trace.0.push("fire");
    }

    fn damage(mut trace: ResMut<ScheduleTrace>) {
        trace.0.push("damage");
    }

    fn outcomes(mut trace: ResMut<ScheduleTrace>) {
        trace.0.push("match outcomes");
    }

    fn combat_lifecycle(mut trace: ResMut<ScheduleTrace>) {
        trace.0.push("combat lifecycle");
    }

    fn finalize(tick: Res<SimulationTick>, mut trace: ResMut<ScheduleTrace>) {
        assert_eq!(tick.0, 0);
        trace.0.push("finalize");
    }

    #[test]
    fn production_match_sets_have_an_explicit_fixed_tick_order() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, GameplayPlugin))
            .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
            .init_resource::<ScheduleTrace>();
        configure_match_schedule(&mut app);
        app.add_systems(
            FixedUpdate,
            (
                lifecycle
                    .in_set(GameplaySet::Lifecycle)
                    .in_set(MatchSet::Lifecycle),
                fire.in_set(GameplaySet::Fire),
            ),
        )
        .add_systems(
            FixedPostUpdate,
            (
                damage.in_set(CombatSet::Damage),
                outcomes.in_set(MatchSet::Outcomes),
                combat_lifecycle.in_set(CombatSet::Lifecycle),
                finalize
                    .in_set(CombatSet::Finalize)
                    .before(crate::gameplay::advance_simulation_tick),
            ),
        );

        app.update();
        app.update();

        assert_eq!(
            app.world().resource::<ScheduleTrace>().0,
            vec![
                "match lifecycle",
                "fire",
                "damage",
                "match outcomes",
                "combat lifecycle",
                "finalize",
            ]
        );
        assert_eq!(app.world().resource::<SimulationTick>().0, 1);
    }
}
