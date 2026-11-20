use super::*;
use crate::{
    ClientJoinPhase, ClientJoinStatus, ClientLobbyFailure, ClientLobbyMembership,
    ClientNetworkConfig, RoutedClientPhase, RoutedClientSession, RuntimeLobbyTarget,
    connection_persistence::ConnectionsFileV1, server_select::MAX_RESOLVED_CANDIDATES,
    ui::surface::Surface,
};
use actions::{FlowUiAction, OverlayCommit, SessionObservation};
use bevy::{
    input::{keyboard::KeyboardInput, mouse::MouseScrollUnit, mouse::MouseWheel},
    ui::{InteractionDisabled, ScrollPosition},
};
use connection::{
    AttemptDeadlineExpiry, ConnectionStage, PendingConnection, accepted_observation,
    attempt_deadline_expiry, bound_resolved_candidates, candidate_time_share,
    connection_presentation, has_next_candidate, netcode_timeout_ceiling, validate_target,
};
use input::{
    dashboard_focus_neighbor, edited_value_mut, insert_editor_text, overlay_allows_button,
    previous_caret, queue_ui_action, repair_dashboard_focus,
};
use lightyear::prelude::client::Client;
use persistence::{local_load_error, startup_server_address};
use reducer::{accept_game_type_draft, favorite_focus_after_removal, rejection_flow_error};
use screens::{
    brawlers::{BRAWLERS_BACK_INDEX, BrawlersNotice, BrawlersRoot},
    connecting::ConnectingProgressSegment,
    dashboard::{
        DASHBOARD_BUILD_INDEX, DASHBOARD_GAME_INDEX, DASHBOARD_MENU_INDEX, DASHBOARD_PLAY_INDEX,
        DASHBOARD_PRACTICE_INDEX, DashboardButtonStyle, DashboardLayoutClass,
        DashboardNavigationDirection, dashboard_game_summary, dashboard_layout_class,
    },
    game_select::GameTypeSelectRoot,
    match_loading::match_loading_text,
    overlays::{FlowErrorRoot, RateLimitTryAgain},
    queue::{queue_cancel_presentation, queue_copy},
    results::MatchCompletionRoot,
    server_select::{EditingField, ServerSelectModel},
    shared::{FlowButton, FlowRoot},
};
use std::time::Duration;

fn flow_test_app() -> App {
    flow_test_app_with(|_| {})
}

fn flow_test_app_with(extend: impl FnOnce(&mut App)) -> App {
    let mut app = App::new();
    let mut config = ClientNetworkConfig::new(0x1234);
    config.transport = brawler::config::NetworkTransport::RoutedUdp;
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
        .add_message::<KeyboardInput>()
        .add_message::<MouseWheel>()
        .insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(config)
        .insert_resource(RoutedClientLifecycle::default())
        .insert_resource(ClientConnectionsPath(std::env::temp_dir().join(format!(
            "brawler-m03-flow-test-{}-connections.ron",
            std::process::id()
        ))))
        .add_plugins((
            brawler::modes::ModeRegistryPlugin,
            brawler::modes::BuiltInModeRegistrationsPlugin,
        ))
        .add_plugins(ClientFlowPlugin);
    extend(&mut app);
    brawler::test_app::finalize(&mut app);
    app.update();
    app
}

#[derive(Resource, Default)]
struct InjectedFlowActions(Option<PendingFlowActions>);

fn inject_flow_actions(
    mut injected: ResMut<InjectedFlowActions>,
    mut pending: ResMut<PendingFlowActions>,
) {
    if let Some(actions) = injected.0.take() {
        *pending = actions;
    }
}

fn flow_action_test_app() -> App {
    flow_action_test_app_with(|_| {})
}

fn flow_action_test_app_with(extend: impl FnOnce(&mut App)) -> App {
    let mut app = flow_test_app_with(extend);
    app.init_resource::<InjectedFlowActions>().add_systems(
        Update,
        inject_flow_actions
            .before(resolve_map_maker_action)
            .before(resolve_flow_action)
            .in_set(ClientFlowSet::ResolveFlowAction),
    );
    app
}

#[test]
fn client_flow_update_schedule_has_no_owned_ambiguities() {
    let mut app = flow_test_app();
    brawler::test_app::reject_owned_schedule_ambiguities(&mut app, Update);
    app.update();
}

fn enter_dashboard(app: &mut App) {
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Dashboard);
    app.update();
    assert_eq!(
        *app.world().resource::<State<ClientFlow>>().get(),
        ClientFlow::Dashboard
    );
}

fn select_first_game(app: &mut App, membership: &ClientLobbyMembership) {
    let game = &membership.game_types[0];
    *app.world_mut().resource_mut::<SelectedGameType>() = SelectedGameType {
        catalog_revision: Some(membership.catalog_revision),
        game_type_id: Some(game.id.clone()),
        configuration_revision: Some(game.configuration_revision),
    };
}

fn inject_action(app: &mut App, action: FlowUiAction) {
    app.world_mut().resource_mut::<InjectedFlowActions>().0 = Some(PendingFlowActions {
        ordinary: Some(action),
        ..PendingFlowActions::default()
    });
    app.update();
}

fn inject_session_observation(app: &mut App, observation: SessionObservation) {
    app.world_mut().resource_mut::<InjectedFlowActions>().0 = Some(PendingFlowActions {
        session: Some(observation),
        ..PendingFlowActions::default()
    });
    app.update();
}

#[test]
fn dashboard_layout_class_uses_effective_ui_space() {
    assert_eq!(
        dashboard_layout_class(1280.0, 720.0, 1.0),
        DashboardLayoutClass::Wide
    );
    assert_eq!(
        dashboard_layout_class(1280.0, 720.0, 1.4),
        DashboardLayoutClass::Compact
    );
    assert_eq!(
        dashboard_layout_class(640.0, 360.0, 0.8),
        DashboardLayoutClass::Compact
    );
    assert_eq!(
        dashboard_layout_class(1000.0, 640.0, 1.0),
        DashboardLayoutClass::Wide
    );
    assert_eq!(
        dashboard_layout_class(999.0, 640.0, 1.0),
        DashboardLayoutClass::Compact
    );
}

#[test]
fn dashboard_spatial_navigation_matches_wide_and_compact_layouts() {
    let all = [
        DASHBOARD_PLAY_INDEX,
        DASHBOARD_PRACTICE_INDEX,
        DASHBOARD_GAME_INDEX,
        DASHBOARD_BUILD_INDEX,
        DASHBOARD_MENU_INDEX,
    ];
    for class in [DashboardLayoutClass::Wide, DashboardLayoutClass::Compact] {
        for (from, direction, to) in [
            (
                DASHBOARD_GAME_INDEX,
                DashboardNavigationDirection::Down,
                DASHBOARD_PLAY_INDEX,
            ),
            (
                DASHBOARD_PLAY_INDEX,
                DashboardNavigationDirection::Down,
                DASHBOARD_PRACTICE_INDEX,
            ),
            (
                DASHBOARD_PRACTICE_INDEX,
                DashboardNavigationDirection::Up,
                DASHBOARD_PLAY_INDEX,
            ),
            (
                DASHBOARD_PLAY_INDEX,
                DashboardNavigationDirection::Up,
                DASHBOARD_GAME_INDEX,
            ),
            (
                DASHBOARD_PLAY_INDEX,
                DashboardNavigationDirection::Left,
                DASHBOARD_BUILD_INDEX,
            ),
            (
                DASHBOARD_BUILD_INDEX,
                DashboardNavigationDirection::Right,
                DASHBOARD_PLAY_INDEX,
            ),
        ] {
            assert_eq!(dashboard_focus_neighbor(class, from, direction, &all), to);
        }
    }
}

#[test]
fn dashboard_navigation_skips_disabled_targets_and_repairs_focus() {
    let available = [
        DASHBOARD_GAME_INDEX,
        DASHBOARD_BUILD_INDEX,
        DASHBOARD_MENU_INDEX,
    ];
    assert_eq!(
        dashboard_focus_neighbor(
            DashboardLayoutClass::Wide,
            DASHBOARD_PRACTICE_INDEX,
            DashboardNavigationDirection::Up,
            &available
        ),
        DASHBOARD_GAME_INDEX
    );
    assert_eq!(
        repair_dashboard_focus(DASHBOARD_PLAY_INDEX, &available),
        DASHBOARD_GAME_INDEX
    );
    assert_eq!(
        dashboard_focus_neighbor(
            DashboardLayoutClass::Wide,
            DASHBOARD_GAME_INDEX,
            DashboardNavigationDirection::Up,
            &[DASHBOARD_MENU_INDEX]
        ),
        DASHBOARD_MENU_INDEX
    );
}

fn count_flow_roots(app: &mut App) -> usize {
    let world = app.world_mut();
    let mut query = world.query_filtered::<Entity, With<FlowRoot>>();
    query.iter(world).count()
}

fn count_error_roots(app: &mut App) -> usize {
    let world = app.world_mut();
    let mut query = world.query_filtered::<Entity, With<FlowErrorRoot>>();
    query.iter(world).count()
}

fn visible_text(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut query = world.query::<&Text>();
    query.iter(world).map(|text| text.0.clone()).collect()
}

fn press_flow_button(app: &mut App, action: &FlowUiAction) {
    let entity = {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &FlowButton)>();
        query
            .iter(world)
            .find_map(|(entity, button)| (&button.action == action).then_some(entity))
            .unwrap_or_else(|| panic!("missing rendered flow button for {action:?}"))
    };
    app.world_mut()
        .entity_mut(entity)
        .insert(Interaction::Pressed);
    app.update();
    // Shell actions fire on an undragged release over the control.
    if let Ok(mut entity) = app.world_mut().get_entity_mut(entity) {
        entity.insert(Interaction::Hovered);
    }
    app.update();
}

fn lobby_membership() -> ClientLobbyMembership {
    let account_id = brawler::profiles::AccountId::new(1).unwrap();
    let catalog = brawler::profiles::AdvertisedCharacterCatalog::embedded().unwrap();
    ClientLobbyMembership {
        logical_server_id: 1,
        player_id: brawler::protocol::PlayerId(1),
        accepted_display_name: "Player".to_string(),
        server_name: "Test Lobby".to_string(),
        catalog_revision: brawler::lobby::CatalogRevision([1; 32]),
        game_types: vec![brawler::lobby::AdvertisedGameType {
            id: brawler::lobby::GameTypeId::new("wipeout-2v2").unwrap(),
            configuration_revision: 1,
            display_name: "Wipeout 2v2".to_string(),
            mode_definition_id: brawler::map::WIPEOUT_MODE_DEFINITION,
            map_preset_ids: vec![brawler::map::MapPresetId(1)],
            practice_map_preset_ids: vec![brawler::map::MapPresetId(1)],
            map_authoring_limits: Some(brawler::map::AUTHORING_LIMITS),
            community_maps: false,
            team_count: 2,
            players_per_team: 2,
            rules_summary: brawler::lobby::AdvertisedRulesSummary::Wipeout {
                target_score: 10,
                active_limit_ticks: 3_600,
            },
        }],
        profile: catalog.profile_from_stored(
            account_id,
            brawler::profiles::ProfileRevision::INITIAL,
            None,
            [],
        ),
        character_catalog: catalog,
    }
}

#[test]
fn explicit_action_preempts_session_and_ordinary_while_profile_decision_still_applies() {
    let mut app = flow_action_test_app();
    let membership = lobby_membership();
    let session = app
        .world_mut()
        .spawn((
            Client,
            membership.clone(),
            RoutedClientSession {
                generation: 1,
                kind: super::super::RoutedClientSessionKind::Lobby,
            },
        ))
        .id();
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_snapshot_for_test(membership.profile);
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_decision_for_test(brawler::profiles::ProfileDecision::InvalidRequest);
    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Brawlers;
    app.world_mut().resource_mut::<InjectedFlowActions>().0 = Some(PendingFlowActions {
        session: Some(SessionObservation::ReservationStarted),
        explicit: Some(FlowUiAction::Cancel),
        ordinary: Some(FlowUiAction::OpenSettings),
    });

    app.update();

    assert!(app.world().get_entity(session).is_err());
    assert!(
        app.world()
            .resource::<BrawlersNotice>()
            .0
            .as_deref()
            .is_some_and(|error| error.contains("does not offer")),
        "the profile decision is processed before action precedence"
    );
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::None
    );

    app.update();
    assert_eq!(
        *app.world().resource::<State<ClientFlow>>().get(),
        ClientFlow::ServerSelect,
        "explicit cancellation wins over the reservation and settings actions"
    );
    assert_eq!(count_flow_roots(&mut app), 1);
}

fn configured_lobby_flow_app(membership: ClientLobbyMembership) -> App {
    configured_lobby_flow_app_with(membership, |_| {})
}

fn configured_lobby_flow_app_with(
    membership: ClientLobbyMembership,
    extend: impl FnOnce(&mut App),
) -> App {
    let mut app = flow_action_test_app_with(extend);
    enter_dashboard(&mut app);
    select_first_game(&mut app, &membership);
    app.world_mut().spawn((
        Client,
        membership,
        RoutedClientSession {
            generation: 1,
            kind: super::super::RoutedClientSessionKind::Lobby,
        },
    ));
    app.world_mut()
        .resource_mut::<super::super::ClientPracticeModel>()
        .bind_generation_for_test(1);
    app.world_mut()
        .resource_mut::<super::super::ClientQueueModel>()
        .bind_lobby_generation(1);
    app
}

fn match_result_context(
    game_type_id: brawler::lobby::GameTypeId,
) -> super::super::ClientMatchResultContext {
    super::super::ClientMatchResultContext {
        result: brawler::matchplay::MatchResult::Draw,
        local_team: None,
        game_type_id: Some(game_type_id),
        game_name: None,
        final_score: None,
        wipeout: None,
        objective: None,
        local_player: None,
        published_map: None,
    }
}

fn queue_membership(membership: &ClientLobbyMembership) -> brawler::lobby::QueueMembership {
    brawler::lobby::QueueMembership {
        ticket_id: brawler::lobby::QueueTicketId::new(1).unwrap(),
        catalog_revision: membership.catalog_revision,
        game_type_id: membership.game_types[0].id.clone(),
        game_type_configuration_revision: membership.game_types[0].configuration_revision,
        profile_revision: membership.profile.revision,
        accepted_build: brawler::builds::AcceptedBuildSummary {
            character: membership.profile.selection(),
            identity: brawler::builds::SelectedBuild {
                recipe_fingerprint: brawler::builds::BuildRecipeFingerprint(1),
                revision: brawler::builds::BuildCatalog::embedded()
                    .unwrap()
                    .balance_revision,
            },
        },
        admitted_at_pool_state_revision: 1,
    }
}

#[test]
fn match_transition_observations_preserve_loading_match_and_overlay_commits() {
    let mut app = configured_lobby_flow_app(lobby_membership());

    inject_session_observation(&mut app, SessionObservation::ReservationStarted);
    let commit = app.world().resource::<FlowCommit>();
    assert_eq!(commit.next_flow, Some(ClientFlow::MatchLoading));
    assert_eq!(commit.overlay, Some(OverlayCommit::Clear));

    inject_session_observation(&mut app, SessionObservation::CountdownObserved);
    let commit = app.world().resource::<FlowCommit>();
    assert_eq!(commit.next_flow, Some(ClientFlow::Match));
    assert_eq!(commit.overlay, Some(OverlayCommit::Clear));
}

fn current_flow(app: &App) -> ClientFlow {
    *app.world().resource::<State<ClientFlow>>().get()
}

/// Apply one lobby matchmaking phase through the real loading model, then run one frame.
fn observe_matchmaking_phase(app: &mut App, phase: brawler::lobby::MatchmakingServerPhase) {
    app.world_mut().resource_scope(
        |world, mut loading: Mut<super::super::ClientMatchLoadingModel>| {
            world.resource_scope(
                |world, mut practice: Mut<super::super::ClientPracticeModel>| {
                    let mut lifecycle = world.resource_mut::<RoutedClientLifecycle>();
                    loading.observe_phase_for_test(&mut practice, &mut lifecycle, phase);
                },
            );
        },
    );
    app.update();
}

/// BRL-k0tr: a queue (`PvP`) handoff reaches the match even when the lobby removes the reservation
/// after this client already accepted its match grant (another participant's lobby session
/// closed first). Before the fix the stale removal committed the Dashboard under the match.
#[test]
fn a_queue_handoff_reaches_the_match_even_after_a_late_reservation_removal() {
    let membership = lobby_membership();
    let joined = queue_membership(&membership);
    let mut app = configured_lobby_flow_app_with(membership, |app| {
        app.insert_resource(super::super::ClientInputContext::Shell);
    });
    app.world_mut()
        .resource_mut::<RoutedClientLifecycle>()
        .start_lobby();

    inject_session_observation(
        &mut app,
        SessionObservation::QueueOutcome(brawler::lobby::QueueCommandOutcome {
            request_id: brawler::lobby::QueueRequestId::new(1).unwrap(),
            decision: brawler::lobby::QueueDecision::Joined(joined.clone()),
        }),
    );
    app.update();
    assert_eq!(current_flow(&app), ClientFlow::Queue);

    let reservation_id = brawler::lobby::MatchReservationId::new(9).unwrap();
    observe_matchmaking_phase(
        &mut app,
        brawler::lobby::MatchmakingServerPhase::ReservationStarted(
            brawler::lobby::ReservationStarted {
                reservation_id,
                ticket_id: Some(joined.ticket_id),
                game_type_id: joined.game_type_id.clone(),
                mode_definition_id: brawler::map::WIPEOUT_MODE_DEFINITION,
                map_preset_id: Some(brawler::map::MapPresetId(1)),
                map_recipe_id: brawler::map::MapRecipeId(1),
                map_recipe_revision: 1,
                map_recipe_fingerprint: brawler::map::MapRecipeFingerprint(1),
                team_count: 2,
                players_per_team: 1,
                accepted_build: joined.accepted_build,
                loading_deadline_millis: 30_000,
                published_map: None,
            },
        ),
    );
    app.update();
    assert_eq!(current_flow(&app), ClientFlow::MatchLoading);

    let grant = brawler::protocol::MatchRouteGrant {
        request_id: brawler_routing::RequestId::new(1).unwrap(),
        allocation_id: brawler_routing::AllocationId::new(2).unwrap(),
        match_id: brawler_routing::MatchId::new(3).unwrap(),
        route_id: brawler_routing::RouteId::new(4).unwrap(),
        peer_id: brawler_routing::PeerId::new(5).unwrap(),
        game_mode: brawler::config::GameMode::Wipeout,
        capability: brawler::protocol::RouteCapability::from_bytes([7; 32]).unwrap(),
        activation_expiry_unix_ms: 10,
        route_expiry_unix_ms: 20,
    };
    observe_matchmaking_phase(
        &mut app,
        brawler::lobby::MatchmakingServerPhase::BeginMatchConnect(
            brawler::lobby::BeginMatchConnect {
                reservation_id,
                generation: 1,
                grant,
            },
        ),
    );
    assert_eq!(
        app.world()
            .resource::<RoutedClientLifecycle>()
            .accepted_grant,
        Some(grant)
    );

    // The other participant's lobby session closed first; the lobby removes the reservation and
    // this still-open lobby session receives the removal after the grant.
    observe_matchmaking_phase(
        &mut app,
        brawler::lobby::MatchmakingServerPhase::Removed {
            reservation_id,
            ticket_id: Some(joined.ticket_id),
            reason: brawler::lobby::MatchStartFailure::ParticipantLost,
        },
    );
    app.update();
    assert_eq!(
        current_flow(&app),
        ClientFlow::MatchLoading,
        "a removal after the match grant must not return the flow to the Dashboard"
    );

    // The match session then reaches its countdown and the match is presented.
    app.world_mut().spawn((
        brawler::matchplay::MatchRoot,
        brawler::matchplay::MatchState {
            match_id: brawler::matchplay::MatchId(3),
            mode_definition_id: brawler::map::WIPEOUT_MODE_DEFINITION,
            phase: brawler::matchplay::MatchPhase::Countdown { starts_at_tick: 60 },
            rules_revision: 1,
        },
    ));
    app.update();
    app.update();
    assert_eq!(current_flow(&app), ClientFlow::Match);
}

