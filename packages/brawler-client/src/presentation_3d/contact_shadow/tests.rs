use super::*;
use crate::presentation_3d::{
    STRAIGHT_PROJECTILE_HEIGHT, ToonMaterial, ToonShading, setup_3d_foundation,
};
use avian2d::prelude::Position;

fn tuning() -> ContactShadowTuning {
    ContactShadowTuning::embedded().expect("embedded contact shadow catalog")
}

fn catalog(color: (f32, f32, f32), core: f32, opacity: f32, size: f32) -> String {
    format!(
        "(schema_version: 1, color: ({}, {}, {}), core: {core}, \
         fighter: (opacity: {opacity}, size: {size}), \
         projectile: (opacity: 0.3, size: 1.5), prop: (opacity: 0.4, size: 1.2))",
        color.0, color.1, color.2
    )
}

#[test]
fn embedded_catalog_is_a_dark_soft_shadow_for_every_class() {
    let tuning = tuning();
    let (red, green, blue) = tuning.color;
    assert!(red.max(green).max(blue) <= MAX_SHADOW_CHANNEL);
    assert!(
        blue >= red && blue >= green,
        "navy-tinted, not neutral black"
    );
    for class in [
        ContactShadowClass::Fighter,
        ContactShadowClass::Projectile,
        ContactShadowClass::Prop,
    ] {
        let class = tuning.class(class);
        assert!(
            class.opacity > 0.0,
            "every class is grounded on the default tier"
        );
    }
}

#[test]
fn catalog_rejects_out_of_bounds_tuning() {
    let navy = (0.06, 0.07, 0.17);
    assert!(ContactShadowTuning::from_ron(&catalog(navy, 0.35, 0.5, 1.3)).is_ok());
    // Opacity 0 is the supported way to disable a class.
    assert!(ContactShadowTuning::from_ron(&catalog(navy, 0.35, 0.0, 1.3)).is_ok());
    for (color, core, opacity, size) in [
        ((0.9, 0.9, 0.9), 0.35, 0.5, 1.3),
        (navy, 1.0, 0.5, 1.3),
        (navy, 0.35, MAX_SHADOW_OPACITY + 0.1, 1.3),
        (navy, 0.35, -0.1, 1.3),
        (navy, 0.35, 0.5, MAX_SHADOW_SIZE + 1.0),
        (navy, 0.35, 0.5, f32::NAN),
    ] {
        assert!(
            ContactShadowTuning::from_ron(&catalog(color, core, opacity, size)).is_err(),
            "color {color:?} core {core} opacity {opacity} size {size} must be rejected"
        );
    }
    let wrong_schema =
        catalog(navy, 0.35, 0.5, 1.3).replace("schema_version: 1", "schema_version: 2");
    assert!(ContactShadowTuning::from_ron(&wrong_schema).is_err());
}

#[test]
fn soft_disc_faces_up_and_fades_from_an_opaque_core_to_a_clear_edge() {
    let core = tuning().core;
    let mesh = soft_disc_mesh(core);
    let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("disc positions");
    };
    let Some(bevy::mesh::VertexAttributeValues::Float32x4(colors)) =
        mesh.attribute(Mesh::ATTRIBUTE_COLOR)
    else {
        panic!("disc vertex colours");
    };
    let radius = |index: usize| Vec2::new(positions[index][0], positions[index][2]).length();
    let mut by_radius: Vec<(f32, f32)> = (0..positions.len())
        .map(|index| (radius(index), colors[index][3]))
        .collect();
    by_radius.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert!(positions.iter().all(|position| position[1] == 0.0));
    assert!(by_radius.iter().all(|(r, _)| *r <= 1.0 + 1.0e-5));
    assert_eq!(by_radius.first().map(|entry| entry.1), Some(1.0));
    let edge = by_radius.last().expect("disc edge");
    assert!((edge.0 - 1.0).abs() < 1.0e-5 && edge.1 == 0.0);
    assert!(
        by_radius
            .windows(2)
            .all(|pair| pair[1].1 <= pair[0].1 + 1.0e-6),
        "opacity never increases outward"
    );
    let Some(Indices::U16(indices)) = mesh.indices() else {
        panic!("u16 indices");
    };
    for triangle in indices.chunks_exact(3) {
        let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
            .map(|index| Vec3::from_array(positions[usize::from(index)]));
        assert!(
            (b - a).cross(c - a).y > 0.0,
            "every triangle is counter-clockwise seen from above"
        );
    }
}

