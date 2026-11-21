//! Sparse wall-adjacent floor decals (BRL-0178): cracks, scuffs, leaves, sand drifts.
//!
//! Each theme authors a small decal set in `assets/catalogs/map_presentation_themes.ron`. Placement
//! is a pure, deterministic function of the map (seeded by its recipe ID and the cell): only open
//! floor cells that touch a static wall or the arena edge are candidates, and cells holding any
//! placement (water, effect tiles, cover, foliage, spawns), the Turf Hold circle, Vault Break safes and
//! their attack sectors, and Zombie ingresses are excluded. Every decal stays inside its one
//! authoritative cell, so it can never overlap a neighbouring surface. All decals of a map form one
//! mesh drawn with one lit, alpha-blended toon material over the generated atlas
//! (`asset_src/environment/floor-decals/build_decals.py`), so they cost one draw call, sort under
//! every other transparent layer, and never exceed the lit floor's brightness (no bloom). The Low
//! quality tier hides them. Presentation only: nothing here touches gameplay or the fingerprint.

use super::{
    GeneratedMapMesh, Indices, NotShadowCaster, PrimitiveTopology, RenderAssetUsages, ToonMaterial,
    floor_tiles, ground_position, map, render_quality, turf_hold_visual_geometry,
};
use bevy::prelude::*;
use serde::Deserialize;
use std::collections::BTreeSet;

/// Generated atlas: `ATLAS_COLUMNS` x `ATLAS_ROWS` sprites in [`FloorDecalSprite`] order, as a
/// KTX2 carrying its pre-filtered mip chain (BRL-dq06), so minified decals never shimmer.
pub(super) const FLOOR_DECAL_ATLAS_PATH: &str =
    "brawler/textures/original/environment/floor-decals.ktx2";
const ATLAS_COLUMNS: u8 = 3;
const ATLAS_ROWS: u8 = 2;
/// Height above the floor surface: under contact shadows (0.6) and effect overlays (1.0).
pub(super) const FLOOR_DECAL_LIFT: f32 = 0.15;
/// Sorts the decal layer before every other transparent layer (which use bias 0 or above), so
/// fields, shadows, rings, and zones always draw over decals.
pub(super) const FLOOR_DECAL_DEPTH_BIAS: f32 = -16.0;
/// Bounded state: at most this many decals per map, whatever the authored density.
pub(super) const MAX_FLOOR_DECALS: usize = 160;
/// Accepted decals keep this Chebyshev cell distance from each other, so they never clump.
const MIN_DECAL_SPACING_CELLS: u16 = 2;
/// Cells around a Vault Break safe footprint (its attack sectors) that stay clear.
const VAULT_BREAK_SAFE_CLEARANCE_CELLS: u16 = 2;
/// Cells around a Zombie ingress that stay clear.
const ZOMBIE_INGRESS_CLEARANCE_CELLS: u16 = 2;
/// Extra clearance beyond the Turf Hold radius, in cells.
const TURF_HOLD_CLEARANCE_CELLS: f32 = 0.5;
/// Decal side as a fraction of the cell (it always fits inside the cell).
const MIN_DECAL_SCALE: f32 = 0.78;
const MAX_DECAL_SCALE: f32 = 0.98;
/// Per-decal opacity spread below the theme opacity.
const MIN_DECAL_OPACITY_FACTOR: f32 = 0.75;
/// Art direction (readability): decals stay quiet.
pub(super) const MAX_DECAL_DENSITY: f32 = 0.4;
pub(super) const MAX_DECAL_OPACITY: f32 = 0.6;
const MAX_DECAL_KINDS: usize = 6;
const MAX_DECAL_WEIGHT: u8 = 10;
/// A decal tint may be at most this much brighter (HSV value) than the theme's brightest floor
/// tone, so a drift reads as sand rather than a light spot competing with effects.
pub(super) const MAX_DECAL_VALUE_LIFT: f32 = 1.2;

/// One sprite of the generated atlas; the order is the atlas order.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::presentation_3d) enum FloorDecalSprite {
    Crack,
    Scuff,
    Leaves,
    SandDrift,
    Stain,
    Pebbles,
}

impl FloorDecalSprite {
    const fn atlas_index(self) -> u8 {
        self as u8
    }
}

