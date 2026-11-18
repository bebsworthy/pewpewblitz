use super::super::entities::reconcile_fighter_visuals;
use super::super::tag_fighter_rim_relation;
use super::*;
use crate::presentation_3d::setup_3d_foundation;
use avian2d::prelude::Position;
use bevy::mesh::Indices;
use bevy::mesh::MeshTag;

fn tuning() -> FighterOutlineTuning {
    FighterOutlineTuning::embedded().expect("embedded fighter outline catalog")
}

fn catalog(width: f32, color: (f32, f32, f32), tint: f32) -> String {
    catalog_with_preview(width, 0.8, color, tint)
}

fn catalog_with_preview(width: f32, preview: f32, color: (f32, f32, f32), tint: f32) -> String {
    format!(
        "(schema_version: 2, width_world: {width}, preview_width_world: {preview}, \
         color: ({}, {}, {}), relationship_tint: {tint})",
        color.0, color.1, color.2
    )
}

#[test]
fn embedded_catalog_is_a_thin_dark_navy_outline() {
    let tuning = tuning();
    assert!((0.6..=2.0).contains(&tuning.width_world));
    assert!((0.3..=2.0).contains(&tuning.preview_width_world));
    let color = tuning.color;
    assert!(
        color.blue > color.red && color.blue > color.green,
        "navy, not black"
    );
    assert!(color.red.max(color.green).max(color.blue) <= MAX_OUTLINE_CHANNEL);
    assert!(color.blue > 0.0, "the outline is never pure black");
}

#[test]
fn catalog_rejects_out_of_bounds_tuning() {
    let navy = (0.055, 0.086, 0.22);
    assert!(FighterOutlineTuning::from_ron(&catalog(1.1, navy, 0.0)).is_ok());
    for (width, color, tint) in [
        (0.0, navy, 0.0),
        (MAX_OUTLINE_WIDTH_WORLD + 1.0, navy, 0.0),
        (f32::NAN, navy, 0.0),
        (2.5, (0.9, 0.9, 0.9), 0.0),
        (2.5, navy, 1.5),
    ] {
        assert!(
            FighterOutlineTuning::from_ron(&catalog(width, color, tint)).is_err(),
            "width {width} color {color:?} tint {tint} must be rejected"
        );
    }
    for preview in [0.0, MAX_OUTLINE_WIDTH_WORLD + 1.0, f32::NAN] {
        assert!(
            FighterOutlineTuning::from_ron(&catalog_with_preview(1.1, preview, navy, 0.0)).is_err(),
            "preview width {preview} must be rejected"
        );
    }
    let wrong_schema = catalog(2.5, navy, 0.0).replace("schema_version: 2", "schema_version: 1");
    assert!(FighterOutlineTuning::from_ron(&wrong_schema).is_err());
}

#[test]
fn relationship_tint_is_optional_and_follows_the_rim_relation() {
    let shading = ToonShading::embedded().unwrap();
    let plain = tuning().material(&shading).settings;
    let [untagged, local, ally, enemy] = plain.colors;
    assert_eq!(
        untagged,
        LinearRgba::from(tuning().color).to_vec4().with_w(1.0)
    );
    assert!(
        [local, ally, enemy].iter().all(|color| *color == untagged),
        "the default outline is plain navy for every relation"
    );

    let tinted = FighterOutlineTuning {
        relationship_tint: 1.0,
        ..tuning()
    }
    .material(&shading)
    .settings;
    let [untagged, local, ally, enemy] = tinted.colors;
    assert_eq!(untagged, plain.colors[0], "untagged hulls stay navy");
    assert!(
        local.y > local.x && local.y > local.z,
        "local tint is green"
    );
    assert!(ally.z > ally.x, "ally tint is blue");
    assert!(enemy.x > enemy.y && enemy.x > enemy.z, "enemy tint is red");
    // Flat and bounded: the outline itself is never an HDR glow.
    for color in tinted.colors {
        assert!(color.truncate().max_element() <= 1.0);
    }
}

