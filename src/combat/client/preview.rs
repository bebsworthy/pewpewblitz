//! Weapon preview geometry for the controlled fighter.

#![allow(clippy::wildcard_imports)]
use super::*;
use avian2d::{collision::collider::contact_query::time_of_impact, prelude::Collider};
use std::collections::HashMap;

/// Sixteen straight deliveries need one corridor and one terminal disc each. The remaining slots
/// preserve the existing lobbed/melee previews without allocating presentation entities per frame.
pub const MAX_PREVIEW_SEGMENTS: usize = 48;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PreviewGeometry {
    Corridor {
        center: Vec2,
        angle: f32,
        length: f32,
        width: f32,
    },
    Disc {
        center: Vec2,
        radius: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PreviewPrimitive {
    pub geometry: PreviewGeometry,
    pub blocked: bool,
}

impl PreviewPrimitive {
    fn corridor(start: Vec2, end: Vec2, width: f32, blocked: bool) -> Self {
        let delta = end - start;
        Self {
            geometry: PreviewGeometry::Corridor {
                center: start.midpoint(end),
                angle: delta.y.atan2(delta.x),
                length: delta.length().max(0.001),
                width,
            },
            blocked,
        }
    }

    fn disc(center: Vec2, radius: f32, blocked: bool) -> Self {
        Self {
            geometry: PreviewGeometry::Disc { center, radius },
            blocked,
        }
    }

    #[must_use]
    #[cfg(test)]
    pub(crate) fn is_finite(self) -> bool {
        match self.geometry {
            PreviewGeometry::Corridor {
                center,
                angle,
                length,
                width,
            } => {
                center.is_finite()
                    && angle.is_finite()
                    && length.is_finite()
                    && width.is_finite()
                    && length > 0.0
                    && width > 0.0
            }
            PreviewGeometry::Disc { center, radius } => {
                center.is_finite() && radius.is_finite() && radius > 0.0
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AimTraceBlockerClass {
    Fighter,
    Sentry,
    HeistSafe,
}

/// One currently replicated dynamic body eligible to stop the local player's next straight shot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AimTraceDynamicBlocker {
    pub class: AimTraceBlockerClass,
    pub stable_id: u128,
    pub position: Vec2,
    pub rotation: f32,
    pub shape: crate::map::MapShape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AimTraceIndexKey {
    instance_id: crate::map::MapInstanceId,
    generation: u64,
    revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct IndexedMapBlocker {
    placement_id: crate::map::MapPlacementId,
    position: Vec2,
    shape: crate::map::MapShape,
}

/// Read-only client index of effective projectile-blocking map geometry.
///
/// It rebuilds only when map identity/generation/revision changes. Buckets are sparse, so a map
/// filled with non-blocking grass does not allocate one entry per cell, while a deliberately full
/// wall map remains supported within the authored placement envelope.
#[derive(Resource, Default)]
pub(crate) struct AimTraceBlockerIndex {
    key: Option<AimTraceIndexKey>,
    dimensions: Option<crate::map::MapDimensions>,
    blockers: Vec<IndexedMapBlocker>,
    buckets: HashMap<u32, Vec<usize>>,
}

impl AimTraceBlockerIndex {
    pub(crate) fn refresh(
        &mut self,
        snapshot: &crate::map::ResolvedMapSnapshot,
        state: &crate::map::MapDynamicState,
        catalog: &crate::map::MapContentCatalog,
    ) {
        let key = AimTraceIndexKey {
            instance_id: snapshot.identity.instance_id,
            generation: state.generation,
            revision: state.revision,
        };
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        self.dimensions = Some(snapshot.dimensions);
        self.blockers.clear();
        self.buckets.clear();
        for placement in &snapshot.placements {
            let Some(collider) =
                crate::map::effective_projectile_collider(placement, snapshot, state, catalog)
            else {
                continue;
            };
            let blocker_index = self.blockers.len();
            let blocker = IndexedMapBlocker {
                placement_id: collider.placement_id,
                position: collider.position,
                shape: collider.shape,
            };
            self.blockers.push(blocker);
            let (min, max) = shape_aabb(blocker.position, blocker.shape, 0.0);
            Self::for_each_bucket(self.dimensions, min, max, |bucket| {
                self.buckets.entry(bucket).or_default().push(blocker_index);
            });
        }
    }

    fn for_each_bucket(
        dimensions: Option<crate::map::MapDimensions>,
        min: Vec2,
        max: Vec2,
        mut visit: impl FnMut(u32),
    ) {
        let Some(dimensions) = dimensions else {
            return;
        };
        let bounds = dimensions.bounds();
        if max.x < bounds.min.x
            || max.y < bounds.min.y
            || min.x > bounds.max.x
            || min.y > bounds.max.y
        {
            return;
        }
        let to_cell = |coordinate: f32, axis_min: f32, count: u16| {
            (((coordinate - axis_min) / crate::map::MAP_CELL_SIZE_WORLD).floor() as i32)
                .clamp(0, i32::from(count) - 1) as u16
        };
        let min_x = to_cell(min.x, bounds.min.x, dimensions.width);
        let max_x = to_cell(max.x, bounds.min.x, dimensions.width);
        let min_y = to_cell(min.y, bounds.min.y, dimensions.height);
        let max_y = to_cell(max.y, bounds.min.y, dimensions.height);
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                visit(u32::from(y) * u32::from(dimensions.width) + u32::from(x));
            }
        }
    }

    fn swept_buckets(&self, start: Vec2, end: Vec2, radius: f32) -> Vec<u32> {
        let Some(dimensions) = self.dimensions else {
            return Vec::new();
        };
        let bounds = dimensions.bounds();
        let cell_size = crate::map::MAP_CELL_SIZE_WORLD;
        let to_cell = |coordinate: f32, axis_min: f32, count: u16| {
            (((coordinate - axis_min) / cell_size).floor() as i32).clamp(0, i32::from(count) - 1)
                as u16
        };
        let sweep_min_x = start.x.min(end.x) - radius;
        let sweep_max_x = start.x.max(end.x) + radius;
        if sweep_max_x < bounds.min.x || sweep_min_x > bounds.max.x {
            return Vec::new();
        }
        let min_x = to_cell(sweep_min_x, bounds.min.x, dimensions.width);
        let max_x = to_cell(sweep_max_x, bounds.min.x, dimensions.width);
        let delta = end - start;
        let mut buckets = Vec::new();
        for x in min_x..=max_x {
            let column_min = bounds.min.x + f32::from(x) * cell_size - radius;
            let column_max = column_min + cell_size + radius * 2.0;
            let (t_min, t_max) = if delta.x.abs() <= f32::EPSILON {
                if start.x < column_min || start.x > column_max {
                    continue;
                }
                (0.0, 1.0)
            } else {
                let t0 = (column_min - start.x) / delta.x;
                let t1 = (column_max - start.x) / delta.x;
                (t0.min(t1).clamp(0.0, 1.0), t0.max(t1).clamp(0.0, 1.0))
            };
            if t_min > t_max {
                continue;
            }
            let y0 = start.y + delta.y * t_min;
            let y1 = start.y + delta.y * t_max;
            let sweep_min_y = y0.min(y1) - radius;
            let sweep_max_y = y0.max(y1) + radius;
            if sweep_max_y < bounds.min.y || sweep_min_y > bounds.max.y {
                continue;
            }
            let min_y = to_cell(sweep_min_y, bounds.min.y, dimensions.height);
            let max_y = to_cell(sweep_max_y, bounds.min.y, dimensions.height);
            for y in min_y..=max_y {
                buckets.push(u32::from(y) * u32::from(dimensions.width) + u32::from(x));
            }
        }
        buckets.sort_unstable();
        buckets.dedup();
        buckets
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct AimTraceResult {
    stop_center: Vec2,
    distance: f32,
    blocked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct StableBlockerKey(u8, u128);

fn consider_hit(best: &mut Option<(f32, StableBlockerKey)>, distance: f32, key: StableBlockerKey) {
    const TIE_EPSILON: f32 = 0.0001;
    if !distance.is_finite() || distance < 0.0 {
        return;
    }
    let replace = best.is_none_or(|(current_distance, current_key)| {
        distance < current_distance - TIE_EPSILON
            || ((distance - current_distance).abs() <= TIE_EPSILON && key < current_key)
    });
    if replace {
        *best = Some((distance, key));
    }
}

fn trace_projectile_body(
    index: &AimTraceBlockerIndex,
    body: ProjectileBody,
    start: Vec2,
    direction: Vec2,
    maximum_distance: f32,
    dynamic: &[AimTraceDynamicBlocker],
) -> AimTraceResult {
    let maximum_distance = maximum_distance.max(0.0);
    let direction = direction.normalize_or_zero();
    if direction == Vec2::ZERO || maximum_distance <= f32::EPSILON {
        return AimTraceResult {
            stop_center: start,
            distance: 0.0,
            blocked: false,
        };
    }
    let mut best = None;
    let radius = body.shape.bounding_radius();
    if let Some(dimensions) = index.dimensions
        && let Some(distance) = distance_to_perimeter(
            dimensions.bounds(),
            start,
            direction,
            radius,
            maximum_distance,
        )
    {
        consider_hit(&mut best, distance, StableBlockerKey(0, u128::MAX));
    }
    let end = start + direction * maximum_distance;
    let body_collider = body.collider();
    let mut candidate_indices = Vec::new();
    for bucket in index.swept_buckets(start, end, radius) {
        let Some(bucket_candidates) = index.buckets.get(&bucket) else {
            continue;
        };
        candidate_indices.extend_from_slice(bucket_candidates);
    }
    candidate_indices.sort_unstable();
    candidate_indices.dedup();
    for candidate in candidate_indices {
        let blocker = index.blockers[candidate];
        if let Some(distance) = collider_time_of_impact(
            &body_collider,
            start,
            direction,
            maximum_distance,
            blocker.position,
            0.0,
            blocker.shape,
        ) {
            consider_hit(
                &mut best,
                distance,
                StableBlockerKey(0, u128::from(blocker.placement_id.0)),
            );
        }
    }
    for blocker in dynamic {
        if let Some(distance) = collider_time_of_impact(
            &body_collider,
            start,
            direction,
            maximum_distance,
            blocker.position,
            blocker.rotation,
            blocker.shape,
        ) {
            consider_hit(
                &mut best,
                distance,
                StableBlockerKey(1 + blocker.class as u8, blocker.stable_id),
            );
        }
    }
    let (distance, blocked) = best.map_or((maximum_distance, false), |(distance, _)| {
        (distance.clamp(0.0, maximum_distance), true)
    });
    AimTraceResult {
        stop_center: start + direction * distance,
        distance,
        blocked,
    }
}

fn collider_time_of_impact(
    projectile: &Collider,
    start: Vec2,
    direction: Vec2,
    maximum_distance: f32,
    blocker_position: Vec2,
    blocker_rotation: f32,
    blocker_shape: crate::map::MapShape,
) -> Option<f32> {
    time_of_impact(
        projectile,
        start,
        0.0,
        direction,
        &map_shape_collider(blocker_shape),
        blocker_position,
        blocker_rotation,
        Vec2::ZERO,
        maximum_distance,
    )
    .ok()
    .flatten()
    .map(|hit| hit.time_of_impact)
}

fn map_shape_collider(shape: crate::map::MapShape) -> Collider {
    match shape {
        crate::map::MapShape::Rectangle { half_extents } => {
            Collider::rectangle(half_extents.x * 2.0, half_extents.y * 2.0)
        }
        crate::map::MapShape::Circle { radius } => Collider::circle(radius),
    }
}

fn shape_aabb(position: Vec2, shape: crate::map::MapShape, rotation: f32) -> (Vec2, Vec2) {
    let half_extents = shape.bounding_half_extents(rotation);
    (position - half_extents, position + half_extents)
}

fn distance_to_perimeter(
    bounds: crate::map::AxisAlignedMapRect,
    start: Vec2,
    direction: Vec2,
    radius: f32,
    maximum_distance: f32,
) -> Option<f32> {
    let min = bounds.min + Vec2::splat(radius);
    let max = bounds.max - Vec2::splat(radius);
    if min.x > max.x || min.y > max.y || !start.cmpge(min).all() || !start.cmple(max).all() {
        return Some(0.0);
    }
    let x = if direction.x > f32::EPSILON {
        (max.x - start.x) / direction.x
    } else if direction.x < -f32::EPSILON {
        (min.x - start.x) / direction.x
    } else {
        f32::INFINITY
    };
    let y = if direction.y > f32::EPSILON {
        (max.y - start.y) / direction.y
    } else if direction.y < -f32::EPSILON {
        (min.y - start.y) / direction.y
    } else {
        f32::INFINITY
    };
    let distance = x.min(y).max(0.0);
    (distance <= maximum_distance).then_some(distance)
}

#[cfg(feature = "client")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn preview_primitives(
    origin: Vec2,
    facing: f32,
    aim_distance: Option<f32>,
    resolved: &ResolvedWeapon,
    map: &crate::map::ResolvedMapSnapshot,
    state: &crate::map::MapDynamicState,
    catalog: &crate::map::MapContentCatalog,
    index: &AimTraceBlockerIndex,
    dynamic: &[AimTraceDynamicBlocker],
) -> Vec<PreviewPrimitive> {
    let mut primitives = match resolved.recipe.delivery {
        DeliveryMethod::Straight { .. } | DeliveryMethod::StickyStraight { .. } => {
            straight_preview_primitives(origin, facing, resolved, index, dynamic)
        }
        DeliveryMethod::Lobbed {
            distance,
            landing_clearance_radius,
            ..
        } => lobbed_preview_primitives(
            origin,
            facing,
            aim_distance,
            distance,
            landing_clearance_radius,
            resolved,
            map,
            state,
            catalog,
        ),
        DeliveryMethod::MeleeArc {
            reach,
            angle_degrees,
        } => melee_preview_primitives(origin, facing, reach, angle_degrees),
    };
    primitives.truncate(MAX_PREVIEW_SEGMENTS);
    primitives
}

fn straight_preview_primitives(
    origin: Vec2,
    facing: f32,
    resolved: &ResolvedWeapon,
    index: &AimTraceBlockerIndex,
    dynamic: &[AimTraceDynamicBlocker],
) -> Vec<PreviewPrimitive> {
    let (DeliveryMethod::Straight {
        radius,
        range,
        muzzle_offset,
        ..
    }
    | DeliveryMethod::StickyStraight {
        radius,
        range,
        muzzle_offset,
        ..
    }) = resolved.recipe.delivery
    else {
        return Vec::new();
    };
    let body = ProjectileBody::circle(radius);
    let angles = match resolved.recipe.firing {
        FiringPattern::Single => vec![facing],
        FiringPattern::Spread {
            delivery_count,
            total_angle_degrees,
        } => spread_angles(facing, delivery_count, total_angle_degrees),
    };
    let muzzle_blockers: Vec<_> = dynamic
        .iter()
        .copied()
        .filter(|blocker| blocker.class == AimTraceBlockerClass::HeistSafe)
        .collect();
    let mut primitives = Vec::with_capacity(angles.len() * 2);
    for angle in angles {
        let direction = Vec2::from_angle(angle);
        let muzzle_trace = trace_projectile_body(
            index,
            body,
            origin,
            direction,
            muzzle_offset,
            &muzzle_blockers,
        );
        if muzzle_trace.blocked {
            if muzzle_trace.distance > f32::EPSILON {
                primitives.push(PreviewPrimitive::corridor(
                    origin,
                    muzzle_trace.stop_center,
                    radius * 2.0,
                    true,
                ));
            }
            primitives.push(PreviewPrimitive::disc(
                muzzle_trace.stop_center,
                radius,
                true,
            ));
            continue;
        }
        let muzzle = origin + direction * muzzle_offset;
        let travel = trace_projectile_body(index, body, muzzle, direction, range, dynamic);
        primitives.push(PreviewPrimitive::corridor(
            muzzle,
            travel.stop_center,
            radius * 2.0,
            travel.blocked,
        ));
        primitives.push(PreviewPrimitive::disc(
            travel.stop_center,
            radius,
            travel.blocked,
        ));
    }
    primitives
}

#[allow(
    clippy::too_many_arguments,
    reason = "lobbed preview repair consumes the complete authored and current map geometry view"
)]
fn lobbed_preview_primitives(
    origin: Vec2,
    facing: f32,
    aim_distance: Option<f32>,
    distance: f32,
    landing_clearance_radius: f32,
    resolved: &ResolvedWeapon,
    map: &crate::map::ResolvedMapSnapshot,
    state: &crate::map::MapDynamicState,
    catalog: &crate::map::MapContentCatalog,
) -> Vec<PreviewPrimitive> {
    let direction = Vec2::from_angle(facing);
    let desired = origin + direction * aim_distance.unwrap_or(distance).clamp(0.0, distance);
    let bounded = desired.clamp(
        map.dimensions.bounds().min + Vec2::splat(landing_clearance_radius),
        map.dimensions.bounds().max - Vec2::splat(landing_clearance_radius),
    );
    let repaired_landing =
        delivery::repaired_landing_point(origin, bounded, landing_clearance_radius, |candidate| {
            !crate::map::circle_overlaps_blocking_map(
                candidate,
                landing_clearance_radius,
                map,
                state,
                catalog,
            )
        });
    let landing = repaired_landing.unwrap_or(bounded);
    let blocked = repaired_landing.is_none();
    let mut primitives = vec![
        PreviewPrimitive::corridor(origin, landing, 2.0, blocked),
        PreviewPrimitive::disc(landing, 6.0, blocked),
    ];
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
        primitives.push(PreviewPrimitive::corridor(
            landing + Vec2::from_angle(a0) * explosion_radius,
            landing + Vec2::from_angle(a1) * explosion_radius,
            3.0,
            blocked,
        ));
    }
    primitives
}

fn melee_preview_primitives(
    origin: Vec2,
    facing: f32,
    reach: f32,
    angle_degrees: f32,
) -> Vec<PreviewPrimitive> {
    let mut primitives = Vec::with_capacity(10);
    for angle in [
        facing - angle_degrees.to_radians() * 0.5,
        facing + angle_degrees.to_radians() * 0.5,
    ] {
        primitives.push(PreviewPrimitive::corridor(
            origin,
            origin + Vec2::from_angle(angle) * reach,
            3.0,
            false,
        ));
    }
    for index in 0..8 {
        let a0 = facing - angle_degrees.to_radians() * 0.5
            + angle_degrees.to_radians() * index as f32 / 8.0;
        let a1 = facing - angle_degrees.to_radians() * 0.5
            + angle_degrees.to_radians() * (index + 1) as f32 / 8.0;
        primitives.push(PreviewPrimitive::corridor(
            origin + Vec2::from_angle(a0) * reach,
            origin + Vec2::from_angle(a1) * reach,
            4.0,
            false,
        ));
    }
    primitives
}

#[cfg(test)]
mod geometry_tests {
    use super::*;

    #[test]
    fn circle_sweep_stops_at_rectangle_face_and_tangent_circle() {
        let projectile = Collider::circle(6.0);
        let face = collider_time_of_impact(
            &projectile,
            Vec2::ZERO,
            Vec2::X,
            100.0,
            Vec2::new(40.0, 0.0),
            0.0,
            crate::map::MapShape::Rectangle {
                half_extents: Vec2::new(10.0, 10.0),
            },
        );
        assert!(face.is_some_and(|distance| (distance - 24.0).abs() < 0.01));
        let tangent = collider_time_of_impact(
            &projectile,
            Vec2::ZERO,
            Vec2::X,
            100.0,
            Vec2::new(40.0, 10.0),
            0.0,
            crate::map::MapShape::Circle { radius: 4.0 },
        );
        assert!(tangent.is_some_and(|distance| (distance - 40.0).abs() < 0.01));
    }

    #[test]
    fn perimeter_insets_by_projectile_radius() {
        let bounds = crate::map::AxisAlignedMapRect {
            min: Vec2::splat(-50.0),
            max: Vec2::splat(50.0),
        };
        assert_eq!(
            distance_to_perimeter(bounds, Vec2::ZERO, Vec2::X, 6.0, 100.0),
            Some(44.0)
        );
    }

    #[test]
    fn trace_uses_first_static_and_dynamic_body_contact() {
        let dimensions = crate::map::MapDimensions {
            width: 20,
            height: 20,
        };
        let blocker = IndexedMapBlocker {
            placement_id: crate::map::MapPlacementId(9),
            position: Vec2::new(40.0, 0.0),
            shape: crate::map::MapShape::Rectangle {
                half_extents: Vec2::new(10.0, 10.0),
            },
        };
        let mut index = AimTraceBlockerIndex {
            dimensions: Some(dimensions),
            blockers: vec![blocker],
            ..Default::default()
        };
        let (min, max) = shape_aabb(blocker.position, blocker.shape, 0.0);
        AimTraceBlockerIndex::for_each_bucket(Some(dimensions), min, max, |bucket| {
            index.buckets.entry(bucket).or_default().push(0);
        });
        let static_hit = trace_projectile_body(
            &index,
            ProjectileBody::circle(6.0),
            Vec2::ZERO,
            Vec2::X,
            100.0,
            &[],
        );
        assert!(static_hit.blocked);
        assert!((static_hit.distance - 24.0).abs() < 0.01);

        let dynamic_hit = trace_projectile_body(
            &AimTraceBlockerIndex {
                dimensions: Some(dimensions),
                ..Default::default()
            },
            ProjectileBody::circle(6.0),
            Vec2::ZERO,
            Vec2::X,
            100.0,
            &[AimTraceDynamicBlocker {
                class: AimTraceBlockerClass::Fighter,
                stable_id: 1,
                position: Vec2::new(40.0, 0.0),
                rotation: 0.0,
                shape: crate::map::MapShape::Circle { radius: 14.0 },
            }],
        );
        assert!(dynamic_hit.blocked);
        assert!((dynamic_hit.distance - 20.0).abs() < 0.01);
    }

    #[test]
    fn projectile_index_ignores_water_and_tracks_removal_revision() {
        let catalog = crate::map::MapContentCatalog::embedded().expect("embedded map catalog");
        let mut snapshot = catalog
            .resolve_preset(crate::map::MapPresetId(1), crate::map::MapInstanceId(92))
            .expect("resolved fixture")
            .snapshot;
        let placement_id = crate::map::MapPlacementId(1);
        snapshot.placements = vec![crate::map::MapAssetPlacement {
            placement_id,
            cell: crate::map::MapCell::new(2, 2),
            asset_id: crate::map::WALL_DUNGEON_ASSET,
            quarter_turns: 0,
            parameters: crate::map::MapPlacementParameters::None,
        }];
        let mut state = crate::map::MapDynamicState {
            map_instance_id: snapshot.identity.instance_id,
            generation: 1,
            revision: 0,
            terminal_states: Vec::new(),
        };
        let mut index = AimTraceBlockerIndex::default();
        index.refresh(&snapshot, &state, &catalog);
        assert_eq!(index.blockers.len(), 1);

        snapshot.placements[0].asset_id = crate::map::WATER_ASSET;
        state.revision = 1;
        index.refresh(&snapshot, &state, &catalog);
        assert!(index.blockers.is_empty());

        snapshot.placements[0].asset_id = crate::map::WALL_DUNGEON_ASSET;
        state.revision = 2;
        state.terminal_states = vec![crate::map::MapPlacementTransition {
            placement_id,
            outcome: crate::map::MapPlacementOutcome::Removed,
        }];
        index.refresh(&snapshot, &state, &catalog);
        assert!(index.blockers.is_empty());
    }

    #[test]
    fn maximum_dimension_full_grass_map_keeps_projectile_index_sparse() {
        let catalog = crate::map::MapContentCatalog::embedded().expect("embedded map catalog");
        let mut snapshot = catalog
            .resolve_preset(crate::map::MapPresetId(1), crate::map::MapInstanceId(91))
            .expect("resolved fixture")
            .snapshot;
        snapshot.dimensions = crate::map::MapDimensions {
            width: crate::map::MAX_MAP_DIMENSION_CELLS,
            height: crate::map::MAX_MAP_DIMENSION_CELLS,
        };
        let width = snapshot.dimensions.width;
        let height = snapshot.dimensions.height;
        snapshot.placements = (0..height)
            .flat_map(|y| {
                (0..width).map(move |x| crate::map::MapAssetPlacement {
                    placement_id: crate::map::MapPlacementId(
                        u32::from(y) * u32::from(width) + u32::from(x) + 1,
                    ),
                    cell: crate::map::MapCell::new(x, y),
                    asset_id: crate::map::TALL_GRASS_ASSET,
                    quarter_turns: 0,
                    parameters: crate::map::MapPlacementParameters::None,
                })
            })
            .collect();
        snapshot.mode_anchors.clear();
        let state = crate::map::MapDynamicState {
            map_instance_id: snapshot.identity.instance_id,
            generation: 1,
            revision: 0,
            terminal_states: Vec::new(),
        };
        let mut index = AimTraceBlockerIndex::default();
        index.refresh(&snapshot, &state, &catalog);
        assert_eq!(snapshot.placements.len(), 512 * 512);
        assert!(index.blockers.is_empty());
        assert!(index.buckets.is_empty());
    }

    #[test]
    fn maximum_dimension_diagonal_trace_visits_a_corridor_not_the_map_area() {
        let dimensions = crate::map::MapDimensions {
            width: crate::map::MAX_MAP_DIMENSION_CELLS,
            height: crate::map::MAX_MAP_DIMENSION_CELLS,
        };
        let bounds = dimensions.bounds();
        let index = AimTraceBlockerIndex {
            dimensions: Some(dimensions),
            ..Default::default()
        };
        let buckets = index.swept_buckets(
            bounds.min + Vec2::splat(8.0),
            bounds.max - Vec2::splat(8.0),
            6.0,
        );
        assert!(buckets.len() < usize::from(dimensions.width) * 8);
        assert!(
            buckets.len() < usize::from(dimensions.width) * usize::from(dimensions.height) / 32
        );
    }
}