/// One authored decal kind of a theme.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(in crate::presentation_3d) struct FloorDecalKind {
    pub(in crate::presentation_3d) sprite: FloorDecalSprite,
    /// sRGB tint multiplied with the near-white atlas texel.
    pub(in crate::presentation_3d) tint: (f32, f32, f32),
    /// Relative pick weight among the theme's kinds.
    pub(in crate::presentation_3d) weight: u8,
}

/// Per-theme decal set from `assets/catalogs/map_presentation_themes.ron`.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(in crate::presentation_3d) struct FloorDecals {
    /// Probability that an eligible wall-adjacent cell gets a decal (before spacing).
    pub(in crate::presentation_3d) density: f32,
    /// Peak decal opacity.
    pub(in crate::presentation_3d) opacity: f32,
    pub(in crate::presentation_3d) kinds: Vec<FloorDecalKind>,
}

fn hsv_value(color: (f32, f32, f32)) -> f32 {
    color.0.max(color.1).max(color.2)
}

fn unit_range(value: f32, min: f32, max: f32) -> bool {
    value.is_finite() && (min..=max).contains(&value)
}

impl FloorDecals {
    /// Validates the set against the art direction and the theme's floor brightness.
    pub(in crate::presentation_3d) fn validate(
        &self,
        label: &str,
        floor: floor_tiles::FloorTiles,
    ) -> Result<(), String> {
        if !unit_range(self.density, 0.0, MAX_DECAL_DENSITY) {
            return Err(format!("{label}: decal density out of range"));
        }
        if !unit_range(self.opacity, 0.0, MAX_DECAL_OPACITY) {
            return Err(format!("{label}: decal opacity out of range"));
        }
        if self.kinds.is_empty() || self.kinds.len() > MAX_DECAL_KINDS {
            return Err(format!("{label}: decals need 1 to {MAX_DECAL_KINDS} kinds"));
        }
        let floor_value = hsv_value(floor.tone_a).max(hsv_value(floor.tone_b));
        let mut sprites = BTreeSet::new();
        for kind in &self.kinds {
            if !sprites.insert(kind.sprite.atlas_index()) {
                return Err(format!("{label}: duplicate decal sprite {:?}", kind.sprite));
            }
            if !(1..=MAX_DECAL_WEIGHT).contains(&kind.weight) {
                return Err(format!("{label}: decal weight out of range"));
            }
            let (r, g, b) = kind.tint;
            if ![r, g, b]
                .into_iter()
                .all(|channel| unit_range(channel, 0.0, 1.0))
            {
                return Err(format!("{label}: decal tints must be sRGB in [0, 1]"));
            }
            if hsv_value(kind.tint) > floor_value * MAX_DECAL_VALUE_LIFT {
                return Err(format!(
                    "{label}: decal {:?} is too bright for the floor",
                    kind.sprite
                ));
            }
        }
        Ok(())
    }

    fn total_weight(&self) -> u32 {
        self.kinds.iter().map(|kind| u32::from(kind.weight)).sum()
    }

    fn pick(&self, roll: f32) -> &FloorDecalKind {
        let total = self.total_weight();
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "roll is in [0, 1) and the total weight is at most 60"
        )]
        let mut target = ((roll * total as f32) as u32).min(total.saturating_sub(1));
        for kind in &self.kinds {
            let weight = u32::from(kind.weight);
            if target < weight {
                return kind;
            }
            target -= weight;
        }
        &self.kinds[self.kinds.len() - 1]
    }
}

/// The cell side a decal leans against; the sprite's top edge faces it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WallSide {
    North,
    East,
    South,
    West,
}

impl WallSide {
    const ALL: [Self; 4] = [Self::North, Self::East, Self::South, Self::West];

    /// Simulation-plane direction from the cell toward the wall.
    const fn direction(self) -> Vec2 {
        match self {
            Self::North => Vec2::Y,
            Self::East => Vec2::X,
            Self::South => Vec2::NEG_Y,
            Self::West => Vec2::NEG_X,
        }
    }

    const fn offset(self) -> (i32, i32) {
        match self {
            Self::North => (0, 1),
            Self::East => (1, 0),
            Self::South => (0, -1),
            Self::West => (-1, 0),
        }
    }
}

