//! Dedicated authoritative server networking and lifecycle systems.
#![allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use crate::{
    VERSION,
    combat::{
        ActiveEffects, AuthoritativeTick, CombatCue, CombatEvidenceSnapshots, CombatStateSnapshot,
        CombatTelemetry, ResolvedWeapon, SelectedBuild, SelectedWeapon, SelectingWeapon,
        ServerCombatPlugin, SpawnState, TestDummy, WeaponCatalogResource, WeaponPresetId,
        WeaponTelemetry, WeaponTelemetryKey, decode_combat_cue, default_fighter_runtime,
        encode_state_snapshot, sandbox_team,
    },
    config::{NetworkTransport, ServerNetworkConfig},
    gameplay::GameplayPlugin,
    movement::{
        AuthoritativeMovementPlugin, AvianNetworkPlugin, GreyboxArenaDefinition, InputFreshness,
        InputValidationState, MovementTuning,
    },
    protocol::{
        ClientHello, DEVELOPMENT_PRIVATE_KEY, Fighter, FighterInput, JoinOutcome, JoinRejection,
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
            .insert_resource(ReplicationMetadata::new(crate::timing::SIMULATION_TICK))
            .add_observer(configure_new_link)
            .add_systems(Startup, spawn_server_endpoint)
            .add_systems(
                Update,
                (
                    observe_server_endpoint,
                    initialize_sessions,
                    process_client_hellos,
                    process_weapon_selection,
                    ApplyDeferred,
                    enforce_session_deadlines,
                    disconnect_rejected_sessions,
                    verify_process_movement,
                    verify_process_combat,
                )
                    .chain(),
            )
            .add_systems(
                Last,
                (forward_app_exit_to_server_stop, finish_server_shutdown).chain(),
            )
            .add_plugins(ServerCombatPlugin);
    }
}

fn verify_process_movement(
    mut check: ResMut<ProcessMovementCheck>,
    tick: Res<crate::timing::SimulationTick>,
    fighters: Query<(&PlayerId, &Position, &Rotation), With<Fighter>>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if !check.enabled || check.completed {
        return;
    }
    let mut current: Vec<_> = fighters
        .iter()
        .map(|(player, position, rotation)| (*player, position.0, rotation.as_radians()))
        .collect();
    current.sort_by_key(|(player, _, _)| player.0);
    if current.len() < 2 {
        return;
    }
    if check.initial_poses.is_empty() {
        check.initial_poses.clone_from(&current);
        check.initial_tick = Some(tick.0);
        return;
    }
    if check
        .initial_tick
        .is_none_or(|initial_tick| tick.0 < initial_tick.saturating_add(120))
    {
        return;
    }
    let moved = current.iter().any(|(player, position, _)| {
        check
            .initial_poses
            .iter()
            .find(|(initial_player, _, _)| initial_player == player)
            .is_some_and(|(_, initial_position, _)| {
                (*position - *initial_position).length() > 100.0
            })
    });
    let aimed = current.iter().any(|(player, _, facing)| {
        check
            .initial_poses
            .iter()
            .find(|(initial_player, _, _)| initial_player == player)
            .is_some_and(|(_, _, initial_facing)| (facing - initial_facing).abs() > 0.5)
    });
    if moved && aimed {
        info!(tick = tick.0, "network movement smoke assertion passed");
        if let Some(path) = check.ready_file.as_ref()
            && let Err(error) = fs::write(path, b"passed\n")
        {
            error!(
                ?path,
                ?error,
                "network movement smoke readiness signal failed"
            );
            app_exit.write(AppExit::error());
        }
        check.completed = true;
    } else {
        error!(
            tick = tick.0,
            moved, aimed, "network movement smoke assertion failed"
        );
        app_exit.write(AppExit::error());
        check.completed = true;
    }
}

