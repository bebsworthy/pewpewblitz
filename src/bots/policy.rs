use super::{
    entropy,
    model::{BotContact, BotObservation, BotRole, BotState, MAX_CONTACTS},
    navigation::{BotNavigationSnapshot, BotRouteProgress, BotRouteStart},
    profile::{BotArbitrationPolicy, BotProfile},
    registry::BotBehaviorRegistry,
};
use crate::{combat::WeaponPhase, map::MAP_CELL_SIZE_WORLD, protocol::FighterInput};
use bevy::prelude::*;

const AIM_ERROR_STREAM: u64 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum BotNavigationSearchStatus {
    #[default]
    None,
    Pending,
    Completed,
    Exhausted,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct BotNavigationDecisionDiagnostics {
    pub search_started: bool,
    pub status: BotNavigationSearchStatus,
    pub expansions: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct BotDecision {
    pub input: FighterInput,
    pub navigation: BotNavigationDecisionDiagnostics,
}

#[derive(Clone, Copy)]
pub(super) struct BotDecisionPolicy<'a> {
    profile: BotProfile,
    arbitration: &'a BotArbitrationPolicy,
    registry: &'a BotBehaviorRegistry,
}

impl<'a> BotDecisionPolicy<'a> {
    pub(super) const fn new(
        profile: BotProfile,
        arbitration: &'a BotArbitrationPolicy,
        registry: &'a BotBehaviorRegistry,
    ) -> Self {
        Self {
            profile,
            arbitration,
            registry,
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the pure decision keeps targeting, gating, and one FighterInput commit auditable together"
)]
pub(super) fn decide(
    observation: &BotObservation,
    state: &mut BotState,
    policy: BotDecisionPolicy<'_>,
    navigation: &BotNavigationSnapshot,
    seed: u64,
    role: BotRole,
    search_budget: usize,
) -> BotDecision {
    let BotDecisionPolicy {
        profile,
        arbitration,
        registry,
    } = policy;
    update_contacts(observation, state, profile.contact_memory_ticks);
    update_stationary_state(observation, state);
    let mut intent =
        super::behaviors::choose_intent(observation, state, profile, arbitration, registry, role);
    if observation.ultimate_kind == crate::builds::UltimateKind::RestorationField
        && observation.ability_ready
    {
        let restoration_target = observation
            .allies
            .iter()
            .filter(|ally| ally.active && ally.current_health < ally.maximum_health)
            .min_by_key(|ally| {
                u32::from(ally.current_health) * u32::from(observation.self_view.maximum_health)
                    / u32::from(ally.maximum_health.max(1))
            })
            .map_or(
                (
                    observation.self_view.position,
                    observation.self_view.velocity,
                ),
                |ally| (ally.position, ally.velocity),
            );
        if health_fraction(
            observation.self_view.current_health,
            observation.self_view.maximum_health,
        ) <= profile.restoration_health_fraction
            || observation.allies.iter().any(|ally| {
                ally.active
                    && health_fraction(ally.current_health, ally.maximum_health)
                        <= profile.restoration_health_fraction
            })
        {
            intent.aim_target = Some(restoration_target);
        }
    }
    let (move_axis, navigation_diagnostics) = movement_axis(
        observation,
        state,
        profile,
        navigation,
        intent.move_goal,
        search_budget,
    );
    let dynamic_blockers: Vec<_> = observation
        .objects
        .iter()
        .filter(|object| object.live)
        .map(|object| object.position)
        .collect();
    let ultimate = plan_ultimate(observation, state, &intent);
    if let UltimatePlan::Use {
        aim_target: Some(target),
        ..
    } = ultimate
    {
        intent.aim_target = Some(target);
    }
    let lead = intent
        .aim_target
        .map(|(position, velocity)| position + velocity * flight_seconds(observation, position));
    let aim = lead.map(|intercept| {
        if observation.tick >= state.aim_error_until_tick {
            state.aim_error_radians =
                entropy::signed_unit(seed, AIM_ERROR_STREAM, observation.tick)
                    * profile.maximum_aim_error_radians;
            state.aim_error_until_tick = observation.tick.saturating_add(profile.aim_hold_ticks);
        }
        rotate(
            intercept - observation.self_view.position,
            state.aim_error_radians,
        )
        .try_normalize()
        .unwrap_or(Vec2::X)
    });
    let mut aim_distance = if observation.lob_flight_ticks_at_range.is_some() {
        lead.map(|intercept| intercept.distance(observation.self_view.position))
    } else {
        intent
            .aim_target
            .map(|(position, _)| position.distance(observation.self_view.position))
    };
    let mut buttons = 0;
    let self_centered_ultimate = matches!(
        ultimate,
        UltimatePlan::Use {
            self_centered: true,
            ..
        }
    );
    // An ultimate that places itself or brings its own target owns this tick's aim, so primary
    // fire waits rather than spending a shot on that aim.
    let ultimate_owns_aim = matches!(
        ultimate,
        UltimatePlan::Use {
            aim_target: Some(_),
            ..
        }
    ) || self_centered_ultimate;
    if intent.fire
        && !ultimate_owns_aim
        && observation.weapon_phase == WeaponPhase::Ready
        && observation.weapon_ammo > 0
        && intent.aim_target.is_some_and(|(position, _)| {
            // A lob lands past cover on the authoritative landing point; other deliveries need a
            // clear line.
            observation.lob_flight_ticks_at_range.is_some() || {
                let blockers: Vec<_> = dynamic_blockers
                    .iter()
                    .copied()
                    .filter(|blocker| blocker.distance_squared(position) > 1.0)
                    .collect();
                navigation.line_clear(observation.self_view.position, position, &blockers)
            }
        })
    {
        buttons |= FighterInput::PRIMARY_FIRE;
    }
    if matches!(ultimate, UltimatePlan::Use { .. })
        && state.last_ultimate_tick.is_none_or(|last| {
            observation.tick.saturating_sub(last) > profile.ultimate_retrigger_ticks
        })
    {
        buttons |= FighterInput::ULTIMATE;
        state.last_ultimate_tick = Some(observation.tick);
        if self_centered_ultimate {
            aim_distance = Some(0.0);
        }
    }
    BotDecision {
        input: FighterInput::from_axes_with_aim_distance(move_axis, aim, aim_distance, buttons),
        navigation: navigation_diagnostics,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum UltimatePlan {
    Hold,
    /// Press the ultimate this tick. `aim_target` replaces the intent's aim when the ultimate
    /// needs its own target; `self_centered` places an area on the bot (aim distance zero).
    Use {
        aim_target: Option<(Vec2, Vec2)>,
        self_centered: bool,
    },
}

/// Choose whether the charged ultimate fires this tick from its resolved kind and reach. Every
/// branch reads only the delayed observation and the bot's own contact memory.
fn plan_ultimate(
    observation: &BotObservation,
    state: &BotState,
    intent: &super::model::BotIntent,
) -> UltimatePlan {
    use crate::builds::UltimateKind;
    if !observation.ability_ready {
        return UltimatePlan::Hold;
    }
    let self_position = observation.self_view.position;
    let within = |position: Vec2| position.distance(self_position) <= observation.ultimate_range;
    let use_on = |aim_target| UltimatePlan::Use {
        aim_target,
        self_centered: false,
    };
    match observation.ultimate_kind {
        UltimateKind::DemolitionStrike
        | UltimateKind::CryogenicField
        | UltimateKind::FireField
        | UltimateKind::PoisonField
        | UltimateKind::RestorationField
        | UltimateKind::BigBlob => {
            if intent
                .aim_target
                .is_some_and(|(position, _)| within(position))
            {
                use_on(None)
            } else {
                UltimatePlan::Hold
            }
        }
        // A Sentry lands beside the bot facing its aim, so it deploys only toward a visible enemy
        // the turret can acquire.
        UltimateKind::Sentry => nearest_visible_enemy(observation)
            .filter(|enemy| within(enemy.position))
            .map_or(UltimatePlan::Hold, |enemy| {
                use_on(Some((enemy.position, enemy.velocity)))
            }),
        // Scan where a remembered contact vanished; a visible enemy needs no reveal.
        UltimateKind::RevealScan => {
            if observation.visible_enemies.iter().any(|enemy| enemy.active) {
                return UltimatePlan::Hold;
            }
            state
                .contacts
                .iter()
                .filter(|contact| within(contact.position))
                .min_by(|a, b| {
                    a.position
                        .distance_squared(self_position)
                        .total_cmp(&b.position.distance_squared(self_position))
                })
                .map_or(UltimatePlan::Hold, |contact| {
                    use_on(Some((contact.position, Vec2::ZERO)))
                })
        }
        // Cover the bot while it is engaged or breaking away.
        UltimateKind::ConcealmentField => {
            let engaged = nearest_visible_enemy(observation).is_some_and(|enemy| {
                enemy.position.distance(self_position) <= observation.weapon_range
            });
            if engaged || (state.tactic == super::model::BotTactic::Retreat && intent.dash) {
                UltimatePlan::Use {
                    aim_target: None,
                    self_centered: true,
                }
            } else {
                UltimatePlan::Hold
            }
        }
        UltimateKind::Dash | UltimateKind::SelfCloak => {
            if intent.dash {
                use_on(None)
            } else {
                UltimatePlan::Hold
            }
        }
    }
}

fn nearest_visible_enemy(observation: &BotObservation) -> Option<&super::model::BotFighterView> {
    let origin = observation.self_view.position;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.active)
        .min_by(|a, b| {
            a.position
                .distance_squared(origin)
                .total_cmp(&b.position.distance_squared(origin))
                .then_with(|| a.network_id.cmp(&b.network_id))
        })
}

/// Seconds until the primary delivery reaches `target`: straight travel at projectile speed, or
/// the distance-scaled authored lob flight. Melee and cone deliveries resolve immediately.
fn flight_seconds(observation: &BotObservation, target: Vec2) -> f32 {
    let distance = target.distance(observation.self_view.position);
    if let Some(ticks) = observation.lob_flight_ticks_at_range {
        #[allow(
            clippy::cast_precision_loss,
            reason = "authored lob flights are small tick counts"
        )]
        let ticks = ticks as f32;
        (distance / observation.weapon_range.max(1.0)).clamp(0.0, 1.0) * ticks
            / crate::timing::SIMULATION_TICK_HZ_F32
    } else if observation.projectile_speed > 0.0 {
        distance / observation.projectile_speed
    } else {
        0.0
    }
}

fn update_contacts(observation: &BotObservation, state: &mut BotState, memory_ticks: u64) {
    state.contacts.retain(|contact| {
        observation.tick.saturating_sub(contact.observed_at_tick) <= memory_ticks
    });
    for enemy in &observation.visible_enemies {
        if let Some(contact) = state
            .contacts
            .iter_mut()
            .find(|contact| contact.network_id == enemy.network_id)
        {
            *contact = BotContact {
                network_id: enemy.network_id,
                position: enemy.position,
                velocity: enemy.velocity,
                observed_at_tick: observation.tick,
            };
        } else if state.contacts.len() < MAX_CONTACTS {
            state.contacts.push(BotContact {
                network_id: enemy.network_id,
                position: enemy.position,
                velocity: enemy.velocity,
                observed_at_tick: observation.tick,
            });
        }
    }
    state.contacts.sort_by_key(|contact| contact.network_id);
}

fn update_stationary_state(observation: &BotObservation, state: &mut BotState) {
    if state
        .last_position
        .is_some_and(|prior| prior.distance_squared(observation.self_view.position) < 1.0)
    {
        state.stationary_ticks = state.stationary_ticks.saturating_add(1);
    } else {
        state.stationary_ticks = 0;
    }
    state.last_position = Some(observation.self_view.position);
}

#[cfg(test)]
pub(super) fn choose_intent(
    observation: &BotObservation,
    state: &mut BotState,
    profile: BotProfile,
    arbitration_policy: &BotArbitrationPolicy,
    role: BotRole,
) -> super::model::BotIntent {
    super::behaviors::choose_intent(
        observation,
        state,
        profile,
        arbitration_policy,
        super::behaviors::built_in_registry(),
        role,
    )
}
fn movement_axis(
    observation: &BotObservation,
    state: &mut BotState,
    profile: BotProfile,
    navigation: &BotNavigationSnapshot,
    goal: Option<Vec2>,
    search_budget: usize,
) -> (Vec2, BotNavigationDecisionDiagnostics) {
    let diagnostics = BotNavigationDecisionDiagnostics::default();
    let position = observation.self_view.position;
    let perimeter_trigger = MAP_CELL_SIZE_WORLD * profile.perimeter_recovery_trigger_cells;
    let perimeter_release = MAP_CELL_SIZE_WORLD * profile.perimeter_recovery_release_cells;
    if !navigation.is_inside_perimeter(position, perimeter_trigger) {
        state.perimeter_recovery = true;
    } else if state.perimeter_recovery
        && navigation.is_inside_perimeter(position, perimeter_release)
    {
        state.perimeter_recovery = false;
    }
    let goal = if state.perimeter_recovery {
        Some(navigation.perimeter_recovery_goal(position, perimeter_release))
    } else {
        goal
    };
    let Some(goal) = goal.filter(|goal| goal.is_finite()) else {
        state.route_search = None;
        return (Vec2::ZERO, diagnostics);
    };
    let goal = navigation.clamp_goal(goal);
    if observation.tick < state.stuck_escape_until_tick {
        return (state.stuck_escape_axis, diagnostics);
    }
    if state.stationary_ticks >= profile.stuck_ticks
        && observation.self_view.position.distance_squared(goal)
            > profile.waypoint_reach_distance.powi(2)
    {
        let dynamic_blockers: Vec<_> = observation
            .objects
            .iter()
            .filter(|object| object.live)
            .map(|object| object.position)
            .collect();
        state.stuck_escape_axis = navigation.escape_axis(
            observation.self_view.position,
            goal,
            &dynamic_blockers,
            observation.self_view.network_id.0,
        );
        state.stuck_escape_until_tick = observation
            .tick
            .saturating_add(profile.tactic_cadence_ticks.saturating_mul(2));
        state.route.clear();
        state.route_cursor = 0;
        state.route_search = None;
        state.route_goal = None;
        state.stationary_ticks = 0;
        return (state.stuck_escape_axis, diagnostics);
    }
    let diagnostics = update_route(observation, state, profile, navigation, goal, search_budget);
    while state.route_cursor < state.route.len()
        && observation
            .self_view
            .position
            .distance_squared(state.route[state.route_cursor])
            <= profile.waypoint_reach_distance.powi(2)
    {
        state.route_cursor += 1;
    }
    let target = state.route.get(state.route_cursor).copied().unwrap_or(goal);
    let mut direction = (target - observation.self_view.position)
        .try_normalize()
        .unwrap_or(Vec2::ZERO);
    for ally in &observation.allies {
        let delta = observation.self_view.position - ally.position;
        if let Some(away) = delta.try_normalize()
            && delta.length_squared() < profile.ally_separation_distance.powi(2)
        {
            direction += away * profile.ally_separation_weight;
        }
    }
    (direction.clamp_length_max(1.0), diagnostics)
}

fn update_route(
    observation: &BotObservation,
    state: &mut BotState,
    profile: BotProfile,
    navigation: &BotNavigationSnapshot,
    goal: Vec2,
    search_budget: usize,
) -> BotNavigationDecisionDiagnostics {
    let mut diagnostics = BotNavigationDecisionDiagnostics::default();
    let goal_changed = state.route_goal.is_none_or(|prior| {
        prior.distance_squared(goal) > profile.route_goal_change_distance.powi(2)
    });
    if goal_changed {
        state.route.clear();
        state.route_cursor = 0;
        state.route_search = None;
    }
    let must_replan = state.route_search.is_none()
        && observation.tick >= state.route_retry_at_tick
        && (goal_changed
            || state.route_cursor >= state.route.len()
            || observation.tick % profile.tactic_cadence_ticks
                == observation.self_view.network_id.0 % profile.tactic_cadence_ticks
            || state.stationary_ticks >= profile.stuck_ticks);
    if must_replan {
        let dynamic_blockers: Vec<_> = observation
            .objects
            .iter()
            .filter(|object| object.live)
            .map(|object| object.position)
            .collect();
        match navigation.begin_route(
            observation.self_view.position,
            goal,
            &dynamic_blockers,
            profile.maximum_search_expansions(),
            profile.maximum_route_points(),
        ) {
            Some(BotRouteStart::Complete(route)) => {
                state.route = route;
                state.route_cursor = 0;
            }
            Some(BotRouteStart::Search(search)) => {
                diagnostics.search_started = true;
                state.route_search = Some(search);
            }
            None => {
                state.route.clear();
                state.route_cursor = 0;
                state.route_retry_at_tick = observation
                    .tick
                    .saturating_add(profile.tactic_cadence_ticks);
            }
        }
        state.route_goal = Some(goal);
        state.stationary_ticks = 0;
    }
    if let Some(mut search) = state.route_search.take() {
        let expansions_before = search.expansions();
        match search.advance(navigation, search_budget) {
            BotRouteProgress::Pending => {
                diagnostics.status = BotNavigationSearchStatus::Pending;
                diagnostics.expansions = search.expansions().saturating_sub(expansions_before);
                state.route_search = Some(search);
            }
            BotRouteProgress::Complete(route) => {
                diagnostics.status = BotNavigationSearchStatus::Completed;
                diagnostics.expansions = search.expansions().saturating_sub(expansions_before);
                state.route = route;
                state.route_cursor = 0;
            }
            BotRouteProgress::Exhausted => {
                diagnostics.status = BotNavigationSearchStatus::Exhausted;
                diagnostics.expansions = search.expansions().saturating_sub(expansions_before);
                state.route.clear();
                state.route_cursor = 0;
                state.route_retry_at_tick = observation
                    .tick
                    .saturating_add(profile.tactic_cadence_ticks);
            }
        }
    }
    diagnostics
}

fn health_fraction(current: u16, maximum: u16) -> f32 {
    f32::from(current) / f32::from(maximum.max(1))
}

fn rotate(vector: Vec2, radians: f32) -> Vec2 {
    let (sin, cos) = radians.sin_cos();
    Vec2::new(
        vector.x * cos - vector.y * sin,
        vector.x * sin + vector.y * cos,
    )
}