/// One placed decal; everything is derived from the map and the theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PlacedFloorDecal {
    pub(super) cell: brawler::map::MapCell,
    pub(super) sprite: FloorDecalSprite,
    pub(super) wall: WallSide,
    pub(super) mirrored: bool,
    /// Side length as a fraction of the cell.
    pub(super) scale: f32,
    /// Offset along the wall in [-1, 1] of the free space.
    pub(super) slide: f32,
    /// sRGB tint and final opacity.
    pub(super) tint: (f32, f32, f32),
    pub(super) opacity: f32,
}

/// Deterministic 64-bit mix of the map seed and a cell (splitmix64 finaliser).
fn cell_hash(seed: u64, cell: brawler::map::MapCell) -> u64 {
    let mut value =
        seed ^ (u64::from(cell.x) << 32 | u64::from(cell.y)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

/// The `lane`-th 16-bit slice of a hash as a unit value in [0, 1).
fn hash_unit(hash: u64, lane: u32) -> f32 {
    let bits = u16::try_from((hash >> (lane * 16)) & 0xFFFF).unwrap_or(0);
    f32::from(bits) / 65_536.0
}

fn cell_offset(
    dimensions: brawler::map::MapDimensions,
    cell: brawler::map::MapCell,
    (dx, dy): (i32, i32),
) -> Option<brawler::map::MapCell> {
    let x = u16::try_from(i32::from(cell.x) + dx).ok()?;
    let y = u16::try_from(i32::from(cell.y) + dy).ok()?;
    let next = brawler::map::MapCell::new(x, y);
    dimensions.contains(next).then_some(next)
}

fn insert_clearance(
    cells: &mut BTreeSet<brawler::map::MapCell>,
    dimensions: brawler::map::MapDimensions,
    min: brawler::map::MapCell,
    size: (u16, u16),
    clearance: u16,
) {
    let x0 = min.x.saturating_sub(clearance);
    let y0 = min.y.saturating_sub(clearance);
    let x1 = min.x.saturating_add(size.0).saturating_add(clearance);
    let y1 = min.y.saturating_add(size.1).saturating_add(clearance);
    for y in y0..y1.min(dimensions.height) {
        for x in x0..x1.min(dimensions.width) {
            cells.insert(brawler::map::MapCell::new(x, y));
        }
    }
}

/// Cells that never get a decal: every placement footprint (surfaces, effect tiles, water,
/// features, foliage, spawns, markers) and the mode objective areas.
pub(super) fn excluded_decal_cells(
    snapshot: &brawler::map::ResolvedMapSnapshot,
    catalog: &brawler::map::MapContentCatalog,
) -> BTreeSet<brawler::map::MapCell> {
    let dimensions = snapshot.dimensions;
    let mut cells: BTreeSet<_> = snapshot
        .placements
        .iter()
        .filter_map(|placement| {
            let asset = catalog.asset(placement.asset_id)?;
            brawler::map::placement_cells(dimensions, asset, placement)
        })
        .flatten()
        .collect();
    for anchor in &snapshot.mode_anchors {
        match anchor.kind {
            brawler::map::MapModeAnchorKind::VaultBreakSafe {
                origin_cell,
                width_cells,
                height_cells,
                ..
            } => insert_clearance(
                &mut cells,
                dimensions,
                origin_cell,
                (width_cells, height_cells),
                VAULT_BREAK_SAFE_CLEARANCE_CELLS,
            ),
            brawler::map::MapModeAnchorKind::ZombieIngress { center_half_cell }
            | brawler::map::MapModeAnchorKind::ShardRushSource { center_half_cell } => {
                if let Some(cell) = dimensions
                    .half_cell_world(center_half_cell)
                    .and_then(|point| dimensions.world_cell(point))
                {
                    insert_clearance(
                        &mut cells,
                        dimensions,
                        cell,
                        (1, 1),
                        ZOMBIE_INGRESS_CLEARANCE_CELLS,
                    );
                }
            }
            brawler::map::MapModeAnchorKind::TurfHoldCircle { .. } => {}
        }
    }
    if let Some((center, radius)) = turf_hold_visual_geometry(snapshot) {
        let reach = radius + TURF_HOLD_CLEARANCE_CELLS * brawler::map::MAP_CELL_SIZE_WORLD;
        for y in 0..dimensions.height {
            for x in 0..dimensions.width {
                let cell = brawler::map::MapCell::new(x, y);
                let min = dimensions.cell_min(cell);
                let max = min + Vec2::splat(brawler::map::MAP_CELL_SIZE_WORLD);
                if center.clamp(min, max).distance(center) < reach {
                    cells.insert(cell);
                }
            }
        }
    }
    cells
}

/// Deterministic decal placement for one map: open wall-adjacent cells only, sparse and spaced.
pub(super) fn place_floor_decals(
    snapshot: &brawler::map::ResolvedMapSnapshot,
    catalog: &brawler::map::MapContentCatalog,
    occluders: &BTreeSet<brawler::map::MapCell>,
    decals: &FloorDecals,
) -> Vec<PlacedFloorDecal> {
    let dimensions = snapshot.dimensions;
    if decals.kinds.is_empty() || decals.density <= 0.0 {
        return Vec::new();
    }
    let excluded = excluded_decal_cells(snapshot, catalog);
    let seed = snapshot.identity.recipe_id.0;
    let mut accepted: BTreeSet<brawler::map::MapCell> = BTreeSet::new();
    let mut placed = Vec::new();
    for y in 0..dimensions.height {
        for x in 0..dimensions.width {
            if placed.len() >= MAX_FLOOR_DECALS {
                return placed;
            }
            let cell = brawler::map::MapCell::new(x, y);
            if excluded.contains(&cell) || occluders.contains(&cell) {
                continue;
            }
            let walls: Vec<WallSide> = WallSide::ALL
                .into_iter()
                .filter(|side| {
                    cell_offset(dimensions, cell, side.offset())
                        .is_none_or(|neighbour| occluders.contains(&neighbour))
                })
                .collect();
            if walls.is_empty() {
                continue;
            }
            let hash = cell_hash(seed, cell);
            if hash_unit(hash, 0) >= decals.density {
                continue;
            }
            let spacing = MIN_DECAL_SPACING_CELLS - 1;
            let crowded = (y.saturating_sub(spacing)..=y.saturating_add(spacing))
                .flat_map(|ny| {
                    (x.saturating_sub(spacing)..=x.saturating_add(spacing))
                        .map(move |nx| brawler::map::MapCell::new(nx, ny))
                })
                .any(|neighbour| accepted.contains(&neighbour));
            if crowded {
                continue;
            }
            let kind = decals.pick(hash_unit(hash, 1));
            let second = cell_hash(hash, cell);
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss,
                reason = "at most four wall sides"
            )]
            let wall =
                walls[((hash_unit(second, 0) * walls.len() as f32) as usize).min(walls.len() - 1)];
            accepted.insert(cell);
            placed.push(PlacedFloorDecal {
                cell,
                sprite: kind.sprite,
                wall,
                mirrored: hash_unit(hash, 2) < 0.5,
                scale: MIN_DECAL_SCALE + (MAX_DECAL_SCALE - MIN_DECAL_SCALE) * hash_unit(hash, 3),
                slide: hash_unit(second, 1) * 2.0 - 1.0,
                tint: kind.tint,
                opacity: decals.opacity
                    * (MIN_DECAL_OPACITY_FACTOR
                        + (1.0 - MIN_DECAL_OPACITY_FACTOR) * hash_unit(second, 2)),
            });
        }
    }
    placed
}