#[test]
fn queue_outcomes_preserve_join_cancel_and_stale_content_policies() {
    let membership = lobby_membership();
    let joined = queue_membership(&membership);
    let mut app = configured_lobby_flow_app(membership);
    app.world_mut()
        .resource_mut::<super::super::ClientMatchResultState>()
        .context = Some(match_result_context(joined.game_type_id.clone()));

    inject_session_observation(
        &mut app,
        SessionObservation::QueueOutcome(brawler::lobby::QueueCommandOutcome {
            request_id: brawler::lobby::QueueRequestId::new(1).unwrap(),
            decision: brawler::lobby::QueueDecision::Joined(joined.clone()),
        }),
    );
    assert!(
        app.world()
            .resource::<super::super::ClientMatchResultState>()
            .context
            .is_none()
    );
    assert_eq!(
        app.world().resource::<SelectedGameType>().game_type_id,
        Some(joined.game_type_id.clone())
    );
    assert_eq!(
        app.world().resource::<FlowCommit>().next_flow,
        Some(ClientFlow::Queue)
    );

    *app.world_mut().resource_mut::<SessionPurpose>() = SessionPurpose::Practice;
    inject_session_observation(
        &mut app,
        SessionObservation::QueueOutcome(brawler::lobby::QueueCommandOutcome {
            request_id: brawler::lobby::QueueRequestId::new(2).unwrap(),
            decision: brawler::lobby::QueueDecision::Cancelled {
                ticket_id: joined.ticket_id,
                resulting_pool_state_revision: 2,
            },
        }),
    );
    assert_eq!(
        *app.world().resource::<SessionPurpose>(),
        SessionPurpose::Multiplayer
    );
    assert_eq!(
        app.world().resource::<DashboardReturnFocus>().0,
        Some(DASHBOARD_PLAY_INDEX)
    );
    assert_eq!(
        app.world().resource::<FlowCommit>().next_flow,
        Some(ClientFlow::Dashboard)
    );

    inject_session_observation(
        &mut app,
        SessionObservation::QueueOutcome(brawler::lobby::QueueCommandOutcome {
            request_id: brawler::lobby::QueueRequestId::new(3).unwrap(),
            decision: brawler::lobby::QueueDecision::Rejected(
                brawler::lobby::QueueRejection::StaleCatalog,
            ),
        }),
    );
    let commit = app.world().resource::<FlowCommit>();
    assert!(commit.teardown);
    let error = commit.error.as_ref().unwrap();
    assert_eq!(error.kind, FlowErrorKind::Content);
    assert_eq!(error.return_flow, ClientFlow::ServerSelect);
    assert!(error.message.contains("content changed incompatibly"));
}

#[test]
fn practice_rejections_distinguish_stale_content_from_ordinary_failures() {
    let mut stale = configured_lobby_flow_app(lobby_membership());
    inject_session_observation(
        &mut stale,
        SessionObservation::PracticeRejected(brawler::lobby::PracticeStartRejection::StaleCatalog),
    );
    let commit = stale.world().resource::<FlowCommit>();
    assert!(commit.teardown);
    assert_eq!(commit.error.as_ref().unwrap().kind, FlowErrorKind::Content);
    assert_eq!(
        commit.error.as_ref().unwrap().return_flow,
        ClientFlow::ServerSelect
    );

    let mut ordinary = configured_lobby_flow_app(lobby_membership());
    inject_session_observation(
        &mut ordinary,
        SessionObservation::PracticeRejected(brawler::lobby::PracticeStartRejection::Busy),
    );
    let commit = ordinary.world().resource::<FlowCommit>();
    assert!(!commit.teardown);
    let error = commit.error.as_ref().unwrap();
    assert_eq!(error.kind, FlowErrorKind::Practice);
    assert_eq!(error.return_flow, ClientFlow::Dashboard);
    assert_eq!(error.message, "Another match start is already in progress.");
}

#[test]
fn match_failure_and_fresh_lobby_return_preserve_result_and_one_shot_error_paths() {
    let membership = lobby_membership();
    let game_type_id = membership.game_types[0].id.clone();
    let mut failed = configured_lobby_flow_app(membership.clone());
    failed
        .world_mut()
        .resource_mut::<super::super::ClientMatchResultState>()
        .context = Some(match_result_context(game_type_id.clone()));
    failed
        .world_mut()
        .resource_mut::<RoutedClientLifecycle>()
        .phase = RoutedClientPhase::Match;

    inject_session_observation(&mut failed, SessionObservation::MatchFailed);
    assert!(
        failed
            .world()
            .resource::<super::super::ClientMatchResultState>()
            .context
            .is_none()
    );
    assert_eq!(
        failed.world().resource::<RoutedClientLifecycle>().phase,
        RoutedClientPhase::AwaitingMatchUnlink
    );
    inject_session_observation(&mut failed, SessionObservation::FreshLobbyReturn);
    let commit = failed.world().resource::<FlowCommit>();
    assert_eq!(commit.next_flow, Some(ClientFlow::Dashboard));
    assert_eq!(commit.overlay, Some(OverlayCommit::Clear));
    let error = commit.error.as_ref().unwrap();
    assert_eq!(error.kind, FlowErrorKind::Connection);
    assert_eq!(
        error.message,
        "The connection to the match was lost, and the match could not be resumed."
    );
    assert_eq!(error.return_flow, ClientFlow::Dashboard);
    assert_eq!(error.actions, [Some(FlowErrorAction::Back), None]);
    assert_eq!(
        *failed.world().resource::<SessionPurpose>(),
        SessionPurpose::Multiplayer
    );
    assert_eq!(
        failed.world().resource::<DashboardReturnFocus>().0,
        Some(DASHBOARD_PLAY_INDEX)
    );
    inject_session_observation(&mut failed, SessionObservation::FreshLobbyReturn);
    let commit = failed.world().resource::<FlowCommit>();
    assert!(commit.error.is_none());
    assert_eq!(commit.overlay, Some(OverlayCommit::Clear));

    let mut completed = configured_lobby_flow_app(membership);
    completed
        .world_mut()
        .resource_mut::<super::super::ClientMatchResultState>()
        .context = Some(match_result_context(game_type_id.clone()));
    *completed.world_mut().resource_mut::<SelectedGameType>() = SelectedGameType::default();
    *completed.world_mut().resource_mut::<SessionPurpose>() = SessionPurpose::Practice;
    inject_session_observation(&mut completed, SessionObservation::FreshLobbyReturn);
    let commit = completed.world().resource::<FlowCommit>();
    assert_eq!(commit.next_flow, Some(ClientFlow::Results));
    assert_eq!(commit.overlay, Some(OverlayCommit::Clear));
    assert!(commit.error.is_none());
    assert_eq!(
        *completed.world().resource::<SessionPurpose>(),
        SessionPurpose::Practice
    );
    assert_eq!(
        completed
            .world()
            .resource::<super::super::ClientMatchResultState>()
            .context
            .as_ref()
            .unwrap()
            .game_name
            .as_deref(),
        Some("Wipeout 2v2")
    );
    assert_eq!(
        completed
            .world()
            .resource::<SelectedGameType>()
            .game_type_id,
        Some(game_type_id)
    );
}

/// BRL-jxfy: a player whose match completed while their seat was held rejoins it in Completed.
/// The rejoin is presented as the match, the accepted rejoin resolves the earlier loss, the
/// completed snapshot captures the result context, and the fresh lobby return shows Results with
/// no "could not be resumed" message. The snapshot may also arrive in the same frame as the
/// accepted rejoin, before the flow observes it.
#[test]
fn a_rejoin_into_a_completed_match_reaches_results_without_the_lost_match_message() {
    let membership = lobby_membership();
    let game_type_id = membership.game_types[0].id.clone();
    for snapshot_with_rejoin in [false, true] {
        let mut app = configured_lobby_flow_app(membership.clone());
        app.world_mut()
            .insert_resource(super::super::ClientInputContext::Shell);
        inject_session_observation(&mut app, SessionObservation::RejoinStarted);
        assert_eq!(
            app.world().resource::<FlowCommit>().next_flow,
            Some(ClientFlow::Match)
        );
        let context = || match_result_context(game_type_id.clone());
        if snapshot_with_rejoin {
            app.world_mut()
                .resource_mut::<super::super::ClientMatchResultState>()
                .context = Some(context());
        }
        app.world_mut()
            .resource_mut::<RoutedClientLifecycle>()
            .rejoined = true;
        inject_session_observation(&mut app, SessionObservation::MatchRejoined);
        assert!(!app.world().resource::<RoutedClientLifecycle>().rejoined);
        assert!(app.world().resource::<FlowCommit>().error.is_none());
        if !snapshot_with_rejoin {
            app.world_mut()
                .resource_mut::<super::super::ClientMatchResultState>()
                .context = Some(context());
        }
        inject_session_observation(&mut app, SessionObservation::FreshLobbyReturn);
        let commit = app.world().resource::<FlowCommit>();
        assert_eq!(
            commit.next_flow,
            Some(ClientFlow::Results),
            "snapshot with rejoin: {snapshot_with_rejoin}"
        );
        assert!(
            commit.error.is_none(),
            "a resolved rejoin shows no lost-match message"
        );
    }
}

fn configured_customization_app(membership: &ClientLobbyMembership) -> App {
    let mut app = configured_lobby_flow_app(membership.clone());
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_snapshot_for_test(membership.profile.clone());
    app
}

#[test]
fn pending_practice_rejects_queue_join() {
    let mut practice_first = configured_lobby_flow_app(lobby_membership());

    inject_action(&mut practice_first, FlowUiAction::StartPractice);
    assert!(
        practice_first
            .world()
            .resource::<super::super::ClientPracticeModel>()
            .pending()
    );
    inject_action(&mut practice_first, FlowUiAction::JoinQueue);
    assert!(
        practice_first
            .world()
            .resource::<super::super::ClientQueueModel>()
            .pending()
            .is_none()
    );
}

#[test]
fn pending_queue_rejects_practice_start() {
    let mut queue_first = configured_lobby_flow_app(lobby_membership());

    inject_action(&mut queue_first, FlowUiAction::JoinQueue);
    assert!(
        queue_first
            .world()
            .resource::<super::super::ClientQueueModel>()
            .pending()
            .is_some()
    );
    inject_action(&mut queue_first, FlowUiAction::StartPractice);
    assert!(
        !queue_first
            .world()
            .resource::<super::super::ClientPracticeModel>()
            .pending()
    );
}

#[test]
fn validated_target_freezes_canonical_address_and_normalized_name() {
    let target = validate_target("LOCALHOST", " Cafe\u{301} ").unwrap();
    assert_eq!(target.logical_address.canonical(), "localhost:5000");
    assert_eq!(target.proposed_display_name, "Café");
}

#[test]
fn startup_server_precedence_is_explicit_then_recent_then_product_default() {
    let mut config = ClientNetworkConfig::new(7);
    let mut connections = ConnectionsFileV1::empty();
    assert_eq!(
        startup_server_address(&config, &connections),
        "127.0.0.1:5000"
    );

    connections
        .record_recent("Last Success", "recent.example:6000")
        .unwrap();
    assert_eq!(
        startup_server_address(&config, &connections),
        "recent.example:6000"
    );

    config.product_server_prefill = Some("explicit.example:7000".to_string());
    assert_eq!(
        startup_server_address(&config, &connections),
        "explicit.example:7000"
    );
}

#[test]
fn rendered_server_select_connect_button_starts_connection() {
    let mut app = flow_test_app();
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::ServerSelect);
    app.update();

    press_flow_button(&mut app, &FlowUiAction::Connect);

    assert!(app.world().contains_resource::<PendingConnection>());
    app.update();
    assert_eq!(
        *app.world().resource::<State<ClientFlow>>().get(),
        ClientFlow::Connecting
    );
}

#[test]
fn rendered_dashboard_menu_buttons_dispatch_their_actions() {
    let mut app = flow_test_app();
    app.world_mut().spawn((Client, lobby_membership()));
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Dashboard);
    app.update();

    press_flow_button(&mut app, &FlowUiAction::OpenDashboardMenu);
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::DashboardMenu
    );
    app.update();

    press_flow_button(&mut app, &FlowUiAction::OpenCredits);
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::Credits
    );
}

#[test]
fn change_server_confirmation_clears_before_server_select_connect() {
    let mut app = flow_test_app();
    app.world_mut().spawn((
        Client,
        lobby_membership(),
        RoutedClientSession {
            generation: 1,
            kind: super::super::RoutedClientSessionKind::Lobby,
        },
        RuntimeLobbyTarget {
            logical_address: "127.0.0.1:5000".to_string(),
            proposed_display_name: "Player".to_string(),
        },
    ));
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Dashboard);
    app.update();

    press_flow_button(&mut app, &FlowUiAction::OpenDashboardMenu);
    app.update();
    press_flow_button(&mut app, &FlowUiAction::RequestChangeServer);
    app.update();
    press_flow_button(&mut app, &FlowUiAction::ConfirmChangeServer);
    app.update();

    assert_eq!(
        *app.world().resource::<State<ClientFlow>>().get(),
        ClientFlow::ServerSelect
    );
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::None
    );

    press_flow_button(&mut app, &FlowUiAction::Connect);
    app.update();
    assert_eq!(
        *app.world().resource::<State<ClientFlow>>().get(),
        ClientFlow::Connecting
    );
}

#[test]
fn dashboard_menu_omits_favorite_without_a_real_server_target() {
    let mut app = flow_test_app();
    let clients = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<Client>>();
        query.iter(world).collect::<Vec<_>>()
    };
    for entity in clients {
        app.world_mut().despawn(entity);
    }
    app.world_mut().spawn((Client, lobby_membership()));
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Dashboard);
    app.update();

    press_flow_button(&mut app, &FlowUiAction::OpenDashboardMenu);
    app.update();

    let world = app.world_mut();
    let mut query = world.query::<&FlowButton>();
    assert!(
        !query
            .iter(world)
            .any(|button| button.action == FlowUiAction::ToggleFavoriteServer)
    );
}

#[test]
fn dashboard_mode_card_summary_contains_only_mode_and_team_size() {
    let game = lobby_membership().game_types.remove(0);
    let summary = dashboard_game_summary(&game);
    assert!(game.display_name.contains("Wipeout"));
    assert!(summary.contains("WIPEOUT"));
    assert!(!summary.contains("Map pool:"));
    assert!(!summary.contains("Selected map"));
}

#[test]
fn dashboard_actions_have_explicit_fact_based_accessible_labels() {
    let mut app = flow_test_app();
    app.world_mut().spawn((Client, lobby_membership()));
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Dashboard);
    app.update();

    let world = app.world_mut();
    let mut query = world.query::<(&DashboardButtonStyle, &AccessibleLabel)>();
    let labels = query
        .iter(world)
        .map(|(style, label)| (*style, label.0.clone()))
        .collect::<Vec<_>>();
    assert_eq!(labels.len(), 6);
    assert!(labels.iter().all(|(_, label)| !label.trim().is_empty()));
    assert!(labels.iter().any(|(style, label)| {
        matches!(style, DashboardButtonStyle::Preview) && label.starts_with("View brawlers: Riff")
    }));
    assert!(labels.iter().any(|(style, label)| {
        matches!(style, DashboardButtonStyle::Mode)
            && !label.contains("Map pool:")
            && !label.contains("Selected map")
    }));
    assert!(
        labels.iter().any(|(style, label)| {
            matches!(style, DashboardButtonStyle::Play) && label == "Play"
        })
    );
}

#[test]
fn flow_has_the_v5_connected_state_set() {
    let states = [
        ClientFlow::Connecting,
        ClientFlow::ServerSelect,
        ClientFlow::Dashboard,
        ClientFlow::GameTypeSelect,
        ClientFlow::Queue,
        ClientFlow::MatchLoading,
        ClientFlow::Match,
        ClientFlow::Results,
    ];
    assert_eq!(states.len(), 8);
}

#[test]
fn match_flow_hands_input_to_gameplay_and_returns_it_to_the_shell() {
    let mut app = flow_test_app();
    app.world_mut()
        .insert_resource(super::super::ClientInputContext::Shell);

    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Match);
    app.update();
    assert_eq!(
        *app.world().resource::<super::super::ClientInputContext>(),
        super::super::ClientInputContext::Gameplay
    );

    *app.world_mut()
        .resource_mut::<super::super::ClientInputContext>() =
        super::super::ClientInputContext::Menu;
    app.update();
    assert_eq!(
        *app.world().resource::<super::super::ClientInputContext>(),
        super::super::ClientInputContext::Menu
    );

    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::GameTypeSelect);
    app.update();
    assert_eq!(
        *app.world().resource::<super::super::ClientInputContext>(),
        super::super::ClientInputContext::Shell
    );
}

#[test]
fn completed_match_stays_covered_until_the_fresh_lobby_is_ready() {
    let mut app = flow_test_app();
    app.world_mut()
        .insert_resource(super::super::ClientInputContext::Shell);
    let match_root = app
        .world_mut()
        .spawn((
            brawler::matchplay::MatchRoot,
            brawler::matchplay::MatchState {
                match_id: brawler::matchplay::MatchId(9),
                mode_definition_id: brawler::map::WIPEOUT_MODE_DEFINITION,
                phase: brawler::matchplay::MatchPhase::Completed {
                    completed_at_tick: 12,
                    restart_unlocked_at_tick: 72,
                    result: brawler::matchplay::MatchResult::Draw,
                },
                rules_revision: 1,
            },
        ))
        .id();
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Match);
    app.update();

    let completion_root = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<MatchCompletionRoot>>();
        query.single(world).unwrap()
    };
    assert!(visible_text(&mut app).iter().any(|text| text == "DRAW"));

    app.world_mut().entity_mut(match_root).despawn();
    app.update();
    assert!(app.world().get_entity(completion_root).is_ok());

    app.world_mut().spawn((
        Client,
        lobby_membership(),
        RoutedClientSession {
            generation: 3,
            kind: super::super::RoutedClientSessionKind::Lobby,
        },
        ClientJoinStatus {
            phase: ClientJoinPhase::LobbyActive {
                player_id: brawler::protocol::PlayerId(1),
            },
            started_at: Duration::ZERO,
            disconnect_requested: false,
        },
    ));
    app.world_mut()
        .resource_mut::<RoutedClientLifecycle>()
        .generation = 3;
    app.update();
    app.update();

    assert_eq!(
        *app.world().resource::<State<ClientFlow>>().get(),
        ClientFlow::Dashboard
    );
    assert!(app.world().get_entity(completion_root).is_err());
    assert_eq!(count_flow_roots(&mut app), 1);
}

#[test]
#[allow(clippy::too_many_lines)]
fn results_replay_uses_the_exact_fresh_lobby_game() {
    let mut app = flow_test_app();
    app.world_mut()
        .insert_resource(super::super::ClientInputContext::Shell);
    let lobby = lobby_membership();
    let game_type_id = lobby.game_types[0].id.clone();
    app.world_mut().spawn((
        Client,
        lobby,
        RoutedClientSession {
            generation: 3,
            kind: super::super::RoutedClientSessionKind::Lobby,
        },
        ClientJoinStatus {
            phase: ClientJoinPhase::LobbyActive {
                player_id: brawler::protocol::PlayerId(1),
            },
            started_at: Duration::ZERO,
            disconnect_requested: false,
        },
    ));
    app.world_mut().spawn((
        Client,
        RoutedClientSession {
            generation: 2,
            kind: super::super::RoutedClientSessionKind::Match,
        },
        ClientJoinStatus {
            phase: ClientJoinPhase::Disconnected,
            started_at: Duration::ZERO,
            disconnect_requested: true,
        },
    ));
    app.world_mut()
        .resource_mut::<RoutedClientLifecycle>()
        .generation = 3;
    {
        let mut result = app
            .world_mut()
            .resource_mut::<super::super::ClientMatchResultState>();
        result.last_accepted_game_type_id = Some(game_type_id.clone());
        result.context = Some(super::super::ClientMatchResultContext {
            result: brawler::matchplay::MatchResult::Draw,
            local_team: None,
            game_type_id: Some(game_type_id.clone()),
            game_name: None,
            final_score: None,
            wipeout: None,
            objective: None,
            local_player: None,
            published_map: None,
        });
    }
    *app.world_mut().resource_mut::<SelectedGameType>() = SelectedGameType::default();
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Match);
    app.update();
    app.update();

    assert_eq!(
        *app.world().resource::<State<ClientFlow>>().get(),
        ClientFlow::Results
    );
    assert_eq!(
        app.world()
            .resource::<SelectedGameType>()
            .game_type_id
            .as_ref(),
        Some(&game_type_id)
    );

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
    app.update();

    let pending = app
        .world()
        .resource::<super::super::ClientQueueModel>()
        .pending()
        .expect("Queue Again should create a fresh Join");
    assert!(matches!(
        &pending.command,
        brawler::lobby::QueueCommand::Join(command) if command.game_type_id == game_type_id
    ));
    assert_eq!(
        app.world()
            .resource::<SelectedGameType>()
            .game_type_id
            .as_ref(),
        Some(&game_type_id)
    );
}

#[test]
fn returning_to_game_select_preserves_a_still_advertised_game() {
    let mut app = flow_test_app();
    let mut lobby = lobby_membership();
    let mut second = lobby.game_types[0].clone();
    second.id = brawler::lobby::GameTypeId::new("hot-zone-2v2").unwrap();
    second.configuration_revision = 2;
    second.display_name = "Hot Zone 2v2".to_string();
    second.mode_definition_id = brawler::map::HOT_ZONE_MODE_DEFINITION;
    second.rules_summary = brawler::lobby::AdvertisedRulesSummary::HotZone {
        target_progress_ticks: 1800,
        active_limit_ticks: 10800,
    };
    lobby.game_types.push(second.clone());
    app.world_mut().spawn((Client, lobby.clone()));
    *app.world_mut().resource_mut::<SelectedGameType>() = SelectedGameType {
        catalog_revision: Some(lobby.catalog_revision),
        game_type_id: Some(second.id.clone()),
        configuration_revision: Some(second.configuration_revision),
    };

    *app.world_mut().resource_mut::<GameTypeSelectionDraft>() = GameTypeSelectionDraft {
        selected_index: Some(1),
        unavailable_previous: false,
    };
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::GameTypeSelect);
    app.update();

    assert_eq!(app.world().resource::<FlowNavigation>().focused, 1);
    assert_eq!(
        app.world()
            .resource::<SelectedGameType>()
            .game_type_id
            .as_ref(),
        Some(&second.id)
    );
}

