//! Atomic resolved-map installation and static/dynamic collider materialization.

use avian2d::prelude::{Collider, Position, RigidBody, Rotation};
use bevy::prelude::*;
use lightyear::prelude::{NetworkTarget, Replicate};

use crate::movement::{
    ArenaWall, destructible_map_collision_layers, player_only_map_collision_layers,
};

use super::{DestructibleMapCollider, MapDynamicOutbox, PlayerOnlyMapCollider};
use crate::map::{
    MapCatalogResource, MapDynamicGeneration, MapDynamicState, MapInstanceId, MapInstanceMember,
    MapPlacementId, ResolvedMap, ResolvedMapSnapshot, placement_world_center,
};
pub fn install_resolved_map(world: &mut World, resolved: ResolvedMap) -> Result<(), String> {
    let instance_id = resolved.snapshot.identity.instance_id;
    if instance_id.0 == 0 {
        return Err("cannot install a zero map instance".to_string());
    }
    validate_resolved_installation(world, &resolved)?;
    let snapshot = resolved.snapshot.clone();
    let dynamic_placements = resolved.dynamic_placements.clone();
    let player_only_surface_rects = resolved.player_only_surface_rects.clone();
    let static_colliders = resolved.static_colliders.clone();
    let spawn_points = resolved.spawn_points_by_team.clone();
    let objective_zone = resolved.objective_zone;
    crate::map::server::teardown_authoritative_map(world);
    if let Some(anchor) = objective_zone {
        world.insert_resource(crate::matchplay::ResolvedObjectiveZone {
            anchor_id: anchor.anchor_id,
            area: anchor.area,
        });
    }
    *world.resource_mut::<MapDynamicOutbox>() = MapDynamicOutbox::default();
    world.spawn((
        crate::map::MapRoot,
        instance_id,
        snapshot.identity,
        snapshot.clone(),
        MapDynamicState {
            map_instance_id: instance_id,
            generation: 1,
            revision: 0,
            terminal_states: Vec::new(),
        },
        Replicate::to_clients(NetworkTarget::All),
    ));
    for (index, (position, size)) in
        crate::map::server::perimeter_wall_shapes(snapshot.dimensions.bounds())
            .into_iter()
            .enumerate()
    {
        spawn_static_collider(
            world,
            instance_id,
            MapPlacementId(u32::MAX - u32::try_from(index).expect("four perimeter indices fit")),
            position,
            crate::map::MapShape::Rectangle {
                half_extents: size * 0.5,
            },
        );
    }
    for collider in static_colliders {
        spawn_static_collider(
            world,
            instance_id,
            collider.placement_id,
            collider.position,
            collider.shape,
        );
    }
    world.insert_resource(crate::map::PlayableBounds(snapshot.dimensions.bounds()));
    world.insert_resource(crate::map::SpawnPointCatalog(spawn_points));
    world.insert_resource(resolved);
    let catalog = world.resource::<MapCatalogResource>().0.clone();
    for placement in dynamic_placements {
        let asset = catalog
            .asset(placement.asset_id)
            .ok_or_else(|| "resolved dynamic asset disappeared".to_string())?;
        spawn_dynamic_collider(world, instance_id, 1, &snapshot, asset, &placement);
    }
    for rectangle in player_only_surface_rects {
        let size = Vec2::new(f32::from(rectangle.width), f32::from(rectangle.height))
            * crate::map::MAP_CELL_SIZE_WORLD;
        let center = snapshot.dimensions.cell_min(rectangle.min) + size * 0.5;
        let placement_id = snapshot
            .placements
            .iter()
            .filter(|placement| {
                placement.cell.x >= rectangle.min.x
                    && placement.cell.x < rectangle.min.x + rectangle.width
                    && placement.cell.y >= rectangle.min.y
                    && placement.cell.y < rectangle.min.y + rectangle.height
            })
            .map(|placement| placement.placement_id)
            .min()
            .ok_or_else(|| "player-only surface placement disappeared".to_string())?;
        world.spawn((
            ArenaWall,
            PlayerOnlyMapCollider,
            MapInstanceMember {
                map_instance_id: instance_id,
                placement_id,
            },
            RigidBody::Static,
            Collider::rectangle(size.x, size.y),
            player_only_map_collision_layers(),
            Position::from_xy(center.x, center.y),
            Rotation::default(),
            Transform::from_translation(center.extend(0.0)),
        ));
    }
    Ok(())
}

