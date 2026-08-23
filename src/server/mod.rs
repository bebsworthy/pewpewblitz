//! Dedicated-server composition and authoritative network-session concerns.

#[cfg(test)]
use crate::combat::TestDummy;
use crate::{
    VERSION,
    builds::{AbilityState, PassiveRuntimeState},
    combat::{
        ActiveEffects, AuthoritativeTick, CombatCue, CombatEvidenceSnapshots, CombatStateSnapshot,
        CombatTelemetry, CurrentHealth, SelectingBuild, ServerCombatPlugin, SpawnState, TeamId,
        WeaponCatalogResource, WeaponPhase, WeaponPresetId, WeaponState, WeaponTelemetry,
        WeaponTelemetryKey, decode_combat_cue, default_fighter_runtime, encode_state_snapshot,
    },
    config::{GameMode, MatchRulesProfile, NetworkTransport, ServerNetworkConfig},
    gameplay::GameplayPlugin,
    map::{
        AuthoritativeMapPlugin, CROSSROADS_HOT_ZONE_PRESET, CROSSROADS_PRESET, MapStartupSet,
        ResolvedMap, ServerMapSelection, SpawnAssignment, SpawnPointCatalog,
    },
    matchplay::{
        AuthoritativeMatchPlugin, MatchLifecycleRules, MatchMember, MatchModeSetup,
        MatchParticipant, MatchPhase, MatchRoot, MatchState, SpawnCandidate,
        WIPEOUT_RULES_REVISION, WipeoutModePlugin, WipeoutRules, assigned_team, select_spawn,
    },
    movement::{
        AuthoritativeMovementPlugin, AvianNetworkPlugin, InputFreshness, InputValidationState,
        MovementTuning,
    },
    protocol::{
        BuildSelectionOutcome, BuildSelectionRequest, DEVELOPMENT_PRIVATE_KEY, Fighter,
        MatchCommand, MatchCommandDecision, MatchCommandOutcome, MatchCommandRequest, MatchHello,
        MatchJoinOutcome, MatchJoinRejection, MatchLoadingClientAction, MatchLoadingClientMessage,
        MatchLoadingServerMessage, MatchLoadingServerOutcome, MatchLoadingStatus, NetworkEntityId,
        PlaceholderState, PlayerId, ProtocolFingerprint, ProtocolPlugin, QueueSnapshotChannel,
        SessionChannel,
    },
};
use avian2d::prelude::{
    AngularVelocity, Collider, CollisionLayers, CustomPositionIntegration, LinearVelocity,
    Position, RigidBody, Rotation,
};
use bevy::{
    app::{ScheduleRunnerPlugin, TerminalCtrlCHandlerPlugin},
    ecs::error::{FallbackErrorHandler, error},
    ecs::system::SystemParam,
    log::LogPlugin,
    prelude::*,
    state::app::StatesPlugin,
};
use core::time::Duration;
use lightyear::prelude::server::{
    NetcodeConfig, NetcodeServer, ServerPlugins, Start, Started, Stop, Stopped,
};
use lightyear::prelude::server::{Server as LightyearServer, ServerUdpIo};
use lightyear::prelude::{Connected, Disconnected, LinkOf, Linked, LocalAddr, RemoteId};
use lightyear::prelude::{
    ControlledBy, InterpolationTarget, Lifetime, MessageReceiver, MessageSender, NetworkTarget,
    Replicate, ReplicationMetadata, ReplicationSender,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fmt::Write as _,
    fs,
    path::PathBuf,
    time::Instant,
};

mod admission;
#[cfg(feature = "balance-lab")]
mod balance_lab;
mod lobby;
mod practice;
mod routed_worker;
mod verification;
mod worker;
pub use admission::{
    MatchWorkerManifestError, ServerRole, ServerRoleResource, admit_manifest_client,
    authenticated_netcode_id, build_lobby_worker_app, build_match_worker_app, routing_identity,
    validate_match_manifest,
};
pub use lobby::{
    LobbyBuildIdentity, LobbyClient, LobbyControlFrame, LobbyPlugin, LobbySessionIdSource,
    LobbyState, QueueCommandResult, QueueState, QueueTelemetry, QueueTicket, QueueTicketIdSource,
    SnapshotPublication, default_build_identity,
};
pub use routed_worker::{
    RoutedPeer, RoutedPeerClose, RoutedWorker, RoutedWorkerFailure, RoutedWorkerPlugin,
};
#[cfg(test)]
#[allow(clippy::wildcard_imports)]
use verification::*;
use verification::{verify_process_combat, verify_process_match, verify_process_movement};
pub use worker::{
    LobbyControlInbox, LobbyControlOutbox, WorkerBootstrap, WorkerBootstrapError,
    WorkerEntrypointRole, WorkerLaunchArguments, install_routed_worker_endpoint,
    parse_worker_arguments,
};

/// Server-side session phase. Lightyear lifecycle components remain the connection truth.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub enum ServerSessionPhase {
    AwaitingHello,
    Active {
        player_id: PlayerId,
        network_entity_id: NetworkEntityId,
    },
    Rejected,
}

#[derive(Component, Debug)]
pub struct ServerSession {
    pub phase: ServerSessionPhase,
    pub deadline: Duration,
    pub last_outcome: Option<MatchJoinOutcome>,
    pub disconnect_requested: bool,
    pub last_selection_request: Option<BuildSelectionRequest>,
    pub last_selection_outcome: Option<BuildSelectionOutcome>,
    pub last_selection_response: Option<BuildSelectionOutcome>,
    pub last_match_request: Option<MatchCommandRequest>,
    pub last_match_outcome: Option<MatchCommandOutcome>,
}

#[derive(Resource, Default, Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "bounded one-shot delivery and shutdown flags keep the loading path direct"
)]
struct MatchLoadingGate {
    checked_in: BTreeSet<u64>,
    readiness_committed: bool,
    cancelled: bool,
    cancel_emitted: bool,
    cancelling_client: Option<u64>,
    cancellation_outcome: Option<MatchLoadingServerOutcome>,
    activated_emitted: bool,
    deadline: Option<Duration>,
    terminal_failure: bool,
    terminal_notified: BTreeSet<u64>,
    last_status: Option<(u8, u8, crate::lobby::MatchLoadingPhase)>,
    status_revision: u32,
    countdown_observed: bool,
    start_failure_emitted: bool,
    terminal_exit_deadline: Option<Duration>,
    terminal_exit_requested: bool,
}

#[derive(Resource, Default, Debug)]
struct ServerShutdown {
    requested_exit: Option<AppExit>,
}

#[derive(Resource, Debug)]
struct ServerStartup {
    ready_file: Option<PathBuf>,
    ready_reported: bool,
    failure_reported: bool,
}

#[derive(Resource, Debug)]
struct ProcessMovementCheck {
    enabled: bool,
    ready_file: Option<PathBuf>,
    initial_poses: Vec<(PlayerId, Vec2, f32)>,
    initial_tick: Option<u64>,
    completed: bool,
}

