//! Weapon preview geometry for the controlled fighter.

use super::{
    ConeSprayState, DeliveryMethod, FiringPattern, PersistentAreaShape, ProjectileBody,
    ResolvedWeapon, TargetSelection, delivery, spread_angles,
};
use avian2d::{collision::collider::contact_query::time_of_impact, prelude::Collider};
use bevy::prelude::*;
use std::collections::HashMap;

/// Rays plus rim segments of one live cone-spray surface.
pub const MAX_CONE_SPRAY_SEGMENTS: usize = CONE_SPRAY_RAYS * 2 - 1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PreviewGeometry {
    Corridor {
        center: Vec2,
        angle: f32,
        length: f32,
        width: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewPrimitive {
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AimTraceBlockerClass {
    Fighter,
    Sentry,
    HostileNpc,
    HeistSafe,
}

/// One currently replicated dynamic body eligible to stop the local player's next straight shot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AimTraceDynamicBlocker {
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
pub struct AimTraceBlockerIndex {
    key: Option<AimTraceIndexKey>,
    dimensions: Option<crate::map::MapDimensions>,
    blockers: Vec<IndexedMapBlocker>,
    buckets: HashMap<u32, Vec<usize>>,
}

impl AimTraceBlockerIndex {
    pub fn refresh(
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
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the floored cell index is clamped to 0..count-1 of a u16 map dimension before narrowing"
        )]
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
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the floored cell index is clamped to 0..count-1 of a u16 map dimension before narrowing"
        )]
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

/// Distance a straight projectile body travels from `start` before effective projectile-blocking
/// map geometry or the playable perimeter stops it. Fighters and deployables are ignored: this is
/// presentation geometry for cosmetic flight only, never hit detection.
#[must_use]
pub fn straight_map_travel(
    index: &AimTraceBlockerIndex,
    body: ProjectileBody,
    start: Vec2,
    direction: Vec2,
    maximum_distance: f32,
) -> f32 {
    trace_projectile_body(index, body, start, direction, maximum_distance, &[]).distance
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

/// How an aim mark reads: an open shot, one stopped early by a blocker or an unrepairable landing,
/// a targeted ultimate, or the Demolition Strike ultimate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AimTone {
    Clear,
    Blocked,
    Ultimate,
    Demolition,
}

impl AimTone {
    fn from_blocked(blocked: bool) -> Self {
        if blocked { Self::Blocked } else { Self::Clear }
    }
}

/// One renderer-agnostic piece of the aim lane, in ground coordinates.
///
/// Fills are translucent regions fanned from `apex` through consecutive `rim` points, so every
/// fill is star-shaped around its apex and never overlaps itself. Strokes are crisp lines of a
/// world-unit width. Presentation turns the marks into one mesh; nothing here decides a hit.
#[derive(Clone, Debug, PartialEq)]
pub enum AimMark {
    Fill {
        apex: Vec2,
        apex_tone: AimTone,
        rim: Vec<(Vec2, AimTone)>,
    },
    Stroke {
        points: Vec<Vec2>,
        closed: bool,
        width: f32,
        tone: AimTone,
    },
}

impl AimMark {
    fn stroke(points: Vec<Vec2>, closed: bool, width: f32, tone: AimTone) -> Self {
        Self::Stroke {
            points,
            closed,
            width,
            tone,
        }
    }

    #[must_use]
    #[cfg(test)]
    pub(crate) fn is_finite(&self) -> bool {
        match self {
            Self::Fill { apex, rim, .. } => {
                apex.is_finite() && rim.len() >= 2 && rim.iter().all(|(point, _)| point.is_finite())
            }
            Self::Stroke { points, width, .. } => {
                points.len() >= 2
                    && points.iter().all(|point| point.is_finite())
                    && width.is_finite()
                    && *width > 0.0
            }
        }
    }
}

/// Outline width of every aim fill, in world units.
pub(crate) const AIM_OUTLINE_WIDTH: f32 = 2.5;
/// Width of a lob or ultimate guide from the fighter to the landing point.
pub(crate) const AIM_GUIDE_WIDTH: f32 = 3.0;
/// Narrowest drawn delivery path; a real projectile body narrower than this still reads.
pub(crate) const AIM_MIN_PATH_WIDTH: f32 = 3.0;
/// Rim of a targeted ultimate's area, thicker than a weapon outline so the two never read alike.
pub(crate) const ULTIMATE_RIM_WIDTH: f32 = 4.5;
const AREA_RIM_POINTS: usize = 40;
const SECTOR_RIM_POINTS: usize = 16;
const CONE_SPRAY_RAYS: usize = 17;

/// The aim lane for the controlled fighter's primary weapon, derived from its resolved delivery.
///
/// Straight and sticky deliveries fill the fighter hit lane: the projectile path from the fighter
/// widened by the projectile radius plus `fighter_radius` on each side, so a fighter standing in
/// the lane is in the path. Each delivery also gets an exact-width path stroke ending at its first
/// client-observed contact. Lobbed and Splash deliveries show a guide and their authored landing
/// area; melee and cone sprays fill their authored sector.
#[allow(
    clippy::too_many_arguments,
    reason = "the aim lane consumes the complete authored delivery and current blocker view"
)]
#[must_use]
pub fn aim_lane_marks(
    origin: Vec2,
    facing: f32,
    aim_distance: Option<f32>,
    resolved: &ResolvedWeapon,
    fighter_radius: f32,
    map: &crate::map::ResolvedMapSnapshot,
    state: &crate::map::MapDynamicState,
    catalog: &crate::map::MapContentCatalog,
    index: &AimTraceBlockerIndex,
    dynamic: &[AimTraceDynamicBlocker],
) -> Vec<AimMark> {
    match resolved.recipe.delivery {
        DeliveryMethod::Straight { .. } | DeliveryMethod::StickyStraight { .. } => {
            straight_lane_marks(origin, facing, resolved, fighter_radius, index, dynamic)
        }
        DeliveryMethod::Lobbed {
            distance,
            landing_clearance_radius,
            ..
        } => lobbed_lane_marks(
            origin,
            facing,
            aim_distance,
            distance,
            landing_clearance_radius,
            resolved,
            map,
            state,
            catalog,
            None,
        ),
        DeliveryMethod::Splash {
            distance,
            landing_clearance_radius,
            shape,
            ..
        } => lobbed_lane_marks(
            origin,
            facing,
            aim_distance,
            distance,
            landing_clearance_radius,
            resolved,
            map,
            state,
            catalog,
            Some(shape),
        ),
        DeliveryMethod::MeleeArc {
            reach,
            angle_degrees,
        } => sector_marks(origin, facing, reach, angle_degrees),
        DeliveryMethod::ConeSpray {
            reach,
            angle_degrees,
            map_occlusion,
            ..
        } => {
            let rays = cone_spray_rays(
                origin,
                facing,
                reach,
                angle_degrees,
                map_occlusion,
                index,
                dynamic,
            );
            let rim: Vec<_> = rays
                .iter()
                .map(|ray| (ray.stop_center, AimTone::from_blocked(ray.blocked)))
                .collect();
            let mut outline = Vec::with_capacity(rim.len() + 1);
            outline.push(origin);
            outline.extend(rim.iter().map(|(point, _)| *point));
            vec![
                AimMark::Fill {
                    apex: origin,
                    apex_tone: AimTone::Clear,
                    rim,
                },
                AimMark::stroke(outline, true, AIM_OUTLINE_WIDTH, AimTone::Clear),
            ]
        }
    }
}

