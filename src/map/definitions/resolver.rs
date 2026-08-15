//! Pure preset-independent map recipe resolution.
#![allow(clippy::wildcard_imports)]

use super::*;
use bevy::prelude::Vec2;
use std::collections::{HashSet, VecDeque};

pub fn resolve_map_recipe(
    recipe: &MapRecipe,
    source_preset_id: Option<MapPresetId>,
    instance_id: MapInstanceId,
    catalog: &MapContentCatalog,
    requirements: &MapLayoutRequirements,
    limits: EngineMapLimits,
) -> Result<ResolvedMap, String> {
    if instance_id.0 == 0 {
        return Err("map instance ID must be nonzero".to_string());
    }
    catalog.validate()?;
    validate_recipe_references(recipe, catalog)?;
    let recipe_bytes = postcard::to_allocvec(recipe)
        .map_err(|error| format!("map recipe serialization failed: {error}"))?;
    if recipe_bytes.len() > limits.max_recipe_bytes {
        return Err("map recipe exceeds serialized byte limit".to_string());
    }
    let mut recipe = recipe.clone();
    normalize_recipe(&mut recipe)?;
    validate_recipe(&recipe, catalog, requirements, limits)?;
    let fingerprint_bytes = postcard::to_allocvec(&(
        MAP_FINGERPRINT_FORMAT_VERSION,
        MAP_RECIPE_SCHEMA_VERSION,
        requirements.schema_version,
        &recipe,
    ))
    .map_err(|error| format!("map recipe fingerprint serialization failed: {error}"))?;
    let identity = ResolvedMapIdentity {
        instance_id,
        source_preset_id,
        recipe_id: recipe.recipe_id,
        recipe_revision: recipe.revision,
        recipe_fingerprint: MapRecipeFingerprint(fnv1a64(&fingerprint_bytes)),
    };
    let visual_instances = expand_visuals(&recipe.visuals, limits.max_visual_instances)?;
    let snapshot = ResolvedMapSnapshot {
        identity,
        catalog_schema_version: catalog.schema_version,
        recipe_schema_version: recipe.recipe_version,
        layout_schema_version: requirements.schema_version,
        presentation_theme_id: recipe.presentation_theme_id,
        mode_definition_id: recipe.mode_definition_id,
        playable_bounds: recipe.playable_bounds,
        camera_bounds: recipe.camera_bounds,
        geometry: recipe.geometry,
        visual_instances,
        entities: recipe.entities,
        regions: recipe.regions,
        spawn_areas: recipe.spawn_areas,
        spawn_points: recipe.spawn_points,
        mode_anchors: recipe.mode_anchors,
    };
    let snapshot_bytes = postcard::to_allocvec(&snapshot)
        .map_err(|error| format!("map snapshot serialization failed: {error}"))?;
    if snapshot_bytes.len() > limits.max_snapshot_bytes {
        return Err("resolved map snapshot exceeds serialized byte limit".to_string());
    }
    let round_trip: ResolvedMapSnapshot = postcard::from_bytes(&snapshot_bytes)
        .map_err(|error| format!("map snapshot round trip failed: {error}"))?;
    if round_trip != snapshot {
        return Err("resolved map snapshot failed exact round trip".to_string());
    }
    Ok(ResolvedMap::from_snapshot(snapshot))
}

