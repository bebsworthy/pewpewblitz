//! Bounded, process-local Practice-bot intent contributions and deterministic arbitration.

use super::{
    model::{BotIntent, BotObservation, BotRole, BotState, BotTactic},
    profile::{BotArbitrationPolicy, BotBehaviorId, BotProfile, MAX_BOT_BEHAVIOR_REGISTRATIONS},
    registry::{BotBehaviorAppExt, BotBehaviorRegistry},
};
use bevy::prelude::{App, Plugin, Vec2};

const MAX_INTENT_CANDIDATES: usize = MAX_BOT_BEHAVIOR_REGISTRATIONS;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct IntentCandidate {
    behavior_id: BotBehaviorId,
    score: u16,
    tactic: Option<BotTactic>,
    intent: BotIntent,
}

impl IntentCandidate {
    const fn new(
        behavior_id: BotBehaviorId,
        score: u16,
        tactic: Option<BotTactic>,
        intent: BotIntent,
    ) -> Self {
        Self {
            behavior_id,
            score,
            tactic,
            intent,
        }
    }
}

pub(super) struct BehaviorContext<'a> {
    pub(super) observation: &'a BotObservation,
    pub(super) state: &'a BotState,
    pub(super) profile: BotProfile,
    pub(super) role: BotRole,
}

#[derive(Clone, Copy)]
pub(super) struct BehaviorRegistration {
    pub(super) id: BotBehaviorId,
    pub(super) contribute: fn(&BehaviorContext<'_>, &mut CandidateBuffer, u16),
}

impl BehaviorRegistration {
    pub(super) const fn new(
        id: BotBehaviorId,
        contribute: fn(&BehaviorContext<'_>, &mut CandidateBuffer, u16),
    ) -> Self {
        Self { id, contribute }
    }
}

#[derive(Default)]
pub(super) struct CandidateBuffer {
    candidates: Vec<IntentCandidate>,
}

impl CandidateBuffer {
    pub(super) fn propose(
        &mut self,
        behavior_id: BotBehaviorId,
        score: u16,
        tactic: Option<BotTactic>,
        intent: BotIntent,
    ) {
        if self.candidates.len() < MAX_INTENT_CANDIDATES {
            self.candidates
                .push(IntentCandidate::new(behavior_id, score, tactic, intent));
        }
    }
}

const HEALING: BehaviorRegistration =
    BehaviorRegistration::new(BotBehaviorId::HEALING, combat::healing);
const PRESSURE: BehaviorRegistration =
    BehaviorRegistration::new(BotBehaviorId::PRESSURE, combat::pressure);
const OBJECT: BehaviorRegistration =
    BehaviorRegistration::new(BotBehaviorId::OBJECT, combat::object);
const FALLBACK: BehaviorRegistration =
    BehaviorRegistration::new(BotBehaviorId::FALLBACK, combat::fallback);
const OBJECTIVES: BehaviorRegistration =
    BehaviorRegistration::new(BotBehaviorId::OBJECTIVES, objectives::contribute);
const PICKUPS: BehaviorRegistration =
    BehaviorRegistration::new(BotBehaviorId::PICKUPS, pickups::contribute);
const RETREAT: BehaviorRegistration =
    BehaviorRegistration::new(BotBehaviorId::RETREAT, retreat::contribute);

pub(super) struct BuiltInBotBehaviorsPlugin;

impl Plugin for BuiltInBotBehaviorsPlugin {
    fn build(&self, app: &mut App) {
        for registration in [
            HEALING, PRESSURE, OBJECT, FALLBACK, OBJECTIVES, PICKUPS, RETREAT,
        ] {
            app.try_register_bot_behavior(registration)
                .expect("built-in Practice bot behavior registration is valid");
        }
    }
}

#[cfg(test)]
pub(super) fn built_in_registry() -> &'static BotBehaviorRegistry {
    static REGISTRY: std::sync::OnceLock<BotBehaviorRegistry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut app = App::new();
        app.add_plugins((
            super::registry::BotBehaviorRegistryPlugin,
            BuiltInBotBehaviorsPlugin,
        ));
        crate::test_app::finalize(&mut app);
        app.world().resource::<BotBehaviorRegistry>().clone()
    })
}

