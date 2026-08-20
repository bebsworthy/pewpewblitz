//! Client terrain convergence, 3D presentation, and readiness input gating.

pub(crate) mod presentation;
pub(crate) mod recovery;

pub use presentation::{TerrainChunkVisual, build_terrain_chunk_mesh};
use recovery::{
    clear_terrain_convergence_on_disconnect, derive_expected_client_terrain,
    drive_terrain_wire_convergence,
};

use super::TerrainCorePlugin;
use super::model::TerrainGeneration;
use super::telemetry::TerrainTelemetry;
use crate::map::{InitialTerrainLayout, MapInstanceId};
use crate::matchplay::MatchId;
use bevy::prelude::*;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum TerrainClientSet {
    Derive,
    Converge,
    Present3d,
}

/// User-facing terrain synchronization state derived from the pure convergence phase.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub enum ClientTerrainReadiness {
    #[default]
    WaitingForMap,
    SyncingTerrain,
    RecoveringTerrain,
    Ready,
    Invalid(String),
}

/// One locally derived expectation from the replicated map and match state.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ExpectedClientTerrain {
    pub(super) generation: TerrainGeneration,
    pub(super) layout: InitialTerrainLayout,
    pub(super) derived_from: (MapInstanceId, MatchId),
}

/// Derivation cache so layout resolution runs only when the replicated pair changes.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub(crate) enum ExpectedClientTerrainSlot {
    #[default]
    Waiting,
    Failed(String),
    Derived(ExpectedClientTerrain),
}

pub struct ClientTerrainPlugin;

impl Plugin for ClientTerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TerrainCorePlugin)
            .init_resource::<ClientTerrainReadiness>()
            .init_resource::<ExpectedClientTerrainSlot>()
            .init_resource::<TerrainTelemetry>()
            .configure_sets(
                Update,
                (
                    TerrainClientSet::Derive,
                    TerrainClientSet::Converge.after(TerrainClientSet::Derive),
                    TerrainClientSet::Present3d.after(TerrainClientSet::Converge),
                ),
            )
            .add_systems(
                Update,
                derive_expected_client_terrain.in_set(TerrainClientSet::Derive),
            )
            .add_systems(
                Update,
                (
                    drive_terrain_wire_convergence,
                    clear_terrain_convergence_on_disconnect,
                )
                    .chain()
                    .in_set(TerrainClientSet::Converge),
            )
            .add_systems(
                Update,
                (
                    presentation::update_terrain_visuals,
                    presentation::spawn_terrain_debris,
                    presentation::expire_terrain_debris,
                )
                    .chain()
                    .in_set(TerrainClientSet::Present3d),
            )
            .add_systems(PostUpdate, gate_inputs_on_terrain_readiness);
    }
}

/// Terrain is the fourth readiness observation: inputs stay suppressed until convergence reports
/// the matching generation committed.
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn gate_inputs_on_terrain_readiness(
    readiness: Res<ClientTerrainReadiness>,
    config: Option<Res<crate::config::ClientNetworkConfig>>,
    joins: Query<&crate::client::ClientJoinStatus>,
    mut playable: ResMut<crate::client::ClientPlayableGate>,
    mut suppressed: Local<bool>,
) {
    if !matches!(&*readiness, ClientTerrainReadiness::Ready) {
        *suppressed = true;
        playable.0 = false;
        return;
    }
    if !*suppressed {
        return;
    }
    *suppressed = false;
    let headless = config.is_none_or(|config| config.headless);
    if headless
        && joins
            .iter()
            .any(|status| matches!(status.phase, crate::client::ClientJoinPhase::Active { .. }))
    {
        playable.0 = true;
    }
}
