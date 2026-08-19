//! Match-worker role, immutable manifest admission, and compatibility checks.
//!
//! Netcode client identity is explicit in the versioned manifest and is never inferred from route
//! peer IDs, ECS entities, or source addresses.

#[cfg(test)]
use super::build_app_with_config;
use super::build_match_worker_graph;
use crate::{
    builds::{BuildCatalog, BuildCatalogResource, resolve_build_recipe},
    combat::{FighterDefinitions, WeaponCatalogResource},
    config::{GameMode, MatchRulesProfile, ServerNetworkConfig},
    content::gameplay_content_fingerprint,
    map::{
        BUILT_IN_MAP_PRESET, HOT_ZONE_LAYOUT_SCHEMA_VERSION, HOT_ZONE_MAP_PRESET,
        MapCatalogResource, WIPEOUT_LAYOUT_SCHEMA_VERSION,
    },
    protocol::protocol_fingerprint,
};
#[cfg(test)]
use bevy::app::TerminalCtrlCHandlerPlugin;
use bevy::{app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*, state::app::StatesPlugin};
use brawler_routing::{LobbyManifest, MatchManifestParticipant, MatchManifestV1};
use lightyear::prelude::server::ServerPlugins;
use lightyear::prelude::{PeerId, RemoteId};
use std::collections::BTreeSet;

/// Server topology selected for one application instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerRole {
    /// The v1 direct-UDP composition.  This remains the default and has no manifest gate.
    DirectBaseline,
    /// The isolated minimum lobby worker.  The lobby manifest is immutable for the process
    /// lifetime and arrives through the worker control stream before app construction.
    LobbyWorker(LobbyManifest),
    /// An isolated match worker with one immutable, validated participant manifest.
    MatchWorker(MatchManifestV1),
}

/// Server-owned role/resource installed before the first schedule runs.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct ServerRoleResource(pub ServerRole);

impl Default for ServerRoleResource {
    fn default() -> Self {
        Self(ServerRole::DirectBaseline)
    }
}

impl ServerRoleResource {
    #[must_use]
    pub fn direct_baseline() -> Self {
        Self(ServerRole::DirectBaseline)
    }

    #[must_use]
    pub fn match_worker(manifest: MatchManifestV1) -> Self {
        Self(ServerRole::MatchWorker(manifest))
    }

    #[must_use]
    pub fn lobby_worker(manifest: LobbyManifest) -> Self {
        Self(ServerRole::LobbyWorker(manifest))
    }

    #[must_use]
    pub fn manifest(&self) -> Option<&MatchManifestV1> {
        match &self.0 {
            ServerRole::DirectBaseline | ServerRole::LobbyWorker(_) => None,
            ServerRole::MatchWorker(manifest) => Some(manifest),
        }
    }

    #[must_use]
    pub fn lobby_manifest(&self) -> Option<&LobbyManifest> {
        match &self.0 {
            ServerRole::LobbyWorker(manifest) => Some(manifest),
            ServerRole::DirectBaseline | ServerRole::MatchWorker(_) => None,
        }
    }

    #[must_use]
    pub fn match_worker_match_id(&self) -> Option<crate::matchplay::MatchId> {
        self.manifest()
            .map(|manifest| crate::matchplay::MatchId(manifest.match_id.get()))
            .filter(|match_id| match_id.0 != 0)
    }
}

/// Application-owned identity supplied to the Bevy-free supervisor and revalidated by every
/// worker before it reports Ready.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoutingIdentity {
    pub network_protocol: u64,
    pub protocol_registry_fingerprint: u64,
    pub content_fingerprint: u64,
}

/// Compute routed process identity from the same protocol registry and embedded catalogs used by
/// production server apps. The temporary app is never run and owns no network endpoint.
pub fn routing_identity() -> Result<RoutingIdentity, MatchWorkerManifestError> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .add_plugins(ServerPlugins {
            tick_duration: crate::timing::SIMULATION_TICK,
        })
        .add_plugins(crate::protocol::ProtocolPlugin);
    let protocol_registry_fingerprint = protocol_fingerprint(app.world_mut());
    let content_fingerprint = gameplay_content_fingerprint(
        &app.world().resource::<WeaponCatalogResource>().0,
        &app.world().resource::<MapCatalogResource>().0,
        &app.world().resource::<BuildCatalogResource>().0,
    )
    .map_err(MatchWorkerManifestError::Configuration)?;
    Ok(RoutingIdentity {
        network_protocol: crate::protocol::NETWORK_PROTOCOL_ID,
        protocol_registry_fingerprint,
        content_fingerprint: content_fingerprint.0,
    })
}