/// A targeted ultimate's preview: a guide to the clamped center, the authored area, and a
/// thicker rim in the ultimate tone.
#[must_use]
pub fn ultimate_area_marks(
    origin: Vec2,
    center: Vec2,
    radius: f32,
    demolition: bool,
) -> Vec<AimMark> {
    let tone = if demolition {
        AimTone::Demolition
    } else {
        AimTone::Ultimate
    };
    let rim = circle_points(center, radius);
    vec![
        AimMark::Fill {
            apex: center,
            apex_tone: tone,
            rim: closed_rim(&rim, tone),
        },
        AimMark::stroke(vec![origin, center], false, AIM_GUIDE_WIDTH, tone),
        AimMark::stroke(rim, true, ULTIMATE_RIM_WIDTH, tone),
    ]
}

/// One traced straight delivery: where its body starts and stops along `direction`.
struct StraightShot {
    direction: Vec2,
    start: Vec2,
    stop: Vec2,
    blocked: bool,
}

/// Traces every delivery of a straight or sticky weapon from the fighter: first the muzzle
/// offset against Heist safes, then the authored range against all blockers.
fn trace_straight_shots(
    origin: Vec2,
    facing: f32,
    resolved: &ResolvedWeapon,
    index: &AimTraceBlockerIndex,
    dynamic: &[AimTraceDynamicBlocker],
) -> Vec<StraightShot> {
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
    angles
        .into_iter()
        .map(|angle| {
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
                return StraightShot {
                    direction,
                    start: origin,
                    stop: muzzle_trace.stop_center,
                    blocked: true,
                };
            }
            let muzzle = origin + direction * muzzle_offset;
            let travel = trace_projectile_body(index, body, muzzle, direction, range, dynamic);
            StraightShot {
                direction,
                start: muzzle,
                stop: travel.stop_center,
                blocked: travel.blocked,
            }
        })
        .collect()
}