pub(super) fn choose_intent(
    observation: &BotObservation,
    state: &mut BotState,
    profile: BotProfile,
    arbitration_policy: &BotArbitrationPolicy,
    registry: &BotBehaviorRegistry,
    role: BotRole,
) -> BotIntent {
    let context = BehaviorContext {
        observation,
        state,
        profile,
        role,
    };
    let candidates = collect_candidates(&context, registry.registrations(), arbitration_policy);
    let committed = (observation.tick < state.tactic_until_tick).then_some(state.tactic);
    let selected = arbitrate(
        &candidates.candidates,
        committed,
        arbitration_policy.commitment_score_bonus,
    )
    .expect("the combat policy always contributes a fallback intent");
    if committed.is_none()
        && let Some(tactic) = selected.tactic
    {
        state.tactic = tactic;
        state.tactic_until_tick = observation
            .tick
            .saturating_add(profile.tactic_commitment_ticks);
    }
    selected.intent
}

fn collect_candidates(
    context: &BehaviorContext<'_>,
    registrations: &[BehaviorRegistration],
    policy: &BotArbitrationPolicy,
) -> CandidateBuffer {
    let mut candidates = CandidateBuffer::default();
    for registration in registrations {
        let behavior = policy
            .behavior(registration.id)
            .expect("validated bot policy covers every code registration");
        if behavior.enabled {
            (registration.contribute)(context, &mut candidates, behavior.base_score);
        }
    }
    candidates
}

fn arbitrate(
    candidates: &[IntentCandidate],
    committed: Option<BotTactic>,
    commitment_score_bonus: u16,
) -> Option<IntentCandidate> {
    let mut selected = None;
    let mut selected_score = None;
    for candidate in candidates.iter().copied() {
        let score = u32::from(candidate.score)
            + u32::from(
                committed
                    .filter(|tactic| candidate.tactic == Some(*tactic))
                    .map_or(0, |_| commitment_score_bonus),
            );
        if selected_score.is_none_or(|prior| score > prior)
            || (selected_score == Some(score)
                && selected
                    .is_none_or(|prior: IntentCandidate| candidate.behavior_id < prior.behavior_id))
        {
            selected = Some(candidate);
            selected_score = Some(score);
        }
    }
    selected
}

mod pickups {
    use super::{
        BehaviorContext, BotIntent, BotTactic, CandidateBuffer, PICKUPS, distance_order,
        health_fraction,
    };

    pub(super) fn contribute(
        context: &BehaviorContext<'_>,
        candidates: &mut CandidateBuffer,
        base_score: u16,
    ) {
        let observation = context.observation;
        if health_fraction(
            observation.self_view.current_health,
            observation.self_view.maximum_health,
        ) > context.profile.retreat_health_fraction
        {
            return;
        }
        let self_position = observation.self_view.position;
        let Some(pickup) = observation
            .pickups
            .iter()
            .min_by(|a, b| distance_order(self_position, a.position, b.position))
        else {
            return;
        };
        candidates.propose(
            PICKUPS.id,
            base_score,
            Some(BotTactic::CollectPickup),
            BotIntent {
                move_goal: Some(pickup.position),
                ..Default::default()
            },
        );
    }
}

mod retreat {
    use super::{
        BehaviorContext, BotIntent, BotTactic, CandidateBuffer, RETREAT, Vec2, health_fraction,
        nearest_enemy,
    };

    pub(super) fn contribute(
        context: &BehaviorContext<'_>,
        candidates: &mut CandidateBuffer,
        base_score: u16,
    ) {
        let observation = context.observation;
        if health_fraction(
            observation.self_view.current_health,
            observation.self_view.maximum_health,
        ) > context.profile.retreat_health_fraction
        {
            return;
        }
        let self_position = observation.self_view.position;
        let Some(enemy) = nearest_enemy(observation) else {
            return;
        };
        let distance = self_position.distance(enemy.position);
        let preferred = observation.weapon_range * context.profile.preferred_range_fraction;
        let away = (self_position - enemy.position)
            .try_normalize()
            .unwrap_or(Vec2::X);
        candidates.propose(
            RETREAT.id,
            base_score,
            Some(BotTactic::Retreat),
            BotIntent {
                move_goal: Some(if distance < preferred {
                    self_position
                        + away
                            * (preferred - distance).clamp(
                                context.profile.retreat_step_min_world,
                                context.profile.retreat_step_max_world,
                            )
                } else {
                    self_position
                }),
                aim_target: Some((enemy.position, enemy.velocity)),
                fire: true,
                dash: distance < preferred * context.profile.retreat_dash_range_fraction,
            },
        );
    }
}

