//! Pure map placement, collision, relaxation, navigation, and access geometry.

#[cfg(feature = "client")]
use super::resolution::ResolvedMapCollider;
use super::{
    AxisAlignedMapRect, MAP_CELL_SIZE_WORLD, MapAssetDefinition, MapAssetPlacement, MapAssetSlot,
    MapCell, MapColliderShape, MapContentCatalog, MapDimensions, MapDynamicState, MapFootprint,
    MapGameplayProfile, MapHalfCellPoint, MapModeAnchorKind, MapModeAnchorPlacement,
    MapPlacementOutcome, MapPlacementParameters, PlayerCollision, ResolvedMapSnapshot,
};
#[cfg(feature = "client")]
use super::{MapShape, ProjectileCollision};
use bevy::prelude::Vec2;
use std::collections::BTreeSet;

impl MapDimensions {
    #[must_use]
    pub fn world_size(self) -> Vec2 {
        Vec2::new(
            f32::from(self.width) * MAP_CELL_SIZE_WORLD,
            f32::from(self.height) * MAP_CELL_SIZE_WORLD,
        )
    }

    #[must_use]
    pub fn bounds(self) -> AxisAlignedMapRect {
        let half = self.world_size() * 0.5;
        AxisAlignedMapRect {
            min: -half,
            max: half,
        }
    }

    #[must_use]
    pub fn contains(self, cell: MapCell) -> bool {
        cell.x < self.width && cell.y < self.height
    }

    #[must_use]
    pub fn cell_min(self, cell: MapCell) -> Vec2 {
        self.bounds().min + Vec2::new(f32::from(cell.x), f32::from(cell.y)) * MAP_CELL_SIZE_WORLD
    }

    #[must_use]
    pub fn cell_center(self, cell: MapCell) -> Vec2 {
        self.cell_min(cell) + Vec2::splat(MAP_CELL_SIZE_WORLD * 0.5)
    }
}

impl MapFootprint {
    #[must_use]
    pub const fn rotated(self, quarter_turns: u8) -> Self {
        if quarter_turns & 1 == 0 {
            self
        } else {
            Self {
                width: self.height,
                height: self.width,
            }
        }
    }
}

#[must_use]
pub fn placement_cells(
    dimensions: MapDimensions,
    asset: &MapAssetDefinition,
    placement: &MapAssetPlacement,
) -> Option<Vec<MapCell>> {
    let footprint = asset.footprint_cells.rotated(placement.quarter_turns);
    let mut cells =
        Vec::with_capacity(usize::from(footprint.width) * usize::from(footprint.height));
    for y in 0..u16::from(footprint.height) {
        for x in 0..u16::from(footprint.width) {
            let cell = MapCell::new(
                placement.cell.x.checked_add(x)?,
                placement.cell.y.checked_add(y)?,
            );
            if !dimensions.contains(cell) {
                return None;
            }
            cells.push(cell);
        }
    }
    Some(cells)
}

#[must_use]
pub fn placement_world_center(
    dimensions: MapDimensions,
    asset: &MapAssetDefinition,
    placement: &MapAssetPlacement,
) -> Vec2 {
    let footprint = asset.footprint_cells.rotated(placement.quarter_turns);
    dimensions.cell_min(placement.cell)
        + Vec2::new(f32::from(footprint.width), f32::from(footprint.height))
            * (MAP_CELL_SIZE_WORLD * 0.5)
}

/// Four-bit north/east/south/west neighborhood mask used by bounded tile presentation.
#[must_use]
pub fn cardinal_adjacency_mask(cell: MapCell, occupied: &BTreeSet<MapCell>) -> u8 {
    let north = cell
        .y
        .checked_add(1)
        .is_some_and(|y| occupied.contains(&MapCell::new(cell.x, y)));
    let east = cell
        .x
        .checked_add(1)
        .is_some_and(|x| occupied.contains(&MapCell::new(x, cell.y)));
    let south = cell
        .y
        .checked_sub(1)
        .is_some_and(|y| occupied.contains(&MapCell::new(cell.x, y)));
    let west = cell
        .x
        .checked_sub(1)
        .is_some_and(|x| occupied.contains(&MapCell::new(x, cell.y)));
    u8::from(north) | (u8::from(east) << 1) | (u8::from(south) << 2) | (u8::from(west) << 3)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapCellRect {
    pub min: MapCell,
    pub width: u16,
    pub height: u16,
}

pub(super) fn merge_cells_to_rectangles(mut cells: BTreeSet<MapCell>) -> Vec<MapCellRect> {
    let mut rectangles = Vec::new();
    while let Some(start) = cells.first().copied() {
        let mut width = 1_u16;
        while cells.contains(&MapCell::new(start.x + width, start.y)) {
            width += 1;
        }
        let mut height = 1_u16;
        'rows: while let Some(y) = start.y.checked_add(height) {
            for x in start.x..start.x + width {
                if !cells.contains(&MapCell::new(x, y)) {
                    break 'rows;
                }
            }
            height += 1;
        }
        for y in start.y..start.y + height {
            for x in start.x..start.x + width {
                cells.remove(&MapCell::new(x, y));
            }
        }
        rectangles.push(MapCellRect {
            min: start,
            width,
            height,
        });
    }
    rectangles
}