/// Simulation-plane corners of a decal in (u, v) order (0,0), (1,0), (1,1), (0,1); v = 0 is the
/// wall side of the sprite.
pub(super) fn decal_corners(
    dimensions: brawler::map::MapDimensions,
    decal: &PlacedFloorDecal,
) -> [Vec2; 4] {
    let size = brawler::map::MAP_CELL_SIZE_WORLD;
    let extent = size * decal.scale;
    let free = (size - extent) * 0.5;
    let toward_wall = decal.wall.direction();
    let along = toward_wall.perp() * if decal.mirrored { -1.0 } else { 1.0 };
    let center =
        dimensions.cell_center(decal.cell) + toward_wall * free + along * free * decal.slide;
    [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
        .map(|(u, v)| center + along * (u - 0.5) * extent + toward_wall * (0.5 - v) * extent)
}

fn atlas_uv(sprite: FloorDecalSprite, u: f32, v: f32) -> [f32; 2] {
    let index = sprite.atlas_index();
    let column = f32::from(index % ATLAS_COLUMNS);
    let row = f32::from(index / ATLAS_COLUMNS);
    [
        (column + u) / f32::from(ATLAS_COLUMNS),
        (row + v) / f32::from(ATLAS_ROWS),
    ]
}

/// One mesh for all decals of a map at `height`; `None` when the map has none.
pub(super) fn floor_decal_mesh(
    dimensions: brawler::map::MapDimensions,
    decals: &[PlacedFloorDecal],
    height: f32,
) -> Option<Mesh> {
    if decals.is_empty() {
        return None;
    }
    let mut positions = Vec::with_capacity(decals.len() * 4);
    let mut uvs = Vec::with_capacity(decals.len() * 4);
    let mut colors = Vec::with_capacity(decals.len() * 4);
    let mut indices = Vec::with_capacity(decals.len() * 6);
    for decal in decals {
        let base = u16::try_from(positions.len()).expect("decal count is bounded");
        let corners = decal_corners(dimensions, decal);
        let tint = Color::srgb(decal.tint.0, decal.tint.1, decal.tint.2).to_linear();
        for (corner, (u, v)) in corners
            .iter()
            .zip([(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)])
        {
            positions.push((ground_position(*corner) + Vec3::Y * height).to_array());
            uvs.push(atlas_uv(decal.sprite, u, v));
            colors.push([tint.red, tint.green, tint.blue, decal.opacity]);
        }
        // Counter-clockwise in the simulation plane is counter-clockwise seen from +Y.
        if (corners[1] - corners[0]).perp_dot(corners[2] - corners[0]) > 0.0 {
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_indices(Indices::U16(indices));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    Some(mesh)
}

/// Samples the decal atlas trilinearly: it is minified at gameplay zoom, and the app default
/// sampler is nearest with no mip filtering.
pub(super) fn floor_decal_atlas_sampler() -> bevy::image::ImageSampler {
    bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
        mag_filter: bevy::image::ImageFilterMode::Linear,
        min_filter: bevy::image::ImageFilterMode::Linear,
        mipmap_filter: bevy::image::ImageFilterMode::Linear,
        ..default()
    })
}

/// The lit, blended material shared by every theme's decals; tint and opacity are vertex colours.
pub(super) fn floor_decal_material(texture: Handle<Image>) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(texture),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.82,
        metallic: 0.0,
        depth_bias: FLOOR_DECAL_DEPTH_BIAS,
        ..default()
    }
}