fn validate_resolved_installation(world: &World, resolved: &ResolvedMap) -> Result<(), String> {
    let catalog = &world.resource::<MapCatalogResource>().0;
    for placement in &resolved.dynamic_placements {
        if catalog.asset(placement.asset_id).is_none() {
            return Err("resolved dynamic asset disappeared".to_string());
        }
    }
    for rectangle in &resolved.player_only_surface_rects {
        let has_member = resolved.snapshot.placements.iter().any(|placement| {
            placement.cell.x >= rectangle.min.x
                && placement.cell.x < rectangle.min.x + rectangle.width
                && placement.cell.y >= rectangle.min.y
                && placement.cell.y < rectangle.min.y + rectangle.height
        });
        if !has_member {
            return Err("player-only surface placement disappeared".to_string());
        }
    }
    Ok(())
}

fn spawn_static_collider(
    world: &mut World,
    instance_id: MapInstanceId,
    placement_id: MapPlacementId,
    position: Vec2,
    shape: crate::map::MapShape,
) {
    let collider = match shape {
        crate::map::MapShape::Rectangle { half_extents } => {
            Collider::rectangle(half_extents.x * 2.0, half_extents.y * 2.0)
        }
        crate::map::MapShape::Circle { radius } => Collider::circle(radius),
    };
    world.spawn((
        ArenaWall,
        MapInstanceMember {
            map_instance_id: instance_id,
            placement_id,
        },
        RigidBody::Static,
        collider,
        crate::movement::map_collision_layers(),
        Position::from_xy(position.x, position.y),
        Rotation::default(),
        Transform::from_translation(position.extend(0.0)),
    ));
}

pub(super) fn spawn_dynamic_collider(
    world: &mut World,
    map_instance_id: MapInstanceId,
    map_generation: u64,
    snapshot: &ResolvedMapSnapshot,
    asset: &crate::map::MapAssetDefinition,
    placement: &crate::map::MapAssetPlacement,
) {
    let footprint = asset.footprint_cells.rotated(placement.quarter_turns);
    let center = placement_world_center(snapshot.dimensions, asset, placement);
    let profile = world
        .resource::<MapCatalogResource>()
        .0
        .profile(asset.gameplay_profile_id)
        .copied()
        .expect("resolved dynamic asset profile exists");
    let collider = match profile.collider_shape {
        crate::map::MapColliderShape::FootprintRectangle => Collider::rectangle(
            f32::from(footprint.width) * crate::map::MAP_CELL_SIZE_WORLD,
            f32::from(footprint.height) * crate::map::MAP_CELL_SIZE_WORLD,
        ),
        crate::map::MapColliderShape::Circle { radius_world_units } => {
            Collider::circle(f32::from(radius_world_units))
        }
        crate::map::MapColliderShape::None => return,
    };
    let entity = world
        .spawn((
            ArenaWall,
            MapInstanceMember {
                map_instance_id,
                placement_id: placement.placement_id,
            },
            RigidBody::Static,
            collider,
            destructible_map_collision_layers(),
            Position::from_xy(center.x, center.y),
            Rotation::default(),
            Transform::from_translation(center.extend(0.0)),
        ))
        .id();
    match profile.durability {
        crate::map::MapDurabilityBehavior::Indestructible => {
            world.entity_mut(entity).insert(DestructibleMapCollider {
                placement_id: placement.placement_id,
            });
        }
        crate::map::MapDurabilityBehavior::HitPoints(damage_profile_id) => {
            let maximum_health = world
                .resource::<MapCatalogResource>()
                .0
                .damage_profile(damage_profile_id)
                .expect("validated damage profile exists")
                .maximum_health;
            world.entity_mut(entity).insert((
                crate::map::DamageableWorldObject,
                crate::map::DamageableTargetIdentity::MapObject {
                    generation: MapDynamicGeneration {
                        map_instance_id,
                        generation: map_generation,
                    },
                    placement_id: placement.placement_id,
                },
                crate::map::DamageableTargetClass::EnvironmentObject,
                crate::map::DamageableMaximumHealth(maximum_health),
                crate::map::DamageableObjectProfile(damage_profile_id),
                crate::map::DamageableObjectAsset(asset.id),
                crate::map::DamageableLifeState::Live,
                crate::combat::CurrentHealth(maximum_health),
                Replicate::to_clients(NetworkTarget::All),
            ));
        }
    }
}