#[derive(Resource, Debug)]
struct ProcessCombatCheck {
    enabled: bool,
    ready_file: Option<PathBuf>,
    client_ready_dir: Option<PathBuf>,
    report_file: Option<PathBuf>,
    run_id: String,
    expected_preset_id: Option<WeaponPresetId>,
    started_at: Instant,
    completed: bool,
}

#[derive(Resource, Debug)]
struct ProcessMatchCheck {
    enabled: bool,
    report_file: Option<PathBuf>,
    initial_match_id: Option<crate::matchplay::MatchId>,
    completed: bool,
}

impl FromWorld for ProcessMatchCheck {
    fn from_world(_: &mut World) -> Self {
        Self {
            enabled: env::var("BRAWLER_NETWORK_ASSERT_MATCH").as_deref() == Ok("1"),
            report_file: env::var_os("BRAWLER_NETWORK_MATCH_REPORT_FILE").map(PathBuf::from),
            initial_match_id: None,
            completed: false,
        }
    }
}

impl FromWorld for ProcessCombatCheck {
    fn from_world(_: &mut World) -> Self {
        Self {
            enabled: env::var("BRAWLER_NETWORK_ASSERT_COMBAT").as_deref() == Ok("1"),
            ready_file: env::var_os("BRAWLER_NETWORK_COMBAT_READY_FILE").map(PathBuf::from),
            client_ready_dir: env::var_os("BRAWLER_NETWORK_COMBAT_READY_DIR").map(PathBuf::from),
            report_file: env::var_os("BRAWLER_NETWORK_COMBAT_REPORT_FILE").map(PathBuf::from),
            run_id: env::var("BRAWLER_NETWORK_RUN_ID").unwrap_or_else(|_| "unknown".to_string()),
            expected_preset_id: env::var("BRAWLER_NETWORK_WEAPON_PRESET")
                .ok()
                .and_then(|value| value.parse::<u16>().ok())
                .map(WeaponPresetId),
            started_at: Instant::now(),
            completed: false,
        }
    }
}

impl FromWorld for ProcessMovementCheck {
    fn from_world(_: &mut World) -> Self {
        Self {
            enabled: env::var("BRAWLER_NETWORK_ASSERT_MOVEMENT").as_deref() == Ok("1"),
            ready_file: env::var_os("BRAWLER_NETWORK_MOVEMENT_READY_FILE").map(PathBuf::from),
            initial_poses: Vec::new(),
            initial_tick: None,
            completed: false,
        }
    }
}

impl FromWorld for ServerStartup {
    fn from_world(_: &mut World) -> Self {
        Self {
            ready_file: env::var_os("BRAWLER_SERVER_READY_FILE").map(PathBuf::from),
            ready_reported: false,
            failure_reported: false,
        }
    }
}

#[derive(Resource, Debug, PartialEq, Eq)]
pub struct NextSessionIds {
    next_player_id: u64,
    next_network_entity_id: u64,
}

impl Default for NextSessionIds {
    fn default() -> Self {
        Self {
            next_player_id: 1,
            next_network_entity_id: 1,
        }
    }
}

impl NextSessionIds {
    pub fn allocate(&mut self) -> Option<(PlayerId, NetworkEntityId)> {
        let player_id = self.next_player_id;
        let network_entity_id = self.next_network_entity_id;
        let next_player_id = player_id.checked_add(1)?;
        let next_network_entity_id = network_entity_id.checked_add(1)?;
        self.next_player_id = next_player_id;
        self.next_network_entity_id = next_network_entity_id;
        Some((PlayerId(player_id), NetworkEntityId(network_entity_id)))
    }
}

/// Marker proving that the network endpoint belongs to the dedicated server.
#[derive(Default, Resource, Debug, PartialEq, Eq)]
pub struct DedicatedServer;

/// Adds server startup diagnostics and clean scheduled execution.
pub struct DedicatedServerPlugin;

impl Plugin for DedicatedServerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DedicatedServer>()
            .add_systems(Startup, log_server_startup);
    }
}

fn log_server_startup(config: Option<Res<ServerNetworkConfig>>) {
    let bind_addr = config.map_or_else(
        || "unknown".to_string(),
        |config| config.bind_addr.to_string(),
    );
    info!(
        mode = "dedicated-server",
        version = VERSION,
        tick_hz = crate::timing::SIMULATION_TICK_HZ,
        bind = %bind_addr,
        "brawler dedicated server started"
    );
}

/// Installs the server Lightyear group, protocol, endpoint, and authoritative session systems.
pub struct ServerNetworkPlugin;

/// Classify a server error exit, append the bounded local failure record when the
/// `BRAWLER_FAILURE_REPORT` control selects one, and request the error exit. Every
/// server error path funnels through here so exit categories and failure records
/// cannot drift apart.
pub(super) fn record_server_failure(
    category: crate::diagnostics::FailureCategory,
    message: &str,
    diagnostics: Option<&crate::diagnostics::ProcessDiagnosticsSettings>,
    classification: &mut crate::diagnostics::ProcessExitClassification,
    app_exit: &mut MessageWriter<AppExit>,
) {
    classification.record_error_exit(category.into());
    if let Some(settings) = diagnostics
        && let Some(path) = settings.failure_record_path()
    {
        crate::diagnostics::write_failure_record(
            &path,
            &crate::diagnostics::ProcessFailureRecordV1::new(category, message),
        );
    }
    app_exit.write(AppExit::error());
}

