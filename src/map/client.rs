//! Client-only reconstruction of a replicated resolved map snapshot.
#![allow(clippy::wildcard_imports)]

use super::*;
use bevy::math::primitives::{Annulus, Circle};
use bevy::mesh::Mesh2d;
use bevy::prelude::*;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use std::collections::HashSet;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapPresentationSet {
    Reconcile,
    Readiness,
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct PresentedMap {
    pub source_root: Entity,
    pub instance_id: MapInstanceId,
    pub recipe_fingerprint: MapRecipeFingerprint,
    pub playable_bounds: AxisAlignedMapRect,
    pub camera_bounds: AxisAlignedMapRect,
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub enum ClientMapReadiness {
    #[default]
    WaitingForSnapshot,
    Ready,
    Invalid(String),
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapPresentationMember {
    pub instance_id: MapInstanceId,
}

/// Client-only objective zone fill derived from the replicated resolved area anchor. The
/// authoritative anchor owns geometry and identity; presentation only tints this visual.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZoneObjectiveFill {
    pub anchor_id: ModeAnchorId,
}

/// Client-only objective zone boundary ring companion to [`ZoneObjectiveFill`].
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZoneObjectiveBoundary {
    pub anchor_id: ModeAnchorId,
}

pub struct MapPresentationPlugin;

impl Plugin for MapPresentationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientMapReadiness>()
            .configure_sets(
                Update,
                (
                    MapPresentationSet::Reconcile,
                    MapPresentationSet::Readiness.after(MapPresentationSet::Reconcile),
                ),
            )
            .add_systems(
                Update,
                reconcile_map_snapshot.in_set(MapPresentationSet::Reconcile),
            )
            .add_systems(
                Update,
                tint_zone_objective.after(MapPresentationSet::Reconcile),
            );
    }
}

#[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
fn reconcile_map_snapshot(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    catalog: Res<MapCatalogResource>,
    presented: Option<Res<PresentedMap>>,
    snapshots: Query<(Entity, &ResolvedMapSnapshot), With<MapRoot>>,
    members: Query<(Entity, &MapPresentationMember)>,
) {
    let Some((root, snapshot)) = snapshots
        .iter()
        .max_by_key(|(_, snapshot)| snapshot.identity.instance_id)
    else {
        if let Some(presented) = presented {
            despawn_generation(&mut commands, &members, presented.instance_id);
            commands.remove_resource::<PresentedMap>();
            commands.insert_resource(ClientMapReadiness::WaitingForSnapshot);
            commands.insert_resource(crate::client::ClientPlayableGate(false));
        }
        return;
    };
    if presented
        .as_ref()
        .is_some_and(|presented| presented.instance_id == snapshot.identity.instance_id)
    {
        return;
    }
    if let Some(presented) = presented {
        despawn_generation(&mut commands, &members, presented.instance_id);
    }
    if let Err(error) = validate_client_snapshot(snapshot, &catalog.0) {
        commands.remove_resource::<PresentedMap>();
        commands.insert_resource(ClientMapReadiness::Invalid(error));
        commands.insert_resource(crate::client::ClientPlayableGate(false));
        return;
    }
    spawn_snapshot_visuals(&mut commands, &mut meshes, &mut color_materials, snapshot);
    info!(
        instance_id = snapshot.identity.instance_id.0,
        snapshot_bytes = postcard::to_allocvec(snapshot).map_or(0, |bytes| bytes.len()),
        geometry = snapshot.geometry.len(),
        visuals = snapshot.visual_instances.len(),
        entities = snapshot.entities.len(),
        regions = snapshot.regions.len(),
        presentation_entities = snapshot.visual_instances.len()
            + snapshot.geometry.len()
            + snapshot.entities.len()
            + snapshot.regions.len()
            + snapshot.spawn_areas.len()
            + snapshot.spawn_points.len()
            + 4,
        "client reconstructed authoritative map snapshot"
    );
    commands.insert_resource(PresentedMap {
        source_root: root,
        instance_id: snapshot.identity.instance_id,
        recipe_fingerprint: snapshot.identity.recipe_fingerprint,
        playable_bounds: snapshot.playable_bounds,
        camera_bounds: snapshot.camera_bounds,
    });
    commands.insert_resource(ClientMapReadiness::Ready);
}

