//! Weapon preview geometry for the controlled fighter.

#![allow(clippy::wildcard_imports)]
use super::*;
pub const MAX_PREVIEW_SEGMENTS: usize = 24;

#[cfg(feature = "client")]
#[allow(clippy::too_many_lines)]
pub(crate) fn preview_segments(
    origin: Vec2,
    facing: f32,
    aim_distance: Option<f32>,
    resolved: &ResolvedWeapon,
    map: &crate::map::ResolvedMapSnapshot,
    terrain_chunks: &BTreeMap<crate::terrain::TerrainChunkId, crate::terrain::TerrainBits>,
) -> Vec<(Vec2, f32, Vec2, Color)> {
    let mut segments = Vec::with_capacity(MAX_PREVIEW_SEGMENTS);
    match resolved.recipe.delivery {
        DeliveryMethod::Straight { range, .. } => {
            let angles = match resolved.recipe.firing {
                FiringPattern::Single => vec![facing],
                FiringPattern::Spread {
                    delivery_count,
                    total_angle_degrees,
                } => spread_angles(facing, delivery_count, total_angle_degrees),
            };
            let is_spread = angles.len() > 1;
            let preview_angles = if is_spread {
                vec![angles[0], *angles.last().expect("spread has an angle")]
            } else {
                angles.clone()
            };
            for angle in preview_angles {
                let direction = Vec2::from_angle(angle);
                let line_range = if is_spread { range * 0.78 } else { range };
                segments.push((
                    origin + direction * (line_range * 0.5),
                    angle,
                    Vec2::new(line_range, if is_spread { 2.0 } else { 3.0 }),
                    if is_spread {
                        Color::srgba(1.0, 0.72, 0.2, 0.25)
                    } else {
                        Color::srgba(0.95, 0.85, 0.25, 0.30)
                    },
                ));
            }
            let marker_color = if is_spread {
                Color::srgba(1.0, 0.72, 0.2, 0.35)
            } else {
                Color::srgba(1.0, 0.9, 0.35, 0.45)
            };
            if is_spread {
                let start = angles[0];
                let end = *angles.last().expect("spread has an angle");
                for index in 0..6 {
                    let a0 = start + (end - start) * index as f32 / 6.0;
                    let a1 = start + (end - start) * (index + 1) as f32 / 6.0;
                    segments.push(segment_between(
                        origin + Vec2::from_angle(a0) * range,
                        origin + Vec2::from_angle(a1) * range,
                        2.0,
                        marker_color,
                    ));
                }
            } else {
                segments.push((
                    origin + Vec2::from_angle(facing) * range,
                    0.0,
                    Vec2::splat(10.0),
                    marker_color,
                ));
            }
        }
        DeliveryMethod::Lobbed {
            distance,
            landing_clearance_radius,
            ..
        } => {
            let direction = Vec2::from_angle(facing);
            let desired =
                origin + direction * aim_distance.unwrap_or(distance).clamp(0.0, distance);
            let bounded = desired.clamp(
                map.playable_bounds.min + Vec2::splat(landing_clearance_radius),
                map.playable_bounds.max - Vec2::splat(landing_clearance_radius),
            );
            let repaired_landing = delivery::repaired_landing_point(
                origin,
                bounded,
                landing_clearance_radius,
                |candidate| {
                    map.geometry.iter().all(|geometry| {
                        !circle_overlaps_map_shape(
                            candidate,
                            landing_clearance_radius,
                            geometry.position,
                            geometry.rotation,
                            geometry.shape,
                        )
                    }) && !crate::terrain::grid::circle_overlaps_occupied(
                        candidate,
                        landing_clearance_radius,
                        terrain_chunks,
                    )
                },
            );
            let landing = repaired_landing.unwrap_or(bounded);
            let landing_color = if repaired_landing.is_none() {
                Color::srgba(1.0, 0.16, 0.16, 0.50)
            } else if landing.distance(bounded) > 0.5 {
                Color::srgba(0.95, 0.35, 1.0, 0.45)
            } else if bounded.distance(desired) > 0.5 {
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
            segments.push((landing, 0.0, Vec2::splat(12.0), landing_color));
            let explosion_radius = resolved
                .recipe
                .payload_bundles
                .iter()
                .find_map(|bundle| match bundle.target {
                    TargetSelection::Area { radius, .. } => Some(radius),
                    TargetSelection::Direct => None,
                })
                .unwrap_or(24.0);
            for index in 0..12 {
                let a0 = std::f32::consts::TAU * index as f32 / 12.0;
                let a1 = std::f32::consts::TAU * (index + 1) as f32 / 12.0;
                segments.push(segment_between(
                    landing + Vec2::from_angle(a0) * explosion_radius,
                    landing + Vec2::from_angle(a1) * explosion_radius,
                    3.0,
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
            for index in 0..8 {
                let a0 = facing - angle_degrees.to_radians() * 0.5
                    + angle_degrees.to_radians() * index as f32 / 8.0;
                let a1 = facing - angle_degrees.to_radians() * 0.5
                    + angle_degrees.to_radians() * (index + 1) as f32 / 8.0;
                segments.push(segment_between(
                    origin + Vec2::from_angle(a0) * reach,
                    origin + Vec2::from_angle(a1) * reach,
                    4.0,
                    Color::srgba(1.0, 0.25, 0.25, 0.30),
                ));
            }
        }
    }
    segments.truncate(MAX_PREVIEW_SEGMENTS);
    segments
}

fn segment_between(start: Vec2, end: Vec2, width: f32, color: Color) -> (Vec2, f32, Vec2, Color) {
    let delta = end - start;
    (
        start.midpoint(end),
        delta.y.atan2(delta.x),
        Vec2::new(delta.length(), width),
        color,
    )
}

fn circle_overlaps_map_shape(
    center: Vec2,
    radius: f32,
    shape_center: Vec2,
    rotation: f32,
    shape: crate::map::MapShape,
) -> bool {
    match shape {
        crate::map::MapShape::Circle {
            radius: shape_radius,
        } => center.distance_squared(shape_center) < (radius + shape_radius).powi(2),
        crate::map::MapShape::Rectangle { half_extents } => {
            let local = Vec2::from_angle(-rotation).rotate(center - shape_center);
            let closest = local.clamp(-half_extents, half_extents);
            local.distance_squared(closest) < radius * radius
        }
    }
}
