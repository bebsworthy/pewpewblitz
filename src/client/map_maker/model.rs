use crate::map::{
    MAP_RECIPE_SCHEMA_VERSION, MapAssetId, MapAssetPlacement, MapAssetSlot, MapCell,
    MapContentCatalog, MapDimensions, MapPlacementId, MapPlacementParameters,
    MapPresentationThemeId, MapRecipe, MapRecipeId, MapSymmetry, ModeDefinitionId,
    authorable_assets, placement_cells, symmetry_placements,
};
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use unicode_normalization::UnicodeNormalization as _;
use unicode_segmentation::UnicodeSegmentation as _;

pub const MAP_DRAFT_SCHEMA_VERSION: u16 = 1;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct MapDraftId(pub u64);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MapDraftDocument {
    pub schema_version: u16,
    pub id: MapDraftId,
    pub name: String,
    pub intended_game_type_id: Option<crate::lobby::GameTypeId>,
    pub recipe: MapRecipe,
}

impl MapDraftDocument {
    pub fn blank(
        id: MapDraftId,
        name: &str,
        mode_definition_id: ModeDefinitionId,
        dimensions: MapDimensions,
        presentation_theme_id: MapPresentationThemeId,
        default_surface_asset_id: MapAssetId,
    ) -> Result<Self, String> {
        if id.0 == 0 {
            return Err("map draft identity must be nonzero".to_string());
        }
        crate::map::AUTHORING_LIMITS
            .validate_dimensions(dimensions)
            .map_err(|error| error.to_string())?;
        let recipe_id = id.0;
        Ok(Self {
            schema_version: MAP_DRAFT_SCHEMA_VERSION,
            id,
            name: normalize_map_name(name)?,
            intended_game_type_id: None,
            recipe: MapRecipe {
                recipe_id: MapRecipeId(recipe_id),
                revision: 1,
                recipe_version: MAP_RECIPE_SCHEMA_VERSION,
                mode_definition_id,
                presentation_theme_id,
                dimensions,
                default_surface_asset_id,
                placements: Vec::new(),
                filled_rects: Vec::new(),
                mode_anchors: Vec::new(),
            },
        })
    }

    pub fn validate_envelope(&self) -> Result<(), String> {
        if self.schema_version != MAP_DRAFT_SCHEMA_VERSION || self.id.0 == 0 {
            return Err("unsupported or invalid map draft envelope".to_string());
        }
        if normalize_map_name(&self.name)? != self.name {
            return Err("map draft name is not normalized".to_string());
        }
        crate::map::AUTHORING_LIMITS
            .validate_dimensions(self.recipe.dimensions)
            .map_err(|error| error.to_string())
    }

    pub fn renamed(&self, name: &str) -> Result<Self, String> {
        let mut renamed = self.clone();
        renamed.name = normalize_map_name(name)?;
        Ok(renamed)
    }

    pub fn duplicate(&self, id: MapDraftId) -> Result<Self, String> {
        let mut duplicate = self.clone();
        duplicate.id = id;
        duplicate.recipe.recipe_id = MapRecipeId(id.0);
        duplicate.recipe.revision = 1;
        duplicate.name = normalize_map_name(&format!("{} Copy", self.name))?;
        Ok(duplicate)
    }
}

pub fn normalize_map_name(value: &str) -> Result<String, String> {
    let normalized: String = value.trim().nfc().collect();
    let graphemes = normalized.graphemes(true).count();
    if graphemes == 0
        || graphemes > crate::map::authoring::MAX_AUTHORED_MAP_NAME_GRAPHEMES
        || normalized.len() > crate::map::authoring::MAX_AUTHORED_MAP_NAME_BYTES
        || normalized.chars().any(char::is_control)
    {
        return Err("map name must be 1..=48 readable characters".to_string());
    }
    Ok(normalized)
}

#[derive(Resource, Clone, Debug, Default)]
pub struct MapDraftLibrary {
    pub drafts: BTreeMap<MapDraftId, MapDraftDocument>,
    pub load_failures: Vec<String>,
}

impl MapDraftLibrary {
    pub fn insert(&mut self, document: MapDraftDocument) -> Result<(), String> {
        document.validate_envelope()?;
        if !self.drafts.contains_key(&document.id)
            && self.drafts.len() >= crate::map::authoring::MAX_AUTHORED_MAP_DRAFTS
        {
            return Err("Map Maker library limit reached (64)".to_string());
        }
        self.drafts.insert(document.id, document);
        Ok(())
    }