fn verify_process_combat(
    mut check: ResMut<ProcessCombatCheck>,
    telemetry: Res<CombatTelemetry>,
    weapon_telemetry: Res<WeaponTelemetry>,
    evidence: Res<CombatEvidenceSnapshots>,
    catalog: Res<WeaponCatalogResource>,
    fighters: Res<crate::combat::FighterDefinitions>,
    sessions: Query<&ServerSession, With<LinkOf>>,
    selected_fighters: Query<(&SelectedBuild, &ResolvedWeapon), With<Fighter>>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if !check.enabled || check.completed {
        return;
    }
    let active_sessions = sessions
        .iter()
        .filter(|session| matches!(session.phase, ServerSessionPhase::Active { .. }))
        .count();
    let accepted_attacks: u64 = weapon_telemetry.accepted_attacks.values().copied().sum();
    let Some(expected_preset_id) = check.expected_preset_id else {
        error!("combat process assertion is missing BRAWLER_NETWORK_WEAPON_PRESET");
        check.completed = true;
        app_exit.write(AppExit::error());
        return;
    };
    let Some(_) = catalog.0.preset(expected_preset_id) else {
        error!(
            preset_id = expected_preset_id.0,
            "combat assertion requested an unknown preset"
        );
        check.completed = true;
        app_exit.write(AppExit::error());
        return;
    };
    let Some(fighter_definition) = fighters.get(crate::combat::STANDARD_FIGHTER_DEFINITION) else {
        return;
    };
    let Ok(expected_resolved) = catalog
        .0
        .resolve_preset(expected_preset_id, fighter_definition)
    else {
        error!(
            preset_id = expected_preset_id.0,
            "combat assertion could not resolve the requested preset"
        );
        check.completed = true;
        app_exit.write(AppExit::error());
        return;
    };
    let tested_fighter = selected_fighters.iter().any(|(build, resolved)| {
        build.source_preset_id == Some(expected_preset_id)
            && resolved.source_preset_id == Some(expected_preset_id)
            && resolved.recipe_fingerprint == expected_resolved.recipe_fingerprint
    });
    let expected_attacks = weapon_telemetry
        .accepted_attacks
        .get(&expected_preset_id)
        .copied()
        .unwrap_or(0);
    let expected_deliveries = weapon_telemetry
        .emitted_deliveries
        .get(&expected_preset_id)
        .copied()
        .unwrap_or(0);
    let expected_aggregate = weapon_telemetry.source_aggregates.get(&WeaponTelemetryKey {
        preset_id: expected_preset_id,
        recipe_fingerprint: expected_resolved.recipe_fingerprint,
    });
    let expected_family_exercised = expected_aggregate.is_some_and(|aggregate| {
        aggregate.accepted_attacks > 0
            && aggregate.emitted_deliveries > 0
            && aggregate.accepted_attacks == expected_attacks
            && aggregate.emitted_deliveries == expected_deliveries
    });
    let clients_observed = check.client_ready_dir.as_ref().is_some_and(|directory| {
        [1_u64, 2].iter().all(|client_id| {
            directory
                .join(format!("client-{client_id}.ready"))
                .is_file()
        })
    });
    if active_sessions < 2
        || accepted_attacks < 4
        || telemetry.applied_damage == 0
        || telemetry.defeats == 0
        || !tested_fighter
        || !expected_family_exercised
        || !clients_observed
    {
        return;
    }
    let Some(path) = check.ready_file.clone() else {
        error!("combat process assertion is enabled without a readiness file");
        check.completed = true;
        app_exit.write(AppExit::error());
        return;
    };

    let Some(client_ready_dir) = check.client_ready_dir.clone() else {
        error!("combat process assertion is enabled without a client evidence directory");
        check.completed = true;
        app_exit.write(AppExit::error());
        return;
    };
    let client_one_path = client_ready_dir.join("client-1.ready");
    let client_two_path = client_ready_dir.join("client-2.ready");
    let client_one = match fs::read_to_string(&client_one_path) {
        Ok(contents) => contents,
        Err(error) => {
            error!(path = %client_one_path.display(), ?error, "client one combat evidence could not be read");
            check.completed = true;
            app_exit.write(AppExit::error());
            return;
        }
    };
    let client_two = match fs::read_to_string(&client_two_path) {
        Ok(contents) => contents,
        Err(error) => {
            error!(path = %client_two_path.display(), ?error, "client two combat evidence could not be read");
            check.completed = true;
            app_exit.write(AppExit::error());
            return;
        }
    };
    let client_evidence_drops = [client_one.as_str(), client_two.as_str()]
        .into_iter()
        .map(|contents| {
            parse_report_counter(contents, "dropped_cue_stream")
                + parse_report_counter(contents, "dropped_cue_timestamps")
        })
        .sum::<u64>();
    if telemetry.dropped_cues > 0
        || telemetry.dropped_records > 0
        || telemetry.dropped_accepted_shot_timestamps > 0
        || client_evidence_drops > 0
    {
        error!(
            server_dropped_cues = telemetry.dropped_cues,
            server_dropped_records = telemetry.dropped_records,
            server_dropped_timestamps = telemetry.dropped_accepted_shot_timestamps,
            client_evidence_drops,
            "combat evidence history was truncated"
        );
        check.completed = true;
        app_exit.write(AppExit::error());
        return;
    }
    let through_reset = |cues: &[CombatCue]| {
        cues.iter()
            .take_while(|cue| {
                !matches!(
                    cue,
                    CombatCue::Reset { .. } | CombatCue::FighterReset { .. }
                )
            })
            .chain(
                cues.iter()
                    .skip_while(|cue| {
                        !matches!(
                            cue,
                            CombatCue::Reset { .. } | CombatCue::FighterReset { .. }
                        )
                    })
                    .take(1),
            )
            .cloned()
            .collect::<Vec<_>>()
    };
    let expected_cue_stream = through_reset(&telemetry.cues);
    let client_one_cue_stream = through_reset(&parse_client_cue_stream(&client_one));
    let client_two_cue_stream = through_reset(&parse_client_cue_stream(&client_two));
    let cue_converged = !expected_cue_stream.is_empty()
        && client_one_cue_stream.as_slice() == expected_cue_stream.as_slice()
        && client_two_cue_stream.as_slice() == expected_cue_stream.as_slice();
    if !cue_converged {
        let first_client_one_mismatch = expected_cue_stream
            .iter()
            .zip(&client_one_cue_stream)
            .position(|(expected, actual)| expected != actual);
        let first_client_two_mismatch = expected_cue_stream
            .iter()
            .zip(&client_two_cue_stream)
            .position(|(expected, actual)| expected != actual);
        error!(
            accepted_attacks,
            expected_cue_count = expected_cue_stream.len(),
            client_one_cue_count = client_one_cue_stream.len(),
            client_two_cue_count = client_two_cue_stream.len(),
            first_client_one_mismatch = ?first_client_one_mismatch,
            first_client_two_mismatch = ?first_client_two_mismatch,
            expected_cue_stream = ?expected_cue_stream,
            client_one_cue_stream = ?client_one_cue_stream,
            client_two_cue_stream = ?client_two_cue_stream,
            "combat cue stream evidence is incomplete"
        );
        check.completed = true;
        app_exit.write(AppExit::error());
        return;
    }
    let required_checkpoints = required_process_checkpoints(expected_preset_id);
    let checkpoint_converged = required_checkpoints
        .iter()
        .all(|required| evidence.checkpoints.contains_key(*required))
        && evidence.checkpoints.keys().all(|checkpoint| {
            let client_one_snapshot =
                parse_report_value(&client_one, &format!("checkpoint_{checkpoint}"));
            let client_two_snapshot =
                parse_report_value(&client_two, &format!("checkpoint_{checkpoint}"));
            evidence
                .checkpoint_candidates
                .get(checkpoint)
                .is_some_and(|candidates| {
                    candidates.iter().any(|(snapshot, _)| {
                        report_matches_snapshot(
                            &client_one,
                            &format!("checkpoint_{checkpoint}"),
                            snapshot,
                        ) && report_matches_snapshot(
                            &client_two,
                            &format!("checkpoint_{checkpoint}"),
                            snapshot,
                        )
                    }) && client_one_snapshot.is_some()
                        && client_two_snapshot.is_some()
                })
        });
    if !checkpoint_converged {
        for checkpoint in evidence.checkpoints.keys() {
            let client_one_value =
                parse_report_value(&client_one, &format!("checkpoint_{checkpoint}"));
            let client_two_value =
                parse_report_value(&client_two, &format!("checkpoint_{checkpoint}"));
            let matches_both =
                evidence
                    .checkpoint_candidates
                    .get(checkpoint)
                    .is_some_and(|candidates| {
                        candidates.iter().any(|(snapshot, _)| {
                            report_matches_snapshot(
                                &client_one,
                                &format!("checkpoint_{checkpoint}"),
                                snapshot,
                            ) && report_matches_snapshot(
                                &client_two,
                                &format!("checkpoint_{checkpoint}"),
                                snapshot,
                            )
                        })
                    });
            error!(
                checkpoint,
                server_candidates = evidence
                    .checkpoint_candidates
                    .get(checkpoint)
                    .map_or(0, Vec::len),
                client_one_present = client_one_value.is_some(),
                client_two_present = client_two_value.is_some(),
                matches_both,
                "combat checkpoint diagnostic"
            );
        }
        error!(
            server_checkpoints = ?evidence.checkpoints.keys().collect::<Vec<_>>(),
            "authoritative combat state snapshots did not converge on both clients"
        );
        check.completed = true;
        app_exit.write(AppExit::error());
        return;
    }
    let client_one_state_latencies =
        checkpoint_latencies(&evidence.checkpoint_candidates, &client_one);
    let client_two_state_latencies =
        checkpoint_latencies(&evidence.checkpoint_candidates, &client_two);
    let Some((client_one_state_median_us, client_one_state_p95_us)) =
        median_p95(&client_one_state_latencies)
    else {
        error!("client one state convergence latency evidence is incomplete");
        check.completed = true;
        app_exit.write(AppExit::error());
        return;
    };
    let Some((client_two_state_median_us, client_two_state_p95_us)) =
        median_p95(&client_two_state_latencies)
    else {
        error!("client two state convergence latency evidence is incomplete");
        check.completed = true;
        app_exit.write(AppExit::error());
        return;
    };
    if let Some(report_path) = check.report_file.clone() {
        let client_one_cues = parse_client_cue_timestamps(&client_one);
        let client_two_cues = parse_client_cue_timestamps(&client_two);
        let mut latency_evidence = String::new();
        for (shot_id, fired_at) in &telemetry.accepted_shot_timestamps {
            for (client_name, cues) in [
                ("client_one", &client_one_cues),
                ("client_two", &client_two_cues),
            ] {
                let Some((_, cue_at)) = cues.iter().find(|(candidate, _)| candidate == &shot_id.0)
                else {
                    continue;
                };
                if *cue_at >= *fired_at {
                    let _ = writeln!(
                        latency_evidence,
                        "fire_to_cue_{client_name}_us={}",
                        cue_at.saturating_sub(*fired_at)
                    );
                }
            }
        }
        if latency_evidence.is_empty() {
            error!("combat fire-to-cue latency evidence is incomplete");
            check.completed = true;
            app_exit.write(AppExit::error());
            return;
        }
        let client_one_cue_count = parse_report_counter(&client_one, "cue_count");
        let client_two_cue_count = parse_report_counter(&client_two, "cue_count");
        if client_one_cue_count == 0 || client_two_cue_count == 0 {
            error!(
                client_one_cue_count,
                client_two_cue_count, "client cue volume evidence is incomplete"
            );
            check.completed = true;
            app_exit.write(AppExit::error());
            return;
        }
        let report = format!(
            "run_id={}\nprofile={}\nserver_elapsed_ms={}\ntested_preset_id={}\ntested_recipe_fingerprint={}\ntested_accepted_attacks={}\ntested_emitted_deliveries={}\naccepted_shots={}\nhostile_hits={}\napplied_damage={}\ndefeats={}\nserver_cue_count={}\nclient_one_cue_count={}\nclient_two_cue_count={}\nserver_state_mutation_count={}\nclient_one_state_mutation_count={}\nclient_two_state_mutation_count={}\nstate_convergence_client_one_us_median={}\nstate_convergence_client_one_us_p95={}\nstate_convergence_client_two_us_median={}\nstate_convergence_client_two_us_p95={}\nserver_dropped_cues={}\nserver_dropped_records={}\nserver_dropped_timestamps={}\nstate_converged={}\ncue_converged={}\nordered_cue_stream_converged={}\n{}client_one={}client_two={}",
            check.run_id,
            env::var("BRAWLER_NETWORK_PROFILE").unwrap_or_else(|_| "local".to_string()),
            check.started_at.elapsed().as_millis(),
            expected_preset_id.0,
            expected_resolved.recipe_fingerprint.0,
            expected_attacks,
            expected_deliveries,
            accepted_attacks,
            telemetry.hostile_fighter_hits,
            telemetry.applied_damage,
            telemetry.defeats,
            telemetry.cues.len(),
            client_one_cue_count,
            client_two_cue_count,
            evidence.state_mutation_timestamps.len(),
            parse_report_counter(&client_one, "state_mutation_count"),
            parse_report_counter(&client_two, "state_mutation_count"),
            client_one_state_median_us,
            client_one_state_p95_us,
            client_two_state_median_us,
            client_two_state_p95_us,
            telemetry.dropped_cues,
            telemetry.dropped_records,
            telemetry.dropped_accepted_shot_timestamps,
            u8::from(checkpoint_converged),
            u8::from(cue_converged),
            u8::from(cue_converged),
            latency_evidence,
            client_one,
            client_two,
        );
        if let Err(error) = fs::write(&report_path, report) {
            error!(path = %report_path.display(), ?error, "combat report write failed");
            check.completed = true;
            app_exit.write(AppExit::error());
            return;
        }
    }
    if let Err(error) = fs::write(&path, b"combat-ready\n") {
        error!(path = %path.display(), ?error, "combat readiness signal failed");
        check.completed = true;
        app_exit.write(AppExit::error());
        return;
    }
    check.completed = true;
    info!(path = %path.display(), "network combat readiness signal written");
}