#[test]
fn game_type_select_scrolls_long_catalog_and_keeps_confirm_available() {
    let mut app = flow_test_app();
    let mut lobby = lobby_membership();
    let prototype = lobby.game_types[0].clone();
    lobby.game_types = (0..brawler::lobby::MAX_GAME_TYPES)
        .map(|index| {
            let mut game = prototype.clone();
            game.id = brawler::lobby::GameTypeId::new(format!("test-game-{index}"))
                .expect("bounded test game ID");
            game.display_name = format!("Test Game {index}");
            game
        })
        .collect();
    app.world_mut().spawn((Client, lobby));
    *app.world_mut().resource_mut::<GameTypeSelectionDraft>() = GameTypeSelectionDraft {
        selected_index: Some(brawler::lobby::MAX_GAME_TYPES - 1),
        unavailable_previous: false,
    };
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::GameTypeSelect);
    app.update();

    let (root, confirm_disabled) = {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<Entity, With<GameTypeSelectRoot>>();
        let root = roots.single(world).unwrap();
        let mut buttons = world.query::<(&FlowButton, Has<InteractionDisabled>)>();
        let confirm_disabled = buttons
            .iter(world)
            .find(|(button, _)| button.action == FlowUiAction::ConfirmGameType)
            .map(|(_, disabled)| disabled)
            .expect("Confirm button is rendered");
        (root, confirm_disabled)
    };
    assert!(!confirm_disabled);
    assert!(app.world().get::<ScrollPosition>(root).is_some());

    app.world_mut().write_message(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: -2.0,
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    app.update();
    assert!((app.world().get::<ScrollPosition>(root).unwrap().0.y - 48.0).abs() <= f32::EPSILON);
}

#[test]
fn game_type_child_drafts_then_discards_or_confirms() {
    let mut lobby = lobby_membership();
    let first = lobby.game_types[0].clone();
    let mut second = first.clone();
    second.id = brawler::lobby::GameTypeId::new("hot-zone-2v2").unwrap();
    second.configuration_revision = 2;
    second.display_name = "Hot Zone 2v2".to_string();
    second.mode_definition_id = brawler::map::HOT_ZONE_MODE_DEFINITION;
    lobby.game_types.push(second.clone());
    let mut selection = SelectedGameType {
        catalog_revision: Some(lobby.catalog_revision),
        game_type_id: Some(first.id.clone()),
        configuration_revision: Some(first.configuration_revision),
    };
    let draft = GameTypeSelectionDraft {
        selected_index: Some(1),
        unavailable_previous: false,
    };

    // Merely editing or discarding the draft cannot mutate the accepted selection.
    assert_eq!(selection.game_type_id.as_ref(), Some(&first.id));
    let discarded = GameTypeSelectionDraft::default();
    assert_eq!(discarded.selected_index, None);
    assert_eq!(selection.game_type_id.as_ref(), Some(&first.id));

    assert!(accept_game_type_draft(&draft, &lobby, &mut selection));
    assert_eq!(selection.game_type_id.as_ref(), Some(&second.id));
    assert_eq!(
        selection.configuration_revision,
        Some(second.configuration_revision)
    );
    assert!(!accept_game_type_draft(
        &GameTypeSelectionDraft::default(),
        &lobby,
        &mut selection
    ));
}

#[test]
fn results_disable_replay_when_the_exact_game_disappears() {
    let mut app = flow_test_app();
    app.world_mut().spawn((
        Client,
        lobby_membership(),
        RoutedClientSession {
            generation: 4,
            kind: super::super::RoutedClientSessionKind::Lobby,
        },
    ));
    app.world_mut()
        .resource_mut::<RoutedClientLifecycle>()
        .generation = 4;
    app.world_mut()
        .resource_mut::<super::super::ClientMatchResultState>()
        .context = Some(super::super::ClientMatchResultContext {
        result: brawler::matchplay::MatchResult::Draw,
        local_team: None,
        game_type_id: Some(brawler::lobby::GameTypeId::new("retired-mode").unwrap()),
        game_name: Some("Retired Mode".to_string()),
        final_score: None,
        wipeout: None,
        objective: None,
        local_player: None,
        published_map: None,
    });
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Results);
    app.update();

    let (replay_disabled, dashboard_disabled) = {
        let world = app.world_mut();
        let mut query = world.query::<(&FlowButton, Has<InteractionDisabled>)>();
        let replay = query
            .iter(world)
            .find(|(button, _)| button.action == FlowUiAction::QueueAgain)
            .map(|(_, disabled)| disabled)
            .unwrap();
        let dashboard = query
            .iter(world)
            .find(|(button, _)| button.action == FlowUiAction::ReturnToDashboard)
            .map(|(_, disabled)| disabled)
            .unwrap();
        (replay, dashboard)
    };
    assert!(replay_disabled);
    assert!(!dashboard_disabled);
    assert!(
        visible_text(&mut app)
            .iter()
            .any(|text| text.contains("previous game is not available"))
    );
    // BRL-51zd: the first presentation bursts the trophy and outcome headline once.
    let bursts = |app: &mut App| {
        app.world_mut()
            .query::<&crate::ui::surface::SurfaceBurst>()
            .iter(app.world())
            .count()
    };
    assert_eq!(bursts(&mut app), 2);
    app.update();
    assert_eq!(bursts(&mut app), 2, "an unchanged result does not re-burst");
}

#[test]
fn queue_copy_uses_advertised_game_and_the_accepted_character_pick() {
    let builds = brawler::builds::BuildCatalog::embedded().unwrap();
    let membership = brawler::lobby::QueueMembership {
        ticket_id: brawler::lobby::QueueTicketId::new(1).unwrap(),
        catalog_revision: brawler::lobby::CatalogRevision([1; 32]),
        game_type_id: brawler::lobby::GameTypeId::new("wipeout-2v2").unwrap(),
        game_type_configuration_revision: 1,
        profile_revision: brawler::profiles::ProfileRevision::INITIAL,
        accepted_build: brawler::builds::AcceptedBuildSummary {
            character: character_choice(9, 10, 3),
            identity: brawler::builds::SelectedBuild {
                recipe_fingerprint: brawler::builds::BuildRecipeFingerprint(1),
                revision: builds.balance_revision,
            },
        },
        admitted_at_pool_state_revision: 2,
    };
    let copy = queue_copy(
        &super::super::ClientQueueModel::default(),
        &membership,
        Some(&lobby_membership()),
        &builds,
    );
    assert_eq!(copy.mode, "WIPEOUT");
    assert_eq!(copy.game, "Wipeout 2v2");
    assert_eq!(copy.brawler, "Bumper");
    assert_eq!(copy.population, "Updating queue");
    assert_eq!(copy.build, "Restoration Field · Adrenal Response");
    assert_eq!(
        queue_copy(
            &super::super::ClientQueueModel::default(),
            &membership,
            None,
            &builds,
        )
        .brawler,
        "Brawler"
    );

    let loading_copy = match_loading_text(
        &brawler::lobby::ReservationStarted {
            reservation_id: brawler::lobby::MatchReservationId::new(1).unwrap(),
            ticket_id: Some(membership.ticket_id),
            game_type_id: membership.game_type_id.clone(),
            mode_definition_id: brawler::map::WIPEOUT_MODE_DEFINITION,
            map_preset_id: Some(brawler::map::MapPresetId(1)),
            map_recipe_id: brawler::map::MapRecipeId(1),
            map_recipe_revision: 1,
            map_recipe_fingerprint: brawler::map::MapRecipeFingerprint(1),
            team_count: 2,
            players_per_team: 2,
            accepted_build: membership.accepted_build,
            loading_deadline_millis: 30_000,
            published_map: None,
        },
        Some(brawler::lobby::MatchLoadingPhase::Synchronizing),
    );
    assert!(loading_copy.contains("Synchronizing map"));
    assert!(!loading_copy.contains("Saved brawler accepted"));
    assert!(!loading_copy.contains("points"));
}

#[test]
fn cancel_pending_copy_is_explicit_and_disables_only_cancel() {
    let cancel = brawler::lobby::QueueCommand::Cancel(brawler::lobby::QueueCancelCommand {
        ticket_id: brawler::lobby::QueueTicketId::new(1).unwrap(),
    });
    assert_eq!(
        queue_cancel_presentation(Some(&cancel)),
        ("CANCELLING...", true)
    );
    assert_eq!(queue_cancel_presentation(None), ("CANCEL QUEUE", false));
}

#[test]
fn state_scoped_flow_roots_replace_exactly_and_error_waits_for_destination() {
    let mut app = flow_test_app();
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::ServerSelect);
    app.update();
    assert_eq!(count_flow_roots(&mut app), 1);

    app.world_mut().resource_mut::<FlowNavigation>().focused = 7;
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Connecting);
    app.update();
    assert_eq!(count_flow_roots(&mut app), 1);
    assert_eq!(app.world().resource::<FlowNavigation>().focused, 0);

    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Error(FlowError {
        kind: FlowErrorKind::Connection,
        message: "recoverable".to_string(),
        return_flow: ClientFlow::ServerSelect,
        actions: [Some(FlowErrorAction::Back), None],
    });
    app.update();
    assert_eq!(count_error_roots(&mut app), 0);

    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::ServerSelect);
    app.update();
    assert_eq!(count_flow_roots(&mut app), 1);
    assert_eq!(count_error_roots(&mut app), 1);
    assert_eq!(app.world().resource::<FlowNavigation>().focused, 1_000);

    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::None;
    app.update();
    assert_eq!(count_error_roots(&mut app), 0);
}

#[test]
fn connection_screens_use_shared_surfaces_and_keep_transient_actions_bounded() {
    let mut app = flow_test_app();
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::ServerSelect);
    app.update();

    let surface_for = |app: &mut App, wanted: FlowUiAction| {
        let world = app.world_mut();
        let mut query = world.query::<(&FlowButton, &Surface)>();
        query
            .iter(world)
            .find(|(button, _)| button.action == wanted)
            .map(|(_, surface)| *surface)
    };
    assert_eq!(
        surface_for(&mut app, FlowUiAction::EditAddress),
        Some(Surface::Field)
    );
    assert_eq!(
        surface_for(&mut app, FlowUiAction::Connect),
        Some(Surface::Primary)
    );
    assert_eq!(
        surface_for(&mut app, FlowUiAction::OpenSettings),
        Some(Surface::Quiet)
    );

    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Connecting);
    app.update();
    let world = app.world_mut();
    let mut button_query = world.query::<&FlowButton>();
    let actions = button_query
        .iter(world)
        .map(|button| button.action.clone())
        .collect::<Vec<_>>();
    assert_eq!(actions, vec![FlowUiAction::Cancel]);
    let mut progress_query = world.query_filtered::<Entity, With<ConnectingProgressSegment>>();
    assert_eq!(progress_query.iter(world).count(), 3);
    let copy = visible_text(&mut app).join(" ");
    assert!(copy.contains("CONNECTING"));
    assert!(!copy.contains("SETTINGS"));
    assert!(!copy.contains("ESC"));
}

#[test]
fn connection_failure_is_full_screen_icon_free_recovery() {
    let mut app = flow_test_app();
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::ServerSelect);
    app.update();
    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Error(FlowError {
        kind: FlowErrorKind::Connection,
        message: "The connection attempt timed out".into(),
        return_flow: ClientFlow::ServerSelect,
        actions: [
            Some(FlowErrorAction::RetryConnection),
            Some(FlowErrorAction::Back),
        ],
    });
    app.update();

    let copy = visible_text(&mut app).join(" ");
    assert!(copy.contains("CONNECTION LOST"));
    assert!(copy.contains("CHANGE SERVER"));
    assert!(!copy.to_ascii_lowercase().contains("chain"));
    let world = app.world_mut();
    let mut query = world.query::<&FlowButton>();
    let recovery_actions = query
        .iter(world)
        .filter(|button| button.error_action)
        .map(|button| button.action.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        recovery_actions,
        vec![
            FlowUiAction::Retry,
            FlowUiAction::DismissError,
            FlowUiAction::Quit,
        ]
    );
}

#[test]
fn replacing_error_in_place_rebuilds_message_and_actions() {
    let mut app = flow_test_app();
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::ServerSelect);
    app.update();
    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Error(FlowError {
        kind: FlowErrorKind::Queue,
        message: "The queue acknowledgement is taking longer than expected".to_string(),
        return_flow: ClientFlow::ServerSelect,
        actions: [
            Some(FlowErrorAction::RetryQueue),
            Some(FlowErrorAction::Disconnect),
        ],
    });
    app.update();
    assert_eq!(count_error_roots(&mut app), 1);

    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Error(FlowError {
        kind: FlowErrorKind::Queue,
        message: "Queue commands are temporarily limited".to_string(),
        return_flow: ClientFlow::ServerSelect,
        actions: [
            Some(FlowErrorAction::TryAgainQueue),
            Some(FlowErrorAction::Disconnect),
        ],
    });
    app.update();

    assert_eq!(count_error_roots(&mut app), 1);
    let text = visible_text(&mut app);
    assert!(text.iter().any(|line| line.contains("temporarily limited")));
    assert!(!text.iter().any(|line| line.contains("taking longer")));
    let world = app.world_mut();
    let mut query = world.query_filtered::<Entity, With<RateLimitTryAgain>>();
    assert_eq!(query.iter(world).count(), 1);
}

#[test]
fn combined_local_load_failure_has_one_fixed_error_shape() {
    let error = local_load_error(ClientLocalLoadFailures {
        settings_failed: true,
        connections_failed: true,
        build_failed: false,
    })
    .unwrap();
    assert!(error.message.contains("Settings and connection data"));
    assert_eq!(error.return_flow, ClientFlow::ServerSelect);
    assert_eq!(
        error.actions,
        [Some(FlowErrorAction::ContinueWithDefaults), None]
    );
}

fn deadline_fixture() -> PendingConnection {
    PendingConnection {
        generation: 1,
        target: validate_target("127.0.0.1:5000", "Player One").unwrap(),
        candidates: vec![
            "127.0.0.1:5000".parse().unwrap(),
            "127.0.0.2:5000".parse().unwrap(),
        ],
        current_candidate: 0,
        overall_deadline: Duration::from_secs(10),
        dns_deadline: Some(Duration::from_secs(5)),
        candidate_deadline: Some(Duration::from_secs(7)),
        current_entity: None,
        stage: ConnectionStage::ResolvingAddress,
    }
}

#[test]
fn deadline_boundaries_accept_exact_and_expire_only_after() {
    let pending = deadline_fixture();
    assert_eq!(
        attempt_deadline_expiry(Duration::from_secs(5), &pending),
        None
    );
    assert_eq!(
        attempt_deadline_expiry(Duration::from_secs(5) + Duration::from_nanos(1), &pending),
        Some(AttemptDeadlineExpiry::Dns)
    );

    let mut pending = deadline_fixture();
    pending.dns_deadline = None;
    assert_eq!(
        attempt_deadline_expiry(Duration::from_secs(7), &pending),
        None
    );
    assert_eq!(
        attempt_deadline_expiry(Duration::from_secs(7) + Duration::from_nanos(1), &pending),
        Some(AttemptDeadlineExpiry::Candidate)
    );
    pending.candidate_deadline = None;
    assert_eq!(
        attempt_deadline_expiry(Duration::from_secs(10), &pending),
        None
    );
    assert_eq!(
        attempt_deadline_expiry(Duration::from_secs(10) + Duration::from_nanos(1), &pending),
        Some(AttemptDeadlineExpiry::Overall)
    );

    assert!(matches!(
        accepted_observation(Duration::from_secs(10), &pending, false),
        SessionObservation::Accepted
    ));
    assert!(matches!(
        accepted_observation(
            Duration::from_secs(10) + Duration::from_nanos(1),
            &pending,
            false
        ),
        SessionObservation::TimedOut
    ));
    assert!(matches!(
        accepted_observation(Duration::from_secs(1), &pending, true),
        SessionObservation::UnexpectedLoss
    ));
}

#[test]
fn connecting_copy_reports_stage_candidate_and_bounded_time() {
    let mut pending = deadline_fixture();
    pending.dns_deadline = None;
    pending.stage = ConnectionStage::ContactingServer {
        current: 1,
        total: 2,
    };

    let copy = connection_presentation(&pending, Duration::from_millis(2_100));

    assert!(copy.contains("STEP 2 OF 3"));
    assert!(copy.contains("Opening routed connection."));
    assert!(copy.contains("127.0.0.1:5000"));
    assert!(copy.contains("Candidate 1 of 2"));
    assert!(copy.contains("up to 8s remaining"));
}

#[test]
fn candidate_shares_rounding_and_ordered_dedup_are_exact() {
    assert_eq!(
        candidate_time_share(Duration::from_secs(10), 4),
        Duration::from_millis(2_500)
    );
    assert_eq!(
        netcode_timeout_ceiling(Duration::from_millis(2_500)),
        Duration::from_secs(3)
    );
    assert_eq!(
        netcode_timeout_ceiling(Duration::ZERO),
        Duration::from_secs(1)
    );

    let input = [
        "127.0.0.3:5000".parse().unwrap(),
        "127.0.0.1:5000".parse().unwrap(),
        "127.0.0.3:5000".parse().unwrap(),
        "127.0.0.2:5000".parse().unwrap(),
        "127.0.0.4:5000".parse().unwrap(),
        "127.0.0.5:5000".parse().unwrap(),
    ];
    let bounded = bound_resolved_candidates(input);
    assert_eq!(bounded.len(), MAX_RESOLVED_CANDIDATES);
    assert_eq!(bounded[0], input[0]);
    assert_eq!(bounded[1], input[1]);
    assert_eq!(bounded[2], input[3]);
    assert_eq!(bounded[3], input[4]);

    let mut pending = deadline_fixture();
    assert!(has_next_candidate(&pending));
    pending.current_candidate = 1;
    assert!(!has_next_candidate(&pending));
}

#[test]
fn name_editor_moves_and_deletes_on_grapheme_boundaries() {
    let mut model = ServerSelectModel {
        address: String::new(),
        committed_name: String::new(),
        name: "A👨‍👩‍👧B".to_string(),
        editing: Some(EditingField::Name),
        caret: "A👨‍👩‍👧".len(),
        inline_error: None,
    };
    let previous = previous_caret(&model.name, model.caret, EditingField::Name);
    assert_eq!(previous, 1);
    let caret = model.caret;
    edited_value_mut(&mut model, EditingField::Name).replace_range(previous..caret, "");
    model.caret = previous;
    assert_eq!(model.name, "AB");
    insert_editor_text(&mut model, EditingField::Name, "é");
    assert_eq!(model.name, "AéB");
}

#[test]
fn address_editor_rejects_non_ascii_and_respects_mid_string_caret() {
    let mut model = ServerSelectModel {
        address: "localhost:5000".to_string(),
        committed_name: String::new(),
        name: String::new(),
        editing: Some(EditingField::Address),
        caret: 9,
        inline_error: None,
    };
    insert_editor_text(&mut model, EditingField::Address, "-dev");
    assert_eq!(model.address, "localhost-dev:5000");
    insert_editor_text(&mut model, EditingField::Address, "é");
    assert!(model.inline_error.is_some());
}

#[test]
fn explicit_cancel_has_its_own_slot_and_overlay_blocks_underlying_controls() {
    let mut actions = PendingFlowActions::default();
    queue_ui_action(&mut actions, FlowUiAction::Connect);
    queue_ui_action(&mut actions, FlowUiAction::Cancel);
    assert!(matches!(actions.explicit, Some(FlowUiAction::Cancel)));
    assert!(matches!(actions.ordinary, Some(FlowUiAction::Connect)));

    let overlay = ClientOverlay::Error(FlowError {
        kind: FlowErrorKind::Connection,
        message: "blocked".to_string(),
        return_flow: ClientFlow::ServerSelect,
        actions: [Some(FlowErrorAction::Back), None],
    });
    let underlying = FlowButton {
        index: 1,
        action: FlowUiAction::Connect,
        error_action: false,
    };
    let error = FlowButton {
        index: 1_000,
        action: FlowUiAction::DismissError,
        error_action: true,
    };
    assert!(!overlay_allows_button(&overlay, &underlying));
    assert!(overlay_allows_button(&overlay, &error));
    assert!(!overlay_allows_button(
        &ClientOverlay::DashboardMenu,
        &underlying
    ));
    assert!(overlay_allows_button(&ClientOverlay::DashboardMenu, &error));
}

#[test]
fn error_kinds_have_specific_user_facing_titles() {
    assert_eq!(FlowErrorKind::Connection.title(), "CONNECTION ERROR");
    assert_eq!(FlowErrorKind::Queue.title(), "QUEUE ERROR");
    assert_eq!(FlowErrorKind::Persistence.title(), "SAVE ERROR");
    assert_eq!(FlowErrorKind::Content.title(), "CONTENT ERROR");
}

#[test]
fn rejection_actions_and_favorite_focus_are_deterministic() {
    let invalid_name = rejection_flow_error(ClientLobbyFailure::Rejected(
        brawler::protocol::LobbyJoinRejection::InvalidName,
    ));
    assert_eq!(
        invalid_name.actions,
        [Some(FlowErrorAction::EditName), Some(FlowErrorAction::Back)]
    );
    let account_in_use = rejection_flow_error(ClientLobbyFailure::Rejected(
        brawler::protocol::LobbyJoinRejection::AccountInUse,
    ));
    assert_eq!(
        account_in_use.message,
        "This account is already connected from another running copy of the game. Close it, then \
         retry."
    );
    assert_eq!(
        account_in_use.actions,
        [
            Some(FlowErrorAction::RetryConnection),
            Some(FlowErrorAction::Back)
        ]
    );
    assert_eq!(favorite_focus_after_removal(Some(1), 2), 5);
    assert_eq!(favorite_focus_after_removal(Some(2), 2), 5);
    assert_eq!(favorite_focus_after_removal(Some(0), 0), 0);
    assert_eq!(favorite_focus_after_removal(None, 2), 0);
}

