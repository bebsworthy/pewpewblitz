//! Stable authored and resolved map data shared by authoritative and presentation roles.

use bevy::prelude::{Component, Resource, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

macro_rules! stable_id {
    ($name:ident, $inner:ty) => {
        #[derive(
            Serialize,
            Deserialize,
            Clone,
            Copy,
            Debug,
            Default,
            PartialEq,
            Eq,
            Hash,
            Ord,
            PartialOrd,
        )]
        pub struct $name(pub $inner);
    };
}

stable_id!(MapPresetId, u16);
stable_id!(MapRecipeId, u64);
#[derive(
    Component,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Hash,
    Ord,
    PartialOrd,
)]
pub struct MapInstanceId(pub u64);
stable_id!(MapRecipeFingerprint, u64);
stable_id!(MapPlacementId, u32);
stable_id!(MapPresentationThemeId, u16);
stable_id!(MapPresentationProfileId, u16);
stable_id!(MapObjectDefinitionId, u16);
stable_id!(MapVisualVariantId, u16);
stable_id!(CollisionProfileId, u16);
stable_id!(RegionProfileId, u16);
stable_id!(EntityDefinitionId, u16);
stable_id!(ModeDefinitionId, u16);
stable_id!(ModeAnchorDefinitionId, u16);
stable_id!(ModeAnchorId, u32);
stable_id!(SpawnPointId, u16);
stable_id!(RegionId, u16);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct AxisAlignedMapRect {
    pub min: Vec2,
    pub max: Vec2,
}