fn validate_client_snapshot(
    snapshot: &ResolvedMapSnapshot,
    catalog: &MapContentCatalog,
) -> Result<(), String> {
    let bytes = postcard::to_allocvec(snapshot)
        .map_err(|error| format!("map snapshot serialization failed: {error}"))?;
    let requirements = MapLayoutRequirements::for_mode_definition(snapshot.mode_definition_id)
        .ok_or_else(|| "replicated map snapshot carries an unknown mode".to_string())?;
    if snapshot.identity.instance_id.0 == 0
        || snapshot.catalog_schema_version != definitions::MAP_CATALOG_SCHEMA_VERSION
        || snapshot.recipe_schema_version != definitions::MAP_RECIPE_SCHEMA_VERSION
        || snapshot.layout_schema_version != requirements.schema_version
        || bytes.len() > EngineMapLimits::default().max_snapshot_bytes
        || snapshot.geometry.len() > catalog.policy.max_geometry
        || snapshot.visual_instances.len() > catalog.policy.max_visual_instances
        || snapshot.entities.len() > catalog.policy.max_entities
        || snapshot.regions.len() > catalog.policy.max_regions
    {
        return Err("replicated map snapshot violates schema or size bounds".to_string());
    }
    let known: HashSet<_> = catalog
        .presentation_profiles
        .iter()
        .map(|definition| definition.id)
        .collect();
    let profiles = snapshot
        .geometry
        .iter()
        .filter_map(|geometry| geometry.presentation_profile_id)
        .chain(
            snapshot
                .visual_instances
                .iter()
                .map(|visual| visual.presentation_profile_id),
        )
        .chain(
            snapshot
                .entities
                .iter()
                .map(|entity| entity.presentation_profile_id),
        )
        .chain(
            snapshot
                .regions
                .iter()
                .map(|region| region.presentation_profile_id),
        );
    if profiles
        .into_iter()
        .any(|profile| !known.contains(&profile))
    {
        return Err(
            "replicated map snapshot references an unknown presentation profile".to_string(),
        );
    }
    Ok(())
}

/// Tint the world-space objective visual from durable replicated Hot Zone state. The
/// presentation reads the same generation-tagged identities the HUD gates on; a mismatched
/// or missing generation keeps a neutral tint instead of guessing ownership.
type ZoneObjectiveMeshQuery<'w, 's, M> =
    Query<'w, 's, &'static MeshMaterial2d<ColorMaterial>, (With<M>, With<Mesh2d>)>;

#[allow(clippy::type_complexity)]
fn tint_zone_objective(
    roots: Query<
        (
            &crate::matchplay::MatchState,
            &crate::matchplay::HotZoneState,
            &crate::matchplay::MatchClock,
        ),
        With<crate::matchplay::MatchRoot>,
    >,
    mut color_materials: ResMut<Assets<ColorMaterial>>,
    mesh_fills: ZoneObjectiveMeshQuery<ZoneObjectiveFill>,
    mesh_boundaries: ZoneObjectiveMeshQuery<ZoneObjectiveBoundary>,
    mut quad_fills: Query<
        &mut Sprite,
        (
            With<ZoneObjectiveFill>,
            Without<Mesh2d>,
            Without<ZoneObjectiveBoundary>,
        ),
    >,
    mut quad_boundaries: Query<
        &mut Sprite,
        (
            With<ZoneObjectiveBoundary>,
            Without<Mesh2d>,
            Without<ZoneObjectiveFill>,
        ),
    >,
) {
    let Ok((state, hot_zone, clock)) = roots.single() else {
        return;
    };
    if state.match_id != hot_zone.match_id || clock.match_id != state.match_id {
        return;
    }
    let (fill_color, boundary_color) = match hot_zone.status {
        crate::matchplay::HotZoneStatus::Empty => (
            Color::srgba(0.2, 0.5, 0.95, 0.3),
            Color::srgba(1.0, 0.82, 0.2, 0.9),
        ),
        crate::matchplay::HotZoneStatus::Contested => (
            Color::srgba(0.95, 0.2, 0.45, 0.32),
            Color::srgba(1.0, 0.35, 0.6, 0.95),
        ),
        crate::matchplay::HotZoneStatus::Controlled { team } => (
            with_alpha(team_color(team.0), 0.32),
            with_alpha(team_color(team.0), 0.95),
        ),
    };
    for material in &mesh_fills {
        if let Some(mut material) = color_materials.get_mut(material.id()) {
            material.color = fill_color;
        }
    }
    for material in &mesh_boundaries {
        if let Some(mut material) = color_materials.get_mut(material.id()) {
            material.color = boundary_color;
        }
    }
    for mut sprite in &mut quad_fills {
        sprite.color = fill_color;
    }
    for mut sprite in &mut quad_boundaries {
        sprite.color = boundary_color;
    }
}