#[test]
fn map_props_are_grounded_by_baked_darkening_or_a_contact_shadow_on_every_built_in_map() {
    let catalog = brawler::map::MapContentCatalog::embedded().expect("embedded map catalog");
    let visuals =
        environment_assets::MapVisualCatalog::embedded(&catalog).expect("embedded visuals");
    let mut walls = 0;
    let mut shadowed = 0;
    for preset in &catalog.presets {
        let map = catalog
            .resolve_preset(preset.id, brawler::map::MapInstanceId(1))
            .expect("built-in map resolves")
            .snapshot;
        for placement in &map.placements {
            let asset = catalog.asset(placement.asset_id).expect("placement asset");
            let profile = catalog
                .profile(asset.gameplay_profile_id)
                .expect("gameplay profile");
            let visual = visuals.profile(asset.visual_profile_id);
            let baked = floor_tiles::grounded_by_floor_darkening(asset, profile);
            let blob = map_prop_casts_contact_shadow(asset, profile, visual);
            assert!(!(baked && blob), "{} is grounded twice", asset.key);
            let standing_prop = matches!(
                asset.slot,
                brawler::map::MapAssetSlot::Feature | brawler::map::MapAssetSlot::Decoration
            ) && profile.effect_tile_definition_id.is_none()
                && !visual.is_some_and(|visual| visual.flat);
            assert_eq!(
                standing_prop,
                baked || blob,
                "{} grounding mismatch",
                asset.key
            );
            walls += usize::from(baked);
            shadowed += usize::from(blob);
        }
    }
    assert!(walls > 0 && shadowed > 0, "built-ins exercise both paths");
}

#[test]
fn contact_shadow_rules_follow_gameplay_and_visual_data() {
    let catalog = brawler::map::MapContentCatalog::embedded().expect("embedded map catalog");
    let visuals =
        environment_assets::MapVisualCatalog::embedded(&catalog).expect("embedded visuals");
    let casts = |asset_id| {
        let asset = catalog.asset(asset_id).expect("asset");
        let profile = catalog.profile(asset.gameplay_profile_id).expect("profile");
        map_prop_casts_contact_shadow(asset, profile, visuals.profile(asset.visual_profile_id))
    };
    assert!(!casts(brawler::map::WATER_ASSET), "surfaces are not props");
    assert!(casts(brawler::map::TALL_GRASS_ASSET), "concealing foliage");
    assert!(casts(brawler::map::OIL_BARREL_ASSET), "damageable objects");
    assert!(
        casts(brawler::map::DESTRUCTIBLE_COVER_ASSET),
        "destructible cover"
    );
    assert!(!casts(brawler::map::RUBBLE_ASSET), "rubble is flat");
    assert!(
        !casts(brawler::map::BARREL_WOOD_DEBRIS_ASSET),
        "debris is flat"
    );
}

fn shadow_app(tuning: &ContactShadowTuning) -> App {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>();
    let assets = {
        let world = app.world_mut();
        world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
            let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
            ContactShadowAssets::new(tuning, &mut meshes, &mut materials)
        })
    };
    app.insert_resource(assets)
        .add_systems(Update, attach_contact_shadows);
    app
}

fn blobs(app: &mut App) -> Vec<(Entity, Transform)> {
    let world = app.world_mut();
    let mut query = world.query_filtered::<(&ChildOf, &Transform), With<ContactShadowBlob>>();
    query
        .iter(world)
        .map(|(parent, transform)| (parent.parent(), *transform))
        .collect()
}

#[test]
fn a_blob_follows_its_root_and_disappears_with_it() {
    let tuning = tuning();
    let mut app = shadow_app(&tuning);
    let fighter = app
        .world_mut()
        .spawn((Transform::default(), GroundContactShadow::fighter(14.0)))
        .id();
    let projectile = app
        .world_mut()
        .spawn((
            Transform::default(),
            GroundContactShadow::projectile(6.0, STRAIGHT_PROJECTILE_HEIGHT),
        ))
        .id();
    let cover = app
        .world_mut()
        .spawn((
            Transform::default(),
            GroundContactShadow::map_footprint(brawler::map::MapFootprint {
                width: 2,
                height: 1,
            }),
        ))
        .id();
    app.update();
    app.update();
    let found = blobs(&mut app);
    assert_eq!(found.len(), 3, "one blob per root, attached once");
    let blob_of = |root| {
        found
            .iter()
            .find(|(parent, _)| *parent == root)
            .map(|(_, transform)| *transform)
            .expect("root has a blob")
    };
    let fighter_blob = blob_of(fighter);
    assert!((fighter_blob.scale.x - 14.0 * tuning.fighter.size).abs() < 1.0e-4);
    assert!((fighter_blob.translation.y - CONTACT_SHADOW_LIFT).abs() < 1.0e-6);
    let projectile_blob = blob_of(projectile);
    assert!(
        (projectile_blob.translation.y - (CONTACT_SHADOW_LIFT - STRAIGHT_PROJECTILE_HEIGHT)).abs()
            < 1.0e-6,
        "an airborne projectile's blob stays on the floor"
    );
    let cover_blob = blob_of(cover);
    assert!(
        (cover_blob.scale.x - brawler::map::MAP_CELL_SIZE_WORLD * tuning.prop.size).abs() < 1.0e-3
    );
    assert!(
        (cover_blob.scale.z - brawler::map::MAP_CELL_SIZE_WORLD * 0.5 * tuning.prop.size).abs()
            < 1.0e-3
    );

    // Destroyed cover (or a despawned fighter) takes its shadow with it.
    app.world_mut().entity_mut(cover).despawn();
    app.update();
    assert_eq!(blobs(&mut app).len(), 2);
}

