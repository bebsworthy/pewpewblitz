//! Stationary propagating cone-spray authority.

#![allow(
    clippy::wildcard_imports,
    reason = "the server-owned spray transaction consumes the combat composition surface"
)]

use super::*;
use avian2d::prelude::Position;
use bevy::prelude::*;
use std::cmp::Ordering;
use std::collections::HashSet;

#[derive(Clone, Copy)]
enum SprayCandidate {
    Combatant {
        entity: Entity,
        position: Vec2,
        team: TeamId,
        network_id: NetworkEntityId,
    },
    World {
        position: Vec2,
        identity: crate::map::DamageableTargetIdentity,
    },
}

impl SprayCandidate {
    fn position(self) -> Vec2 {
        match self {
            Self::Combatant { position, .. } | Self::World { position, .. } => position,
        }
    }

    fn stable_cmp(self, other: Self) -> Ordering {
        match (self, other) {
            (
                Self::Combatant {
                    network_id: left, ..
                },
                Self::Combatant {
                    network_id: right, ..
                },
            ) => left.cmp(&right),
            (Self::Combatant { .. }, Self::World { .. }) => Ordering::Less,
            (Self::World { .. }, Self::Combatant { .. }) => Ordering::Greater,
            (
                Self::World { identity: left, .. },
                Self::World {
                    identity: right, ..
                },
            ) => left.stable_order_key().cmp(&right.stable_order_key()),
        }
    }
}

fn settle_unresolved_spray(trackers: &mut ActiveAttackTrackers, attack_id: AttackId) {
    while trackers.active.contains_key(&attack_id) {
        finish_attack_delivery(trackers, attack_id);
    }
}

#[allow(
    clippy::type_complexity,
    reason = "the ownership check shares the authoritative fighter/sentry lifecycle view"
)]
fn spray_owner_is_valid(
    runtime: &ConeSprayRuntime,
    disconnected: &HashSet<Entity>,
    fighters: &Query<
        (
            Entity,
            &Position,
            &TeamId,
            &NetworkEntityId,
            Option<&Defeated>,
            Option<&lightyear::prelude::ControlledBy>,
            Has<crate::matchplay::MatchParticipant>,
            Has<crate::matchplay::ActiveCombatant>,
        ),
        Or<(With<Fighter>, With<crate::abilities::Sentry>)>,
    >,
) -> bool {
    fighters.get(runtime.owner_entity).is_ok_and(
        |(_, _, _, network_id, defeated, controlled, participant, active)| {
            *network_id == runtime.source.owner_network_entity_id
                && defeated.is_none()
                && (!participant || active)
                && controlled.is_none_or(|controlled| !disconnected.contains(&controlled.owner))
        },
    )
}

fn spray_match_is_valid(
    runtime: &ConeSprayRuntime,
    root: Option<&crate::matchplay::MatchState>,
) -> bool {
    runtime.match_id.is_none_or(|match_id| {
        root.is_some_and(|root| {
            root.match_id == match_id
                && !matches!(root.phase, crate::matchplay::MatchPhase::Completed { .. })
        })
    })
}

