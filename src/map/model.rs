//! Stable identities and generic runtime facts shared by every map role.

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
stable_id!(MapRecipeFingerprint, u64);
stable_id!(MapPlacementId, u32);
stable_id!(MapPresentationThemeId, u16);
stable_id!(ModeDefinitionId, u16);
stable_id!(ModeAnchorId, u32);
stable_id!(SpawnPointId, u16);

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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TeamSpawnPoint {
    pub placement_id: MapPlacementId,
    pub spawn_point_id: SpawnPointId,
    pub team_slot: u8,
    pub position: Vec2,
    pub facing: f32,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedMapIdentity {
    pub instance_id: MapInstanceId,
    pub source_preset_id: Option<MapPresetId>,
    pub recipe_id: MapRecipeId,
    pub recipe_revision: u32,
    pub recipe_fingerprint: MapRecipeFingerprint,
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