fn straight_lane_marks(
    origin: Vec2,
    facing: f32,
    resolved: &ResolvedWeapon,
    fighter_radius: f32,
    index: &AimTraceBlockerIndex,
    dynamic: &[AimTraceDynamicBlocker],
) -> Vec<AimMark> {
    let (DeliveryMethod::Straight { radius, .. } | DeliveryMethod::StickyStraight { radius, .. }) =
        resolved.recipe.delivery
    else {
        return Vec::new();
    };
    let shots = trace_straight_shots(origin, facing, resolved, index, dynamic);
    let (Some(first), Some(last)) = (shots.first(), shots.last()) else {
        return Vec::new();
    };
    let half_width = radius + fighter_radius;
    let side = |shot: &StraightShot| shot.direction.perp() * half_width;
    // Ordered by angle around the fighter: the clockwise-most side, every stop, then back. The
    // fill starts beside the fighter so the lane reads as leaving its body; the exact path strokes
    // below still start at the muzzle.
    let mut rim = Vec::with_capacity(shots.len() + 4);
    let first_tone = AimTone::from_blocked(first.blocked);
    let last_tone = AimTone::from_blocked(last.blocked);
    rim.push((origin - side(first), first_tone));
    rim.push((first.stop - side(first), first_tone));
    if shots.len() > 1 {
        rim.extend(
            shots
                .iter()
                .map(|shot| (shot.stop, AimTone::from_blocked(shot.blocked))),
        );
    }
    rim.push((last.stop + side(last), last_tone));
    rim.push((origin + side(last), last_tone));
    let all_blocked = shots.iter().all(|shot| shot.blocked);
    let outline_tone = AimTone::from_blocked(all_blocked);
    let mut marks = Vec::with_capacity(shots.len() + 2);
    marks.push(AimMark::Fill {
        apex: origin,
        apex_tone: outline_tone,
        rim: rim.clone(),
    });
    marks.push(AimMark::stroke(
        rim.into_iter().map(|(point, _)| point).collect(),
        false,
        AIM_OUTLINE_WIDTH,
        outline_tone,
    ));
    marks.extend(
        shots
            .iter()
            .filter(|shot| shot.start != shot.stop)
            .map(|shot| {
                AimMark::stroke(
                    vec![shot.start, shot.stop],
                    false,
                    (radius * 2.0).max(AIM_MIN_PATH_WIDTH),
                    AimTone::from_blocked(shot.blocked),
                )
            }),
    );
    marks
}

