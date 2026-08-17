//! Dedicated-server composition and authoritative network-session concerns.
#![allow(clippy::needless_pass_by_value, clippy::type_complexity)]

use crate::{
    VERSION,
    combat::{
        ActiveEffects, AuthoritativeTick, CombatCue, CombatEvidenceSnapshots, CombatStateSnapshot,
        CombatTelemetry, ResolvedWeapon, SelectedBuild, SelectingBuild, ServerCombatPlugin,
        SpawnState, TeamId, TestDummy, WeaponCatalogResource, WeaponPresetId, WeaponTelemetry,
        WeaponTelemetryKey, decode_combat_cue, default_fighter_runtime, encode_state_snapshot,
    },
    config::{GameMode, MatchRulesProfile, NetworkTransport, ServerNetworkConfig},
    gameplay::GameplayPlugin,
    map::{
        AuthoritativeMapPlugin, BUILT_IN_MAP_PRESET, HOT_ZONE_MAP_PRESET, MapStartupSet,
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
        BuildSelectionOutcome, BuildSelectionRequest, ClientHello, DEVELOPMENT_PRIVATE_KEY,
        Fighter, JoinOutcome, JoinRejection, MatchCommand, MatchCommandDecision,
        MatchCommandOutcome, MatchCommandRequest, NetworkEntityId, PlaceholderState, PlayerId,
        ProtocolFingerprint, ProtocolPlugin, SessionChannel,
    },
};
use avian2d::prelude::{
    AngularVelocity, Collider, CollisionLayers, CustomPositionIntegration, LinearVelocity,
    Position, RigidBody, Rotation,
};
use bevy::{
    app::{ScheduleRunnerPlugin, TerminalCtrlCHandlerPlugin},
    ecs::error::{FallbackErrorHandler, error},
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
use std::{collections::BTreeMap, env, fmt::Write as _, fs, path::PathBuf, time::Instant};

mod verification;
#[cfg(test)]
#[allow(clippy::wildcard_imports)]
use verification::*;
use verification::{
    verify_process_combat, verify_process_match, verify_process_movement, verify_process_terrain,
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
    pub last_outcome: Option<JoinOutcome>,
    pub disconnect_requested: bool,
    pub last_selection_request: Option<BuildSelectionRequest>,
    pub last_selection_outcome: Option<BuildSelectionOutcome>,
    pub last_selection_response: Option<BuildSelectionOutcome>,
    pub last_match_request: Option<MatchCommandRequest>,
    pub last_match_outcome: Option<MatchCommandOutcome>,
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

#[derive(Resource, Debug)]
struct ProcessTerrainCheck {
    enabled: bool,
    ready_file: Option<PathBuf>,
    report_file: Option<PathBuf>,
    target_revision: u64,
    window_ticks: u64,
    initial_observed_tick: Option<u64>,
    initial_cells: Option<u32>,
    peak_revision: u64,
    peak_destroyed: bool,
    completed: bool,
}

impl FromWorld for ProcessTerrainCheck {
    fn from_world(_: &mut World) -> Self {
        Self {
            enabled: env::var("BRAWLER_NETWORK_ASSERT_TERRAIN").as_deref() == Ok("1"),
            ready_file: env::var_os("BRAWLER_NETWORK_TERRAIN_READY_FILE").map(PathBuf::from),
            report_file: env::var_os("BRAWLER_NETWORK_TERRAIN_REPORT_FILE").map(PathBuf::from),
            target_revision: env::var("BRAWLER_NETWORK_TERRAIN_TARGET_REVISION")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(1),
            window_ticks: env::var("BRAWLER_NETWORK_TERRAIN_WINDOW_TICKS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(1800),
            initial_observed_tick: None,
            initial_cells: None,
            peak_revision: 0,
            peak_destroyed: false,
            completed: false,
        }
    }
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

impl Plugin for ServerNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FallbackErrorHandler(error))
            .init_resource::<NextSessionIds>()
            .init_resource::<ServerShutdown>()
            .init_resource::<ServerStartup>()
            .init_resource::<ProcessMovementCheck>()
            .init_resource::<ProcessCombatCheck>()
            .init_resource::<ProcessMatchCheck>()
            .init_resource::<ProcessTerrainCheck>()
            .init_resource::<crate::builds::BuildTelemetry>()
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
                    process_match_commands,
                    ApplyDeferred,
                    enforce_session_deadlines,
                    disconnect_rejected_sessions,
                    verify_process_movement,
                    verify_process_combat,
                    verify_process_match,
                    verify_process_terrain,
                    exit_after_verification,
                )
                    .chain(),
            )
            .add_systems(
                Last,
                (forward_app_exit_to_server_stop, finish_server_shutdown)
                    .chain()
                    .before(crate::diagnostics::DiagnosticsSet),
            )
            .add_plugins((ServerCombatPlugin, crate::abilities::ServerAbilityPlugin));
    }
}

