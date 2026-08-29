//! Match-worker role, immutable manifest admission, and compatibility checks.
//!
//! Netcode client identity is explicit in the versioned manifest and is never inferred from route
//! peer IDs, ECS entities, or source addresses.

#[cfg(test)]
use super::build_app_with_config;
use super::build_match_worker_graph;
use crate::{
    builds::{BuildCatalog, BuildCatalogResource},
    combat::{FighterDefinitions, WeaponCatalogResource},
    config::{GameMode, MatchRulesProfile, ServerNetworkConfig},
    content::gameplay_content_fingerprint,
    map::{MapCatalogResource, MapContentCatalog, MapInstanceId, MapPresetId, ServerMapSelection},
    protocol::protocol_fingerprint,
};
#[cfg(test)]
use bevy::app::TerminalCtrlCHandlerPlugin;
use bevy::{app::ScheduleRunnerPlugin, prelude::*, state::app::StatesPlugin};
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
    InvalidDisplayName,
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

fn expected_routing_mode(mode: GameMode) -> brawler_routing::GameMode {
    crate::modes::descriptor_for_mode(mode)
        .expect("every configured game mode has a registered descriptor")
        .routing_mode
}

fn validate_manifest_map(
    config: &ServerNetworkConfig,
    manifest: &MatchManifestV1,
) -> Result<(), MatchWorkerManifestError> {
    let catalog = MapContentCatalog::embedded().map_err(MatchWorkerManifestError::Configuration)?;
    validate_manifest_map_against_catalog(config, manifest, &catalog)
}

fn validate_manifest_map_against_catalog(
    config: &ServerNetworkConfig,
    manifest: &MatchManifestV1,
    catalog: &MapContentCatalog,
) -> Result<(), MatchWorkerManifestError> {
    let preset_id = MapPresetId(manifest.map_preset);
    let mode = crate::modes::descriptor_for_mode(config.game_mode)
        .expect("every configured game mode has a registered descriptor");
    let preset = catalog
        .preset(preset_id)
        .ok_or(MatchWorkerManifestError::MapPresetMismatch)?;
    if preset.admission_revision != manifest.map_revision {
        return Err(MatchWorkerManifestError::MapRevisionMismatch);
    }
    if !mode.accepts_map(preset.recipe.mode_definition_id) {
        return Err(MatchWorkerManifestError::ModeMismatch);
    }
    catalog
        .resolve_preset(preset.id, MapInstanceId(1))
        .map(|_| ())
        .map_err(MatchWorkerManifestError::Configuration)?;
    Ok(())
}

fn expected_rules_profile(profile: MatchRulesProfile) -> u8 {
    profile.routing_id()
}