#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "candidate collection declares the complete authoritative combatant, world, and map-occlusion view"
)]
fn spray_candidates(
    state: ConeSprayState,
    source: AttackSource,
    recipe: &WeaponRecipe,
    reached_distance: f32,
    fighter_radius: f32,
    disconnected: &HashSet<Entity>,
    fighters: &Query<
        (
            Entity,
            &Position,
            &TeamId,
            &NetworkEntityId,
            Option<&Defeated>,
            Option<&lightyear::prelude::ControlledBy>,
            Has<crate::matchplay::MatchParticipant>,
            Has<crate::matchplay::ActiveCombatant>,
        ),
        Or<(With<Fighter>, With<crate::abilities::Sentry>)>,
    >,
    objects: &Query<
        (
            Entity,
            &Position,
            &crate::map::DamageableTargetIdentity,
            &CurrentHealth,
            &crate::map::DamageableLifeState,
        ),
        Or<(
            With<crate::map::DamageableWorldObject>,
            With<crate::matchplay::HeistSafe>,
        )>,
    >,
    spatial_query: &avian2d::prelude::SpatialQuery,
) -> Vec<SprayCandidate> {
    let origin = state.origin.as_vec2();
    let has_direct_damage = recipe.payload_bundles.iter().any(|bundle| {
        matches!(bundle.target, TargetSelection::Direct)
            && bundle
                .effects
                .iter()
                .any(|effect| matches!(effect, PayloadEffectDefinition::Damage { .. }))
    });
    let mut candidates = Vec::new();
    for (entity, position, team, network_id, defeated, controlled, participant, active) in fighters
    {
        if defeated.is_some()
            || (participant && !active)
            || controlled.is_some_and(|controlled| disconnected.contains(&controlled.owner))
            || !payload_target_visible(source, *team, *network_id)
            || !recipe.payload_bundles.iter().any(|bundle| {
                matches!(bundle.target, TargetSelection::Direct)
                    && payload_can_affect_target(bundle, source, *team, *network_id)
            })
            || !delivery::sector_contains(
                origin,
                state.facing,
                reached_distance,
                state.angle_degrees,
                position.0,
                fighter_radius,
            )
            || (state.map_occlusion && !area_line_of_sight_clear(origin, position.0, spatial_query))
        {
            continue;
        }
        candidates.push(SprayCandidate::Combatant {
            entity,
            position: position.0,
            team: *team,
            network_id: *network_id,
        });
    }
    if has_direct_damage {
        for (entity, position, identity, health, life) in objects {
            if !crate::map::object_is_live(*health, *life)
                || !delivery::sector_contains(
                    origin,
                    state.facing,
                    reached_distance,
                    state.angle_degrees,
                    position.0,
                    16.0,
                )
                || (state.map_occlusion
                    && !area_line_of_sight_clear_excluding(
                        origin,
                        position.0,
                        entity,
                        spatial_query,
                    ))
            {
                continue;
            }
            candidates.push(SprayCandidate::World {
                position: position.0,
                identity: *identity,
            });
        }
    }
    candidates.sort_by(|left, right| {
        origin
            .distance_squared(left.position())
            .total_cmp(&origin.distance_squared(right.position()))
            .then_with(|| left.stable_cmp(*right))
    });
    candidates.truncate(usize::from(state.max_targets));
    candidates
}