#[test]
fn controller_can_enter_and_leave_text_editing_without_becoming_trapped() {
    let mut app = flow_test_app();
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::ServerSelect);
    app.update();
    assert_eq!(app.world().resource::<FlowNavigation>().focused, 2);

    let mut gamepad = Gamepad::default();
    gamepad.digital_mut().press(GamepadButton::DPadUp);
    let gamepad_entity = app.world_mut().spawn(gamepad).id();
    app.update();
    assert_eq!(app.world().resource::<FlowNavigation>().focused, 1);

    {
        let mut gamepad = app.world_mut().entity_mut(gamepad_entity);
        let mut gamepad = gamepad.get_mut::<Gamepad>().unwrap();
        gamepad.digital_mut().reset_all();
        gamepad.digital_mut().press(GamepadButton::South);
    }
    app.update();
    assert_eq!(
        app.world().resource::<ServerSelectModel>().editing,
        Some(EditingField::Name)
    );

    {
        let mut gamepad = app.world_mut().entity_mut(gamepad_entity);
        let mut gamepad = gamepad.get_mut::<Gamepad>().unwrap();
        gamepad.digital_mut().reset_all();
        gamepad.digital_mut().press(GamepadButton::East);
    }
    app.update();
    assert_eq!(app.world().resource::<ServerSelectModel>().editing, None);
}

#[test]
fn committing_an_undrawable_name_explains_the_rejection_inline() {
    let mut app = flow_test_app();
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::ServerSelect);
    app.update();
    {
        let mut model = app.world_mut().resource_mut::<ServerSelectModel>();
        model.name = "Ab\u{E000}cd".to_string();
        model.caret = model.name.len();
        model.editing = Some(EditingField::Name);
    }
    press_key(&mut app, KeyCode::Enter);
    let model = app.world().resource::<ServerSelectModel>();
    assert_eq!(
        model.inline_error.as_deref(),
        Some("Name has a character that can't be shown.")
    );
    assert_ne!(
        model.committed_name, model.name,
        "the rejected name is not committed"
    );
}

#[test]
fn dashboard_settings_is_menu_owned_and_mode_card_opens_selection() {
    let mut app = flow_test_app();
    app.world_mut().spawn((
        Client,
        lobby_membership(),
        RoutedClientSession {
            generation: 1,
            kind: super::super::RoutedClientSessionKind::Lobby,
        },
    ));
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Dashboard);
    app.update();
    let buttons = |app: &mut App| {
        let world = app.world_mut();
        let mut query = world.query::<&FlowButton>();
        query
            .iter(world)
            .map(|b| b.action.clone())
            .collect::<Vec<_>>()
    };
    assert!(!buttons(&mut app).contains(&FlowUiAction::OpenSettings));
    let copy = visible_text(&mut app).join(" ");
    assert!(!copy.contains("ONLINE"));
    assert!(!copy.contains("Population"));
    assert!(!copy.contains("Map pool"));
    press_flow_button(&mut app, &FlowUiAction::OpenDashboardMenu);
    for mut interaction in app
        .world_mut()
        .query::<&mut Interaction>()
        .iter_mut(app.world_mut())
    {
        *interaction = Interaction::None;
    }
    app.update();
    assert!(buttons(&mut app).contains(&FlowUiAction::OpenSettings));
    press_flow_button(&mut app, &FlowUiAction::CloseDashboardMenu);
    app.update();
    assert_eq!(
        app.world().resource::<FlowNavigation>().focused,
        DASHBOARD_MENU_INDEX
    );
    press_flow_button(&mut app, &FlowUiAction::OpenGameTypeSelect);
    app.update();
    assert_eq!(
        *app.world().resource::<State<ClientFlow>>().get(),
        ClientFlow::GameTypeSelect
    );
}

#[test]
fn dashboard_chrome_focus_belongs_only_to_the_active_surface() {
    let mut app = App::new();
    app.init_resource::<FlowNavigation>()
        .init_resource::<ClientOverlay>()
        .init_resource::<GameTypeSelectionDraft>()
        .add_systems(Update, update_flow_button_chrome);
    let mut buttons = Vec::new();
    for overlay_owned in [false, true] {
        for index in 0..2 {
            buttons.push(
                app.world_mut()
                    .spawn((
                        FlowButton {
                            index,
                            action: FlowUiAction::OpenDashboardMenu,
                            error_action: overlay_owned,
                        },
                        DashboardButtonStyle::Header,
                        Interaction::Hovered,
                        BackgroundColor::default(),
                        BorderColor::default(),
                    ))
                    .id(),
            );
        }
    }
    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::DashboardMenu;
    for selected in 0..2 {
        app.world_mut().resource_mut::<FlowNavigation>().focused = selected;
        app.update();
        for (index, &entity) in buttons.iter().enumerate() {
            let expected_focus = index == selected + 2;
            let state = app
                .world()
                .get::<crate::ui::surface::SurfaceState>(entity)
                .unwrap();
            assert_eq!(state.focused, expected_focus);
            assert_eq!(state.inactive, index < 2);
        }
    }
    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::None;
    app.update();
    assert!(
        app.world()
            .get::<crate::ui::surface::SurfaceState>(buttons[1])
            .unwrap()
            .focused
    );
    assert!(
        app.world()
            .get::<crate::ui::surface::SurfaceState>(buttons[3])
            .unwrap()
            .inactive
    );
}

#[test]
fn game_selection_groups_modes_preserves_variants_and_has_one_back_button() {
    let mut app = flow_test_app();
    let mut lobby = lobby_membership();
    let mut second = lobby.game_types[0].clone();
    second.id = brawler::lobby::GameTypeId::new("alternate-wipeout").unwrap();
    second.display_name = "Alternate Wipeout".into();
    lobby.game_types.push(second);
    app.world_mut().spawn((
        Client,
        lobby,
        RoutedClientSession {
            generation: 1,
            kind: super::super::RoutedClientSessionKind::Lobby,
        },
    ));
    app.world_mut()
        .resource_mut::<GameTypeSelectionDraft>()
        .selected_index = Some(0);
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::GameTypeSelect);
    app.update();
    let world = app.world_mut();
    let mut query = world.query::<&FlowButton>();
    let buttons = query.iter(world).collect::<Vec<_>>();
    assert_eq!(buttons.iter().filter(|b| b.index < 100).count(), 1);
    assert_eq!(
        buttons
            .iter()
            .filter(|b| (100..1000).contains(&b.index))
            .count(),
        2
    );
    // BRL-0198: touch players need an on-screen way back (desktop also has Esc).
    assert_eq!(
        buttons
            .iter()
            .filter(|b| b.action == FlowUiAction::CancelGameType)
            .map(|b| b.index)
            .collect::<Vec<_>>(),
        [screens::game_select::GAME_TYPE_BACK_INDEX]
    );
    assert!(
        buttons
            .iter()
            .any(|b| b.index == 101 && b.action == FlowUiAction::SelectGameTypeDraft(1))
    );
    press_flow_button(&mut app, &FlowUiAction::SelectGameTypeDraft(1));
    assert_eq!(
        app.world()
            .resource::<GameTypeSelectionDraft>()
            .selected_index,
        Some(1)
    );
    assert_eq!(
        *app.world().resource::<State<ClientFlow>>().get(),
        ClientFlow::GameTypeSelect
    );
    press_flow_button(&mut app, &FlowUiAction::ConfirmGameType);
    app.update();
    assert_eq!(
        *app.world().resource::<State<ClientFlow>>().get(),
        ClientFlow::Dashboard
    );
    assert_eq!(
        app.world()
            .resource::<SelectedGameType>()
            .game_type_id
            .as_ref()
            .unwrap()
            .as_str(),
        "alternate-wipeout"
    );
}

#[test]
fn game_selection_navigation_moves_between_cards_sizes_and_confirmation() {
    use super::input::game_selection_neighbor;
    use super::screens::dashboard::DashboardNavigationDirection::{Down, Left, Right, Up};
    let available = [0, 1, 2, 3, 100, 101, 102, 1000];
    assert_eq!(game_selection_neighbor(0, Right, &available, 4), 1);
    assert_eq!(game_selection_neighbor(1, Left, &available, 4), 0);
    assert_eq!(game_selection_neighbor(2, Down, &available, 4), 102);
    assert_eq!(game_selection_neighbor(102, Down, &available, 4), 1000);
    assert_eq!(game_selection_neighbor(1000, Up, &available, 4), 100);
    assert_eq!(game_selection_neighbor(3, Right, &available, 4), 3);
    assert_eq!(game_selection_neighbor(0, Down, &available, 2), 2);
    assert_eq!(game_selection_neighbor(3, Up, &available, 2), 1);
}

#[test]
fn practice_picker_defers_start_and_passes_only_confirmed_map() {
    let mut membership = lobby_membership();
    membership.game_types[0]
        .practice_map_preset_ids
        .push(brawler::map::MapPresetId(7));
    let mut app = configured_lobby_flow_app(membership);
    inject_action(&mut app, FlowUiAction::OpenPracticeMaps);
    app.update();
    assert_eq!(
        *app.world().resource::<State<ClientFlow>>().get(),
        ClientFlow::PracticeMapSelect
    );
    assert!(
        !app.world()
            .resource::<super::super::ClientPracticeModel>()
            .pending()
    );
    inject_action(
        &mut app,
        FlowUiAction::ChoosePracticeMap(brawler::map::MapPresetId(99)),
    );
    assert_eq!(
        app.world()
            .resource::<super::super::ClientPracticeModel>()
            .selected_map
            .as_ref()
            .unwrap()
            .1,
        brawler::map::MapPresetId(1)
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ArrowRight);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    assert_eq!(
        app.world()
            .resource::<super::super::ClientPracticeModel>()
            .selected_map
            .as_ref()
            .unwrap()
            .1,
        brawler::map::MapPresetId(1)
    );
    inject_action(
        &mut app,
        FlowUiAction::ChoosePracticeMap(brawler::map::MapPresetId(7)),
    );
    inject_action(&mut app, FlowUiAction::StartPractice);
    assert_eq!(
        app.world()
            .resource::<super::super::ClientPracticeModel>()
            .pending_request_for_test()
            .unwrap()
            .map_preset_id,
        Some(brawler::map::MapPresetId(7))
    );
}

#[test]
fn practice_picker_cancel_returns_home_without_request() {
    let mut app = configured_lobby_flow_app(lobby_membership());
    inject_action(&mut app, FlowUiAction::OpenPracticeMaps);
    app.update();
    inject_action(&mut app, FlowUiAction::CancelPracticeMaps);
    app.update();
    assert_eq!(
        *app.world().resource::<State<ClientFlow>>().get(),
        ClientFlow::Dashboard
    );
    assert!(
        !app.world()
            .resource::<super::super::ClientPracticeModel>()
            .pending()
    );
}

#[test]
fn practice_picker_gamepad_focus_does_not_select_and_overlay_blocks_activation() {
    let mut membership = lobby_membership();
    membership.game_types[0]
        .practice_map_preset_ids
        .push(brawler::map::MapPresetId(7));
    let mut app = configured_lobby_flow_app(membership);
    inject_action(&mut app, FlowUiAction::OpenPracticeMaps);
    app.update();
    let mut pad = Gamepad::default();
    pad.digital_mut().press(GamepadButton::DPadLeft);
    let pad_entity = app.world_mut().spawn(pad).id();
    app.update();
    assert_eq!(
        app.world()
            .resource::<super::super::ClientPracticeModel>()
            .selected_map
            .as_ref()
            .unwrap()
            .1,
        brawler::map::MapPresetId(1)
    );
    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Settings;
    {
        let mut entity = app.world_mut().entity_mut(pad_entity);
        let mut pad = entity.get_mut::<Gamepad>().unwrap();
        pad.digital_mut().reset_all();
        pad.digital_mut().press(GamepadButton::South);
    }
    app.update();
    assert_eq!(
        app.world()
            .resource::<super::super::ClientPracticeModel>()
            .selected_map
            .as_ref()
            .unwrap()
            .1,
        brawler::map::MapPresetId(1)
    );
    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::None;
    app.update();
    assert_eq!(
        app.world()
            .resource::<super::super::ClientPracticeModel>()
            .selected_map
            .as_ref()
            .unwrap()
            .1,
        brawler::map::MapPresetId(7)
    );
}

#[test]
fn connection_retry_and_saved_target_clear_obsolete_error_overlay() {
    for action in [
        FlowUiAction::Retry,
        FlowUiAction::JoinSaved("127.0.0.1:5000".into()),
    ] {
        let mut app = flow_action_test_app();
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::ServerSelect);
        app.update();
        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Error(FlowError {
            kind: FlowErrorKind::Connection,
            message: "The connection attempt timed out".into(),
            return_flow: ClientFlow::ServerSelect,
            actions: [
                Some(FlowErrorAction::RetryConnection),
                Some(FlowErrorAction::Back),
            ],
        });
        inject_action(&mut app, action);
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::None
        );
        assert_eq!(
            app.world().resource::<FlowCommit>().next_flow,
            Some(ClientFlow::Connecting)
        );
    }
}

#[test]
fn accepted_connection_retires_old_error_and_dashboard_accepts_input() {
    let mut app = configured_lobby_flow_app(lobby_membership());
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Connecting);
    app.update();
    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Error(FlowError {
        kind: FlowErrorKind::Connection,
        message: "Old timeout".into(),
        return_flow: ClientFlow::ServerSelect,
        actions: [Some(FlowErrorAction::RetryConnection), None],
    });
    inject_session_observation(&mut app, SessionObservation::Accepted);
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::None
    );
    app.update();
    press_flow_button(&mut app, &FlowUiAction::OpenDashboardMenu);
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::DashboardMenu
    );
}

#[test]
fn accepted_connection_preserves_new_persistence_failure() {
    let mut app = configured_lobby_flow_app(lobby_membership());
    app.world_mut().resource_mut::<ClientConnectionsPath>().0 = std::env::temp_dir();
    inject_session_observation(&mut app, SessionObservation::Accepted);
    assert!(matches!(
        app.world().resource::<ClientOverlay>(),
        ClientOverlay::Error(FlowError {
            kind: FlowErrorKind::Persistence,
            ..
        })
    ));
}

#[test]
fn shell_back_actions_share_the_same_role_and_preview_has_no_chrome() {
    use crate::ui::surface::Surface;
    for action in [
        FlowUiAction::Back,
        FlowUiAction::CloseBrawlers,
        FlowUiAction::CancelGameType,
    ] {
        assert_eq!(screens::shared::surface_role(&action), Surface::Back);
    }
    let mut app = App::new();
    app.init_resource::<FlowNavigation>()
        .init_resource::<ClientOverlay>()
        .init_resource::<GameTypeSelectionDraft>()
        .add_systems(Update, update_flow_button_chrome);
    let preview = app
        .world_mut()
        .spawn((
            FlowButton {
                index: 0,
                action: FlowUiAction::OpenBrawlers,
                error_action: false,
            },
            DashboardButtonStyle::Preview,
        ))
        .id();
    app.update();
    assert!(app.world().get::<Surface>(preview).is_none());
}

#[test]
fn confirmation_navigation_starts_safe_and_supports_both_axes() {
    let membership = lobby_membership();
    let mut app = configured_customization_app(&membership);
    inject_action(&mut app, FlowUiAction::RequestChangeServer);
    app.update();
    assert_eq!(app.world().resource::<FlowNavigation>().focused, 0);
    for (key, expected) in [
        (KeyCode::ArrowRight, 1),
        (KeyCode::ArrowLeft, 0),
        (KeyCode::ArrowDown, 1),
        (KeyCode::ArrowUp, 0),
    ] {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        assert_eq!(app.world().resource::<FlowNavigation>().focused, expected);
        assert!(
            !app.world()
                .resource::<super::super::ClientProfileModel>()
                .pending()
        );
    }
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::None
    );
}

/// Pixel 8a landscape logical window: 2400x1080 at density 420 minus the 121 px camera-cutout
/// strip the Android activity keeps clear (914x411 full screen).
const PHONE_VIEWPORT: (u32, u32) = (868, 411);

fn phone_layout_flow_app() -> App {
    let mut app = flow_test_app_with(|app| {
        crate::ui::layout_harness::add_layout(app, PHONE_VIEWPORT.0, PHONE_VIEWPORT.1);
    });
    crate::ui::layout_harness::apply_viewport_fit(&mut app);
    app
}

fn settle_layout(app: &mut App) {
    for _ in 0..3 {
        app.update();
    }
}

#[test]
fn server_select_fits_a_landscape_phone_and_shows_field_values() {
    let mut app = phone_layout_flow_app();
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::ServerSelect);
    settle_layout(&mut app);

    let unreachable = crate::ui::layout_harness::unreachable_buttons(app.world_mut());
    assert!(
        unreachable.is_empty(),
        "unreachable controls: {unreachable:?}"
    );
    let mut fields = app
        .world_mut()
        .query::<(Entity, &screens::server_select::FieldLabel, &Text)>();
    let fields: Vec<_> = fields
        .iter(app.world())
        .map(|(entity, _, text)| (entity, text.0.clone()))
        .collect();
    assert_eq!(fields.len(), 2);
    for (entity, text) in fields {
        assert!(!text.is_empty());
        let rect = crate::ui::layout_harness::node_rect(app.world(), entity).unwrap();
        assert!(
            rect.width() > 40.0 && rect.height() > 10.0,
            "{text}: {rect:?}"
        );
    }

    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Error(FlowError {
        kind: FlowErrorKind::Connection,
        message: "The lobby connection was lost unexpectedly".to_string(),
        return_flow: ClientFlow::ServerSelect,
        actions: [
            Some(FlowErrorAction::RetryConnection),
            Some(FlowErrorAction::Back),
        ],
    });
    assert_phone_reachable(&mut app, "Connection lost");
    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::None;
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Connecting);
    assert_phone_reachable(&mut app, "Connecting");
}

fn assert_phone_reachable(app: &mut App, screen: &str) {
    settle_layout(app);
    let mut buttons = app
        .world_mut()
        .query_filtered::<&ComputedNode, With<Button>>();
    let laid_out = buttons
        .iter(app.world())
        .filter(|node| !node.is_empty())
        .count();
    let total = buttons.iter(app.world()).count();
    let state = *app.world().resource::<State<ClientFlow>>().get();
    assert!(
        laid_out > 0,
        "{screen}: no laid-out controls ({total} buttons, {state:?})"
    );
    let unreachable = crate::ui::layout_harness::unreachable_buttons(app.world_mut());
    assert!(
        unreachable.is_empty(),
        "{screen}: unreachable controls at the phone viewport: {unreachable:?}"
    );
}

#[test]
fn player_path_screens_keep_every_control_reachable_on_a_landscape_phone() {
    let membership = lobby_membership();
    let mut app = configured_lobby_flow_app_with(membership.clone(), |app| {
        crate::ui::layout_harness::add_layout(app, PHONE_VIEWPORT.0, PHONE_VIEWPORT.1);
    });
    crate::ui::layout_harness::apply_viewport_fit(&mut app);
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_snapshot_for_test(membership.profile.clone());
    // Re-enter the Dashboard now that the lobby membership exists, as a real join does.
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::ServerSelect);
    app.update();
    enter_dashboard(&mut app);
    assert_phone_reachable(&mut app, "Dashboard");
    let mut classes = app.world_mut().query::<&DashboardLayoutClass>();
    assert_eq!(
        classes.iter(app.world()).copied().collect::<Vec<_>>(),
        [DashboardLayoutClass::Wide],
        "the viewport fit keeps the phone on the wide Dashboard"
    );

    inject_action(&mut app, FlowUiAction::OpenBrawlers);
    assert_phone_reachable(&mut app, "Brawlers");
    inject_action(&mut app, FlowUiAction::CloseBrawlers);
    assert_phone_reachable(&mut app, "Dashboard after Brawlers");

    inject_action(&mut app, FlowUiAction::OpenGameTypeSelect);
    assert_phone_reachable(&mut app, "Choose Game");
    inject_action(&mut app, FlowUiAction::CancelGameType);
    inject_action(&mut app, FlowUiAction::OpenPracticeMaps);
    assert_phone_reachable(&mut app, "Practice maps");
}

/// The screens the ring-band checks walk, in order, from the Dashboard.
const RING_BAND_SCREENS: [&str; 4] = ["Dashboard", "Brawlers", "Choose Game", "Practice maps"];

