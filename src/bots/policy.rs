use super::{
    entropy,
    model::{
        BotContact, BotIntent, BotModeView, BotObjectKind, BotObservation, BotRole, BotState,
        BotTactic, MAX_CONTACTS,
    },
    navigation::{BotNavigationSnapshot, BotRouteProgress, BotRouteStart},
    profile::BotProfile,
};
use crate::{combat::WeaponPhase, map::MAP_CELL_SIZE_WORLD, protocol::FighterInput};
use bevy::prelude::*;

const AIM_ERROR_STREAM: u64 = 1;
const PERIMETER_RECOVERY_TRIGGER: f32 = MAP_CELL_SIZE_WORLD * 2.0;
const PERIMETER_RECOVERY_RELEASE: f32 = MAP_CELL_SIZE_WORLD * 5.0;

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

pub(super) fn decide(
    observation: &BotObservation,
    state: &mut BotState,
    profile: BotProfile,
    navigation: &BotNavigationSnapshot,
    seed: u64,
    role: BotRole,
    search_budget: usize,
) -> BotDecision {
    update_contacts(observation, state, profile.contact_memory_ticks);
    update_stationary_state(observation, state);
    let intent = choose_intent(observation, state, profile, role);
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
    let aim = intent.aim_target.map(|(position, velocity)| {
        let delta = position - observation.self_view.position;
        let distance = delta.length();
        let flight_ticks = if observation.projectile_speed > 0.0 {
            distance / observation.projectile_speed * 60.0
        } else {
            0.0
        };
        let intercept = position + velocity * (flight_ticks / 60.0);
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
    let mut buttons = 0;
    if intent.fire
        && observation.weapon_phase == WeaponPhase::Ready
        && observation.weapon_ammo > 0
        && intent.aim_target.is_some_and(|(position, _)| {
            let blockers: Vec<_> = dynamic_blockers
                .iter()
                .copied()
                .filter(|blocker| blocker.distance_squared(position) > 1.0)
                .collect();
            navigation.line_clear(observation.self_view.position, position, &blockers)
        })
    {
        buttons |= FighterInput::PRIMARY_FIRE;
    }
    if intent.dash
        && observation.ability_ready
        && state
            .last_dash_tick
            .is_none_or(|last| observation.tick.saturating_sub(last) > 12)
    {
        buttons |= FighterInput::ULTIMATE;
        state.last_dash_tick = Some(observation.tick);
    }
    BotDecision {
        input: FighterInput::from_axes_with_aim_distance(
            move_axis,
            aim,
            intent
                .aim_target
                .map(|(position, _)| position.distance(observation.self_view.position)),
            buttons,
        ),
        navigation: navigation_diagnostics,
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

#[allow(
    clippy::too_many_lines,
    reason = "the small utility transition remains one pure auditable decision"
)]
pub(super) fn choose_intent(
    observation: &BotObservation,
    state: &mut BotState,
    profile: BotProfile,
    role: BotRole,
) -> BotIntent {
    let self_position = observation.self_view.position;
    let health_fraction = f32::from(observation.self_view.current_health)
        / f32::from(observation.self_view.maximum_health.max(1));
    let pickup = observation
        .pickups
        .iter()
        .min_by(|a, b| distance_order(self_position, a.position, b.position));
    let enemy = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.active)
        .min_by(|a, b| distance_order(self_position, a.position, b.position));
    let remembered = state
        .contacts
        .iter()
        .min_by(|a, b| distance_order(self_position, a.position, b.position));

    if observation.tick >= state.tactic_until_tick {
        state.tactic = if health_fraction <= profile.retreat_health_fraction && pickup.is_some() {
            BotTactic::CollectPickup
        } else if health_fraction <= profile.retreat_health_fraction && enemy.is_some() {
            BotTactic::Retreat
        } else {
            match (role, observation.mode) {
                (BotRole::Objective, BotModeView::HotZone { .. }) => BotTactic::Contest,
                (BotRole::Defender, BotModeView::Heist) => BotTactic::DefendSafe,
                (BotRole::Objective, BotModeView::Heist) => BotTactic::AttackSafe,
                _ if enemy.is_some() => BotTactic::Pressure,
                _ if observation.objects.iter().any(|object| object.live) => BotTactic::BreakObject,
                _ => BotTactic::Pressure,
            }
        };
        state.tactic_until_tick = observation
            .tick
            .saturating_add(profile.tactic_commitment_ticks);
    }

    if state.tactic == BotTactic::CollectPickup
        && let Some(pickup) = pickup
    {
        return BotIntent {
            move_goal: Some(pickup.position),
            ..default()
        };
    }
    if state.tactic == BotTactic::Retreat
        && let Some(enemy) = enemy
    {
        let distance = self_position.distance(enemy.position);
        let preferred = observation.weapon_range * profile.preferred_range_fraction;
        let away = (self_position - enemy.position)
            .try_normalize()
            .unwrap_or(Vec2::X);
        return BotIntent {
            move_goal: Some(if distance < preferred {
                self_position + away * (preferred - distance).clamp(64.0, 160.0)
            } else {
                self_position
            }),
            aim_target: Some((enemy.position, enemy.velocity)),
            fire: true,
            dash: distance < preferred * 0.6,
        };
    }
    if let BotModeView::HotZone { center, radius, .. } = observation.mode
        && state.tactic == BotTactic::Contest
    {
        let hold = hot_zone_hold_point(center, radius, observation.self_view.network_id.0);
        return BotIntent {
            move_goal: Some(hold),
            aim_target: enemy.map(|enemy| (enemy.position, enemy.velocity)),
            fire: enemy.is_some_and(|enemy| {
                self_position.distance(enemy.position) <= observation.weapon_range
            }),
            dash: self_position.distance(center) > radius * 1.5,
        };
    }
    if state.tactic == BotTactic::AttackSafe
        && let Some(safe) = safe_object(observation, false)
    {
        let distance = self_position.distance(safe.position);
        let standoff = observation.weapon_range * profile.preferred_range_fraction;
        let direction = (safe.position - self_position)
            .try_normalize()
            .unwrap_or(Vec2::X);
        return BotIntent {
            move_goal: Some(if distance > standoff * 1.05 {
                safe.position - direction * standoff
            } else {
                self_position
            }),
            aim_target: Some((safe.position, Vec2::ZERO)),
            fire: distance <= observation.weapon_range,
            dash: distance > observation.weapon_range * 1.35,
        };
    }
    if state.tactic == BotTactic::DefendSafe
        && let Some(friendly_safe) = safe_object(observation, true)
    {
        let hostile_direction = safe_object(observation, false)
            .and_then(|safe| (safe.position - friendly_safe.position).try_normalize())
            .unwrap_or(Vec2::X);
        let anchor = friendly_safe.position
            + hostile_direction * (observation.weapon_range * 0.25).min(240.0);
        return BotIntent {
            move_goal: Some(anchor),
            aim_target: enemy.map(|enemy| (enemy.position, enemy.velocity)),
            fire: enemy.is_some_and(|enemy| {
                self_position.distance(enemy.position) <= observation.weapon_range
            }),
            dash: false,
        };
    }
    if let Some(enemy) = enemy {
        let distance = self_position.distance(enemy.position);
        let preferred = observation.weapon_range * profile.preferred_range_fraction;
        let direction = (enemy.position - self_position)
            .try_normalize()
            .unwrap_or(Vec2::X);
        let move_goal = if distance > preferred * 1.1 {
            enemy.position - direction * preferred
        } else if distance < preferred * 0.65 {
            self_position - direction * preferred * 0.4
        } else {
            let strafe = Vec2::new(-direction.y, direction.x)
                * if observation.self_view.network_id.0 & 1 == 0 {
                    1.0
                } else {
                    -1.0
                };
            self_position + strafe * 96.0
        };
        return BotIntent {
            move_goal: Some(move_goal),
            aim_target: Some((enemy.position, enemy.velocity)),
            fire: distance <= observation.weapon_range,
            dash: distance > observation.weapon_range * 1.2,
        };
    }

    let goal = remembered.map(|contact| contact.position).or_else(|| {
        observation
            .objects
            .iter()
            .filter(|object| object.live)
            .min_by(|a, b| distance_order(self_position, a.position, b.position))
            .map(|object| object.position)
    });
    let object_target = matches!(state.tactic, BotTactic::BreakObject | BotTactic::AttackSafe)
        .then(|| {
            observation.objects.iter().find(|object| {
                object.live
                    && goal.is_some_and(|goal| goal.distance_squared(object.position) < 1.0)
                    && matches!(
                        object.kind,
                        BotObjectKind::OilBarrel
                            | BotObjectKind::TreasureChest
                            | BotObjectKind::HeistSafe { .. }
                    )
            })
        })
        .flatten();
    if let Some(object) = object_target {
        let distance = self_position.distance(object.position);
        let standoff = observation.weapon_range * profile.preferred_range_fraction;
        let direction = (object.position - self_position)
            .try_normalize()
            .unwrap_or(Vec2::X);
        return BotIntent {
            move_goal: Some(if distance > standoff * 1.05 {
                object.position - direction * standoff
            } else {
                self_position
            }),
            aim_target: Some((object.position, Vec2::ZERO)),
            fire: distance <= observation.weapon_range,
            dash: false,
        };
    }
    BotIntent {
        move_goal: goal.or(Some(Vec2::ZERO)),
        aim_target: None,
        fire: false,
        dash: false,
    }
}

fn safe_object(
    observation: &BotObservation,
    friendly: bool,
) -> Option<&super::model::BotObjectView> {
    observation.objects.iter().find(|object| {
        matches!(
            object.kind,
            BotObjectKind::HeistSafe { defending_team }
                if object.live
                    && (defending_team == observation.self_view.team) == friendly
        )
    })
}

fn hot_zone_hold_point(center: Vec2, radius: f32, stable_id: u64) -> Vec2 {
    let directions = [Vec2::X, Vec2::Y, Vec2::NEG_X, Vec2::NEG_Y];
    let index = usize::try_from(stable_id % directions.len() as u64).unwrap_or(0);
    center + directions[index] * radius * 0.45
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
    if !navigation.is_inside_perimeter(position, PERIMETER_RECOVERY_TRIGGER) {
        state.perimeter_recovery = true;
    } else if state.perimeter_recovery
        && navigation.is_inside_perimeter(position, PERIMETER_RECOVERY_RELEASE)
    {
        state.perimeter_recovery = false;
    }
    let goal = if state.perimeter_recovery {
        Some(navigation.perimeter_recovery_goal(position, PERIMETER_RECOVERY_RELEASE))
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
        && observation.self_view.position.distance_squared(goal) > 28.0_f32.powi(2)
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
            <= 28.0_f32.powi(2)
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
            && delta.length_squared() < 72.0_f32.powi(2)
        {
            direction += away * 0.45;
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
    let goal_changed = state
        .route_goal
        .is_none_or(|prior| prior.distance_squared(goal) > 256.0);
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
            profile.maximum_search_expansions,
            profile.maximum_route_points,
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

fn rotate(vector: Vec2, radians: f32) -> Vec2 {
    let (sin, cos) = radians.sin_cos();
    Vec2::new(
        vector.x * cos - vector.y * sin,
        vector.x * sin + vector.y * cos,
    )
}

fn distance_order(origin: Vec2, a: Vec2, b: Vec2) -> std::cmp::Ordering {
    origin
        .distance_squared(a)
        .total_cmp(&origin.distance_squared(b))
        .then_with(|| a.x.total_cmp(&b.x))
        .then_with(|| a.y.total_cmp(&b.y))
}