fn configure_new_link(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands
        .entity(trigger.entity)
        .insert((ReplicationSender, InputValidationState::default()));
}

fn spawn_server_endpoint(mut commands: Commands, config: Res<ServerNetworkConfig>) -> Result {
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

fn observe_server_endpoint(
    mut startup: ResMut<ServerStartup>,
    ready_query: Query<
        (),
        (
            With<NetcodeServer>,
            With<ServerUdpIo>,
            With<Started>,
            With<Linked>,
        ),
    >,
    failed_query: Query<
        (),
        (
            With<NetcodeServer>,
            With<ServerUdpIo>,
            With<Started>,
            Without<Linked>,
        ),
    >,
    diagnostics: Option<Res<crate::diagnostics::ProcessDiagnosticsSettings>>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if startup.ready_reported || startup.failure_reported {
        return;
    }
    if failed_query.iter().next().is_some() {
        startup.failure_reported = true;
        error!("brawler server endpoint failed to bind or link");
        if let Some(settings) = diagnostics
            && let Some(path) = settings.failure_record_path()
        {
            crate::diagnostics::write_failure_record(
                &path,
                &crate::diagnostics::ProcessFailureRecordV1::new(
                    crate::diagnostics::FailureCategory::EndpointStart,
                    "server endpoint failed to bind or link",
                ),
            );
        }
        app_exit.write(AppExit::error());
        return;
    }
    if ready_query.iter().next().is_none() {
        return;
    }
    if let Some(path) = startup.ready_file.clone() {
        if let Err(error) = fs::write(&path, b"ready\n") {
            startup.failure_reported = true;
            error!(path = %path.display(), ?error, "brawler server readiness signal failed");
            app_exit.write(AppExit::error());
            return;
        }
        info!(path = %path.display(), "brawler server readiness signal written");
    }
    startup.ready_reported = true;
}

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

#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
fn process_client_hellos(
    mut commands: Commands,
    config: Res<ServerNetworkConfig>,
    fingerprint: Res<ProtocolFingerprint>,
    content_fingerprint: Res<crate::content::GameplayContentFingerprint>,
    spawn_points: Res<SpawnPointCatalog>,
    resolved_map: Res<ResolvedMap>,
    movement_tuning: Res<MovementTuning>,
    lifecycle_rules: Res<MatchLifecycleRules>,
    fighters: Res<crate::combat::FighterDefinitions>,
    weapons: Res<crate::combat::WeaponDefinitions>,
    mut ids: ResMut<NextSessionIds>,
    mut receivers: Query<(
        Entity,
        &RemoteId,
        &mut MessageReceiver<ClientHello>,
        &mut MessageSender<JoinOutcome>,
        &mut ServerSession,
        Has<Disconnected>,
    )>,
    placeholders: Query<(), (With<Fighter>, Without<TestDummy>)>,
    match_root: Query<&MatchState, With<MatchRoot>>,
    participants: Query<(&TeamId, &MatchParticipant, &Position), With<Fighter>>,
) {
    let mut active_count = placeholders.iter().count();
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
    ordered_receivers.sort_by_key(|(_, remote_id, _, _, _, _)| remote_id.0.to_bits());
    for (connection, _, mut receiver, mut sender, mut session, disconnected) in ordered_receivers {
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
                    sender.send::<SessionChannel>(JoinOutcome::Accepted {
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
                        JoinOutcome::Rejected {
                            reason: JoinRejection::ProtocolVersionMismatch,
                        }
                    } else if hello.build_version != VERSION {
                        JoinOutcome::Rejected {
                            reason: JoinRejection::BuildVersionMismatch,
                        }
                    } else if hello.registry_fingerprint != fingerprint.0 {
                        JoinOutcome::Rejected {
                            reason: JoinRejection::RegistryMismatch,
                        }
                    } else if hello.content_fingerprint != *content_fingerprint {
                        JoinOutcome::Rejected {
                            reason: JoinRejection::ContentMismatch,
                        }
                    } else if !matches!(match_state.phase, MatchPhase::Waiting) {
                        JoinOutcome::Rejected {
                            reason: JoinRejection::MatchInProgress,
                        }
                    } else if active_count >= config.max_clients {
                        JoinOutcome::Rejected {
                            reason: JoinRejection::ServerFull,
                        }
                    } else if assigned_team(
                        team_counts,
                        lifecycle_rules.maximum_participants_per_team,
                    )
                    .is_none()
                    {
                        JoinOutcome::Rejected {
                            reason: JoinRejection::MatchFull,
                        }
                    } else {
                        match ids.allocate() {
                            Some((player_id, network_entity_id)) => {
                                let accepted = JoinOutcome::Accepted {
                                    player_id,
                                    network_entity_id,
                                };
                                let assigned_team = assigned_team(
                                    team_counts,
                                    lifecycle_rules.maximum_participants_per_team,
                                )
                                .expect("capacity was checked before identifier allocation");
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
                                let (fighter_definition, _build, team, health, _weapon) =
                                    default_fighter_runtime(assigned_team, &fighters, &weapons);
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
                                session.phase = ServerSessionPhase::Active {
                                    player_id,
                                    network_entity_id,
                                };
                                active_count += 1;
                                team_counts[usize::from(assigned_team.0)] =
                                    team_counts[usize::from(assigned_team.0)].saturating_add(1);
                                living_fighters.push((assigned_team, spawn_position));
                                accepted
                            }
                            None => JoinOutcome::Rejected {
                                reason: JoinRejection::IdentifierExhausted,
                            },
                        }
                    };
                    let rejected = matches!(outcome, JoinOutcome::Rejected { .. });
                    sender.send::<SessionChannel>(outcome.clone());
                    session.last_outcome = Some(outcome);
                    if rejected {
                        session.phase = ServerSessionPhase::Rejected;
                    }
                }
            }
        }
    }
}

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

