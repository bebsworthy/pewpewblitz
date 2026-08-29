use super::{
    diagnostics::{BotDecisionTrace, BotDiagnostics},
    model::{
        BotFighterView, BotModeView, BotObjectKind, BotObjectView, BotObservation, BotPickupView,
        PracticeBotController,
    },
    navigation::BotNavigationSnapshot,
    policy,
    profile::BotProfile,
    team::{BotPlanMember, assign_roles},
};
use crate::{
    builds::{AbilityPhase, AbilityState, ResolvedMatchLoadout},
    combat::{CurrentHealth, Defeated, TeamId, WeaponState},
    concealment::{
        ConcealmentPresentationState, ConcealmentSources, ObserverRelation,
        ObserverVisibilityInput, observer_can_see,
    },
    gameplay::GameplaySet,
    map::{
        DamageableLifeState, DamageableMaximumHealth, DamageableObjectAsset,
        DamageableTargetIdentity, MapDynamicState, OIL_BARREL_ASSET, ResolvedMap,
        RestorationPickup, TREASURE_CHEST_ASSET,
    },
    matchplay::{
        ActiveCombatant, HeistSafe, HeistState, HotZoneState, MatchPhase, MatchRoot, MatchState,
        WipeoutState,
    },
    movement::{InputFreshness, MovementTuning, decoded_input_is_valid},
    protocol::{Fighter, FighterInput, NetworkEntityId},
    timing::SimulationTick,
};
use avian2d::prelude::{LinearVelocity, Position};
use bevy::prelude::*;
use lightyear::prelude::input::native::ActionState;

#[derive(Resource, Default)]
struct BotNavigationRuntime {
    map_instance_id: Option<crate::map::MapInstanceId>,
    snapshot: Option<BotNavigationSnapshot>,
}

pub(crate) fn install_controller_systems(app: &mut App) {
    let profile = BotProfile::default();
    assert!(
        profile.validate(),
        "the built-in Practice bot profile is valid"
    );
    app.init_resource::<BotNavigationRuntime>()
        .init_resource::<BotDiagnostics>()
        .add_systems(
            FixedUpdate,
            (ApplyDeferred, capture_observations)
                .chain()
                .after(GameplaySet::Lifecycle)
                .before(GameplaySet::Input),
        )
        .add_systems(
            FixedUpdate,
            decide_and_commit_inputs.in_set(GameplaySet::Input),
        );
}