pub(crate) fn normalize_recipe(recipe: &mut MapRecipe) -> Result<(), String> {
    recipe.playable_bounds.min = normalize_vec2(recipe.playable_bounds.min)?;
    recipe.playable_bounds.max = normalize_vec2(recipe.playable_bounds.max)?;
    recipe.camera_bounds.min = normalize_vec2(recipe.camera_bounds.min)?;
    recipe.camera_bounds.max = normalize_vec2(recipe.camera_bounds.max)?;
    for placement in &mut recipe.geometry {
        placement.position = normalize_vec2(placement.position)?;
        placement.rotation = normalize_rotation(placement.rotation)?;
        normalize_shape(&mut placement.shape)?;
    }
    for placement in &mut recipe.visuals {
        placement.position = normalize_vec2(placement.position)?;
        placement.rotation = normalize_rotation(placement.rotation)?;
        if let VisualPlacementKind::TiledRectangle {
            half_extents,
            cell_size,
        } = &mut placement.kind
        {
            *half_extents = normalize_vec2(*half_extents)?;
            *cell_size = normalize_vec2(*cell_size)?;
        }
    }
    for placement in &mut recipe.entities {
        placement.position = normalize_vec2(placement.position)?;
        placement.rotation = normalize_rotation(placement.rotation)?;
    }
    for placement in &mut recipe.regions {
        placement.position = normalize_vec2(placement.position)?;
        placement.rotation = normalize_rotation(placement.rotation)?;
        normalize_shape(&mut placement.shape)?;
    }
    for area in &mut recipe.spawn_areas {
        area.bounds.min = normalize_vec2(area.bounds.min)?;
        area.bounds.max = normalize_vec2(area.bounds.max)?;
    }
    for point in &mut recipe.spawn_points {
        point.position = normalize_vec2(point.position)?;
        point.facing = normalize_rotation(point.facing)?;
    }
    for anchor in &mut recipe.mode_anchors {
        match &mut anchor.shape {
            ModeAnchorShape::Point { position, facing } => {
                *position = normalize_vec2(*position)?;
                *facing = normalize_rotation(*facing)?;
            }
            ModeAnchorShape::Area { position, shape } => {
                *position = normalize_vec2(*position)?;
                normalize_shape(shape)?;
            }
        }
    }
    recipe.geometry.sort_by_key(|value| value.placement_id);
    recipe.visuals.sort_by_key(|value| value.placement_id);
    recipe.entities.sort_by_key(|value| value.placement_id);
    recipe.regions.sort_by_key(|value| value.placement_id);
    recipe.spawn_areas.sort_by_key(|value| value.placement_id);
    recipe.spawn_points.sort_by_key(|value| value.placement_id);
    recipe.mode_anchors.sort_by_key(|value| value.placement_id);
    Ok(())
}

fn validate_recipe(
    recipe: &MapRecipe,
    catalog: &MapContentCatalog,
    requirements: &MapLayoutRequirements,
    limits: EngineMapLimits,
) -> Result<(), String> {
    if recipe.recipe_id.0 == 0
        || recipe.revision == 0
        || recipe.recipe_version != MAP_RECIPE_SCHEMA_VERSION
        || recipe.mode_definition_id != requirements.mode_definition_id
    {
        return Err("unsupported map recipe identity, revision, or layout".to_string());
    }
    validate_bounds(recipe.playable_bounds, recipe.camera_bounds, limits)?;
    if recipe.geometry.len() > catalog.policy.max_geometry
        || recipe.entities.len() > catalog.policy.max_entities
        || recipe.regions.len() > catalog.policy.max_regions
        || recipe.spawn_areas.len() > catalog.policy.max_spawn_areas
        || recipe.spawn_points.len() > catalog.policy.max_spawn_points
        || recipe.mode_anchors.len() > catalog.policy.max_mode_anchors
    {
        return Err("map recipe exceeds catalog count policy".to_string());
    }
    validate_global_placement_ids(recipe)?;
    for geometry in &recipe.geometry {
        validate_placed_shape(
            geometry.position,
            geometry.rotation,
            geometry.shape,
            recipe.playable_bounds,
            limits,
        )?;
    }
    for region in &recipe.regions {
        validate_placed_shape(
            region.position,
            region.rotation,
            region.shape,
            recipe.playable_bounds,
            limits,
        )?;
        if !requirements
            .allowed_region_profiles
            .contains(&region.profile_id)
        {
            return Err("region profile is not allowed by the layout".to_string());
        }
    }
    if recipe
        .regions
        .iter()
        .filter(|region| region.profile_id == RegionProfileId(1))
        .count()
        > limits.max_destructible_reservations
    {
        return Err("too many destructible terrain reservations".to_string());
    }
    for entity in &recipe.entities {
        if !entity.position.is_finite()
            || !recipe.playable_bounds.contains(entity.position)
            || !requirements
                .allowed_entity_profiles
                .contains(&entity.definition_id)
        {
            return Err("invalid or unsupported map entity placement".to_string());
        }
    }
    validate_layout(recipe, requirements)?;
    validate_spawns(recipe)?;
    validate_spawn_reachability(recipe)?;
    expand_visuals(&recipe.visuals, catalog.policy.max_visual_instances)?;
    Ok(())
}

