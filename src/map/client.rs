//! Renderer-neutral client reconstruction of a replicated resolved map snapshot.
#![allow(clippy::wildcard_imports)]

use super::catalog::MAP_CATALOG_SCHEMA_VERSION;
use super::*;
use bevy::prelude::*;
use lightyear::prelude::client::Client;
use lightyear::prelude::{MessageReceiver, MessageSender};

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
    pub presentation_theme_id: MapPresentationThemeId,
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

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub enum ClientWorldObjectReadiness {
    #[default]
    Waiting,
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

/// Network-facing dynamic state convergence, installed for both headless and windowed clients.
pub struct ClientMapPlugin;

#[derive(Resource, Default)]
struct SeenWorldObjectCueIds(std::collections::VecDeque<crate::combat::CombatEventId>);

impl Plugin for ClientMapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ReceivedWorldObjectCues>()
            .init_resource::<SeenWorldObjectCueIds>()
            .init_resource::<ClientWorldObjectReadiness>()
            .add_systems(
                Update,
                (
                    converge_map_dynamic_state,
                    converge_world_object_readiness.after(converge_map_dynamic_state),
                    receive_world_object_cues,
                ),
            );
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy systems receive resource system parameters by value"
)]
fn converge_world_object_readiness(
    roots: Query<(&ResolvedMapSnapshot, &MapDynamicState), With<MapRoot>>,
    objects: Query<&DamageableTargetIdentity, With<DamageableWorldObject>>,
    catalog: Res<MapCatalogResource>,
    mut readiness: ResMut<ClientWorldObjectReadiness>,
) {
    let Ok((snapshot, state)) = roots.single() else {
        *readiness = ClientWorldObjectReadiness::Waiting;
        return;
    };
    let terminal: std::collections::BTreeSet<_> = state
        .terminal_states
        .iter()
        .map(|transition| transition.placement_id)
        .collect();
    let expected: std::collections::BTreeSet<_> = snapshot
        .placements
        .iter()
        .filter(|placement| !terminal.contains(&placement.placement_id))
        .filter_map(|placement| {
            let asset = catalog.0.asset(placement.asset_id)?;
            let profile = catalog.0.profile(asset.gameplay_profile_id)?;
            matches!(profile.durability, MapDurabilityBehavior::HitPoints(_))
                .then_some(placement.placement_id)
        })
        .collect();
    let generation = state.generation_id();
    let actual: std::collections::BTreeSet<_> = objects
        .iter()
        .filter_map(|identity| {
            (identity.generation() == generation).then_some(identity.placement_id())
        })
        .collect();
    *readiness = if actual == expected {
        ClientWorldObjectReadiness::Ready
    } else if actual.is_subset(&expected) {
        ClientWorldObjectReadiness::Waiting
    } else {
        ClientWorldObjectReadiness::Invalid(
            "replicated damageable objects do not match the map generation".to_string(),
        )
    };
}

fn receive_world_object_cues(
    mut receivers: Query<Option<&mut MessageReceiver<WorldObjectCue>>, With<Client>>,
    mut inbox: ResMut<ReceivedWorldObjectCues>,
    mut seen: ResMut<SeenWorldObjectCueIds>,
) {
    for receiver in &mut receivers {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for cue in receiver.receive() {
            if inbox.0.len() < MAX_WORLD_OBJECT_CUES && !seen.0.contains(&cue.event_id()) {
                if seen.0.len() >= MAX_WORLD_OBJECT_CUES {
                    seen.0.pop_front();
                }
                seen.0.push_back(cue.event_id());
                inbox.0.push(cue);
            }
        }
    }
    inbox.0.sort_by_key(|cue| cue.event_id().0);
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "the convergence system owns distinct replicated channels and catalog state"
)]
fn converge_map_dynamic_state(
    mut requested: Local<Option<MapDynamicGeneration>>,
    mut roots: Query<(&ResolvedMapSnapshot, &mut MapDynamicState), With<MapRoot>>,
    mut resets: Query<Option<&mut MessageReceiver<MapDynamicResetEvent>>, With<Client>>,
    mut recoveries: Query<Option<&mut MessageReceiver<MapDynamicRecoverySnapshot>>, With<Client>>,
    mut mutations: Query<Option<&mut MessageReceiver<MapMutationEvent>>, With<Client>>,
    mut requests: Query<&mut MessageSender<MapDynamicRecoveryRequest>, With<Client>>,
    catalog: Res<MapCatalogResource>,
) {
    let Ok((snapshot, mut state)) = roots.single_mut() else {
        *requested = None;
        return;
    };
    let valid_dynamic_outcomes = legal_dynamic_outcomes(snapshot, &catalog.0);
    for receiver in &mut resets {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for reset in receiver.receive() {
            if reset.next_generation.map_instance_id == state.map_instance_id
                && reset.next_generation.generation >= state.generation
            {
                state.generation = reset.next_generation.generation;
                state.revision = 0;
                state.terminal_states.clear();
                *requested = None;
            }
        }
    }
    for receiver in &mut recoveries {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for recovery in receiver.receive() {
            if recovery.state.map_instance_id == state.map_instance_id
                && recovery.state.generation >= state.generation
                && transitions_are_legal(
                    &recovery.state.terminal_states,
                    &valid_dynamic_outcomes,
                    &[],
                )
            {
                *state = recovery.state;
                *requested = None;
            }
        }
    }
    let mut recovery_generation = None;
    for receiver in &mut mutations {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for event in receiver.receive() {
            let outcomes_valid = !event.transitions.is_empty()
                && transitions_are_legal(
                    &event.transitions,
                    &valid_dynamic_outcomes,
                    &state.terminal_states,
                );
            if outcomes_valid
                && event.generation == state.generation_id()
                && event.revision == state.revision + 1
            {
                state.revision = event.revision;
                state.terminal_states.extend(event.transitions);
                state
                    .terminal_states
                    .sort_by_key(|transition| transition.placement_id);
            } else if event.generation.map_instance_id == state.map_instance_id
                && (event.generation.generation > state.generation
                    || (event.generation == state.generation_id()
                        && event.revision > state.revision + 1))
            {
                recovery_generation = Some(event.generation);
            }
        }
    }
    if let Some(generation) = recovery_generation
        && requested.is_none_or(|current| current != generation)
    {
        for mut sender in &mut requests {
            sender.send::<crate::protocol::MapDynamicChannel>(MapDynamicRecoveryRequest {
                generation,
            });
        }
        *requested = Some(generation);
    }
}