fn enforce_session_deadlines(
    time: Res<Time<Real>>,
    mut query: Query<(Entity, &mut ServerSession, Has<Disconnected>), With<LinkOf>>,
) {
    let now = time.elapsed();
    for (entity, mut session, disconnected) in query.iter_mut() {
        if !disconnected
            && matches!(session.phase, ServerSessionPhase::AwaitingHello)
            && now >= session.deadline
        {
            session.phase = ServerSessionPhase::Rejected;
            session.last_outcome = Some(JoinOutcome::Rejected {
                reason: JoinRejection::HandshakeTimeout,
            });
            warn!(?entity, "brawler server handshake timed out");
        }
    }
}

/// Deterministic graceful exit for measurement runs: once every enabled verification check has
/// completed, request a clean shutdown so terminal evidence (closeout reports, ordered stop)
/// is produced instead of the launcher terminating the process.
fn exit_after_verification(
    movement: Res<ProcessMovementCheck>,
    combat: Res<ProcessCombatCheck>,
    match_check: Res<ProcessMatchCheck>,
    terrain: Res<ProcessTerrainCheck>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if env::var("BRAWLER_SERVER_EXIT_AFTER_VERIFICATION").as_deref() != Ok("1") {
        return;
    }
    let checks = [
        (movement.enabled, movement.completed),
        (combat.enabled, combat.completed),
        (match_check.enabled, match_check.completed),
        (terrain.enabled, terrain.completed),
    ];
    let any_enabled = checks.iter().any(|(enabled, _)| *enabled);
    let all_done = checks.iter().all(|(enabled, done)| !enabled || *done);
    if any_enabled && all_done {
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
    let mut app = App::new();
    let lifecycle = match_lifecycle_rules_for_profile(config.match_rules_profile);
    app.insert_resource(config)
        .insert_resource(lifecycle)
        .add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
            crate::timing::SIMULATION_TICK,
        )))
        .add_plugins(StatesPlugin)
        .add_plugins(TerminalCtrlCHandlerPlugin)
        .add_plugins(LogPlugin::default())
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
            crate::terrain::AuthoritativeTerrainPlugin,
            crate::diagnostics::ProcessDiagnosticsPlugin,
        ));
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
    match mode {
        GameMode::Wipeout => {
            app.insert_resource(MatchModeSetup {
                mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
                rules_revision: WIPEOUT_RULES_REVISION,
            })
            .insert_resource(wipeout_rules_for_profile(profile))
            .insert_resource(ServerMapSelection {
                preset_id: BUILT_IN_MAP_PRESET,
            })
            .add_plugins(WipeoutModePlugin);
        }
        GameMode::HotZone => {
            app.insert_resource(crate::matchplay::hot_zone_setup_for_composition())
                .insert_resource(crate::matchplay::hot_zone_rules_for_profile(profile))
                .insert_resource(ServerMapSelection {
                    preset_id: HOT_ZONE_MAP_PRESET,
                })
                .add_plugins(crate::matchplay::HotZoneModePlugin);
        }
    }
}

/// Common lifecycle rules for one rules profile; deadlines shorten without changing semantics.
#[must_use]
pub fn match_lifecycle_rules_for_profile(profile: MatchRulesProfile) -> MatchLifecycleRules {
    match profile {
        MatchRulesProfile::Production => MatchLifecycleRules::default(),
        MatchRulesProfile::ProcessVerification => MatchLifecycleRules {
            minimum_participants_per_team: 2,
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