#[allow(
    clippy::too_many_arguments,
    reason = "lobbed preview repair consumes the complete authored and current map geometry view"
)]
fn lobbed_lane_marks(
    origin: Vec2,
    facing: f32,
    aim_distance: Option<f32>,
    distance: f32,
    landing_clearance_radius: f32,
    resolved: &ResolvedWeapon,
    map: &crate::map::ResolvedMapSnapshot,
    state: &crate::map::MapDynamicState,
    catalog: &crate::map::MapContentCatalog,
    persistent_shape: Option<PersistentAreaShape>,
) -> Vec<AimMark> {
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
    let tone = AimTone::from_blocked(repaired_landing.is_none());
    let shape = persistent_shape.unwrap_or_else(|| {
        let radius = resolved
            .recipe
            .payload_bundles
            .iter()
            .find_map(|bundle| match bundle.target {
                TargetSelection::Area { radius, .. } => Some(radius),
                TargetSelection::Direct => None,
            })
            .unwrap_or(24.0);
        PersistentAreaShape::Circle { radius }
    });
    let outline = match shape {
        PersistentAreaShape::Circle { radius } => circle_points(landing, radius),
        PersistentAreaShape::Rectangle {
            half_extents: [x, y],
        } => {
            let rotation = Mat2::from_angle(facing);
            [
                Vec2::new(-x, -y),
                Vec2::new(x, -y),
                Vec2::new(x, y),
                Vec2::new(-x, y),
            ]
            .map(|corner| rotation * corner + landing)
            .to_vec()
        }
    };
    let mut marks = Vec::with_capacity(3);
    if landing.distance(origin) > f32::EPSILON {
        marks.push(AimMark::stroke(
            vec![origin, landing],
            false,
            AIM_GUIDE_WIDTH,
            tone,
        ));
    }
    marks.push(AimMark::Fill {
        apex: landing,
        apex_tone: tone,
        rim: closed_rim(&outline, tone),
    });
    marks.push(AimMark::stroke(outline, true, AIM_OUTLINE_WIDTH, tone));
    marks
}

#[expect(
    clippy::cast_precision_loss,
    reason = "rim/ray indices are bounded by small code-owned preview point counts"
)]
fn sector_marks(origin: Vec2, facing: f32, reach: f32, angle_degrees: f32) -> Vec<AimMark> {
    let angle = angle_degrees.to_radians();
    let arc: Vec<_> = (0..=SECTOR_RIM_POINTS)
        .map(|index| {
            let progress = index as f32 / SECTOR_RIM_POINTS as f32;
            origin + Vec2::from_angle(facing - angle * 0.5 + angle * progress) * reach
        })
        .collect();
    let mut outline = Vec::with_capacity(arc.len() + 1);
    outline.push(origin);
    outline.extend_from_slice(&arc);
    vec![
        AimMark::Fill {
            apex: origin,
            apex_tone: AimTone::Clear,
            rim: arc
                .into_iter()
                .map(|point| (point, AimTone::Clear))
                .collect(),
        },
        AimMark::stroke(outline, true, AIM_OUTLINE_WIDTH, AimTone::Clear),
    ]
}

#[expect(
    clippy::cast_precision_loss,
    reason = "rim/ray indices are bounded by small code-owned preview point counts"
)]
fn circle_points(center: Vec2, radius: f32) -> Vec<Vec2> {
    (0..AREA_RIM_POINTS)
        .map(|index| {
            center
                + Vec2::from_angle(std::f32::consts::TAU * index as f32 / AREA_RIM_POINTS as f32)
                    * radius
        })
        .collect()
}