#[test]
fn hull_welds_split_flat_normals_into_one_outward_normal_per_corner() {
    let cube = Mesh::from(Cuboid::from_length(2.0));
    let hull = smoothed_hull_mesh(&cube).expect("a cuboid is a hull-able triangle list");
    let Some(VertexAttributeValues::Float32x3(positions)) =
        hull.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("hull keeps positions");
    };
    let Some(VertexAttributeValues::Float32x3(normals)) = hull.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        panic!("hull carries smoothed normals");
    };
    assert_eq!(
        positions.len(),
        24,
        "the flat cube splits each corner three ways"
    );
    assert_eq!(
        hull.indices().map(Indices::len),
        cube.indices().map(Indices::len)
    );
    for (position, normal) in positions.iter().zip(normals) {
        let expected = Vec3::from(*position).normalize();
        assert!(
            Vec3::from(*normal).abs_diff_eq(expected, 1.0e-5),
            "corner {position:?} has normal {normal:?}, expected the outward diagonal"
        );
    }
    assert!(hull.attribute(Mesh::ATTRIBUTE_UV_0).is_none());

    let mut skinned = Mesh::from(Cuboid::from_length(2.0));
    skinned.insert_attribute(
        Mesh::ATTRIBUTE_JOINT_INDEX,
        VertexAttributeValues::Uint16x4(vec![[0; 4]; 24]),
    );
    assert!(
        smoothed_hull_mesh(&skinned).is_none(),
        "joint indices without weights cannot be skinned"
    );
    skinned.insert_attribute(
        Mesh::ATTRIBUTE_JOINT_WEIGHT,
        vec![[1.0_f32, 0.0, 0.0, 0.0]; 24],
    );
    let skinned_hull = smoothed_hull_mesh(&skinned).expect("a skinned hull keeps its joints");
    assert!(
        skinned_hull
            .attribute(Mesh::ATTRIBUTE_JOINT_INDEX)
            .is_some()
    );
    assert!(
        skinned_hull
            .attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT)
            .is_some()
    );
}

fn outline_app() -> App {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<super::super::elemental_style::FireAreaMaterial>>()
        .init_resource::<Assets<super::super::elemental_style::ElementalSupportAreaMaterial>>()
        .init_resource::<Assets<super::super::cone_spray::ConeSprayMaterial>>()
        .init_resource::<
            Assets<super::super::straight_projectile_style::StraightProjectileTrailMaterial>,
        >()
        .init_resource::<brawler::builds::BuildCatalogResource>()
        .init_resource::<Assets<ToonMaterial>>()
        .insert_resource(ToonShading::embedded().unwrap())
        .init_resource::<Assets<FighterOutlineMaterial>>()
        .insert_resource(tuning())
        .init_resource::<FighterOutlineAssets>()
        .add_systems(
            Startup,
            (setup_3d_foundation, prepare_fighter_outline_material),
        )
        .add_systems(
            Update,
            (
                reconcile_fighter_visuals,
                attach_fighter_outlines,
                tag_fighter_rim_relation,
            )
                .chain(),
        );
    app
}

/// Hull entities and the fighter each one belongs to (through its visual root).
fn hulls_by_owner(app: &mut App) -> Vec<(Entity, Entity, Visibility)> {
    let world = app.world_mut();
    let mut hulls = world.query_filtered::<(Entity, &Visibility), With<FighterOutlineHull>>();
    let hulls = hulls
        .iter(world)
        .map(|(entity, visibility)| (entity, *visibility))
        .collect::<Vec<_>>();
    hulls
        .into_iter()
        .map(|(hull, visibility)| {
            let root = world
                .entity(hull)
                .get::<ChildOf>()
                .and_then(|body| world.entity(body.parent()).get::<ChildOf>())
                .map(ChildOf::parent)
                .expect("a hull is a child of a body mesh under the fighter root");
            let owner = world
                .entity(root)
                .get::<CombatVisualOwner>()
                .expect("the hull's grandparent is the fighter visual root")
                .0;
            (hull, owner, visibility)
        })
        .collect()
}