/// Lays out the ring-band screens at one viewport, with or without the surface content reserve,
/// and hands each settled screen to `visit`.
fn walk_ring_band_screens(
    width: u32,
    height: u32,
    reserve: bool,
    mut visit: impl FnMut(&mut App, &str),
) {
    let membership = lobby_membership();
    let mut app = configured_lobby_flow_app_with(membership.clone(), |app| {
        crate::ui::layout_harness::add_layout(app, width, height);
        if reserve {
            app.add_systems(
                PostUpdate,
                crate::ui::surface::reserve_surface_content.before(UiSystems::Layout),
            );
        }
    });
    crate::ui::layout_harness::apply_viewport_fit(&mut app);
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_snapshot_for_test(membership.profile.clone());
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::ServerSelect);
    app.update();
    enter_dashboard(&mut app);
    let actions = [
        vec![],
        vec![FlowUiAction::OpenBrawlers],
        vec![
            FlowUiAction::CloseBrawlers,
            FlowUiAction::OpenGameTypeSelect,
        ],
        vec![FlowUiAction::CancelGameType, FlowUiAction::OpenPracticeMaps],
    ];
    for (screen, actions) in RING_BAND_SCREENS.into_iter().zip(actions) {
        for action in actions {
            inject_action(&mut app, action);
        }
        assert_phone_reachable(&mut app, screen);
        visit(&mut app, screen);
    }
}

/// Logical outer size of every visible, laid-out flow button, keyed by its action. A Back's
/// width is left out (zero): in the client, `SurfacePlugin::bind` floors it from the label, wider
/// than its padded content, and this harness does not run `bind`.
fn flow_button_sizes(app: &mut App) -> Vec<(String, Vec2)> {
    let mut buttons = app.world_mut().query::<(
        &FlowButton,
        &ComputedNode,
        &InheritedVisibility,
        Option<&Surface>,
    )>();
    let mut sizes: Vec<_> = buttons
        .iter(app.world())
        .filter(|(_, node, visible, _)| visible.get() && !node.is_empty())
        .map(|(button, node, _, role)| {
            let mut size = node.size() * node.inverse_scale_factor();
            if role == Some(&Surface::Back) {
                size.x = 0.0;
            }
            (format!("{:?}", button.action), size)
        })
        .collect();
    // Two buttons can share an action (Dashboard's brawler tile and card); order them by size
    // too, so the comparison never depends on entity iteration order.
    sizes.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(a.1.x.total_cmp(&b.1.x))
            .then(a.1.y.total_cmp(&b.1.y))
    });
    sizes
}

/// BRL-fe7t: on the phone and desktop viewports, no screen content reaches into a surface's
/// ring band, so the focus frame, current rim and pointer never draw over a label or icon.
#[test]
fn shell_surface_content_stays_clear_of_the_ring_band() {
    for (width, height) in [PHONE_VIEWPORT, (914, 411), (1280, 720)] {
        walk_ring_band_screens(width, height, true, |app, screen| {
            let intrusions = crate::ui::layout_harness::surface_ring_intrusions(app.world_mut());
            assert!(
                intrusions.is_empty(),
                "{screen} at {width}x{height}: content in the ring band: {intrusions:#?}"
            );
        });
    }
}

/// BRL-fe7t: the ring band fits inside the existing button and card sizes. Every flow button
/// keeps its outer size within 3.5 px of the layout without the reserve (the pre-fe7t sizes),
/// and the loadout header Back does not move at all.
#[test]
fn ring_band_keeps_button_and_card_outer_sizes() {
    for (width, height) in [PHONE_VIEWPORT, (914, 411), (1280, 720)] {
        let mut baseline = Vec::new();
        walk_ring_band_screens(width, height, false, |app, screen| {
            baseline.push((screen.to_owned(), flow_button_sizes(app)));
        });
        let mut baseline = baseline.into_iter();
        walk_ring_band_screens(width, height, true, |app, screen| {
            let (expected_screen, before) = baseline.next().expect("same screens");
            assert_eq!(expected_screen, screen);
            let after = flow_button_sizes(app);
            assert_eq!(
                before.iter().map(|(key, _)| key).collect::<Vec<_>>(),
                after.iter().map(|(key, _)| key).collect::<Vec<_>>(),
                "{screen} at {width}x{height}: the same buttons"
            );
            let grown: Vec<_> = before
                .iter()
                .zip(&after)
                .filter(|((_, before), (_, after))| (*after - *before).abs().max_element() > 3.5)
                .map(|((key, before), (_, after))| format!("{key}: {before} -> {after}"))
                .collect();
            assert!(
                grown.is_empty(),
                "{screen} at {width}x{height}: outer sizes changed: {grown:#?}"
            );
        });
    }
}

/// Settles the layout, then asserts every control stays reachable and is a 48 dp touch target.
fn assert_phone_touch_targets(app: &mut App, screen: &str) {
    assert_phone_reachable(app, screen);
    let short = crate::ui::layout_harness::short_buttons(
        app.world_mut(),
        crate::ui::touch_target::MIN_TOUCH_TARGET_DP,
    );
    assert!(
        short.is_empty(),
        "{screen}: controls under 48 dp at the phone viewport: {short:?}"
    );
    // The Dashboard brawler preview is a backdrop hit area the action column sits on.
    let previews: Vec<_> = app
        .world_mut()
        .query_filtered::<Entity, With<DashboardPreviewHost>>()
        .iter(app.world())
        .collect();
    let overlapping = crate::ui::layout_harness::overlapping_buttons(app.world_mut(), &previews);
    assert!(
        overlapping.is_empty(),
        "{screen}: raised controls overlap: {overlapping:?}"
    );
}

/// BRL-q148: with the touch policy on, every shell control on the player path is at least 48 dp
/// tall at the default and the smallest UI scale, and stays reachable.
#[test]
fn shell_controls_are_48_dp_touch_targets_on_a_landscape_phone() {
    use crate::settings::persistence::MIN_UI_SCALE;
    let touch_phone_app = |membership: &ClientLobbyMembership, ui_scale: f32| {
        let mut app = configured_lobby_flow_app_with(membership.clone(), |app| {
            crate::ui::layout_harness::add_layout(app, PHONE_VIEWPORT.0, PHONE_VIEWPORT.1);
            app.insert_resource(crate::ui::touch_target::TouchTargetPolicy { enabled: true })
                .insert_resource(super::super::ClientInputContext::Shell)
                .insert_resource(brawler::map::MapCatalogResource(
                    brawler::map::MapContentCatalog::embedded().unwrap(),
                ));
        });
        crate::ui::layout_harness::apply_viewport_fit(&mut app);
        app.world_mut().resource_mut::<UiScale>().0 *= ui_scale;
        app.world_mut()
            .resource_mut::<super::super::ClientProfileModel>()
            .set_snapshot_for_test(membership.profile.clone());
        app
    };
    for ui_scale in [1.0, MIN_UI_SCALE] {
        let membership = lobby_membership();
        let at = |screen: &str| format!("{screen} at {:.0}%", ui_scale * 100.0);

        let mut app = touch_phone_app(&membership, ui_scale);
        app.world_mut()
            .resource_mut::<RoutedClientLifecycle>()
            .start_lobby();
        app.world_mut()
            .resource_mut::<super::super::ClientQueueModel>()
            .set_membership_for_test(queue_membership(&membership));
        inject_session_observation(
            &mut app,
            SessionObservation::QueueOutcome(brawler::lobby::QueueCommandOutcome {
                request_id: brawler::lobby::QueueRequestId::new(1).unwrap(),
                decision: brawler::lobby::QueueDecision::Joined(queue_membership(&membership)),
            }),
        );
        app.update();
        assert_eq!(current_flow(&app), ClientFlow::Queue);
        assert_phone_touch_targets(&mut app, &at("Queue"));

        let mut app = touch_phone_app(&membership, ui_scale);

        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::ServerSelect);
        assert_phone_touch_targets(&mut app, &at("Server Select"));
        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Error(FlowError {
            kind: FlowErrorKind::Connection,
            message: "The lobby connection was lost unexpectedly".to_string(),
            return_flow: ClientFlow::ServerSelect,
            actions: [
                Some(FlowErrorAction::RetryConnection),
                Some(FlowErrorAction::Back),
            ],
        });
        assert_phone_touch_targets(&mut app, &at("Connection error"));
        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::None;

        enter_dashboard(&mut app);
        assert_phone_touch_targets(&mut app, &at("Dashboard"));
        inject_action(&mut app, FlowUiAction::OpenDashboardMenu);
        assert_phone_touch_targets(&mut app, &at("Dashboard menu"));
        inject_action(&mut app, FlowUiAction::RequestChangeServer);
        assert_phone_touch_targets(&mut app, &at("Change server confirmation"));
        *app.world_mut().resource_mut::<ClientOverlay>() = lost_match_dashboard_error();
        assert_phone_touch_targets(&mut app, &at("Dashboard error"));
        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::None;
        app.update();

        inject_action(&mut app, FlowUiAction::OpenGameTypeSelect);
        assert_phone_touch_targets(&mut app, &at("Choose Game"));
        inject_action(&mut app, FlowUiAction::CancelGameType);
        inject_action(&mut app, FlowUiAction::OpenPracticeMaps);
        assert_phone_touch_targets(&mut app, &at("Practice maps"));
        inject_action(&mut app, FlowUiAction::CancelPracticeMaps);
        inject_action(&mut app, FlowUiAction::OpenBrawlers);
        assert_phone_touch_targets(&mut app, &at("Brawlers"));
        inject_action(&mut app, FlowUiAction::CloseBrawlers);

        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::MatchLoading);
        *app.world_mut().resource_mut::<ClientOverlay>() = cancel_match_start_overlay();
        assert_phone_touch_targets(&mut app, &at("Cancel confirmation"));
        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::None;

        app.world_mut()
            .resource_mut::<super::super::ClientMatchResultState>()
            .context = Some(match_result_context(membership.game_types[0].id.clone()));
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::Results);
        assert_phone_touch_targets(&mut app, &at("Results"));

        assert_map_maker_phone_touch_targets(&mut app, &at);
    }
}

/// BRL-8q4c: Map Maker's library, create dialog and editor (every panel state) on the phone.
fn assert_map_maker_phone_touch_targets(app: &mut App, at: &dyn Fn(&str) -> String) {
    *app.world_mut()
        .resource_mut::<super::super::MapDraftLibrary>() = sample_map_drafts(5);
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::MapLibrary);
    assert_phone_touch_targets(app, &at("My Maps"));
    inject_action(app, FlowUiAction::CreateMap);
    assert_phone_touch_targets(app, &at("Create map"));
    inject_action(app, FlowUiAction::ConfirmCreateMap);
    // The committed flow change applies on the next frame's state transition.
    app.update();
    assert_eq!(current_flow(app), ClientFlow::MapEditor);
    assert_phone_touch_targets(app, &at("Map editor"));
    inject_action(app, FlowUiAction::MapMakerToggleAssets);
    assert_phone_touch_targets(app, &at("Map editor assets"));
    inject_action(app, FlowUiAction::MapMakerToggleCategoryMenu);
    assert_phone_touch_targets(app, &at("Map editor categories"));
    inject_action(app, FlowUiAction::MapMakerToggleValidation);
    assert_phone_touch_targets(app, &at("Map editor check"));
    inject_action(app, FlowUiAction::MapMakerUseEraseTool);
    assert_phone_touch_targets(app, &at("Map editor erase"));
}

/// `count` saved drafts in every Map Maker mode, for library layout checks.
fn sample_map_drafts(count: u64) -> super::super::MapDraftLibrary {
    let modes = [
        brawler::map::WIPEOUT_MODE_DEFINITION,
        brawler::map::HOT_ZONE_MODE_DEFINITION,
        brawler::map::HEIST_MODE_DEFINITION,
    ];
    let mut library = super::super::MapDraftLibrary::default();
    for id in 1..=count {
        let document = super::super::MapDraftDocument::blank(
            super::super::MapDraftId(id),
            &format!("Sample Arena {id}"),
            modes[usize::try_from(id).unwrap() % modes.len()],
            brawler::map::MapDimensions {
                width: 48,
                height: 32,
            },
            brawler::map::MapPresentationThemeId(1),
            brawler::map::GROUND_ASSET,
        )
        .unwrap();
        library.insert(document).unwrap();
    }
    library
}

/// BRL-2xpc: with the side panel collapsed, the phone editor's controls (top bar, collapsed
/// panel and asset chip) cover about a fifth of the screen, as on the approved design board.
/// Measured like the board: the summed laid-out areas of the control containers over the screen.
#[test]
fn map_editor_controls_leave_the_arena_most_of_a_landscape_phone() {
    use screens::map_maker::{CurrentAssetChip, EditorSidePanel, EditorTopBar};
    for (width, height, limit) in [(914, 411, 22.0), (1280, 720, 13.0)] {
        let membership = lobby_membership();
        let mut app = configured_lobby_flow_app_with(membership, |app| {
            crate::ui::layout_harness::add_layout(app, width, height);
            app.insert_resource(brawler::map::MapCatalogResource(
                brawler::map::MapContentCatalog::embedded().unwrap(),
            ));
        });
        crate::ui::layout_harness::apply_viewport_fit(&mut app);
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::MapLibrary);
        app.update();
        inject_action(&mut app, FlowUiAction::CreateMap);
        inject_action(&mut app, FlowUiAction::ConfirmCreateMap);
        // The committed flow change applies on the next frame's state transition.
        app.update();
        assert_eq!(current_flow(&app), ClientFlow::MapEditor);
        if height >= 640 {
            // Desktop opens on Assets; collapse it to compare the same state.
            inject_action(&mut app, FlowUiAction::MapMakerToggleAssets);
        }
        settle_layout(&mut app);
        let world = app.world_mut();
        let area = |node: &ComputedNode| node.size().x * node.size().y;
        let mut covered = 0.0;
        let mut regions = 0;
        let mut bar = world.query_filtered::<&ComputedNode, With<EditorTopBar>>();
        let mut panel = world.query_filtered::<&ComputedNode, With<EditorSidePanel>>();
        let mut chip = world.query_filtered::<&ComputedNode, With<CurrentAssetChip>>();
        for node in bar
            .iter(world)
            .chain(panel.iter(world))
            .chain(chip.iter(world))
        {
            covered += area(node);
            regions += 1;
        }
        assert_eq!(regions, 3, "top bar, collapsed panel and asset chip");
        let screen =
            f32::from(u16::try_from(width).unwrap()) * f32::from(u16::try_from(height).unwrap());
        let share = 100.0 * covered / screen;
        println!(
            "map editor at {width}x{height}, panel collapsed: controls {share:.1}% of the screen, arena {:.1}%",
            100.0 - share
        );
        assert!(
            share <= limit,
            "{width}x{height}: editor controls cover {share:.1}% of the screen"
        );
    }
}

/// A new draft open in the editor at 1280x720, laid out.
fn laid_out_map_editor_app() -> App {
    let membership = lobby_membership();
    let mut app = configured_lobby_flow_app_with(membership, |app| {
        crate::ui::layout_harness::add_layout(app, 1280, 720);
        app.insert_resource(brawler::map::MapCatalogResource(
            brawler::map::MapContentCatalog::embedded().unwrap(),
        ));
    });
    crate::ui::layout_harness::apply_viewport_fit(&mut app);
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::MapLibrary);
    app.update();
    inject_action(&mut app, FlowUiAction::CreateMap);
    inject_action(&mut app, FlowUiAction::ConfirmCreateMap);
    app.update();
    assert_eq!(current_flow(&app), ClientFlow::MapEditor);
    settle_layout(&mut app);
    app
}

/// The indices of the flat controls drawing the focus ring.
fn flat_focus_rings(app: &mut App) -> Vec<usize> {
    use screens::map_maker::FlatControl;
    let world = app.world_mut();
    world
        .query::<(&FlowButton, &FlatControl, &Outline)>()
        .iter(world)
        .filter(|(_, _, outline)| outline.width != Val::Px(0.0))
        .map(|(button, ..)| button.index)
        .collect()
}

/// BRL-q5sm: Q switches the editor between the map cursor and chrome focus. In chrome focus the
/// arrows walk the laid-out controls (top bar, side-panel tabs, asset rows) with the flat ring,
/// Enter presses the focused control, and Esc returns the arrows to the map cursor.
#[test]
fn map_editor_chrome_focus_walks_the_controls_and_returns_to_the_map_cursor() {
    use screens::map_maker::MapMakerRuntime;
    // Editor control indices (`map_maker/editor.rs`).
    const PLACE: usize = 1001;
    const SELECT: usize = 1002;
    const ASSETS_TAB: usize = 1020;
    const FIRST_ASSET: usize = 1100;
    let mut app = laid_out_map_editor_app();
    // Returns the map cursor on the press frame: later frames may snap it back under the
    // harness window's pointer.
    // Keys go through `InputPlugin`, which the layout harness installs.
    let tap = |app: &mut App, key: KeyCode| {
        let message = |state| KeyboardInput {
            key_code: key,
            logical_key: bevy::input::keyboard::Key::Unidentified(
                bevy::input::keyboard::NativeKey::Unidentified,
            ),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        };
        app.world_mut()
            .write_message(message(bevy::input::ButtonState::Pressed));
        app.update();
        let cursor = app
            .world()
            .resource::<MapMakerRuntime>()
            .focus_test_state()
            .2;
        app.world_mut()
            .write_message(message(bevy::input::ButtonState::Released));
        settle_layout(app);
        cursor
    };
    let state = |app: &App| app.world().resource::<MapMakerRuntime>().focus_test_state();
    let focused = |app: &App| app.world().resource::<FlowNavigation>().focused;
    let ring_on = |app: &mut App| flat_focus_rings(app) == vec![focused(app)];
    let texts = |app: &mut App| {
        let world = app.world_mut();
        world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect::<Vec<_>>()
    };

    // Map-cursor mode: the arrows move the cursor, no control shows the ring, the badge says so.
    let (chrome, _, cursor) = state(&app);
    assert!(!chrome);
    assert!(
        texts(&mut app)
            .iter()
            .any(|text| text.starts_with("Map cursor"))
    );
    assert_eq!(tap(&mut app, KeyCode::ArrowRight).x, cursor.x + 1);
    assert!(flat_focus_rings(&mut app).is_empty());

    // Q enters chrome focus on the active tool; the arrows now move focus, not the cursor.
    tap(&mut app, KeyCode::KeyQ);
    assert!(state(&app).0);
    assert!(
        texts(&mut app)
            .iter()
            .any(|text| text.starts_with("Controls"))
    );
    assert_eq!(focused(&app), PLACE);
    assert!(ring_on(&mut app));
    assert_eq!(
        tap(&mut app, KeyCode::ArrowRight),
        state(&app).2,
        "the map cursor stays put"
    );
    assert_eq!(focused(&app), SELECT);
    assert!(ring_on(&mut app));

    // From the Assets tab, Down reaches the asset rows, and Enter selects the focused row.
    app.world_mut().resource_mut::<FlowNavigation>().focused = ASSETS_TAB;
    let mut visited = Vec::new();
    for _ in 0..8 {
        tap(&mut app, KeyCode::ArrowDown);
        visited.push(focused(&app));
        if focused(&app) >= FIRST_ASSET {
            break;
        }
    }
    assert!(
        focused(&app) >= FIRST_ASSET,
        "down from the tab: {visited:?}"
    );
    tap(&mut app, KeyCode::ArrowDown);
    let row = focused(&app);
    assert!(row > FIRST_ASSET || visited.len() > 1);
    tap(&mut app, KeyCode::Enter);
    assert_eq!(state(&app).1, row - FIRST_ASSET);
    assert!(state(&app).0, "pressing a control keeps chrome focus");

    // Esc returns to the map cursor; the arrows move it again and the ring goes.
    tap(&mut app, KeyCode::Escape);
    assert!(!state(&app).0);
    assert_eq!(
        current_flow(&app),
        ClientFlow::MapEditor,
        "Esc did not leave"
    );
    let before = state(&app).2;
    assert_eq!(tap(&mut app, KeyCode::ArrowRight).x, before.x + 1);
    assert!(flat_focus_rings(&mut app).is_empty());
}

