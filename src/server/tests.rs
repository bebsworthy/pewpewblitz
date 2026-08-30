//! Focused dedicated-server composition and session tests.

use super::*;

#[test]
fn server_network_plugin_does_not_select_gameplay_or_routed_transport() {
    let mut app = App::new();
    app.add_plugins(ServerNetworkPlugin);

    assert!(!app.is_plugin_added::<GameplayContentPlugin>());
    assert!(!app.is_plugin_added::<ProtocolPlugin>());
    assert!(!app.is_plugin_added::<ServerAuthoritativeGameplayPlugin>());
    assert!(!app.is_plugin_added::<AuthoritativeMapPlugin>());
    assert!(!app.is_plugin_added::<AuthoritativeMovementPlugin>());
    assert!(!app.is_plugin_added::<ServerCombatPlugin>());
    assert!(!app.is_plugin_added::<crate::concealment::ServerConcealmentPlugin>());
    assert!(!app.is_plugin_added::<crate::abilities::ServerAbilityPlugin>());
    assert!(!app.is_plugin_added::<AuthoritativeMatchPlugin>());
    assert!(!app.is_plugin_added::<RoutedWorkerPlugin>());
}

#[test]
fn authoritative_server_builder_explicitly_installs_role_plugins() {
    let app = build_app_with_config(ServerNetworkConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        ..default()
    });

    assert!(app.is_plugin_added::<GameplayContentPlugin>());
    assert!(app.is_plugin_added::<ProtocolPlugin>());
    assert!(app.is_plugin_added::<ServerAuthoritativeGameplayPlugin>());
    assert!(app.is_plugin_added::<AuthoritativeMapPlugin>());
    assert!(app.is_plugin_added::<AuthoritativeMovementPlugin>());
    assert!(app.is_plugin_added::<ServerCombatPlugin>());
    assert!(app.is_plugin_added::<crate::concealment::ServerConcealmentPlugin>());
    assert!(app.is_plugin_added::<crate::abilities::ServerAbilityPlugin>());
    assert!(app.is_plugin_added::<AuthoritativeMatchPlugin>());
    assert!(app.is_plugin_added::<practice::PracticeBotPlugin>());
    assert!(app.is_plugin_added::<ServerNetworkPlugin>());
    assert!(app.is_plugin_added::<RoutedWorkerPlugin>());
}

#[derive(Resource, Default)]
struct SessionScheduleTrace(Vec<&'static str>);

fn session_probe(label: &'static str) -> impl FnMut(ResMut<SessionScheduleTrace>) + 'static {
    move |mut trace: ResMut<SessionScheduleTrace>| trace.0.push(label)
}

#[test]
fn server_session_phases_have_one_explicit_authority_order() {
    let mut app = App::new();
    app.init_resource::<SessionScheduleTrace>();
    configure_server_session_schedule(&mut app);
    app.add_systems(
        Update,
        (
            session_probe("receive").in_set(ServerSessionSet::ReceiveAndValidate),
            session_probe("commit").in_set(ServerSessionSet::CommitCommands),
            session_probe("enforce").in_set(ServerSessionSet::Enforce),
            session_probe("observe").in_set(ServerSessionSet::Observe),
            session_probe("terminal").in_set(ServerSessionSet::Terminal),
        ),
    );
    crate::test_app::reject_owned_schedule_ambiguities(&mut app, Update);

    app.update();

    assert_eq!(
        app.world().resource::<SessionScheduleTrace>().0,
        vec!["receive", "commit", "enforce", "observe", "terminal"]
    );
}

#[test]
fn server_receive_phase_preserves_cleanup_before_deferred_flush() {
    let mut app = App::new();
    app.init_resource::<SessionScheduleTrace>();
    configure_server_session_schedule(&mut app);
    app.add_systems(
        Update,
        (
            session_probe("network").in_set(ServerSessionReceiveSet::NetworkLifecycle),
            session_probe("gameplay-cleanup").in_set(ServerSessionReceiveSet::GameplayCleanup),
            session_probe("flush").in_set(ServerSessionReceiveSet::Flush),
        ),
    );
    crate::test_app::reject_owned_schedule_ambiguities(&mut app, Update);

    app.update();

    assert_eq!(
        app.world().resource::<SessionScheduleTrace>().0,
        vec!["network", "gameplay-cleanup", "flush"]
    );
}

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
fn production_startup_instantiates_wipeout_map_and_match_without_practice_dummy() {
    let mut app = build_app_with_config(ServerNetworkConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        ..default()
    });
    crate::test_app::finalize(&mut app);
    app.update();
    assert!(app.world().contains_resource::<ResolvedMap>());
    let world = app.world_mut();
    let mut dummies = world.query_filtered::<Entity, With<TestDummy>>();
    assert_eq!(dummies.iter(world).count(), 0);
    let mut matches = world.query_filtered::<&MatchState, With<MatchRoot>>();
    assert_eq!(matches.iter(world).count(), 1);
}

#[test]
fn shortened_rules_require_the_explicit_verification_profile() {
    let lifecycle = match_lifecycle_rules_for_profile(MatchRulesProfile::ProcessVerification);
    assert_eq!(lifecycle.minimum_participants_per_team, 1);
    assert_eq!(lifecycle.active_limit_ticks, 3_600);
    assert_eq!(lifecycle.countdown_ticks, 30);
    let wipeout = crate::modes::wipeout_rules_for_profile(MatchRulesProfile::ProcessVerification);
    assert_eq!(wipeout.target_score, 10);
    let hot_zone =
        crate::matchplay::hot_zone_rules_for_profile(MatchRulesProfile::ProcessVerification);
    assert_eq!(hot_zone.target_progress_ticks, 30);
}

#[test]
fn started_unlinked_udp_server_requests_error_exit() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .init_resource::<lightyear::prelude::PeerMetadata>()
        .init_resource::<ServerStartup>()
        .init_resource::<crate::diagnostics::ProcessExitClassification>()
        .add_systems(Update, observe_server_endpoint);
    app.world_mut().spawn((
        NetcodeServer::new(NetcodeConfig::default()),
        ServerUdpIo::default(),
        Started,
    ));

    app.update();

    assert!(app.should_exit().is_some_and(|exit| exit.is_error()));
}