fn with_alpha(color: Color, alpha: f32) -> Color {
    let mut result = color;
    result.set_alpha(alpha);
    result
}

fn despawn_generation(
    commands: &mut Commands,
    members: &Query<(Entity, &MapPresentationMember)>,
    instance_id: MapInstanceId,
) {
    for (entity, member) in members {
        if member.instance_id == instance_id {
            commands.entity(entity).despawn();
        }
    }
}

#[allow(clippy::too_many_lines)]
/// Objective boundary ring width in world units. Thickness/color are recorded
/// presentation tuning, not gameplay semantics.
pub const ZONE_RING_WIDTH: f32 = 28.0;

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn spawn_snapshot_visuals(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    snapshot: &ResolvedMapSnapshot,
) {
    let marker = MapPresentationMember {
        instance_id: snapshot.identity.instance_id,
    };
    for anchor in &snapshot.mode_anchors {
        if let ModeAnchorShape::Area { position, shape } = anchor.shape
            && let Some(_profile) = objective_presentation_profile(anchor.definition_id)
        {
            // The objective shape must match authoritative containment exactly: circular
            // anchors render a `Circle` fill with an `Annulus` boundary ring. Rectangle
            // anchors keep axis-aligned quads. Both stay behind fighters and projectiles
            // and above the destructible-reservation planning overlay.
            match shape {
                MapShape::Circle { radius } => {
                    let ring = meshes.add(Annulus::new(radius, radius + ZONE_RING_WIDTH));
                    let fill = meshes.add(Circle::new(radius));
                    commands.spawn((
                        marker,
                        ZoneObjectiveBoundary {
                            anchor_id: anchor.anchor_id,
                        },
                        Mesh2d(ring),
                        MeshMaterial2d(
                            materials.add(ColorMaterial::from(Color::srgba(1.0, 0.82, 0.2, 0.9))),
                        ),
                        Transform::from_translation(position.extend(-4.9)),
                    ));
                    commands.spawn((
                        marker,
                        ZoneObjectiveFill {
                            anchor_id: anchor.anchor_id,
                        },
                        Mesh2d(fill),
                        MeshMaterial2d(
                            materials.add(ColorMaterial::from(Color::srgba(0.2, 0.5, 0.95, 0.3))),
                        ),
                        Transform::from_translation(position.extend(-4.8)),
                    ));
                }
                MapShape::Rectangle { half_extents } => {
                    let size = half_extents * 2.0;
                    commands.spawn((
                        marker,
                        ZoneObjectiveBoundary {
                            anchor_id: anchor.anchor_id,
                        },
                        Sprite::from_color(
                            Color::srgba(1.0, 0.82, 0.2, 0.9),
                            size + Vec2::splat(ZONE_RING_WIDTH * 2.0),
                        ),
                        Transform::from_translation(position.extend(-4.9)),
                    ));
                    commands.spawn((
                        marker,
                        ZoneObjectiveFill {
                            anchor_id: anchor.anchor_id,
                        },
                        Sprite::from_color(Color::srgba(0.2, 0.5, 0.95, 0.3), size),
                        Transform::from_translation(position.extend(-4.8)),
                    ));
                }
            }
        }
    }
    for visual in &snapshot.visual_instances {
        let (color, size, z) = match visual.presentation_profile_id.0 {
            1 => (Color::srgb(0.055, 0.075, 0.10), Vec2::splat(64.0), -10.0),
            5 => (Color::srgb(0.18, 0.24, 0.30), Vec2::splat(32.0), 0.0),
            _ => (Color::srgb(0.12, 0.18, 0.24), Vec2::splat(32.0), 0.0),
        };
        commands.spawn((
            marker,
            Sprite::from_color(color, size),
            Transform {
                translation: visual.position.extend(z),
                rotation: Quat::from_rotation_z(visual.rotation),
                ..default()
            },
        ));
    }
    for geometry in &snapshot.geometry {
        let size = match geometry.shape {
            MapShape::Rectangle { half_extents } => half_extents * 2.0,
            MapShape::Circle { radius } => Vec2::splat(radius * 2.0),
        };
        commands.spawn((
            marker,
            Sprite::from_color(Color::srgb(0.10, 0.36, 0.58), size),
            Transform {
                translation: geometry.position.extend(2.0),
                rotation: Quat::from_rotation_z(geometry.rotation),
                ..default()
            },
        ));
    }
    for (position, size) in perimeter_visual_shapes(snapshot.playable_bounds) {
        commands.spawn((
            marker,
            Sprite::from_color(Color::srgb(0.38, 0.84, 1.0), size),
            Transform::from_translation(position.extend(2.0)),
        ));
    }
    // The authored destructible regions no longer render a planning overlay: live
    // occupancy sprites from the terrain presentation own that space above the floor.
    for area in &snapshot.spawn_areas {
        let color = team_color(area.team_slot).with_alpha(0.10);
        commands.spawn((
            marker,
            Sprite::from_color(color, area.bounds.size()),
            Transform::from_translation(area.bounds.center().extend(-4.0)),
        ));
    }
    for point in &snapshot.spawn_points {
        commands.spawn((
            marker,
            Sprite::from_color(team_color(point.team_slot), Vec2::splat(14.0)),
            Transform::from_translation(point.position.extend(-3.0)),
        ));
    }
    for entity in &snapshot.entities {
        commands.spawn((
            marker,
            Sprite::from_color(Color::srgb(0.42, 0.52, 0.62), Vec2::splat(28.0)),
            Transform {
                translation: entity.position.extend(0.0),
                rotation: Quat::from_rotation_z(entity.rotation),
                ..default()
            },
        ));
    }
}