/// Wipeout 3v3, Hot Zone and Heist victories for the local team 1, as Results presents them.
fn results_layout_contexts(
    game_type_id: &brawler::lobby::GameTypeId,
) -> [(&'static str, super::super::ClientMatchResultContext); 3] {
    use brawler::matchplay::{MatchId, MatchResult, ObjectiveResult, ObjectiveTotals};
    let result = MatchResult::TeamVictory {
        team: brawler::combat::TeamId(0),
    };
    let mut wipeout = brawler::matchplay::WipeoutState {
        team_scores: [10, 7],
        target_score: 10,
        ..Default::default()
    };
    for (index, slot) in wipeout.players.iter_mut().take(6).enumerate() {
        let id = u64::try_from(index).unwrap() + 1;
        *slot = Some(brawler::matchplay::WipeoutPlayerStats {
            player_id: brawler::protocol::PlayerId(id),
            network_id: brawler::protocol::NetworkEntityId(id),
            team: brawler::combat::TeamId(u8::try_from(index % 2).unwrap()),
            display_name: format!("Brawler {id}"),
            kills: 4,
            deaths: 2,
        });
    }
    let base = super::super::ClientMatchResultContext {
        result,
        local_team: Some(brawler::combat::TeamId(0)),
        local_player: Some(brawler::protocol::PlayerId(1)),
        // BRL-1sgj: every mode's Results also fits a community map credit.
        published_map: Some(brawler::lobby::PublishedMapCredit {
            published_map_id: brawler::lobby::PublishedMapId::new(4).unwrap(),
            name: "Switchback Crossfire Remix".to_string(),
            author_display_name: "Brawler-1BBD".to_string(),
        }),
        ..match_result_context(game_type_id.clone())
    };
    let objective = |totals| {
        Some(ObjectiveResult {
            match_id: MatchId(1),
            result,
            totals,
        })
    };
    [
        (
            "Wipeout",
            super::super::ClientMatchResultContext {
                wipeout: Some(wipeout),
                ..base.clone()
            },
        ),
        (
            "Hot Zone",
            super::super::ClientMatchResultContext {
                objective: objective(ObjectiveTotals::HotZone {
                    progress: [1800, 927],
                    target: 1800,
                }),
                ..base.clone()
            },
        ),
        (
            "Heist",
            super::super::ClientMatchResultContext {
                objective: objective(ObjectiveTotals::Heist {
                    health: [1800, 0],
                    maximum: [2000, 2000],
                }),
                ..base
            },
        ),
    ]
}

/// BRL-bnxx: on a landscape phone at the default and the smallest UI scale, every mode's Results
/// shows the trophy, outcome header, both team cards and both 48 dp actions without scrolling
/// and without overlap. Desktop 1280x720 keeps its stacked header.
#[test]
fn results_fit_a_landscape_phone_without_scrolling() {
    use crate::settings::persistence::MIN_UI_SCALE;
    use crate::ui::layout_harness::{node_rect, on_screen, overlapping_buttons, short_buttons};
    use screens::results::{ResultsLayoutRole, ResultsView};
    for (width, height, ui_scale) in [
        (868, 411, 1.0),
        (868, 411, MIN_UI_SCALE),
        (914, 411, 1.0),
        (914, 411, MIN_UI_SCALE),
        (1280, 720, 1.0),
    ] {
        let membership = lobby_membership();
        let game_type_id = membership.game_types[0].id.clone();
        for (mode, context) in results_layout_contexts(&game_type_id) {
            let screen = format!(
                "{mode} Results at {width}x{height}, {:.0}%",
                ui_scale * 100.0
            );
            let mut app = configured_lobby_flow_app_with(membership.clone(), |app| {
                crate::ui::layout_harness::add_layout(app, width, height);
                app.insert_resource(crate::ui::touch_target::TouchTargetPolicy { enabled: true });
            });
            crate::ui::layout_harness::apply_viewport_fit(&mut app);
            app.world_mut().resource_mut::<UiScale>().0 *= ui_scale;
            app.world_mut()
                .resource_mut::<super::super::ClientMatchResultState>()
                .context = Some(context);
            app.world_mut()
                .resource_mut::<NextState<ClientFlow>>()
                .set(ClientFlow::Results);
            settle_layout(&mut app);

            let mut roles = app.world_mut().query::<(Entity, &ResultsLayoutRole)>();
            let roles: Vec<_> = roles
                .iter(app.world())
                .map(|(entity, role)| (entity, *role))
                .collect();
            let count = |wanted| roles.iter().filter(|(_, role)| *role == wanted).count();
            assert_eq!(count(ResultsLayoutRole::Trophy), 1, "{screen}");
            assert_eq!(count(ResultsLayoutRole::Headline), 1, "{screen}");
            assert_eq!(count(ResultsLayoutRole::MapCredit), 1, "{screen}");
            assert_eq!(count(ResultsLayoutRole::TeamCard), 2, "{screen}");
            if height >= 640 {
                // Desktop keeps the stacked header: the phone row never applies there.
                assert_eq!(count(ResultsLayoutRole::Header), 0, "{screen}");
                continue;
            }
            assert_eq!(count(ResultsLayoutRole::Header), 1, "{screen}");

            let (content, size) = app
                .world_mut()
                .query_filtered::<&ComputedNode, With<ResultsView>>()
                .single(app.world())
                .map(|node| (node.content_size().y, node.size().y))
                .expect("one Results root");
            assert!(
                content <= size + 0.5,
                "{screen}: content {content} > {size}, so Results would scroll"
            );
            let mut buttons = app.world_mut().query_filtered::<Entity, With<FlowButton>>();
            let buttons: Vec<_> = buttons.iter(app.world()).collect();
            assert_eq!(buttons.len(), 2, "{screen}");
            let parts: Vec<_> = roles
                .iter()
                .filter(|(_, role)| *role != ResultsLayoutRole::Header)
                .map(|(entity, _)| *entity)
                .chain(buttons)
                .collect();
            for &part in &parts {
                assert!(
                    on_screen(app.world_mut(), part),
                    "{screen}: {:?} {part:?} is off screen at {:?}",
                    app.world().get::<ResultsLayoutRole>(part),
                    node_rect(app.world(), part)
                );
            }
            for (index, &a) in parts.iter().enumerate() {
                for &b in &parts[index + 1..] {
                    let overlap = node_rect(app.world(), a)
                        .unwrap()
                        .intersect(node_rect(app.world(), b).unwrap());
                    assert!(
                        overlap.width() <= 1.0 || overlap.height() <= 1.0,
                        "{screen}: {a:?} overlaps {b:?}"
                    );
                }
            }
            let short = short_buttons(
                app.world_mut(),
                crate::ui::touch_target::MIN_TOUCH_TARGET_DP,
            );
            assert!(short.is_empty(), "{screen}: actions under 48 dp: {short:?}");
            let overlapping = overlapping_buttons(app.world_mut(), &[]);
            assert!(overlapping.is_empty(), "{screen}: {overlapping:?}");
        }
    }
}

/// BRL-q148: desktop keeps the authored control heights; the touch floor is off by default.
#[test]
fn desktop_policy_keeps_authored_control_heights() {
    let mut app = flow_test_app_with(|app| {
        crate::ui::layout_harness::add_layout(app, 1280, 720);
    });
    assert!(
        !app.world()
            .resource::<crate::ui::touch_target::TouchTargetPolicy>()
            .enabled
    );
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::ServerSelect);
    settle_layout(&mut app);
    let heights = |app: &mut App| {
        let mut nodes = app
            .world_mut()
            .query_filtered::<(Entity, &Node), With<FlowButton>>();
        let mut heights: Vec<_> = nodes
            .iter(app.world())
            .map(|(entity, node)| (entity, node.min_height))
            .collect();
        heights.sort_by_key(|(entity, _)| *entity);
        heights
    };
    let authored = heights(&mut app);
    assert!(!authored.is_empty());
    // Enabled at a small scale the floor raises every control; disabling restores them.
    app.world_mut()
        .resource_mut::<crate::ui::touch_target::TouchTargetPolicy>()
        .enabled = true;
    app.world_mut().resource_mut::<UiScale>().0 = 0.5;
    settle_layout(&mut app);
    assert!(
        heights(&mut app)
            .iter()
            .all(|(_, height)| *height == px(96))
    );
    app.world_mut()
        .resource_mut::<crate::ui::touch_target::TouchTargetPolicy>()
        .enabled = false;
    app.world_mut().resource_mut::<UiScale>().0 = 1.0;
    settle_layout(&mut app);
    assert_eq!(heights(&mut app), authored);
}

/// BRL-0199 / BRL-0162: after a completed or lost match the client opens a fresh lobby session
/// while the match screen is still presented. If that lobby attempt fails, the real observation
/// path must leave MATCH COMPLETE for the recoverable connection error (Retry / Change Server).
#[test]
fn failed_lobby_return_during_match_presentation_shows_recoverable_connection_error() {
    for (match_lost, expected) in [
        (
            false,
            "Could not reconnect to the lobby after the match. Check your network, then retry.",
        ),
        (
            true,
            "The connection to the match was lost and the lobby could not be reached. Check your network, then retry.",
        ),
    ] {
        let mut app = flow_test_app_with(|app| {
            app.init_resource::<crate::ClientInputContext>();
        });
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::Match);
        app.update();
        assert_eq!(
            *app.world().resource::<State<ClientFlow>>().get(),
            ClientFlow::Match
        );
        app.world_mut().resource_mut::<MatchFailureNotice>().0 = match_lost;
        let generation = {
            let mut routed = app.world_mut().resource_mut::<RoutedClientLifecycle>();
            routed.start_lobby();
            routed.start_lobby();
            routed.start_lobby();
            routed.generation
        };
        let lobby = app
            .world_mut()
            .spawn((
                Client,
                RoutedClientSession {
                    generation,
                    kind: super::super::RoutedClientSessionKind::Lobby,
                },
                ClientJoinStatus {
                    phase: ClientJoinPhase::Disconnected,
                    started_at: Duration::ZERO,
                    disconnect_requested: false,
                },
            ))
            .id();
        app.update();
        app.update();

        let ClientOverlay::Error(error) = app.world().resource::<ClientOverlay>().clone() else {
            panic!("a failed lobby return must show the connection error");
        };
        assert_eq!(error.kind, FlowErrorKind::Connection);
        assert_eq!(error.message, expected);
        assert_eq!(
            error.actions,
            [
                Some(FlowErrorAction::RetryConnection),
                Some(FlowErrorAction::Back),
            ]
        );
        assert_eq!(
            *app.world().resource::<State<ClientFlow>>().get(),
            ClientFlow::ServerSelect
        );
        assert!(app.world().get_entity(lobby).is_err());
        assert_eq!(
            app.world().resource::<RoutedClientLifecycle>().phase,
            RoutedClientPhase::Disabled
        );
        assert!(!app.world().resource::<MatchFailureNotice>().0);
    }
}

/// BRL-hjmq: the match session's connect request times out during Match Loading (the match
/// worker is gone). The real observation path must leave "Match loading / Connecting" for a fresh
/// lobby and show the connection error over the Dashboard, while the lobby session before the
/// handoff is never mistaken for that return.
#[test]
fn match_loading_connect_timeout_shows_the_error_and_returns_to_the_dashboard() {
    let mut app = flow_test_app_with(|app| {
        app.init_resource::<crate::ClientInputContext>();
    });
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::MatchLoading);
    app.update();
    assert_eq!(current_flow(&app), ClientFlow::MatchLoading);

    // Before the handoff the accepted lobby session is not a return to the Dashboard.
    let lobby_generation = {
        let mut routed = app.world_mut().resource_mut::<RoutedClientLifecycle>();
        routed.start_lobby();
        routed.generation
    };
    let lobby = spawn_active_lobby_session(&mut app, lobby_generation);
    app.update();
    app.update();
    assert_eq!(current_flow(&app), ClientFlow::MatchLoading);

    // The handoff replaced the lobby with a match session whose connect request then timed out.
    app.world_mut().despawn(lobby);
    let match_generation = {
        let mut routed = app.world_mut().resource_mut::<RoutedClientLifecycle>();
        routed.generation += 1;
        routed.phase = RoutedClientPhase::Match;
        routed.generation
    };
    let match_session = app
        .world_mut()
        .spawn((
            Client,
            RoutedClientSession {
                generation: match_generation,
                kind: super::super::RoutedClientSessionKind::Match,
            },
            ClientJoinStatus {
                phase: ClientJoinPhase::Disconnected,
                started_at: Duration::ZERO,
                disconnect_requested: false,
            },
        ))
        .id();
    app.update();
    assert_eq!(
        app.world().resource::<RoutedClientLifecycle>().phase,
        RoutedClientPhase::AwaitingMatchUnlink,
        "the lost match session starts the fresh lobby return"
    );
    assert!(app.world().resource::<MatchFailureNotice>().0);
    assert_eq!(current_flow(&app), ClientFlow::MatchLoading);

    // The session transition unlinks the match session and opens a fresh lobby.
    app.world_mut().despawn(match_session);
    let fresh_generation = {
        let mut routed = app.world_mut().resource_mut::<RoutedClientLifecycle>();
        routed.start_lobby();
        routed.generation
    };
    spawn_active_lobby_session(&mut app, fresh_generation);
    app.update();
    app.update();

    assert_eq!(current_flow(&app), ClientFlow::Dashboard);
    let ClientOverlay::Error(error) = app.world().resource::<ClientOverlay>().clone() else {
        panic!("a match connect timeout must show the connection error");
    };
    assert_eq!(error.kind, FlowErrorKind::Connection);
    assert_eq!(error.message, reducer::MATCH_CONNECT_FAILED);
    assert_eq!(error.return_flow, ClientFlow::Dashboard);
    assert_eq!(error.actions, [Some(FlowErrorAction::Back), None]);
    assert!(!app.world().resource::<MatchFailureNotice>().0);
    assert_eq!(count_error_roots(&mut app), 1);

    press_flow_button(&mut app, &FlowUiAction::DismissError);
    app.update();
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::None
    );
    assert_eq!(current_flow(&app), ClientFlow::Dashboard);
}

fn spawn_active_lobby_session(app: &mut App, generation: u64) -> Entity {
    app.world_mut()
        .spawn((
            Client,
            lobby_membership(),
            RoutedClientSession {
                generation,
                kind: super::super::RoutedClientSessionKind::Lobby,
            },
            ClientJoinStatus {
                phase: ClientJoinPhase::LobbyActive {
                    player_id: brawler::protocol::PlayerId(1),
                },
                started_at: Duration::ZERO,
                disconnect_requested: false,
            },
        ))
        .id()
}

/// BRL-0198: one shell screen reached from a fresh connected Dashboard.
struct BackCase {
    screen: &'static str,
    open: Vec<FlowUiAction>,
}

fn shell_back_cases() -> Vec<BackCase> {
    let case = |screen, open| BackCase { screen, open };
    vec![
        case("Choose Game", vec![FlowUiAction::OpenGameTypeSelect]),
        case("Choose Practice Map", vec![FlowUiAction::OpenPracticeMaps]),
        case("Dashboard menu", vec![FlowUiAction::OpenDashboardMenu]),
        case(
            "Change server confirmation",
            vec![
                FlowUiAction::OpenDashboardMenu,
                FlowUiAction::RequestChangeServer,
            ],
        ),
        case("Brawlers", vec![FlowUiAction::OpenBrawlers]),
    ]
}

fn shell_position(app: &App) -> (ClientFlow, ClientOverlay) {
    (
        *app.world().resource::<State<ClientFlow>>().get(),
        app.world().resource::<ClientOverlay>().clone(),
    )
}

fn press_platform_back(app: &mut App) {
    app.world_mut().resource_mut::<crate::BackInput>().request();
    app.update();
    app.update();
}

fn press_escape(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.update();
}

fn opened_shell_screen(case: &BackCase, membership: &ClientLobbyMembership) -> App {
    let mut app = configured_customization_app(membership);
    for action in &case.open {
        inject_action(&mut app, action.clone());
        app.update();
    }
    app
}

#[test]
fn platform_back_steps_back_from_every_shell_screen_exactly_like_escape() {
    let membership = lobby_membership();
    for case in shell_back_cases() {
        let mut back = opened_shell_screen(&case, &membership);
        let opened = shell_position(&back);
        assert_ne!(
            opened,
            (ClientFlow::Dashboard, ClientOverlay::None),
            "{}: setup did not open the screen",
            case.screen
        );
        assert!(
            !back.world().resource::<crate::BackInput>().leaves_app(),
            "{}: an inner screen must keep Back in the app",
            case.screen
        );
        press_platform_back(&mut back);
        let after_back = shell_position(&back);
        assert_ne!(after_back, opened, "{}: Back did nothing", case.screen);

        let mut escape = opened_shell_screen(&case, &membership);
        press_escape(&mut escape);
        assert_eq!(
            after_back,
            shell_position(&escape),
            "{}: Back and Esc diverged",
            case.screen
        );
    }
}

#[test]
fn platform_back_on_root_screens_leaves_the_app_to_the_platform() {
    let mut app = configured_customization_app(&lobby_membership());
    app.update();
    assert!(app.world().resource::<crate::BackInput>().leaves_app());
    press_platform_back(&mut app);
    assert_eq!(
        shell_position(&app),
        (ClientFlow::Dashboard, ClientOverlay::None),
        "the root Dashboard has no in-app Back destination"
    );

    let mut app = flow_test_app();
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::ServerSelect);
    app.update();
    app.update();
    assert!(app.world().resource::<crate::BackInput>().leaves_app());
    // Editing a field: Back cancels the edit first and only then leaves the app.
    app.world_mut().resource_mut::<ServerSelectModel>().editing = Some(EditingField::Address);
    app.update();
    assert!(!app.world().resource::<crate::BackInput>().leaves_app());
    press_platform_back(&mut app);
    assert_eq!(app.world().resource::<ServerSelectModel>().editing, None);
    assert!(app.world().resource::<crate::BackInput>().leaves_app());
}

#[test]
fn back_leaves_the_app_only_from_idle_root_screens() {
    use input::back_leaves_app;
    assert!(back_leaves_app(
        ClientFlow::Dashboard,
        &ClientOverlay::None,
        false
    ));
    assert!(back_leaves_app(
        ClientFlow::ServerSelect,
        &ClientOverlay::None,
        false
    ));
    assert!(!back_leaves_app(
        ClientFlow::ServerSelect,
        &ClientOverlay::None,
        true
    ));
    assert!(!back_leaves_app(
        ClientFlow::Dashboard,
        &ClientOverlay::Settings,
        false
    ));
    assert!(!back_leaves_app(
        ClientFlow::Dashboard,
        &ClientOverlay::DashboardMenu,
        false
    ));
    for flow in [
        ClientFlow::Connecting,
        ClientFlow::GameTypeSelect,
        ClientFlow::PracticeMapSelect,
        ClientFlow::MapLibrary,
        ClientFlow::MapEditor,
        ClientFlow::Queue,
        ClientFlow::MatchLoading,
        ClientFlow::Match,
        ClientFlow::Results,
    ] {
        assert!(
            !back_leaves_app(flow, &ClientOverlay::None, false),
            "{flow:?} must forward Back into the app (BRL-0197 owns the match)"
        );
    }
}

/// The on-screen Back of Choose Game and Choose Practice Map is fully on screen (not only
/// reachable by scrolling) on the phone and the default desktop window, and a tap goes back.
#[test]
fn choose_game_and_practice_map_back_controls_are_on_screen_and_go_back() {
    for (width, height) in [(914, 411), (1280, 720)] {
        for open in [
            FlowUiAction::OpenGameTypeSelect,
            FlowUiAction::OpenPracticeMaps,
        ] {
            let membership = lobby_membership();
            let mut app = configured_lobby_flow_app_with(membership.clone(), |app| {
                crate::ui::layout_harness::add_layout(app, width, height);
            });
            crate::ui::layout_harness::apply_viewport_fit(&mut app);
            app.world_mut()
                .resource_mut::<super::super::ClientProfileModel>()
                .set_snapshot_for_test(membership.profile.clone());
            inject_action(&mut app, open.clone());
            settle_layout(&mut app);
            let screen = format!("{open:?} at {width}x{height}");
            let back = {
                let mut controls = app
                    .world_mut()
                    .query_filtered::<(Entity, &AccessibleLabel), With<Button>>();
                controls
                    .iter(app.world())
                    .find(|(_, label)| label.0 == "Back")
                    .map_or_else(|| panic!("{screen}: no Back control"), |(entity, _)| entity)
            };
            assert!(
                crate::ui::layout_harness::on_screen(app.world_mut(), back),
                "{screen}: Back is not fully on screen"
            );
            assert_eq!(
                app.world().get::<Surface>(back),
                Some(&Surface::Back),
                "{screen}: Back uses the shared surface"
            );
            let center = crate::ui::layout_harness::node_rect(app.world(), back)
                .expect("laid out")
                .center();
            crate::ui::layout_harness::tap(&mut app, center);
            app.update();
            assert_eq!(
                shell_position(&app),
                (ClientFlow::Dashboard, ClientOverlay::None),
                "{screen}: tapping Back did not return to the Dashboard"
            );
        }
    }
}

/// BRL-51zd: the burst reaches the requested action on the active screen only, survives the
/// commit-frame rebuild, and closes after its short window.
#[test]
fn flow_burst_targets_the_active_action_and_expires() {
    use crate::ui::surface::SurfaceBurst;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_millis(100),
        ))
        .insert_resource(ClientOverlay::Brawlers)
        .init_resource::<screens::shared::FlowBurst>()
        .add_systems(Update, screens::shared::apply_flow_burst);
    let button = |index, error_action| {
        (
            Surface::Card,
            FlowButton {
                index,
                action: FlowUiAction::PickCharacter(brawler::characters::CharacterId(
                    u16::try_from(index).unwrap() + 1,
                )),
                error_action,
            },
        )
    };
    let target = app.world_mut().spawn(button(1, true)).id();
    let other = app.world_mut().spawn(button(0, true)).id();
    let behind = app.world_mut().spawn(button(1, false)).id();
    app.world_mut()
        .resource_mut::<screens::shared::FlowBurst>()
        .request(1);
    app.update();
    assert!(app.world().get::<SurfaceBurst>(target).is_some());
    assert!(app.world().get::<SurfaceBurst>(other).is_none());
    assert!(
        app.world().get::<SurfaceBurst>(behind).is_none(),
        "the screen behind the overlay stays still"
    );
    // A rebuild inside the window bursts the new entity too.
    app.world_mut().entity_mut(target).despawn();
    let rebuilt = app.world_mut().spawn(button(1, true)).id();
    app.update();
    assert!(app.world().get::<SurfaceBurst>(rebuilt).is_some());
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(
        app.world()
            .resource::<screens::shared::FlowBurst>()
            .requested(),
        None
    );
    let later = app.world_mut().spawn(button(1, true)).id();
    app.update();
    assert!(
        app.world().get::<SurfaceBurst>(later).is_none(),
        "later rebuilds do not replay the burst"
    );
}

#[derive(Resource, Default)]
struct RecordedUiFeedback(Vec<crate::ui::feedback::UiFeedback>);

