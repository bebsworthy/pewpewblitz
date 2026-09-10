//! Shared, bounded rules for player-authored map recipes.
//!
//! The editor may provide faster local feedback, but only this headless-safe validation path can
//! accept a candidate for Practice. It deliberately reuses the production recipe resolver.

use super::{
    MapAssetDefinition, MapAssetPlacement, MapCell, MapContentCatalog, MapDimensions,
    MapInstanceId, MapPlacementParameterKind, MapRecipe, ModeDefinitionId, ResolvedMap,
};

#[cfg(feature = "client")]
pub const MAX_AUTHORED_MAP_DRAFTS: usize = 64;
#[cfg(feature = "client")]
pub const MAX_AUTHORED_MAP_NAME_GRAPHEMES: usize = 48;
#[cfg(feature = "client")]
pub const MAX_AUTHORED_MAP_NAME_BYTES: usize = 96;
#[cfg(feature = "client")]
pub const MAX_AUTHORED_DOCUMENT_BYTES: usize = 512 * 1024;
pub const MAX_AUTHORED_SOURCE_ITEMS: usize = 4_096;
#[cfg(feature = "client")]
pub const MAX_MAP_UNDO_ENTRIES: usize = 128;
#[cfg(feature = "client")]
pub const MAX_MAP_UNDO_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapAuthoringLimits {
    pub minimum_axis_cells: u16,
    pub maximum_axis_cells: u16,
    pub maximum_total_cells: u32,
    pub maximum_source_items: u16,
    pub maximum_expanded_placements: u16,
    pub maximum_recipe_bytes: u32,
}

pub const AUTHORING_LIMITS: MapAuthoringLimits = MapAuthoringLimits {
    minimum_axis_cells: 20,
    maximum_axis_cells: 128,
    maximum_total_cells: 16_384,
    maximum_source_items: 4_096,
    maximum_expanded_placements: 4_096,
    maximum_recipe_bytes: 256 * 1_024,
};

/// One client-authored candidate bound to a local document and exact recipe revision.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredMapCandidate {
    pub draft_id: u64,
    pub recipe: MapRecipe,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthoringRecipeError {
    UnsupportedMode,
    Dimensions,
    SourceItems,
    ExpandedPlacements,
    SourceBytes,
    Recipe(String),
}

impl core::fmt::Display for AuthoringRecipeError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedMode => formatter.write_str("this mode is unavailable in Map Maker"),
            Self::Dimensions => write!(
                formatter,
                "map dimensions must be {}..={} cells per axis and no more than {} cells total",
                AUTHORING_LIMITS.minimum_axis_cells,
                AUTHORING_LIMITS.maximum_axis_cells,
                AUTHORING_LIMITS.maximum_total_cells
            ),
            Self::SourceItems => write!(
                formatter,
                "map recipe exceeds the {} source-item limit",
                AUTHORING_LIMITS.maximum_source_items
            ),
            Self::ExpandedPlacements => write!(
                formatter,
                "map recipe exceeds the {} expanded-placement limit",
                AUTHORING_LIMITS.maximum_expanded_placements
            ),
            Self::SourceBytes => write!(
                formatter,
                "map recipe exceeds the {} KiB source limit",
                AUTHORING_LIMITS.maximum_recipe_bytes / 1024
            ),
            Self::Recipe(reason) => formatter.write_str(reason),
        }
    }
}

impl std::error::Error for AuthoringRecipeError {}

impl MapAuthoringLimits {
    pub fn validate_dimensions(
        self,
        dimensions: MapDimensions,
    ) -> Result<(), AuthoringRecipeError> {
        if !(self.minimum_axis_cells..=self.maximum_axis_cells).contains(&dimensions.width)
            || !(self.minimum_axis_cells..=self.maximum_axis_cells).contains(&dimensions.height)
            || dimensions.cell_count() > self.maximum_total_cells as usize
        {
            return Err(AuthoringRecipeError::Dimensions);
        }
        Ok(())
    }
}

