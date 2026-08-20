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
    settings: Option<Res<crate::client::ClientShellSettings>>,
    fighters: Query<(Entity, &Position, &crate::builds::AbilityState), With<Fighter>>,
    trails: Query<&DashTrailVisual>,
) {
    let reduced = settings.is_some_and(|settings| settings.reduced_combat_effects);
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
                Sprite::from_color(
                    Color::srgba(0.25, 0.9, 1.0, if reduced { 0.3 } else { 0.55 }),
                    Vec2::ONE,
                ),
                Transform::from_translation(position.0.extend(10.0)),
                Name::new("Dash Trail"),
            ));
        }
    }
}

#[cfg(feature = "client")]
pub(crate) fn sync_dash_trails(
    mut commands: Commands,
    settings: Option<Res<crate::client::ClientShellSettings>>,
    fighters: Query<(&Position, &crate::builds::AbilityState), With<Fighter>>,
    mut trails: Query<(Entity, &mut DashTrailVisual, &mut Transform, &mut Sprite)>,
) {
    let reduced = settings.is_some_and(|settings| settings.reduced_combat_effects);
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
            sprite.custom_size = Some(Vec2::new(
                delta.length().max(2.0) * if reduced { 0.65 } else { 1.0 },
                if reduced { 8.0 } else { 14.0 },
            ));
            trail.last_position = position.0;
        }
    }
}

#[cfg(feature = "client")]
#[allow(
    clippy::type_complexity,
    reason = "the query declares this system's complete world view inline at its schedule boundary"
)]
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