mod objectives {
    use super::{
        BehaviorContext, BotIntent, BotRole, BotTactic, CandidateBuffer, OBJECTIVES, Vec2,
        hot_zone_hold_point, nearest_enemy, safe_object,
    };
    use crate::matchplay::BotObjectiveView;

    pub(super) fn contribute(
        context: &BehaviorContext<'_>,
        candidates: &mut CandidateBuffer,
        base_score: u16,
    ) {
        match (context.role, context.observation.objective) {
            (BotRole::Objective, BotObjectiveView::ControlArea { center, radius }) => {
                hot_zone(context, candidates, base_score, center, radius);
            }
            (BotRole::Defender, BotObjectiveView::AttackAndDefend) => {
                defend_safe(context, candidates, base_score);
            }
            (BotRole::Objective, BotObjectiveView::AttackAndDefend) => {
                attack_safe(context, candidates, base_score);
            }
            _ => {}
        }
    }

    fn hot_zone(
        context: &BehaviorContext<'_>,
        candidates: &mut CandidateBuffer,
        base_score: u16,
        center: Vec2,
        radius: f32,
    ) {
        let observation = context.observation;
        let enemy = nearest_enemy(observation);
        let hold = hot_zone_hold_point(
            center,
            radius,
            observation.self_view.network_id.0,
            context.profile.hot_zone_hold_radius_fraction,
        );
        candidates.propose(
            OBJECTIVES.id,
            base_score,
            Some(BotTactic::Contest),
            BotIntent {
                move_goal: Some(hold),
                aim_target: enemy.map(|enemy| (enemy.position, enemy.velocity)),
                fire: enemy.is_some_and(|enemy| {
                    observation.self_view.position.distance(enemy.position)
                        <= observation.weapon_range
                }),
                dash: observation.self_view.position.distance(center)
                    > radius * context.profile.hot_zone_dash_radius_fraction,
            },
        );
    }

    fn attack_safe(
        context: &BehaviorContext<'_>,
        candidates: &mut CandidateBuffer,
        base_score: u16,
    ) {
        let observation = context.observation;
        let Some(safe) = safe_object(observation, false) else {
            return;
        };
        let self_position = observation.self_view.position;
        let distance = self_position.distance(safe.position);
        let standoff = observation.weapon_range * context.profile.preferred_range_fraction;
        let direction = (safe.position - self_position)
            .try_normalize()
            .unwrap_or(Vec2::X);
        candidates.propose(
            OBJECTIVES.id,
            base_score,
            Some(BotTactic::AttackSafe),
            BotIntent {
                move_goal: Some(
                    if distance > standoff * context.profile.standoff_arrival_fraction {
                        safe.position - direction * standoff
                    } else {
                        self_position
                    },
                ),
                aim_target: Some((safe.position, Vec2::ZERO)),
                fire: distance <= observation.weapon_range,
                dash: distance
                    > observation.weapon_range * context.profile.attack_safe_dash_range_fraction,
            },
        );
    }