impl MapDimensions {
    #[must_use]
    pub fn half_cell_world(self, point: MapHalfCellPoint) -> Option<Vec2> {
        (u32::from(point.x) <= u32::from(self.width) * 2
            && u32::from(point.y) <= u32::from(self.height) * 2)
            .then(|| {
                self.bounds().min
                    + Vec2::new(f32::from(point.x), f32::from(point.y))
                        * (MAP_CELL_SIZE_WORLD * 0.5)
            })
    }
}

#[derive(Clone, Copy)]
pub(super) enum DerivedColliderShape {
    Rectangle { center: Vec2, half_extents: Vec2 },
    Circle { center: Vec2, radius: f32 },
}

fn placement_collider_shape(
    dimensions: MapDimensions,
    asset: &MapAssetDefinition,
    placement: &MapAssetPlacement,
    profile: MapGameplayProfile,
) -> Option<DerivedColliderShape> {
    let center = placement_world_center(dimensions, asset, placement);
    match profile.collider_shape {
        MapColliderShape::None => None,
        MapColliderShape::FootprintRectangle => {
            let footprint = asset.footprint_cells.rotated(placement.quarter_turns);
            Some(DerivedColliderShape::Rectangle {
                center,
                half_extents: Vec2::new(f32::from(footprint.width), f32::from(footprint.height))
                    * (MAP_CELL_SIZE_WORLD * 0.5),
            })
        }
        MapColliderShape::Circle { radius_world_units } => Some(DerivedColliderShape::Circle {
            center,
            radius: f32::from(radius_world_units),
        }),
    }
}

pub(super) fn circle_overlaps_derived_shape(
    center: Vec2,
    radius: f32,
    shape: DerivedColliderShape,
) -> bool {
    match shape {
        DerivedColliderShape::Rectangle {
            center: obstacle_center,
            half_extents,
        } => {
            let min = obstacle_center - half_extents;
            let max = obstacle_center + half_extents;
            center.distance_squared(center.clamp(min, max)) < radius * radius
        }
        DerivedColliderShape::Circle {
            center: obstacle_center,
            radius: obstacle_radius,
        } => center.distance_squared(obstacle_center) < (radius + obstacle_radius).powi(2),
    }
}

/// Whether the current effective map state blocks a fighter-sized circle at `center`.
#[must_use]
pub fn circle_overlaps_blocking_map(
    center: Vec2,
    radius: f32,
    snapshot: &ResolvedMapSnapshot,
    state: &MapDynamicState,
    catalog: &MapContentCatalog,
) -> bool {
    snapshot.placements.iter().any(|placement| {
        effective_blocking_shape(placement, snapshot, state, catalog)
            .is_some_and(|shape| circle_overlaps_derived_shape(center, radius, shape))
    })
}

/// Clamp and relax a circle against the effective current map blockers.
#[must_use]
pub fn resolve_circle_against_blocking_map(
    position: Vec2,
    radius: f32,
    snapshot: &ResolvedMapSnapshot,
    state: &MapDynamicState,
    catalog: &MapContentCatalog,
) -> Vec2 {
    let bounds = snapshot.dimensions.bounds();
    let mut resolved = bounds.clamp_circle(position, radius);
    for _ in 0..2 {
        for placement in &snapshot.placements {
            if let Some(shape) = effective_blocking_shape(placement, snapshot, state, catalog) {
                resolved = push_circle_out_of_shape(resolved, radius, shape);
            }
        }
        resolved = bounds.clamp_circle(resolved, radius);
    }
    resolved
}

