use super::*;

#[derive(Resource, Default)]
struct SessionScheduleTrace(Vec<&'static str>);

fn session_probe(label: &'static str) -> impl FnMut(ResMut<SessionScheduleTrace>) + 'static {
    move |mut trace: ResMut<SessionScheduleTrace>| trace.0.push(label)
}

#[test]
fn client_session_phases_have_one_explicit_lifecycle_order() {
    let mut app = App::new();
    app.init_resource::<SessionScheduleTrace>();
    configure_client_session_schedule(&mut app);
    app.add_systems(
        Update,
        (
            session_probe("materialize").in_set(ClientSessionSet::MaterializeConnection),
            session_probe("handshake").in_set(ClientSessionSet::Handshake),
            session_probe("transition").in_set(ClientSessionSet::Transition),
            session_probe("transition-enforcement").in_set(ClientSessionSet::EnforceTransition),
            session_probe("commands").in_set(ClientSessionSet::MatchCommands),
            session_probe("observe").in_set(ClientSessionSet::Observe),
            session_probe("session-enforcement").in_set(ClientSessionSet::EnforceSession),
        ),
    );
    crate::test_app::reject_owned_schedule_ambiguities(&mut app, Update);

    app.update();

    assert_eq!(
        app.world().resource::<SessionScheduleTrace>().0,
        vec![
            "materialize",
            "handshake",
            "transition",
            "transition-enforcement",
            "commands",
            "observe",
            "session-enforcement",
        ]
    );
}

#[derive(Component)]
struct CompleteConnectionFixture;

#[derive(Resource, Default)]
struct ObservedConnect(bool);

#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
fn observe_connect_after_materialization(
    trigger: On<Connect>,
    fixtures: Query<
        (),
        (
            With<CompleteConnectionFixture>,
            Without<Unlinked>,
            Without<Disconnected>,
        ),
    >,
    mut commands: Commands,
    mut observed: ResMut<ObservedConnect>,
) {
    observed.0 = fixtures.get(trigger.entity).is_ok();
    commands.entity(trigger.entity).insert(Connecting);
}

#[test]
fn deferred_client_connect_runs_after_the_complete_entity_is_materialized() {
    let mut app = App::new();
    app.init_resource::<ObservedConnect>()
        .add_observer(observe_connect_after_materialization)
        .add_systems(
            Update,
            (connect_spawned_clients, finish_spawned_client_connect).chain(),
        );
    let entity = app
        .world_mut()
        .spawn((
            PendingClientConnect,
            CompleteConnectionFixture,
            Unlinked::default(),
            Disconnected::default(),
        ))
        .id();

    app.update();

    assert!(app.world().resource::<ObservedConnect>().0);
    assert!(app.world().get::<PendingClientConnect>(entity).is_none());
}

#[test]
fn product_flow_is_the_only_owner_of_its_attempt_timeout() {
    let mut config = ClientNetworkConfig::new(1);
    config.transport = NetworkTransport::RoutedUdp;
    assert!(!owns_automatic_routed_recovery(&config));

    config.auto_connect = true;
    assert!(owns_automatic_routed_recovery(&config));
}