    fn defend_safe(
        context: &BehaviorContext<'_>,
        candidates: &mut CandidateBuffer,
        base_score: u16,
    ) {
        let observation = context.observation;
        let Some(friendly_safe) = safe_object(observation, true) else {
            return;
        };
        let enemy = nearest_enemy(observation);
        let hostile_direction = safe_object(observation, false)
            .and_then(|safe| (safe.position - friendly_safe.position).try_normalize())
            .unwrap_or(Vec2::X);
        let anchor = friendly_safe.position
            + hostile_direction
                * (observation.weapon_range * context.profile.defend_anchor_range_fraction)
                    .min(context.profile.defend_anchor_max_distance);
        candidates.propose(
            OBJECTIVES.id,
            base_score,
            Some(BotTactic::DefendSafe),
            BotIntent {
                move_goal: Some(anchor),
                aim_target: enemy.map(|enemy| (enemy.position, enemy.velocity)),
                fire: enemy.is_some_and(|enemy| {
                    observation.self_view.position.distance(enemy.position)
                        <= observation.weapon_range
                }),
                dash: false,
            },
        );
    }
}

mod combat {
    use super::{
        BehaviorContext, BotIntent, BotTactic, CandidateBuffer, FALLBACK, HEALING, OBJECT,
        PRESSURE, Vec2, distance_order, nearest_enemy, nearest_live_object, object_attack_intent,
    };

    pub(super) fn healing(
        context: &BehaviorContext<'_>,
        candidates: &mut CandidateBuffer,
        base_score: u16,
    ) {
        let observation = context.observation;
        if !observation.healing_weapon {
            return;
        }
        let Some(ally) = observation
            .allies
            .iter()
            .filter(|ally| ally.active && ally.current_health < ally.maximum_health)
            .min_by_key(|ally| {
                u32::from(ally.current_health) * 10_000 / u32::from(ally.maximum_health.max(1))
            })
        else {
            return;
        };
        candidates.propose(
            HEALING.id,
            base_score,
            None,
            BotIntent {
                move_goal: Some(ally.position),
                aim_target: Some((ally.position, ally.velocity)),
                fire: observation.self_view.position.distance(ally.position)
                    <= observation.weapon_range,
                dash: false,
            },
        );
    }

    pub(super) fn pressure(
        context: &BehaviorContext<'_>,
        candidates: &mut CandidateBuffer,
        base_score: u16,
    ) {
        let observation = context.observation;
        let Some(enemy) = nearest_enemy(observation) else {
            return;
        };
        let self_position = observation.self_view.position;
        let distance = self_position.distance(enemy.position);
        let preferred = observation.weapon_range * context.profile.preferred_range_fraction;
        let direction = (enemy.position - self_position)
            .try_normalize()
            .unwrap_or(Vec2::X);
        let move_goal = if distance > preferred * context.profile.pressure_far_range_fraction {
            enemy.position - direction * preferred
        } else if distance < preferred * context.profile.pressure_near_range_fraction {
            self_position - direction * preferred * context.profile.pressure_retreat_fraction
        } else {
            let strafe = Vec2::new(-direction.y, direction.x)
                * if observation.self_view.network_id.0 & 1 == 0 {
                    1.0
                } else {
                    -1.0
                };
            self_position + strafe * context.profile.pressure_strafe_distance
        };
        candidates.propose(
            PRESSURE.id,
            base_score,
            Some(BotTactic::Pressure),
            BotIntent {
                move_goal: Some(move_goal),
                aim_target: Some((enemy.position, enemy.velocity)),
                fire: distance <= observation.weapon_range,
                dash: distance
                    > observation.weapon_range * context.profile.pressure_dash_range_fraction,
            },
        );
    }

    pub(super) fn object(
        context: &BehaviorContext<'_>,
        candidates: &mut CandidateBuffer,
        base_score: u16,
    ) {
        let observation = context.observation;
        if !observation.objects.iter().any(|object| object.live) {
            return;
        }
        let self_position = observation.self_view.position;
        let goal = context
            .state
            .contacts
            .iter()
            .min_by(|a, b| distance_order(self_position, a.position, b.position))
            .map(|contact| contact.position)
            .or_else(|| nearest_live_object(observation).map(|object| object.position));
        let object = goal.and_then(|goal| {
            observation.objects.iter().find(|object| {
                object.live
                    && goal.distance_squared(object.position) < 1.0
                    && (object.hazardous || object.valuable || object.defending_team.is_some())
            })
        });
        let intent = object.map_or(
            BotIntent {
                move_goal: goal,
                ..Default::default()
            },
            |object| object_attack_intent(observation, context.profile, object.position),
        );
        candidates.propose(OBJECT.id, base_score, Some(BotTactic::BreakObject), intent);
    }

