//! Grid-aligned two-tone floor tile surface (BRL-0147).
//!
//! One quad per authoritative map cell, coloured through linear vertex colours on a white matte
//! material: a checkerboard of the theme's two close tones, a deterministic per-cell value jitter,
//! and darkening at cell corners near the arena edge and static walls. Presentation only.

use super::{
    Indices, PrimitiveTopology, RenderAssetUsages, ground_position, map_profile_has_dynamic_runtime,
};
use bevy::prelude::*;
use serde::Deserialize;

/// Corner rings over which wall/edge darkening fades out (a touching corner gets full strength).
const EDGE_DARKENING_FALLOFF_CORNERS: i32 = 2;
/// Art direction: the two floor tones differ by 8% value or less.
pub(super) const MAX_TONE_VALUE_DIFFERENCE: f32 = 0.08;
pub(super) const MAX_VALUE_JITTER: f32 = 0.04;
pub(super) const MAX_EDGE_DARKENING: f32 = 0.4;

/// Per-theme floor parameters from `assets/catalogs/map_presentation_themes.ron`.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(in crate::presentation_3d) struct FloorTiles {
    /// sRGB tone of cells where `(x + y)` is even.
    pub(in crate::presentation_3d) tone_a: (f32, f32, f32),
    /// sRGB tone of cells where `(x + y)` is odd.
    pub(in crate::presentation_3d) tone_b: (f32, f32, f32),
    /// Maximum per-cell multiplicative value jitter (plus or minus).
    pub(in crate::presentation_3d) value_jitter: f32,
    /// Value reduction at a corner touching a wall or the arena edge.
    pub(in crate::presentation_3d) edge_darkening: f32,
}

fn hsv_value(color: (f32, f32, f32)) -> f32 {
    color.0.max(color.1).max(color.2)
}

fn channel_is_unit(channel: f32) -> bool {
    channel.is_finite() && (0.0..=1.0).contains(&channel)
}

impl FloorTiles {
    pub(in crate::presentation_3d) fn validate(&self, label: &str) -> Result<(), String> {
        let tones = [self.tone_a, self.tone_b];
        if !tones
            .iter()
            .all(|tone| [tone.0, tone.1, tone.2].into_iter().all(channel_is_unit))
        {
            return Err(format!(
                "{label}: floor tones must be finite sRGB in [0, 1]"
            ));
        }
        let (value_a, value_b) = (hsv_value(self.tone_a), hsv_value(self.tone_b));
        let brighter = value_a.max(value_b);
        if brighter <= 0.0 || (value_a - value_b).abs() / brighter > MAX_TONE_VALUE_DIFFERENCE {
            return Err(format!(
                "{label}: floor tones must differ by at most {}% value",
                MAX_TONE_VALUE_DIFFERENCE * 100.0
            ));
        }
        if !(self.value_jitter.is_finite() && (0.0..=MAX_VALUE_JITTER).contains(&self.value_jitter))
        {
            return Err(format!("{label}: floor value_jitter out of range"));
        }
        if !(self.edge_darkening.is_finite()
            && (0.0..=MAX_EDGE_DARKENING).contains(&self.edge_darkening))
        {
            return Err(format!("{label}: floor edge_darkening out of range"));
        }
        Ok(())
    }

    /// Brightest sRGB channel any floor cell can reach after jitter.
    #[cfg(test)]
    pub(in crate::presentation_3d) fn max_channel(&self) -> f32 {
        hsv_value(self.tone_a).max(hsv_value(self.tone_b)) * (1.0 + self.value_jitter)
    }

    fn tone(&self, cell: brawler::map::MapCell) -> (f32, f32, f32) {
        if (u32::from(cell.x) + u32::from(cell.y)).is_multiple_of(2) {
            self.tone_a
        } else {
            self.tone_b
        }
    }
}

/// Cells whose surroundings darken: the floor stays readable, walls and the edge get depth.
///
/// Only static (never-destroyed) player-blocking features count, because the floor mesh is built
/// once per map generation and must not keep darkening after destructible cover disappears.
pub(super) fn floor_occluder_cells(
    snapshot: &brawler::map::ResolvedMapSnapshot,
    catalog: &brawler::map::MapContentCatalog,
) -> std::collections::BTreeSet<brawler::map::MapCell> {
    snapshot
        .placements
        .iter()
        .filter_map(|placement| {
            let asset = catalog.asset(placement.asset_id)?;
            let profile = catalog.profile(asset.gameplay_profile_id)?;
            grounded_by_floor_darkening(asset, profile)
                .then(|| brawler::map::placement_cells(snapshot.dimensions, asset, placement))
                .flatten()
        })
        .flatten()
        .collect()
}