/// Why a worker manifest cannot be installed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MatchWorkerManifestError {
    InvalidManifest,
    Configuration(String),
    NetworkProtocolMismatch,
    ProtocolRegistryMismatch,
    ContentMismatch,
    BuildVersionMismatch,
    ModeMismatch,
    MapPresetMismatch,
    MapRevisionMismatch,
    RulesProfileMismatch,
    ParticipantCapacity,
    DuplicatePlayer,
    DuplicateClient,
    UnlistedClient,
    InvalidTeam,
    BuildSelectionMismatch,
}

/// Return the manifest participant selected by an authenticated Netcode client id, rejecting
/// duplicate manifest entries and duplicate live admissions rather than picking first-match order.
pub fn admit_manifest_client<'a>(
    manifest: &'a MatchManifestV1,
    client_id: u64,
    peer_id: Option<brawler_routing::PeerId>,
    admitted_client_ids: &BTreeSet<u64>,
) -> Result<&'a MatchManifestParticipant, MatchWorkerManifestError> {
    if admitted_client_ids.contains(&client_id) {
        return Err(MatchWorkerManifestError::DuplicateClient);
    }
    let mut found = None;
    for participant in &manifest.participants {
        if participant.netcode_client_id.get() == client_id && Some(participant.peer_id) == peer_id
        {
            if found.is_some() {
                return Err(MatchWorkerManifestError::DuplicateClient);
            }
            found = Some(participant);
        }
    }
    found.ok_or(MatchWorkerManifestError::UnlistedClient)
}

fn expected_mode_fields(mode: GameMode) -> (brawler_routing::GameMode, u16, u16) {
    match mode {
        GameMode::Wipeout => (
            brawler_routing::GameMode::Wipeout,
            BUILT_IN_MAP_PRESET.0,
            WIPEOUT_LAYOUT_SCHEMA_VERSION,
        ),
        GameMode::HotZone => (
            brawler_routing::GameMode::HotZone,
            HOT_ZONE_MAP_PRESET.0,
            HOT_ZONE_LAYOUT_SCHEMA_VERSION,
        ),
    }
}

fn expected_rules_profile(profile: MatchRulesProfile) -> u8 {
    match profile {
        MatchRulesProfile::Production => 1,
        MatchRulesProfile::ProcessVerification => 2,
    }
}

fn validate_build_rows(manifest: &MatchManifestV1) -> Result<(), MatchWorkerManifestError> {
    let builds = BuildCatalog::embedded().map_err(MatchWorkerManifestError::Configuration)?;
    let weapons = crate::combat::WeaponCatalog::embedded()
        .map_err(MatchWorkerManifestError::Configuration)?;
    let fighter_definitions = FighterDefinitions::default();
    let fighter = fighter_definitions
        .get(crate::combat::STANDARD_FIGHTER_DEFINITION)
        .ok_or(MatchWorkerManifestError::BuildSelectionMismatch)?;
    for participant in &manifest.participants {
        let snapshot = crate::builds::MatchBuildSnapshotV1::decode(&participant.build_snapshot)
            .map_err(|_| MatchWorkerManifestError::BuildSelectionMismatch)?;
        let (recipe, source_preset) = match snapshot.candidate.selection {
            crate::builds::BuildSelection::Preset(preset_id) => {
                let definition = builds
                    .preset(preset_id)
                    .ok_or(MatchWorkerManifestError::BuildSelectionMismatch)?;
                (definition.recipe, Some(preset_id))
            }
            crate::builds::BuildSelection::Custom(recipe) => (recipe, None),
        };
        let resolved = resolve_build_recipe(&builds, &weapons, fighter, recipe, source_preset)
            .map_err(|_| MatchWorkerManifestError::BuildSelectionMismatch)?;
        if snapshot.candidate.build_revision != builds.balance_revision
            || snapshot.accepted.canonical_recipe != recipe
            || snapshot.accepted.identity != resolved.identity
            || snapshot.accepted.total_points != resolved.total_points
            || resolved.identity.source_build_preset_id.map(|id| id.0)
                != participant.source_build_preset
            || resolved.identity.recipe_fingerprint.0 != participant.recipe_fingerprint
            || resolved.identity.revision.0 != participant.revision
        {
            return Err(MatchWorkerManifestError::BuildSelectionMismatch);
        }
    }
    Ok(())
}