#[test]
fn outline_exists_only_while_the_observer_may_see_the_fighter() {
    let mut app = outline_app();
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
    // The ally is inside a bush: allowed for its own team, drawn as a translucent ghost.
    app.world_mut()
        .entity_mut(ally)
        .insert(brawler::concealment::ConcealmentPresentationState {
            inside_concealing_terrain: true,
            ..default()
        });
    app.update();
    app.update();

    let hulls = hulls_by_owner(&mut app);
    let hull_of = |owner: Entity| {
        hulls
            .iter()
            .find(|(_, hull_owner, _)| *hull_owner == owner)
            .copied()
            .unwrap_or_else(|| panic!("fighter {owner} has an outline hull"))
    };
    assert_eq!(hulls.len(), 3, "one hull per fallback body, none on rings");
    assert_eq!(hull_of(local).2, Visibility::Inherited);
    assert_eq!(
        hull_of(ally).2,
        Visibility::Hidden,
        "a locally concealed ally keeps no outline over its ghost"
    );
    let (enemy_hull, _, enemy_visibility) = hull_of(enemy);
    // Inherited: the hull follows the body's own visibility (hidden fallback, hidden root).
    assert_eq!(enemy_visibility, Visibility::Inherited);
    assert_eq!(
        app.world().get::<MeshTag>(enemy_hull),
        Some(&RimRelation::Enemy.tag())
    );
    assert!(app.world().get::<NotShadowCaster>(enemy_hull).is_some());

    // The enemy enters a bush away from the observer: the server stops replicating it to this
    // connection, so the fighter disappears from the client world. Its outline goes with it.
    app.world_mut().entity_mut(enemy).despawn();
    app.update();
    let hulls = hulls_by_owner(&mut app);
    assert_eq!(hulls.len(), 2);
    assert!(hulls.iter().all(|(_, owner, _)| *owner != enemy));

    // Leaving the bush restores the ally's outline.
    app.world_mut()
        .entity_mut(ally)
        .remove::<brawler::concealment::ConcealmentPresentationState>();
    app.update();
    let hulls = hulls_by_owner(&mut app);
    assert!(
        hulls
            .iter()
            .all(|(_, _, visibility)| *visibility == Visibility::Inherited)
    );
}

#[test]
fn a_preview_root_outlines_its_toon_meshes_once_on_the_preview_layer() {
    let mut app = App::new();
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<ToonMaterial>>()
        .insert_resource(ToonShading::embedded().unwrap())
        .init_resource::<Assets<FighterOutlineMaterial>>()
        .insert_resource(tuning())
        .init_resource::<FighterOutlineAssets>()
        .add_systems(Startup, prepare_fighter_outline_material)
        .add_systems(Update, attach_preview_outlines);
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::from_length(2.0));
    let toon = app.world_mut().resource_mut::<Assets<ToonMaterial>>().add(
        ToonShading::embedded()
            .unwrap()
            .material(StandardMaterial::default()),
    );
    let layer = RenderLayers::layer(29);
    let root = app.world_mut().spawn((PreviewOutline, layer.clone())).id();
    let scene = app.world_mut().spawn(ChildOf(root)).id();
    let body = app
        .world_mut()
        .spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(toon.clone()),
            layer.clone(),
            ChildOf(scene),
        ))
        .id();
    // A weapon close-up scene has toon meshes but no preview outline marker.
    let close_up = app.world_mut().spawn(RenderLayers::layer(28)).id();
    app.world_mut().spawn((
        Mesh3d(mesh),
        MeshMaterial3d(toon),
        RenderLayers::layer(28),
        ChildOf(close_up),
    ));

    for _ in 0..3 {
        app.update();
        let world = app.world_mut();
        let hulls = world
            .query_filtered::<(
                &ChildOf,
                &RenderLayers,
                &MeshMaterial3d<FighterOutlineMaterial>,
            ), With<FighterOutlineHull>>()
            .iter(world)
            .map(|(parent, layers, material)| (parent.parent(), layers.clone(), material.0.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            hulls.len(),
            1,
            "exactly one hull, never duplicated, none on the close-up"
        );
        let (parent, layers, material) = &hulls[0];
        assert_eq!(*parent, body);
        assert_eq!(
            *layers, layer,
            "the hull carries the preview layer explicitly"
        );
        assert_eq!(
            *material,
            world.resource::<FighterOutlineAssets>().preview_material,
            "previews use the preview-width material"
        );
    }
}