fn validate_build_rows(manifest: &MatchManifestV1) -> Result<(), MatchWorkerManifestError> {
    let builds = BuildCatalog::embedded().map_err(MatchWorkerManifestError::Configuration)?;
    let weapons = crate::combat::WeaponCatalog::embedded()
        .map_err(MatchWorkerManifestError::Configuration)?;
    let fighter_definitions = FighterDefinitions::default();
    let fighter = fighter_definitions
        .get(crate::combat::STANDARD_FIGHTER_DEFINITION)
        .ok_or(MatchWorkerManifestError::BuildSelectionMismatch)?;
    let validate = |snapshot_bytes: &brawler_routing::MatchBuildSnapshot,
                    recipe_fingerprint: u64,
                    revision: u16|
     -> Result<(), MatchWorkerManifestError> {
        let snapshot = crate::profiles::MatchBuildSnapshotV3::decode(snapshot_bytes)
            .map_err(|_| MatchWorkerManifestError::BuildSelectionMismatch)?;
        let resolved = snapshot
            .resolve(&builds, &weapons, fighter)
            .map_err(|_| MatchWorkerManifestError::BuildSelectionMismatch)?;
        if snapshot.accepted_identity != resolved.identity
            || resolved.identity.recipe_fingerprint.0 != recipe_fingerprint
            || resolved.identity.revision.0 != revision
        {
            return Err(MatchWorkerManifestError::BuildSelectionMismatch);
        }
        Ok(())
    };
    for participant in &manifest.participants {
        validate(
            &participant.build_snapshot,
            participant.recipe_fingerprint,
            participant.revision,
        )?;
    }
    for bot in &manifest.bots {
        validate(&bot.build_snapshot, bot.recipe_fingerprint, bot.revision)?;
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
    let mode = expected_routing_mode(config.game_mode);
    if manifest.mode != mode {
        return Err(MatchWorkerManifestError::ModeMismatch);
    }
    validate_manifest_map(config, manifest)?;
    if manifest.rules_profile != expected_rules_profile(config.match_rules_profile) {
        return Err(MatchWorkerManifestError::RulesProfileMismatch);
    }
    if manifest
        .participants
        .len()
        .saturating_add(manifest.bots.len())
        > config.max_clients
    {
        return Err(MatchWorkerManifestError::ParticipantCapacity);
    }
    let mut players = BTreeSet::new();
    let mut clients = BTreeSet::new();
    let mut teams = [0_usize; 2];
    for participant in &manifest.participants {
        if participant.team > 1 {
            return Err(MatchWorkerManifestError::InvalidTeam);
        }
        if !crate::lobby::normalize_proposed_display_name(participant.display_name.as_str())
            .is_ok_and(|name| name == participant.display_name.as_str())
        {
            return Err(MatchWorkerManifestError::InvalidDisplayName);
        }
        if !players.insert(participant.player_id.get()) {
            return Err(MatchWorkerManifestError::DuplicatePlayer);
        }
        if !clients.insert(participant.netcode_client_id.get()) {
            return Err(MatchWorkerManifestError::DuplicateClient);
        }
        teams[usize::from(participant.team)] += 1;
    }
    for bot in &manifest.bots {
        if bot.team > 1 {
            return Err(MatchWorkerManifestError::InvalidTeam);
        }
        if !crate::lobby::normalize_proposed_display_name(bot.display_name.as_str())
            .is_ok_and(|name| name == bot.display_name.as_str())
        {
            return Err(MatchWorkerManifestError::InvalidDisplayName);
        }
        if !players.insert(bot.player_id.get()) {
            return Err(MatchWorkerManifestError::DuplicatePlayer);
        }
        teams[usize::from(bot.team)] += 1;
    }
    if !matches!(teams, [1, 1] | [2, 2] | [3, 3]) {
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
    mut config: ServerNetworkConfig,
    manifest: MatchManifestV1,
) -> Result<App, MatchWorkerManifestError> {
    config.match_objective_target = Some(manifest.objective_target);
    config.match_duration_ticks = Some(manifest.match_duration_ticks);
    config.match_countdown_ticks = Some(manifest.countdown_ticks);
    config.match_respawn_ticks = Some(manifest.respawn_ticks);
    validate_match_manifest(&config, &manifest)?;
    let players_per_team = u8::try_from(
        manifest
            .participants
            .len()
            .saturating_add(manifest.bots.len())
            / 2,
    )
    .map_err(|_| MatchWorkerManifestError::ParticipantCapacity)?;
    let mut app = build_match_worker_graph(config, players_per_team);
    validate_runtime_identity(
        &mut app,
        manifest.common.protocol_registry_fingerprint,
        manifest.common.content_fingerprint,
    )?;
    app.insert_resource(ServerMapSelection {
        preset_id: MapPresetId(manifest.map_preset),
    });
    app.insert_resource(ServerRoleResource::match_worker(manifest));
    Ok(app)
}

/// Build the minimum lobby worker.  It intentionally contains the shared protocol registry and
/// Lightyear server lifecycle, but no map, combat, or match-authority plugins.  M01 keeps
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
        .add_plugins(StatesPlugin);
    crate::logging::add_log_plugin_once(&mut app);
    app.add_plugins(ServerPlugins {
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
    use brawler_routing::{
        AllocationId, Generation, LobbySessionId, LogicalServerId, ManifestCommon, MatchId,
        MatchManifestBot, PeerId, ProcessId, RouteId, WorkerId, WorkerRole,
    };

    fn manifest() -> MatchManifestV1 {
        let config = ServerNetworkConfig::default();
        let builds = BuildCatalog::embedded().unwrap();
        let weapons = crate::combat::WeaponCatalog::embedded().unwrap();
        let fighter_definitions = FighterDefinitions::default();
        let fighter = fighter_definitions
            .get(crate::combat::STANDARD_FIGHTER_DEFINITION)
            .unwrap();
        let brawler = crate::profiles::SavedBrawler {
            id: crate::profiles::SavedBrawlerId::new(1).unwrap(),
            creation_ordinal: 1,
            name: "Player One".into(),
            fighter_profile_id: crate::profiles::FighterProfileId(1),
            weapon_base_id: crate::profiles::WeaponBaseId(1),
            ultimate_id: crate::builds::UltimateDefinitionId(1),
            passive_ids: [
                crate::builds::PassiveDefinitionId(3),
                crate::builds::PassiveDefinitionId(4),
            ],
            equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
            revision: crate::profiles::ProfileRevision::INITIAL,
        };
        let snapshot = crate::profiles::MatchBuildSnapshotV3::from_brawler(
            &brawler, &builds, &weapons, fighter,
        )
        .unwrap();
        let map = MapContentCatalog::embedded().unwrap().presets[0].clone();
        let map_preset = map.id.0;
        let map_revision = map.admission_revision;
        MatchManifestV1 {
            common: ManifestCommon {
                manifest_version: 3,
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
            objective_target: 10,
            match_duration_ticks: 10_800,
            countdown_ticks: 180,
            respawn_ticks: 180,
            reserved: 0,
            seed: 1,
            participants: {
                let base = MatchManifestParticipant {
                    lobby_session_id: LobbySessionId::new(6).unwrap(),
                    player_id: brawler_routing::PlayerId::new(7).unwrap(),
                    netcode_client_id: brawler_routing::NetcodeClientId::new(8).unwrap(),
                    peer_id: PeerId::new(8).unwrap(),
                    team: 0,
                    display_name: brawler_routing::MatchDisplayName::new("Player One").unwrap(),
                    recipe_fingerprint: snapshot.accepted_identity.recipe_fingerprint.0,
                    revision: snapshot.accepted_identity.revision.0,
                    build_snapshot: snapshot.encode().unwrap(),
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
            bots: Vec::new(),
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
    fn admission_rejects_unknown_map_presets() {
        let config = ServerNetworkConfig::default();
        let catalog = MapContentCatalog::embedded().unwrap();

        let mut value = manifest();
        value.map_preset = 99;
        value.map_revision = 7;
        assert_eq!(
            validate_manifest_map_against_catalog(&config, &value, &catalog),
            Err(MatchWorkerManifestError::MapPresetMismatch)
        );
    }

    #[test]
    fn manifest_admission_accepts_feature_yard_wipeout_through_the_grid_catalog() {
        let config = ServerNetworkConfig::default();
        let catalog = MapContentCatalog::embedded().unwrap();
        let mut value = manifest();
        value.map_preset = crate::map::FEATURE_YARD_WIPEOUT_PRESET.0;
        value.map_revision = crate::map::FEATURE_YARD_WIPEOUT_ADMISSION_REVISION;
        assert!(validate_manifest_map_against_catalog(&config, &value, &catalog).is_ok());
    }

    #[test]
    fn manifest_admission_accepts_hidden_ashen_and_feature_yard_hot_zone_presets() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let mut wipeout = manifest();
        wipeout.map_preset = crate::map::ASHEN_COURT_PRESET.0;
        wipeout.map_revision = crate::map::ASHEN_COURT_ADMISSION_REVISION;
        assert!(
            validate_manifest_map_against_catalog(
                &ServerNetworkConfig::default(),
                &wipeout,
                &catalog
            )
            .is_ok()
        );

        let mut hot_zone = wipeout;
        hot_zone.mode = brawler_routing::GameMode::HotZone;
        hot_zone.map_preset = crate::map::FEATURE_YARD_HOT_ZONE_PRESET.0;
        hot_zone.map_revision = crate::map::FEATURE_YARD_HOT_ZONE_ADMISSION_REVISION;
        let config = ServerNetworkConfig {
            game_mode: GameMode::HotZone,
            ..default()
        };
        assert!(validate_manifest_map_against_catalog(&config, &hot_zone, &catalog).is_ok());
    }

    #[test]
    fn manifest_admission_accepts_all_proper_three_vs_three_map_presets() {
        let catalog = MapContentCatalog::embedded().unwrap();
        for (game_mode, routing_mode, preset, revision) in [
            (
                GameMode::Wipeout,
                brawler_routing::GameMode::Wipeout,
                crate::map::VERDANT_CROSSFIRE_PRESET,
                crate::map::VERDANT_CROSSFIRE_ADMISSION_REVISION,
            ),
            (
                GameMode::HotZone,
                brawler_routing::GameMode::HotZone,
                crate::map::SWITCHBACK_BASIN_PRESET,
                crate::map::SWITCHBACK_BASIN_ADMISSION_REVISION,
            ),
            (
                GameMode::Heist,
                brawler_routing::GameMode::Heist,
                crate::map::POWDERLINE_VAULT_PRESET,
                crate::map::POWDERLINE_VAULT_ADMISSION_REVISION,
            ),
        ] {
            let mut value = manifest();
            value.mode = routing_mode;
            value.map_preset = preset.0;
            value.map_revision = revision;
            let config = ServerNetworkConfig {
                game_mode,
                ..default()
            };
            assert!(validate_manifest_map_against_catalog(&config, &value, &catalog).is_ok());
        }
    }

    #[test]
    fn manifest_admission_preserves_stable_team_and_saved_brawler_loadout() {
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
        value.objective_target = 1;
        value.match_duration_ticks = 3_600;
        value.countdown_ticks = 60;
        value.respawn_ticks = 120;
        let mut worker = build_match_worker_app(config, value.clone()).unwrap();
        crate::test_app::finalize(&mut worker);
        assert_eq!(
            worker.world().resource::<ServerRoleResource>().manifest(),
            Some(&value)
        );
        assert_eq!(
            worker.world().resource::<ServerMapSelection>().preset_id,
            MapPresetId(value.map_preset)
        );
        assert!(!worker.is_plugin_added::<TerminalCtrlCHandlerPlugin>());
        assert_eq!(
            worker
                .world()
                .resource::<crate::matchplay::WipeoutRules>()
                .target_score,
            1
        );
        let lifecycle = worker
            .world()
            .resource::<crate::matchplay::MatchLifecycleRules>();
        assert_eq!(lifecycle.active_limit_ticks, 3_600);
        assert_eq!(lifecycle.countdown_ticks, 60);
        assert_eq!(lifecycle.respawn_delay_ticks, 120);
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

    type BotWorkerRow = (String, u16, bool, crate::protocol::FighterInput, bool);

    fn practice_worker_for_mode(
        game_mode: GameMode,
        map_preset: MapPresetId,
        map_revision: u16,
    ) -> App {
        let config = ServerNetworkConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            game_mode,
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
        let template = value.participants[0];
        value.mode = expected_routing_mode(game_mode);
        value.map_preset = map_preset.0;
        value.map_revision = map_revision;
        value.participants.truncate(1);
        value.bots = [(100, 0, "Bot 1"), (101, 1, "Bot 2"), (102, 1, "Bot 3")]
            .into_iter()
            .map(|(player_id, team, name)| MatchManifestBot {
                player_id: brawler_routing::PlayerId::new(player_id).unwrap(),
                team,
                display_name: brawler_routing::MatchDisplayName::new(name).unwrap(),
                recipe_fingerprint: template.recipe_fingerprint,
                revision: template.revision,
                build_snapshot: template.build_snapshot,
            })
            .collect();
        value.common.protocol_registry_fingerprint = protocol;
        value.common.content_fingerprint = content.0;
        let mut worker = build_match_worker_app(config, value).unwrap();
        crate::test_app::finalize(&mut worker);
        worker
    }

    fn activate_bot_worker(worker: &mut App) {
        worker
            .world_mut()
            .resource_mut::<BuildCatalogResource>()
            .0
            .fighter_profiles
            .default
            .maximum_health = 211;
        worker.update();
        let world = worker.world_mut();
        let mut roots = world
            .query_filtered::<&mut crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>(
            );
        roots.single_mut(world).unwrap().phase = crate::matchplay::MatchPhase::Active {
            ends_at_tick: 10_000,
        };
        let bot_entities = world
            .query_filtered::<Entity, With<crate::bots::PracticeBotController>>()
            .iter(world)
            .collect::<Vec<_>>();
        for entity in bot_entities {
            world
                .entity_mut(entity)
                .insert(crate::matchplay::ActiveCombatant);
        }
    }

    fn drive_bot_schedules(worker: &mut App, ticks: usize) {
        for _ in 0..ticks {
            let world = worker.world_mut();
            world.run_schedule(FixedUpdate);
            world.run_schedule(FixedPostUpdate);
        }
    }

    fn bot_worker_rows(worker: &mut App) -> Vec<BotWorkerRow> {
        let world = worker.world_mut();
        let mut query = world.query_filtered::<(
            &crate::matchplay::FighterDisplayName,
            &crate::combat::CurrentHealth,
            Has<lightyear::prelude::ControlledBy>,
            &lightyear::prelude::input::native::ActionState<crate::protocol::FighterInput>,
            Has<crate::bots::PracticeBotController>,
        ), With<crate::protocol::Fighter>>();
        query
            .iter(world)
            .map(|(name, health, controlled, input, has_controller)| {
                (
                    name.0.clone(),
                    health.0,
                    controlled,
                    input.0,
                    has_controller,
                )
            })
            .collect()
    }

    #[test]
    fn match_workers_control_manifest_bots_in_every_practice_mode() {
        let cases = [
            (
                GameMode::Wipeout,
                crate::map::FEATURE_YARD_WIPEOUT_PRESET,
                crate::map::FEATURE_YARD_WIPEOUT_ADMISSION_REVISION,
            ),
            (
                GameMode::HotZone,
                crate::map::FEATURE_YARD_HOT_ZONE_PRESET,
                crate::map::FEATURE_YARD_HOT_ZONE_ADMISSION_REVISION,
            ),
            (
                GameMode::Heist,
                crate::map::FEATURE_YARD_HEIST_PRESET,
                crate::map::FEATURE_YARD_HEIST_ADMISSION_REVISION,
            ),
        ];
        for (game_mode, map_preset, map_revision) in cases {
            let mut worker = practice_worker_for_mode(game_mode, map_preset, map_revision);
            activate_bot_worker(&mut worker);
            drive_bot_schedules(&mut worker, 12);
            let rows = bot_worker_rows(&mut worker);
            assert_eq!(rows.len(), 3, "unexpected {game_mode:?} bot count");
            assert!(rows.iter().all(|(_, health, _, _, _)| *health == 211));
            assert!(rows.iter().all(|(_, _, controlled, _, _)| !controlled));
            assert!(
                rows.iter().any(|(_, _, _, input, _)| {
                    input.move_axis.to_vec2() != Vec2::ZERO || input.gameplay_buttons != 0
                }),
                "{game_mode:?} bots stayed neutral after the reaction window"
            );
            assert!(
                rows.iter()
                    .all(|(_, _, _, _, has_controller)| *has_controller)
            );
            let mut names = rows
                .iter()
                .map(|(name, _, _, _, _)| name.as_str())
                .collect::<Vec<_>>();
            names.sort_unstable();
            assert_eq!(names, vec!["Bot 1", "Bot 2", "Bot 3"]);
        }
    }

    fn bot_input_points_toward(worker: &mut App, network_id: u64, target: Vec2) -> bool {
        let world = worker.world_mut();
        world
            .query_filtered::<(
                &crate::protocol::NetworkEntityId,
                &avian2d::prelude::Position,
                &lightyear::prelude::input::native::ActionState<crate::protocol::FighterInput>,
            ), With<crate::bots::PracticeBotController>>()
            .iter(world)
            .find(|(id, _, _)| id.0 == network_id)
            .is_some_and(|(_, position, input)| {
                (target - position.0)
                    .try_normalize()
                    .is_some_and(|direction| input.0.move_axis.to_vec2().dot(direction) > 0.5)
            })
    }

    #[test]
    fn objective_bots_emit_hot_zone_and_heist_directed_input_in_real_schedules() {
        let mut hot_zone = practice_worker_for_mode(
            GameMode::HotZone,
            crate::map::FEATURE_YARD_HOT_ZONE_PRESET,
            crate::map::FEATURE_YARD_HOT_ZONE_ADMISSION_REVISION,
        );
        activate_bot_worker(&mut hot_zone);
        let zone_center = hot_zone
            .world()
            .resource::<crate::map::ResolvedMap>()
            .objective_zone
            .expect("Hot Zone worker has its objective")
            .area
            .center;
        drive_bot_schedules(&mut hot_zone, 12);
        assert!(bot_input_points_toward(&mut hot_zone, 100, zone_center));
        assert!(bot_input_points_toward(&mut hot_zone, 102, zone_center));

        let mut heist = practice_worker_for_mode(
            GameMode::Heist,
            crate::map::FEATURE_YARD_HEIST_PRESET,
            crate::map::FEATURE_YARD_HEIST_ADMISSION_REVISION,
        );
        activate_bot_worker(&mut heist);
        let safe_positions = {
            let world = heist.world_mut();
            world
                .query::<(&crate::matchplay::HeistSafe, &avian2d::prelude::Position)>()
                .iter(world)
                .map(|(safe, position)| (safe.defending_team, position.0))
                .collect::<std::collections::BTreeMap<_, _>>()
        };
        drive_bot_schedules(&mut heist, 12);
        assert!(bot_input_points_toward(
            &mut heist,
            100,
            safe_positions[&crate::combat::TeamId(1)]
        ));
        assert!(bot_input_points_toward(
            &mut heist,
            102,
            safe_positions[&crate::combat::TeamId(0)]
        ));
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
                crate::map::FEATURE_YARD_WIPEOUT_PRESET.0,
                crate::map::FEATURE_YARD_WIPEOUT_ADMISSION_REVISION,
                expected_rules_profile(MatchRulesProfile::Production),
            )
        );
        assert_eq!(
            policy.for_mode(brawler_routing::GameMode::HotZone),
            brawler_routing::ModeAllocationPolicy::new(
                crate::map::FEATURE_YARD_HOT_ZONE_PRESET.0,
                crate::map::FEATURE_YARD_HOT_ZONE_ADMISSION_REVISION,
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
                manifest_version: 2,
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
            profile_database_path: "profiles.sqlite3".to_string(),
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
    }
}
