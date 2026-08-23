//! Authoritative map resolution, instantiation, and exact-generation lifecycle.
#![allow(clippy::wildcard_imports)]

use super::*;
use bevy::prelude::*;

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
            preset_id: CROSSROADS_PRESET,
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
        register_map_runtime(app);
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
    let selection = *world.resource::<ServerMapSelection>();
    let resolved = world
        .resource::<MapCatalogResource>()
        .0
        .resolve_preset(selection.preset_id, instance_id)
        .expect("embedded grid map must resolve");
    install_resolved_map(world, resolved).expect("resolved grid map must instantiate");
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
    world.remove_resource::<crate::matchplay::ResolvedObjectiveZone>();
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