    pub(super) fn fallback(
        context: &BehaviorContext<'_>,
        candidates: &mut CandidateBuffer,
        base_score: u16,
    ) {
        let observation = context.observation;
        let self_position = observation.self_view.position;
        let goal = context
            .state
            .contacts
            .iter()
            .min_by(|a, b| distance_order(self_position, a.position, b.position))
            .map(|contact| contact.position)
            .or_else(|| nearest_live_object(observation).map(|object| object.position))
            .or(Some(Vec2::ZERO));
        candidates.propose(
            FALLBACK.id,
            base_score,
            Some(BotTactic::Pressure),
            BotIntent {
                move_goal: goal,
                ..Default::default()
            },
        );
    }
}

fn object_attack_intent(
    observation: &BotObservation,
    profile: BotProfile,
    position: Vec2,
) -> BotIntent {
    let self_position = observation.self_view.position;
    let distance = self_position.distance(position);
    let standoff = observation.weapon_range * profile.preferred_range_fraction;
    let direction = (position - self_position)
        .try_normalize()
        .unwrap_or(Vec2::X);
    BotIntent {
        move_goal: Some(if distance > standoff * profile.standoff_arrival_fraction {
            position - direction * standoff
        } else {
            self_position
        }),
        aim_target: Some((position, Vec2::ZERO)),
        fire: distance <= observation.weapon_range,
        dash: false,
    }
}

fn nearest_enemy(observation: &BotObservation) -> Option<&super::model::BotFighterView> {
    let self_position = observation.self_view.position;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.active)
        .min_by(|a, b| distance_order(self_position, a.position, b.position))
}

fn nearest_live_object(observation: &BotObservation) -> Option<&super::model::BotObjectView> {
    let self_position = observation.self_view.position;
    observation
        .objects
        .iter()
        .filter(|object| object.live)
        .min_by(|a, b| distance_order(self_position, a.position, b.position))
}

fn safe_object(
    observation: &BotObservation,
    friendly: bool,
) -> Option<&super::model::BotObjectView> {
    observation.objects.iter().find(|object| {
        object.live
            && object.defending_team.is_some_and(|defending_team| {
                (defending_team == observation.self_view.team) == friendly
            })
    })
}

fn hot_zone_hold_point(
    center: Vec2,
    radius: f32,
    stable_id: u64,
    hold_radius_fraction: f32,
) -> Vec2 {
    let directions = [Vec2::X, Vec2::Y, Vec2::NEG_X, Vec2::NEG_Y];
    let index = usize::try_from(stable_id % directions.len() as u64).unwrap_or(0);
    center + directions[index] * radius * hold_radius_fraction
}

fn health_fraction(current: u16, maximum: u16) -> f32 {
    f32::from(current) / f32::from(maximum.max(1))
}

fn distance_order(origin: Vec2, a: Vec2, b: Vec2) -> std::cmp::Ordering {
    origin
        .distance_squared(a)
        .total_cmp(&origin.distance_squared(b))
        .then_with(|| a.x.total_cmp(&b.x))
        .then_with(|| a.y.total_cmp(&b.y))
}

#[cfg(test)]
mod tests {
    use super::super::{profile::BotCatalogResource, registry::BotBehaviorRegistryPlugin};
    use super::*;

    const TEST_BEHAVIOR: BehaviorRegistration =
        BehaviorRegistration::new(BotBehaviorId(77), contribute_test_behavior);

    struct TestBehaviorPlugin;

    impl Plugin for TestBehaviorPlugin {
        fn build(&self, app: &mut App) {
            app.try_register_bot_behavior(TEST_BEHAVIOR).unwrap();
        }
    }

    fn contribute_test_behavior(
        _context: &BehaviorContext<'_>,
        candidates: &mut CandidateBuffer,
        base_score: u16,
    ) {
        candidates.propose(
            TEST_BEHAVIOR.id,
            base_score,
            Some(BotTactic::Contest),
            BotIntent {
                move_goal: Some(Vec2::new(7.0, 7.0)),
                ..Default::default()
            },
        );
    }