/// A failed process verification must classify as `verification-failed` instead of
/// collapsing into the undifferentiated error-exit mapping, and must carry the shared
/// server failure path (failure record selected by the environment control).
#[test]
fn movement_verification_failure_classifies_as_verification_failed() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(crate::timing::SimulationTick(1_000))
        .insert_resource(ProcessMovementCheck {
            enabled: true,
            ready_file: None,
            initial_poses: vec![
                (PlayerId(1), Vec2::ZERO, 0.0),
                (PlayerId(2), Vec2::ZERO, 0.0),
            ],
            initial_tick: Some(0),
            completed: false,
        })
        .init_resource::<crate::diagnostics::ProcessExitClassification>()
        .add_systems(Update, verify_process_movement);
    app.world_mut().spawn((
        Fighter,
        PlayerId(1),
        Position::from_xy(0.0, 0.0),
        Rotation::radians(0.0),
    ));
    app.world_mut().spawn((
        Fighter,
        PlayerId(2),
        Position::from_xy(10.0, 0.0),
        Rotation::radians(0.0),
    ));

    app.update();

    assert!(app.should_exit().is_some_and(|exit| exit.is_error()));
    assert_eq!(
        app.world()
            .resource::<crate::diagnostics::ProcessExitClassification>()
            .classified_category(&AppExit::error()),
        crate::diagnostics::ProcessExitCategory::VerificationFailed
    );
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
fn waiting_after_activation_is_not_a_routed_start_failure() {
    let initial_departure = MatchLoadingGate {
        countdown_observed: true,
        ..default()
    };
    assert!(failed_initial_countdown(
        &initial_departure,
        MatchPhase::Waiting
    ));

    let reset_after_activation = MatchLoadingGate {
        countdown_observed: true,
        activated_emitted: true,
        ..default()
    };
    assert!(!failed_initial_countdown(
        &reset_after_activation,
        MatchPhase::Waiting
    ));
    assert!(!failed_initial_countdown(
        &reset_after_activation,
        MatchPhase::Countdown { starts_at_tick: 10 }
    ));
}