    #[must_use]
    pub fn next_id(&self) -> MapDraftId {
        MapDraftId(
            self.drafts
                .keys()
                .map(|id| id.0)
                .max()
                .unwrap_or(0)
                .saturating_add(1)
                .max(1),
        )
    }
}

#[derive(Resource, Clone, Debug)]
pub struct MapEditorSession {
    pub document: MapDraftDocument,
    pub saved_document: MapDraftDocument,
    pub symmetry: MapSymmetry,
    pub selected_asset: Option<MapAssetId>,
    pub validation_error: Option<String>,
    undo: VecDeque<MapRecipe>,
    redo: VecDeque<MapRecipe>,
    undo_bytes: usize,
}

impl MapEditorSession {
    #[must_use]
    pub fn open(document: MapDraftDocument) -> Self {
        Self {
            saved_document: document.clone(),
            document,
            symmetry: MapSymmetry::None,
            selected_asset: None,
            validation_error: None,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            undo_bytes: 0,
        }
    }

    #[must_use]
    pub fn dirty(&self) -> bool {
        self.document != self.saved_document
    }

    pub fn mark_saved(&mut self) {
        self.saved_document = self.document.clone();
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn paint_stroke(
        &mut self,
        catalog: &MapContentCatalog,
        asset_id: MapAssetId,
        cells: impl IntoIterator<Item = MapCell>,
        quarter_turns: u8,
    ) -> Result<(), String> {
        let cells = cells.into_iter().collect::<BTreeSet<_>>();
        if cells.is_empty() {
            return Ok(());
        }
        let before = self.document.recipe.clone();
        let mut candidate_session = self.clone();
        for cell in cells {
            candidate_session.place(catalog, asset_id, cell, quarter_turns)?;
        }
        let mut candidate = candidate_session.document.recipe;
        candidate.revision = before
            .revision
            .checked_add(1)
            .ok_or_else(|| "map revision space is exhausted".to_string())?;
        self.commit(before, candidate);
        Ok(())
    }

    pub fn resize(&mut self, dimensions: MapDimensions) -> Result<(), String> {
        crate::map::AUTHORING_LIMITS
            .validate_dimensions(dimensions)
            .map_err(|error| error.to_string())?;
        let before = self.document.recipe.clone();
        if before.dimensions == dimensions {
            return Ok(());
        }
        let mut candidate = before.clone();
        candidate.dimensions = dimensions;
        let catalog = MapContentCatalog::embedded()?;
        validate_direct_edit(&candidate, &catalog)
            .map_err(|error| format!("resize would clip or conflict with content: {error}"))?;
        if candidate
            .mode_anchors
            .iter()
            .any(|anchor| !anchor_fits(dimensions, anchor))
        {
            return Err("resize would clip an objective anchor".to_string());
        }
        bump_revision(&mut candidate)?;
        self.commit(before, candidate);
        Ok(())
    }

    pub fn move_placement(
        &mut self,
        catalog: &MapContentCatalog,
        placement_id: MapPlacementId,
        cell: MapCell,
    ) -> Result<(), String> {
        self.change_placement(catalog, placement_id, |placement| placement.cell = cell)
    }

    pub fn rotate_placement(
        &mut self,
        catalog: &MapContentCatalog,
        placement_id: MapPlacementId,
    ) -> Result<(), String> {
        self.change_placement(catalog, placement_id, |placement| {
            placement.quarter_turns = (placement.quarter_turns + 1) % 4;
        })
    }

    fn change_placement(
        &mut self,
        catalog: &MapContentCatalog,
        placement_id: MapPlacementId,
        change: impl FnOnce(&mut MapAssetPlacement),
    ) -> Result<(), String> {
        let before = self.document.recipe.clone();
        let mut candidate = before.clone();
        let placement = candidate
            .placements
            .iter_mut()
            .find(|placement| placement.placement_id == placement_id)
            .ok_or_else(|| "the selected placement no longer exists".to_string())?;
        change(placement);
        validate_direct_edit(&candidate, catalog)?;
        bump_revision(&mut candidate)?;
        self.commit(before, candidate);
        Ok(())
    }

    pub fn place(
        &mut self,
        catalog: &MapContentCatalog,
        asset_id: MapAssetId,
        cell: MapCell,
        quarter_turns: u8,
    ) -> Result<(), String> {
        let asset = authorable_assets(catalog)
            .into_iter()
            .find(|asset| asset.id == asset_id)
            .ok_or_else(|| "asset is not available in Map Maker".to_string())?;
        let seed = MapAssetPlacement {
            placement_id: MapPlacementId(1),
            cell,
            asset_id,
            quarter_turns,
            parameters: MapPlacementParameters::None,
        };
        let transformed =
            symmetry_placements(self.document.recipe.dimensions, asset, &seed, self.symmetry)?;
        let before = self.document.recipe.clone();
        let mut candidate = before.clone();
        let mut next_id = candidate
            .placements
            .iter()
            .map(|placement| placement.placement_id.0)
            .chain(
                candidate
                    .filled_rects
                    .iter()
                    .map(|rect| rect.first_placement_id.0),
            )
            .chain(
                candidate
                    .mode_anchors
                    .iter()
                    .map(|anchor| anchor.placement_id.0),
            )
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| "map placement identity space is exhausted".to_string())?;
        for transformed in transformed {
            let pending = MapAssetPlacement {
                placement_id: MapPlacementId(next_id),
                cell: transformed.cell,
                asset_id,
                quarter_turns: transformed.quarter_turns,
                parameters: MapPlacementParameters::None,
            };
            let pending_cells = placement_cells(candidate.dimensions, asset, &pending)
                .ok_or_else(|| "placement footprint falls outside the map".to_string())?;
            candidate.placements.retain(|existing| {
                let Some(existing_asset) = catalog.asset(existing.asset_id) else {
                    return true;
                };
                if existing_asset.slot != asset.slot {
                    return true;
                }
                placement_cells(candidate.dimensions, existing_asset, existing)
                    .is_none_or(|cells| !cells.iter().any(|cell| pending_cells.contains(cell)))
            });
            candidate.placements.push(pending);
            next_id = next_id
                .checked_add(1)
                .ok_or_else(|| "map placement identity space is exhausted".to_string())?;
        }
        validate_direct_edit(&candidate, catalog)?;
        candidate.revision = candidate
            .revision
            .checked_add(1)
            .ok_or_else(|| "map revision space is exhausted".to_string())?;
        self.commit(before, candidate);
        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), String> {
        let before = self.document.recipe.clone();
        if before.placements.is_empty()
            && before.filled_rects.is_empty()
            && before.mode_anchors.is_empty()
        {
            return Ok(());
        }
        let mut candidate = before.clone();
        candidate.placements.clear();
        candidate.filled_rects.clear();
        candidate.mode_anchors.clear();
        candidate.revision = candidate
            .revision
            .checked_add(1)
            .ok_or_else(|| "map revision space is exhausted".to_string())?;
        self.commit(before, candidate);
        Ok(())
    }