fn record_ui_feedback(
    mut feedback: MessageReader<crate::ui::feedback::UiFeedback>,
    mut recorded: ResMut<RecordedUiFeedback>,
) {
    recorded.0.extend(feedback.read().copied());
}

fn take_ui_feedback(app: &mut App) -> Vec<crate::ui::feedback::UiFeedback> {
    std::mem::take(&mut app.world_mut().resource_mut::<RecordedUiFeedback>().0)
}

fn record_feedback_in(app: &mut App) {
    app.init_resource::<RecordedUiFeedback>()
        .add_systems(Last, record_ui_feedback);
}

fn press_key(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keyboard.release(key);
    keyboard.clear();
    app.update();
}

#[test]
fn escape_and_platform_back_publish_one_back_feedback_on_every_shell_screen() {
    use crate::ui::feedback::UiFeedback;
    let membership = lobby_membership();
    for case in shell_back_cases() {
        for platform in [false, true] {
            let mut app = opened_shell_screen(&case, &membership);
            record_feedback_in(&mut app);
            app.update();
            take_ui_feedback(&mut app);
            if platform {
                press_platform_back(&mut app);
            } else {
                press_escape(&mut app);
            }
            assert_eq!(
                take_ui_feedback(&mut app),
                vec![UiFeedback::Back],
                "{} (platform Back: {platform})",
                case.screen
            );
        }
    }
    // The root Dashboard has no in-app Back destination, so Back is silent there.
    let mut app = configured_customization_app(&membership);
    record_feedback_in(&mut app);
    app.update();
    press_escape(&mut app);
    assert_eq!(take_ui_feedback(&mut app), Vec::new());
}

#[test]
fn dashboard_keyboard_moves_and_activation_publish_feedback_with_the_surface_role() {
    use crate::ui::{feedback::UiFeedback, surface::Surface};
    let mut app = configured_customization_app(&lobby_membership());
    record_feedback_in(&mut app);
    // The test Dashboard was entered before the lobby membership existed; re-enter it so its
    // controls are spawned.
    inject_action(&mut app, FlowUiAction::OpenGameTypeSelect);
    app.update();
    press_escape(&mut app);
    assert_eq!(
        shell_position(&app),
        (ClientFlow::Dashboard, ClientOverlay::None)
    );
    app.update();
    take_ui_feedback(&mut app);
    let mut buttons = app
        .world_mut()
        .query::<(&FlowButton, Has<InteractionDisabled>)>();
    let enabled = buttons
        .iter(app.world())
        .filter(|(_, disabled)| !disabled)
        .map(|(button, _)| button.index)
        .collect::<Vec<_>>();
    assert!(enabled.contains(&DASHBOARD_PLAY_INDEX), "{enabled:?}");
    assert!(enabled.contains(&DASHBOARD_PRACTICE_INDEX), "{enabled:?}");
    app.world_mut().resource_mut::<FlowNavigation>().focused = DASHBOARD_PLAY_INDEX;

    press_key(&mut app, KeyCode::ArrowDown);
    assert_eq!(
        app.world().resource::<FlowNavigation>().focused,
        DASHBOARD_PRACTICE_INDEX
    );
    assert_eq!(take_ui_feedback(&mut app), vec![UiFeedback::Moved]);

    // Practice is the bottom of the column: an edge press does not move focus and is silent.
    press_key(&mut app, KeyCode::ArrowDown);
    assert_eq!(take_ui_feedback(&mut app), Vec::new());

    press_key(&mut app, KeyCode::ArrowUp);
    take_ui_feedback(&mut app);
    let play_surface = {
        let mut buttons = app
            .world_mut()
            .query::<(&FlowButton, Option<&Surface>, Has<InteractionDisabled>)>();
        buttons
            .iter(app.world())
            .find(|(button, _, disabled)| button.index == DASHBOARD_PLAY_INDEX && !disabled)
            .and_then(|(_, surface, _)| surface.copied())
    };
    assert_eq!(play_surface, Some(Surface::Primary));
    press_key(&mut app, KeyCode::Enter);
    assert_eq!(
        take_ui_feedback(&mut app),
        vec![UiFeedback::Activated(Some(Surface::Primary))]
    );
}

#[test]
fn server_select_editor_commit_and_cancel_publish_feedback_and_a_rejected_name_is_silent() {
    use crate::ui::{feedback::UiFeedback, surface::Surface};
    let mut app = flow_test_app();
    record_feedback_in(&mut app);
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::ServerSelect);
    app.update();
    take_ui_feedback(&mut app);
    let edit = |app: &mut App, field: EditingField, name: Option<&str>| {
        let mut model = app.world_mut().resource_mut::<ServerSelectModel>();
        if let Some(name) = name {
            model.name = name.to_string();
            model.caret = model.name.len();
        }
        model.editing = Some(field);
    };

    edit(&mut app, EditingField::Address, None);
    press_key(&mut app, KeyCode::Enter);
    assert_eq!(
        take_ui_feedback(&mut app),
        vec![UiFeedback::Activated(Some(Surface::Primary))]
    );

    edit(&mut app, EditingField::Name, Some("Renamed"));
    press_key(&mut app, KeyCode::Enter);
    assert_eq!(
        app.world().resource::<ServerSelectModel>().committed_name,
        "Renamed"
    );
    assert_eq!(
        take_ui_feedback(&mut app),
        vec![UiFeedback::Activated(Some(Surface::Primary))]
    );

    edit(&mut app, EditingField::Name, Some("Ab\u{E000}cd"));
    press_key(&mut app, KeyCode::Enter);
    assert!(
        app.world()
            .resource::<ServerSelectModel>()
            .inline_error
            .is_some()
    );
    assert_eq!(
        take_ui_feedback(&mut app),
        Vec::new(),
        "a rejected name plays nothing"
    );

    edit(&mut app, EditingField::Name, None);
    press_escape(&mut app);
    assert_eq!(app.world().resource::<ServerSelectModel>().editing, None);
    assert_eq!(take_ui_feedback(&mut app), vec![UiFeedback::Back]);

    // Ordinary typing inside the field is silent.
    edit(&mut app, EditingField::Address, None);
    press_key(&mut app, KeyCode::ArrowLeft);
    assert_eq!(take_ui_feedback(&mut app), Vec::new());
}

/// BRL-nqnm: a lobby app with the real shell, in a match, with Settings opened from the
/// match menu.
fn match_settings_app(membership: &ClientLobbyMembership, name: &str) -> App {
    let path = std::env::temp_dir().join(format!(
        "brawler-brl-nqnm-{name}-{}-settings.ron",
        std::process::id()
    ));
    let mut app = configured_lobby_flow_app_with(membership.clone(), |app| {
        crate::shell::flow_test_support::add_shell(app, path);
    });
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Match);
    app.update();
    assert_eq!(current_flow(&app), ClientFlow::Match);
    *app.world_mut()
        .resource_mut::<super::super::ClientInputContext>() =
        super::super::ClientInputContext::Menu;
    app.world_mut()
        .resource_mut::<super::super::MatchSettingsRequest>()
        .0 = true;
    app.update();
    app.update();
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::Settings
    );
    assert_eq!(
        crate::shell::flow_test_support::overlay_root_count(app.world_mut()),
        1
    );
    assert!(crate::shell::flow_test_support::has_settings_drafts(
        app.world()
    ));
    assert!(crate::shell::flow_test_support::returns_to_match_menu(
        app.world()
    ));
    app
}

fn assert_match_settings_closed(app: &mut App) {
    assert_eq!(
        crate::shell::flow_test_support::overlay_root_count(app.world_mut()),
        0,
        "the match-owned Settings outlived the match"
    );
    assert!(!crate::shell::flow_test_support::has_settings_drafts(
        app.world()
    ));
    assert!(!crate::shell::flow_test_support::returns_to_match_menu(
        app.world()
    ));
    assert_eq!(
        *app.world().resource::<super::super::ClientInputContext>(),
        super::super::ClientInputContext::Shell
    );
}

#[test]
fn match_completion_closes_settings_opened_from_the_match_menu() {
    let membership = lobby_membership();
    let game_type_id = membership.game_types[0].id.clone();
    let mut app = match_settings_app(&membership, "results");
    app.world_mut()
        .resource_mut::<super::super::ClientMatchResultState>()
        .context = Some(match_result_context(game_type_id));

    inject_session_observation(&mut app, SessionObservation::FreshLobbyReturn);
    app.update();
    app.update();

    assert_eq!(current_flow(&app), ClientFlow::Results);
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::None
    );
    assert_match_settings_closed(&mut app);
    press_flow_button(&mut app, &FlowUiAction::ReturnToDashboard);
    app.update();
    assert_eq!(current_flow(&app), ClientFlow::Dashboard);
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::None
    );
    assert_match_settings_closed(&mut app);
}

#[test]
fn connection_lost_over_match_settings_leaves_only_the_error_interactive() {
    let membership = lobby_membership();
    let mut app = match_settings_app(&membership, "error");
    app.world_mut()
        .resource_mut::<RoutedClientLifecycle>()
        .phase = RoutedClientPhase::Match;

    inject_session_observation(&mut app, SessionObservation::MatchFailed);
    inject_session_observation(&mut app, SessionObservation::FreshLobbyReturn);
    app.update();
    app.update();

    let ClientOverlay::Error(error) = app.world().resource::<ClientOverlay>().clone() else {
        panic!("the lost match must show the connection error");
    };
    assert_eq!(error.kind, FlowErrorKind::Connection);
    assert_eq!(current_flow(&app), ClientFlow::Dashboard);
    assert_eq!(count_error_roots(&mut app), 1);
    assert_eq!(
        crate::shell::flow_test_support::active_shell_buttons(app.world_mut()),
        0
    );
    assert_match_settings_closed(&mut app);

    press_flow_button(&mut app, &FlowUiAction::DismissError);
    app.update();
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::None
    );
    assert_eq!(count_error_roots(&mut app), 0);
    assert_eq!(current_flow(&app), ClientFlow::Dashboard);
    assert_match_settings_closed(&mut app);
}

fn commit_ime_text(app: &mut App, value: &str) {
    app.world_mut().write_message(Ime::Commit {
        window: Entity::PLACEHOLDER,
        value: value.to_string(),
    });
    app.update();
}

fn published_text_entry(app: &App) -> Option<(crate::TextEntryField, String)> {
    app.world()
        .resource::<crate::TextEntryFocus>()
        .active()
        .map(|entry| (entry.field(), entry.text().to_string()))
}

/// BRL-06pz: on-screen keyboard text arrives as `Ime::Commit` and keeps each field's bounds.
#[test]
fn ime_commits_edit_the_server_address_and_name_within_their_bounds() {
    let mut app = flow_test_app();
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::ServerSelect);
    app.update();
    {
        let mut model = app.world_mut().resource_mut::<ServerSelectModel>();
        model.address = "10.0".to_string();
        model.editing = Some(EditingField::Address);
        model.caret = 4;
    }
    app.update();
    assert_eq!(
        published_text_entry(&app),
        Some((crate::TextEntryField::ServerAddress, "10.0".to_string()))
    );

    commit_ime_text(&mut app, ".0.1:5000");
    let model = app.world().resource::<ServerSelectModel>();
    assert_eq!((model.address.as_str(), model.caret), ("10.0.0.1:5000", 13));
    assert_eq!(
        published_text_entry(&app),
        Some((
            crate::TextEntryField::ServerAddress,
            "10.0.0.1:5000".to_string()
        ))
    );
    // Address validation still rejects non-ASCII committed text.
    commit_ime_text(&mut app, "é");
    let model = app.world().resource::<ServerSelectModel>();
    assert_eq!(model.address, "10.0.0.1:5000");
    assert!(model.inline_error.is_some());

    {
        let mut model = app.world_mut().resource_mut::<ServerSelectModel>();
        model.name = "Zo".to_string();
        model.editing = Some(EditingField::Name);
        model.caret = 2;
        model.inline_error = None;
    }
    commit_ime_text(&mut app, "ë");
    assert_eq!(app.world().resource::<ServerSelectModel>().name, "Zoë");
    // The name keeps its 64-byte bound for committed text.
    commit_ime_text(&mut app, &"n".repeat(64));
    let model = app.world().resource::<ServerSelectModel>();
    assert_eq!(model.name, "Zoë");
    assert!(model.inline_error.is_some());
    assert_eq!(
        published_text_entry(&app),
        Some((crate::TextEntryField::PlayerName, "Zoë".to_string()))
    );

    app.world_mut().resource_mut::<ServerSelectModel>().editing = None;
    app.update();
    assert_eq!(published_text_entry(&app), None);
}

/// BRL-fe7t: a touch hides the focus frame without moving navigation focus, and the next
/// keyboard or gamepad navigation shows it again and continues from the same control.
#[test]
fn touch_hides_focus_and_navigation_resumes_where_it_was() {
    use crate::ui::focus_visibility::{FocusVisibility, track_focus_visibility};
    use bevy::input::{
        ButtonState,
        gamepad::GamepadButtonStateChangedEvent,
        keyboard::Key,
        mouse::MouseButtonInput,
        touch::{TouchInput, TouchPhase},
    };
    let mut app = phone_layout_flow_app();
    app.add_message::<MouseButtonInput>()
        .add_message::<GamepadButtonStateChangedEvent>()
        .init_resource::<FocusVisibility>()
        .add_systems(PreUpdate, track_focus_visibility);
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::ServerSelect);
    settle_layout(&mut app);
    app.world_mut().resource_mut::<FlowNavigation>().focused = 1;
    app.update();
    let focused_states = |app: &mut App| {
        let mut buttons = app
            .world_mut()
            .query::<(&FlowButton, &crate::ui::surface::SurfaceState)>();
        buttons
            .iter(app.world())
            .filter(|(_, state)| state.focused)
            .map(|(button, _)| button.index)
            .collect::<Vec<_>>()
    };
    assert_eq!(focused_states(&mut app), [1]);

    // A finger on empty space: focus hides, navigation and the focused state stay put.
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<bevy::window::PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut().write_message(TouchInput {
        phase: TouchPhase::Started,
        position: Vec2::new(2.0, 2.0),
        window,
        force: None,
        id: 7,
    });
    app.update();
    assert!(!app.world().resource::<FocusVisibility>().visible);
    assert_eq!(app.world().resource::<FlowNavigation>().focused, 1);
    assert_eq!(focused_states(&mut app), [1]);

    // The next keyboard navigation shows focus again and moves on from the same control.
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::ArrowDown,
        logical_key: Key::ArrowDown,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
    app.update();
    assert!(app.world().resource::<FocusVisibility>().visible);
    assert_eq!(
        focused_states(&mut app),
        [2],
        "navigation continues from the control the touch left focused"
    );
}

/// BRL-wr0s: the first tap after launch activates a Server Select control on the phone viewport,
/// however its touch start and end fall across frames and layout.
mod first_tap {
    use super::*;
    use bevy::input::touch::{TouchInput, TouchPhase};

    fn server_select_app() -> App {
        let mut app = phone_layout_flow_app();
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::ServerSelect);
        app
    }

    fn quit(app: &mut App) -> Entity {
        let mut buttons = app.world_mut().query::<(Entity, &FlowButton)>();
        buttons
            .iter(app.world())
            .find(|(_, button)| button.action == FlowUiAction::Quit)
            .map(|(entity, _)| entity)
            .expect("Server Select QUIT")
    }

    fn quit_center() -> Vec2 {
        let mut app = server_select_app();
        settle_layout(&mut app);
        let quit = quit(&mut app);
        crate::ui::layout_harness::node_rect(app.world(), quit)
            .unwrap()
            .center()
    }

    fn send(app: &mut App, phase: TouchPhase, position: Vec2) {
        let window = app
            .world_mut()
            .query_filtered::<Entity, With<bevy::window::PrimaryWindow>>()
            .single(app.world())
            .unwrap();
        app.world_mut().write_message(TouchInput {
            phase,
            position,
            window,
            force: None,
            id: 1,
        });
    }

    /// Runs `frames` frames, counting the app exits requested along the way.
    fn exits(app: &mut App, frames: usize) -> usize {
        (0..frames)
            .map(|_| {
                app.update();
                let mut reader = app
                    .world_mut()
                    .remove_resource::<ExitReader>()
                    .unwrap_or_default();
                let count = reader
                    .0
                    .read(app.world().resource::<Messages<AppExit>>())
                    .count();
                app.world_mut().insert_resource(reader);
                count
            })
            .sum()
    }

    #[derive(Resource, Default)]
    struct ExitReader(bevy::ecs::message::MessageCursor<AppExit>);

    #[test]
    fn a_tap_within_one_frame_on_server_select_quit_quits_once() {
        let at = quit_center();
        let mut app = server_select_app();
        settle_layout(&mut app);
        send(&mut app, TouchPhase::Started, at);
        send(&mut app, TouchPhase::Ended, at);
        assert_eq!(exits(&mut app, 4), 1);
    }

    #[test]
    fn a_press_and_release_on_separate_frames_on_server_select_quit_quits_once() {
        let at = quit_center();
        let mut app = server_select_app();
        settle_layout(&mut app);
        send(&mut app, TouchPhase::Started, at);
        let mut count = exits(&mut app, 1);
        send(&mut app, TouchPhase::Ended, at);
        count += exits(&mut app, 4);
        assert_eq!(count, 1);
    }

    #[test]
    fn a_finger_down_before_server_select_is_laid_out_still_quits_on_release() {
        let at = quit_center();
        let mut app = server_select_app();
        // The finger lands on the launch frame that first builds the screen, before layout.
        send(&mut app, TouchPhase::Started, at);
        let mut count = exits(&mut app, 1);
        settle_layout(&mut app);
        send(&mut app, TouchPhase::Ended, at);
        count += exits(&mut app, 4);
        assert_eq!(count, 1);
    }

    #[test]
    fn a_ui_scale_change_mid_press_keeps_server_select_quit_and_quits_once() {
        let at = quit_center();
        let mut app = server_select_app();
        settle_layout(&mut app);
        let before = quit(&mut app);
        send(&mut app, TouchPhase::Started, at);
        let mut count = exits(&mut app, 1);
        // The shell's viewport fit settles once the real window height arrives.
        app.world_mut().resource_mut::<UiScale>().0 *= 1.02;
        count += exits(&mut app, 2);
        assert_eq!(
            quit(&mut app),
            before,
            "a scale change does not rebuild the screen"
        );
        send(&mut app, TouchPhase::Ended, at);
        count += exits(&mut app, 4);
        assert_eq!(count, 1);
    }
}

/// The Dashboard error that a lost, unresumable match leaves behind (`match_flow.rs`).
fn lost_match_dashboard_error() -> ClientOverlay {
    ClientOverlay::Error(FlowError {
        kind: FlowErrorKind::Connection,
        message: "The connection to the match was lost, and the match could not be resumed."
            .to_string(),
        return_flow: ClientFlow::Dashboard,
        actions: [Some(FlowErrorAction::Back), None],
    })
}

fn cancel_match_start_overlay() -> ClientOverlay {
    ClientOverlay::Confirmation(CancelMatchStartConfirmation {
        reservation_id: brawler::lobby::MatchReservationId::new(1).unwrap(),
        generation: 1,
    })
}

fn overlay_root<C: Component>(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut roots = world.query_filtered::<Entity, With<C>>();
    let found: Vec<_> = roots.iter(world).collect();
    assert_eq!(found.len(), 1, "exactly one overlay root");
    found[0]
}

fn overlay_panel_entity(app: &App, root: Entity) -> Entity {
    let world = app.world();
    world
        .entity(root)
        .get::<Children>()
        .into_iter()
        .flatten()
        .copied()
        .find(|child| world.get::<Surface>(*child) == Some(&Surface::Panel))
        .expect("the overlay content sits in the shell Surface::Panel")
}

/// BRL-zvqd: every in-flow overlay uses the shell panel, the shell font and Surface-role
/// buttons, like the full-screen Connection Lost surface.
fn assert_shell_overlay(app: &mut App, root: Entity, screen: &str) {
    let font = app
        .world()
        .resource::<crate::ClientAssetHandles>()
        .dashboard_font
        .clone();
    overlay_panel_entity(app, root);
    let world = app.world_mut();
    let mut entities = Vec::new();
    let mut pending = vec![root];
    while let Some(entity) = pending.pop() {
        entities.push(entity);
        pending.extend(world.get::<Children>(entity).into_iter().flatten().copied());
    }
    let mut texts = 0;
    let mut buttons = 0;
    for entity in entities {
        if world.get::<Text>(entity).is_some() {
            texts += 1;
            assert_eq!(
                world.get::<TextFont>(entity).map(|text| text.font.clone()),
                Some(FontSource::Handle(font.clone())),
                "{screen}: text {:?} uses the shell font",
                world.get::<Text>(entity).map(|text| text.0.clone())
            );
        }
        if world.get::<FlowButton>(entity).is_some() {
            buttons += 1;
            assert!(
                world.get::<Surface>(entity).is_some(),
                "{screen}: every overlay button has a Surface role"
            );
        }
    }
    assert!(
        texts >= 2 && buttons >= 1,
        "{screen}: {texts} texts, {buttons} buttons"
    );
}