#[derive(Clone)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the private capture record mirrors independent authoritative visibility and condition facts"
)]
struct RawFighterView {
    network_id: NetworkEntityId,
    team: TeamId,
    position: Vec2,
    velocity: Vec2,
    current_health: u16,
    maximum_health: u16,
    reveal_radius: f32,
    active: bool,
    cold_meter: u16,
    frozen: bool,
    poisoned: bool,
    burning: bool,
    concealment: ConcealmentSources,
    forced_reveals: Vec<crate::concealment::TeamRevealDeadline>,
    reveal_locked: bool,
    loadout: ResolvedMatchLoadout,
    weapon: WeaponState,
    ability: AbilityState,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    reason = "the Practice adapter owns one explicit authoritative observation allowlist"
)]
fn capture_observations(
    tick: Res<SimulationTick>,
    map: Res<ResolvedMap>,
    movement: Res<MovementTuning>,
    dynamic_states: Query<&MapDynamicState>,
    roots: Query<
        (
            &MatchState,
            Option<&WipeoutState>,
            Option<&HotZoneState>,
            Has<HeistState>,
        ),
        With<MatchRoot>,
    >,
    mut fighters: ParamSet<(
        Query<
            (
                &NetworkEntityId,
                &TeamId,
                &Position,
                &LinearVelocity,
                &CurrentHealth,
                &ResolvedMatchLoadout,
                &WeaponState,
                &AbilityState,
                Has<ActiveCombatant>,
                Has<Defeated>,
                Option<&ConcealmentPresentationState>,
                &crate::combat::ActiveEffects,
            ),
            With<Fighter>,
        >,
        Query<(&NetworkEntityId, &mut PracticeBotController)>,
    )>,
    objects: Query<(
        &DamageableTargetIdentity,
        &Position,
        &CurrentHealth,
        &DamageableMaximumHealth,
        Option<&DamageableObjectAsset>,
        Option<&DamageableLifeState>,
        Option<&HeistSafe>,
    )>,
    pickups: Query<&Position, With<RestorationPickup>>,
    mut navigation: ResMut<BotNavigationRuntime>,
) {
    let Ok((match_state, wipeout, hot_zone, heist)) = roots.single() else {
        return;
    };
    let Ok(dynamic) = dynamic_states.single() else {
        return;
    };
    if navigation.map_instance_id != Some(map.snapshot.identity.instance_id) {
        navigation.snapshot = BotNavigationSnapshot::from_map(&map, movement.radius + 1.0);
        navigation.map_instance_id = Some(map.snapshot.identity.instance_id);
    }
    let Some(_navigation) = navigation.snapshot.as_ref() else {
        return;
    };

    let mut raw_fighters: Vec<_> = fighters
        .p0()
        .iter()
        .map(
            |(
                network_id,
                team,
                position,
                velocity,
                health,
                loadout,
                weapon,
                ability,
                active,
                defeated,
                concealment,
                effects,
            )| {
                let presentation = concealment.cloned().unwrap_or_default();
                RawFighterView {
                    network_id: *network_id,
                    team: *team,
                    position: position.0,
                    velocity: velocity.0,
                    current_health: health.0,
                    maximum_health: loadout.fighter_stats.maximum_health,
                    reveal_radius: loadout.fighter_stats.reveal_proximity_radius,
                    active: active && !defeated,
                    cold_meter: effects.cold.meter,
                    frozen: effects.is_frozen(tick.0),
                    poisoned: effects.is_poisoned(tick.0),
                    burning: effects
                        .fire
                        .is_some_and(|fire| tick.0 <= fire.expires_at_tick),
                    concealment: ConcealmentSources {
                        terrain: presentation.inside_concealing_terrain,
                        self_cloak: tick.0 < presentation.self_cloaked_until_tick,
                        allied_field: presentation.inside_allied_concealment_field,
                    },
                    forced_reveals: presentation.forced_reveals,
                    reveal_locked: tick.0 < presentation.revealed_until_tick,
                    loadout: loadout.clone(),
                    weapon: *weapon,
                    ability: *ability,
                }
            },
        )
        .collect();
    raw_fighters.sort_by_key(|fighter| fighter.network_id);
    let mut object_views: Vec<_> = objects
        .iter()
        .filter_map(|(identity, position, health, maximum, asset, life, safe)| {
            let kind = if let Some(safe) = safe {
                BotObjectKind::HeistSafe {
                    defending_team: safe.defending_team,
                }
            } else {
                match asset?.0 {
                    OIL_BARREL_ASSET => BotObjectKind::OilBarrel,
                    TREASURE_CHEST_ASSET => BotObjectKind::TreasureChest,
                    _ => return None,
                }
            };
            Some(BotObjectView {
                identity: *identity,
                kind,
                position: position.0,
                current_health: health.0,
                maximum_health: maximum.0,
                live: life.is_none_or(|life| *life == DamageableLifeState::Live) && health.0 > 0,
            })
        })
        .collect();
    object_views.sort_by_key(|object| object.identity.stable_order_key());
    let mut pickup_views: Vec<_> = pickups
        .iter()
        .map(|position| BotPickupView {
            position: position.0,
        })
        .collect();
    pickup_views.sort_by(|a, b| {
        a.position
            .x
            .total_cmp(&b.position.x)
            .then_with(|| a.position.y.total_cmp(&b.position.y))
    });
    let mode = if let Some(state) = wipeout {
        BotModeView::Wipeout {
            scores: state.team_scores,
        }
    } else if let Some(state) = hot_zone {
        let Some(zone) = map.objective_zone else {
            return;
        };
        let crate::map::MapShape::Circle { radius } = zone.area.shape else {
            return;
        };
        BotModeView::HotZone {
            center: zone.area.center,
            radius,
            status: state.status,
            progress: state.progress_ticks,
        }
    } else if heist {
        BotModeView::Heist
    } else {
        return;
    };
    let match_active = matches!(match_state.phase, MatchPhase::Active { .. });

    for (network_id, mut controller) in &mut fighters.p1() {
        let Some(observer) = raw_fighters
            .iter()
            .find(|fighter| fighter.network_id == *network_id)
        else {
            continue;
        };
        if observer.active && !controller.was_active {
            controller.reset_life();
        }
        controller.was_active = observer.active;
        if controller.history.back().is_some_and(|prior| {
            prior.match_id != match_state.match_id
                || prior.map_generation != dynamic.generation_id()
                || prior.map_instance_id != map.snapshot.identity.instance_id
        }) {
            controller.reset_context();
        }
        let self_view = public_fighter_view(observer);
        let mut allies = Vec::new();
        let mut visible_enemies = Vec::new();
        for subject in &raw_fighters {
            if subject.network_id == observer.network_id {
                continue;
            }
            if subject.team == observer.team {
                allies.push(public_fighter_view(subject));
                continue;
            }
            if observer_can_see(ObserverVisibilityInput {
                relation: ObserverRelation::Enemy,
                observer_alive: observer.active,
                concealment: subject.concealment,
                forced_revealed: subject
                    .forced_reveals
                    .iter()
                    .any(|reveal| reveal.team == observer.team && tick.0 < reveal.expires_at_tick),
                subject_reveal_locked: subject.reveal_locked,
                distance_squared: observer.position.distance_squared(subject.position),
                reveal_radius: observer.reveal_radius,
            }) {
                visible_enemies.push(public_fighter_view(subject));
            }
        }
        let (weapon_range, projectile_speed) = weapon_capabilities(&observer.loadout);
        let ultimate_range = match observer.loadout.ultimate.parameters {
            crate::builds::UltimateParameters::DemolitionStrike {
                maximum_range_milliunits,
                ..
            }
            | crate::builds::UltimateParameters::ElementalField {
                maximum_range_milliunits,
                ..
            }
            | crate::builds::UltimateParameters::BigBlob {
                maximum_range_milliunits,
                ..
            } => {
                crate::builds::world_units_from_milliunits(maximum_range_milliunits).unwrap_or(0.0)
            }
            _ => 0.0,
        };
        controller.push_observation(BotObservation {
            tick: tick.0,
            match_id: match_state.match_id,
            map_instance_id: map.snapshot.identity.instance_id,
            map_generation: dynamic.generation_id(),
            map_revision: dynamic.revision,
            match_active,
            self_view,
            allies,
            visible_enemies,
            objects: object_views.clone(),
            pickups: pickup_views.clone(),
            mode,
            weapon_phase: observer.weapon.phase,
            weapon_ammo: observer.weapon.ammo,
            ability_ready: matches!(observer.ability.phase, AbilityPhase::Ready),
            ultimate_kind: observer.loadout.ultimate.kind,
            ultimate_range,
            weapon_range,
            projectile_speed,
            healing_weapon: observer
                .loadout
                .primary_weapon
                .recipe
                .payload_bundles
                .iter()
                .any(|bundle| {
                    bundle.effects.iter().any(|effect| {
                        matches!(effect, crate::combat::PayloadEffectDefinition::Heal { .. })
                    })
                }),
        });
    }
}

