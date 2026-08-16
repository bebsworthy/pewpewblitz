//! Client-only reconstruction of a replicated resolved map snapshot.
#![allow(clippy::wildcard_imports)]

use super::*;
use bevy::prelude::*;
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
            );
    }
}

#[allow(clippy::needless_pass_by_value)]
fn reconcile_map_snapshot(
    mut commands: Commands,
    catalog: Res<MapCatalogResource>,
    asset_server: Option<Res<AssetServer>>,
    assets: Option<Res<crate::client::ClientAssetHandles>>,
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
    let facility_image = assets.as_ref().and_then(|assets| {
        asset_server
            .as_ref()
            .is_some_and(|server| server.is_loaded(&assets.facility_tileset))
            .then_some(&assets.facility_tileset)
    });
    spawn_snapshot_visuals(&mut commands, snapshot, facility_image);
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

fn spawn_snapshot_visuals(
    commands: &mut Commands,
    snapshot: &ResolvedMapSnapshot,
    facility_image: Option<&Handle<Image>>,
) {
    let marker = MapPresentationMember {
        instance_id: snapshot.identity.instance_id,
    };
    for visual in &snapshot.visual_instances {
        let (color, size, z) = match visual.presentation_profile_id.0 {
            1 => (Color::srgb(0.055, 0.075, 0.10), Vec2::splat(64.0), -10.0),
            5 => (Color::srgb(0.18, 0.24, 0.30), Vec2::splat(32.0), 0.0),
            _ => (Color::srgb(0.12, 0.18, 0.24), Vec2::splat(32.0), 0.0),
        };
        let sprite = if visual.presentation_profile_id.0 == 1 {
            facility_image.map_or_else(
                || Sprite::from_color(color, size),
                |image| Sprite {
                    image: image.clone(),
                    rect: Some(Rect::new(16.0, 32.0, 32.0, 48.0)),
                    custom_size: Some(size),
                    ..default()
                },
            )
        } else {
            Sprite::from_color(color, size)
        };
        commands.spawn((
            marker,
            sprite,
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
    for region in &snapshot.regions {
        let size = match region.shape {
            MapShape::Rectangle { half_extents } => half_extents * 2.0,
            MapShape::Circle { radius } => Vec2::splat(radius * 2.0),
        };
        commands.spawn((
            marker,
            Sprite::from_color(Color::srgba(0.95, 0.62, 0.12, 0.24), size),
            Transform {
                translation: region.position.extend(-5.0),
                rotation: Quat::from_rotation_z(region.rotation),
                ..default()
            },
        ));
    }
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
        assert_eq!(initial, 525);
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

    #[test]
    fn perimeter_visuals_are_inside_resolved_bounds() {
        let bounds = snapshot(1).playable_bounds;
        for (position, size) in perimeter_visual_shapes(bounds) {
            assert!(bounds.contains(position - size * 0.5));
            assert!(bounds.contains(position + size * 0.5));
        }
    }
}
