//! Authoritative map resolution, instantiation, and exact-generation lifecycle.
#![allow(clippy::wildcard_imports)]

use super::*;
use crate::movement::{ArenaWall, terrain_collision_layers};
use avian2d::prelude::{Collider, Position, RigidBody, Rotation};
use bevy::prelude::*;
use lightyear::prelude::{NetworkTarget, Replicate};

pub const BUILT_IN_MAP_PRESET: MapPresetId = MapPresetId(1);
pub const HOT_ZONE_MAP_PRESET: MapPresetId = MapPresetId(2);
pub const ARENA_WALL_THICKNESS: f32 = 48.0;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapStartupSet {
    Instantiate,
}

/// Server-owned selection of which built-in map preset to install. Derived once from the
/// validated game-mode configuration; the preset's mode definition picks the layout
/// requirements, so a mode/map mismatch fails startup.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServerMapSelection {
    pub preset_id: MapPresetId,
}

impl Default for ServerMapSelection {
    fn default() -> Self {
        Self {
            preset_id: BUILT_IN_MAP_PRESET,
        }
    }
}

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct NextMapInstanceId(u64);

impl Default for NextMapInstanceId {
    fn default() -> Self {
        Self(1)
    }
}

impl NextMapInstanceId {
    fn allocate(&mut self) -> Option<MapInstanceId> {
        let current = self.0;
        self.0 = current.checked_add(1)?;
        (current != 0).then_some(MapInstanceId(current))
    }
}

pub struct AuthoritativeMapPlugin;

impl Plugin for AuthoritativeMapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapCatalogResource>()
            .init_resource::<ServerMapSelection>()
            .init_resource::<NextMapInstanceId>()
            .configure_sets(Startup, MapStartupSet::Instantiate)
            .add_systems(
                Startup,
                initialize_authoritative_map.in_set(MapStartupSet::Instantiate),
            );
    }
}

fn initialize_authoritative_map(world: &mut World) {
    let mut roots = world.query_filtered::<Entity, With<MapRoot>>();
    if roots.iter(world).next().is_some() {
        return;
    }
    let instance_id = world
        .resource_mut::<NextMapInstanceId>()
        .allocate()
        .expect("map instance identifier space is available");
    let catalog = world.resource::<MapCatalogResource>().0.clone();
    let selection = *world.resource::<ServerMapSelection>();
    let preset = catalog
        .preset(selection.preset_id)
        .expect("selected built-in map preset exists");
    let requirements = MapLayoutRequirements::for_mode_definition(preset.recipe.mode_definition_id)
        .expect("selected built-in map preset has a known mode");
    let resolved = catalog
        .resolve_preset(selection.preset_id, instance_id, &requirements)
        .expect("embedded built-in map must resolve");
    install_resolved_map(world, resolved).expect("resolved built-in map must instantiate");
}

pub fn install_resolved_map(world: &mut World, resolved: ResolvedMap) -> Result<(), String> {
    let snapshot = &resolved.snapshot;
    if snapshot.identity.instance_id.0 == 0 {
        return Err("cannot install a zero map instance".to_string());
    }
    teardown_authoritative_map(world);
    let instance_id = snapshot.identity.instance_id;
    world.spawn((
        MapRoot,
        instance_id,
        snapshot.identity,
        snapshot.clone(),
        Replicate::to_clients(NetworkTarget::All),
    ));
    for (index, (position, size)) in perimeter_wall_shapes(snapshot.playable_bounds)
        .into_iter()
        .enumerate()
    {
        let index = u32::try_from(index).expect("four perimeter indices fit u32");
        spawn_wall(
            world,
            instance_id,
            MapPlacementId(u32::MAX - index),
            position,
            0.0,
            MapShape::Rectangle {
                half_extents: size * 0.5,
            },
        );
    }
    for geometry in &snapshot.geometry {
        spawn_wall(
            world,
            instance_id,
            geometry.placement_id,
            geometry.position,
            geometry.rotation,
            geometry.shape,
        );
    }
    world.insert_resource(PlayableBounds(snapshot.playable_bounds));
    world.insert_resource(SpawnPointCatalog(resolved.spawn_points_by_team.clone()));
    world.insert_resource(resolved);
    Ok(())
}

pub fn teardown_authoritative_map(world: &mut World) {
    let roots: Vec<_> = {
        let mut query = world.query_filtered::<Entity, With<MapRoot>>();
        query.iter(world).collect()
    };
    let instances: Vec<_> = roots
        .iter()
        .filter_map(|entity| world.get::<MapInstanceId>(*entity).copied())
        .collect();
    let members: Vec<_> = {
        let mut query = world.query::<(Entity, &MapInstanceMember)>();
        query
            .iter(world)
            .filter(|(_, member)| instances.contains(&member.map_instance_id))
            .map(|(entity, _)| entity)
            .collect()
    };
    for entity in members.into_iter().chain(roots) {
        if let Ok(entity_mut) = world.get_entity_mut(entity) {
            entity_mut.despawn();
        }
    }
    world.remove_resource::<ResolvedMap>();
    world.remove_resource::<PlayableBounds>();
    world.remove_resource::<SpawnPointCatalog>();
}

fn spawn_wall(
    world: &mut World,
    instance_id: MapInstanceId,
    placement_id: MapPlacementId,
    position: Vec2,
    rotation: f32,
    shape: MapShape,
) {
    let collider = match shape {
        MapShape::Rectangle { half_extents } => {
            Collider::rectangle(half_extents.x * 2.0, half_extents.y * 2.0)
        }
        MapShape::Circle { radius } => Collider::circle(radius),
    };
    world.spawn((
        ArenaWall,
        MapInstanceMember {
            map_instance_id: instance_id,
            placement_id,
        },
        RigidBody::Static,
        collider,
        terrain_collision_layers(),
        Position::from_xy(position.x, position.y),
        Rotation::radians(rotation),
        Transform::from_translation(position.extend(0.0)),
    ));
}

#[must_use]
pub fn perimeter_wall_shapes(bounds: AxisAlignedMapRect) -> [(Vec2, Vec2); 4] {
    let thickness = ARENA_WALL_THICKNESS;
    let size = bounds.size();
    let center = bounds.center();
    [
        (
            Vec2::new(bounds.min.x - thickness * 0.5, center.y),
            Vec2::new(thickness, size.y + thickness * 2.0),
        ),
        (
            Vec2::new(bounds.max.x + thickness * 0.5, center.y),
            Vec2::new(thickness, size.y + thickness * 2.0),
        ),
        (
            Vec2::new(center.x, bounds.min.y - thickness * 0.5),
            Vec2::new(size.x, thickness),
        ),
        (
            Vec2::new(center.x, bounds.max.y + thickness * 0.5),
            Vec2::new(size.x, thickness),
        ),
    ]
}