fn effective_blocking_shape(
    placement: &MapAssetPlacement,
    snapshot: &ResolvedMapSnapshot,
    state: &MapDynamicState,
    catalog: &MapContentCatalog,
) -> Option<DerivedColliderShape> {
    let asset_id = state
        .terminal_states
        .binary_search_by_key(&placement.placement_id, |transition| {
            transition.placement_id
        })
        .ok()
        .map_or(Some(placement.asset_id), |index| {
            match state.terminal_states[index].outcome {
                MapPlacementOutcome::Removed => None,
                MapPlacementOutcome::ReplacedWith(asset_id) => Some(asset_id),
            }
        })?;
    let asset = catalog.asset(asset_id)?;
    let profile = catalog.profile(asset.gameplay_profile_id).copied()?;
    if profile.player_collision != PlayerCollision::Block {
        return None;
    }
    let mut effective = placement.clone();
    effective.asset_id = asset_id;
    placement_collider_shape(snapshot.dimensions, asset, &effective, profile)
}

/// Current projectile-blocking collider for one authored placement.
///
/// This mirrors authoritative dynamic replacement/removal resolution while deliberately using
/// projectile collision policy rather than fighter collision policy. Client aim tracing consumes
/// the result as read-only presentation data; authority continues to use installed Avian bodies.
#[cfg(feature = "client")]
pub(crate) fn effective_projectile_collider(
    placement: &MapAssetPlacement,
    snapshot: &ResolvedMapSnapshot,
    state: &MapDynamicState,
    catalog: &MapContentCatalog,
) -> Option<ResolvedMapCollider> {
    let asset_id = state
        .terminal_states
        .binary_search_by_key(&placement.placement_id, |transition| {
            transition.placement_id
        })
        .ok()
        .map_or(Some(placement.asset_id), |index| {
            match state.terminal_states[index].outcome {
                MapPlacementOutcome::Removed => None,
                MapPlacementOutcome::ReplacedWith(asset_id) => Some(asset_id),
            }
        })?;
    let asset = catalog.asset(asset_id)?;
    let profile = catalog.profile(asset.gameplay_profile_id).copied()?;
    if profile.projectile_collision != ProjectileCollision::BlockAndConsume {
        return None;
    }
    let mut effective = placement.clone();
    effective.asset_id = asset_id;
    let shape = placement_collider_shape(snapshot.dimensions, asset, &effective, profile)?;
    let (position, shape) = match shape {
        DerivedColliderShape::Rectangle {
            center,
            half_extents,
        } => (center, MapShape::Rectangle { half_extents }),
        DerivedColliderShape::Circle { center, radius } => (center, MapShape::Circle { radius }),
    };
    Some(ResolvedMapCollider {
        placement_id: placement.placement_id,
        position,
        shape,
    })
}

fn push_circle_out_of_shape(position: Vec2, radius: f32, shape: DerivedColliderShape) -> Vec2 {
    match shape {
        DerivedColliderShape::Circle {
            center,
            radius: obstacle_radius,
        } => {
            let delta = position - center;
            let distance = delta.length();
            let minimum = radius + obstacle_radius;
            if distance >= minimum {
                position
            } else if distance <= f32::EPSILON {
                center + Vec2::X * minimum
            } else {
                center + delta / distance * minimum
            }
        }
        DerivedColliderShape::Rectangle {
            center,
            half_extents,
        } => {
            let local = position - center;
            let closest = local.clamp(-half_extents, half_extents);
            let offset = local - closest;
            let distance = offset.length();
            if distance >= radius {
                return position;
            }
            if distance > f32::EPSILON {
                return center + closest + offset / distance * radius;
            }
            let exit_x = half_extents.x - local.x.abs();
            let exit_y = half_extents.y - local.y.abs();
            if exit_x <= exit_y {
                let direction = if local.x < 0.0 { -1.0 } else { 1.0 };
                center + Vec2::new(direction * (half_extents.x + radius), local.y)
            } else {
                let direction = if local.y < 0.0 { -1.0 } else { 1.0 };
                center + Vec2::new(local.x, direction * (half_extents.y + radius))
            }
        }
    }
}

fn blocking_shapes(
    placements: &[MapAssetPlacement],
    dimensions: MapDimensions,
    catalog: &MapContentCatalog,
) -> Vec<DerivedColliderShape> {
    placements
        .iter()
        .filter_map(|placement| {
            let asset = catalog.asset(placement.asset_id)?;
            let profile = *catalog.profile(asset.gameplay_profile_id)?;
            (profile.player_collision == PlayerCollision::Block)
                .then(|| placement_collider_shape(dimensions, asset, placement, profile))
                .flatten()
        })
        .collect()
}

