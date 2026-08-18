//! Bounded transient combat visual effects and durable status markers.

#![allow(clippy::wildcard_imports)]
use super::*;

#[derive(Component)]
pub(crate) struct CombatEffect {
    pub(crate) timer: Timer,
}

#[cfg(feature = "client")]
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CombatStatusMarker {
    target: Entity,
    kind: CombatStatusKind,
}

#[cfg(feature = "client")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CombatStatusKind {
    Slow,
    Knockback,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime; the query declares this system's complete world view inline at its schedule boundary"
)]
pub(crate) fn update_durable_effect_markers(
    mut commands: Commands,
    fighters: Query<
        (
            Entity,
            &Position,
            Option<&AuthoritativeTick>,
            Option<&ActiveEffects>,
            Option<&KnockbackFeedback>,
            Option<&Defeated>,
        ),
        With<Fighter>,
    >,
    mut markers: Query<(Entity, &CombatStatusMarker, &mut Transform, &mut Sprite)>,
) {
    let desired: HashMap<_, _> = fighters
        .iter()
        .flat_map(
            |(entity, position, authoritative_tick, active_effects, knockback, defeated)| {
                if defeated.is_some() {
                    return Vec::new();
                }
                let mut markers = Vec::with_capacity(2);
                if active_effects.is_some_and(|effects| {
                    effects.slow.is_some_and(|slow| {
                        authoritative_tick.is_none_or(|tick| tick.0 < slow.expires_at_tick)
                    })
                }) {
                    markers.push((
                        CombatStatusMarker {
                            target: entity,
                            kind: CombatStatusKind::Slow,
                        },
                        (position.0, Color::srgba(0.25, 0.75, 1.0, 0.85)),
                    ));
                }
                if knockback.is_some() {
                    markers.push((
                        CombatStatusMarker {
                            target: entity,
                            kind: CombatStatusKind::Knockback,
                        },
                        (position.0, Color::srgba(1.0, 0.55, 0.18, 0.85)),
                    ));
                }
                markers
            },
        )
        .collect();
    let mut existing = HashSet::new();
    for (marker_entity, marker, mut transform, mut sprite) in &mut markers {
        if let Some((position, color)) = desired.get(marker) {
            existing.insert(*marker);
            transform.translation = position.extend(39.0);
            sprite.color = *color;
        } else {
            commands.entity(marker_entity).despawn();
        }
    }
    for (marker, (position, color)) in desired {
        if !existing.contains(&marker) {
            commands.spawn((
                marker,
                Sprite::from_color(color, Vec2::splat(13.0)),
                Transform::from_translation(position.extend(39.0)),
            ));
        }
    }
}

#[cfg(feature = "client")]
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(crate) fn update_combat_effects(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut effects: Query<(Entity, &mut CombatEffect)>,
) {
    for (entity, mut effect) in &mut effects {
        effect.timer.tick(time.delta());
        if effect.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}