/// Validate all manifest/config identity and stable roster fields that are independent of Bevy
/// app construction. The registry fingerprint is checked by the builder after it reuses the
/// production plugin graph; content is recomputed here from the same embedded catalogs.
pub fn validate_match_manifest(
    config: &ServerNetworkConfig,
    manifest: &MatchManifestV1,
) -> Result<(), MatchWorkerManifestError> {
    config
        .validate()
        .map_err(MatchWorkerManifestError::Configuration)?;
    manifest
        .validate()
        .map_err(|_| MatchWorkerManifestError::InvalidManifest)?;
    if manifest.common.network_protocol != config.network_protocol_id {
        return Err(MatchWorkerManifestError::NetworkProtocolMismatch);
    }
    if manifest.common.process_id.get() == 0 {
        return Err(MatchWorkerManifestError::InvalidManifest);
    }
    let (mode, map_preset, map_revision) = expected_mode_fields(config.game_mode);
    if manifest.mode != mode {
        return Err(MatchWorkerManifestError::ModeMismatch);
    }
    if manifest.map_preset != map_preset {
        return Err(MatchWorkerManifestError::MapPresetMismatch);
    }
    if manifest.map_revision != map_revision {
        return Err(MatchWorkerManifestError::MapRevisionMismatch);
    }
    if manifest.rules_profile != expected_rules_profile(config.match_rules_profile) {
        return Err(MatchWorkerManifestError::RulesProfileMismatch);
    }
    if manifest.participants.len() > config.max_clients {
        return Err(MatchWorkerManifestError::ParticipantCapacity);
    }
    let mut players = BTreeSet::new();
    let mut clients = BTreeSet::new();
    let mut teams = [0_usize; 2];
    for participant in &manifest.participants {
        if participant.team > 1 {
            return Err(MatchWorkerManifestError::InvalidTeam);
        }
        if !players.insert(participant.player_id.get()) {
            return Err(MatchWorkerManifestError::DuplicatePlayer);
        }
        if !clients.insert(participant.netcode_client_id.get()) {
            return Err(MatchWorkerManifestError::DuplicateClient);
        }
        teams[usize::from(participant.team)] += 1;
    }
    if !matches!(teams, [2, 2] | [3, 3]) {
        return Err(MatchWorkerManifestError::ParticipantCapacity);
    }
    validate_build_rows(manifest)?;
    Ok(())
}

fn validate_runtime_identity(
    app: &mut App,
    protocol_registry_fingerprint: u64,
    content_fingerprint: u64,
) -> Result<(), MatchWorkerManifestError> {
    let protocol = protocol_fingerprint(app.world_mut());
    if protocol_registry_fingerprint != protocol {
        return Err(MatchWorkerManifestError::ProtocolRegistryMismatch);
    }
    let content = gameplay_content_fingerprint(
        &app.world().resource::<WeaponCatalogResource>().0,
        &app.world().resource::<MapCatalogResource>().0,
        &app.world().resource::<BuildCatalogResource>().0,
    )
    .map_err(MatchWorkerManifestError::Configuration)?;
    if content_fingerprint != content.0 {
        return Err(MatchWorkerManifestError::ContentMismatch);
    }
    Ok(())
}

/// Build a match worker by reusing the production dedicated-server graph. The role is installed
/// before the caller can run or update the app; invalid compatibility never reaches Ready.
pub fn build_match_worker_app(
    config: ServerNetworkConfig,
    manifest: MatchManifestV1,
) -> Result<App, MatchWorkerManifestError> {
    validate_match_manifest(&config, &manifest)?;
    let players_per_team = u8::try_from(manifest.participants.len() / 2)
        .map_err(|_| MatchWorkerManifestError::ParticipantCapacity)?;
    let mut app = build_match_worker_graph(config, players_per_team);
    validate_runtime_identity(
        &mut app,
        manifest.common.protocol_registry_fingerprint,
        manifest.common.content_fingerprint,
    )?;
    app.insert_resource(ServerRoleResource::match_worker(manifest));
    Ok(app)
}

