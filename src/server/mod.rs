//! Dedicated-server composition and authoritative network-session concerns.
#![allow(clippy::needless_pass_by_value, clippy::type_complexity)]

use crate::{
    VERSION,
    combat::{
        ActiveEffects, AuthoritativeTick, CombatCue, CombatEvidenceSnapshots, CombatStateSnapshot,
        CombatTelemetry, ResolvedWeapon, SelectedBuild, SelectedWeapon, SelectingWeapon,
        ServerCombatPlugin, SpawnState, TeamId, TestDummy, WeaponCatalogResource, WeaponPresetId,
        WeaponTelemetry, WeaponTelemetryKey, decode_combat_cue, default_fighter_runtime,
        encode_state_snapshot,
    },
    config::{NetworkTransport, ServerNetworkConfig},
    gameplay::GameplayPlugin,
    map::{AuthoritativeMapPlugin, MapStartupSet, ResolvedMap, SpawnAssignment, SpawnPointCatalog},
    matchplay::{
        MatchMember, MatchParticipant, MatchPhase, MatchRoot, MatchState, WipeoutPlugin,
        WipeoutRules, assigned_team,
    },
    movement::{
        AuthoritativeMovementPlugin, AvianNetworkPlugin, InputFreshness, InputValidationState,
        MovementTuning,
    },
    protocol::{
        ClientHello, DEVELOPMENT_PRIVATE_KEY, Fighter, FighterInput, JoinOutcome, JoinRejection,
        MatchCommand, MatchCommandDecision, MatchCommandOutcome, MatchCommandRequest,
        NetworkEntityId, PlaceholderState, PlayerId, ProtocolFingerprint, ProtocolPlugin,
        SessionChannel, WeaponSelectionDecision, WeaponSelectionOutcome, WeaponSelectionRequest,
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
use lightyear::prelude::input::native::{ActionState, NativeBuffer};
use lightyear::prelude::server::{
    NetcodeConfig, NetcodeServer, ServerPlugins, Start, Started, Stop, Stopped,
};
use lightyear::prelude::server::{Server as LightyearServer, ServerUdpIo};
use lightyear::prelude::{Connected, Disconnected, LinkOf, Linked, LocalAddr};
use lightyear::prelude::{
    ControlledBy, InterpolationTarget, Lifetime, MessageReceiver, MessageSender, NetworkTarget,
    Replicate, ReplicationMetadata, ReplicationSender,
};
use std::{
    collections::{BTreeMap, HashSet},
    env,
    fmt::Write as _,
    fs,
    path::PathBuf,
    time::Instant,
};

mod verification;
#[cfg(test)]
#[allow(clippy::wildcard_imports)]
use verification::*;
use verification::{verify_process_combat, verify_process_movement};

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
    pub last_selection_request: Option<WeaponSelectionRequest>,
    pub last_selection_outcome: Option<WeaponSelectionOutcome>,
    pub last_selection_response: Option<WeaponSelectionOutcome>,
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
                    process_weapon_selection,
                    process_match_commands,
                    ApplyDeferred,
                    enforce_session_deadlines,
                    disconnect_rejected_sessions,
                    verify_process_movement,
                    verify_process_combat,
                    verify_process_match,
                )
                    .chain(),
            )
            .add_systems(
                Last,
                (forward_app_exit_to_server_stop, finish_server_shutdown).chain(),
            )
            .add_plugins((ServerCombatPlugin, WipeoutPlugin));
    }
}