/// A fan rim around a closed outline, repeating the first point so the fan closes.
fn closed_rim(outline: &[Vec2], tone: AimTone) -> Vec<(Vec2, AimTone)> {
    outline
        .iter()
        .chain(outline.first())
        .map(|point| (*point, tone))
        .collect()
}

#[must_use]
pub fn cone_spray_primitives(
    state: ConeSprayState,
    tick: u64,
    index: &AimTraceBlockerIndex,
    dynamic: &[AimTraceDynamicBlocker],
) -> Vec<PreviewPrimitive> {
    cone_spray_preview_primitives(
        state.origin.as_vec2(),
        state.facing,
        state.reached_distance(tick),
        state.angle_degrees,
        state.map_occlusion,
        index,
        dynamic,
    )
}

/// Traced cone-spray rays from the clockwise edge to the counter-clockwise edge. Only Heist safes
/// join map geometry as occluders, matching the authoritative spray.
#[expect(
    clippy::cast_precision_loss,
    reason = "rim/ray indices are bounded by small code-owned preview point counts"
)]
fn cone_spray_rays(
    origin: Vec2,
    facing: f32,
    reach: f32,
    angle_degrees: f32,
    map_occlusion: bool,
    index: &AimTraceBlockerIndex,
    dynamic: &[AimTraceDynamicBlocker],
) -> Vec<AimTraceResult> {
    let half_angle = angle_degrees.to_radians() * 0.5;
    let geometry_blockers = dynamic
        .iter()
        .copied()
        .filter(|blocker| blocker.class == AimTraceBlockerClass::HeistSafe)
        .collect::<Vec<_>>();
    (0..CONE_SPRAY_RAYS)
        .map(|ray| {
            let progress = ray as f32 / (CONE_SPRAY_RAYS - 1) as f32;
            let angle = facing - half_angle + angle_degrees.to_radians() * progress;
            let direction = Vec2::from_angle(angle);
            if map_occlusion {
                trace_projectile_body(
                    index,
                    ProjectileBody::circle(0.5),
                    origin,
                    direction,
                    reach,
                    &geometry_blockers,
                )
            } else {
                AimTraceResult {
                    stop_center: origin + direction * reach,
                    distance: reach,
                    blocked: false,
                }
            }
        })
        .collect()
}

#[expect(
    clippy::cast_precision_loss,
    reason = "rim/ray indices are bounded by small code-owned preview point counts"
)]
fn cone_spray_preview_primitives(
    origin: Vec2,
    facing: f32,
    reach: f32,
    angle_degrees: f32,
    map_occlusion: bool,
    index: &AimTraceBlockerIndex,
    dynamic: &[AimTraceDynamicBlocker],
) -> Vec<PreviewPrimitive> {
    let rays = cone_spray_rays(
        origin,
        facing,
        reach,
        angle_degrees,
        map_occlusion,
        index,
        dynamic,
    );
    let mut primitives = Vec::with_capacity(CONE_SPRAY_RAYS * 2 - 1);
    for trace in &rays {
        primitives.push(PreviewPrimitive::corridor(
            origin,
            trace.stop_center,
            (trace.distance * angle_degrees.to_radians() / (CONE_SPRAY_RAYS - 1) as f32).max(4.0),
            trace.blocked,
        ));
    }
    for pair in rays.windows(2) {
        primitives.push(PreviewPrimitive::corridor(
            pair[0].stop_center,
            pair[1].stop_center,
            3.0,
            pair[0].blocked || pair[1].blocked,
        ));
    }
    primitives
}

#[cfg(test)]
mod geometry_tests {
    use super::*;
    use crate::combat::{WeaponCatalog, WeaponPresetId};

    const FIGHTER_RADIUS: f32 = 14.0;