#[allow(clippy::too_many_arguments)]
fn queue_spray_candidate(
    candidate: SprayCandidate,
    source: AttackSource,
    recipe: &WeaponRecipe,
    delivery_index: u8,
    origin: Vec2,
    pending: &mut MessageWriter<PendingPayload>,
    world_pending: &mut crate::map::PendingWorldTargetDamages,
    objective_pending: &mut crate::matchplay::PendingModeObjectiveDamages,
) {
    let position = candidate.position();
    let distance = origin.distance(position);
    match candidate {
        SprayCandidate::Combatant {
            entity,
            team,
            network_id,
            ..
        } => {
            for (bundle_index, bundle) in
                recipe
                    .payload_bundles
                    .iter()
                    .enumerate()
                    .filter(|(_, bundle)| {
                        matches!(bundle.target, TargetSelection::Direct)
                            && payload_can_affect_target(bundle, source, team, network_id)
                    })
            {
                pending.write(PendingPayload {
                    source,
                    delivery_index,
                    bundle_index: u8::try_from(bundle_index).unwrap_or(u8::MAX),
                    target: entity,
                    target_network_id: network_id,
                    position,
                    engagement_distance: distance,
                    delivery_travel: distance,
                    contact_fraction: 1.0,
                    bundle: bundle.clone(),
                });
            }
        }
        SprayCandidate::World { identity, .. } => {
            for (bundle_index, bundle) in recipe
                .payload_bundles
                .iter()
                .enumerate()
                .filter(|(_, bundle)| matches!(bundle.target, TargetSelection::Direct))
            {
                for (effect_index, effect) in bundle.effects.iter().enumerate() {
                    let PayloadEffectDefinition::Damage {
                        amount, falloff, ..
                    } = *effect
                    else {
                        continue;
                    };
                    delivery::queue_damageable_target(
                        world_pending,
                        objective_pending,
                        crate::map::PendingWorldTargetDamage {
                            target: identity,
                            source,
                            attack_id: source.attack_id,
                            requested_damage: effects::requested_damage(
                                amount, falloff, distance, 1.0, false, distance,
                            ),
                            delivery_index,
                            bundle_index: u8::try_from(bundle_index).unwrap_or(u8::MAX),
                            effect_index: u8::try_from(effect_index).unwrap_or(u8::MAX),
                        },
                    );
                }
            }
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "the fixed-tick spray system owns bounded lifecycle, propagation, candidate ordering, and payload emission"
)]
pub(super) fn advance_cone_sprays(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    tuning: Res<MovementTuning>,
    mut trackers: ResMut<ActiveAttackTrackers>,
    mut pending: MessageWriter<PendingPayload>,
    mut deliveries: MessageWriter<PendingDelivery>,
    mut world_pending: ResMut<crate::map::PendingWorldTargetDamages>,
    mut objective_pending: ResMut<crate::matchplay::PendingModeObjectiveDamages>,
    disconnected: Query<Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
    roots: Query<&crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>,
    fighters: Query<
        (
            Entity,
            &Position,
            &TeamId,
            &NetworkEntityId,
            Option<&Defeated>,
            Option<&lightyear::prelude::ControlledBy>,
            Has<crate::matchplay::MatchParticipant>,
            Has<crate::matchplay::ActiveCombatant>,
        ),
        Or<(With<Fighter>, With<crate::abilities::Sentry>)>,
    >,
    objects: Query<
        (
            Entity,
            &Position,
            &crate::map::DamageableTargetIdentity,
            &CurrentHealth,
            &crate::map::DamageableLifeState,
        ),
        Or<(
            With<crate::map::DamageableWorldObject>,
            With<crate::matchplay::HeistSafe>,
        )>,
    >,
    mut sprays: Query<(Entity, &ConeSprayState, &mut ConeSprayRuntime)>,
    spatial_query: avian2d::prelude::SpatialQuery,
) {
    let disconnected: HashSet<_> = disconnected.iter().collect();
    let root = roots.single().ok();
    let mut ordered = sprays.iter_mut().collect::<Vec<_>>();
    ordered.sort_by_key(|(_, _, runtime)| runtime.source.attack_id);
    for (entity, state, mut runtime) in ordered {
        if !spray_owner_is_valid(&runtime, &disconnected, &fighters)
            || !spray_match_is_valid(&runtime, root)
        {
            settle_unresolved_spray(&mut trackers, runtime.source.attack_id);
            commands.entity(entity).despawn();
            continue;
        }
        while tick.0 >= runtime.next_pulse_tick && runtime.next_pulse_tick <= state.expires_at_tick
        {
            let pulse_tick = runtime.next_pulse_tick;
            let delivery_index = runtime.next_delivery_index;
            let reached_distance = state.reached_distance(pulse_tick);
            deliveries.write(PendingDelivery {
                entity: None,
                source: runtime.source,
                delivery_index,
                tick: pulse_tick,
                engagement_distance: 0.0,
                delivery_travel: reached_distance,
                kind: PendingDeliveryKind::ConeSprayPulse {
                    origin: state.origin,
                    facing: state.facing,
                    reached_distance,
                    angle_degrees: state.angle_degrees,
                },
                world_effects: Vec::new(),
            });
            for candidate in spray_candidates(
                *state,
                runtime.source,
                &runtime.recipe,
                reached_distance,
                tuning.radius,
                &disconnected,
                &fighters,
                &objects,
                &spatial_query,
            ) {
                queue_spray_candidate(
                    candidate,
                    runtime.source,
                    &runtime.recipe,
                    delivery_index,
                    state.origin.as_vec2(),
                    &mut pending,
                    &mut world_pending,
                    &mut objective_pending,
                );
            }
            runtime.next_delivery_index = runtime.next_delivery_index.saturating_add(1);
            runtime.next_pulse_tick = runtime
                .next_pulse_tick
                .saturating_add(state.pulse_interval_ticks);
        }
        if tick.0 >= state.expires_at_tick {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reached_distance_fills_then_lingers_without_following_an_owner() {
        let state = ConeSprayState {
            origin: WorldPoint::from(Vec2::new(10.0, 20.0)),
            facing: 0.5,
            propagation_speed: 480.0,
            maximum_reach: 240.0,
            angle_degrees: 70.0,
            emitted_at_tick: 100,
            full_at_tick: 130,
            expires_at_tick: 160,
            pulse_interval_ticks: 10,
            map_occlusion: true,
            max_targets: 6,
        };
        assert!(state.reached_distance(100).abs() < f32::EPSILON);
        assert!((state.reached_distance(115) - 120.0).abs() < 0.001);
        assert!((state.reached_distance(130) - 240.0).abs() < 0.001);
        assert!((state.reached_distance(160) - 240.0).abs() < 0.001);
        assert_eq!(state.origin, WorldPoint::from(Vec2::new(10.0, 20.0)));
        assert!((state.facing - 0.5).abs() < f32::EPSILON);
    }
}