    fn observation() -> BotObservation {
        BotObservation {
            tick: 1,
            match_id: crate::matchplay::MatchId(1),
            map_instance_id: crate::map::MapInstanceId(1),
            map_generation: crate::map::MapDynamicGeneration {
                map_instance_id: crate::map::MapInstanceId(1),
                generation: 1,
            },
            map_revision: 1,
            match_active: true,
            self_view: super::super::model::BotFighterView {
                network_id: crate::protocol::NetworkEntityId(1),
                team: crate::combat::TeamId(0),
                position: Vec2::ZERO,
                velocity: Vec2::ZERO,
                current_health: 100,
                maximum_health: 100,
                active: true,
                cold_meter: 0,
                frozen: false,
                poisoned: false,
                burning: false,
            },
            allies: Vec::new(),
            visible_enemies: Vec::new(),
            objects: Vec::new(),
            pickups: Vec::new(),
            objective: crate::matchplay::BotObjectiveView::Elimination,
            weapon_phase: crate::combat::WeaponPhase::Ready,
            weapon_ammo: 1,
            ability_ready: false,
            ultimate_kind: crate::builds::UltimateKind::Dash,
            ultimate_range: 100.0,
            weapon_range: 100.0,
            projectile_speed: 100.0,
            healing_weapon: false,
        }
    }

    #[test]
    fn arbiter_prefers_score_then_lower_stable_behavior_id() {
        let intent = BotIntent {
            move_goal: Some(Vec2::X),
            ..Default::default()
        };
        let candidates = [
            IntentCandidate::new(BotBehaviorId(9), 20, Some(BotTactic::Pressure), intent),
            IntentCandidate::new(BotBehaviorId(3), 20, Some(BotTactic::Retreat), intent),
            IntentCandidate::new(BotBehaviorId(1), 10, Some(BotTactic::Contest), intent),
        ];
        assert_eq!(
            arbitrate(&candidates, None, 1_000).unwrap().behavior_id,
            BotBehaviorId(3)
        );
        assert_eq!(
            arbitrate(&candidates, Some(BotTactic::Contest), 1_000)
                .unwrap()
                .behavior_id,
            BotBehaviorId(1)
        );
    }

    #[test]
    fn candidate_buffer_is_bounded() {
        let mut candidates = CandidateBuffer::default();
        for score in 0..MAX_INTENT_CANDIDATES + 3 {
            candidates.propose(
                BotBehaviorId(99),
                u16::try_from(score).unwrap(),
                Some(BotTactic::Pressure),
                BotIntent::default(),
            );
        }
        assert_eq!(candidates.candidates.len(), MAX_INTENT_CANDIDATES);
    }

    #[test]
    fn plugin_registered_eighth_behavior_participates_without_arbiter_changes() {
        let mut catalog = super::super::profile::BotCatalog::embedded().unwrap();
        catalog
            .arbitration
            .behaviors
            .push(super::super::profile::BotBehaviorPolicy {
                id: TEST_BEHAVIOR.id,
                enabled: true,
                base_score: 777,
            });
        catalog.validate().unwrap();

        let mut app = App::new();
        app.add_plugins((
            BotBehaviorRegistryPlugin,
            BuiltInBotBehaviorsPlugin,
            TestBehaviorPlugin,
        ));
        app.insert_resource(BotCatalogResource(catalog.clone()));
        crate::test_app::finalize(&mut app);

        let registry = app.world().resource::<BotBehaviorRegistry>();
        assert_eq!(registry.registrations().len(), MAX_INTENT_CANDIDATES);
        assert!(
            registry
                .registrations()
                .windows(2)
                .all(|pair| pair[0].id < pair[1].id)
        );

        let observation = observation();
        let state = BotState::default();
        let context = BehaviorContext {
            observation: &observation,
            state: &state,
            profile: BotProfile::embedded().unwrap(),
            role: BotRole::Pressure,
        };
        let candidates =
            collect_candidates(&context, registry.registrations(), &catalog.arbitration);
        let selected = arbitrate(
            &candidates.candidates,
            None,
            catalog.arbitration.commitment_score_bonus,
        )
        .unwrap();
        assert_eq!(selected.behavior_id, BotBehaviorId(77));
        assert_eq!(selected.intent.move_goal, Some(Vec2::new(7.0, 7.0)));
    }