/// Build the minimum lobby worker.  It intentionally contains the shared protocol registry and
/// Lightyear server lifecycle, but no map, combat, terrain, or match-authority plugins.  M01 keeps
/// allocation policy outside this process graph; the immutable manifest still gates the process
/// identity and protocol before the first schedule runs.
pub fn build_lobby_worker_app(
    config: ServerNetworkConfig,
    manifest: LobbyManifest,
) -> Result<App, MatchWorkerManifestError> {
    config
        .validate()
        .map_err(MatchWorkerManifestError::Configuration)?;
    manifest
        .validate()
        .map_err(|_| MatchWorkerManifestError::InvalidManifest)?;
    if manifest.common.network_protocol != config.network_protocol_id {
        return Err(MatchWorkerManifestError::NetworkProtocolMismatch);
    }
    let catalog = super::lobby::resolve_operator_catalog(&manifest.raw_catalog)
        .map_err(MatchWorkerManifestError::Configuration)?;
    let mut app = App::new();
    app.insert_resource(config)
        .add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
            crate::timing::SIMULATION_TICK,
        )))
        .add_plugins(StatesPlugin)
        .add_plugins(LogPlugin::default())
        .add_plugins(ServerPlugins {
            tick_duration: crate::timing::SIMULATION_TICK,
        })
        .add_plugins((
            crate::protocol::ProtocolPlugin,
            super::lobby::LobbyPlugin,
            super::routed_worker::RoutedWorkerPlugin,
        ));
    validate_runtime_identity(
        &mut app,
        manifest.common.protocol_registry_fingerprint,
        manifest.common.content_fingerprint,
    )?;
    app.insert_resource(ServerRoleResource::lobby_worker(manifest));
    app.insert_resource(catalog);
    Ok(app)
}

