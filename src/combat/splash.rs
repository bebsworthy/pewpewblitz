//! Stationary persistent Splash-area authority.

#![allow(
    clippy::wildcard_imports,
    reason = "the server-owned Splash transaction consumes the combat composition surface"
)]

use super::*;
use avian2d::prelude::Position;
use bevy::prelude::*;
use std::collections::HashSet;

pub(crate) const MAX_ACTIVE_PERSISTENT_SPLASHES: usize = 16;

#[must_use]
pub(crate) fn splash_timing(
    activated_at_tick: u64,
    duration_ticks: u64,
    pulse_interval_ticks: u64,
) -> (u64, u8) {
    let expires_at_tick = activated_at_tick.saturating_add(duration_ticks);
    let pulse_count = duration_ticks / pulse_interval_ticks + 1;
    (
        expires_at_tick,
        u8::try_from(pulse_count).unwrap_or(u8::MAX),
    )
}

#[must_use]
pub(crate) fn presentation_effects(recipe: &WeaponRecipe) -> [Option<PayloadEffectDefinition>; 2] {
    let mut effects = [None, None];
    if let Some(bundle) = recipe.payload_bundles.first() {
        for (slot, effect) in effects.iter_mut().zip(bundle.effects.iter().copied()) {
            *slot = Some(effect);
        }
    }
    effects
}

pub(crate) fn settle_unresolved_splash(trackers: &mut ActiveAttackTrackers, attack_id: AttackId) {
    while trackers.active.contains_key(&attack_id) {
        finish_attack_delivery(trackers, attack_id);
    }
}

fn match_is_valid(
    runtime: &PersistentSplashRuntime,
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
    reason = "candidate collection declares the complete authoritative fighter and occlusion view"
)]
fn candidates(
    state: PersistentSplashState,
    source: AttackSource,
    bundle: &PayloadBundleDefinition,
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
        With<Fighter>,
    >,
    spatial_query: &avian2d::prelude::SpatialQuery,
) -> Vec<(Entity, Vec2, TeamId, NetworkEntityId)> {
    let center = state.center.as_vec2();
    let mut candidates = fighters
        .iter()
        .filter_map(
            |(entity, position, team, network_id, defeated, controlled, participant, active)| {
                (defeated.is_none()
                    && (!participant || active)
                    && controlled
                        .is_none_or(|controlled| !disconnected.contains(&controlled.owner))
                    && payload_can_affect_target(bundle, source, *team, *network_id)
                    && state
                        .shape
                        .contains(center, state.facing, position.0, fighter_radius)
                    && (!state.map_occlusion
                        || area_line_of_sight_clear(center, position.0, spatial_query)))
                .then_some((entity, position.0, *team, *network_id))
            },
        )
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        center
            .distance_squared(left.1)
            .total_cmp(&center.distance_squared(right.1))
            .then_with(|| left.3.cmp(&right.3))
    });
    candidates.truncate(usize::from(state.max_targets));
    candidates
}

#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "the fixed-tick Splash system owns bounded lifecycle, occupancy, ordering, and payload emission"
)]
pub(super) fn advance_persistent_splashes(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    builds: Res<crate::builds::BuildCatalogResource>,
    mut trackers: ResMut<ActiveAttackTrackers>,
    mut pending: MessageWriter<PendingPayload>,
    mut deliveries: MessageWriter<PendingDelivery>,
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
        With<Fighter>,
    >,
    mut splashes: Query<(
        Entity,
        &mut PersistentSplashState,
        &mut PersistentSplashRuntime,
    )>,
    spatial_query: avian2d::prelude::SpatialQuery,
) {
    let disconnected = disconnected.iter().collect::<HashSet<_>>();
    let root = roots.single().ok();
    let mut ordered = splashes.iter_mut().collect::<Vec<_>>();
    ordered.sort_by_key(|(_, _, runtime)| runtime.source.attack_id);
    for (entity, mut state, mut runtime) in ordered {
        if !match_is_valid(&runtime, root) {
            settle_unresolved_splash(&mut trackers, runtime.source.attack_id);
            commands.entity(entity).despawn();
            continue;
        }
        let Some(bundle) = runtime.recipe.payload_bundles.first().cloned() else {
            settle_unresolved_splash(&mut trackers, runtime.source.attack_id);
            commands.entity(entity).despawn();
            continue;
        };
        while tick.0 >= state.next_pulse_tick && state.next_pulse_tick <= state.expires_at_tick {
            let pulse_tick = state.next_pulse_tick;
            let delivery_index = runtime.next_delivery_index;
            deliveries.write(PendingDelivery {
                entity: None,
                source: runtime.source,
                delivery_index,
                tick: pulse_tick,
                engagement_distance: 0.0,
                delivery_travel: runtime
                    .source
                    .origin
                    .as_vec2()
                    .distance(state.center.as_vec2()),
                kind: PendingDeliveryKind::SplashPulse {
                    center: state.center,
                },
                world_effects: Vec::new(),
            });
            for (target, position, _, network_id) in candidates(
                *state,
                runtime.source,
                &bundle,
                builds.0.fighter_body.radius,
                &disconnected,
                &fighters,
                &spatial_query,
            ) {
                pending.write(PendingPayload {
                    source: runtime.source,
                    delivery_index,
                    bundle_index: 0,
                    target,
                    target_network_id: network_id,
                    position,
                    engagement_distance: state.center.as_vec2().distance(position),
                    delivery_travel: runtime
                        .source
                        .origin
                        .as_vec2()
                        .distance(state.center.as_vec2()),
                    contact_fraction: 1.0,
                    bundle: bundle.clone(),
                });
            }
            runtime.next_delivery_index = runtime.next_delivery_index.saturating_add(1);
            state.next_pulse_tick = state
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
    fn splash_timing_includes_landing_and_expiry_pulses() {
        assert_eq!(splash_timing(10, 240, 30), (250, 9));
    }

    #[test]
    fn circle_and_oriented_rectangle_include_fighter_footprints() {
        let circle = PersistentAreaShape::Circle { radius: 10.0 };
        assert!(circle.contains(Vec2::ZERO, 0.0, Vec2::new(12.0, 0.0), 2.0));
        assert!(!circle.contains(Vec2::ZERO, 0.0, Vec2::new(12.1, 0.0), 2.0));

        let rectangle = PersistentAreaShape::Rectangle {
            half_extents: [10.0, 4.0],
        };
        assert!(rectangle.contains(
            Vec2::ZERO,
            core::f32::consts::FRAC_PI_2,
            Vec2::Y * 11.0,
            1.0
        ));
        assert!(!rectangle.contains(Vec2::ZERO, core::f32::consts::FRAC_PI_2, Vec2::X * 5.1, 1.0));
    }
}