impl AxisAlignedMapRect {
    #[must_use]
    pub fn center(self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    #[must_use]
    pub fn size(self) -> Vec2 {
        self.max - self.min
    }

    #[must_use]
    pub fn contains(self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    #[must_use]
    pub fn contains_with_inset(self, point: Vec2, inset: f32) -> bool {
        point.x >= self.min.x + inset
            && point.x <= self.max.x - inset
            && point.y >= self.min.y + inset
            && point.y <= self.max.y - inset
    }

    #[must_use]
    pub fn clamp_circle(self, point: Vec2, radius: f32) -> Vec2 {
        point.clamp(
            self.min + Vec2::splat(radius),
            self.max - Vec2::splat(radius),
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum MapShape {
    Rectangle { half_extents: Vec2 },
    Circle { radius: f32 },
}

/// A normalized, axis-aligned objective area derived from one resolved area anchor.
///
/// Containment is inclusive on the boundary: a fighter center exactly on the circle or
/// rectangle edge counts as inside. Server occupancy uses this math authoritatively;
/// clients may repeat it only for presentation.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct NormalizedArea {
    pub center: Vec2,
    pub shape: MapShape,
}

impl NormalizedArea {
    #[must_use]
    pub fn contains_point(&self, point: Vec2) -> bool {
        let delta = point - self.center;
        match self.shape {
            MapShape::Rectangle { half_extents } => {
                delta.x.abs() <= half_extents.x && delta.y.abs() <= half_extents.y
            }
            MapShape::Circle { radius } => delta.length_squared() <= radius * radius,
        }
    }
}

impl MapShape {
    #[must_use]
    pub fn bounding_half_extents(self, rotation: f32) -> Vec2 {
        match self {
            Self::Rectangle { half_extents } => {
                let (sin, cos) = rotation.sin_cos();
                Vec2::new(
                    cos.abs() * half_extents.x + sin.abs() * half_extents.y,
                    sin.abs() * half_extents.x + cos.abs() * half_extents.y,
                )
            }
            Self::Circle { radius } => Vec2::splat(radius),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GeometryPlacement {
    pub placement_id: MapPlacementId,
    pub object_definition_id: MapObjectDefinitionId,
    pub visual_variant_id: Option<MapVisualVariantId>,
    pub collision_profile_id: CollisionProfileId,
    pub presentation_profile_id: Option<MapPresentationProfileId>,
    pub position: Vec2,
    pub rotation: f32,
    pub shape: MapShape,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum VisualPlacementKind {
    TiledRectangle { half_extents: Vec2, cell_size: Vec2 },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VisualPlacement {
    pub placement_id: MapPlacementId,
    pub presentation_profile_id: MapPresentationProfileId,
    pub position: Vec2,
    pub rotation: f32,
    pub kind: VisualPlacementKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapEntityPlacement {
    pub placement_id: MapPlacementId,
    pub object_definition_id: MapObjectDefinitionId,
    pub visual_variant_id: Option<MapVisualVariantId>,
    pub definition_id: EntityDefinitionId,
    pub presentation_profile_id: MapPresentationProfileId,
    pub position: Vec2,
    pub rotation: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapRegionPlacement {
    pub placement_id: MapPlacementId,
    pub region_id: RegionId,
    pub profile_id: RegionProfileId,
    pub presentation_profile_id: MapPresentationProfileId,
    pub position: Vec2,
    pub rotation: f32,
    pub shape: MapShape,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TeamSpawnArea {
    pub placement_id: MapPlacementId,
    pub team_slot: u8,
    pub bounds: AxisAlignedMapRect,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TeamSpawnPoint {
    pub placement_id: MapPlacementId,
    pub spawn_point_id: SpawnPointId,
    pub team_slot: u8,
    pub position: Vec2,
    pub facing: f32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum ModeAnchorShape {
    Point { position: Vec2, facing: f32 },
    Area { position: Vec2, shape: MapShape },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ModeAnchorPlacement {
    pub placement_id: MapPlacementId,
    pub anchor_id: ModeAnchorId,
    pub definition_id: ModeAnchorDefinitionId,
    pub shape: ModeAnchorShape,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapRecipe {
    pub recipe_id: MapRecipeId,
    pub revision: u32,
    pub recipe_version: u16,
    pub mode_definition_id: ModeDefinitionId,
    pub presentation_theme_id: MapPresentationThemeId,
    pub playable_bounds: AxisAlignedMapRect,
    pub camera_bounds: AxisAlignedMapRect,
    pub geometry: Vec<GeometryPlacement>,
    pub visuals: Vec<VisualPlacement>,
    pub entities: Vec<MapEntityPlacement>,
    pub regions: Vec<MapRegionPlacement>,
    pub spawn_areas: Vec<TeamSpawnArea>,
    pub spawn_points: Vec<TeamSpawnPoint>,
    pub mode_anchors: Vec<ModeAnchorPlacement>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ResolvedVisualInstance {
    pub placement_id: MapPlacementId,
    pub instance_index: u16,
    pub presentation_profile_id: MapPresentationProfileId,
    pub position: Vec2,
    pub rotation: f32,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedMapIdentity {
    pub instance_id: MapInstanceId,
    pub source_preset_id: Option<MapPresetId>,
    pub recipe_id: MapRecipeId,
    pub recipe_revision: u32,
    pub recipe_fingerprint: MapRecipeFingerprint,
}

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ResolvedMapSnapshot {
    pub identity: ResolvedMapIdentity,
    pub catalog_schema_version: u16,
    pub recipe_schema_version: u16,
    pub layout_schema_version: u16,
    pub presentation_theme_id: MapPresentationThemeId,
    pub mode_definition_id: ModeDefinitionId,
    pub playable_bounds: AxisAlignedMapRect,
    pub camera_bounds: AxisAlignedMapRect,
    pub geometry: Vec<GeometryPlacement>,
    pub visual_instances: Vec<ResolvedVisualInstance>,
    pub entities: Vec<MapEntityPlacement>,
    pub regions: Vec<MapRegionPlacement>,
    pub spawn_areas: Vec<TeamSpawnArea>,
    pub spawn_points: Vec<TeamSpawnPoint>,
    pub mode_anchors: Vec<ModeAnchorPlacement>,
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct ResolvedMap {
    pub snapshot: ResolvedMapSnapshot,
    pub spawn_points_by_team: BTreeMap<u8, Vec<TeamSpawnPoint>>,
    pub regions_by_id: BTreeMap<RegionId, MapRegionPlacement>,
    pub anchors_by_definition: BTreeMap<ModeAnchorDefinitionId, Vec<ModeAnchorPlacement>>,
}

impl ResolvedMap {
    #[must_use]
    pub fn from_snapshot(snapshot: ResolvedMapSnapshot) -> Self {
        let mut spawn_points_by_team: BTreeMap<u8, Vec<TeamSpawnPoint>> = BTreeMap::new();
        for point in &snapshot.spawn_points {
            spawn_points_by_team
                .entry(point.team_slot)
                .or_default()
                .push(point.clone());
        }
        let regions_by_id = snapshot
            .regions
            .iter()
            .map(|region| (region.region_id, region.clone()))
            .collect();
        let mut anchors_by_definition: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for anchor in &snapshot.mode_anchors {
            anchors_by_definition
                .entry(anchor.definition_id)
                .or_default()
                .push(anchor.clone());
        }
        Self {
            snapshot,
            spawn_points_by_team,
            regions_by_id,
            anchors_by_definition,
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct PlayableBounds(pub AxisAlignedMapRect);

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct SpawnPointCatalog(pub BTreeMap<u8, Vec<TeamSpawnPoint>>);

impl SpawnPointCatalog {
    #[must_use]
    pub fn deterministic_point(&self, team_slot: u8, ordinal: u64) -> Option<&TeamSpawnPoint> {
        let points = self.0.get(&team_slot)?;
        let index = usize::try_from(ordinal).ok()? % points.len();
        points.get(index)
    }
}

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct PracticeDummySpawn {
    pub position: Vec2,
    pub facing: f32,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MapRoot;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapInstanceMember {
    pub map_instance_id: MapInstanceId,
    pub placement_id: MapPlacementId,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnAssignment {
    pub map_instance_id: MapInstanceId,
    pub spawn_point_id: SpawnPointId,
}