    fn resolved(id: u16) -> ResolvedWeapon {
        let body = crate::builds::BuildCatalog::embedded()
            .expect("embedded build catalog")
            .fighter_body;
        assert!((body.radius - FIGHTER_RADIUS).abs() < f32::EPSILON);
        WeaponCatalog::embedded()
            .expect("embedded weapon catalog")
            .resolve_preset(WeaponPresetId(id), body)
            .expect("embedded weapon preset")
    }

    /// The aim lane from the origin facing +X in an open 64-cell field with the given blockers.
    fn lane(
        id: u16,
        aim_distance: Option<f32>,
        dynamic: &[AimTraceDynamicBlocker],
    ) -> Vec<AimMark> {
        let catalog = crate::map::MapContentCatalog::embedded().expect("embedded map catalog");
        let map = catalog
            .resolve_preset(crate::map::MapPresetId(1), crate::map::MapInstanceId(1))
            .expect("resolved fixture");
        let state = crate::map::MapDynamicState {
            map_instance_id: crate::map::MapInstanceId(1),
            generation: 1,
            revision: 0,
            terminal_states: Vec::new(),
        };
        let index = AimTraceBlockerIndex {
            dimensions: Some(crate::map::MapDimensions {
                width: 64,
                height: 64,
            }),
            ..Default::default()
        };
        aim_lane_marks(
            Vec2::ZERO,
            0.0,
            aim_distance,
            &resolved(id),
            FIGHTER_RADIUS,
            &map.snapshot,
            &state,
            &catalog,
            &index,
            dynamic,
        )
    }

    fn fill(mark: &AimMark) -> (Vec2, &[(Vec2, AimTone)]) {
        let AimMark::Fill { apex, rim, .. } = mark else {
            panic!("expected a fill, got {mark:?}");
        };
        (*apex, rim)
    }

    fn stroke(mark: &AimMark) -> (&[Vec2], bool, f32, AimTone) {
        let AimMark::Stroke {
            points,
            closed,
            width,
            tone,
        } = mark
        else {
            panic!("expected a stroke, got {mark:?}");
        };
        (points, *closed, *width, *tone)
    }

    fn close(a: Vec2, b: Vec2) -> bool {
        a.distance(b) < 0.01
    }

    fn straight(id: u16) -> (f32, f32, f32) {
        match resolved(id).recipe.delivery {
            DeliveryMethod::Straight {
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
            } => (radius, range, muzzle_offset),
            other => panic!("weapon {id} is not straight: {other:?}"),
        }
    }

    #[test]
    fn single_straight_lanes_fill_the_fighter_hit_lane_over_the_authored_range() {
        // Pulse Sidearm (Straight) and Sticky Blomb (StickyStraight).
        for id in [1, 5] {
            let (radius, range, muzzle_offset) = straight(id);
            let marks = lane(id, None, &[]);
            assert_eq!(marks.len(), 3, "fill, outline, one path");
            let (apex, rim) = fill(&marks[0]);
            let half_width = radius + FIGHTER_RADIUS;
            let muzzle = Vec2::X * muzzle_offset;
            let end = Vec2::X * (muzzle_offset + range);
            assert!(close(apex, Vec2::ZERO));
            let corners: Vec<_> = rim.iter().map(|(point, _)| *point).collect();
            assert_eq!(corners.len(), 4);
            assert!(close(corners[0], -Vec2::Y * half_width));
            assert!(close(corners[1], end - Vec2::Y * half_width));
            assert!(close(corners[2], end + Vec2::Y * half_width));
            assert!(close(corners[3], Vec2::Y * half_width));
            assert!(rim.iter().all(|(_, tone)| *tone == AimTone::Clear));
            let (points, closed, width, tone) = stroke(&marks[2]);
            assert!(!closed);
            assert!(close(points[0], muzzle) && close(points[1], end));
            assert!((width - (radius * 2.0).max(AIM_MIN_PATH_WIDTH)).abs() < f32::EPSILON);
            assert_eq!(tone, AimTone::Clear);
        }
        // Pulse is exactly one cell wide against a fighter.
        assert!(
            ((straight(1).0 + FIGHTER_RADIUS) * 2.0 - crate::map::MAP_CELL_SIZE_WORLD).abs()
                < 0.001
        );
    }