fn verify_process_match(
    mut check: ResMut<ProcessMatchCheck>,
    roots: Query<&MatchState, With<MatchRoot>>,
    telemetry: Res<crate::matchplay::MatchTelemetry>,
    participants: Query<&MatchParticipant, With<Fighter>>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if !check.enabled || check.completed {
        return;
    }
    let Ok(state) = roots.single() else {
        return;
    };
    let initial = *check.initial_match_id.get_or_insert(state.match_id);
    let Some(summary) = telemetry.summaries.back() else {
        return;
    };
    if state.match_id.0 <= initial.0 || !matches!(state.phase, MatchPhase::Waiting) {
        return;
    }
    let participant_count = participants
        .iter()
        .filter(|participant| participant.match_id == state.match_id)
        .count();
    let (Some(map_identity), Some(content_fingerprint)) =
        (summary.map_identity, summary.content_fingerprint)
    else {
        error!("match summary omitted map or content identity");
        app_exit.write(AppExit::error());
        check.completed = true;
        return;
    };
    if summary.participants.len() != 4 {
        error!(
            participant_count = summary.participants.len(),
            "match summary omitted initial participant identity"
        );
        app_exit.write(AppExit::error());
        check.completed = true;
        return;
    }
    if !has_preset_outcome_evidence(summary) {
        error!("match summary omitted preset defeat/death evidence");
        app_exit.write(AppExit::error());
        check.completed = true;
        return;
    }
    let weapon_preset_ids = summary
        .weapon_aggregates
        .iter()
        .map(|(key, _)| key.preset_id.0.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let accepted_attacks = summary
        .weapon_aggregates
        .iter()
        .map(|(_, aggregate)| aggregate.accepted_attacks)
        .sum::<u64>();
    let attacks_with_hostile_contact = summary
        .weapon_aggregates
        .iter()
        .map(|(_, aggregate)| aggregate.attacks_with_hostile_contact)
        .sum::<u64>();
    let preset_defeats = format_preset_counts(&summary.credited_defeats_by_preset);
    let preset_deaths = format_preset_counts(&summary.suffered_deaths_by_preset);
    let preset_death_rates =
        format_preset_rates(&summary.suffered_deaths_per_participant_minute_by_preset);
    let report = format!(
        "initial_match_id={}\nrestarted_match_id={}\nparticipant_count={}\nsummary_participant_count={}\nmap_instance_id={}\nmap_recipe_fingerprint={}\ncontent_fingerprint={}\nrules_revision={}\nfinal_score_team_1={}\nfinal_score_team_2={}\nresult={:?}\nactive_duration_ticks={}\ndefeats={}\nrespawns={}\nparticipant_active_ticks_team_1={}\nparticipant_active_ticks_team_2={}\nrecords={}\ndropped_records={}\nsummary_count={}\nweapon_aggregate_count={}\nweapon_preset_ids={}\npreset_defeats={}\npreset_deaths={}\npreset_death_rates={}\naccepted_attacks={}\nattacks_with_hostile_contact={}\n",
        initial.0,
        state.match_id.0,
        participant_count,
        summary.participants.len(),
        map_identity.instance_id.0,
        map_identity.recipe_fingerprint.0,
        content_fingerprint.0,
        summary.rules_revision,
        summary.final_scores[0],
        summary.final_scores[1],
        summary.result,
        summary.active_duration_ticks,
        summary.suffered_deaths_by_team.iter().sum::<u32>(),
        summary.respawns,
        summary.participant_active_ticks_by_team[0],
        summary.participant_active_ticks_by_team[1],
        telemetry.records.len(),
        summary.dropped_records,
        telemetry.summaries.len(),
        summary.weapon_aggregates.len(),
        weapon_preset_ids,
        preset_defeats,
        preset_deaths,
        preset_death_rates,
        accepted_attacks,
        attacks_with_hostile_contact,
    );
    if let Some(path) = &check.report_file
        && let Err(error) = fs::write(path, report.as_bytes())
    {
        error!(path = %path.display(), ?error, "match report write failed");
        app_exit.write(AppExit::error());
        check.completed = true;
        return;
    }
    info!(%report, "authoritative Wipeout process verification complete");
    check.completed = true;
    app_exit.write(AppExit::Success);
}

fn has_preset_outcome_evidence(summary: &crate::matchplay::MatchSummary) -> bool {
    !summary.credited_defeats_by_preset.is_empty()
        && !summary.suffered_deaths_by_preset.is_empty()
        && !summary
            .suffered_deaths_per_participant_minute_by_preset
            .is_empty()
}

fn format_preset_counts(values: &[(WeaponPresetId, u32)]) -> String {
    values
        .iter()
        .map(|(preset, count)| format!("{}:{count}", preset.0))
        .collect::<Vec<_>>()
        .join(",")
}

fn format_preset_rates(values: &[(WeaponPresetId, f64)]) -> String {
    values
        .iter()
        .map(|(preset, rate)| format!("{}:{rate:.3}", preset.0))
        .collect::<Vec<_>>()
        .join(",")
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
    mut app_exit: MessageWriter<AppExit>,
) {
    if startup.ready_reported || startup.failure_reported {
        return;
    }
    if failed_query.iter().next().is_some() {
        startup.failure_reported = true;
        error!("brawler server endpoint failed to bind or link");
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
    wipeout_rules: Res<WipeoutRules>,
    fighters: Res<crate::combat::FighterDefinitions>,
    weapons: Res<crate::combat::WeaponDefinitions>,
    mut ids: ResMut<NextSessionIds>,
    mut receivers: Query<(
        Entity,
        &mut MessageReceiver<ClientHello>,
        &mut MessageSender<JoinOutcome>,
        &mut ServerSession,
        Has<Disconnected>,
    )>,
    placeholders: Query<(), (With<Fighter>, Without<TestDummy>)>,
    match_root: Query<&MatchState, With<MatchRoot>>,
    participants: Query<(&TeamId, &MatchParticipant), With<Fighter>>,
) {
    let mut active_count = placeholders.iter().count();
    let Ok(match_state) = match_root.single() else {
        return;
    };
    let mut team_counts = [0_u8; 2];
    for (team, participant) in &participants {
        if participant.match_id == match_state.match_id && team.0 <= 1 {
            team_counts[usize::from(team.0)] = team_counts[usize::from(team.0)].saturating_add(1);
        }
    }
    for (connection, mut receiver, mut sender, mut session, disconnected) in receivers.iter_mut() {
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
                        wipeout_rules.maximum_participants_per_team,
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
                                    wipeout_rules.maximum_participants_per_team,
                                )
                                .expect("capacity was checked before identifier allocation");
                                let spawn_ordinal = player_id.0.saturating_sub(1) / 2;
                                let spawn_point = spawn_points
                                    .deterministic_point(assigned_team.0, spawn_ordinal)
                                    .expect("validated map has a spawn for each sandbox team");
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
                                            spawn_slot: u64::from(spawn_point.spawn_point_id.0),
                                        },
                                        fighter_definition,
                                        team,
                                        health,
                                        SelectingWeapon,
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
                                        spawn_point_id: spawn_point.spawn_point_id,
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

/// Resolve a preset request against the server's embedded catalog. The request is scoped to the
/// receiving link; it never carries a fighter/entity target or any client-authored recipe data.
#[allow(clippy::too_many_lines)]
fn process_weapon_selection(
    mut commands: Commands,
    catalog: Res<WeaponCatalogResource>,
    definitions: Res<crate::combat::FighterDefinitions>,
    tick: Res<crate::timing::SimulationTick>,
    mut telemetry: ResMut<WeaponTelemetry>,
    mut sessions: Query<(
        Entity,
        &mut MessageReceiver<WeaponSelectionRequest>,
        &mut MessageSender<WeaponSelectionOutcome>,
        &mut ServerSession,
        Has<Disconnected>,
    )>,
    mut fighter_query: Query<
        (
            Entity,
            &ControlledBy,
            &crate::combat::FighterDefinitionId,
            Option<&SelectingWeapon>,
            Option<&mut NativeBuffer<FighterInput>>,
            Option<&mut ActionState<FighterInput>>,
            Option<&mut InputFreshness>,
        ),
        With<Fighter>,
    >,
) {
    // Commands are deferred until the explicit ApplyDeferred boundary after this system. Keep
    // an in-tick lock as well, so two queued requests cannot both observe SelectingWeapon and
    // schedule competing accepted builds for the same fighter.
    let mut accepted_fighters_this_tick = HashSet::new();
    for (connection, mut receiver, mut sender, mut session, disconnected) in &mut sessions {
        if disconnected {
            receiver.receive().for_each(drop);
            continue;
        }
        let requests: Vec<_> = receiver.receive().collect();
        for request in requests {
            if session
                .last_selection_request
                .is_some_and(|previous| request.request_id < previous.request_id)
            {
                let outcome = WeaponSelectionOutcome {
                    request_id: request.request_id,
                    decision: WeaponSelectionDecision::StaleRequest,
                    accepted_preset_id: None,
                    accepted_recipe_fingerprint: None,
                };
                session.last_selection_response = Some(outcome);
                sender.send::<SessionChannel>(outcome);
                continue;
            }
            if session
                .last_selection_request
                .is_some_and(|previous| request.request_id == previous.request_id)
            {
                if let Some(outcome) = session.last_selection_outcome {
                    session.last_selection_response = Some(outcome);
                    sender.send::<SessionChannel>(outcome);
                }
                continue;
            }

            let fighter = fighter_query
                .iter_mut()
                .find(|(_, controlled, _, _, _, _, _)| controlled.owner == connection);
            let outcome = if let Some((
                fighter_entity,
                _,
                fighter_definition_id,
                selecting,
                mut input_buffer,
                mut action,
                mut input_freshness,
            )) = fighter
            {
                if selecting.is_none() || accepted_fighters_this_tick.contains(&fighter_entity) {
                    WeaponSelectionOutcome {
                        request_id: request.request_id,
                        decision: WeaponSelectionDecision::NotSelecting,
                        accepted_preset_id: None,
                        accepted_recipe_fingerprint: None,
                    }
                } else if catalog.0.preset(request.preset_id).is_none() {
                    WeaponSelectionOutcome {
                        request_id: request.request_id,
                        decision: WeaponSelectionDecision::UnknownPreset,
                        accepted_preset_id: None,
                        accepted_recipe_fingerprint: None,
                    }
                } else {
                    let resolved = definitions
                        .get(*fighter_definition_id)
                        .ok_or_else(|| "fighter definition missing".to_string())
                        .and_then(|fighter_definition| {
                            catalog
                                .0
                                .resolve_preset(request.preset_id, fighter_definition)
                        });
                    match resolved {
                        Ok(resolved) => {
                            accepted_fighters_this_tick.insert(fighter_entity);
                            // Selection acceptance is a hard input epoch boundary. Discard every
                            // buffered native state, the currently applied action, and its cached
                            // watermark so a packet sent before acceptance, including one carrying
                            // a future tick, cannot satisfy the post-selection freshness barrier.
                            if let Some(buffer) = input_buffer.as_mut() {
                                **buffer = NativeBuffer::default();
                            }
                            if let Some(action) = action.as_mut() {
                                **action = ActionState::default();
                            }
                            if let Some(input_freshness) = input_freshness.as_mut() {
                                **input_freshness = InputFreshness::default();
                            }
                            let capacity = resolved.recipe.economy.capacity();
                            commands
                                .entity(fighter_entity)
                                .insert((
                                    SelectedBuild {
                                        primary_weapon: crate::combat::WeaponDefinitionId(
                                            request.preset_id.0,
                                        ),
                                        source_preset_id: Some(request.preset_id),
                                        recipe_fingerprint: Some(resolved.recipe_fingerprint),
                                    },
                                    SelectedWeapon {
                                        source_preset_id: request.preset_id,
                                        recipe_fingerprint: resolved.recipe_fingerprint,
                                    },
                                    resolved.clone(),
                                    crate::combat::WeaponState {
                                        ammo: capacity,
                                        phase: crate::combat::WeaponPhase::Ready,
                                    },
                                    ActiveEffects::default(),
                                    crate::combat::AwaitingPostSelectionInput {
                                        accepted_at_tick: tick.0,
                                    },
                                    CollisionLayers::new(
                                        crate::movement::FIGHTER_LAYER,
                                        crate::movement::INDESTRUCTIBLE_TERRAIN_LAYER
                                            | crate::movement::DESTRUCTIBLE_TERRAIN_LAYER,
                                    ),
                                ))
                                .remove::<SelectingWeapon>();
                            telemetry.record_selection(
                                request.preset_id,
                                resolved.recipe_fingerprint,
                                tick.0,
                                request.request_id,
                            );
                            let _ = fighter_definition_id;
                            WeaponSelectionOutcome {
                                request_id: request.request_id,
                                decision: WeaponSelectionDecision::Accepted,
                                accepted_preset_id: Some(request.preset_id),
                                accepted_recipe_fingerprint: Some(resolved.recipe_fingerprint),
                            }
                        }
                        Err(error) => {
                            warn!(
                                ?error,
                                preset_id = request.preset_id.0,
                                "weapon selection resolution failed"
                            );
                            WeaponSelectionOutcome {
                                request_id: request.request_id,
                                decision: WeaponSelectionDecision::ResolutionFailed,
                                accepted_preset_id: None,
                                accepted_recipe_fingerprint: None,
                            }
                        }
                    }
                }
            } else {
                WeaponSelectionOutcome {
                    request_id: request.request_id,
                    decision: WeaponSelectionDecision::NotSelecting,
                    accepted_preset_id: None,
                    accepted_recipe_fingerprint: None,
                }
            };
            session.last_selection_request = Some(request);
            session.last_selection_outcome = Some(outcome);
            session.last_selection_response = Some(outcome);
            sender.send::<SessionChannel>(outcome);
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
            Option<&SelectedWeapon>,
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
                .find(|(controlled, _, _)| controlled.owner == connection);
            let decision = if request.match_id != match_state.match_id {
                MatchCommandDecision::WrongMatch
            } else if let Some((_, mut participant, selected)) = participant {
                match (request.command, match_state.phase) {
                    (MatchCommand::SetReady(value), MatchPhase::Waiting) if selected.is_some() => {
                        participant.ready = value;
                        MatchCommandDecision::Accepted
                    }
                    (MatchCommand::SetReady(value), MatchPhase::Countdown { .. })
                        if selected.is_some() =>
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
    app.insert_resource(config)
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
        ));
    app
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