pub fn validate_authored_recipe(
    recipe: &MapRecipe,
    expected_mode: ModeDefinitionId,
    catalog: &MapContentCatalog,
    instance_id: MapInstanceId,
) -> Result<ResolvedMap, AuthoringRecipeError> {
    if !matches!(
        expected_mode,
        super::WIPEOUT_MODE_DEFINITION
            | super::HOT_ZONE_MODE_DEFINITION
            | super::HEIST_MODE_DEFINITION
    ) || recipe.mode_definition_id != expected_mode
    {
        return Err(AuthoringRecipeError::UnsupportedMode);
    }
    AUTHORING_LIMITS.validate_dimensions(recipe.dimensions)?;
    let source_items = recipe
        .placements
        .len()
        .saturating_add(recipe.filled_rects.len())
        .saturating_add(recipe.mode_anchors.len());
    if source_items > usize::from(AUTHORING_LIMITS.maximum_source_items) {
        return Err(AuthoringRecipeError::SourceItems);
    }
    let mut expanded = recipe.placements.len();
    for rectangle in &recipe.filled_rects {
        let rectangle_count = usize::from(rectangle.width)
            .checked_mul(usize::from(rectangle.height))
            .ok_or(AuthoringRecipeError::ExpandedPlacements)?;
        expanded = expanded
            .checked_add(rectangle_count)
            .ok_or(AuthoringRecipeError::ExpandedPlacements)?;
        if expanded > usize::from(AUTHORING_LIMITS.maximum_expanded_placements) {
            return Err(AuthoringRecipeError::ExpandedPlacements);
        }
    }
    let bytes = postcard::to_allocvec(recipe).map_err(|error| {
        AuthoringRecipeError::Recipe(format!("map recipe serialization failed: {error}"))
    })?;
    if bytes.len() > AUTHORING_LIMITS.maximum_recipe_bytes as usize {
        return Err(AuthoringRecipeError::SourceBytes);
    }
    catalog
        .resolve_recipe(recipe, instance_id)
        .map_err(AuthoringRecipeError::Recipe)
}

