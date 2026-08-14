//! Client-only combat presentation geometry.

use super::{
    DeliveryMethod, FiringPattern, ResolvedWeapon, TargetSelection, delivery, spread_angles,
};
use bevy::prelude::{Color, Vec2};

#[cfg(feature = "client")]
pub(super) fn preview_segments(
    origin: Vec2,
    facing: f32,
    resolved: &ResolvedWeapon,
    arena: &crate::movement::GreyboxArenaDefinition,
) -> Vec<(Vec2, f32, Vec2, Color)> {
    let mut segments = Vec::with_capacity(10);
    match resolved.recipe.delivery {
        DeliveryMethod::Straight { range, .. } => {
            let angles = match resolved.recipe.firing {
                FiringPattern::Single => vec![facing],
                FiringPattern::Spread {
                    delivery_count,
                    total_angle_degrees,
                } => spread_angles(facing, delivery_count, total_angle_degrees),
            };
            let preview_angles = if angles.len() > 1 {
                vec![angles[0], *angles.last().expect("spread has an angle")]
            } else {
                angles
            };
            for angle in preview_angles {
                let direction = Vec2::from_angle(angle);
                segments.push((
                    origin + direction * (range * 0.5),
                    angle,
                    Vec2::new(range, 2.0),
                    Color::srgba(0.95, 0.85, 0.25, 0.30),
                ));
            }
        }
        DeliveryMethod::Lobbed {
            distance,
            landing_clearance_radius,
            ..
        } => {
            let direction = Vec2::from_angle(facing);
            let desired = origin + direction * distance;
            let bounded = desired.clamp(
                arena.min + Vec2::splat(landing_clearance_radius),
                arena.max - Vec2::splat(landing_clearance_radius),
            );
            let landing = delivery::repaired_landing_point(
                origin,
                bounded,
                landing_clearance_radius,
                |candidate| {
                    arena
                        .perimeter_wall_shapes()
                        .into_iter()
                        .chain(arena.cover_shapes())
                        .all(|(center, size)| {
                            !circle_overlaps_rect(candidate, landing_clearance_radius, center, size)
                        })
                },
            )
            .unwrap_or(bounded);
            let landing_color = if landing.distance(desired) > 0.5 {
                Color::srgba(1.0, 0.65, 0.2, 0.40)
            } else {
                Color::srgba(0.35, 0.85, 1.0, 0.34)
            };
            segments.push((
                origin + direction * (origin.distance(landing) * 0.5),
                facing,
                Vec2::new(origin.distance(landing), 2.0),
                landing_color,
            ));
            let explosion_radius = resolved
                .recipe
                .payload_bundles
                .iter()
                .find_map(|bundle| match bundle.target {
                    TargetSelection::Area { radius, .. } => Some(radius),
                    TargetSelection::Direct => None,
                })
                .unwrap_or(24.0);
            for index in 0..4 {
                let angle = std::f32::consts::FRAC_PI_2 * index as f32;
                let radial = Vec2::from_angle(angle);
                segments.push((
                    landing + radial * explosion_radius,
                    angle,
                    Vec2::new(explosion_radius * 2.0, 3.0),
                    landing_color,
                ));
            }
        }
        DeliveryMethod::MeleeArc {
            reach,
            angle_degrees,
        } => {
            for angle in [
                facing - angle_degrees.to_radians() * 0.5,
                facing + angle_degrees.to_radians() * 0.5,
            ] {
                let direction = Vec2::from_angle(angle);
                segments.push((
                    origin + direction * (reach * 0.5),
                    angle,
                    Vec2::new(reach, 3.0),
                    Color::srgba(1.0, 0.35, 0.35, 0.32),
                ));
            }
            let center_direction = Vec2::from_angle(facing);
            segments.push((
                origin + center_direction * (reach * 0.5),
                facing,
                Vec2::new(reach, 4.0),
                Color::srgba(1.0, 0.25, 0.25, 0.18),
            ));
        }
    }
    segments
}

fn circle_overlaps_rect(center: Vec2, radius: f32, rect_center: Vec2, rect_size: Vec2) -> bool {
    let rect_min = rect_center - rect_size * 0.5;
    let rect_max = rect_center + rect_size * 0.5;
    let closest = center.clamp(rect_min, rect_max);
    closest.distance_squared(center) < radius * radius
}