fn validate_bounds(
    playable: AxisAlignedMapRect,
    camera: AxisAlignedMapRect,
    limits: EngineMapLimits,
) -> Result<(), String> {
    let size = playable.size();
    if !playable.min.is_finite()
        || !playable.max.is_finite()
        || size.x < limits.min_playable_width
        || size.x > limits.max_playable_width
        || size.y < limits.min_playable_height
        || size.y > limits.max_playable_height
        || playable.min.min_element() < -limits.max_absolute_coordinate
        || playable.max.max_element() > limits.max_absolute_coordinate
        || !camera.min.is_finite()
        || !camera.max.is_finite()
        || camera.min.x < playable.min.x
        || camera.min.y < playable.min.y
        || camera.max.x > playable.max.x
        || camera.max.y > playable.max.y
        || camera.min.x >= camera.max.x
        || camera.min.y >= camera.max.y
    {
        return Err("invalid playable or camera bounds".to_string());
    }
    Ok(())
}

fn validate_global_placement_ids(recipe: &MapRecipe) -> Result<(), String> {
    let mut ids = HashSet::new();
    let all = recipe
        .geometry
        .iter()
        .map(|value| value.placement_id)
        .chain(recipe.visuals.iter().map(|value| value.placement_id))
        .chain(recipe.entities.iter().map(|value| value.placement_id))
        .chain(recipe.regions.iter().map(|value| value.placement_id))
        .chain(recipe.spawn_areas.iter().map(|value| value.placement_id))
        .chain(recipe.spawn_points.iter().map(|value| value.placement_id))
        .chain(recipe.mode_anchors.iter().map(|value| value.placement_id));
    for id in all {
        if id.0 == 0 || !ids.insert(id) {
            return Err("map placement IDs must be globally unique and nonzero".to_string());
        }
    }
    Ok(())
}

fn validate_placed_shape(
    position: Vec2,
    rotation: f32,
    shape: MapShape,
    bounds: AxisAlignedMapRect,
    limits: EngineMapLimits,
) -> Result<(), String> {
    let half = shape.bounding_half_extents(rotation);
    let valid_shape = match shape {
        MapShape::Rectangle { half_extents } => {
            half_extents.is_finite()
                && half_extents.min_element() * 2.0 >= limits.min_shape_extent
                && half_extents.max_element() * 2.0 <= limits.max_shape_extent
        }
        MapShape::Circle { radius } => {
            radius.is_finite()
                && radius * 2.0 >= limits.min_shape_extent
                && radius * 2.0 <= limits.max_shape_extent
        }
    };
    if !position.is_finite()
        || !rotation.is_finite()
        || !valid_shape
        || !bounds.contains(position - half)
        || !bounds.contains(position + half)
    {
        return Err("invalid or out-of-bounds map shape".to_string());
    }
    Ok(())
}