fn legal_dynamic_outcomes(
    snapshot: &ResolvedMapSnapshot,
    catalog: &MapContentCatalog,
) -> std::collections::BTreeMap<MapPlacementId, MapPlacementOutcome> {
    snapshot
        .placements
        .iter()
        .filter_map(|placement| {
            let asset = catalog.asset(placement.asset_id)?;
            let profile = catalog.profile(asset.gameplay_profile_id)?;
            let outcome = match profile.durability {
                MapDurabilityBehavior::HitPoints(id) => {
                    let damage = catalog.damage_profile(id)?;
                    match damage.terminal {
                        MapObjectTerminalBehavior::Explode { outcome, .. } => outcome,
                    }
                }
                MapDurabilityBehavior::Indestructible => match profile.destruction {
                    MapDestructionBehavior::Indestructible => return None,
                    MapDestructionBehavior::RemoveOnMapDestruction => MapPlacementOutcome::Removed,
                    MapDestructionBehavior::ReplaceOnMapDestruction(asset_id) => {
                        MapPlacementOutcome::ReplacedWith(asset_id)
                    }
                },
            };
            Some((placement.placement_id, outcome))
        })
        .collect()
}

fn transitions_are_legal(
    transitions: &[MapPlacementTransition],
    legal: &std::collections::BTreeMap<MapPlacementId, MapPlacementOutcome>,
    existing: &[MapPlacementTransition],
) -> bool {
    transitions
        .windows(2)
        .all(|pair| pair[0].placement_id < pair[1].placement_id)
        && transitions.iter().all(|transition| {
            legal.get(&transition.placement_id) == Some(&transition.outcome)
                && !existing
                    .iter()
                    .any(|current| current.placement_id == transition.placement_id)
        })
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn reconcile_map_snapshot(
    mut commands: Commands,
    grid_catalog: Res<MapCatalogResource>,
    presented: Option<Res<PresentedMap>>,
    snapshots: Query<(Entity, &ResolvedMapSnapshot, Option<&MapDynamicState>), With<MapRoot>>,
) {
    let Some((root, snapshot, dynamic_state)) = snapshots
        .iter()
        .max_by_key(|(_, snapshot, _)| snapshot.identity.instance_id)
    else {
        if presented.is_some() {
            commands.remove_resource::<PresentedMap>();
            commands.insert_resource(ClientMapReadiness::WaitingForSnapshot);
            commands.insert_resource(crate::client::ClientPlayableGate(false));
        }
        return;
    };
    if presented.as_ref().is_some_and(|presented| {
        presented.instance_id == snapshot.identity.instance_id
            && presented.recipe_fingerprint == snapshot.identity.recipe_fingerprint
            && presented.presentation_theme_id == snapshot.presentation_theme_id
    }) {
        return;
    }
    if dynamic_state.is_none() {
        return;
    }
    if let Err(error) = validate_client_grid_snapshot(snapshot, &grid_catalog.0) {
        warn!(%error, "client rejected authoritative map snapshot");
        commands.remove_resource::<PresentedMap>();
        commands.insert_resource(ClientMapReadiness::Invalid(error));
        commands.insert_resource(crate::client::ClientPlayableGate(false));
        return;
    }
    info!(
        instance_id = snapshot.identity.instance_id.0,
        snapshot_bytes = postcard::to_allocvec(snapshot).map_or(0, |bytes| bytes.len()),
        placements = snapshot.placements.len(),
        "client accepted authoritative map snapshot"
    );
    commands.insert_resource(PresentedMap {
        source_root: root,
        instance_id: snapshot.identity.instance_id,
        recipe_fingerprint: snapshot.identity.recipe_fingerprint,
        presentation_theme_id: snapshot.presentation_theme_id,
        playable_bounds: snapshot.dimensions.bounds(),
        camera_bounds: snapshot.dimensions.bounds(),
    });
    commands.insert_resource(ClientMapReadiness::Ready);
}

fn validate_client_grid_snapshot(
    snapshot: &ResolvedMapSnapshot,
    catalog: &MapContentCatalog,
) -> Result<(), String> {
    if snapshot.identity.instance_id.0 == 0
        || snapshot.catalog_schema_version != MAP_CATALOG_SCHEMA_VERSION
        || snapshot.recipe_schema_version != MAP_RECIPE_SCHEMA_VERSION
    {
        return Err("replicated map snapshot violates identity or schema".to_string());
    }
    let preset_id = snapshot
        .identity
        .source_preset_id
        .ok_or_else(|| "replicated map has no preset identity".to_string())?;
    let expected = catalog.resolve_preset(preset_id, snapshot.identity.instance_id)?;
    if expected.snapshot != *snapshot {
        return Err("replicated map differs from embedded canonical content".to_string());
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
            .resolve_preset(MapPresetId(1), MapInstanceId(instance))
            .unwrap()
            .snapshot
    }

    fn app_with_snapshot(snapshot: ResolvedMapSnapshot) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, MapContentPlugin, MapPresentationPlugin));
        let instance_id = snapshot.identity.instance_id;
        app.world_mut().spawn((
            MapRoot,
            instance_id,
            snapshot.identity,
            snapshot,
            MapDynamicState {
                map_instance_id: instance_id,
                generation: 1,
                revision: 0,
                terminal_states: Vec::new(),
            },
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
        let instance_id = replacement.identity.instance_id;
        app.world_mut().spawn((
            MapRoot,
            instance_id,
            replacement.identity,
            replacement,
            MapDynamicState {
                map_instance_id: instance_id,
                generation: 1,
                revision: 0,
                terminal_states: Vec::new(),
            },
        ));
        app.update();
        assert_eq!(
            app.world().resource::<PresentedMap>().instance_id,
            MapInstanceId(2)
        );
    }

    #[test]
    fn invalid_schema_fails_visibly_and_closes_gate() {
        let mut invalid = snapshot(1);
        invalid.catalog_schema_version = u16::MAX;
        let mut app = app_with_snapshot(invalid);
        app.update();
        assert!(matches!(
            app.world().resource::<ClientMapReadiness>(),
            ClientMapReadiness::Invalid(error) if error.contains("schema")
        ));
        assert!(!app.world().contains_resource::<PresentedMap>());
    }

    #[test]
    fn perimeter_visuals_are_outside_resolved_bounds() {
        let bounds = snapshot(1).dimensions.bounds();
        let shapes = perimeter_visual_shapes(bounds);
        assert!(shapes[0].0.x < bounds.min.x && shapes[1].0.x > bounds.max.x);
        assert!(shapes[2].0.y < bounds.min.y && shapes[3].0.y > bounds.max.y);
    }

    #[test]
    fn world_object_readiness_requires_exact_live_generation_and_accepts_terminal_absence() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let resolved = catalog
            .resolve_preset(BARREL_YARD_PRESET, MapInstanceId(5))
            .unwrap();
        let snapshot = resolved.snapshot.clone();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, MapContentPlugin, ClientMapPlugin));
        let root = app
            .world_mut()
            .spawn((
                MapRoot,
                snapshot.clone(),
                MapDynamicState {
                    map_instance_id: MapInstanceId(5),
                    generation: 1,
                    revision: 0,
                    terminal_states: Vec::new(),
                },
            ))
            .id();
        app.update();
        assert_eq!(
            *app.world().resource::<ClientWorldObjectReadiness>(),
            ClientWorldObjectReadiness::Waiting
        );

        let mut object_entities = std::collections::BTreeMap::new();
        for placement in resolved.dynamic_placements {
            let identity = DamageableTargetIdentity::MapObject {
                generation: MapDynamicGeneration {
                    map_instance_id: MapInstanceId(5),
                    generation: 1,
                },
                placement_id: placement.placement_id,
            };
            let entity = app
                .world_mut()
                .spawn((identity, DamageableWorldObject))
                .id();
            object_entities.insert(placement.placement_id, entity);
        }
        app.update();
        assert_eq!(
            *app.world().resource::<ClientWorldObjectReadiness>(),
            ClientWorldObjectReadiness::Ready
        );

        let terminal = MapPlacementId(101);
        app.world_mut()
            .entity_mut(object_entities[&terminal])
            .despawn();
        app.world_mut().entity_mut(root).insert(MapDynamicState {
            map_instance_id: MapInstanceId(5),
            generation: 1,
            revision: 1,
            terminal_states: vec![MapPlacementTransition {
                placement_id: terminal,
                outcome: MapPlacementOutcome::Removed,
            }],
        });
        app.update();
        assert_eq!(
            *app.world().resource::<ClientWorldObjectReadiness>(),
            ClientWorldObjectReadiness::Ready
        );
    }
}