    #[test]
    fn spread_lanes_fan_over_the_authored_angle_and_draw_every_delivery() {
        let (radius, range, muzzle_offset) = straight(2);
        let FiringPattern::Spread {
            delivery_count,
            total_angle_degrees,
        } = resolved(2).recipe.firing
        else {
            panic!("Scatter Cannon spreads");
        };
        let marks = lane(2, None, &[]);
        assert_eq!(marks.len(), 2 + usize::from(delivery_count));
        let angles = spread_angles(0.0, delivery_count, total_angle_degrees);
        for (mark, angle) in marks[2..].iter().zip(&angles) {
            let direction = Vec2::from_angle(*angle);
            let (points, _, _, _) = stroke(mark);
            assert!(close(points[0], direction * muzzle_offset));
            assert!(close(points[1], direction * (muzzle_offset + range)));
        }
        let (_, rim) = fill(&marks[0]);
        assert_eq!(rim.len(), usize::from(delivery_count) + 4);
        let half_width = radius + FIGHTER_RADIUS;
        let outer = Vec2::from_angle(-total_angle_degrees.to_radians() * 0.5);
        assert!(close(rim[0].0, -outer.perp() * half_width));
        assert!(close(
            rim[1].0,
            outer * (muzzle_offset + range) - outer.perp() * half_width
        ));
    }

    #[test]
    fn blocked_straight_deliveries_stop_and_turn_the_lane_blocked() {
        let blocker = AimTraceDynamicBlocker {
            class: AimTraceBlockerClass::Fighter,
            stable_id: 7,
            position: Vec2::new(150.0, 0.0),
            rotation: 0.0,
            shape: crate::map::MapShape::Circle {
                radius: FIGHTER_RADIUS,
            },
        };
        let (radius, _, _) = straight(1);
        let marks = lane(1, None, &[blocker]);
        let (_, rim) = fill(&marks[0]);
        assert!(rim.iter().all(|(_, tone)| *tone == AimTone::Blocked));
        let (points, _, _, tone) = stroke(&marks[2]);
        assert_eq!(tone, AimTone::Blocked);
        assert!((points[1].x - (150.0 - FIGHTER_RADIUS - radius)).abs() < 0.01);
    }

    #[test]
    fn lobbed_and_splash_lanes_show_the_authored_landing_area() {
        let area_radius = resolved(3)
            .recipe
            .payload_bundles
            .iter()
            .find_map(|bundle| match bundle.target {
                TargetSelection::Area { radius, .. } => Some(radius),
                TargetSelection::Direct => None,
            })
            .expect("Arc Launcher has an area payload");
        let DeliveryMethod::Splash {
            shape:
                PersistentAreaShape::Circle {
                    radius: splash_radius,
                },
            ..
        } = resolved(7).recipe.delivery
        else {
            panic!("Splash lands a circle");
        };
        for (id, radius) in [(3, area_radius), (7, splash_radius)] {
            let marks = lane(id, Some(180.0), &[]);
            assert_eq!(marks.len(), 3, "guide, area fill, area outline");
            let landing = Vec2::X * 180.0;
            let (guide, _, guide_width, _) = stroke(&marks[0]);
            assert!(close(guide[0], Vec2::ZERO) && close(guide[1], landing));
            assert!((guide_width - AIM_GUIDE_WIDTH).abs() < f32::EPSILON);
            let (apex, rim) = fill(&marks[1]);
            assert!(close(apex, landing));
            assert!(close(rim[0].0, rim[rim.len() - 1].0), "the area fan closes");
            assert!(
                rim.iter()
                    .all(|(point, _)| (point.distance(landing) - radius).abs() < 0.01)
            );
            let (outline, closed, _, _) = stroke(&marks[2]);
            assert!(closed && outline.len() == AREA_RIM_POINTS);
        }
    }