fn validate_layout(recipe: &MapRecipe, requirements: &MapLayoutRequirements) -> Result<(), String> {
    if requirements.allowed_team_slots.is_empty()
        || requirements
            .allowed_team_slots
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err("invalid layout team requirements".to_string());
    }
    for team in &requirements.allowed_team_slots {
        let areas = recipe
            .spawn_areas
            .iter()
            .filter(|area| area.team_slot == *team)
            .count();
        let points = recipe
            .spawn_points
            .iter()
            .filter(|point| point.team_slot == *team)
            .count();
        if !requirements.spawn_areas_per_team.contains(&areas)
            || !requirements.spawn_points_per_team.contains(&points)
        {
            return Err("map does not satisfy team spawn count requirements".to_string());
        }
    }
    if recipe.spawn_areas.iter().any(|area| {
        !requirements.allowed_team_slots.contains(&area.team_slot)
            || !rect_contains_rect(recipe.playable_bounds, area.bounds)
    }) || recipe
        .spawn_points
        .iter()
        .any(|point| !requirements.allowed_team_slots.contains(&point.team_slot))
    {
        return Err("map contains an unsupported team slot or spawn area".to_string());
    }
    for requirement in &requirements.required_anchors {
        let anchors: Vec<_> = recipe
            .mode_anchors
            .iter()
            .filter(|anchor| anchor.definition_id == requirement.definition_id)
            .collect();
        if anchors.len() < requirement.minimum
            || anchors.len() > requirement.maximum
            || (requirement.point_only
                && anchors
                    .iter()
                    .any(|anchor| !matches!(anchor.shape, ModeAnchorShape::Point { .. })))
        {
            return Err("map does not satisfy required mode-anchor shape/count".to_string());
        }
    }
    let required: HashSet<_> = requirements
        .required_anchors
        .iter()
        .map(|requirement| requirement.definition_id)
        .collect();
    if recipe
        .mode_anchors
        .iter()
        .any(|anchor| !required.contains(&anchor.definition_id))
    {
        return Err("map contains an unsupported mode anchor".to_string());
    }
    for anchor in &recipe.mode_anchors {
        match anchor.shape {
            ModeAnchorShape::Point { position, facing } => {
                if !position.is_finite()
                    || !facing.is_finite()
                    || !recipe.playable_bounds.contains_with_inset(position, 32.0)
                    || overlaps_geometry(position, 32.0, &recipe.geometry)
                {
                    return Err(
                        "mode anchor point is unsafe or outside playable bounds".to_string()
                    );
                }
            }
            ModeAnchorShape::Area { position, shape } => {
                let valid_shape = match shape {
                    MapShape::Rectangle { half_extents } => {
                        half_extents.is_finite() && half_extents.min_element() > 0.0
                    }
                    MapShape::Circle { radius } => radius.is_finite() && radius > 0.0,
                };
                if !position.is_finite()
                    || !valid_shape
                    || !recipe.playable_bounds.contains(position)
                {
                    return Err(
                        "mode anchor area is invalid or outside playable bounds".to_string()
                    );
                }
            }
        }
    }
    Ok(())
}