/// Assets shown to players. Runtime debris and the typed spawn marker are intentionally excluded.
#[must_use]
pub fn authorable_assets(catalog: &MapContentCatalog) -> Vec<&MapAssetDefinition> {
    catalog
        .assets
        .iter()
        .filter(|asset| {
            asset.parameter_kind == MapPlacementParameterKind::None
                && asset.id != super::BARREL_WOOD_DEBRIS_ASSET
                && asset.id != super::RUBBLE_ASSET
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MapSymmetry {
    #[default]
    None,
    LeftRight,
    TopBottom,
    Rotate180,
    FourWay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymmetryPlacement {
    pub cell: MapCell,
    pub quarter_turns: u8,
}

pub fn symmetry_placements(
    dimensions: MapDimensions,
    asset: &MapAssetDefinition,
    placement: &MapAssetPlacement,
    symmetry: MapSymmetry,
) -> Result<Vec<SymmetryPlacement>, String> {
    if !matches!(placement.parameters, super::MapPlacementParameters::None) {
        return Err("spawn markers and objectives use explicit placement tools".to_string());
    }
    let quarter_turns = placement.quarter_turns % 4;
    let (width, height) = if quarter_turns.is_multiple_of(2) {
        (
            u16::from(asset.footprint_cells.width),
            u16::from(asset.footprint_cells.height),
        )
    } else {
        (
            u16::from(asset.footprint_cells.height),
            u16::from(asset.footprint_cells.width),
        )
    };
    let horizontal = || {
        transform(
            dimensions.width.checked_sub(placement.cell.x + width),
            Some(placement.cell.y),
            reflected_horizontal(quarter_turns),
        )
    };
    let vertical = || {
        transform(
            Some(placement.cell.x),
            dimensions.height.checked_sub(placement.cell.y + height),
            reflected_vertical(quarter_turns),
        )
    };
    let rotated = || {
        transform(
            dimensions.width.checked_sub(placement.cell.x + width),
            dimensions.height.checked_sub(placement.cell.y + height),
            (quarter_turns + 2) % 4,
        )
    };
    let mut result = vec![SymmetryPlacement {
        cell: placement.cell,
        quarter_turns,
    }];
    match symmetry {
        MapSymmetry::None => {}
        MapSymmetry::LeftRight => result.push(horizontal()?),
        MapSymmetry::TopBottom => result.push(vertical()?),
        MapSymmetry::Rotate180 => result.push(rotated()?),
        MapSymmetry::FourWay => {
            result.push(horizontal()?);
            result.push(vertical()?);
            result.push(rotated()?);
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    result.retain(|candidate| seen.insert(candidate.cell));
    result.sort_by_key(|candidate| (candidate.cell.y, candidate.cell.x));
    if result
        .iter()
        .any(|candidate| asset.allowed_quarter_turns & (1 << candidate.quarter_turns) == 0)
    {
        return Err(
            "this asset cannot represent the selected symmetry with its allowed rotations"
                .to_string(),
        );
    }
    Ok(result)
}

fn transform(
    x: Option<u16>,
    y: Option<u16>,
    quarter_turns: u8,
) -> Result<SymmetryPlacement, String> {
    Ok(SymmetryPlacement {
        cell: MapCell::new(
            x.ok_or_else(|| "symmetric footprint falls outside the map".to_string())?,
            y.ok_or_else(|| "symmetric footprint falls outside the map".to_string())?,
        ),
        quarter_turns,
    })
}

const fn reflected_horizontal(quarter_turns: u8) -> u8 {
    (2 + 4 - quarter_turns) % 4
}

const fn reflected_vertical(quarter_turns: u8) -> u8 {
    (4 - quarter_turns) % 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::MapAssetId;

    fn asset(width: u8, height: u8) -> MapAssetDefinition {
        MapAssetDefinition {
            id: MapAssetId(99),
            key: "test".to_string(),
            display_name: "Test".to_string(),
            slot: super::super::MapAssetSlot::Feature,
            gameplay_profile_id: super::super::MapGameplayProfileId(1),
            visual_profile_id: super::super::MapVisualProfileId(1),
            footprint_cells: super::super::MapFootprint { width, height },
            allowed_quarter_turns: 0b1111,
            surface_tag: None,
            allowed_surface_tags: vec![],
            parameter_kind: MapPlacementParameterKind::None,
        }
    }

    fn placement(x: u16, y: u16, quarter_turns: u8) -> MapAssetPlacement {
        MapAssetPlacement {
            placement_id: super::super::MapPlacementId(1),
            cell: MapCell::new(x, y),
            asset_id: MapAssetId(99),
            quarter_turns,
            parameters: super::super::MapPlacementParameters::None,
        }
    }

    #[test]
    fn four_way_deduplicates_odd_center_cell() {
        let result = symmetry_placements(
            MapDimensions {
                width: 21,
                height: 21,
            },
            &asset(1, 1),
            &placement(10, 10, 0),
            MapSymmetry::FourWay,
        )
        .unwrap();
        assert_eq!(result.len(), 1);
        assert!(
            result
                .iter()
                .all(|candidate| candidate.cell == MapCell::new(10, 10))
        );
    }

    #[test]
    fn four_way_reflects_multicell_extent_on_even_dimensions() {
        let result = symmetry_placements(
            MapDimensions {
                width: 20,
                height: 24,
            },
            &asset(3, 2),
            &placement(2, 4, 0),
            MapSymmetry::FourWay,
        )
        .unwrap();
        let cells: Vec<_> = result.iter().map(|candidate| candidate.cell).collect();
        assert_eq!(
            cells,
            vec![
                MapCell::new(2, 4),
                MapCell::new(15, 4),
                MapCell::new(2, 18),
                MapCell::new(15, 18)
            ]
        );
    }

    #[test]
    fn unsupported_reflected_orientation_rejects_atomically() {
        let mut asset = asset(1, 2);
        asset.allowed_quarter_turns = 0b0001;
        let error = symmetry_placements(
            MapDimensions {
                width: 20,
                height: 20,
            },
            &asset,
            &placement(2, 3, 0),
            MapSymmetry::LeftRight,
        )
        .unwrap_err();
        assert!(error.contains("cannot represent"));
    }
}