    fn arbitration_observation() -> BotObservation {
        let mut observation = observation();
        observation.objective = crate::matchplay::BotObjectiveView::ControlArea {
            center: Vec2::ZERO,
            radius: 160.0,
        };
        observation
            .visible_enemies
            .push(super::super::model::BotFighterView {
                network_id: crate::protocol::NetworkEntityId(2),
                team: crate::combat::TeamId(1),
                position: Vec2::new(80.0, 0.0),
                velocity: Vec2::ZERO,
                current_health: 100,
                maximum_health: 100,
                active: true,
                cold_meter: 0,
                frozen: false,
                poisoned: false,
                burning: false,
            });
        observation
    }

    #[test]
    fn authored_scores_enablement_commitment_and_stable_ties_drive_arbitration() {
        let observation = arbitration_observation();
        let state = BotState::default();
        let context = BehaviorContext {
            observation: &observation,
            state: &state,
            profile: BotProfile::embedded().unwrap(),
            role: BotRole::Objective,
        };
        let mut policy = super::super::profile::BotCatalog::embedded()
            .unwrap()
            .arbitration;

        let candidates = collect_candidates(&context, built_in_registry().registrations(), &policy);
        assert_eq!(
            arbitrate(&candidates.candidates, None, policy.commitment_score_bonus)
                .unwrap()
                .behavior_id,
            BotBehaviorId::OBJECTIVES
        );

        policy
            .behaviors
            .iter_mut()
            .find(|behavior| behavior.id == BotBehaviorId::PRESSURE)
            .unwrap()
            .base_score = 701;
        let candidates = collect_candidates(&context, built_in_registry().registrations(), &policy);
        assert_eq!(
            arbitrate(&candidates.candidates, None, policy.commitment_score_bonus)
                .unwrap()
                .behavior_id,
            BotBehaviorId::PRESSURE
        );

        let pressure = policy
            .behaviors
            .iter_mut()
            .find(|behavior| behavior.id == BotBehaviorId::PRESSURE)
            .unwrap();
        pressure.base_score = 700;
        policy
            .behaviors
            .iter_mut()
            .find(|behavior| behavior.id == BotBehaviorId::OBJECTIVES)
            .unwrap()
            .enabled = false;
        let candidates = collect_candidates(&context, built_in_registry().registrations(), &policy);
        assert_eq!(
            arbitrate(&candidates.candidates, None, policy.commitment_score_bonus)
                .unwrap()
                .behavior_id,
            BotBehaviorId::PRESSURE
        );

        policy
            .behaviors
            .iter_mut()
            .find(|behavior| behavior.id == BotBehaviorId::OBJECTIVES)
            .unwrap()
            .enabled = true;
        let candidates = collect_candidates(&context, built_in_registry().registrations(), &policy);
        assert_eq!(
            arbitrate(&candidates.candidates, None, policy.commitment_score_bonus)
                .unwrap()
                .behavior_id,
            BotBehaviorId::PRESSURE,
            "equal authored scores retain the lower stable behavior-ID tie-break"
        );

        policy
            .behaviors
            .iter_mut()
            .find(|behavior| behavior.id == BotBehaviorId::PRESSURE)
            .unwrap()
            .base_score = 600;
        policy.commitment_score_bonus = 101;
        let candidates = collect_candidates(&context, built_in_registry().registrations(), &policy);
        assert_eq!(
            arbitrate(
                &candidates.candidates,
                Some(BotTactic::Pressure),
                policy.commitment_score_bonus,
            )
            .unwrap()
            .behavior_id,
            BotBehaviorId::PRESSURE
        );
        policy.commitment_score_bonus = 0;
        assert_eq!(
            arbitrate(
                &candidates.candidates,
                Some(BotTactic::Pressure),
                policy.commitment_score_bonus,
            )
            .unwrap()
            .behavior_id,
            BotBehaviorId::OBJECTIVES
        );
    }
}