    pub fn place_spawn(
        &mut self,
        catalog: &MapContentCatalog,
        cell: MapCell,
        team_slot: u8,
        facing_quarter_turns: u8,
    ) -> Result<(), String> {
        if team_slot > 1 || facing_quarter_turns > 3 {
            return Err("spawn parameters are outside the supported range".to_string());
        }
        let before = self.document.recipe.clone();
        let mut candidate = before.clone();
        let ordinal = candidate
            .placements
            .iter()
            .filter_map(|placement| match placement.parameters {
                MapPlacementParameters::PlayerSpawn {
                    team_slot: team,
                    ordinal,
                    ..
                } if team == team_slot => Some(ordinal),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| "spawn ordinal space is exhausted".to_string())?;
        replace_slot_at_cell(&mut candidate, catalog, cell, MapAssetSlot::Marker);
        candidate.placements.push(MapAssetPlacement {
            placement_id: next_placement_id(&candidate)?,
            cell,
            asset_id: crate::map::PLAYER_SPAWN_ASSET,
            quarter_turns: 0,
            parameters: MapPlacementParameters::PlayerSpawn {
                team_slot,
                ordinal,
                facing_quarter_turns,
            },
        });
        validate_direct_edit(&candidate, catalog)?;
        bump_revision(&mut candidate)?;
        self.commit(before, candidate);
        Ok(())
    }

    pub fn place_hot_zone(&mut self, cell: MapCell) -> Result<(), String> {
        if self.document.recipe.mode_definition_id != crate::map::HOT_ZONE_MODE_DEFINITION {
            return Err("capture circles are only available for Hot Zone".to_string());
        }
        let before = self.document.recipe.clone();
        let mut candidate = before.clone();
        candidate.mode_anchors.clear();
        candidate
            .mode_anchors
            .push(crate::map::MapModeAnchorPlacement {
                placement_id: MapPlacementId(next_placement_id(&candidate)?.0),
                anchor_id: crate::map::ModeAnchorId(1),
                kind: crate::map::MapModeAnchorKind::HotZoneCircle {
                    center_half_cell: crate::map::MapHalfCellPoint {
                        x: cell.x.saturating_mul(2).saturating_add(1),
                        y: cell.y.saturating_mul(2).saturating_add(1),
                    },
                    radius_half_cells: 5,
                },
            });
        bump_revision(&mut candidate)?;
        self.commit(before, candidate);
        Ok(())
    }

    pub fn place_heist_safe(&mut self, cell: MapCell, team_slot: u8) -> Result<(), String> {
        if self.document.recipe.mode_definition_id != crate::map::HEIST_MODE_DEFINITION
            || team_slot > 1
        {
            return Err("team safes are only available for Heist".to_string());
        }
        if cell.x.saturating_add(3) > self.document.recipe.dimensions.width
            || cell.y.saturating_add(2) > self.document.recipe.dimensions.height
        {
            return Err("the 3x2 safe footprint falls outside the map".to_string());
        }
        let before = self.document.recipe.clone();
        let mut candidate = before.clone();
        candidate.mode_anchors.retain(|anchor| !matches!(anchor.kind, crate::map::MapModeAnchorKind::HeistSafe { team_slot: team, .. } if team == team_slot));
        let next = next_placement_id(&candidate)?;
        candidate
            .mode_anchors
            .push(crate::map::MapModeAnchorPlacement {
                placement_id: next,
                anchor_id: crate::map::ModeAnchorId(u32::from(team_slot) + 1),
                kind: crate::map::MapModeAnchorKind::HeistSafe {
                    team_slot,
                    origin_cell: cell,
                    width_cells: 3,
                    height_cells: 2,
                    quarter_turns: 0,
                    objective_visual_profile_id: crate::map::HEIST_SAFE_VISUAL_PROFILE,
                },
            });
        bump_revision(&mut candidate)?;
        self.commit(before, candidate);
        Ok(())
    }

    pub fn erase_at(&mut self, catalog: &MapContentCatalog, cell: MapCell) -> Result<(), String> {
        let before = self.document.recipe.clone();
        let mut candidate = before.clone();
        let original_count = candidate.placements.len() + candidate.mode_anchors.len();
        candidate.placements.retain(|placement| {
            catalog
                .asset(placement.asset_id)
                .and_then(|asset| placement_cells(candidate.dimensions, asset, placement))
                .is_none_or(|cells| !cells.contains(&cell))
        });
        candidate
            .mode_anchors
            .retain(|anchor| !anchor_contains_cell(anchor, cell));
        if candidate.placements.len() + candidate.mode_anchors.len() == original_count {
            return Ok(());
        }
        bump_revision(&mut candidate)?;
        self.commit(before, candidate);
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop_back() else {
            return false;
        };
        self.undo_bytes = self.undo_bytes.saturating_sub(recipe_bytes(&previous));
        self.redo.push_back(self.document.recipe.clone());
        self.document.recipe = previous;
        self.validation_error = None;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop_back() else {
            return false;
        };
        let before = self.document.recipe.clone();
        self.document.recipe = next;
        self.push_undo(before);
        self.validation_error = None;
        true
    }

    fn commit(&mut self, before: MapRecipe, candidate: MapRecipe) {
        if before == candidate {
            return;
        }
        self.document.recipe = candidate;
        self.push_undo(before);
        self.redo.clear();
        self.validation_error = None;
    }

    fn push_undo(&mut self, recipe: MapRecipe) {
        self.undo_bytes = self.undo_bytes.saturating_add(recipe_bytes(&recipe));
        self.undo.push_back(recipe);
        while self.undo.len() > crate::map::authoring::MAX_MAP_UNDO_ENTRIES
            || self.undo_bytes > crate::map::authoring::MAX_MAP_UNDO_BYTES
        {
            if let Some(removed) = self.undo.pop_front() {
                self.undo_bytes = self.undo_bytes.saturating_sub(recipe_bytes(&removed));
            } else {
                break;
            }
        }
    }
}

fn recipe_bytes(recipe: &MapRecipe) -> usize {
    postcard::to_allocvec(recipe).map_or(usize::MAX, |bytes| bytes.len())
}

fn next_placement_id(recipe: &MapRecipe) -> Result<MapPlacementId, String> {
    recipe
        .placements
        .iter()
        .map(|placement| placement.placement_id.0)
        .chain(
            recipe
                .filled_rects
                .iter()
                .map(|rect| rect.first_placement_id.0),
        )
        .chain(
            recipe
                .mode_anchors
                .iter()
                .map(|anchor| anchor.placement_id.0),
        )
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .map(MapPlacementId)
        .ok_or_else(|| "map placement identity space is exhausted".to_string())
}

fn bump_revision(recipe: &mut MapRecipe) -> Result<(), String> {
    recipe.revision = recipe
        .revision
        .checked_add(1)
        .ok_or_else(|| "map revision space is exhausted".to_string())?;
    Ok(())
}

fn replace_slot_at_cell(
    recipe: &mut MapRecipe,
    catalog: &MapContentCatalog,
    cell: MapCell,
    slot: MapAssetSlot,
) {
    recipe.placements.retain(|placement| {
        let Some(asset) = catalog.asset(placement.asset_id) else {
            return true;
        };
        asset.slot != slot
            || placement_cells(recipe.dimensions, asset, placement)
                .is_none_or(|cells| !cells.contains(&cell))
    });
}

fn anchor_contains_cell(anchor: &crate::map::MapModeAnchorPlacement, cell: MapCell) -> bool {
    match anchor.kind {
        crate::map::MapModeAnchorKind::HotZoneCircle {
            center_half_cell, ..
        } => center_half_cell.x / 2 == cell.x && center_half_cell.y / 2 == cell.y,
        crate::map::MapModeAnchorKind::HeistSafe {
            origin_cell,
            width_cells,
            height_cells,
            ..
        } => {
            cell.x >= origin_cell.x
                && cell.x < origin_cell.x.saturating_add(width_cells)
                && cell.y >= origin_cell.y
                && cell.y < origin_cell.y.saturating_add(height_cells)
        }
        crate::map::MapModeAnchorKind::ZombieIngress { center_half_cell } => {
            center_half_cell.x / 2 == cell.x && center_half_cell.y / 2 == cell.y
        }
    }
}

fn anchor_fits(dimensions: MapDimensions, anchor: &crate::map::MapModeAnchorPlacement) -> bool {
    match anchor.kind {
        crate::map::MapModeAnchorKind::HotZoneCircle {
            center_half_cell,
            radius_half_cells,
        } => {
            center_half_cell.x >= radius_half_cells
                && center_half_cell.y >= radius_half_cells
                && center_half_cell.x.saturating_add(radius_half_cells)
                    <= dimensions.width.saturating_mul(2)
                && center_half_cell.y.saturating_add(radius_half_cells)
                    <= dimensions.height.saturating_mul(2)
        }
        crate::map::MapModeAnchorKind::HeistSafe {
            origin_cell,
            width_cells,
            height_cells,
            ..
        } => {
            origin_cell.x.saturating_add(width_cells) <= dimensions.width
                && origin_cell.y.saturating_add(height_cells) <= dimensions.height
        }
        crate::map::MapModeAnchorKind::ZombieIngress { center_half_cell } => {
            center_half_cell.x < dimensions.width.saturating_mul(2)
                && center_half_cell.y < dimensions.height.saturating_mul(2)
        }
    }
}

fn validate_direct_edit(recipe: &MapRecipe, catalog: &MapContentCatalog) -> Result<(), String> {
    crate::map::AUTHORING_LIMITS
        .validate_dimensions(recipe.dimensions)
        .map_err(|error| error.to_string())?;
    let source_count = recipe
        .placements
        .len()
        .saturating_add(recipe.filled_rects.len())
        .saturating_add(recipe.mode_anchors.len());
    if source_count > crate::map::authoring::MAX_AUTHORED_SOURCE_ITEMS {
        return Err("map source-item limit reached".to_string());
    }
    let mut occupied = BTreeMap::new();
    for placement in &recipe.placements {
        let asset = catalog
            .asset(placement.asset_id)
            .ok_or_else(|| "placement references unavailable content".to_string())?;
        for cell in placement_cells(recipe.dimensions, asset, placement)
            .ok_or_else(|| "placement footprint falls outside the map".to_string())?
        {
            if occupied
                .insert((cell, asset.slot), placement.placement_id)
                .is_some()
            {
                return Err("placement conflicts with another asset in the same slot".to_string());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_gets_new_document_and_recipe_identity() {
        let document = MapDraftDocument::blank(
            MapDraftId(8),
            "Arena",
            crate::map::WIPEOUT_MODE_DEFINITION,
            MapDimensions {
                width: 32,
                height: 24,
            },
            MapPresentationThemeId(1),
            crate::map::GROUND_ASSET,
        )
        .unwrap();
        let duplicate = document.duplicate(MapDraftId(9)).unwrap();
        assert_ne!(document.id, duplicate.id);
        assert_ne!(document.recipe.recipe_id, duplicate.recipe.recipe_id);
        assert_eq!(duplicate.name, "Arena Copy");
    }

    #[test]
    fn place_with_symmetry_is_one_undo_transaction() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let document = MapDraftDocument::blank(
            MapDraftId(8),
            "Arena",
            crate::map::WIPEOUT_MODE_DEFINITION,
            MapDimensions {
                width: 32,
                height: 24,
            },
            MapPresentationThemeId(1),
            crate::map::GROUND_ASSET,
        )
        .unwrap();
        let mut session = MapEditorSession::open(document);
        session.symmetry = MapSymmetry::Rotate180;
        session
            .place(
                &catalog,
                crate::map::WALL_ARENA_ASSET,
                MapCell::new(2, 3),
                0,
            )
            .unwrap();
        assert_eq!(session.document.recipe.placements.len(), 2);
        assert!(session.undo());
        assert!(session.document.recipe.placements.is_empty());
    }

    #[test]
    fn paint_stroke_deduplicates_cells_and_is_one_undo_transaction() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let document = MapDraftDocument::blank(
            MapDraftId(10),
            "Stroke Arena",
            crate::map::WIPEOUT_MODE_DEFINITION,
            MapDimensions {
                width: 32,
                height: 24,
            },
            MapPresentationThemeId(1),
            crate::map::GROUND_ASSET,
        )
        .unwrap();
        let mut session = MapEditorSession::open(document);
        session
            .paint_stroke(
                &catalog,
                crate::map::WALL_ARENA_ASSET,
                [MapCell::new(2, 3), MapCell::new(2, 3), MapCell::new(3, 3)],
                0,
            )
            .unwrap();
        assert_eq!(session.document.recipe.placements.len(), 2);
        assert!(session.undo());
        assert!(session.document.recipe.placements.is_empty());
        assert!(session.can_redo());
    }

    #[test]
    fn resize_refuses_clipping_and_success_is_undoable() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let document = MapDraftDocument::blank(
            MapDraftId(11),
            "Resize Arena",
            crate::map::WIPEOUT_MODE_DEFINITION,
            MapDimensions {
                width: 32,
                height: 24,
            },
            MapPresentationThemeId(1),
            crate::map::GROUND_ASSET,
        )
        .unwrap();
        let mut session = MapEditorSession::open(document);
        session
            .place(
                &catalog,
                crate::map::WALL_ARENA_ASSET,
                MapCell::new(31, 23),
                0,
            )
            .unwrap();
        assert!(
            session
                .resize(MapDimensions {
                    width: 20,
                    height: 20,
                })
                .is_err()
        );
        assert_eq!(session.document.recipe.dimensions.width, 32);
        session
            .resize(MapDimensions {
                width: 40,
                height: 30,
            })
            .unwrap();
        assert_eq!(session.document.recipe.dimensions.width, 40);
        assert!(session.undo());
        assert_eq!(session.document.recipe.dimensions.width, 32);
    }
}
