use super::*;
use crate::movement::ArenaWall;
use bevy::prelude::*;

#[derive(Component)]
struct Unrelated;

fn authoritative_map_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, MapContentPlugin, AuthoritativeMapPlugin));
    app
}

#[test]
fn authoritative_plugin_instantiates_one_root_and_exact_collider_set() {
    let mut app = authoritative_map_app();
    app.update();
    let world = app.world_mut();
    let roots = world
        .query_filtered::<(&MapInstanceId, &ResolvedMapSnapshot), With<MapRoot>>()
        .iter(world)
        .count();
    let walls = world
        .query_filtered::<(&MapInstanceMember, &avian2d::prelude::Collider), With<ArenaWall>>()
        .iter(world)
        .count();
    assert_eq!(roots, 1);
    assert_eq!(walls, 10);
    assert_eq!(world.resource::<ResolvedMap>().snapshot.geometry.len(), 6);
    assert_eq!(world.resource::<SpawnPointCatalog>().0.len(), 2);
    assert!(
        world
            .resource::<ResolvedMap>()
            .snapshot
            .mode_anchors
            .is_empty()
    );

    app.update();
    let world = app.world_mut();
    assert_eq!(
        world
            .query_filtered::<Entity, With<MapRoot>>()
            .iter(world)
            .count(),
        1
    );
    assert_eq!(
        world
            .query_filtered::<Entity, With<ArenaWall>>()
            .iter(world)
            .count(),
        10
    );
}

#[test]
fn replacement_cleans_only_the_previous_map_generation() {
    let mut app = authoritative_map_app();
    app.update();
    let unrelated = app.world_mut().spawn(Unrelated).id();
    let catalog = app.world().resource::<MapCatalogResource>().0.clone();
    let replacement = catalog
        .resolve_preset(
            BUILT_IN_MAP_PRESET,
            MapInstanceId(2),
            &MapLayoutRequirements::wipeout(),
        )
        .unwrap();
    install_resolved_map(app.world_mut(), replacement).unwrap();
    let world = app.world_mut();
    let root_instance = world
        .query_filtered::<&MapInstanceId, With<MapRoot>>()
        .single(world)
        .copied()
        .unwrap();
    assert_eq!(root_instance, MapInstanceId(2));
    assert!(world.get::<Unrelated>(unrelated).is_some());
    assert!(
        world
            .query::<&MapInstanceMember>()
            .iter(world)
            .all(|member| member.map_instance_id == MapInstanceId(2))
    );
}

#[test]
fn deterministic_spawn_lookup_is_stable_per_team() {
    let mut app = authoritative_map_app();
    app.update();
    let catalog = app.world().resource::<SpawnPointCatalog>();
    assert_eq!(
        catalog.deterministic_point(0, 0).unwrap().spawn_point_id,
        SpawnPointId(1)
    );
    assert_eq!(
        catalog.deterministic_point(0, 4).unwrap().spawn_point_id,
        SpawnPointId(1)
    );
    assert_eq!(
        catalog.deterministic_point(1, 0).unwrap().spawn_point_id,
        SpawnPointId(5)
    );
}