pub(super) fn validate_spawn_clearance(
    placements: &[MapAssetPlacement],
    dimensions: MapDimensions,
    catalog: &MapContentCatalog,
) -> Result<(), String> {
    let blocked = blocking_shapes(placements, dimensions, catalog);
    for placement in placements {
        if matches!(
            placement.parameters,
            MapPlacementParameters::PlayerSpawn { .. }
        ) {
            let center = dimensions.cell_center(placement.cell);
            if blocked.iter().any(|shape| {
                circle_overlaps_derived_shape(
                    center,
                    crate::movement::STANDARD_FIGHTER_RADIUS,
                    *shape,
                )
            }) {
                return Err("spawn overlaps blocking feature".to_string());
            }
            if !dimensions.bounds().contains_with_inset(center, 32.0) {
                return Err("spawn lacks perimeter clearance".to_string());
            }
        }
    }
    Ok(())
}

pub(super) fn validate_fighter_navigation(
    placements: &[MapAssetPlacement],
    dimensions: MapDimensions,
    catalog: &MapContentCatalog,
) -> Result<(), String> {
    let blocked = blocking_shapes(placements, dimensions, catalog);
    let center_is_clear = |cell: MapCell| {
        let center = dimensions.cell_center(cell);
        dimensions
            .bounds()
            .contains_with_inset(center, crate::movement::STANDARD_FIGHTER_RADIUS)
            && blocked.iter().all(|shape| {
                !circle_overlaps_derived_shape(
                    center,
                    crate::movement::STANDARD_FIGHTER_RADIUS,
                    *shape,
                )
            })
    };
    let spawns: Vec<_> = placements
        .iter()
        .filter(|placement| {
            matches!(
                placement.parameters,
                MapPlacementParameters::PlayerSpawn { .. }
            )
        })
        .map(|placement| placement.cell)
        .collect();
    let Some(start) = spawns.first().copied() else {
        return Err("map has no navigation start".to_string());
    };
    if !center_is_clear(start) {
        return Err("spawn is not fighter-radius navigable".to_string());
    }
    let mut reached = BTreeSet::from([start]);
    let mut frontier = std::collections::VecDeque::from([start]);
    while let Some(cell) = frontier.pop_front() {
        for candidate in [
            cell.y.checked_add(1).map(|y| MapCell::new(cell.x, y)),
            cell.x.checked_add(1).map(|x| MapCell::new(x, cell.y)),
            cell.y.checked_sub(1).map(|y| MapCell::new(cell.x, y)),
            cell.x.checked_sub(1).map(|x| MapCell::new(x, cell.y)),
        ]
        .into_iter()
        .flatten()
        {
            if dimensions.contains(candidate)
                && center_is_clear(candidate)
                && reached.insert(candidate)
            {
                frontier.push_back(candidate);
            }
        }
    }
    if spawns.into_iter().all(|spawn| reached.contains(&spawn)) {
        Ok(())
    } else {
        Err("fighter-radius navigation does not connect every spawn".to_string())
    }
}

#[derive(Clone)]
struct HeistSafeAccessGeometry {
    team_slot: u8,
    footprint: BTreeSet<MapCell>,
    shape: DerivedColliderShape,
    sectors: [Vec<MapCell>; 4],
}

fn heist_safe_access_geometry(
    anchor: &MapModeAnchorPlacement,
    dimensions: MapDimensions,
) -> Result<HeistSafeAccessGeometry, String> {
    let MapModeAnchorKind::HeistSafe {
        team_slot,
        origin_cell,
        width_cells,
        height_cells,
        ..
    } = anchor.kind
    else {
        return Err("Heist maps cannot contain non-safe anchors".to_string());
    };
    let footprint = (0..height_cells)
        .flat_map(|y| {
            (0..width_cells).map(move |x| MapCell::new(origin_cell.x + x, origin_cell.y + y))
        })
        .collect::<BTreeSet<_>>();
    let size = Vec2::new(f32::from(width_cells), f32::from(height_cells)) * MAP_CELL_SIZE_WORLD;
    let left_x = origin_cell
        .x
        .checked_sub(2)
        .ok_or_else(|| "Heist safe lacks a left attack sector".to_string())?;
    let bottom_y = origin_cell
        .y
        .checked_sub(2)
        .ok_or_else(|| "Heist safe lacks a lower attack sector".to_string())?;
    let right_x = origin_cell
        .x
        .checked_add(width_cells)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| "Heist safe right attack sector overflows".to_string())?;
    let top_y = origin_cell
        .y
        .checked_add(height_cells)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| "Heist safe upper attack sector overflows".to_string())?;
    let vertical = |x| {
        (0..height_cells)
            .map(|y| MapCell::new(x, origin_cell.y + y))
            .collect::<Vec<_>>()
    };
    let horizontal = |y| {
        (0..width_cells)
            .map(|x| MapCell::new(origin_cell.x + x, y))
            .collect::<Vec<_>>()
    };
    Ok(HeistSafeAccessGeometry {
        team_slot,
        footprint,
        shape: DerivedColliderShape::Rectangle {
            center: dimensions.cell_min(origin_cell) + size * 0.5,
            half_extents: size * 0.5,
        },
        sectors: [
            vertical(left_x),
            vertical(right_x),
            horizontal(bottom_y),
            horizontal(top_y),
        ],
    })
}