#[test]
fn dashboard_error_and_cancel_confirmation_use_the_shell_panel_font_and_surfaces() {
    let font = bevy::asset::uuid_handle!("6f1f5c7e-2d6b-4a4e-9a51-0f4d3f6f9b21");
    let mut app = configured_lobby_flow_app(lobby_membership());
    app.insert_resource(crate::ClientAssetHandles::with_font_for_test(font));

    *app.world_mut().resource_mut::<ClientOverlay>() = lost_match_dashboard_error();
    app.update();
    let root = overlay_root::<FlowErrorRoot>(&mut app);
    assert_shell_overlay(&mut app, root, "Dashboard connection error");
    let world = app.world_mut();
    let mut buttons = world.query::<(&FlowButton, &Surface)>();
    let back: Vec<_> = buttons
        .iter(world)
        .filter(|(button, _)| button.error_action)
        .map(|(button, surface)| (button.index, button.action.clone(), *surface))
        .collect();
    assert_eq!(
        back,
        [(1_000, FlowUiAction::DismissError, Surface::Secondary)]
    );
    assert_eq!(app.world().resource::<FlowNavigation>().focused, 1_000);

    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Error(FlowError {
        kind: FlowErrorKind::Queue,
        message: "Queue commands are temporarily limited".to_string(),
        return_flow: ClientFlow::Dashboard,
        actions: [
            Some(FlowErrorAction::TryAgainQueue),
            Some(FlowErrorAction::Disconnect),
        ],
    });
    app.update();
    let root = overlay_root::<FlowErrorRoot>(&mut app);
    assert_shell_overlay(&mut app, root, "Rate-limited queue error");
    let world = app.world_mut();
    let mut try_again =
        world.query_filtered::<(&FlowButton, &Surface, Has<InteractionDisabled>), With<RateLimitTryAgain>>();
    let (button, surface, disabled) = try_again.single(world).expect("one Try Again button");
    assert_eq!(
        (button.index, &button.action, *surface),
        (1_000, &FlowUiAction::TryAgainQueue, Surface::Primary)
    );
    assert!(
        !disabled,
        "no rate-limit window is pending, so Try Again is enabled"
    );

    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::None;
    app.update();
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::MatchLoading);
    *app.world_mut().resource_mut::<ClientOverlay>() = cancel_match_start_overlay();
    app.update();
    app.update();
    let root = overlay_root::<screens::overlays::CancelConfirmationRoot>(&mut app);
    assert_shell_overlay(&mut app, root, "Cancel match start");
}

/// BRL-zvqd: the restyled overlays fit the Pixel 8a landscape viewports and the default desktop
/// window with every action reachable and the panel fully on screen.
#[test]
fn shell_overlays_fit_phone_and_desktop_viewports() {
    for (width, height) in [(868, 411), (914, 411), (1280, 720)] {
        let membership = lobby_membership();
        let mut app = configured_lobby_flow_app_with(membership.clone(), |app| {
            crate::ui::layout_harness::add_layout(app, width, height);
        });
        crate::ui::layout_harness::apply_viewport_fit(&mut app);
        app.world_mut()
            .resource_mut::<super::super::ClientProfileModel>()
            .set_snapshot_for_test(membership.profile.clone());
        let screen = format!("Dashboard error at {width}x{height}");
        *app.world_mut().resource_mut::<ClientOverlay>() = lost_match_dashboard_error();
        assert_phone_reachable(&mut app, &screen);
        let root = overlay_root::<FlowErrorRoot>(&mut app);
        let panel = overlay_panel_entity(&app, root);
        assert!(
            crate::ui::layout_harness::on_screen(app.world_mut(), panel),
            "{screen}: the panel is not fully on screen"
        );

        let screen = format!("Two-action error at {width}x{height}");
        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Error(FlowError {
            kind: FlowErrorKind::Persistence,
            message: "Your brawler changes could not be saved to this device. Check free storage, then retry or continue without saving.".to_string(),
            return_flow: ClientFlow::Dashboard,
            actions: [
                Some(FlowErrorAction::RetrySave),
                Some(FlowErrorAction::ContinueWithoutSaving),
            ],
        });
        assert_phone_reachable(&mut app, &screen);
        let root = overlay_root::<FlowErrorRoot>(&mut app);
        let panel = overlay_panel_entity(&app, root);
        assert!(
            crate::ui::layout_harness::on_screen(app.world_mut(), panel),
            "{screen}: the panel is not fully on screen"
        );

        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::None;
        app.update();
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::MatchLoading);
        *app.world_mut().resource_mut::<ClientOverlay>() = cancel_match_start_overlay();
        let screen = format!("Cancel confirmation at {width}x{height}");
        assert_phone_reachable(&mut app, &screen);
        let root = overlay_root::<screens::overlays::CancelConfirmationRoot>(&mut app);
        let panel = overlay_panel_entity(&app, root);
        assert!(
            crate::ui::layout_harness::on_screen(app.world_mut(), panel),
            "{screen}: the panel is not fully on screen"
        );
    }
}

/// BRL-pw9k: every shell-capture route step names exactly the screen the reducer commits for
/// its action, from an idle connected Dashboard. A character pick settles as the server would
/// accept it.
#[cfg(feature = "automation")]
#[test]
fn shell_capture_route_steps_match_the_reducer() {
    let membership = lobby_membership();
    // Live-match routes wait on the match server; `live_shell_capture_routes_leave_the_match`
    // covers their tables.
    for route in ShellCaptureRoute::ALL
        .into_iter()
        .filter(|route| !route.live())
    {
        // The Map Maker views load icons, previews and their body font, so this harness carries
        // an asset server.
        let mut app = configured_lobby_flow_app_with(membership.clone(), |app| {
            app.add_plugins(AssetPlugin::default())
                .init_asset::<Image>()
                .init_asset::<Font>()
                .init_resource::<brawler::map::MapCatalogResource>();
        });
        app.world_mut()
            .resource_mut::<super::super::ClientProfileModel>()
            .set_snapshot_for_test(membership.profile.clone());
        app.update();
        assert_eq!(
            shell_position(&app),
            (ClientFlow::Dashboard, ClientOverlay::None),
            "{}: the route starts from an idle Dashboard",
            route.name()
        );
        for (index, step) in route.steps().iter().enumerate() {
            let action = step
                .action()
                .unwrap_or_else(|| panic!("{} step {index}: no action", route.name()));
            inject_action(&mut app, action);
            app.update();
            accept_pending_pick(&mut app, &membership);
            if step.expected().0 == ClientFlow::Queue {
                // The Queue screen opens on the server's join acceptance, not on the request.
                assert!(
                    app.world()
                        .resource::<super::super::ClientQueueModel>()
                        .pending()
                        .is_some(),
                    "{} step {index}: the join was not requested",
                    route.name()
                );
                inject_session_observation(
                    &mut app,
                    SessionObservation::QueueOutcome(brawler::lobby::QueueCommandOutcome {
                        request_id: brawler::lobby::QueueRequestId::new(1).unwrap(),
                        decision: brawler::lobby::QueueDecision::Joined(queue_membership(
                            &membership,
                        )),
                    }),
                );
                app.update();
            }
            let (flow, overlay) = shell_position(&app);
            assert_eq!(
                (flow, overlay, false),
                step.expected(),
                "{} step {index}",
                route.name()
            );
        }
    }
}

fn published_summary(
    id: u64,
    name: &str,
    listing: brawler::lobby::PublishedMapListing,
) -> brawler::lobby::PublishedMapSummary {
    brawler::lobby::PublishedMapSummary {
        published_map_id: brawler::lobby::PublishedMapId::new(id).unwrap(),
        name: name.to_string(),
        game_type_id: brawler::lobby::GameTypeId::new("wipeout-1v1").unwrap(),
        listing,
    }
}

/// The newest (pending) map is selected; Withdraw first asks for confirmation, and only the
/// confirmed press queues one withdraw request for the server.
fn assert_withdraw_needs_confirmation(app: &mut App) {
    use brawler::lobby::PublishedMapsAction;
    use screens::map_maker::MapMakerRuntime;
    // The newest map is selected; Withdraw first asks for confirmation.
    let pending = brawler::lobby::PublishedMapId::new(4).unwrap();
    {
        let world = app.world_mut();
        let enabled = world
            .query_filtered::<&FlowButton, Without<InteractionDisabled>>()
            .iter(world)
            .any(|button| button.action == FlowUiAction::WithdrawPublishedMap(pending));
        assert!(enabled, "the pending map offers Withdraw");
    }
    inject_action(app, FlowUiAction::WithdrawPublishedMap(pending));
    settle_layout(app);
    assert!(visible_text(app).iter().any(|line| line == "WITHDRAW MAP?"));
    assert!(
        app.world()
            .resource::<crate::ClientPublishedMapsModel>()
            .outbound_for_test()
            .is_empty()
    );
    inject_action(app, FlowUiAction::WithdrawPublishedMap(pending));
    let outbound = app
        .world()
        .resource::<crate::ClientPublishedMapsModel>()
        .outbound_for_test();
    assert_eq!(outbound.len(), 1);
    assert_eq!(
        outbound[0].action,
        PublishedMapsAction::Withdraw {
            published_map_id: pending
        }
    );
    assert_eq!(
        app.world().resource::<MapMakerRuntime>().editing_map_name(),
        None
    );
}

/// BRL-s3dh: My Maps' Published view shows each of this account's maps with the state chip the
/// server reported, keeps 48 dp controls on a landscape phone, and withdraws a pending map only
/// after its confirmation, through one request the server decides.
#[test]
fn published_view_shows_server_states_and_confirms_a_withdraw() {
    use brawler::lobby::PublishedMapListing;
    let maps = vec![
        published_summary(4, "Neon Crossing", PublishedMapListing::Pending),
        published_summary(
            3,
            "Moonlit Yard",
            PublishedMapListing::Removed {
                reason: Some(brawler::lobby::WITHDRAWN_BY_AUTHOR_REASON.to_string()),
            },
        ),
        published_summary(
            2,
            "Crate Maze",
            PublishedMapListing::Rejected {
                reason: "Both spawns face a wall.".to_string(),
            },
        ),
        published_summary(
            1,
            "Riverside Rumble",
            PublishedMapListing::Approved { in_rotation: true },
        ),
    ];
    for (width, height) in [(914, 411), (1280, 720)] {
        let mut app = configured_lobby_flow_app_with(lobby_membership(), |app| {
            crate::ui::layout_harness::add_layout(app, width, height);
            app.insert_resource(crate::ui::touch_target::TouchTargetPolicy { enabled: true });
            app.insert_resource(brawler::map::MapCatalogResource(
                brawler::map::MapContentCatalog::embedded().unwrap(),
            ));
        });
        crate::ui::layout_harness::apply_viewport_fit(&mut app);
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::MapLibrary);
        app.update();
        // A lobby session exists (bound by a first listing) when the view opens.
        app.world_mut()
            .resource_mut::<crate::ClientPublishedMapsModel>()
            .listed_for_test(1, Vec::new());
        inject_action(&mut app, FlowUiAction::ShowPublishedMaps(true));
        app.update();
        assert!(
            app.world()
                .resource::<crate::ClientPublishedMapsModel>()
                .pending(),
            "opening the view asks the server"
        );
        // Answer with this account's listing.
        app.world_mut()
            .resource_mut::<crate::ClientPublishedMapsModel>()
            .listed_for_test(1, maps.clone());
        settle_layout(&mut app);
        let text = visible_text(&mut app);
        for expected in [
            "Published (4)",
            "PENDING",
            "REMOVED",
            "REJECTED",
            "IN ROTATION",
            "Rejected: Both spawns face a wall.",
            brawler::lobby::WITHDRAWN_BY_AUTHOR_REASON,
        ] {
            assert!(
                text.iter().any(|line| line == expected),
                "{width}x{height}: missing {expected:?} in {text:?}"
            );
        }
        let short = crate::ui::layout_harness::short_buttons(
            app.world_mut(),
            crate::ui::touch_target::MIN_TOUCH_TARGET_DP,
        );
        assert!(short.is_empty(), "{width}x{height}: under 48 dp: {short:?}");

        assert_withdraw_needs_confirmation(&mut app);
        // A removed map offers no Withdraw.
        inject_action(
            &mut app,
            FlowUiAction::SelectPublishedMap(brawler::lobby::PublishedMapId::new(3).unwrap()),
        );
        settle_layout(&mut app);
        let world = app.world_mut();
        let withdraw = world
            .query::<&FlowButton>()
            .iter(world)
            .any(|button| matches!(button.action, FlowUiAction::WithdrawPublishedMap(_)));
        assert!(
            !withdraw,
            "{width}x{height}: a removed map cannot be withdrawn"
        );
    }
}

fn character_choice(
    character: u16,
    ultimate: u16,
    passive: u16,
) -> brawler::characters::CharacterChoice {
    brawler::characters::CharacterChoice {
        character_id: brawler::characters::CharacterId(character),
        ultimate_id: brawler::builds::UltimateDefinitionId(ultimate),
        passive_id: brawler::builds::PassiveDefinitionId(passive),
    }
}

fn sent_picks(app: &App) -> Vec<brawler::characters::CharacterChoice> {
    app.world()
        .resource::<super::super::ClientProfileModel>()
        .queued_commands_for_test()
        .iter()
        .map(|command| command.choice)
        .collect()
}

#[test]
fn practice_queue_again_uses_fresh_generation_game_and_profile_revision() {
    let membership = lobby_membership();
    let game_type_id = membership.game_types[0].id.clone();
    let mut app = configured_lobby_flow_app(membership);
    let mut fresh_membership = lobby_membership();
    fresh_membership.game_types[0].id = brawler::lobby::GameTypeId::new("stale-wipeout").unwrap();
    fresh_membership.profile.revision = brawler::profiles::ProfileRevision::new(9).unwrap();
    app.world_mut().spawn((
        Client,
        fresh_membership,
        RoutedClientSession {
            generation: 2,
            kind: super::super::RoutedClientSessionKind::Lobby,
        },
    ));
    app.world_mut()
        .resource_mut::<RoutedClientLifecycle>()
        .generation = 1;
    *app.world_mut().resource_mut::<SessionPurpose>() = SessionPurpose::Practice;
    *app.world_mut().resource_mut::<SelectedGameType>() = SelectedGameType::default();
    app.world_mut()
        .resource_mut::<super::super::ClientMatchResultState>()
        .context = Some(match_result_context(game_type_id.clone()));

    inject_action(&mut app, FlowUiAction::QueueAgain);

    let request = app
        .world()
        .resource::<super::super::ClientPracticeModel>()
        .pending_request_for_test()
        .expect("fresh-generation Practice replay should emit one request");
    assert_eq!(request.game_type_id, game_type_id);
    assert_eq!(
        request.profile_revision,
        brawler::profiles::ProfileRevision::INITIAL,
        "the request names the profile revision of the lobby whose game it replays"
    );
    let selection = app.world().resource::<SelectedGameType>();
    assert_eq!(selection.game_type_id.as_ref(), Some(&request.game_type_id));
    assert!(app.world().resource::<FlowCommit>().error.is_none());
}

/// BRL-egn6: a tile picks its character with the remembered (or default) options, an option
/// picks it for the current character, and a pick that changes nothing sends nothing.
#[test]
fn brawler_picks_send_one_character_choice_with_remembered_options() {
    let mut membership = lobby_membership();
    let sumi = character_choice(7, 7, 7);
    membership.profile = membership.character_catalog.profile_from_stored(
        membership.profile.account_id,
        membership.profile.revision,
        None,
        [sumi],
    );
    let mut app = configured_customization_app(&membership);
    inject_action(&mut app, FlowUiAction::OpenBrawlers);
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::Brawlers
    );

    inject_action(
        &mut app,
        FlowUiAction::PickCharacter(brawler::characters::CharacterId(1)),
    );
    assert!(
        sent_picks(&app).is_empty(),
        "the selected character is a no-op"
    );
    inject_action(
        &mut app,
        FlowUiAction::PickCharacter(brawler::characters::CharacterId(7)),
    );
    assert_eq!(
        sent_picks(&app),
        [sumi],
        "Sumi keeps her remembered options"
    );

    let mut app = configured_customization_app(&membership);
    inject_action(&mut app, FlowUiAction::OpenBrawlers);
    inject_action(
        &mut app,
        FlowUiAction::PickPassive(brawler::builds::PassiveDefinitionId(5)),
    );
    assert_eq!(sent_picks(&app), [character_choice(1, 1, 5)]);

    let mut app = configured_customization_app(&membership);
    inject_action(&mut app, FlowUiAction::OpenBrawlers);
    inject_action(
        &mut app,
        FlowUiAction::PickUltimate(brawler::builds::UltimateDefinitionId(2)),
    );
    assert!(sent_picks(&app).is_empty(), "Riff does not offer Sentry");
}

#[test]
fn brawler_picks_preserve_queue_practice_and_profile_locks() {
    let membership = lobby_membership();
    let bumper = FlowUiAction::PickCharacter(brawler::characters::CharacterId(9));
    let open_and_pick = |app: &mut App| {
        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Brawlers;
        inject_action(app, bumper.clone());
    };

    let mut queue_locked = configured_customization_app(&membership);
    inject_action(&mut queue_locked, FlowUiAction::JoinQueue);
    open_and_pick(&mut queue_locked);
    assert!(sent_picks(&queue_locked).is_empty());

    let mut practice_locked = configured_customization_app(&membership);
    inject_action(&mut practice_locked, FlowUiAction::StartPractice);
    open_and_pick(&mut practice_locked);
    assert!(sent_picks(&practice_locked).is_empty());

    let mut profile_locked = configured_customization_app(&membership);
    open_and_pick(&mut profile_locked);
    assert_eq!(sent_picks(&profile_locked).len(), 1);
    inject_action(
        &mut profile_locked,
        FlowUiAction::PickCharacter(brawler::characters::CharacterId(2)),
    );
    assert_eq!(
        sent_picks(&profile_locked).len(),
        1,
        "a second pick waits for the first decision"
    );
}

#[test]
fn refused_pick_is_shown_on_the_brawlers_screen_and_cleared_by_the_next_pick() {
    let membership = lobby_membership();
    let mut app = configured_customization_app(&membership);
    inject_action(&mut app, FlowUiAction::OpenBrawlers);
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_decision_for_test(brawler::profiles::ProfileDecision::QueueLocked);
    app.update();
    assert!(
        app.world()
            .resource::<BrawlersNotice>()
            .0
            .as_deref()
            .is_some_and(|notice| notice.contains("Leave the queue"))
    );
    assert!(app.world().resource::<DashboardNotice>().0.is_none());
    inject_action(
        &mut app,
        FlowUiAction::PickCharacter(brawler::characters::CharacterId(4)),
    );
    assert!(app.world().resource::<BrawlersNotice>().0.is_none());
    inject_action(&mut app, FlowUiAction::CloseBrawlers);
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::None
    );
}

/// The rendered screen offers every roster character and the selected character's two
/// ultimates and two passives as focusable controls, plus Back.
#[test]
fn brawlers_screen_renders_the_roster_and_the_current_options() {
    let membership = lobby_membership();
    let mut app = flow_test_app();
    enter_dashboard(&mut app);
    app.world_mut().spawn((Client, membership.clone()));
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_snapshot_for_test(membership.profile.clone());
    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Brawlers;
    app.update();
    app.update();
    let mut roots = app.world_mut().query::<&BrawlersRoot>();
    assert_eq!(roots.iter(app.world()).count(), 1);
    let mut buttons = app.world_mut().query::<&FlowButton>();
    let actions: Vec<_> = buttons
        .iter(app.world())
        .map(|button| (button.index, button.action.clone()))
        .collect();
    let tiles = actions
        .iter()
        .filter(|(_, action)| matches!(action, FlowUiAction::PickCharacter(_)))
        .count();
    assert_eq!(tiles, membership.character_catalog.characters.len());
    for action in [
        FlowUiAction::PickUltimate(brawler::builds::UltimateDefinitionId(1)),
        FlowUiAction::PickUltimate(brawler::builds::UltimateDefinitionId(3)),
        FlowUiAction::PickPassive(brawler::builds::PassiveDefinitionId(3)),
        FlowUiAction::PickPassive(brawler::builds::PassiveDefinitionId(5)),
    ] {
        assert!(
            actions.iter().any(|(_, offered)| *offered == action),
            "{action:?}"
        );
    }
    assert!(
        actions
            .iter()
            .any(|(index, action)| *index == BRAWLERS_BACK_INDEX
                && *action == FlowUiAction::CloseBrawlers)
    );
    let text = visible_text(&mut app).join(" ");
    for name in [
        "RIFF",
        "BUMPER",
        "SUMI",
        "DASH",
        "SELF CLOAK",
        "ADRENAL RESPONSE",
    ] {
        assert!(text.contains(name), "{name} is on screen");
    }
}

/// Settles an in-flight character pick the way the server accepts it: the next profile revision
/// selects and remembers the pick.
#[cfg(feature = "automation")]
fn accept_pending_pick(app: &mut App, membership: &ClientLobbyMembership) {
    let profile = app.world().resource::<super::super::ClientProfileModel>();
    let (Some(choice), Some(current)) = (profile.pending_choice(), profile.snapshot()) else {
        return;
    };
    let accepted = membership.character_catalog.profile_from_stored(
        current.account_id,
        current.revision.next().unwrap(),
        Some(choice.character_id),
        current
            .choices
            .iter()
            .copied()
            .filter(|stored| stored.character_id != choice.character_id)
            .chain([choice]),
    );
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_snapshot_for_test(accepted);
    app.update();
}
