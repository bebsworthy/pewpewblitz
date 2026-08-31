//! Canonical recipe validation, ordering, fingerprinting, and runtime-fact construction.

use super::catalog::{MAP_FINGERPRINT_FORMAT_VERSION, MAX_RESOLVED_MAP_SNAPSHOT_BYTES};
use super::geometry::{
    MapCellRect, merge_cells_to_rectangles, placement_cells, placement_world_center,
    validate_fighter_navigation, validate_heist_map_access, validate_spawn_clearance,
};
use super::{
    HEIST_SAFE_VISUAL_PROFILE, MAP_CELL_SIZE_WORLD, MAP_RECIPE_SCHEMA_VERSION, MapAssetId,
    MapAssetPlacement, MapAssetSlot, MapCell, MapColliderShape, MapConcealmentBehavior,
    MapContentCatalog, MapDestructionBehavior, MapDimensions, MapDurabilityBehavior,
    MapEffectTileBehavior, MapInstanceId, MapModeAnchorKind, MapModeAnchorPlacement,
    MapPlacementId, MapPlacementParameterKind, MapPlacementParameters, MapPresetId, MapRecipe,
    MapRecipeFingerprint, MapShape, MapVisualProfileId, ModeAnchorId, PlayerCollision,
    ProjectileCollision, ResolvedEffectTile, ResolvedMapIdentity, ResolvedMapSnapshot,
    SpawnPointId, TeamSpawnPoint,
};
use crate::modes::ModeTopologyPolicy;
use bevy::prelude::{Resource, Vec2};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct ResolvedMap {
    pub snapshot: ResolvedMapSnapshot,
    pub spawn_points_by_team: BTreeMap<u8, Vec<TeamSpawnPoint>>,
    pub static_colliders: Vec<ResolvedMapCollider>,
    pub dynamic_placements: Vec<MapAssetPlacement>,
    pub player_only_surface_rects: Vec<MapCellRect>,
    pub objective_zone: Option<ResolvedMapObjective>,
    pub heist_safes: Vec<ResolvedHeistSafeAnchor>,
    pub effect_tiles: Vec<ResolvedEffectTile>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedMapCollider {
    pub placement_id: MapPlacementId,
    pub position: Vec2,
    pub shape: MapShape,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedMapObjective {
    pub anchor_id: ModeAnchorId,
    pub area: super::NormalizedArea,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedHeistSafeAnchor {
    pub placement_id: MapPlacementId,
    pub anchor_id: ModeAnchorId,
    pub defending_team: crate::combat::TeamId,
    pub center: Vec2,
    pub half_extents: Vec2,
    pub quarter_turns: u8,
    pub objective_visual_profile_id: MapVisualProfileId,
}

pub(super) fn resolve_grid_recipe(
    recipe: &MapRecipe,
    preset_id: MapPresetId,
    instance_id: MapInstanceId,
    catalog: &MapContentCatalog,
) -> Result<ResolvedMap, String> {
    let default_surface_tag = validate_recipe_identity(recipe, instance_id, catalog)?;
    let topology = crate::modes::builtin_mode_catalog()
        .descriptor_for_definition(recipe.mode_definition_id)
        .expect("validated map mode descriptor remains registered")
        .topology();
    let mut placements = expand_recipe_placements(recipe)?;
    validate_recipe_capacity(recipe.dimensions, &placements, catalog)?;
    let mut placement_ids = validate_placement_contracts(recipe, &placements, catalog)?;
    validate_effective_surfaces(recipe.dimensions, &placements, default_surface_tag, catalog)?;
    canonicalize_placements(&mut placements, catalog);

    let mut mode_anchors = recipe.mode_anchors.clone();
    mode_anchors.sort_by_key(|anchor| (anchor.placement_id, anchor.anchor_id));
    let (objective_zone, heist_safes) = validate_and_resolve_mode_anchors(
        topology,
        recipe.dimensions,
        &mode_anchors,
        &mut placement_ids,
    )?;
    validate_mode_topology(recipe, topology, &placements, &mode_anchors, catalog)?;

    let snapshot = build_resolved_snapshot(
        recipe,
        preset_id,
        instance_id,
        placements.clone(),
        mode_anchors,
        catalog,
    )?;
    validate_damageable_capacity(&placements, catalog)?;
    let runtime = derive_runtime_facts(&snapshot, catalog)?;
    let dynamic = derive_dynamic_facts(recipe.dimensions, &placements, catalog);

    Ok(ResolvedMap {
        snapshot,
        spawn_points_by_team: runtime.spawn_points_by_team,
        static_colliders: runtime.static_colliders,
        dynamic_placements: dynamic.dynamic_placements,
        player_only_surface_rects: dynamic.player_only_surface_rects,
        objective_zone,
        heist_safes,
        effect_tiles: dynamic.effect_tiles,
    })
}

fn validate_recipe_identity(
    recipe: &MapRecipe,
    instance_id: MapInstanceId,
    catalog: &MapContentCatalog,
) -> Result<super::MapSurfaceTagId, String> {
    if instance_id.0 == 0
        || recipe.recipe_id.0 == 0
        || recipe.revision == 0
        || recipe.recipe_version != MAP_RECIPE_SCHEMA_VERSION
        || crate::modes::builtin_mode_catalog()
            .descriptor_for_definition(recipe.mode_definition_id)
            .is_none()
    {
        return Err("invalid grid map recipe identity or mode".to_string());
    }
    recipe.dimensions.validate()?;
    let default_surface = catalog
        .asset(recipe.default_surface_asset_id)
        .ok_or_else(|| "unknown default surface asset".to_string())?;
    if default_surface.slot != MapAssetSlot::Surface {
        return Err("default surface must use the surface slot".to_string());
    }
    default_surface
        .surface_tag
        .ok_or_else(|| "default surface must declare a surface tag".to_string())
}

fn expand_recipe_placements(recipe: &MapRecipe) -> Result<Vec<MapAssetPlacement>, String> {
    let mut placements = recipe.placements.clone();
    for rect in &recipe.filled_rects {
        if rect.width == 0 || rect.height == 0 {
            return Err("filled rectangle dimensions must be positive".to_string());
        }
        let count = u32::from(rect.width)
            .checked_mul(u32::from(rect.height))
            .ok_or_else(|| "filled rectangle count overflow".to_string())?;
        for offset in 0..count {
            let x = u32::from(rect.min.x) + offset % u32::from(rect.width);
            let y = u32::from(rect.min.y) + offset / u32::from(rect.width);
            placements.push(MapAssetPlacement {
                placement_id: MapPlacementId(
                    rect.first_placement_id
                        .0
                        .checked_add(offset)
                        .ok_or_else(|| "filled rectangle placement ID overflow".to_string())?,
                ),
                cell: MapCell::new(
                    u16::try_from(x).map_err(|_| "filled rectangle x overflow")?,
                    u16::try_from(y).map_err(|_| "filled rectangle y overflow")?,
                ),
                asset_id: rect.asset_id,
                quarter_turns: rect.quarter_turns,
                parameters: MapPlacementParameters::None,
            });
        }
    }
    Ok(placements)
}

fn validate_recipe_capacity(
    dimensions: MapDimensions,
    placements: &[MapAssetPlacement],
    catalog: &MapContentCatalog,
) -> Result<(), String> {
    let mut concealment_count = 0;
    let mut effect_tile_count = 0;
    for placement in placements {
        let profile = catalog
            .asset(placement.asset_id)
            .and_then(|asset| catalog.profile(asset.gameplay_profile_id));
        concealment_count +=
            usize::from(profile.is_some_and(|profile| {
                profile.concealment == MapConcealmentBehavior::HideOccupants
            }));
        effect_tile_count += usize::from(
            profile.is_some_and(|profile| profile.effect_tile.capabilities().is_effect_tile()),
        );
    }
    if effect_tile_count > super::MAX_EFFECT_TILE_PLACEMENTS {
        return Err("map recipe exceeds the effect-tile ceiling".to_string());
    }
    validate_placement_capacity(dimensions, placements.len(), concealment_count)
}

fn validate_placement_contracts(
    recipe: &MapRecipe,
    placements: &[MapAssetPlacement],
    catalog: &MapContentCatalog,
) -> Result<BTreeSet<MapPlacementId>, String> {
    let mut ids = BTreeSet::new();
    let mut occupied = BTreeMap::new();
    let mut spawn_ordinals = BTreeSet::new();
    for placement in placements {
        let asset = catalog
            .asset(placement.asset_id)
            .ok_or_else(|| "placement references an unknown asset".to_string())?;
        if placement.placement_id.0 == 0
            || !ids.insert(placement.placement_id)
            || placement.quarter_turns > 3
            || asset.allowed_quarter_turns & (1 << placement.quarter_turns) == 0
        {
            return Err("invalid, duplicate, out-of-bounds, or conflicting placement".to_string());
        }
        for cell in placement_cells(recipe.dimensions, asset, placement).ok_or_else(|| {
            "invalid, duplicate, out-of-bounds, or conflicting placement".to_string()
        })? {
            if occupied
                .insert((cell, asset.slot), placement.placement_id)
                .is_some()
            {
                return Err(
                    "invalid, duplicate, out-of-bounds, or conflicting placement".to_string(),
                );
            }
        }
        validate_placement_parameters(asset, placement, &mut spawn_ordinals)?;
    }
    Ok(ids)
}

fn validate_placement_parameters(
    asset: &super::MapAssetDefinition,
    placement: &MapAssetPlacement,
    spawn_ordinals: &mut BTreeSet<(u8, u8)>,
) -> Result<(), String> {
    match (asset.parameter_kind, placement.parameters) {
        (MapPlacementParameterKind::None, MapPlacementParameters::None) => Ok(()),
        (
            MapPlacementParameterKind::PlayerSpawn,
            MapPlacementParameters::PlayerSpawn {
                team_slot,
                ordinal,
                facing_quarter_turns,
            },
        ) if team_slot <= 1
            && ordinal > 0
            && facing_quarter_turns <= 3
            && spawn_ordinals.insert((team_slot, ordinal)) =>
        {
            Ok(())
        }
        _ => Err("placement parameters do not match the asset contract".to_string()),
    }
}

fn validate_effective_surfaces(
    dimensions: MapDimensions,
    placements: &[MapAssetPlacement],
    default_tag: super::MapSurfaceTagId,
    catalog: &MapContentCatalog,
) -> Result<(), String> {
    let mut effective_surface_tags = BTreeMap::new();
    for placement in placements {
        let asset = catalog
            .asset(placement.asset_id)
            .ok_or_else(|| "placement asset disappeared".to_string())?;
        if asset.slot == MapAssetSlot::Surface {
            let tag = asset
                .surface_tag
                .ok_or_else(|| "surface placement has no surface tag".to_string())?;
            effective_surface_tags.insert(placement.cell, tag);
        }
    }
    for placement in placements {
        let asset = catalog
            .asset(placement.asset_id)
            .ok_or_else(|| "placement asset disappeared".to_string())?;
        if asset.slot == MapAssetSlot::Surface || asset.allowed_surface_tags.is_empty() {
            continue;
        }
        let cells = placement_cells(dimensions, asset, placement)
            .ok_or_else(|| "placement footprint disappeared".to_string())?;
        if cells.into_iter().any(|cell| {
            !asset
                .allowed_surface_tags
                .contains(effective_surface_tags.get(&cell).unwrap_or(&default_tag))
        }) {
            return Err("placement is incompatible with the effective surface".to_string());
        }
    }
    Ok(())
}

fn canonicalize_placements(placements: &mut [MapAssetPlacement], catalog: &MapContentCatalog) {
    placements.sort_by_key(|placement| {
        let slot = catalog
            .asset(placement.asset_id)
            .map_or(MapAssetSlot::Marker, |asset| asset.slot);
        (
            placement.cell.y,
            placement.cell.x,
            slot,
            placement.asset_id,
            placement.placement_id,
        )
    });
}

fn validate_mode_topology(
    recipe: &MapRecipe,
    topology: ModeTopologyPolicy,
    placements: &[MapAssetPlacement],
    mode_anchors: &[MapModeAnchorPlacement],
    catalog: &MapContentCatalog,
) -> Result<(), String> {
    for team in 0..=1 {
        let spawn_count = placements
            .iter()
            .filter(|placement| {
                matches!(
                    placement.parameters,
                    MapPlacementParameters::PlayerSpawn { team_slot, .. } if team_slot == team
                )
            })
            .count();
        if spawn_count < 3 {
            return Err("grid map lacks spawn capacity".to_string());
        }
    }
    validate_spawn_clearance(placements, recipe.dimensions, catalog)?;
    validate_effect_tile_spawn_safety(placements, catalog)?;
    validate_fighter_navigation(placements, recipe.dimensions, catalog)?;
    if topology == ModeTopologyPolicy::MirroredHeistSafes {
        validate_heist_map_access(placements, mode_anchors, recipe.dimensions, catalog)?;
    }
    Ok(())
}

fn build_resolved_snapshot(
    recipe: &MapRecipe,
    preset_id: MapPresetId,
    instance_id: MapInstanceId,
    placements: Vec<MapAssetPlacement>,
    mode_anchors: Vec<MapModeAnchorPlacement>,
    catalog: &MapContentCatalog,
) -> Result<ResolvedMapSnapshot, String> {
    let fingerprint_material = postcard::to_allocvec(&(
        MAP_FINGERPRINT_FORMAT_VERSION,
        MAP_RECIPE_SCHEMA_VERSION,
        recipe.recipe_id,
        recipe.revision,
        recipe.mode_definition_id,
        recipe.presentation_theme_id,
        recipe.dimensions,
        recipe.default_surface_asset_id,
        &placements,
        &mode_anchors,
    ))
    .map_err(|error| format!("map recipe fingerprint serialization failed: {error}"))?;
    let snapshot = ResolvedMapSnapshot {
        identity: ResolvedMapIdentity {
            instance_id,
            source_preset_id: Some(preset_id),
            recipe_id: recipe.recipe_id,
            recipe_revision: recipe.revision,
            recipe_fingerprint: MapRecipeFingerprint(crate::content::fnv1a64(
                &fingerprint_material,
            )),
        },
        catalog_schema_version: catalog.schema_version,
        recipe_schema_version: recipe.recipe_version,
        presentation_theme_id: recipe.presentation_theme_id,
        mode_definition_id: recipe.mode_definition_id,
        dimensions: recipe.dimensions,
        default_surface_asset_id: recipe.default_surface_asset_id,
        placements,
        mode_anchors,
    };
    let bytes = postcard::to_allocvec(&snapshot)
        .map_err(|error| format!("map snapshot serialization failed: {error}"))?;
    if bytes.len() > MAX_RESOLVED_MAP_SNAPSHOT_BYTES {
        return Err("map snapshot exceeds the byte ceiling".to_string());
    }
    Ok(snapshot)
}

fn validate_damageable_capacity(
    placements: &[MapAssetPlacement],
    catalog: &MapContentCatalog,
) -> Result<(), String> {
    let damageable_count = placements
        .iter()
        .filter(|placement| {
            catalog
                .asset(placement.asset_id)
                .and_then(|asset| catalog.profile(asset.gameplay_profile_id))
                .is_some_and(|profile| {
                    matches!(profile.durability, MapDurabilityBehavior::HitPoints(_))
                })
        })
        .count();
    if damageable_count > super::MAX_DAMAGEABLE_MAP_OBJECTS {
        Err("map recipe exceeds the damageable-object ceiling".to_string())
    } else {
        Ok(())
    }
}

struct DynamicMapFacts {
    dynamic_placements: Vec<MapAssetPlacement>,
    player_only_surface_rects: Vec<MapCellRect>,
    effect_tiles: Vec<ResolvedEffectTile>,
}

fn derive_dynamic_facts(
    dimensions: MapDimensions,
    placements: &[MapAssetPlacement],
    catalog: &MapContentCatalog,
) -> DynamicMapFacts {
    let dynamic_placements = placements
        .iter()
        .filter(|placement| {
            catalog
                .asset(placement.asset_id)
                .and_then(|asset| catalog.profile(asset.gameplay_profile_id))
                .is_some_and(|profile| {
                    profile.destruction != MapDestructionBehavior::Indestructible
                        || profile.durability != MapDurabilityBehavior::Indestructible
                })
        })
        .cloned()
        .collect();
    let player_only_surface_cells = placements
        .iter()
        .filter(|placement| {
            catalog
                .asset(placement.asset_id)
                .and_then(|asset| catalog.profile(asset.gameplay_profile_id))
                .is_some_and(|profile| {
                    profile.player_collision == PlayerCollision::Block
                        && profile.projectile_collision == ProjectileCollision::Pass
                })
        })
        .flat_map(|placement| {
            let asset = catalog
                .asset(placement.asset_id)
                .expect("resolved placement asset exists");
            placement_cells(dimensions, asset, placement).unwrap_or_default()
        })
        .collect::<BTreeSet<_>>();
    let effect_tiles = placements
        .iter()
        .filter_map(|placement| {
            let behavior = catalog
                .asset(placement.asset_id)
                .and_then(|asset| catalog.profile(asset.gameplay_profile_id))?
                .effect_tile;
            behavior.kind().map(|_| ResolvedEffectTile {
                placement_id: placement.placement_id,
                cell: placement.cell,
                behavior,
            })
        })
        .collect();
    DynamicMapFacts {
        dynamic_placements,
        player_only_surface_rects: merge_cells_to_rectangles(player_only_surface_cells),
        effect_tiles,
    }
}
pub(super) fn validate_placement_capacity(
    dimensions: MapDimensions,
    placement_count: usize,
    concealment_placement_count: usize,
) -> Result<(), String> {
    if placement_count > dimensions.placement_capacity() {
        return Err("grid map exceeds the per-cell asset-slot capacity".to_string());
    }
    if concealment_placement_count > dimensions.cell_count() {
        return Err("grid map exceeds one concealment feature per cell".to_string());
    }
    Ok(())
}

fn validate_effect_tile_spawn_safety(
    placements: &[MapAssetPlacement],
    catalog: &MapContentCatalog,
) -> Result<(), String> {
    let spawn_cells: Vec<_> = placements
        .iter()
        .filter(|placement| {
            matches!(
                placement.parameters,
                MapPlacementParameters::PlayerSpawn { .. }
            )
        })
        .map(|placement| placement.cell)
        .collect();
    for placement in placements {
        let behavior = catalog
            .asset(placement.asset_id)
            .and_then(|asset| catalog.profile(asset.gameplay_profile_id))
            .map_or(MapEffectTileBehavior::None, |profile| profile.effect_tile);
        let capabilities = behavior.capabilities();
        if !capabilities.is_effect_tile() {
            continue;
        }
        for spawn in &spawn_cells {
            if capabilities.violates_spawn_clearance(placement.cell, *spawn) {
                return Err("effect tile violates spawn safety clearance".to_string());
            }
        }
    }
    Ok(())
}

pub(super) fn validate_and_resolve_mode_anchors(
    topology: ModeTopologyPolicy,
    dimensions: MapDimensions,
    anchors: &[MapModeAnchorPlacement],
    placement_ids: &mut BTreeSet<MapPlacementId>,
) -> Result<(Option<ResolvedMapObjective>, Vec<ResolvedHeistSafeAnchor>), String> {
    match topology {
        ModeTopologyPolicy::NoAnchors => {
            return anchors
                .is_empty()
                .then_some((None, Vec::new()))
                .ok_or_else(|| "anchorless modes cannot contain mode anchors".to_string());
        }
        ModeTopologyPolicy::MirroredHeistSafes => {
            return resolve_heist_safe_anchors(dimensions, anchors, placement_ids)
                .map(|resolved| (None, resolved));
        }
        ModeTopologyPolicy::HotZoneCircle => {}
    }
    if anchors.len() != 1 {
        return Err("zone modes require exactly one objective anchor".to_string());
    }
    let anchor = anchors[0];
    if anchor.placement_id.0 == 0
        || anchor.anchor_id.0 == 0
        || !placement_ids.insert(anchor.placement_id)
    {
        return Err("invalid or duplicate mode anchor identity".to_string());
    }
    let MapModeAnchorKind::HotZoneCircle {
        center_half_cell,
        radius_half_cells,
    } = anchor.kind
    else {
        return Err("Hot Zone maps cannot contain non-zone anchors".to_string());
    };
    if radius_half_cells == 0 || radius_half_cells > 64 {
        return Err("invalid Hot Zone objective radius".to_string());
    }
    let center = dimensions
        .half_cell_world(center_half_cell)
        .ok_or_else(|| "Hot Zone objective center is out of bounds".to_string())?;
    let radius = f32::from(radius_half_cells) * (MAP_CELL_SIZE_WORLD * 0.5);
    let bounds = dimensions.bounds();
    if center.x - radius < bounds.min.x
        || center.x + radius > bounds.max.x
        || center.y - radius < bounds.min.y
        || center.y + radius > bounds.max.y
    {
        return Err("Hot Zone objective does not fit playable bounds".to_string());
    }
    Ok((
        Some(ResolvedMapObjective {
            anchor_id: anchor.anchor_id,
            area: super::NormalizedArea {
                center,
                shape: MapShape::Circle { radius },
            },
        }),
        Vec::new(),
    ))
}

fn resolve_heist_safe_anchors(
    dimensions: MapDimensions,
    anchors: &[MapModeAnchorPlacement],
    placement_ids: &mut BTreeSet<MapPlacementId>,
) -> Result<Vec<ResolvedHeistSafeAnchor>, String> {
    if anchors.len() != 2 {
        return Err("Heist maps require exactly two safe anchors".to_string());
    }
    let mut teams = BTreeSet::new();
    let mut resolved = Vec::with_capacity(2);
    for anchor in anchors {
        if anchor.placement_id.0 == 0
            || anchor.anchor_id.0 == 0
            || !placement_ids.insert(anchor.placement_id)
        {
            return Err("invalid or duplicate mode anchor identity".to_string());
        }
        let MapModeAnchorKind::HeistSafe {
            team_slot,
            origin_cell,
            width_cells,
            height_cells,
            quarter_turns,
            objective_visual_profile_id,
        } = anchor.kind
        else {
            return Err("Heist maps cannot contain non-safe anchors".to_string());
        };
        if team_slot > 1
            || !teams.insert(team_slot)
            || width_cells != 3
            || height_cells != 2
            || quarter_turns > 3
            || objective_visual_profile_id != HEIST_SAFE_VISUAL_PROFILE
            || origin_cell.x < 2
            || origin_cell.y < 2
            || origin_cell.x.saturating_add(width_cells).saturating_add(2) > dimensions.width
            || origin_cell.y.saturating_add(height_cells).saturating_add(2) > dimensions.height
        {
            return Err("invalid Heist safe anchor topology".to_string());
        }
        let size = Vec2::new(f32::from(width_cells), f32::from(height_cells)) * MAP_CELL_SIZE_WORLD;
        resolved.push(ResolvedHeistSafeAnchor {
            placement_id: anchor.placement_id,
            anchor_id: anchor.anchor_id,
            defending_team: crate::combat::TeamId(team_slot),
            center: dimensions.cell_min(origin_cell) + size * 0.5,
            half_extents: size * 0.5,
            quarter_turns,
            objective_visual_profile_id,
        });
    }
    resolved.sort_by_key(|safe| safe.defending_team);
    Ok(resolved)
}

struct ResolvedRuntimeFacts {
    spawn_points_by_team: BTreeMap<u8, Vec<TeamSpawnPoint>>,
    static_colliders: Vec<ResolvedMapCollider>,
}

fn derive_runtime_facts(
    grid: &ResolvedMapSnapshot,
    catalog: &MapContentCatalog,
) -> Result<ResolvedRuntimeFacts, String> {
    Ok(ResolvedRuntimeFacts {
        spawn_points_by_team: derive_spawn_points(grid, catalog)?,
        static_colliders: derive_static_colliders(grid, catalog)?,
    })
}

fn derive_static_colliders(
    grid: &ResolvedMapSnapshot,
    catalog: &MapContentCatalog,
) -> Result<Vec<ResolvedMapCollider>, String> {
    let mut rectangle_cells: BTreeMap<MapAssetId, BTreeSet<MapCell>> = BTreeMap::new();
    let mut placement_origins = BTreeMap::new();
    let mut colliders = Vec::new();

    for placement in &grid.placements {
        let asset = catalog
            .asset(placement.asset_id)
            .ok_or_else(|| "resolved placement asset disappeared".to_string())?;
        let profile = catalog
            .profile(asset.gameplay_profile_id)
            .ok_or_else(|| "resolved placement profile disappeared".to_string())?;
        if profile.destruction != MapDestructionBehavior::Indestructible
            || profile.durability != MapDurabilityBehavior::Indestructible
        {
            continue;
        }
        match profile.collider_shape {
            MapColliderShape::FootprintRectangle
                if asset.slot == MapAssetSlot::Feature
                    && profile.player_collision == PlayerCollision::Block
                    && profile.projectile_collision == ProjectileCollision::BlockAndConsume =>
            {
                rectangle_cells
                    .entry(asset.id)
                    .or_default()
                    .extend(placement_cells(grid.dimensions, asset, placement).unwrap_or_default());
                placement_origins.insert((asset.id, placement.cell), placement.placement_id);
            }
            MapColliderShape::Circle { radius_world_units } => {
                colliders.push(ResolvedMapCollider {
                    placement_id: placement.placement_id,
                    position: placement_world_center(grid.dimensions, asset, placement),
                    shape: MapShape::Circle {
                        radius: f32::from(radius_world_units),
                    },
                });
            }
            MapColliderShape::None | MapColliderShape::FootprintRectangle => {}
        }
    }

    for (asset_id, cells) in rectangle_cells {
        for rectangle in merge_cells_to_rectangles(cells) {
            let placement_id = rectangle_cells_iter(rectangle)
                .filter_map(|cell| placement_origins.get(&(asset_id, cell)).copied())
                .min()
                .expect("merged collider contains at least one placement origin");
            let size = Vec2::new(f32::from(rectangle.width), f32::from(rectangle.height))
                * MAP_CELL_SIZE_WORLD;
            colliders.push(ResolvedMapCollider {
                placement_id,
                position: grid.dimensions.cell_min(rectangle.min) + size * 0.5,
                shape: MapShape::Rectangle {
                    half_extents: size * 0.5,
                },
            });
        }
    }
    colliders.sort_by_key(|collider| collider.placement_id);
    Ok(colliders)
}

fn rectangle_cells_iter(rectangle: MapCellRect) -> impl Iterator<Item = MapCell> {
    (0..rectangle.height).flat_map(move |y| {
        (0..rectangle.width).map(move |x| MapCell::new(rectangle.min.x + x, rectangle.min.y + y))
    })
}

fn derive_spawn_points(
    grid: &ResolvedMapSnapshot,
    catalog: &MapContentCatalog,
) -> Result<BTreeMap<u8, Vec<TeamSpawnPoint>>, String> {
    let mut spawn_points = Vec::new();
    for placement in &grid.placements {
        let asset = catalog
            .asset(placement.asset_id)
            .ok_or_else(|| "resolved placement asset disappeared".to_string())?;
        if let (
            MapAssetSlot::Marker,
            MapPlacementParameters::PlayerSpawn {
                team_slot,
                ordinal,
                facing_quarter_turns,
            },
        ) = (asset.slot, placement.parameters)
        {
            spawn_points.push(TeamSpawnPoint {
                placement_id: placement.placement_id,
                spawn_point_id: SpawnPointId(u16::from(ordinal)),
                team_slot,
                position: grid.dimensions.cell_center(placement.cell),
                facing: f32::from(facing_quarter_turns) * core::f32::consts::FRAC_PI_2,
            });
        }
    }
    spawn_points.sort_by_key(|point| (point.team_slot, point.spawn_point_id));
    let mut by_team: BTreeMap<u8, Vec<TeamSpawnPoint>> = BTreeMap::new();
    for point in spawn_points {
        by_team.entry(point.team_slot).or_default().push(point);
    }
    Ok(by_team)
}