fn parse_client_cue_timestamps(contents: &str) -> Vec<(u64, u128)> {
    contents
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("cue_shot_id=")?;
            let (shot_id, timestamp) = rest.split_once("_epoch_us=")?;
            Some((shot_id.parse().ok()?, timestamp.parse().ok()?))
        })
        .collect()
}

fn parse_report_counter(contents: &str, key: &str) -> u64 {
    contents
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn checkpoint_latencies(
    server_candidates: &BTreeMap<String, Vec<(CombatStateSnapshot, u128)>>,
    client_report: &str,
) -> Vec<u128> {
    server_candidates
        .iter()
        .filter_map(|(checkpoint, candidates)| {
            let client_timestamp = parse_report_value(
                client_report,
                &format!("checkpoint_{checkpoint}_observed_epoch_us"),
            )?
            .parse::<u128>()
            .ok()?;
            let client_tick =
                parse_report_value(client_report, &format!("checkpoint_{checkpoint}_tick"))?
                    .parse::<u64>()
                    .ok()?;
            let (_, server_timestamp) = candidates
                .iter()
                .find(|(snapshot, _)| snapshot.authoritative_tick == client_tick)?;
            client_timestamp.checked_sub(*server_timestamp)
        })
        .collect()
}

fn required_process_checkpoints(preset_id: WeaponPresetId) -> &'static [&'static str] {
    match preset_id.0 {
        2 => &["active_scatter_flight", "defeat", "reset"],
        3 => &[
            "active_lob_flight",
            "active_slow",
            "active_knockback",
            "defeat",
            "reset",
        ],
        4 => &["active_knockback", "defeat", "reset"],
        _ => &["defeat", "reset"],
    }
}

