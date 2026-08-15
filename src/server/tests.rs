//! Focused dedicated-server composition and session tests.

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
fn production_startup_instantiates_wipeout_map_and_match_without_practice_dummy() {
    let mut app = build_app_with_config(ServerNetworkConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        ..default()
    });
    app.finish();
    app.cleanup();
    app.update();
    assert!(app.world().contains_resource::<ResolvedMap>());
    let world = app.world_mut();
    let mut dummies = world.query_filtered::<Entity, With<TestDummy>>();
    assert_eq!(dummies.iter(world).count(), 0);
    let mut matches = world.query_filtered::<&MatchState, With<MatchRoot>>();
    assert_eq!(matches.iter(world).count(), 1);
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
