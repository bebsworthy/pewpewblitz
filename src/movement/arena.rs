//! Shared arena geometry, spawn layout, and collision-layer reservations.

use avian2d::prelude::{CollisionLayers, LayerMask};
use bevy::prelude::{Component, Resource, Vec2};

pub const ARENA_MIN: Vec2 = Vec2::new(-800.0, -500.0);
pub const ARENA_MAX: Vec2 = Vec2::new(800.0, 500.0);
pub const CAMERA_VERTICAL_SPAN: f32 = 720.0;
pub const ARENA_WALL_THICKNESS: f32 = 48.0;

/// Immutable, code-authored greybox geometry shared by authoritative and client composition.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct GreyboxArenaDefinition {
    pub min: Vec2,
    pub max: Vec2,
    pub cover_centers: [Vec2; 2],
    pub cover_size: Vec2,
    pub spawn_x: [f32; 2],
    pub spawn_y: [f32; 4],
}

impl Default for GreyboxArenaDefinition {
    fn default() -> Self {
        Self {
            min: ARENA_MIN,
            max: ARENA_MAX,
            cover_centers: [Vec2::new(0.0, -220.0), Vec2::new(0.0, 220.0)],
            cover_size: Vec2::new(180.0, 120.0),
            spawn_x: [-620.0, 620.0],
            spawn_y: [-300.0, -100.0, 100.0, 300.0],
        }
    }
}

impl GreyboxArenaDefinition {
    #[must_use]
    pub fn spawn_slot(player_id: u64) -> u8 {
        u8::try_from(player_id.saturating_sub(1) % 8).expect("spawn slot modulo fits in u8")
    }

    #[must_use]
    pub fn spawn_position(self, player_id: u64) -> Vec2 {
        let slot = usize::from(Self::spawn_slot(player_id));
        Vec2::new(self.spawn_x[slot % 2], self.spawn_y[slot / 2])
    }

    #[must_use]
    pub fn perimeter_wall_shapes(self) -> [(Vec2, Vec2); 4] {
        let thickness = ARENA_WALL_THICKNESS;
        let width = self.max.x - self.min.x;
        let height = self.max.y - self.min.y;
        let center = (self.min + self.max) / 2.0;
        [
            (
                Vec2::new(self.min.x - thickness / 2.0, center.y),
                Vec2::new(thickness, height + thickness * 2.0),
            ),
            (
                Vec2::new(self.max.x + thickness / 2.0, center.y),
                Vec2::new(thickness, height + thickness * 2.0),
            ),
            (
                Vec2::new(center.x, self.min.y - thickness / 2.0),
                Vec2::new(width, thickness),
            ),
            (
                Vec2::new(center.x, self.max.y + thickness / 2.0),
                Vec2::new(width, thickness),
            ),
        ]
    }

    /// Return an in-bounds debug representation of the perimeter collision faces.
    #[must_use]
    pub fn perimeter_visual_shapes(self) -> [(Vec2, Vec2); 4] {
        const VISUAL_THICKNESS: f32 = 24.0;
        let width = self.max.x - self.min.x;
        let height = self.max.y - self.min.y;
        let center = (self.min + self.max) / 2.0;
        [
            (
                Vec2::new(self.min.x + VISUAL_THICKNESS / 2.0, center.y),
                Vec2::new(VISUAL_THICKNESS, height),
            ),
            (
                Vec2::new(self.max.x - VISUAL_THICKNESS / 2.0, center.y),
                Vec2::new(VISUAL_THICKNESS, height),
            ),
            (
                Vec2::new(center.x, self.min.y + VISUAL_THICKNESS / 2.0),
                Vec2::new(width, VISUAL_THICKNESS),
            ),
            (
                Vec2::new(center.x, self.max.y - VISUAL_THICKNESS / 2.0),
                Vec2::new(width, VISUAL_THICKNESS),
            ),
        ]
    }

    /// Return the high-contrast inner edge for the in-bounds perimeter debug geometry.
    #[must_use]
    pub fn perimeter_visual_edge_shapes(self) -> [(Vec2, Vec2); 4] {
        const VISUAL_THICKNESS: f32 = 24.0;
        const EDGE_THICKNESS: f32 = 6.0;
        let edge_offset = VISUAL_THICKNESS - EDGE_THICKNESS / 2.0;
        let width = self.max.x - self.min.x;
        let height = self.max.y - self.min.y;
        let center = (self.min + self.max) / 2.0;
        [
            (
                Vec2::new(self.min.x + edge_offset, center.y),
                Vec2::new(EDGE_THICKNESS, height),
            ),
            (
                Vec2::new(self.max.x - edge_offset, center.y),
                Vec2::new(EDGE_THICKNESS, height),
            ),
            (
                Vec2::new(center.x, self.min.y + edge_offset),
                Vec2::new(width, EDGE_THICKNESS),
            ),
            (
                Vec2::new(center.x, self.max.y - edge_offset),
                Vec2::new(width, EDGE_THICKNESS),
            ),
        ]
    }

    #[must_use]
    pub fn cover_shapes(self) -> [(Vec2, Vec2); 2] {
        self.cover_centers.map(|center| (center, self.cover_size))
    }

    #[must_use]
    pub fn clamp_position(self, position: Vec2, radius: f32) -> Vec2 {
        position.clamp(
            self.min + Vec2::splat(radius),
            self.max - Vec2::splat(radius),
        )
    }
}

/// Semantic marker for static greybox physics entities.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArenaWall;

/// Stable local marker for an arena spawn location.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnMarker(pub u8);

/// Typed collision-layer reservation for the first combat milestones.
pub const FIGHTER_LAYER: LayerMask = LayerMask(1 << 1);
pub const PROJECTILE_LAYER: LayerMask = LayerMask(1 << 2);
pub const INDESTRUCTIBLE_TERRAIN_LAYER: LayerMask = LayerMask(1 << 3);
pub const DESTRUCTIBLE_TERRAIN_LAYER: LayerMask = LayerMask(1 << 4);
pub const OBJECTIVE_LAYER: LayerMask = LayerMask(1 << 5);
pub const PICKUP_LAYER: LayerMask = LayerMask(1 << 6);
pub const HAZARD_LAYER: LayerMask = LayerMask(1 << 7);
pub const DEPLOYABLE_LAYER: LayerMask = LayerMask(1 << 8);

#[must_use]
pub fn fighter_collision_layers() -> CollisionLayers {
    CollisionLayers::new(
        FIGHTER_LAYER,
        INDESTRUCTIBLE_TERRAIN_LAYER | DESTRUCTIBLE_TERRAIN_LAYER,
    )
}

#[must_use]
pub fn terrain_collision_layers() -> CollisionLayers {
    CollisionLayers::new(
        INDESTRUCTIBLE_TERRAIN_LAYER,
        FIGHTER_LAYER | PROJECTILE_LAYER | DEPLOYABLE_LAYER,
    )
}

#[must_use]
pub fn pose_is_valid(
    position: Vec2,
    facing: f32,
    arena: GreyboxArenaDefinition,
    radius: f32,
) -> bool {
    let min = arena.min + Vec2::splat(radius);
    let max = arena.max - Vec2::splat(radius);
    position.is_finite()
        && facing.is_finite()
        && position.x >= min.x
        && position.x <= max.x
        && position.y >= min.y
        && position.y <= max.y
}