/// Authenticated Netcode client identity. Other Lightyear peer kinds are never admitted to a
/// routed match worker.
#[must_use]
pub fn authenticated_netcode_id(remote_id: &RemoteId) -> Option<u64> {
    match remote_id.0 {
        PeerId::Netcode(client_id) => Some(client_id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builds::BuildPresetId;
    use brawler_routing::{
        AllocationId, Generation, LobbySessionId, LogicalServerId, ManifestCommon, MatchId, PeerId,
        ProcessId, RouteId, WorkerId, WorkerRole,
    };

    fn manifest() -> MatchManifestV1 {
        let config = ServerNetworkConfig::default();
        let builds = BuildCatalog::embedded().unwrap();
        let weapons = crate::combat::WeaponCatalog::embedded().unwrap();
        let fighter_definitions = FighterDefinitions::default();
        let fighter = fighter_definitions
            .get(crate::combat::STANDARD_FIGHTER_DEFINITION)
            .unwrap();
        let preset = builds.preset(BuildPresetId(1)).unwrap();
        let resolved =
            resolve_build_recipe(&builds, &weapons, fighter, preset.recipe, Some(preset.id))
                .unwrap();
        let (_, map_preset, map_revision) = expected_mode_fields(config.game_mode);
        MatchManifestV1 {
            common: ManifestCommon {
                manifest_version: 2,
                role: WorkerRole::Match,
                logical_server_id: LogicalServerId::new(1).unwrap(),
                process_id: ProcessId::new(2).unwrap(),
                worker_id: WorkerId::new(3).unwrap(),
                generation: Generation::new(1).unwrap(),
                network_protocol: config.network_protocol_id,
                protocol_registry_fingerprint: 0,
                content_fingerprint: 0,
                route_version: brawler_routing::ROUTE_VERSION_V1,
                packet_version: brawler_routing::PACKET_VERSION_V1,
                control_version: brawler_routing::CONTROL_VERSION_CURRENT,
                flags: 0,
            },
            request_id: brawler_routing::RequestId::new(3).unwrap(),
            match_id: MatchId::new(4).unwrap(),
            allocation_id: AllocationId::new(5).unwrap(),
            mode: brawler_routing::GameMode::Wipeout,
            map_preset,
            map_revision,
            rules_profile: 1,
            reserved: 0,
            seed: 1,
            participants: {
                let base = MatchManifestParticipant {
                    lobby_session_id: LobbySessionId::new(6).unwrap(),
                    player_id: brawler_routing::PlayerId::new(7).unwrap(),
                    netcode_client_id: brawler_routing::NetcodeClientId::new(8).unwrap(),
                    peer_id: PeerId::new(8).unwrap(),
                    team: 0,
                    source_build_preset: Some(1),
                    recipe_fingerprint: resolved.identity.recipe_fingerprint.0,
                    revision: resolved.identity.revision.0,
                    build_snapshot: crate::builds::MatchBuildSnapshotV1 {
                        schema_version: crate::builds::MatchBuildSnapshotV1::SCHEMA_VERSION,
                        candidate: crate::builds::BuildCandidate {
                            build_revision: builds.balance_revision,
                            selection: crate::builds::BuildSelection::Preset(preset.id),
                        },
                        accepted: crate::builds::AcceptedBuildSummary {
                            canonical_recipe: preset.recipe,
                            identity: resolved.identity,
                            total_points: resolved.total_points,
                        },
                    }
                    .encode()
                    .unwrap(),
                };
                let mut second = base;
                second.lobby_session_id = LobbySessionId::new(9).unwrap();
                second.player_id = brawler_routing::PlayerId::new(10).unwrap();
                second.netcode_client_id = brawler_routing::NetcodeClientId::new(11).unwrap();
                second.peer_id = PeerId::new(11).unwrap();
                second.team = 1;
                let mut third = base;
                third.lobby_session_id = LobbySessionId::new(12).unwrap();
                third.player_id = brawler_routing::PlayerId::new(13).unwrap();
                third.netcode_client_id = brawler_routing::NetcodeClientId::new(14).unwrap();
                third.peer_id = PeerId::new(14).unwrap();
                let mut fourth = base;
                fourth.lobby_session_id = LobbySessionId::new(15).unwrap();
                fourth.player_id = brawler_routing::PlayerId::new(16).unwrap();
                fourth.netcode_client_id = brawler_routing::NetcodeClientId::new(17).unwrap();
                fourth.peer_id = PeerId::new(17).unwrap();
                fourth.team = 1;
                vec![base, second, third, fourth]
            },
            heartbeat_ms: 1_000,
            nonce: 9,
            digest: [0; 32],
        }
    }

    #[test]
    fn manifest_validation_rejects_duplicate_players_clients_and_bad_identity() {
        let mut value = manifest();
        assert!(validate_match_manifest(&ServerNetworkConfig::default(), &value).is_ok());
        value.participants.push(value.participants[0]);
        assert_eq!(
            validate_match_manifest(&ServerNetworkConfig::default(), &value),
            Err(MatchWorkerManifestError::DuplicatePlayer)
        );
        value.participants.pop();
        let mut admitted = BTreeSet::new();
        admitted.insert(8);
        assert_eq!(
            admit_manifest_client(&value, 8, Some(value.participants[0].peer_id), &admitted),
            Err(MatchWorkerManifestError::DuplicateClient)
        );
        assert_eq!(
            admit_manifest_client(
                &value,
                99,
                Some(value.participants[0].peer_id),
                &BTreeSet::new()
            ),
            Err(MatchWorkerManifestError::UnlistedClient)
        );
        assert_eq!(
            admit_manifest_client(&value, 8, PeerId::new(99), &BTreeSet::new()),
            Err(MatchWorkerManifestError::UnlistedClient)
        );
    }

    #[test]
    fn manifest_admission_preserves_stable_team_and_build_selection() {
        let value = manifest();
        let participant = admit_manifest_client(
            &value,
            8,
            Some(value.participants[0].peer_id),
            &BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(participant.player_id.get(), 7);
        assert_eq!(participant.team, 0);
        assert_eq!(participant.source_build_preset, Some(1));
        assert_ne!(participant.recipe_fingerprint, 0);
    }

    #[test]
    fn match_worker_builder_reuses_direct_server_plugin_graph_and_installs_role() {
        let config = ServerNetworkConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            ..default()
        };
        let mut direct = build_app_with_config(config.clone());
        let protocol = protocol_fingerprint(direct.world_mut());
        let content = gameplay_content_fingerprint(
            &direct.world().resource::<WeaponCatalogResource>().0,
            &direct.world().resource::<MapCatalogResource>().0,
            &direct.world().resource::<BuildCatalogResource>().0,
        )
        .unwrap();
        let mut value = manifest();
        // MatchId is a routed u128 identity; the worker must install it without narrowing to u64.
        value.match_id = MatchId::new(u128::from(u64::MAX) + 1).unwrap();
        value.common.protocol_registry_fingerprint = protocol;
        value.common.content_fingerprint = content.0;
        let mut worker = build_match_worker_app(config, value.clone()).unwrap();
        assert_eq!(
            worker.world().resource::<ServerRoleResource>().manifest(),
            Some(&value)
        );
        assert!(!worker.is_plugin_added::<TerminalCtrlCHandlerPlugin>());
        worker.update();
        let world = worker.world_mut();
        let state = world
            .query_filtered::<&crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>()
            .single(world);
        assert_eq!(
            state.unwrap().match_id,
            crate::matchplay::MatchId(value.match_id.get())
        );
    }

    #[test]
    fn routing_identity_matches_the_production_server_graph() {
        let identity = routing_identity().unwrap();
        let mut direct = build_app_with_config(ServerNetworkConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            ..default()
        });
        let protocol = protocol_fingerprint(direct.world_mut());
        let content = gameplay_content_fingerprint(
            &direct.world().resource::<WeaponCatalogResource>().0,
            &direct.world().resource::<MapCatalogResource>().0,
            &direct.world().resource::<BuildCatalogResource>().0,
        )
        .unwrap();
        assert_eq!(
            identity.network_protocol,
            crate::protocol::NETWORK_PROTOCOL_ID
        );
        assert_eq!(identity.protocol_registry_fingerprint, protocol);
        assert_eq!(identity.content_fingerprint, content.0);
    }

    #[test]
    fn supervisor_m01_policy_matches_server_admission_constants() {
        let policy = brawler_routing::AllocationPolicy::brawler_m01();
        assert_eq!(
            policy.for_mode(brawler_routing::GameMode::Wipeout),
            brawler_routing::ModeAllocationPolicy::new(
                BUILT_IN_MAP_PRESET.0,
                WIPEOUT_LAYOUT_SCHEMA_VERSION,
                expected_rules_profile(MatchRulesProfile::Production),
            )
        );
        assert_eq!(
            policy.for_mode(brawler_routing::GameMode::HotZone),
            brawler_routing::ModeAllocationPolicy::new(
                HOT_ZONE_MAP_PRESET.0,
                HOT_ZONE_LAYOUT_SCHEMA_VERSION,
                expected_rules_profile(MatchRulesProfile::Production),
            )
        );
    }

    #[test]
    fn lobby_worker_validates_runtime_identity_without_gameplay_authority() {
        let config = ServerNetworkConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            ..default()
        };
        let mut identity_app = build_app_with_config(config.clone());
        let protocol = protocol_fingerprint(identity_app.world_mut());
        let content = gameplay_content_fingerprint(
            &identity_app.world().resource::<WeaponCatalogResource>().0,
            &identity_app.world().resource::<MapCatalogResource>().0,
            &identity_app.world().resource::<BuildCatalogResource>().0,
        )
        .unwrap();
        let mut lobby = LobbyManifest {
            common: ManifestCommon {
                manifest_version: 1,
                role: WorkerRole::Lobby,
                logical_server_id: LogicalServerId::new(10).unwrap(),
                process_id: ProcessId::new(11).unwrap(),
                worker_id: WorkerId::new(12).unwrap(),
                generation: Generation::new(1).unwrap(),
                network_protocol: config.network_protocol_id,
                protocol_registry_fingerprint: protocol.wrapping_add(1),
                content_fingerprint: content.0,
                route_version: brawler_routing::ROUTE_VERSION_V1,
                packet_version: brawler_routing::PACKET_VERSION_V1,
                control_version: brawler_routing::CONTROL_VERSION_CURRENT,
                flags: 0,
            },
            default_route_id: RouteId::new(13).unwrap(),
            max_authenticated_sessions: 32,
            outstanding_allocations: 2,
            active_matches: 4,
            heartbeat_ms: 1_000,
            raw_catalog: include_bytes!("../../config/server/game-types.ron").to_vec(),
            raw_catalog_fingerprint: brawler_routing::raw_catalog_fingerprint(include_bytes!(
                "../../config/server/game-types.ron"
            )),
            nonce: 14,
            digest: [0; 32],
        };
        assert!(matches!(
            build_lobby_worker_app(config.clone(), lobby.clone()),
            Err(MatchWorkerManifestError::ProtocolRegistryMismatch)
        ));
        lobby.common.protocol_registry_fingerprint = protocol;
        let worker = build_lobby_worker_app(config, lobby).unwrap();
        assert!(!worker.is_plugin_added::<TerminalCtrlCHandlerPlugin>());
        assert!(!worker.is_plugin_added::<crate::combat::ServerCombatPlugin>());
        assert!(!worker.is_plugin_added::<crate::map::AuthoritativeMapPlugin>());
        assert!(!worker.is_plugin_added::<crate::terrain::AuthoritativeTerrainPlugin>());
    }
}