fn median_p95(values: &[u128]) -> Option<(u128, u128)> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let median = sorted[(sorted.len() - 1) / 2];
    let p95_rank = (sorted.len() * 95).saturating_add(99) / 100;
    let p95 = sorted[p95_rank.saturating_sub(1).min(sorted.len() - 1)];
    Some((median, p95))
}

fn parse_report_value<'a>(contents: &'a str, key: &str) -> Option<&'a str> {
    contents
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
}

fn report_matches_snapshot(
    report: &str,
    key: &str,
    authoritative_snapshot: &CombatStateSnapshot,
) -> bool {
    let Some(encoded) = encode_state_snapshot(authoritative_snapshot) else {
        return false;
    };
    parse_report_value(report, key) == Some(encoded.as_str())
        || report
            .lines()
            .any(|line| line.strip_prefix(&format!("{key}_candidate=")) == Some(encoded.as_str()))
}

fn parse_client_cue_stream(contents: &str) -> Vec<CombatCue> {
    contents
        .lines()
        .filter_map(|line| decode_combat_cue(line.strip_prefix("cue_stream=")?))
        .collect()
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
            });
        }
    }
}

fn process_client_hellos(
    mut commands: Commands,
    config: Res<ServerNetworkConfig>,
    fingerprint: Res<ProtocolFingerprint>,
    content_fingerprint: Res<crate::combat::GameplayContentFingerprint>,
    arena: Res<GreyboxArenaDefinition>,
    movement_tuning: Res<MovementTuning>,
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
) {
    let mut active_count = placeholders.iter().count();
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
                    let outcome = if active_count >= config.max_clients {
                        JoinOutcome::Rejected {
                            reason: JoinRejection::ServerFull,
                        }
                    } else if hello.protocol_version != crate::protocol::SUPPORTED_PROTOCOL_VERSION
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
                    } else {
                        match ids.allocate() {
                            Some((player_id, network_entity_id)) => {
                                let accepted = JoinOutcome::Accepted {
                                    player_id,
                                    network_entity_id,
                                };
                                let spawn_position = arena.spawn_position(player_id.0);
                                let (fighter_definition, _build, team, health, _weapon) =
                                    default_fighter_runtime(
                                        sandbox_team(player_id),
                                        &fighters,
                                        &weapons,
                                    );
                                let fighter_entity = commands
                                    .spawn((
                                        Fighter,
                                        player_id,
                                        network_entity_id,
                                        PlaceholderState {
                                            spawn_slot: u64::from(
                                                GreyboxArenaDefinition::spawn_slot(player_id.0),
                                            ),
                                        },
                                        fighter_definition,
                                        team,
                                        health,
                                        SelectingWeapon,
                                        ActiveEffects::default(),
                                        AuthoritativeTick::default(),
                                        SpawnState {
                                            position: spawn_position,
                                            facing: movement_tuning.spawn_facing,
                                        },
                                        Position::from_xy(spawn_position.x, spawn_position.y),
                                        Rotation::radians(movement_tuning.spawn_facing),
                                        LinearVelocity::default(),
                                        AngularVelocity::default(),
                                    ))
                                    .id();
                                commands.entity(fighter_entity).insert((
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
mod tests {
    use super::*;

    #[test]
    fn checked_ids_start_at_one_and_never_wrap() {
        let mut ids = NextSessionIds::default();
        assert_eq!(ids.allocate(), Some((PlayerId(1), NetworkEntityId(1))));
        assert_eq!(ids.allocate(), Some((PlayerId(2), NetworkEntityId(2))));
        ids.next_player_id = u64::MAX;
        assert_eq!(ids.allocate(), None);
        assert_eq!(ids.next_player_id, u64::MAX);
    }

    #[test]
    fn server_config_rejects_unbounded_values() {
        let config = ServerNetworkConfig {
            max_clients: 0,
            ..default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn started_unlinked_udp_server_requests_error_exit() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_resource::<lightyear::prelude::PeerMetadata>()
            .init_resource::<ServerStartup>()
            .add_systems(Update, observe_server_endpoint);
        app.world_mut().spawn((
            NetcodeServer::new(NetcodeConfig::default()),
            ServerUdpIo::default(),
            Started,
        ));

        app.update();

        assert!(app.should_exit().is_some_and(|exit| exit.is_error()));
    }

    #[test]
    fn app_exit_is_forwarded_after_update_producers_run() {
        fn request_exit(mut app_exit: MessageWriter<AppExit>) {
            app_exit.write(AppExit::Success);
        }

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<AppExit>()
            .init_resource::<lightyear::prelude::PeerMetadata>()
            .init_resource::<ServerShutdown>()
            .add_systems(Update, request_exit)
            .add_systems(
                Last,
                (forward_app_exit_to_server_stop, finish_server_shutdown).chain(),
            )
            .add_observer(|trigger: On<Stop>, mut commands: Commands| {
                commands.entity(trigger.entity).insert(Stopped);
            });
        let server = app
            .world_mut()
            .spawn(NetcodeServer::new(NetcodeConfig::default()))
            .id();

        app.update();

        assert!(
            app.world()
                .resource::<ServerShutdown>()
                .requested_exit
                .is_none()
        );
        assert!(app.world().get::<Stopped>(server).is_some());
        assert!(app.should_exit().is_some_and(|exit| exit.is_success()));
    }

    #[test]
    fn checkpoint_reports_fail_closed_on_missing_or_altered_state() {
        let snapshot = CombatStateSnapshot {
            authoritative_tick: 7,
            fighters: Vec::new(),
            projectiles: Vec::new(),
        };
        let encoded = encode_state_snapshot(&snapshot).expect("snapshot encoding");
        let report = format!("checkpoint_reset={encoded}\n");
        assert!(report_matches_snapshot(
            &report,
            "checkpoint_reset",
            &snapshot
        ));
        assert!(!report_matches_snapshot(
            "checkpoint_reset=00\n",
            "checkpoint_reset",
            &snapshot,
        ));
        assert!(!report_matches_snapshot(
            "checkpoint_reset_missing=true\n",
            "checkpoint_reset",
            &snapshot,
        ));
    }
}
