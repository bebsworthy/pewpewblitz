//! Renderer-neutral client reconstruction of a replicated resolved map snapshot.
#![allow(clippy::wildcard_imports)]

use super::*;
use bevy::prelude::*;
use std::collections::HashSet;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapPresentationSet {
    Reconcile,
    Materialize3d,
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
                    MapPresentationSet::Materialize3d.after(MapPresentationSet::Reconcile),
                    MapPresentationSet::Readiness.after(MapPresentationSet::Materialize3d),
                ),
            )
            .add_systems(
                Update,
                reconcile_map_snapshot.in_set(MapPresentationSet::Reconcile),
            );
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn reconcile_map_snapshot(
    mut commands: Commands,
    catalog: Res<MapCatalogResource>,
    presented: Option<Res<PresentedMap>>,
    snapshots: Query<(Entity, &ResolvedMapSnapshot), With<MapRoot>>,
) {
    let Some((root, snapshot)) = snapshots
        .iter()
        .max_by_key(|(_, snapshot)| snapshot.identity.instance_id)
    else {
        if presented.is_some() {
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
    if let Err(error) = validate_client_snapshot(snapshot, &catalog.0) {
        commands.remove_resource::<PresentedMap>();
        commands.insert_resource(ClientMapReadiness::Invalid(error));
        commands.insert_resource(crate::client::ClientPlayableGate(false));
        return;
    }
    info!(
        instance_id = snapshot.identity.instance_id.0,
        snapshot_bytes = postcard::to_allocvec(snapshot).map_or(0, |bytes| bytes.len()),
        geometry = snapshot.geometry.len(),
        visuals = snapshot.visual_instances.len(),
        entities = snapshot.entities.len(),
        regions = snapshot.regions.len(),
        "client accepted authoritative map snapshot"
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

/// Decorative perimeter cuboids sit outside the playable edge and never alter collision.
#[must_use]
pub fn perimeter_visual_shapes(bounds: AxisAlignedMapRect) -> [(Vec2, Vec2); 4] {
    const THICKNESS: f32 = 24.0;
    let size = bounds.size();
    let center = bounds.center();
    [
        (
            Vec2::new(bounds.min.x - THICKNESS * 0.5, center.y),
            Vec2::new(THICKNESS, size.y + THICKNESS * 2.0),
        ),
        (
            Vec2::new(bounds.max.x + THICKNESS * 0.5, center.y),
            Vec2::new(THICKNESS, size.y + THICKNESS * 2.0),
        ),
        (
            Vec2::new(center.x, bounds.min.y - THICKNESS * 0.5),
            Vec2::new(size.x, THICKNESS),
        ),
        (
            Vec2::new(center.x, bounds.max.y + THICKNESS * 0.5),
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
        assert_eq!(
            app.world().resource::<PresentedMap>().instance_id,
            MapInstanceId(1)
        );
        assert_eq!(
            *app.world().resource::<ClientMapReadiness>(),
            ClientMapReadiness::Ready
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
        assert!(!app.world().contains_resource::<PresentedMap>());
    }

    #[test]
    fn perimeter_visuals_are_outside_resolved_bounds() {
        let bounds = snapshot(1).playable_bounds;
        let shapes = perimeter_visual_shapes(bounds);
        assert!(shapes[0].0.x < bounds.min.x && shapes[1].0.x > bounds.max.x);
        assert!(shapes[2].0.y < bounds.min.y && shapes[3].0.y > bounds.max.y);
    }
}