impl Plugin for ServerNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FallbackErrorHandler(error))
            .init_resource::<ServerRoleResource>()
            .init_resource::<NextSessionIds>()
            .init_resource::<MatchLoadingGate>()
            .init_resource::<ServerShutdown>()
            .init_resource::<ServerStartup>()
            .init_resource::<ProcessMovementCheck>()
            .init_resource::<ProcessCombatCheck>()
            .init_resource::<ProcessMatchCheck>()
            .init_resource::<crate::builds::BuildTelemetry>()
            .init_resource::<crate::diagnostics::ProcessExitClassification>()
            .insert_resource(ReplicationMetadata::new(crate::timing::SIMULATION_TICK))
            .add_observer(configure_new_link)
            .add_systems(
                Startup,
                spawn_server_endpoint.after(MapStartupSet::Instantiate),
            )
            .add_systems(
                Update,
                (
                    observe_server_endpoint,
                    initialize_sessions,
                    process_client_hellos,
                    crate::builds::server::process_build_selection,
                    crate::abilities::cleanup_requested_sentries,
                    ApplyDeferred,
                    process_match_loading_messages,
                    process_match_commands,
                    ApplyDeferred,
                    enforce_session_deadlines,
                    disconnect_rejected_sessions,
                    verify_process_movement,
                    verify_process_combat,
                    verify_process_match,
                    exit_after_verification,
                )
                    .chain(),
            )
            .add_systems(
                FixedUpdate,
                commit_product_match_activation.before(crate::matchplay::MatchSet::Lifecycle),
            )
            .add_systems(
                FixedUpdate,
                detect_product_countdown_departure.after(crate::matchplay::MatchSet::Lifecycle),
            )
            .add_systems(
                Last,
                (forward_app_exit_to_server_stop, finish_server_shutdown)
                    .chain()
                    // Order before the terminal observation set so closeout observations and
                    // the final report see post-shutdown counts and the re-emitted exit.
                    .before(crate::diagnostics::TerminalObservationSet),
            )
            .add_plugins((
                ServerCombatPlugin,
                crate::abilities::ServerAbilityPlugin,
                RoutedWorkerPlugin,
            ));
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "observer triggers are delivered by value by the Bevy observer runtime"
)]
fn configure_new_link(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands
        .entity(trigger.entity)
        .insert((ReplicationSender, InputValidationState::default()));
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn spawn_server_endpoint(
    mut commands: Commands,
    config: Res<ServerNetworkConfig>,
    role: Res<ServerRoleResource>,
) -> Result {
    // A routed worker installs its already-connected endpoint during control-plane bootstrap.
    // The production graph remains shared with direct UDP, but must not accidentally bind a
    // second public socket in a worker process.
    if !matches!(role.0, ServerRole::DirectBaseline) {
        return Ok(());
    }
    // The role resource is initialized by ServerNetworkPlugin before Startup.  It is queried in
    // a separate system parameter below so this function's direct-UDP behavior remains unchanged
    // for the v1 baseline.
    spawn_server_endpoint_udp(&mut commands, &config)
}

fn spawn_server_endpoint_udp(commands: &mut Commands, config: &ServerNetworkConfig) -> Result {
    if config.transport != NetworkTransport::Udp {
        return Ok(());
    }
    let netcode_config =
        NetcodeConfig::default()
            .with_protocol_id(config.network_protocol_id)
            .with_key(DEVELOPMENT_PRIVATE_KEY)
            .with_client_timeout_secs(config.client_timeout.as_secs().try_into().map_err(
                |_| "server client timeout does not fit in Netcode's i32 seconds field",
            )?);
    let server = commands
        .spawn((
            NetcodeServer::new(netcode_config),
            LightyearServer::new(config.impairment_profile.receive_conditioner()),
            LocalAddr(config.bind_addr),
            ServerUdpIo::default(),
            Name::new("Brawler UDP server"),
        ))
        .id();
    commands.trigger(Start { entity: server });
    Ok(())
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime; the query declares this system's complete world view inline at its schedule boundary"
)]
fn observe_server_endpoint(
    mut startup: ResMut<ServerStartup>,
    ready_query: Query<
        (),
        (
            With<NetcodeServer>,
            With<Started>,
            With<Linked>,
            Or<(With<ServerUdpIo>, With<RoutedWorker>)>,
        ),
    >,
    failed_query: Query<
        (),
        (
            With<NetcodeServer>,
            With<Started>,
            Without<Linked>,
            Or<(With<ServerUdpIo>, With<RoutedWorker>)>,
        ),
    >,
    diagnostics: Option<Res<crate::diagnostics::ProcessDiagnosticsSettings>>,
    mut classification: ResMut<crate::diagnostics::ProcessExitClassification>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if startup.ready_reported || startup.failure_reported {
        return;
    }
    if failed_query.iter().next().is_some() {
        startup.failure_reported = true;
        error!("brawler server endpoint failed to bind or link");
        record_server_failure(
            crate::diagnostics::FailureCategory::EndpointStart,
            "server endpoint failed to bind or link",
            diagnostics.as_deref(),
            &mut classification,
            &mut app_exit,
        );
        return;
    }
    if ready_query.iter().next().is_none() {
        return;
    }
    if let Some(path) = startup.ready_file.clone() {
        if let Err(error) = fs::write(&path, b"ready\n") {
            startup.failure_reported = true;
            error!(path = %path.display(), ?error, "brawler server readiness signal failed");
            record_server_failure(
                crate::diagnostics::FailureCategory::EndpointStart,
                "server readiness signal write failed",
                diagnostics.as_deref(),
                &mut classification,
                &mut app_exit,
            );
            return;
        }
        info!(path = %path.display(), "brawler server readiness signal written");
    }
    startup.ready_reported = true;
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn initialize_sessions(
    mut commands: Commands,
    config: Res<ServerNetworkConfig>,
    time: Res<Time<Real>>,
    query: Query<(Entity, Has<Connected>, Option<&ServerSession>), With<LinkOf>>,
) {
    let now = time.elapsed();
    for (entity, connected, session) in query.iter() {
        if connected && session.is_none() {
            commands.entity(entity).insert(ServerSession {
                phase: ServerSessionPhase::AwaitingHello,
                deadline: now.saturating_add(config.handshake_timeout),
                last_outcome: None,
                disconnect_requested: false,
                last_selection_request: None,
                last_selection_outcome: None,
                last_selection_response: None,
                last_match_request: None,
                last_match_outcome: None,
            });
        }
    }
}

#[derive(SystemParam)]
struct ServerHelloContent<'w> {
    fighters: Res<'w, crate::combat::FighterDefinitions>,
    weapons: Res<'w, crate::combat::WeaponDefinitions>,
    builds: Res<'w, crate::builds::BuildCatalogResource>,
    weapon_catalog: Res<'w, WeaponCatalogResource>,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
fn process_client_hellos(
    mut commands: Commands,
    config: Res<ServerNetworkConfig>,
    role: Res<ServerRoleResource>,
    fingerprint: Res<ProtocolFingerprint>,
    content_fingerprint: Res<crate::content::GameplayContentFingerprint>,
    spawn_points: Res<SpawnPointCatalog>,
    resolved_map: Res<ResolvedMap>,
    movement_tuning: Res<MovementTuning>,
    lifecycle_rules: Res<MatchLifecycleRules>,
    content: ServerHelloContent,
    mut ids: ResMut<NextSessionIds>,
    mut diagnostics: Option<ResMut<crate::diagnostics::ProcessDiagnosticsState>>,
    mut receivers: Query<(
        Entity,
        &RemoteId,
        &mut MessageReceiver<MatchHello>,
        &mut MessageSender<MatchJoinOutcome>,
        &mut ServerSession,
        Option<&RoutedPeer>,
        Has<Disconnected>,
    )>,
    match_root: Query<&MatchState, With<MatchRoot>>,
    participants: Query<(&TeamId, &MatchParticipant, &Position), With<Fighter>>,
) {
    let mut active_count = participants.iter().count();
    let Ok(match_state) = match_root.single() else {
        return;
    };
    let mut team_counts = [0_u8; 2];
    let mut living_fighters = Vec::new();
    for (team, participant, position) in &participants {
        if participant.match_id == match_state.match_id && team.0 <= 1 {
            team_counts[usize::from(team.0)] = team_counts[usize::from(team.0)].saturating_add(1);
            living_fighters.push((*team, position.0));
        }
    }
    let mut ordered_receivers: Vec<_> = receivers.iter_mut().collect();
    ordered_receivers.sort_by_key(|(_, remote_id, _, _, _, _, _)| remote_id.0.to_bits());
    let mut admitted_client_ids: BTreeSet<u64> = ordered_receivers
        .iter()
        .filter_map(|(_, remote_id, _, _, session, _, disconnected)| {
            if !disconnected && matches!(session.phase, ServerSessionPhase::Active { .. }) {
                authenticated_netcode_id(remote_id)
            } else {
                None
            }
        })
        .collect();
    for (connection, remote_id, mut receiver, mut sender, mut session, routed_peer, disconnected) in
        ordered_receivers
    {
        if disconnected {
            receiver.receive().for_each(drop);
            continue;
        }
        let messages: Vec<_> = receiver.receive().collect();
        for hello in messages {
            match &session.phase {
                ServerSessionPhase::Active {
                    player_id,
                    network_entity_id,
                } => {
                    sender.send::<SessionChannel>(MatchJoinOutcome::Accepted {
                        player_id: *player_id,
                        network_entity_id: *network_entity_id,
                    });
                }
                ServerSessionPhase::Rejected => {
                    if let Some(outcome) = &session.last_outcome {
                        sender.send::<SessionChannel>(outcome.clone());
                    }
                }
                ServerSessionPhase::AwaitingHello => {
                    let outcome = if hello.protocol_version
                        != crate::protocol::SUPPORTED_PROTOCOL_VERSION
                    {
                        MatchJoinOutcome::Rejected {
                            reason: MatchJoinRejection::ProtocolVersionMismatch,
                        }
                    } else if hello.build_version != VERSION {
                        MatchJoinOutcome::Rejected {
                            reason: MatchJoinRejection::BuildVersionMismatch,
                        }
                    } else if hello.registry_fingerprint != fingerprint.0 {
                        MatchJoinOutcome::Rejected {
                            reason: MatchJoinRejection::RegistryMismatch,
                        }
                    } else if hello.content_fingerprint != *content_fingerprint {
                        MatchJoinOutcome::Rejected {
                            reason: MatchJoinRejection::ContentMismatch,
                        }
                    } else if matches!(role.0, ServerRole::MatchWorker(_))
                        && authenticated_netcode_id(remote_id).is_none_or(|client_id| {
                            admit_manifest_client(
                                role.manifest().expect("match worker has a manifest"),
                                client_id,
                                routed_peer.map(|peer| peer.peer_id),
                                &admitted_client_ids,
                            )
                            .is_err()
                        })
                    {
                        MatchJoinOutcome::Rejected {
                            reason: MatchJoinRejection::MatchFull,
                        }
                    } else if !matches!(match_state.phase, MatchPhase::Waiting) {
                        MatchJoinOutcome::Rejected {
                            reason: MatchJoinRejection::MatchInProgress,
                        }
                    } else if active_count >= config.max_clients {
                        MatchJoinOutcome::Rejected {
                            reason: MatchJoinRejection::ServerFull,
                        }
                    } else if !matches!(role.0, ServerRole::MatchWorker(_))
                        && assigned_team(team_counts, lifecycle_rules.maximum_participants_per_team)
                            .is_none()
                    {
                        MatchJoinOutcome::Rejected {
                            reason: MatchJoinRejection::MatchFull,
                        }
                    } else {
                        match ids.allocate() {
                            Some((baseline_player_id, baseline_network_entity_id)) => {
                                let worker_participant = role
                                    .manifest()
                                    .and_then(|manifest| {
                                        authenticated_netcode_id(remote_id).and_then(|client_id| {
                                            admit_manifest_client(
                                                manifest,
                                                client_id,
                                                routed_peer.map(|peer| peer.peer_id),
                                                &admitted_client_ids,
                                            )
                                            .ok()
                                        })
                                    })
                                    .copied();
                                let accepted = MatchJoinOutcome::Accepted {
                                    player_id: worker_participant
                                        .map_or(baseline_player_id, |participant| {
                                            PlayerId(participant.player_id.get())
                                        }),
                                    network_entity_id: baseline_network_entity_id,
                                };
                                let (assigned_team, manifest_loadout) = if let Some(participant) =
                                    worker_participant.as_ref()
                                {
                                    let team = TeamId(participant.team);
                                    let fighter = content
                                        .fighters
                                        .get(crate::combat::STANDARD_FIGHTER_DEFINITION)
                                        .expect("validated standard fighter definition");
                                    let snapshot = crate::profiles::MatchBuildSnapshotV3::decode(
                                        &participant.build_snapshot,
                                    )
                                    .expect("validated manifest build snapshot");
                                    let loadout = snapshot
                                        .resolve(
                                            &content.builds.0,
                                            &content.weapon_catalog.0,
                                            fighter,
                                        )
                                        .expect("validated manifest build resolution");
                                    (team, Some(loadout))
                                } else {
                                    let assigned_team = assigned_team(
                                        team_counts,
                                        lifecycle_rules.maximum_participants_per_team,
                                    )
                                    .expect("capacity was checked before identifier allocation");
                                    (assigned_team, None)
                                };
                                let player_id = worker_participant
                                    .map_or(baseline_player_id, |participant| {
                                        PlayerId(participant.player_id.get())
                                    });
                                let network_entity_id = baseline_network_entity_id;
                                let display_name = worker_participant.map_or_else(
                                    || crate::lobby::generated_display_name(player_id.0),
                                    |participant| participant.display_name.as_str().to_string(),
                                );
                                let candidates = spawn_points
                                    .0
                                    .get(&assigned_team.0)
                                    .into_iter()
                                    .flatten()
                                    .map(|point| SpawnCandidate {
                                        id: point.spawn_point_id,
                                        position: point.position,
                                        facing: point.facing,
                                    })
                                    .collect();
                                let spawn_point = select_spawn(
                                    candidates,
                                    &living_fighters,
                                    assigned_team,
                                    movement_tuning.radius * 2.0 + movement_tuning.skin_width,
                                    match_state.match_id,
                                    player_id,
                                    0,
                                )
                                .expect("validated map has a finite spawn for each Wipeout team");
                                let spawn_position = spawn_point.position;
                                let spawn_facing = spawn_point.facing;
                                let (fighter_definition, team, mut health, mut weapon) =
                                    default_fighter_runtime(
                                        assigned_team,
                                        &content.fighters,
                                        &content.weapons,
                                    );
                                if let Some(loadout) = manifest_loadout.as_ref() {
                                    health = CurrentHealth(loadout.fighter_stats.maximum_health);
                                    weapon = WeaponState {
                                        ammo: loadout.primary_weapon.recipe.economy.capacity(),
                                        phase: WeaponPhase::Ready,
                                    };
                                }
                                let fighter_entity = commands
                                    .spawn((
                                        Fighter,
                                        player_id,
                                        network_entity_id,
                                        PlaceholderState {
                                            spawn_slot: u64::from(spawn_point.id.0),
                                        },
                                        fighter_definition,
                                        team,
                                        health,
                                        SelectingBuild,
                                        ActiveEffects::default(),
                                        AuthoritativeTick::default(),
                                        SpawnState {
                                            position: spawn_position,
                                            facing: spawn_facing,
                                        },
                                        Position::from_xy(spawn_position.x, spawn_position.y),
                                        Rotation::radians(spawn_facing),
                                        LinearVelocity::default(),
                                        AngularVelocity::default(),
                                    ))
                                    .id();
                                commands.entity(fighter_entity).insert((
                                    crate::matchplay::FighterDisplayName(display_name),
                                    MatchParticipant {
                                        match_id: match_state.match_id,
                                        ready: false,
                                        restart_ready: false,
                                    },
                                    MatchMember(match_state.match_id),
                                    SpawnAssignment {
                                        map_instance_id: resolved_map.snapshot.identity.instance_id,
                                        spawn_point_id: spawn_point.id,
                                    },
                                    Collider::circle(movement_tuning.radius),
                                    RigidBody::Kinematic,
                                    CustomPositionIntegration,
                                    CollisionLayers::new(
                                        crate::movement::FIGHTER_LAYER,
                                        avian2d::prelude::LayerMask::NONE,
                                    ),
                                    InputFreshness::default(),
                                    (
                                        Replicate::to_clients(NetworkTarget::All),
                                        InterpolationTarget::to_clients(NetworkTarget::All),
                                    ),
                                    ControlledBy {
                                        owner: connection,
                                        lifetime: Lifetime::SessionBased,
                                    },
                                ));
                                if let Some(loadout) = manifest_loadout {
                                    commands
                                        .entity(fighter_entity)
                                        .insert((
                                            loadout.identity,
                                            loadout,
                                            AbilityState::default(),
                                            PassiveRuntimeState::default(),
                                            weapon,
                                            ActiveEffects::default(),
                                        ))
                                        .remove::<SelectingBuild>();
                                }
                                session.phase = ServerSessionPhase::Active {
                                    player_id,
                                    network_entity_id,
                                };
                                active_count += 1;
                                if let Some(client_id) = authenticated_netcode_id(remote_id) {
                                    admitted_client_ids.insert(client_id);
                                }
                                team_counts[usize::from(assigned_team.0)] =
                                    team_counts[usize::from(assigned_team.0)].saturating_add(1);
                                living_fighters.push((assigned_team, spawn_position));
                                accepted
                            }
                            None => MatchJoinOutcome::Rejected {
                                reason: MatchJoinRejection::IdentifierExhausted,
                            },
                        }
                    };
                    let rejected = matches!(outcome, MatchJoinOutcome::Rejected { .. });
                    sender.send::<SessionChannel>(outcome.clone());
                    session.last_outcome = Some(outcome);
                    if rejected {
                        session.phase = ServerSessionPhase::Rejected;
                        if let Some(state) = diagnostics.as_deref_mut() {
                            state.record_rejected_connection();
                        }
                    }
                }
            }
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn process_match_loading_messages(
    time: Res<Time<Real>>,
    role: Res<ServerRoleResource>,
    match_root: Query<&MatchState, With<MatchRoot>>,
    mut gate: ResMut<MatchLoadingGate>,
    mut control_outbox: Option<ResMut<worker::MatchControlOutbox>>,
    mut exit: MessageWriter<AppExit>,
    mut sessions: Query<(
        &RemoteId,
        &mut MessageReceiver<MatchLoadingClientMessage>,
        &mut MessageSender<MatchLoadingServerMessage>,
        &mut MessageSender<MatchLoadingStatus>,
        &mut ServerSession,
        Has<Disconnected>,
    )>,
) {
    let Some(manifest) = role.manifest() else {
        for (_, mut receiver, _, _, _, _) in &mut sessions {
            receiver.receive().for_each(drop);
        }
        return;
    };
    let Ok(match_state) = match_root.single() else {
        return;
    };
    let deadline = *gate
        .deadline
        .get_or_insert_with(|| time.elapsed().saturating_add(Duration::from_secs(20)));
    if time.elapsed() >= deadline && !gate.readiness_committed {
        gate.terminal_failure = true;
        gate.cancelled = true;
    }
    if gate.terminal_failure && gate.terminal_exit_deadline.is_none() {
        gate.terminal_exit_deadline = Some(time.elapsed().saturating_add(Duration::from_secs(2)));
    }
    let mut cancellation_delivered = false;
    let mut connected = 0_u8;
    for (remote_id, mut receiver, mut sender, _, session, disconnected) in &mut sessions {
        if disconnected || !matches!(session.phase, ServerSessionPhase::Active { .. }) {
            receiver.receive().for_each(drop);
            continue;
        }
        let Some(client_id) = authenticated_netcode_id(remote_id) else {
            receiver.receive().for_each(drop);
            continue;
        };
        let is_participant = manifest
            .participants
            .iter()
            .any(|participant| participant.netcode_client_id.get() == client_id);
        if is_participant {
            connected = connected.saturating_add(1);
        }
        if gate.terminal_failure && gate.terminal_notified.insert(client_id) {
            let response = MatchLoadingServerMessage {
                request_id: manifest.request_id.get(),
                allocation_id: manifest.allocation_id.get(),
                match_id: manifest.match_id.get(),
                outcome: MatchLoadingServerOutcome::TerminalFailure,
            };
            sender.send::<SessionChannel>(response);
        }
        if gate.cancelling_client == Some(client_id)
            && let Some(outcome) = gate.cancellation_outcome
        {
            let response = MatchLoadingServerMessage {
                request_id: manifest.request_id.get(),
                allocation_id: manifest.allocation_id.get(),
                match_id: manifest.match_id.get(),
                outcome,
            };
            sender.send::<SessionChannel>(response);
            cancellation_delivered = true;
        }
        for message in receiver.receive() {
            if message.request_id != manifest.request_id.get()
                || message.allocation_id != manifest.allocation_id.get()
                || message.match_id != manifest.match_id.get()
                || !is_participant
            {
                continue;
            }
            let outcome = match message.action {
                MatchLoadingClientAction::Ready
                    if matches!(match_state.phase, MatchPhase::Waiting) && !gate.cancelled =>
                {
                    gate.checked_in.insert(client_id);
                    continue;
                }
                MatchLoadingClientAction::CancelMatchStart
                    if matches!(match_state.phase, MatchPhase::Waiting)
                        && !gate.readiness_committed =>
                {
                    gate.cancelled = true;
                    gate.checked_in.clear();
                    gate.cancelling_client = Some(client_id);
                    gate.cancellation_outcome =
                        Some(MatchLoadingServerOutcome::CancellationAccepted);
                    gate.terminal_failure = true;
                    continue;
                }
                MatchLoadingClientAction::CancelMatchStart => {
                    MatchLoadingServerOutcome::CancellationTooLate
                }
                MatchLoadingClientAction::Ready => continue,
            };
            let response = MatchLoadingServerMessage {
                request_id: manifest.request_id.get(),
                allocation_id: manifest.allocation_id.get(),
                match_id: manifest.match_id.get(),
                outcome,
            };
            sender.send::<SessionChannel>(response);
        }
    }
    if cancellation_delivered {
        gate.cancellation_outcome = None;
    }
    let expected = u8::try_from(manifest.participants.len()).unwrap_or(u8::MAX);
    let checked_in = u8::try_from(gate.checked_in.len()).unwrap_or(u8::MAX);
    let status_phase = if connected < expected {
        crate::lobby::MatchLoadingPhase::Connecting
    } else if checked_in < expected {
        crate::lobby::MatchLoadingPhase::Synchronizing
    } else {
        crate::lobby::MatchLoadingPhase::WaitingForPlayers
    };
    let status_key = (connected, checked_in, status_phase);
    if gate.last_status != Some(status_key) {
        gate.last_status = Some(status_key);
        gate.status_revision = gate.status_revision.saturating_add(1).max(1);
        let status = MatchLoadingStatus {
            generation: 1,
            revision: gate.status_revision,
            request_id: manifest.request_id.get(),
            allocation_id: manifest.allocation_id.get(),
            match_id: manifest.match_id.get(),
            phase: status_phase,
            expected,
            connected,
            checked_in,
        };
        for (_, _, _, mut status_sender, _, disconnected) in &mut sessions {
            if !disconnected {
                status_sender.send::<QueueSnapshotChannel>(status);
            }
        }
    }
    if gate.cancelled
        && !gate.cancel_emitted
        && let Some(outbox) = control_outbox.as_mut()
    {
        outbox.cancel(brawler_routing::ActivationBody {
            request_id: manifest.request_id,
            allocation_id: manifest.allocation_id,
            match_id: manifest.match_id,
        });
        gate.cancel_emitted = true;
    }
    if matches!(match_state.phase, MatchPhase::Active { .. })
        && !gate.activated_emitted
        && let Some(outbox) = control_outbox.as_mut()
    {
        outbox.activated(brawler_routing::ActivationBody {
            request_id: manifest.request_id,
            allocation_id: manifest.allocation_id,
            match_id: manifest.match_id,
        });
        gate.activated_emitted = true;
    }
    if gate.terminal_failure
        && !gate.terminal_exit_requested
        && gate
            .terminal_exit_deadline
            .is_some_and(|deadline| time.elapsed() >= deadline)
    {
        gate.terminal_exit_requested = true;
        exit.write(AppExit::Success);
    }
}

#[allow(clippy::needless_pass_by_value)]
fn commit_product_match_activation(
    role: Res<ServerRoleResource>,
    mut gate: ResMut<MatchLoadingGate>,
    roots: Query<&MatchState, With<MatchRoot>>,
    mut fighters: Query<&mut MatchParticipant, With<Fighter>>,
) {
    if role.manifest().is_none()
        || gate.cancelled
        || gate.readiness_committed
        || gate.checked_in.len()
            != role
                .manifest()
                .map_or(0, |manifest| manifest.participants.len())
    {
        return;
    }
    let Ok(state) = roots.single() else {
        return;
    };
    if !matches!(state.phase, MatchPhase::Waiting) {
        return;
    }
    for mut participant in &mut fighters {
        if participant.match_id == state.match_id {
            participant.ready = true;
        }
    }
    gate.readiness_committed = true;
}

#[allow(clippy::needless_pass_by_value)]
fn detect_product_countdown_departure(
    role: Res<ServerRoleResource>,
    roots: Query<&MatchState, With<MatchRoot>>,
    mut gate: ResMut<MatchLoadingGate>,
    mut outbox: Option<ResMut<worker::MatchControlOutbox>>,
) {
    let Some(manifest) = role.manifest() else {
        return;
    };
    let Ok(state) = roots.single() else {
        return;
    };
    if matches!(state.phase, MatchPhase::Countdown { .. }) {
        gate.countdown_observed = true;
        return;
    }
    if failed_initial_countdown(&gate, state.phase)
        && !gate.start_failure_emitted
        && let Some(outbox) = outbox.as_mut()
    {
        outbox.start_failed(brawler_routing::ActivationBody {
            request_id: manifest.request_id,
            allocation_id: manifest.allocation_id,
            match_id: manifest.match_id,
        });
        gate.start_failure_emitted = true;
        gate.terminal_failure = true;
    }
}

/// A return to Waiting is a routed start failure only before the worker has announced its first
/// activation. Later gameplay epochs deliberately pass through Waiting during an authoritative
/// Balance Lab reset and must keep the existing worker and client session alive.
fn failed_initial_countdown(gate: &MatchLoadingGate, phase: MatchPhase) -> bool {
    gate.countdown_observed && !gate.activated_emitted && matches!(phase, MatchPhase::Waiting)
}

#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
fn process_match_commands(
    tick: Res<crate::timing::SimulationTick>,
    match_root: Query<&MatchState, With<MatchRoot>>,
    mut sessions: Query<(
        Entity,
        &mut MessageReceiver<MatchCommandRequest>,
        &mut MessageSender<MatchCommandOutcome>,
        &mut ServerSession,
        Has<Disconnected>,
    )>,
    mut fighters: Query<
        (
            &ControlledBy,
            &mut MatchParticipant,
            Option<&crate::builds::ResolvedMatchLoadout>,
            Option<&SelectingBuild>,
        ),
        With<Fighter>,
    >,
) {
    let Ok(match_state) = match_root.single() else {
        return;
    };
    for (connection, mut receiver, mut sender, mut session, disconnected) in &mut sessions {
        if disconnected {
            receiver.receive().for_each(drop);
            continue;
        }
        let requests: Vec<_> = receiver.receive().collect();
        for request in requests {
            if session
                .last_match_request
                .is_some_and(|previous| request.request_id < previous.request_id)
            {
                sender.send::<SessionChannel>(MatchCommandOutcome {
                    request_id: request.request_id,
                    match_id: request.match_id,
                    decision: MatchCommandDecision::Stale,
                });
                continue;
            }
            if session
                .last_match_request
                .is_some_and(|previous| request.request_id == previous.request_id)
            {
                if let Some(outcome) = session.last_match_outcome {
                    sender.send::<SessionChannel>(outcome);
                }
                continue;
            }
            let participant = fighters
                .iter_mut()
                .find(|(controlled, _, _, _)| controlled.owner == connection);
            let decision = if request.match_id != match_state.match_id {
                MatchCommandDecision::WrongMatch
            } else if let Some((_, mut participant, selected, selecting)) = participant {
                match (request.command, match_state.phase) {
                    (MatchCommand::SetReady(value), MatchPhase::Waiting)
                        if selected.is_some() && selecting.is_none() =>
                    {
                        participant.ready = value;
                        MatchCommandDecision::Accepted
                    }
                    (MatchCommand::SetReady(value), MatchPhase::Countdown { .. })
                        if selected.is_some() && selecting.is_none() =>
                    {
                        participant.ready = value;
                        MatchCommandDecision::Accepted
                    }
                    (
                        MatchCommand::ReadyForRestart,
                        MatchPhase::Completed {
                            restart_unlocked_at_tick,
                            ..
                        },
                    ) if tick.0 < restart_unlocked_at_tick => MatchCommandDecision::Locked,
                    (MatchCommand::ReadyForRestart, MatchPhase::Completed { .. }) => {
                        participant.restart_ready = true;
                        MatchCommandDecision::Accepted
                    }
                    _ => MatchCommandDecision::WrongPhase,
                }
            } else {
                MatchCommandDecision::NotParticipant
            };
            let outcome = MatchCommandOutcome {
                request_id: request.request_id,
                match_id: request.match_id,
                decision,
            };
            session.last_match_request = Some(request);
            session.last_match_outcome = Some(outcome);
            sender.send::<SessionChannel>(outcome);
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn enforce_session_deadlines(
    time: Res<Time<Real>>,
    mut diagnostics: Option<ResMut<crate::diagnostics::ProcessDiagnosticsState>>,
    mut query: Query<(Entity, &mut ServerSession, Has<Disconnected>), With<LinkOf>>,
) {
    let now = time.elapsed();
    for (entity, mut session, disconnected) in query.iter_mut() {
        if !disconnected
            && matches!(session.phase, ServerSessionPhase::AwaitingHello)
            && now >= session.deadline
        {
            session.phase = ServerSessionPhase::Rejected;
            session.last_outcome = Some(MatchJoinOutcome::Rejected {
                reason: MatchJoinRejection::HandshakeTimeout,
            });
            if let Some(state) = diagnostics.as_deref_mut() {
                state.record_rejected_connection();
            }
            warn!(?entity, "brawler server handshake timed out");
        }
    }
}

/// Deterministic graceful exit for measurement runs: once every enabled verification check has
/// completed, request a clean shutdown so terminal evidence (closeout reports, ordered stop)
/// is produced instead of the launcher terminating the process.
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn exit_after_verification(
    tick: Res<crate::timing::SimulationTick>,
    movement: Res<ProcessMovementCheck>,
    combat: Res<ProcessCombatCheck>,
    match_check: Res<ProcessMatchCheck>,
    mut app_exit: MessageWriter<AppExit>,
) {
    // Idle-endpoint baseline control: exit cleanly after a bounded tick window even with
    // no verification checks enabled, so the closeout report records cold-start and idle
    // cost. A development verification control, not a v2 process contract.
    if let Ok(idle_ticks) = env::var("BRAWLER_SERVER_EXIT_AFTER_TICKS")
        && let Ok(idle_ticks) = idle_ticks.parse::<u64>()
        && tick.0 >= idle_ticks
    {
        info!("brawler server exiting after the configured idle tick window");
        app_exit.write(AppExit::Success);
        return;
    }
    if env::var("BRAWLER_SERVER_EXIT_AFTER_VERIFICATION").as_deref() != Ok("1") {
        return;
    }
    let checks = [
        (movement.enabled, movement.completed),
        (combat.enabled, combat.completed),
        (match_check.enabled, match_check.completed),
    ];
    let any_enabled = checks.iter().any(|(enabled, _)| *enabled);
    let all_done = checks.iter().all(|(enabled, done)| !enabled || *done);
    // Some process profiles intentionally keep clients running after the authoritative
    // assertion completes so each endpoint can produce its own clean closeout. The
    // launcher may therefore hold the server until a bounded minimum simulation tick.
    let minimum_tick = env::var("BRAWLER_SERVER_EXIT_AFTER_VERIFICATION_MIN_TICKS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    if any_enabled && all_done && tick.0 >= minimum_tick {
        info!("brawler server exiting after completed process verification");
        app_exit.write(AppExit::Success);
    }
}

fn disconnect_rejected_sessions(
    mut commands: Commands,
    mut query: Query<(Entity, &mut ServerSession), With<LinkOf>>,
) {
    for (entity, mut session) in query.iter_mut() {
        if matches!(session.phase, ServerSessionPhase::Rejected) && !session.disconnect_requested {
            session.disconnect_requested = true;
            continue;
        }
        if matches!(session.phase, ServerSessionPhase::Rejected) {
            // Server-side `Disconnect` is a client/host trigger in Lightyear 0.29. A
            // rejected link has no authoritative entity to preserve, so mark the lifecycle
            // outcome and remove the link after its prior-frame rejection has flushed.
            commands
                .entity(entity)
                .insert(Disconnected::default())
                .despawn();
        }
    }
}

fn forward_app_exit_to_server_stop(
    mut app_exits: ResMut<Messages<AppExit>>,
    mut shutdown: ResMut<ServerShutdown>,
    mut commands: Commands,
    query: Query<(Entity, Option<&Stopped>), With<NetcodeServer>>,
) {
    if shutdown.requested_exit.is_some() {
        return;
    }
    let exits: Vec<_> = app_exits.drain().collect();
    let Some(exit) = exits
        .iter()
        .find(|exit| exit.is_error())
        .or_else(|| exits.first())
        .cloned()
    else {
        return;
    };
    shutdown.requested_exit = Some(exit);
    for (entity, stopped) in query.iter() {
        if stopped.is_none() {
            commands.trigger(Stop { entity });
        }
    }
}

fn finish_server_shutdown(
    mut app_exits: ResMut<Messages<AppExit>>,
    mut shutdown: ResMut<ServerShutdown>,
    query: Query<(), (With<NetcodeServer>, With<Stopped>)>,
) {
    if query.iter().next().is_some()
        && let Some(exit) = shutdown.requested_exit.take()
    {
        app_exits.write(exit);
    }
}

/// Build the production headless dedicated server application.
pub fn build_app_with_config(config: ServerNetworkConfig) -> App {
    build_authoritative_app(config, true, None)
}

/// Build an authoritative worker whose shutdown is owned by supervisor control IPC rather than
/// process-local terminal signal handling.
fn build_match_worker_graph(config: ServerNetworkConfig, players_per_team: u8) -> App {
    build_authoritative_app(config, false, Some(players_per_team))
}

fn build_authoritative_app(
    config: ServerNetworkConfig,
    install_terminal_handler: bool,
    exact_players_per_team: Option<u8>,
) -> App {
    let mut app = App::new();
    let mut lifecycle = match_lifecycle_rules_for_profile(config.match_rules_profile);
    if let Some(ticks) = config.match_duration_ticks {
        lifecycle.active_limit_ticks = ticks;
    }
    if let Some(ticks) = config.match_countdown_ticks {
        lifecycle.countdown_ticks = ticks;
    }
    if let Some(ticks) = config.match_respawn_ticks {
        lifecycle.respawn_delay_ticks = ticks;
    }
    if let Some(players) = exact_players_per_team {
        lifecycle.minimum_participants_per_team = players;
        lifecycle.maximum_participants_per_team = players;
        lifecycle = lifecycle
            .validate()
            .expect("validated manifest topology fits lifecycle bounds");
    }
    app.insert_resource(config)
        .insert_resource(lifecycle)
        .add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
            crate::timing::SIMULATION_TICK,
        )))
        .add_plugins(StatesPlugin);
    if install_terminal_handler {
        app.add_plugins(TerminalCtrlCHandlerPlugin);
    }
    app.add_plugins(LogPlugin::default())
        .add_plugins(ServerPlugins {
            tick_duration: crate::timing::SIMULATION_TICK,
        })
        .add_plugins((
            GameplayPlugin,
            ProtocolPlugin,
            AvianNetworkPlugin,
            AuthoritativeMapPlugin,
            AuthoritativeMovementPlugin,
            ServerNetworkPlugin,
            DedicatedServerPlugin,
            AuthoritativeMatchPlugin,
            crate::diagnostics::ProcessDiagnosticsPlugin,
            practice::InertPracticeBotPlugin,
        ));
    #[cfg(feature = "balance-lab")]
    app.add_plugins(balance_lab::BalanceLabPlugin);
    if let Some(path) =
        crate::diagnostics::ProcessDiagnosticsSettings::default().failure_record_path()
    {
        crate::diagnostics::install_panic_failure_hook(path);
    }
    install_server_game_mode(&mut app);
    app
}

/// Install the configured mode's rules and mode plugin. The production composition reads the
/// validated configuration exactly once during app construction; no runtime hot-swap occurs.
fn install_server_game_mode(app: &mut App) {
    let mode = app.world().resource::<ServerNetworkConfig>().game_mode;
    let profile = app
        .world()
        .resource::<ServerNetworkConfig>()
        .match_rules_profile;
    let objective_target = app
        .world()
        .resource::<ServerNetworkConfig>()
        .match_objective_target;
    match mode {
        GameMode::Wipeout => {
            app.insert_resource(MatchModeSetup {
                mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
                rules_revision: WIPEOUT_RULES_REVISION,
            })
            .insert_resource(objective_target.map_or_else(
                || wipeout_rules_for_profile(profile),
                |target_score| {
                    WipeoutRules { target_score }
                        .validate()
                        .expect("validated manifest Wipeout objective")
                },
            ))
            .insert_resource(ServerMapSelection {
                preset_id: CROSSROADS_PRESET,
            })
            .add_plugins(WipeoutModePlugin);
        }
        GameMode::HotZone => {
            let rules = objective_target.map_or_else(
                || crate::matchplay::hot_zone_rules_for_profile(profile),
                |target_progress_ticks| crate::matchplay::HotZoneRules {
                    target_progress_ticks,
                },
            );
            let rules = rules
                .validate_with(app.world().resource::<MatchLifecycleRules>())
                .expect("validated manifest Hot Zone objective");
            app.insert_resource(crate::matchplay::hot_zone_setup_for_composition())
                .insert_resource(rules)
                .insert_resource(ServerMapSelection {
                    preset_id: CROSSROADS_HOT_ZONE_PRESET,
                })
                .add_plugins(crate::matchplay::HotZoneModePlugin);
        }
    }
}

/// The required scenario checkpoints for one asserted weapon preset. Public for the
/// `validate-closeout` terminal gate, which re-derives the declared checkpoint count
/// from the asserted preset so a launcher-side declaration cannot drift silently.
#[must_use]
pub fn required_process_checkpoints(preset_id: WeaponPresetId) -> &'static [&'static str] {
    verification::required_process_checkpoints(preset_id)
}

/// Common lifecycle rules for one rules profile; deadlines shorten without changing semantics.
#[must_use]
pub fn match_lifecycle_rules_for_profile(profile: MatchRulesProfile) -> MatchLifecycleRules {
    match profile {
        MatchRulesProfile::Production => MatchLifecycleRules::default(),
        MatchRulesProfile::ProcessVerification => MatchLifecycleRules {
            // Keep the verification profile's minimum compatible with the production
            // lifecycle so a two-client routed 1v1 can reach its authoritative deadline.  The
            // profile still supports the production 2v2 capacity through its unchanged maximum;
            // the four-client process-match harness therefore retains its 2v2 coverage.
            minimum_participants_per_team: 1,
            countdown_ticks: 30,
            active_limit_ticks: 3_600,
            respawn_delay_ticks: 30,
            spawn_protection_ticks: 10,
            completed_input_lock_ticks: 10,
            ..MatchLifecycleRules::default()
        },
    }
    .validate()
    .expect("configured match lifecycle rules profile must be valid")
}

fn wipeout_rules_for_profile(profile: MatchRulesProfile) -> WipeoutRules {
    match profile {
        MatchRulesProfile::Production => WipeoutRules::default(),
        MatchRulesProfile::ProcessVerification => WipeoutRules { target_score: 10 },
    }
    .validate()
    .expect("configured Wipeout rules profile must be valid")
}

/// Build the default production server application.
pub fn build_app() -> App {
    build_app_with_config(ServerNetworkConfig::default())
}

/// Request a graceful server stop through Lightyear's lifecycle.
pub fn request_stop(world: &mut World, server: Entity) {
    world.trigger(Stop { entity: server });
}

#[cfg(feature = "network-test")]
pub fn spawn_crossbeam_server(world: &mut World, config: &ServerNetworkConfig) -> Entity {
    let timeout_secs = i32::try_from(config.client_timeout.as_secs())
        .expect("test client timeout must fit in Netcode's i32 seconds field");
    let netcode_config = NetcodeConfig::default()
        .with_protocol_id(config.network_protocol_id)
        .with_key(DEVELOPMENT_PRIVATE_KEY)
        .with_client_timeout_secs(timeout_secs);
    let netcode_config = NetcodeConfig {
        server_addr_check: false,
        ..netcode_config
    };
    let server = world
        .spawn((
            NetcodeServer::new(netcode_config),
            LightyearServer::new(config.impairment_profile.receive_conditioner()),
        ))
        .id();
    world.flush();
    world.trigger(Start { entity: server });
    world.flush();
    server
}

#[cfg(feature = "network-test")]
pub fn spawn_crossbeam_link(
    world: &mut World,
    server: Entity,
    io: lightyear::crossbeam::CrossbeamIo,
) -> Entity {
    let link = world.spawn((LinkOf { server }, io)).id();
    world.flush();
    world.trigger(lightyear::link::LinkStart { entity: link });
    world.flush();
    link
}

#[cfg(test)]
mod tests;