/// Static walls and indestructible blockers: the baked floor darkening grounds them, so they need
/// no contact shadow (BRL-0152).
pub(super) fn grounded_by_floor_darkening(
    asset: &brawler::map::MapAssetDefinition,
    profile: &brawler::map::MapGameplayProfile,
) -> bool {
    asset.slot == brawler::map::MapAssetSlot::Feature
        && profile.player_collision == brawler::map::PlayerCollision::Block
        && !map_profile_has_dynamic_runtime(profile)
}

/// Deterministic per-cell value in [-1, 1].
fn cell_jitter(cell: brawler::map::MapCell) -> f32 {
    let mut hash =
        u32::from(cell.x).wrapping_mul(0x9E37_79B1) ^ u32::from(cell.y).wrapping_mul(0x85EB_CA77);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x2C1B_3C6D);
    hash ^= hash >> 12;
    let unit = f32::from(u16::try_from(hash & 0xFFFF).unwrap_or(0)) / f32::from(u16::MAX);
    unit * 2.0 - 1.0
}

/// Darkening weight in [0, 1] for the cell corner at `(x, y)` (corner grid is `width + 1` wide).
fn corner_darkness(
    dimensions: brawler::map::MapDimensions,
    occluders: &std::collections::BTreeSet<brawler::map::MapCell>,
    x: i32,
    y: i32,
) -> f32 {
    let occluded = |cell_x: i32, cell_y: i32| match (u16::try_from(cell_x), u16::try_from(cell_y)) {
        (Ok(cell_x), Ok(cell_y)) => {
            let cell = brawler::map::MapCell::new(cell_x, cell_y);
            !dimensions.contains(cell) || occluders.contains(&cell)
        }
        _ => true,
    };
    for ring in 0..EDGE_DARKENING_FALLOFF_CORNERS {
        let touched = (x - 1 - ring..=x + ring)
            .any(|cell_x| (y - 1 - ring..=y + ring).any(|cell_y| occluded(cell_x, cell_y)));
        if touched {
            #[allow(
                clippy::cast_precision_loss,
                reason = "ring counts are tiny code-owned constants"
            )]
            return 1.0 - ring as f32 / EDGE_DARKENING_FALLOFF_CORNERS as f32;
        }
    }
    0.0
}

/// Build the world-space floor mesh: one upward quad per authoritative cell at `height`.
pub(super) fn floor_tile_mesh(
    dimensions: brawler::map::MapDimensions,
    occluders: &std::collections::BTreeSet<brawler::map::MapCell>,
    floor: FloorTiles,
    height: f32,
) -> Mesh {
    let cell_count = usize::from(dimensions.width) * usize::from(dimensions.height);
    let mut positions = Vec::with_capacity(cell_count * 4);
    let mut colors = Vec::with_capacity(cell_count * 4);
    let mut uvs = Vec::with_capacity(cell_count * 4);
    let size = brawler::map::MAP_CELL_SIZE_WORLD;
    for y in 0..dimensions.height {
        for x in 0..dimensions.width {
            let cell = brawler::map::MapCell::new(x, y);
            let min = dimensions.cell_min(cell);
            let tone = floor.tone(cell);
            let value = 1.0 + floor.value_jitter * cell_jitter(cell);
            for (corner_x, corner_y) in [(0_u8, 0_u8), (1, 0), (1, 1), (0, 1)] {
                let corner = min + Vec2::new(f32::from(corner_x), f32::from(corner_y)) * size;
                positions.push((ground_position(corner) + Vec3::Y * height).to_array());
                let shade = value
                    * (1.0
                        - floor.edge_darkening
                            * corner_darkness(
                                dimensions,
                                occluders,
                                i32::from(x) + i32::from(corner_x),
                                i32::from(y) + i32::from(corner_y),
                            ));
                let linear = Color::srgb(
                    (tone.0 * shade).clamp(0.0, 1.0),
                    (tone.1 * shade).clamp(0.0, 1.0),
                    (tone.2 * shade).clamp(0.0, 1.0),
                )
                .to_linear();
                colors.push(linear.to_f32_array());
                uvs.push([f32::from(corner_x), f32::from(corner_y)]);
            }
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    // Simulation +y maps to world -z, so (0,0)->(1,0)->(1,1) is counter-clockwise seen from +Y.
    let quad = |base: u32| [base, base + 1, base + 2, base, base + 2, base + 3];
    let quads = u32::try_from(cell_count).expect("validated map dimensions fit u32");
    if u16::try_from(cell_count * 4).is_ok() {
        mesh.insert_indices(Indices::U16(
            (0..quads)
                .flat_map(|index| quad(index * 4))
                .map(|index| u16::try_from(index).expect("checked vertex count"))
                .collect(),
        ));
    } else {
        mesh.insert_indices(Indices::U32(
            (0..quads).flat_map(|index| quad(index * 4)).collect(),
        ));
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh
}

#[cfg(test)]
mod tests;