fn reachable_clear_cells(
    spawn: MapCell,
    center_is_clear: &impl Fn(MapCell) -> bool,
) -> BTreeSet<MapCell> {
    let mut reached = BTreeSet::from([spawn]);
    let mut frontier = std::collections::VecDeque::from([spawn]);
    while let Some(cell) = frontier.pop_front() {
        for candidate in [
            cell.y.checked_add(1).map(|y| MapCell::new(cell.x, y)),
            cell.x.checked_add(1).map(|x| MapCell::new(x, cell.y)),
            cell.y.checked_sub(1).map(|y| MapCell::new(cell.x, y)),
            cell.x.checked_sub(1).map(|x| MapCell::new(x, cell.y)),
        ]
        .into_iter()
        .flatten()
        {
            if center_is_clear(candidate) && reached.insert(candidate) {
                frontier.push_back(candidate);
            }
        }
    }
    reached
}

pub(super) fn validate_heist_map_access(
    placements: &[MapAssetPlacement],
    anchors: &[MapModeAnchorPlacement],
    dimensions: MapDimensions,
    catalog: &MapContentCatalog,
) -> Result<(), String> {
    let mut safes = Vec::with_capacity(2);
    for anchor in anchors {
        safes.push(heist_safe_access_geometry(anchor, dimensions)?);
    }
    safes.sort_by_key(|safe| safe.team_slot);
    if safes.len() != 2 || safes[0].team_slot != 0 || safes[1].team_slot != 1 {
        return Err("Heist safe access requires exact team slots 0 and 1".to_string());
    }
    if !safes[0].footprint.is_disjoint(&safes[1].footprint) {
        return Err("Heist safe reservations overlap".to_string());
    }
    for placement in placements {
        let asset = catalog
            .asset(placement.asset_id)
            .ok_or_else(|| "placement references an unknown asset".to_string())?;
        if asset.slot == MapAssetSlot::Surface {
            continue;
        }
        let cells = placement_cells(dimensions, asset, placement)
            .ok_or_else(|| "placement footprint disappeared".to_string())?;
        if safes
            .iter()
            .any(|safe| cells.iter().any(|cell| safe.footprint.contains(cell)))
        {
            return Err("Heist safe reservation overlaps a map placement".to_string());
        }
    }

    let mut blocked = blocking_shapes(placements, dimensions, catalog);
    blocked.extend(safes.iter().map(|safe| safe.shape));
    let center_is_clear = |cell: MapCell| {
        dimensions.contains(cell)
            && dimensions.bounds().contains_with_inset(
                dimensions.cell_center(cell),
                crate::movement::STANDARD_FIGHTER_RADIUS,
            )
            && blocked.iter().all(|shape| {
                !circle_overlaps_derived_shape(
                    dimensions.cell_center(cell),
                    crate::movement::STANDARD_FIGHTER_RADIUS,
                    *shape,
                )
            })
    };
    for safe in &safes {
        let open_sectors = safe
            .sectors
            .iter()
            .filter(|sector| {
                sector
                    .windows(2)
                    .any(|pair| pair.iter().copied().all(&center_is_clear))
            })
            .count();
        if open_sectors < 2 {
            return Err("Heist safe exposes fewer than two legal attack sectors".to_string());
        }
    }

    let spawns = placements
        .iter()
        .filter_map(|placement| match placement.parameters {
            MapPlacementParameters::PlayerSpawn { team_slot, .. } => {
                Some((team_slot, placement.cell))
            }
            MapPlacementParameters::None => None,
        })
        .collect::<Vec<_>>();
    for (spawn_team, spawn) in spawns {
        if !center_is_clear(spawn) {
            return Err("Heist spawn is not fighter-radius navigable".to_string());
        }
        let reached = reachable_clear_cells(spawn, &center_is_clear);
        for safe in &safes {
            let reaches_ring = safe
                .sectors
                .iter()
                .flatten()
                .any(|cell| reached.contains(cell) && center_is_clear(*cell));
            if !reaches_ring {
                let relation = if safe.team_slot == spawn_team {
                    "defence"
                } else {
                    "attack"
                };
                return Err(format!(
                    "Heist spawn cannot reach its required {relation} ring"
                ));
            }
        }
    }
    Ok(())
}