fn validate_spawns(recipe: &MapRecipe) -> Result<(), String> {
    const CLEARANCE: f32 = 32.0;
    const SAME_TEAM_DISTANCE: f32 = 64.0;
    const OPPOSING_DISTANCE: f32 = 600.0;
    for (index, point) in recipe.spawn_points.iter().enumerate() {
        let area = recipe
            .spawn_areas
            .iter()
            .find(|area| area.team_slot == point.team_slot)
            .ok_or_else(|| "spawn point has no team area".to_string())?;
        let facing = Vec2::from_angle(point.facing);
        let to_center = (recipe.playable_bounds.center() - point.position).normalize_or_zero();
        if point.spawn_point_id.0 == 0
            || !area.bounds.contains_with_inset(point.position, CLEARANCE)
            || !point.position.is_finite()
            || !point.facing.is_finite()
            || facing.dot(to_center) <= 0.0
            || overlaps_geometry(point.position, CLEARANCE, &recipe.geometry)
            || segment_hits_geometry(
                point.position,
                point.position + facing * 96.0,
                CLEARANCE,
                &recipe.geometry,
            )
        {
            return Err("unsafe, blocked, or wrong-facing spawn point".to_string());
        }
        for other in recipe.spawn_points.iter().skip(index + 1) {
            let minimum = if point.team_slot == other.team_slot {
                SAME_TEAM_DISTANCE
            } else {
                OPPOSING_DISTANCE
            };
            if point.position.distance(other.position) < minimum {
                return Err("spawn points are too close".to_string());
            }
        }
    }
    let mut spawn_ids = HashSet::new();
    if recipe
        .spawn_points
        .iter()
        .any(|point| !spawn_ids.insert(point.spawn_point_id))
    {
        return Err("spawn point IDs must be globally unique".to_string());
    }
    for (index, area) in recipe.spawn_areas.iter().enumerate() {
        for other in recipe.spawn_areas.iter().skip(index + 1) {
            if area.team_slot != other.team_slot && rects_overlap(area.bounds, other.bounds) {
                return Err("opposing team spawn areas overlap".to_string());
            }
        }
    }
    Ok(())
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn validate_spawn_reachability(recipe: &MapRecipe) -> Result<(), String> {
    const CELL: f32 = 32.0;
    const RADIUS: f32 = 24.0;
    let size = recipe.playable_bounds.size();
    let width = (size.x / CELL).floor() as usize;
    let height = (size.y / CELL).floor() as usize;
    if width == 0 || height == 0 || width.saturating_mul(height) > 32_768 {
        return Err("invalid spawn clearance grid".to_string());
    }
    let to_cell = |point: Vec2| -> Option<(usize, usize)> {
        let relative = point - recipe.playable_bounds.min;
        let x = (relative.x / CELL).floor() as isize;
        let y = (relative.y / CELL).floor() as isize;
        (x >= 0 && y >= 0 && (x as usize) < width && (y as usize) < height)
            .then_some((x as usize, y as usize))
    };
    let center = to_cell(recipe.playable_bounds.center())
        .ok_or_else(|| "central combat probe is outside map".to_string())?;
    let is_clear = |x: usize, y: usize| {
        let point = recipe.playable_bounds.min
            + Vec2::new((x as f32 + 0.5) * CELL, (y as f32 + 0.5) * CELL);
        recipe.playable_bounds.contains_with_inset(point, RADIUS)
            && !overlaps_geometry(point, RADIUS, &recipe.geometry)
    };
    if !is_clear(center.0, center.1) {
        return Err("central combat probe is blocked".to_string());
    }
    let mut reachable = vec![false; width * height];
    let mut queue = VecDeque::from([center]);
    reachable[center.1 * width + center.0] = true;
    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in [(1_isize, 0_isize), (-1, 0), (0, 1), (0, -1)] {
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            if nx < 0 || ny < 0 || nx as usize >= width || ny as usize >= height {
                continue;
            }
            let next = (nx as usize, ny as usize);
            let index = next.1 * width + next.0;
            if !reachable[index] && is_clear(next.0, next.1) {
                reachable[index] = true;
                queue.push_back(next);
            }
        }
    }
    for point in &recipe.spawn_points {
        let cell = to_cell(point.position).ok_or_else(|| "spawn is outside grid".to_string())?;
        if !reachable[cell.1 * width + cell.0] {
            return Err("spawn cannot reach the central combat probe".to_string());
        }
    }
    Ok(())
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn expand_visuals(
    visuals: &[VisualPlacement],
    maximum: usize,
) -> Result<Vec<ResolvedVisualInstance>, String> {
    let mut resolved = Vec::new();
    for visual in visuals {
        match visual.kind {
            VisualPlacementKind::Sprite => resolved.push(ResolvedVisualInstance {
                placement_id: visual.placement_id,
                instance_index: 0,
                presentation_profile_id: visual.presentation_profile_id,
                position: visual.position,
                rotation: visual.rotation,
            }),
            VisualPlacementKind::TiledRectangle {
                half_extents,
                cell_size,
            } => {
                if !half_extents.is_finite()
                    || !cell_size.is_finite()
                    || half_extents.min_element() <= 0.0
                    || cell_size.min_element() <= 0.0
                {
                    return Err("invalid tiled visual dimensions".to_string());
                }
                let count = half_extents * 2.0 / cell_size;
                let columns = count.x.round();
                let rows = count.y.round();
                if (count.x - columns).abs() > 0.001
                    || (count.y - rows).abs() > 0.001
                    || columns < 1.0
                    || rows < 1.0
                {
                    return Err("tiled visual does not align to whole cells".to_string());
                }
                let columns = columns as usize;
                let rows = rows as usize;
                for row in 0..rows {
                    for column in 0..columns {
                        let index = row * columns + column;
                        let instance_index = u16::try_from(index)
                            .map_err(|_| "tiled visual instance index overflow".to_string())?;
                        let local = -half_extents
                            + cell_size * 0.5
                            + Vec2::new(column as f32 * cell_size.x, row as f32 * cell_size.y);
                        resolved.push(ResolvedVisualInstance {
                            placement_id: visual.placement_id,
                            instance_index,
                            presentation_profile_id: visual.presentation_profile_id,
                            position: visual.position
                                + Vec2::from_angle(visual.rotation).rotate(local),
                            rotation: visual.rotation,
                        });
                    }
                }
            }
        }
        if resolved.len() > maximum {
            return Err("expanded visuals exceed count limit".to_string());
        }
    }
    Ok(resolved)
}

fn overlaps_geometry(point: Vec2, radius: f32, geometry: &[GeometryPlacement]) -> bool {
    geometry.iter().any(|placement| match placement.shape {
        MapShape::Circle { radius: obstacle } => {
            point.distance_squared(placement.position) < (radius + obstacle).powi(2)
        }
        MapShape::Rectangle { half_extents } => {
            let local = Vec2::from_angle(-placement.rotation).rotate(point - placement.position);
            let closest = local.clamp(-half_extents, half_extents);
            local.distance_squared(closest) < radius * radius
        }
    })
}

#[allow(clippy::cast_precision_loss)]
fn segment_hits_geometry(
    start: Vec2,
    end: Vec2,
    radius: f32,
    geometry: &[GeometryPlacement],
) -> bool {
    (0..=12).any(|step| {
        let fraction = step as f32 / 12.0;
        overlaps_geometry(start.lerp(end, fraction), radius, geometry)
    })
}

fn rect_contains_rect(outer: AxisAlignedMapRect, inner: AxisAlignedMapRect) -> bool {
    outer.contains(inner.min)
        && outer.contains(inner.max)
        && inner.min.x < inner.max.x
        && inner.min.y < inner.max.y
}

fn rects_overlap(a: AxisAlignedMapRect, b: AxisAlignedMapRect) -> bool {
    a.min.x < b.max.x && a.max.x > b.min.x && a.min.y < b.max.y && a.max.y > b.min.y
}

fn normalize_shape(shape: &mut MapShape) -> Result<(), String> {
    match shape {
        MapShape::Rectangle { half_extents } => *half_extents = normalize_vec2(*half_extents)?,
        MapShape::Circle { radius } => *radius = normalize_float(*radius)?,
    }
    Ok(())
}

fn normalize_vec2(value: Vec2) -> Result<Vec2, String> {
    Ok(Vec2::new(
        normalize_float(value.x)?,
        normalize_float(value.y)?,
    ))
}

fn normalize_float(value: f32) -> Result<f32, String> {
    if !value.is_finite() {
        return Err("map data contains a non-finite float".to_string());
    }
    Ok(if value == 0.0 { 0.0 } else { value })
}

fn normalize_rotation(value: f32) -> Result<f32, String> {
    let value = normalize_float(value)?;
    let normalized =
        (value + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    Ok(if normalized.abs() < 1.0e-6 {
        0.0
    } else if (normalized + std::f32::consts::PI).abs() < 1.0e-6 {
        -std::f32::consts::PI
    } else {
        normalized
    })
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}