/// Spawns a map's decal mesh as a generation-owned map member (nothing when it has no decals).
pub(super) fn spawn_floor_decals(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: &Handle<ToonMaterial>,
    marker: brawler::map::MapPresentationMember,
    snapshot: &brawler::map::ResolvedMapSnapshot,
    catalog: &brawler::map::MapContentCatalog,
    decals: &FloorDecals,
) {
    let occluders = floor_tiles::floor_occluder_cells(snapshot, catalog);
    let placed = place_floor_decals(snapshot, catalog, &occluders, decals);
    let Some(mesh) = floor_decal_mesh(
        snapshot.dimensions,
        &placed,
        map::PLAYABLE_FLOOR_Y + FLOOR_DECAL_LIFT,
    ) else {
        return;
    };
    let mesh = meshes.add(mesh);
    commands.spawn((
        marker,
        Mesh3d(mesh.clone()),
        GeneratedMapMesh(mesh),
        MeshMaterial3d(material.clone()),
        FloorDecalLayer,
        NotShadowCaster,
        Transform::IDENTITY,
        Name::new("Floor decals"),
    ));
}

/// Marks a map's decal mesh so the quality tier can hide it.
#[derive(Component)]
pub(super) struct FloorDecalLayer;

/// Hides decals on tiers that drop them (Low); shows them otherwise.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are passed by value"
)]
pub(super) fn apply_floor_decal_quality(
    quality: Option<Res<render_quality::ActiveRenderQuality>>,
    mut layers: Query<&mut Visibility, With<FloorDecalLayer>>,
) {
    let visibility = if render_quality::floor_decals_enabled(quality.as_deref()) {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut current in &mut layers {
        current.set_if_neq(visibility);
    }
}

#[cfg(test)]
mod tests;