fn team_color(team: u8) -> Color {
    match team {
        0 => Color::srgb(0.12, 0.72, 0.96),
        1 => Color::srgb(1.0, 0.42, 0.12),
        _ => Color::srgb(0.72, 0.72, 0.72),
    }
}

#[must_use]
pub fn perimeter_visual_shapes(bounds: AxisAlignedMapRect) -> [(Vec2, Vec2); 4] {
    const THICKNESS: f32 = 24.0;
    let size = bounds.size();
    let center = bounds.center();
    [
        (
            Vec2::new(bounds.min.x + THICKNESS * 0.5, center.y),
            Vec2::new(THICKNESS, size.y),
        ),
        (
            Vec2::new(bounds.max.x - THICKNESS * 0.5, center.y),
            Vec2::new(THICKNESS, size.y),
        ),
        (
            Vec2::new(center.x, bounds.min.y + THICKNESS * 0.5),
            Vec2::new(size.x, THICKNESS),
        ),
        (
            Vec2::new(center.x, bounds.max.y - THICKNESS * 0.5),
            Vec2::new(size.x, THICKNESS),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(instance: u64) -> ResolvedMapSnapshot {
        MapContentCatalog::embedded()
            .unwrap()
            .resolve_preset(
                MapPresetId(1),
                MapInstanceId(instance),
                &MapLayoutRequirements::wipeout(),
            )
            .unwrap()
            .snapshot
    }

    fn app_with_snapshot(snapshot: ResolvedMapSnapshot) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, MapContentPlugin, MapPresentationPlugin));
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<ColorMaterial>>();
        app.world_mut().spawn((
            MapRoot,
            snapshot.identity.instance_id,
            snapshot.identity,
            snapshot,
        ));
        app
    }

    #[test]
    fn reconciliation_is_idempotent_and_replaces_exact_generation() {
        let mut app = app_with_snapshot(snapshot(1));
        app.update();
        let initial = app
            .world_mut()
            .query::<&MapPresentationMember>()
            .iter(app.world())
            .count();
        // One fewer sprite than the M09 count: the destructible-reservation planning
        // overlay is gone and live terrain occupancy presents its own chunk sprites.
        assert_eq!(initial, 524);
        assert_eq!(
            *app.world().resource::<ClientMapReadiness>(),
            ClientMapReadiness::Ready
        );
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<&MapPresentationMember>()
                .iter(app.world())
                .count(),
            initial
        );

        let old_root = app.world().resource::<PresentedMap>().source_root;
        app.world_mut().entity_mut(old_root).despawn();
        let replacement = snapshot(2);
        app.world_mut().spawn((
            MapRoot,
            replacement.identity.instance_id,
            replacement.identity,
            replacement,
        ));
        app.update();
        assert_eq!(
            app.world().resource::<PresentedMap>().instance_id,
            MapInstanceId(2)
        );
        assert!(
            app.world_mut()
                .query::<&MapPresentationMember>()
                .iter(app.world())
                .all(|member| member.instance_id == MapInstanceId(2))
        );
    }

    #[test]
    fn floor_visuals_use_the_subdued_primitive_palette() {
        let snapshot = snapshot(1);
        let expected_floor_tiles = snapshot.visual_instances.len();
        let mut app = app_with_snapshot(snapshot);
        app.update();

        let world = app.world_mut();
        let mut visuals = world.query::<(&Sprite, &Transform)>();
        let floor_tiles: Vec<_> = visuals
            .iter(world)
            .filter(|(_, transform)| (transform.translation.z + 10.0).abs() < f32::EPSILON)
            .collect();

        assert_eq!(floor_tiles.len(), expected_floor_tiles);
        assert!(floor_tiles.iter().all(|(sprite, _)| {
            sprite.image == Handle::<Image>::default()
                && sprite.rect.is_none()
                && sprite.color == Color::srgb(0.055, 0.075, 0.10)
        }));
    }

    #[test]
    fn unknown_required_profile_fails_visibly_and_closes_gate() {
        let mut invalid = snapshot(1);
        invalid.visual_instances[0].presentation_profile_id = MapPresentationProfileId(999);
        let mut app = app_with_snapshot(invalid);
        app.update();
        assert!(matches!(
            app.world().resource::<ClientMapReadiness>(),
            ClientMapReadiness::Invalid(error) if error.contains("unknown presentation")
        ));
        assert!(
            !app.world()
                .resource::<crate::client::ClientPlayableGate>()
                .0
        );
        assert!(!app.world().contains_resource::<PresentedMap>());
    }

    fn hot_zone_snapshot_test() -> ResolvedMapSnapshot {
        MapContentCatalog::embedded()
            .unwrap()
            .resolve_preset(
                MapPresetId(2),
                MapInstanceId(11),
                &MapLayoutRequirements::hot_zone(),
            )
            .unwrap()
            .snapshot
    }

    #[test]
    fn hot_zone_objective_visuals_spawn_with_the_exact_generation_marker() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, MapContentPlugin, MapPresentationPlugin));
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<ColorMaterial>>();
        let snapshot = hot_zone_snapshot_test();
        let instance_id = snapshot.identity.instance_id;
        let anchor_id = snapshot.mode_anchors[0].anchor_id;
        let ModeAnchorShape::Area {
            shape: MapShape::Circle { radius },
            ..
        } = snapshot.mode_anchors[0].shape
        else {
            panic!("built-in Hot Zone anchor is circular")
        };
        app.world_mut()
            .spawn((MapRoot, instance_id, snapshot.identity, snapshot));
        app.update();
        let world = app.world_mut();
        let mut fills = world.query::<&ZoneObjectiveFill>();
        let mut boundaries = world.query::<&ZoneObjectiveBoundary>();
        assert_eq!(fills.iter(world).count(), 1);
        assert_eq!(boundaries.iter(world).count(), 1);
        let fill = fills.single(world).unwrap();
        let boundary = boundaries.single(world).unwrap();
        assert_eq!(fill.anchor_id, anchor_id);
        assert_eq!(boundary.anchor_id, anchor_id);
        let members = world
            .query::<&MapPresentationMember>()
            .iter(world)
            .filter(|member| member.instance_id == instance_id)
            .count();
        assert!(
            members > 0,
            "zone visuals share the exact map generation marker"
        );

        // Shape fidelity: the presented objective must be a circle matching authoritative
        // containment, with an annulus boundary of the recorded ring width — never a quad.
        let mut fill_meshes = world.query_filtered::<&Mesh2d, With<ZoneObjectiveFill>>();
        let mut boundary_meshes = world.query_filtered::<&Mesh2d, With<ZoneObjectiveBoundary>>();
        let fill_handle = fill_meshes.single(world).unwrap().0.clone();
        let boundary_handle = boundary_meshes.single(world).unwrap().0.clone();
        let mesh_radii = |handle: Handle<Mesh>| -> (f32, f32, usize) {
            let meshes = world.resource::<Assets<Mesh>>();
            let mesh = meshes.get(&handle).expect("objective mesh exists");
            let positions = mesh
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .expect("objective mesh has positions");
            let bevy::mesh::VertexAttributeValues::Float32x3(values) = positions else {
                panic!("objective mesh positions are Float32x3");
            };
            let radii: Vec<f32> = values.iter().map(|[x, y, _]| f32::hypot(*x, *y)).collect();
            (
                radii.iter().copied().fold(f32::INFINITY, f32::min),
                radii.iter().copied().fold(f32::NEG_INFINITY, f32::max),
                radii.len(),
            )
        };
        let (fill_inner, fill_outer, fill_vertices) = mesh_radii(fill_handle);
        let (ring_inner, ring_outer, ring_vertices) = mesh_radii(boundary_handle);
        // A disc mesh of rim vertices at the containment radius; a quad of the same size
        // would reach radius * sqrt(2) at its corners and use only four vertices.
        assert!(
            (fill_inner - radius).abs() < 1.0
                && (fill_outer - radius).abs() < 1.0
                && fill_vertices > 8,
            "fill is a circle of radius {radius}, got inner {fill_inner} outer {fill_outer} with {fill_vertices} vertices"
        );
        assert!(
            (ring_inner - radius).abs() < 1.0
                && (ring_outer - (radius + ZONE_RING_WIDTH)).abs() < 1.0
                && ring_vertices > 16,
            "boundary is an annulus {radius}..{} got {ring_inner}..{ring_outer} with {ring_vertices} vertices",
            radius + ZONE_RING_WIDTH,
        );
        let mut sprites = world.query_filtered::<&Sprite, With<ZoneObjectiveFill>>();
        assert_eq!(
            sprites.iter(world).count(),
            0,
            "a circular objective never renders a rectangular sprite"
        );
    }

    #[test]
    fn perimeter_visuals_are_inside_resolved_bounds() {
        let bounds = snapshot(1).playable_bounds;
        for (position, size) in perimeter_visual_shapes(bounds) {
            assert!(bounds.contains(position - size * 0.5));
            assert!(bounds.contains(position + size * 0.5));
        }
    }
}
