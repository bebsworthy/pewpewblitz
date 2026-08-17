//! Projectile, sentry, and dash world-space visuals.

#![allow(clippy::wildcard_imports)]
use super::*;
#[derive(Component)]
pub(crate) struct DashTrailVisual {
    target: Entity,
    last_position: Vec2,
}

#[cfg(feature = "client")]
pub(crate) fn ensure_dash_trails(
    mut commands: Commands,
    fighters: Query<(Entity, &Position, &crate::builds::AbilityState), With<Fighter>>,
    trails: Query<&DashTrailVisual>,
) {
    let existing: HashSet<_> = trails.iter().map(|trail| trail.target).collect();
    for (entity, position, ability) in &fighters {
        if matches!(ability.phase, crate::builds::AbilityPhase::Dashing { .. })
            && !existing.contains(&entity)
        {
            commands.spawn((
                DashTrailVisual {
                    target: entity,
                    last_position: position.0,
                },
                Sprite::from_color(Color::srgba(0.25, 0.9, 1.0, 0.55), Vec2::ONE),
                Transform::from_translation(position.0.extend(10.0)),
                Name::new("Dash Trail"),
            ));
        }
    }
}

#[cfg(feature = "client")]
pub(crate) fn sync_dash_trails(
    mut commands: Commands,
    fighters: Query<(&Position, &crate::builds::AbilityState), With<Fighter>>,
    mut trails: Query<(Entity, &mut DashTrailVisual, &mut Transform, &mut Sprite)>,
) {
    for (entity, mut trail, mut transform, mut sprite) in &mut trails {
        let Ok((position, ability)) = fighters.get(trail.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !matches!(ability.phase, crate::builds::AbilityPhase::Dashing { .. }) {
            commands.entity(entity).despawn();
            continue;
        }
        let delta = position.0 - trail.last_position;
        if delta.length_squared() > f32::EPSILON {
            transform.translation = trail.last_position.midpoint(position.0).extend(10.0);
            transform.rotation = Quat::from_rotation_z(delta.y.atan2(delta.x));
            sprite.custom_size = Some(Vec2::new(delta.length().max(2.0), 14.0));
            trail.last_position = position.0;
        }
    }
}

#[cfg(feature = "client")]
pub(crate) fn ensure_sentry_visuals(
    mut commands: Commands,
    sentries: Query<
        (
            Entity,
            &crate::abilities::SentryIdentity,
            &Position,
            &Rotation,
            Option<&Transform>,
        ),
        With<crate::abilities::Sentry>,
    >,
) {
    for (entity, identity, position, rotation, transform) in &sentries {
        if transform.is_none() {
            let color = if identity.team_id.0 == 0 {
                Color::srgb(0.2, 0.75, 1.0)
            } else {
                Color::srgb(1.0, 0.35, 0.15)
            };
            commands.entity(entity).insert((
                Transform {
                    translation: position.0.extend(12.0),
                    rotation: Quat::from_rotation_z(rotation.as_radians()),
                    ..default()
                },
                Sprite::from_color(color, Vec2::splat(38.0)),
                Name::new("Sentry"),
            ));
        }
    }
}

#[cfg(feature = "client")]
pub(crate) fn sync_sentry_visuals(
    mut sentries: Query<(&Position, &Rotation, &mut Transform), With<crate::abilities::Sentry>>,
) {
    for (position, rotation, mut transform) in &mut sentries {
        transform.translation = position.0.extend(12.0);
        transform.rotation = Quat::from_rotation_z(rotation.as_radians());
    }
}

/// Coordinates the headless client lifecycle with its process-level combat evidence contract.
#[cfg(feature = "client")]
pub(crate) fn ensure_projectile_visuals(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &Position,
            &Rotation,
            Option<&Transform>,
            Option<&mut Sprite>,
            &ProjectileSource,
            &ReplicatedAttackSource,
            Option<&LobbedFlight>,
        ),
        With<Projectile>,
    >,
) {
    for (entity, position, rotation, transform, sprite, source, replicated_attack, lobbed) in
        &mut query
    {
        if transform.is_none() {
            commands.entity(entity).insert(Transform {
                translation: position.0.extend(20.0),
                rotation: Quat::from_rotation_z(rotation.as_radians()),
                ..default()
            });
        }
        let color = projectile_color(source.player_id);
        let profile_id = replicated_attack.attack.presentation_profile_id.0;
        let size = match profile_id {
            2 => Vec2::new(9.0, 5.0),
            3 => Vec2::new(16.0, 16.0),
            4 => Vec2::new(24.0, 6.0),
            _ => Vec2::new(20.0, 8.0),
        };
        if let Some(mut sprite) = sprite {
            sprite.color = color;
            sprite.custom_size = Some(size);
        } else {
            commands.entity(entity).insert((
                Sprite::from_color(color, size),
                Name::new(if lobbed.is_some() {
                    "Arc projectile"
                } else {
                    "Weapon delivery"
                }),
            ));
        }
    }
}

#[cfg(feature = "client")]
pub(crate) fn sync_projectile_visuals(
    tick: Query<&AuthoritativeTick>,
    mut query: Query<
        (&Position, &Rotation, &mut Transform, Option<&LobbedFlight>),
        With<Projectile>,
    >,
) {
    let current_tick = tick.iter().next().map_or(0, |tick| tick.0);
    for (position, rotation, mut transform, lobbed) in &mut query {
        transform.translation.x = position.0.x;
        transform.translation.y = position.0.y;
        if let Some(lobbed) = lobbed {
            let progress = (current_tick.saturating_sub(lobbed.launched_at_tick) as f32)
                / (lobbed
                    .lands_at_tick
                    .saturating_sub(lobbed.launched_at_tick)
                    .max(1) as f32);
            transform.translation.z =
                20.0 + delivery::lob_height(progress, lobbed.visual_arc_height);
            transform.rotation = Quat::IDENTITY;
        } else {
            transform.translation.z = 20.0;
            transform.rotation = Quat::from_rotation_z(rotation.as_radians());
        }
    }
}