    #[test]
    fn melee_and_cone_spray_fill_their_authored_sector() {
        for id in [4, 6] {
            let (reach, angle_degrees) = match resolved(id).recipe.delivery {
                DeliveryMethod::MeleeArc {
                    reach,
                    angle_degrees,
                }
                | DeliveryMethod::ConeSpray {
                    reach,
                    angle_degrees,
                    ..
                } => (reach, angle_degrees),
                other => panic!("weapon {id} is not a sector: {other:?}"),
            };
            let marks = lane(id, None, &[]);
            assert_eq!(marks.len(), 2, "sector fill and outline");
            let (apex, rim) = fill(&marks[0]);
            assert!(close(apex, Vec2::ZERO));
            assert!(
                rim.iter()
                    .all(|(point, _)| (point.length() - reach).abs() < 0.01)
            );
            let half = angle_degrees.to_radians() * 0.5;
            let first = rim[0].0;
            let last = rim[rim.len() - 1].0;
            assert!((first.y.atan2(first.x) + half).abs() < 0.001);
            assert!((last.y.atan2(last.x) - half).abs() < 0.001);
            let (outline, closed, _, _) = stroke(&marks[1]);
            assert!(closed && close(outline[0], Vec2::ZERO));
        }
    }

    #[test]
    fn targeted_ultimates_use_a_distinct_tone_and_a_thicker_rim() {
        let center = Vec2::new(200.0, 40.0);
        for (demolition, tone) in [(false, AimTone::Ultimate), (true, AimTone::Demolition)] {
            let marks = ultimate_area_marks(Vec2::ZERO, center, 120.0, demolition);
            let (apex, rim) = fill(&marks[0]);
            assert!(close(apex, center));
            assert!(rim.iter().all(|(point, rim_tone)| {
                (point.distance(center) - 120.0).abs() < 0.01 && *rim_tone == tone
            }));
            let (guide, _, _, guide_tone) = stroke(&marks[1]);
            assert!(close(guide[1], center) && guide_tone == tone);
            let (_, closed, width, _) = stroke(&marks[2]);
            assert!(closed && width > AIM_OUTLINE_WIDTH);
            assert!((width - ULTIMATE_RIM_WIDTH).abs() < f32::EPSILON);
        }
    }

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
    fn cone_spray_clips_only_blocked_rays_and_reopens_after_dynamic_removal() {
        let index = AimTraceBlockerIndex {
            dimensions: Some(crate::map::MapDimensions {
                width: 20,
                height: 20,
            }),
            ..Default::default()
        };
        let blocker = AimTraceDynamicBlocker {
            class: AimTraceBlockerClass::HeistSafe,
            stable_id: 1,
            position: Vec2::new(45.0, 0.0),
            rotation: 0.0,
            shape: crate::map::MapShape::Rectangle {
                half_extents: Vec2::new(8.0, 10.0),
            },
        };
        let clipped =
            cone_spray_preview_primitives(Vec2::ZERO, 0.0, 100.0, 70.0, true, &index, &[blocker]);
        let reopened =
            cone_spray_preview_primitives(Vec2::ZERO, 0.0, 100.0, 70.0, true, &index, &[]);
        let corridor_length = |primitive: PreviewPrimitive| {
            let PreviewGeometry::Corridor { length, .. } = primitive.geometry;
            length
        };

        assert!(clipped[8].blocked, "the center ray crosses the blocker");
        assert!(corridor_length(clipped[8]) < 40.0);
        assert!(!clipped[0].blocked, "the neighboring edge ray stays open");
        assert!((corridor_length(clipped[0]) - 100.0).abs() < 0.01);
        assert!(!reopened[8].blocked);
        assert!((corridor_length(reopened[8]) - 100.0).abs() < 0.01);
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
