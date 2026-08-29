//! Shared serialized resolved-map snapshots and dynamic generation state.

use super::{
    MapAssetId, MapAssetPlacement, MapDimensions, MapInstanceId, MapModeAnchorPlacement,
    MapPlacementId, MapPresentationThemeId, ModeDefinitionId, ResolvedMapIdentity,
};
use bevy::prelude::Component;
use serde::{Deserialize, Serialize};

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ResolvedMapSnapshot {
    pub identity: ResolvedMapIdentity,
    pub catalog_schema_version: u16,
    pub recipe_schema_version: u16,
    pub presentation_theme_id: MapPresentationThemeId,
    pub mode_definition_id: ModeDefinitionId,
    pub dimensions: MapDimensions,
    pub default_surface_asset_id: MapAssetId,
    pub placements: Vec<MapAssetPlacement>,
    pub mode_anchors: Vec<MapModeAnchorPlacement>,
}

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MapDynamicState {
    pub map_instance_id: MapInstanceId,
    pub generation: u64,
    pub revision: u64,
    pub terminal_states: Vec<MapPlacementTransition>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapDynamicGeneration {
    pub map_instance_id: MapInstanceId,
    pub generation: u64,
}

impl MapDynamicState {
    #[must_use]
    pub const fn generation_id(&self) -> MapDynamicGeneration {
        MapDynamicGeneration {
            map_instance_id: self.map_instance_id,
            generation: self.generation,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MapMutationEvent {
    pub generation: MapDynamicGeneration,
    pub revision: u64,
    pub transitions: Vec<MapPlacementTransition>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub enum MapPlacementOutcome {
    Removed,
    ReplacedWith(MapAssetId),
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct MapPlacementTransition {
    pub placement_id: MapPlacementId,
    pub outcome: MapPlacementOutcome,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapDynamicResetEvent {
    pub previous_generation: MapDynamicGeneration,
    pub next_generation: MapDynamicGeneration,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapDynamicRecoveryRequest {
    pub generation: MapDynamicGeneration,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MapDynamicRecoverySnapshot {
    pub state: MapDynamicState,
}
