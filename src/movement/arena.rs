//! Shared collision semantics and geometry helpers for resolved authoritative maps.

use avian2d::prelude::{CollisionLayers, LayerMask};
use bevy::prelude::{Component, Vec2};

pub const CAMERA_VERTICAL_SPAN: f32 = 720.0;

/// Semantic marker for static authoritative map collider entities.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArenaWall;

/// Typed collision-layer reservation for the first combat milestones.
pub const FIGHTER_LAYER: LayerMask = LayerMask(1 << 1);
pub const PROJECTILE_LAYER: LayerMask = LayerMask(1 << 2);
pub const STATIC_MAP_LAYER: LayerMask = LayerMask(1 << 3);
pub const DESTRUCTIBLE_MAP_LAYER: LayerMask = LayerMask(1 << 4);
pub const OBJECTIVE_LAYER: LayerMask = LayerMask(1 << 5);
pub const PICKUP_LAYER: LayerMask = LayerMask(1 << 6);
pub const HAZARD_LAYER: LayerMask = LayerMask(1 << 7);
pub const DEPLOYABLE_LAYER: LayerMask = LayerMask(1 << 8);
/// Map surfaces such as deep water that block fighters and deployables while allowing shots.
pub const PLAYER_ONLY_MAP_LAYER: LayerMask = LayerMask(1 << 9);

#[must_use]
pub fn fighter_collision_layers() -> CollisionLayers {
    CollisionLayers::new(
        FIGHTER_LAYER,
        STATIC_MAP_LAYER | DESTRUCTIBLE_MAP_LAYER | PLAYER_ONLY_MAP_LAYER,
    )
}

#[must_use]
pub fn player_only_map_collision_layers() -> CollisionLayers {
    CollisionLayers::new(PLAYER_ONLY_MAP_LAYER, FIGHTER_LAYER | DEPLOYABLE_LAYER)
}

#[must_use]
pub fn map_collision_layers() -> CollisionLayers {
    CollisionLayers::new(
        STATIC_MAP_LAYER,
        FIGHTER_LAYER | PROJECTILE_LAYER | DEPLOYABLE_LAYER,
    )
}

/// Collision layers for authoritative destructible-map colliders. Queries that
/// already combine both map membership masks see permanent and destructible geometry
/// identically.
#[must_use]
pub fn destructible_map_collision_layers() -> CollisionLayers {
    CollisionLayers::new(
        DESTRUCTIBLE_MAP_LAYER,
        FIGHTER_LAYER | PROJECTILE_LAYER | DEPLOYABLE_LAYER,
    )
}

#[must_use]
pub fn pose_is_valid(
    position: Vec2,
    facing: f32,
    bounds: crate::map::PlayableBounds,
    radius: f32,
) -> bool {
    let min = bounds.0.min + Vec2::splat(radius);
    let max = bounds.0.max - Vec2::splat(radius);
    position.is_finite()
        && facing.is_finite()
        && position.x >= min.x
        && position.x <= max.x
        && position.y >= min.y
        && position.y <= max.y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_only_map_layer_blocks_fighters_and_deployables_but_not_projectiles() {
        let surface = player_only_map_collision_layers();
        let fighter = fighter_collision_layers();
        let deployable =
            CollisionLayers::new(DEPLOYABLE_LAYER, PLAYER_ONLY_MAP_LAYER | STATIC_MAP_LAYER);
        let projectile = CollisionLayers::new(
            PROJECTILE_LAYER,
            FIGHTER_LAYER | STATIC_MAP_LAYER | DESTRUCTIBLE_MAP_LAYER,
        );
        assert!(surface.interacts_with(fighter));
        assert!(surface.interacts_with(deployable));
        assert!(!surface.interacts_with(projectile));
    }
}