#[test]
fn checkpoint_reports_fail_closed_on_missing_or_altered_state() {
    let snapshot = CombatStateSnapshot {
        authoritative_tick: 7,
        fighters: Vec::new(),
        projectiles: Vec::new(),
        cone_sprays: Vec::new(),
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

#[test]
fn combat_prerequisites_require_every_authoritative_and_client_signal() {
    let mut telemetry = CombatTelemetry {
        applied_damage: 10,
        defeats: 1,
        ..default()
    };
    assert!(combat_prerequisites_met(2, 4, &telemetry, true, true, true));

    telemetry.defeats = 0;
    assert!(!combat_prerequisites_met(
        2, 4, &telemetry, true, true, true
    ));
    telemetry.defeats = 1;
    assert!(!combat_prerequisites_met(
        2, 4, &telemetry, true, true, false
    ));
}

#[test]
fn cue_comparison_stops_at_the_first_reset() {
    let defeat = CombatCue::Defeat {
        event_id: crate::combat::CombatEventId(1),
        tick: 10,
        source: None,
        target: NetworkEntityId(2),
    };
    let reset = CombatCue::Reset {
        event_id: crate::combat::CombatEventId(2),
        tick: 11,
        target: NetworkEntityId(2),
        position: crate::combat::WorldPoint::from(Vec2::ZERO),
    };
    let after_reset = CombatCue::Defeat {
        event_id: crate::combat::CombatEventId(3),
        tick: 12,
        source: None,
        target: NetworkEntityId(3),
    };

    assert_eq!(
        cues_through_first_reset(&[defeat.clone(), reset.clone(), after_reset]),
        vec![defeat, reset]
    );
}

#[test]
fn cue_comparison_keeps_expected_cues_and_ignores_other_authoritative_events() {
    let expected_defeat = CombatCue::Defeat {
        event_id: crate::combat::CombatEventId(1),
        tick: 10,
        source: None,
        target: NetworkEntityId(2),
    };
    let additional_environment_cue = CombatCue::Defeat {
        event_id: crate::combat::CombatEventId(99),
        tick: 10,
        source: Some(crate::combat::DamageSource::Environment {
            map_instance_id: 1,
            generation: 1,
            placement_id: 4,
            initiating_player: None,
            initiating_fighter: None,
        }),
        target: NetworkEntityId(7),
    };
    let expected_reset = CombatCue::Reset {
        event_id: crate::combat::CombatEventId(2),
        tick: 11,
        target: NetworkEntityId(2),
        position: crate::combat::WorldPoint::from(Vec2::ZERO),
    };
    let expected = vec![expected_defeat.clone(), expected_reset.clone()];
    let observed = vec![expected_defeat, additional_environment_cue, expected_reset];

    assert_eq!(cues_matching_expected(&expected, &observed), expected);
}

#[test]
fn checkpoint_convergence_requires_the_same_server_candidate_on_both_clients() {
    let snapshot = CombatStateSnapshot {
        authoritative_tick: 7,
        fighters: Vec::new(),
        projectiles: Vec::new(),
        cone_sprays: Vec::new(),
    };
    let encoded = encode_state_snapshot(&snapshot).expect("snapshot encoding");
    let matching_report = format!("checkpoint_reset={encoded}\n");
    let mut evidence = CombatEvidenceSnapshots::default();
    evidence
        .checkpoints
        .insert("reset".to_string(), snapshot.clone());
    evidence
        .checkpoint_candidates
        .insert("reset".to_string(), vec![(snapshot, 100)]);

    assert!(checkpoint_reports_converge(
        &evidence,
        &["reset"],
        &matching_report,
        &matching_report,
    ));
    assert!(!checkpoint_reports_converge(
        &evidence,
        &["reset"],
        &matching_report,
        "checkpoint_reset=00\n",
    ));
    assert!(!checkpoint_reports_converge(
        &evidence,
        &["defeat", "reset"],
        &matching_report,
        &matching_report,
    ));
}

#[test]
fn fire_to_cue_latency_uses_only_nonnegative_matching_samples() {
    let evidence = fire_to_cue_latency_evidence(
        &[
            (crate::combat::ShotId(7), 100),
            (crate::combat::ShotId(8), 200),
        ],
        "cue_shot_id=7_epoch_us=150\ncue_shot_id=8_epoch_us=190\n",
        "cue_shot_id=7_epoch_us=175\n",
    );

    assert!(evidence.contains("fire_to_cue_client_one_us=50\n"));
    assert!(evidence.contains("fire_to_cue_client_two_us=75\n"));
    assert!(!evidence.contains("shot_id=8"));
}