#[test]
fn a_class_with_zero_opacity_spawns_no_blob() {
    let mut tuning = tuning();
    tuning.projectile.opacity = 0.0;
    let mut app = shadow_app(&tuning);
    app.world_mut().spawn((
        Transform::default(),
        GroundContactShadow::projectile(6.0, STRAIGHT_PROJECTILE_HEIGHT),
    ));
    app.update();
    assert!(blobs(&mut app).is_empty());
}

#[test]
fn fighter_shadow_exists_only_while_the_observer_may_see_the_fighter() {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<combat::FireAreaMaterial>>()
        .init_resource::<Assets<combat::ElementalSupportAreaMaterial>>()
        .init_resource::<Assets<combat::ConeSprayMaterial>>()
        .init_resource::<Assets<combat::StraightProjectileTrailMaterial>>()
        .init_resource::<brawler::builds::BuildCatalogResource>()
        .init_resource::<Assets<ToonMaterial>>()
        .insert_resource(ToonShading::embedded().unwrap())
        .add_systems(
            Startup,
            (setup_3d_foundation, prepare_contact_shadow_assets),
        )
        .add_systems(
            Update,
            (
                combat::reconcile_fighter_visuals,
                attach_contact_shadows,
                update_fighter_contact_shadows,
            )
                .chain(),
        );
    let spawn = |app: &mut App, team: u8, controlled: bool, x: f32| {
        let mut fighter = app.world_mut().spawn((
            Fighter,
            Position(Vec2::new(x, 0.0)),
            brawler::combat::TeamId(team),
            AuthoritativeTick(40),
        ));
        if controlled {
            fighter.insert(Controlled);
        }
        fighter.id()
    };
    let local = spawn(&mut app, 0, true, 0.0);
    let ally = spawn(&mut app, 0, false, 100.0);
    let enemy = spawn(&mut app, 1, false, 200.0);
    app.world_mut()
        .entity_mut(ally)
        .insert(brawler::concealment::ConcealmentPresentationState {
            inside_concealing_terrain: true,
            ..default()
        });
    app.update();
    app.update();

    let shadow_of = |app: &mut App, fighter: Entity| {
        let world = app.world_mut();
        let mut roots = world.query::<(Entity, &CombatVisualOwner)>();
        let root = roots
            .iter(world)
            .find(|(_, owner)| owner.0 == fighter)
            .map(|(root, _)| root)?;
        let mut blobs = world.query_filtered::<(&ChildOf, &Visibility), With<ContactShadowBlob>>();
        blobs
            .iter(world)
            .find(|(parent, _)| parent.parent() == root)
            .map(|(_, visibility)| *visibility)
    };
    assert_eq!(shadow_of(&mut app, local), Some(Visibility::Inherited));
    assert_eq!(shadow_of(&mut app, enemy), Some(Visibility::Inherited));
    assert_eq!(
        shadow_of(&mut app, ally),
        Some(Visibility::Hidden),
        "a locally concealed ally's ghost casts no contact shadow"
    );

    // The server stops replicating an enemy that enters a bush away from the observer; the
    // fighter, its visual, and its shadow leave the client world together.
    app.world_mut().entity_mut(enemy).despawn();
    app.update();
    assert_eq!(shadow_of(&mut app, enemy), None);
    let world = app.world_mut();
    let remaining = world
        .query_filtered::<(), With<ContactShadowBlob>>()
        .iter(world)
        .count();
    assert_eq!(remaining, 2);

    app.world_mut()
        .entity_mut(ally)
        .remove::<brawler::concealment::ConcealmentPresentationState>();
    app.update();
    assert_eq!(shadow_of(&mut app, ally), Some(Visibility::Inherited));
}