fn public_fighter_view(raw: &RawFighterView) -> BotFighterView {
    BotFighterView {
        network_id: raw.network_id,
        team: raw.team,
        position: raw.position,
        velocity: raw.velocity,
        current_health: raw.current_health,
        maximum_health: raw.maximum_health,
        active: raw.active,
        cold_meter: raw.cold_meter,
        frozen: raw.frozen,
        poisoned: raw.poisoned,
        burning: raw.burning,
    }
}

fn weapon_capabilities(loadout: &ResolvedMatchLoadout) -> (f32, f32) {
    match loadout.primary_weapon.recipe.delivery {
        crate::combat::DeliveryMethod::Straight { speed, range, .. }
        | crate::combat::DeliveryMethod::StickyStraight { speed, range, .. } => (range, speed),
        crate::combat::DeliveryMethod::Lobbed { distance, .. } => (distance, 0.0),
        crate::combat::DeliveryMethod::MeleeArc { reach, .. }
        | crate::combat::DeliveryMethod::ConeSpray { reach, .. } => (reach, 0.0),
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "the controller atomically owns its local input and freshness commit"
)]
fn decide_and_commit_inputs(
    tick: Res<SimulationTick>,
    navigation: Res<BotNavigationRuntime>,
    roots: Query<&MatchState, With<MatchRoot>>,
    mut diagnostics: ResMut<BotDiagnostics>,
    mut bots: Query<(
        &NetworkEntityId,
        &mut PracticeBotController,
        &mut ActionState<FighterInput>,
        &mut InputFreshness,
        Has<ActiveCombatant>,
        Has<Defeated>,
    )>,
) {
    let started_at = std::time::Instant::now();
    let profile = BotProfile::default();
    let match_active = roots
        .single()
        .is_ok_and(|state| matches!(state.phase, MatchPhase::Active { .. }));
    let plan_members = bots
        .iter_mut()
        .filter_map(|(network_id, controller, ..)| {
            controller
                .delayed_observation(tick.0, profile.reaction_ticks)
                .map(|observation| BotPlanMember {
                    network_id: *network_id,
                    team: observation.self_view.team,
                    mode: observation.mode,
                })
        })
        .collect::<Vec<_>>();
    let roles = assign_roles(&plan_members);
    let per_bot_search_budget = profile.search_budget_per_bot(plan_members.len());
    for (network_id, mut controller, mut action, mut freshness, active, defeated) in &mut bots {
        let role = roles.get(network_id).copied().unwrap_or_default();
        let decision = (match_active && active && !defeated)
            .then_some(())
            .and_then(|()| navigation.snapshot.as_ref())
            .and_then(|navigation| {
                controller
                    .delayed_observation(tick.0, profile.reaction_ticks)
                    .cloned()
                    .map(|observation| (navigation, observation))
            })
            .filter(|(_, observation)| observation.match_active && observation.self_view.active)
            .map_or_else(policy::BotDecision::default, |(navigation, observation)| {
                let seed = controller.seed ^ controller.life_generation.rotate_left(17);
                policy::decide(
                    &observation,
                    &mut controller.state,
                    profile,
                    navigation,
                    seed,
                    role,
                    per_bot_search_budget,
                )
            });
        diagnostics.record_navigation(decision.navigation);
        let input = if decoded_input_is_valid(decision.input) {
            decision.input
        } else {
            diagnostics.record_invalid_output();
            FighterInput::default()
        };
        action.0 = input;
        freshness.last_fresh_tick = Some(tick.0);
        controller.last_decision_tick = Some(tick.0);
        diagnostics.record(BotDecisionTrace {
            tick: tick.0,
            network_id: *network_id,
            role,
            tactic: controller.state.tactic,
            input,
        });
    }
    diagnostics.record_controller_duration(started_at.elapsed());
}
