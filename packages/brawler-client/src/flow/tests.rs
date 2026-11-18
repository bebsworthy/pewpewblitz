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
    brawlers::{
        BrawlerCreationRoot, BrawlerDetailsRoot, BrawlerEditorRoot, BrawlerListRoot,
        DeleteBrawlerConfirmationRoot, WeaponEquipmentRoot, WeaponEquipmentScrollArea,
        ultimate_name,
    },
    connecting::ConnectingProgressSegment,
    dashboard::{
        DASHBOARD_BUILD_INDEX, DASHBOARD_GAME_INDEX, DASHBOARD_MENU_INDEX, DASHBOARD_PLAY_INDEX,
        DASHBOARD_PRACTICE_INDEX, DashboardButtonStyle, DashboardLayoutClass,
        DashboardNavigationDirection, dashboard_game_summary, dashboard_layout_class,
    },
    game_select::GameTypeSelectRoot,
    match_loading::match_loading_text,
    overlays::{FlowErrorRoot, RateLimitTryAgain},
    queue::{queue_cancel_presentation, queue_membership_text},
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
fn brawler_editor_uses_catalog_name_for_concealment_field() {
    let _app = flow_test_app();
    let catalog = brawler::profiles::AdvertisedBrawlerCatalog::from_content(
        &brawler::builds::BuildCatalog::embedded().unwrap(),
        &brawler::combat::WeaponCatalog::embedded().unwrap(),
    )
    .unwrap();

    assert_eq!(
        ultimate_name(&catalog, brawler::builds::UltimateDefinitionId(5)),
        "Concealment Field"
    );
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
            team_count: 2,
            players_per_team: 2,
            rules_summary: brawler::lobby::AdvertisedRulesSummary::Wipeout {
                target_score: 10,
                active_limit_ticks: 3_600,
            },
        }],
        brawler_catalog: brawler::profiles::AdvertisedBrawlerCatalog::from_content(
            &brawler::builds::BuildCatalog::embedded().unwrap(),
            &brawler::combat::WeaponCatalog::embedded().unwrap(),
        )
        .unwrap(),
        profile: brawler::profiles::ProfileSnapshot::empty(account_id),
    }
}

fn lobby_membership_with_brawler() -> ClientLobbyMembership {
    let mut membership = lobby_membership();
    let brawler_id = brawler::profiles::SavedBrawlerId::new(2).unwrap();
    membership
        .profile
        .brawlers
        .push(brawler::profiles::SavedBrawler {
            id: brawler_id,
            creation_ordinal: 1,
            name: "Test Brawler".into(),
            fighter_profile_id: brawler::profiles::FighterProfileId(1),
            weapon_base_id: brawler::profiles::WeaponBaseId(1),
            ultimate_id: brawler::builds::UltimateDefinitionId(1),
            passive_ids: [
                brawler::builds::PassiveDefinitionId(3),
                brawler::builds::PassiveDefinitionId(4),
            ],
            equipped_part_ids: [None; brawler::weapon_parts::WEAPON_PART_SLOT_COUNT],
            revision: brawler::profiles::ProfileRevision::INITIAL,
        });
    membership.profile.selected_brawler_id = Some(brawler_id);
    membership.profile.next_brawler_ordinal = 2;
    membership
}

fn lobby_membership_with_two_brawlers() -> ClientLobbyMembership {
    let mut membership = lobby_membership_with_brawler();
    let mut second = membership.profile.brawlers[0].clone();
    second.id = brawler::profiles::SavedBrawlerId::new(3).unwrap();
    second.creation_ordinal = 2;
    second.name = "Second Brawler".into();
    second.revision = brawler::profiles::ProfileRevision::INITIAL;
    membership.profile.brawlers.push(second);
    membership.profile.next_brawler_ordinal = 3;
    membership
}

#[test]
fn explicit_action_preempts_session_and_ordinary_while_profile_decision_still_applies() {
    let mut app = flow_action_test_app();
    let membership = lobby_membership_with_brawler();
    let brawler_id = membership.profile.selected_brawler_id.unwrap();
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
    app.world_mut().resource_mut::<PendingEditedBrawler>().0 = Some(brawler_id);
    app.world_mut().resource_mut::<InjectedFlowActions>().0 = Some(PendingFlowActions {
        session: Some(SessionObservation::ReservationStarted),
        explicit: Some(FlowUiAction::Cancel),
        ordinary: Some(FlowUiAction::OpenSettings),
    });

    app.update();

    assert!(app.world().get_entity(session).is_err());
    assert!(
        app.world()
            .resource::<BrawlerEditDraft>()
            .inline_error
            .as_deref()
            .is_some_and(|error| error.contains("not valid")),
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
    }
}

fn queue_membership(membership: &ClientLobbyMembership) -> brawler::lobby::QueueMembership {
    let brawler = &membership.profile.brawlers[0];
    let recipe = brawler::builds::BrawlerBuildRecipe {
        weapon: brawler::builds::WeaponChoice::Preset(brawler::combat::WeaponPresetId(1)),
        ultimate: brawler.ultimate_id,
        passives: brawler.passive_ids,
    };
    brawler::lobby::QueueMembership {
        ticket_id: brawler::lobby::QueueTicketId::new(1).unwrap(),
        catalog_revision: membership.catalog_revision,
        game_type_id: membership.game_types[0].id.clone(),
        game_type_configuration_revision: membership.game_types[0].configuration_revision,
        brawler_id: brawler.id,
        brawler_revision: brawler.revision,
        accepted_build: brawler::builds::AcceptedBuildSummary {
            canonical_recipe: recipe,
            identity: brawler::builds::SelectedBuild {
                recipe_fingerprint: brawler::builds::BuildRecipeFingerprint(1),
                revision: brawler::builds::BuildCatalog::embedded()
                    .unwrap()
                    .balance_revision,
            },
            total_points: 10,
        },
        admitted_at_pool_state_revision: 1,
    }
}

#[test]
fn match_transition_observations_preserve_loading_match_and_overlay_commits() {
    let mut app = configured_lobby_flow_app(lobby_membership_with_brawler());

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
    let membership = lobby_membership_with_brawler();
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
    let membership = lobby_membership_with_brawler();
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
    let mut stale = configured_lobby_flow_app(lobby_membership_with_brawler());
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

    let mut ordinary = configured_lobby_flow_app(lobby_membership_with_brawler());
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
    let membership = lobby_membership_with_brawler();
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
    let membership = lobby_membership_with_brawler();
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

#[test]
fn practice_queue_again_uses_fresh_generation_game_and_selected_brawler() {
    let membership = lobby_membership_with_brawler();
    let game_type_id = membership.game_types[0].id.clone();
    let selected_brawler = membership.profile.brawlers[0].clone();
    let mut app = configured_lobby_flow_app(membership);
    let mut stale_membership = lobby_membership_with_brawler();
    stale_membership.game_types[0].id = brawler::lobby::GameTypeId::new("stale-wipeout").unwrap();
    stale_membership.profile.brawlers[0].id = brawler::profiles::SavedBrawlerId::new(77).unwrap();
    stale_membership.profile.brawlers[0].revision =
        brawler::profiles::ProfileRevision::new(9).unwrap();
    stale_membership.profile.selected_brawler_id = Some(stale_membership.profile.brawlers[0].id);
    app.world_mut().spawn((
        Client,
        stale_membership,
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
    assert_eq!(request.brawler_id, selected_brawler.id);
    assert_eq!(request.brawler_revision, selected_brawler.revision);
    let selection = app.world().resource::<SelectedGameType>();
    assert_eq!(selection.game_type_id.as_ref(), Some(&request.game_type_id));
    assert!(selection.catalog_revision.is_some());
    assert!(selection.configuration_revision.is_some());
    assert!(app.world().resource::<FlowCommit>().error.is_none());
}

fn configured_customization_app(membership: &ClientLobbyMembership) -> App {
    let mut app = configured_lobby_flow_app(membership.clone());
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_snapshot_for_test(membership.profile.clone());
    app
}

#[test]
fn queue_and_practice_require_a_selected_brawler() {
    let empty_membership = lobby_membership();
    let mut missing = configured_lobby_flow_app(empty_membership);

    inject_action(&mut missing, FlowUiAction::JoinQueue);
    assert!(
        missing
            .world()
            .resource::<super::super::ClientQueueModel>()
            .pending()
            .is_none()
    );
    inject_action(&mut missing, FlowUiAction::StartPractice);
    assert!(
        !missing
            .world()
            .resource::<super::super::ClientPracticeModel>()
            .pending()
    );
}

#[test]
fn pending_practice_rejects_queue_join() {
    let mut practice_first = configured_lobby_flow_app(lobby_membership_with_brawler());

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
    let mut queue_first = configured_lobby_flow_app(lobby_membership_with_brawler());

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

fn assert_customization_entry_actions_are_blocked(
    app: &mut App,
    brawler_id: brawler::profiles::SavedBrawlerId,
    profile_pending: bool,
) {
    for action in [
        FlowUiAction::CreateBrawler,
        FlowUiAction::SelectBrawler(brawler_id),
        FlowUiAction::OpenBrawlerEditor(brawler_id),
        FlowUiAction::OpenWeaponEquipment(brawler_id),
        FlowUiAction::DeleteBrawler(brawler_id),
    ] {
        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Settings;
        inject_action(app, action);
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::Settings
        );
        assert_eq!(
            app.world()
                .resource::<super::super::ClientProfileModel>()
                .pending(),
            profile_pending
        );
    }
}

#[test]
fn customization_entry_actions_preserve_queue_practice_and_profile_locks() {
    let membership = lobby_membership_with_brawler();
    let brawler = membership.profile.brawlers[0].clone();

    let mut queue_locked = configured_customization_app(&membership);
    inject_action(&mut queue_locked, FlowUiAction::JoinQueue);
    assert!(
        queue_locked
            .world()
            .resource::<super::super::ClientQueueModel>()
            .pending()
            .is_some()
    );
    assert_customization_entry_actions_are_blocked(&mut queue_locked, brawler.id, false);

    let mut practice_locked = configured_customization_app(&membership);
    inject_action(&mut practice_locked, FlowUiAction::StartPractice);
    assert!(
        practice_locked
            .world()
            .resource::<super::super::ClientPracticeModel>()
            .pending()
    );
    assert_customization_entry_actions_are_blocked(&mut practice_locked, brawler.id, false);

    let mut profile_locked = configured_customization_app(&membership);
    assert!(
        profile_locked
            .world_mut()
            .resource_mut::<super::super::ClientProfileModel>()
            .edit(
                brawler.id,
                brawler::profiles::BrawlerEdit {
                    name: brawler.name,
                    ultimate_id: brawler.ultimate_id,
                    passive_ids: brawler.passive_ids,
                },
            )
    );
    assert_customization_entry_actions_are_blocked(&mut profile_locked, brawler.id, true);
}

#[test]
fn equipment_actions_preserve_slot_conflict_draft_and_overlay_contracts() {
    let mut membership = lobby_membership_with_two_brawlers();
    let target = membership.profile.brawlers[0].id;
    let occupied_part = brawler::weapon_parts::WeaponPartInstanceId::new(41).unwrap();
    membership.profile.brawlers[1].equipped_part_ids[2] = Some(occupied_part);
    let mut app = configured_customization_app(&membership);

    inject_action(&mut app, FlowUiAction::OpenWeaponEquipment(target));
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::WeaponEquipment
    );
    {
        let mut draft = app.world_mut().resource_mut::<WeaponEquipmentDraft>();
        draft.selected_slot = 1;
        draft.inline_error = Some("keep this error".into());
    }
    inject_action(
        &mut app,
        FlowUiAction::SelectEquipmentSlot(brawler::weapon_parts::WEAPON_PART_SLOT_COUNT),
    );
    let draft = app.world().resource::<WeaponEquipmentDraft>();
    assert_eq!(draft.selected_slot, 1);
    assert_eq!(draft.inline_error.as_deref(), Some("keep this error"));

    inject_action(&mut app, FlowUiAction::EquipWeaponPart(occupied_part));
    let draft = app.world().resource::<WeaponEquipmentDraft>();
    assert_eq!(
        draft.inline_error.as_deref(),
        Some("That physical part is equipped on another brawler.")
    );
    assert_eq!(draft.equipped_part_ids, [None; 4]);
    assert!(
        !app.world()
            .resource::<super::super::ClientProfileModel>()
            .pending()
    );

    let free_part = brawler::weapon_parts::WeaponPartInstanceId::new(42).unwrap();
    inject_action(&mut app, FlowUiAction::EquipWeaponPart(free_part));
    assert_eq!(
        app.world()
            .resource::<WeaponEquipmentDraft>()
            .equipped_part_ids[1],
        Some(free_part)
    );
    assert!(
        app.world()
            .resource::<WeaponEquipmentDraft>()
            .inline_error
            .is_none()
    );
    // A second choice and Back cannot outrun the in-flight server mutation.
    inject_action(&mut app, FlowUiAction::UnequipWeaponPart);
    inject_action(&mut app, FlowUiAction::CancelWeaponEquipment);
    assert_eq!(
        app.world()
            .resource::<WeaponEquipmentDraft>()
            .equipped_part_ids[1],
        Some(free_part)
    );
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::WeaponEquipment
    );
    assert!(app.world().resource::<WeaponEquipmentDraft>().awaiting_save);
    membership.profile.brawlers[0].equipped_part_ids[1] = Some(free_part);
    {
        let mut profile = app
            .world_mut()
            .resource_mut::<super::super::ClientProfileModel>();
        profile.set_snapshot_for_test(membership.profile.clone());
        profile.set_decision_for_test(brawler::profiles::ProfileDecision::Accepted);
    }
    app.update();
    assert!(!app.world().resource::<WeaponEquipmentDraft>().picker_open);
    assert!(!app.world().resource::<WeaponEquipmentDraft>().awaiting_save);
    assert!(app.world().resource::<DashboardNotice>().0.is_none());
    let burst = app.world().resource::<screens::shared::FlowBurst>();
    assert_eq!(burst.requested(), Some(1), "the accepted slot bursts");
    inject_action(&mut app, FlowUiAction::SelectEquipmentSlot(1));
    inject_action(&mut app, FlowUiAction::UnequipWeaponPart);
    assert!(app.world().resource::<WeaponEquipmentDraft>().awaiting_save);
    {
        let mut profile = app
            .world_mut()
            .resource_mut::<super::super::ClientProfileModel>();
        profile.set_snapshot_for_test(membership.profile.clone());
        profile.set_decision_for_test(brawler::profiles::ProfileDecision::StorageFault);
    }
    app.update();
    let draft = app.world().resource::<WeaponEquipmentDraft>();
    assert!(draft.picker_open);
    assert!(draft.inline_error.is_some());
    assert_eq!(draft.equipped_part_ids[1], Some(free_part));
    inject_action(&mut app, FlowUiAction::CancelWeaponEquipment);
    assert!(!app.world().resource::<WeaponEquipmentDraft>().picker_open);
    inject_action(&mut app, FlowUiAction::CancelWeaponEquipment);
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerDetails(target)
    );
}

#[test]
fn profile_decisions_preserve_exact_notices_overlays_focus_and_draft_errors() {
    let mut app = flow_action_test_app();
    let membership = lobby_membership_with_brawler();
    let created = membership.profile.brawlers[0].clone();
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_snapshot_for_test(membership.profile);
    app.world_mut().resource_mut::<PendingCreatedBrawler>().0 = Some(created.creation_ordinal);
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_decision_for_test(brawler::profiles::ProfileDecision::Accepted);

    app.update();

    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerDetails(created.id)
    );
    assert_eq!(
        app.world().resource::<DashboardNotice>().0.as_deref(),
        Some("Created Test Brawler.")
    );
    assert_eq!(app.world().resource::<FlowNavigation>().selected, 0);

    app.world_mut().resource_mut::<PendingEditedBrawler>().0 = Some(created.id);
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_decision_for_test(brawler::profiles::ProfileDecision::Accepted);
    app.update();

    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerDetails(created.id)
    );
    assert_eq!(
        app.world().resource::<DashboardNotice>().0.as_deref(),
        Some("Profile saved.")
    );
    assert_eq!(app.world().resource::<FlowNavigation>().selected, 1);

    app.world_mut().resource_mut::<PendingCreatedBrawler>().0 = Some(created.creation_ordinal);
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_decision_for_test(brawler::profiles::ProfileDecision::InvalidRequest);
    app.update();

    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerCreation
    );
    assert_eq!(
        app.world()
            .resource::<BrawlerCreationDraft>()
            .inline_error
            .as_deref(),
        Some("That brawler change is not valid.")
    );

    app.world_mut().resource_mut::<PendingEditedBrawler>().0 = Some(created.id);
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_decision_for_test(brawler::profiles::ProfileDecision::StaleRevision);
    app.update();

    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerEditor
    );
    assert!(
        app.world()
            .resource::<BrawlerEditDraft>()
            .inline_error
            .as_deref()
            .is_some_and(|error| error.contains("changed"))
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
fn empty_profile_creation_is_an_opaque_full_screen_destination() {
    let mut app = flow_test_app();
    let membership = lobby_membership();
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_snapshot_for_test(membership.profile.clone());
    app.world_mut().spawn((Client, membership));
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Dashboard);
    app.update();
    app.update();

    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerCreation
    );
    let world = app.world_mut();
    let mut roots = world.query_filtered::<(&Node, &BackgroundColor), With<BrawlerCreationRoot>>();
    let (node, background) = roots.single(world).unwrap();
    assert_eq!(
        (node.left, node.right, node.top, node.bottom),
        (px(0), px(0), px(0), px(0))
    );
    assert!((background.0.to_srgba().alpha - 1.0).abs() <= f32::EPSILON);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end UI regression follows the approved Dashboard-to-list-to-detail-to-customization path"
)]
fn selected_brawler_cards_open_list_details_and_reach_equipment() {
    let mut app = flow_test_app();
    let membership = lobby_membership_with_brawler();
    let brawler_id = membership.profile.selected_brawler_id.unwrap();
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_snapshot_for_test(membership.profile.clone());
    app.world_mut().spawn((Client, membership));
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Dashboard);
    app.update();
    let dashboard_copy = visible_text(&mut app);
    assert!(
        dashboard_copy
            .iter()
            .any(|text| text.contains("Pulse Sidearm · Dash"))
    );
    assert!(
        dashboard_copy
            .iter()
            .any(|text| text.contains("2 passives · 4 part slots"))
    );
    assert!(
        !dashboard_copy
            .iter()
            .any(|text| text.contains("Weapon base 1"))
    );
    assert!(dashboard_copy.iter().any(|text| text == "VIEW BRAWLERS"));
    assert!(
        !dashboard_copy
            .iter()
            .any(|text| text.contains("SELECTED FOR PLAY"))
    );

    let preview = {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &DashboardButtonStyle, &FlowButton)>();
        let mut selected = None;
        for (entity, style, button) in query.iter(world) {
            if matches!(
                style,
                DashboardButtonStyle::Preview | DashboardButtonStyle::Build
            ) {
                assert_eq!(button.action, FlowUiAction::OpenBrawlerList);
                if matches!(style, DashboardButtonStyle::Preview) {
                    selected = Some(entity);
                }
            }
        }
        selected.expect("Dashboard renders its selected-brawler preview")
    };
    app.world_mut()
        .entity_mut(preview)
        .insert(Interaction::Pressed);
    app.update();
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::None,
        "pressing only focuses; navigation waits for release"
    );
    app.world_mut()
        .entity_mut(preview)
        .insert(Interaction::Hovered);
    app.update();
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerList
    );
    app.update();
    {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<(&Node, &BackgroundColor), With<BrawlerListRoot>>();
        let (node, background) = roots.single(world).unwrap();
        assert_eq!(node.left, px(0));
        assert_eq!(node.right, px(0));
        assert_eq!(node.top, px(0));
        assert_eq!(node.bottom, px(0));
        assert!((background.0.to_srgba().alpha - 1.0).abs() <= f32::EPSILON);
    }
    assert!(
        !visible_text(&mut app)
            .iter()
            .any(|text| text.contains("SELECTED FOR PLAY"))
    );
    assert!(
        visible_text(&mut app)
            .iter()
            .any(|text| text.to_lowercase().contains("pulse sidearm"))
    );

    press_flow_button(&mut app, &FlowUiAction::OpenBrawlerDetails(brawler_id));
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerDetails(brawler_id)
    );
    app.update();
    {
        let world = app.world_mut();
        let mut roots =
            world.query_filtered::<(&Node, &BackgroundColor), With<BrawlerDetailsRoot>>();
        let (node, background) = roots.single(world).unwrap();
        assert_eq!(node.left, px(0));
        assert_eq!(node.right, px(0));
        assert_eq!(node.top, px(0));
        assert_eq!(node.bottom, px(0));
        assert!((background.0.to_srgba().alpha - 1.0).abs() <= f32::EPSILON);
        let mut previews = world.query_filtered::<Entity, With<BrawlerCardPreview>>();
        assert_eq!(previews.iter(world).count(), 1);
    }
    assert!(
        visible_text(&mut app)
            .iter()
            .any(|text| text.to_lowercase().contains("pulse sidearm"))
    );

    let select_disabled = {
        let world = app.world_mut();
        let mut buttons = world.query::<(&FlowButton, Has<InteractionDisabled>)>();
        buttons
            .iter(world)
            .find(|(button, _)| button.action == FlowUiAction::SelectBrawler(brawler_id))
            .map(|(_, disabled)| disabled)
            .expect("selected brawler retains its primary action")
    };
    assert!(!select_disabled);
    press_flow_button(&mut app, &FlowUiAction::SelectBrawler(brawler_id));
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::None
    );
    assert!(
        !app.world()
            .resource::<super::super::ClientProfileModel>()
            .pending(),
        "returning with the selected brawler must not send a profile mutation"
    );
    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::BrawlerDetails(brawler_id);
    app.update();

    press_flow_button(&mut app, &FlowUiAction::DeleteBrawler(brawler_id));
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::DeleteBrawlerConfirmation(brawler_id)
    );
    app.update();
    {
        let world = app.world_mut();
        let mut details = world.query::<&BrawlerDetailsRoot>();
        assert!(details.single(world).unwrap().contextual_confirmation);
        let mut confirmations =
            world.query_filtered::<Entity, With<DeleteBrawlerConfirmationRoot>>();
        assert_eq!(confirmations.iter(world).count(), 1);
        let mut background_actions = world.query::<(&FlowButton, Has<InteractionDisabled>)>();
        assert!(
            background_actions
                .iter(world)
                .filter(|(button, _)| {
                    matches!(
                        button.action,
                        FlowUiAction::SelectBrawler(_)
                            | FlowUiAction::OpenBrawlerEditor(_)
                            | FlowUiAction::OpenWeaponEquipment(_)
                            | FlowUiAction::DeleteBrawler(_)
                    )
                })
                .all(|(_, disabled)| disabled)
        );
    }
    press_flow_button(&mut app, &FlowUiAction::CancelDeleteBrawler);
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerDetails(brawler_id)
    );
    app.update();

    press_flow_button(&mut app, &FlowUiAction::OpenBrawlerEditor(brawler_id));
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerEditor
    );
    app.update();
    {
        let world = app.world_mut();
        let mut roots =
            world.query_filtered::<(&Node, &BackgroundColor), With<BrawlerEditorRoot>>();
        let (node, background) = roots.single(world).unwrap();
        assert_eq!(
            (node.left, node.right, node.top, node.bottom),
            (px(0), px(0), px(0), px(0))
        );
        assert!((background.0.to_srgba().alpha - 1.0).abs() <= f32::EPSILON);
    }
    press_flow_button(&mut app, &FlowUiAction::CancelBrawlerEdit);
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerDetails(brawler_id)
    );
    app.update();
    press_flow_button(&mut app, &FlowUiAction::OpenWeaponEquipment(brawler_id));
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::WeaponEquipment
    );
    app.update();
    {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&Node, With<WeaponEquipmentRoot>>();
        let node = roots.single(world).unwrap();
        assert_eq!(
            (node.left, node.right, node.top, node.bottom),
            (px(0), px(0), px(0), px(0))
        );
    }
    press_flow_button(&mut app, &FlowUiAction::SelectEquipmentSlot(0));
    app.update();
    {
        let world = app.world_mut();
        let mut areas = world.query_filtered::<&Node, With<WeaponEquipmentScrollArea>>();
        let node = areas.single(world).unwrap();
        assert_eq!(node.overflow, Overflow::scroll_y());
        let mut buttons = world.query::<&FlowButton>();
        assert!(
            buttons
                .iter(world)
                .any(|b| b.index == 0 && b.action == FlowUiAction::UnequipWeaponPart)
        );
    }
    press_flow_button(&mut app, &FlowUiAction::UnequipWeaponPart);
    assert!(!app.world().resource::<WeaponEquipmentDraft>().picker_open);
    assert!(
        !app.world()
            .resource::<super::super::ClientProfileModel>()
            .pending()
    );
}

#[test]
fn brawler_details_refreshes_when_selection_request_finishes() {
    let mut app = flow_test_app();
    let mut membership = lobby_membership_with_brawler();
    let selected_id = membership.profile.selected_brawler_id.unwrap();
    let mut candidate = membership.profile.brawlers[0].clone();
    candidate.id = brawler::profiles::SavedBrawlerId::new(3).unwrap();
    candidate.creation_ordinal = 2;
    candidate.name = "Candidate Brawler".into();
    membership.profile.brawlers.push(candidate.clone());
    membership.profile.next_brawler_ordinal = 3;
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_snapshot_for_test(membership.profile.clone());
    app.world_mut().spawn((Client, membership.clone()));
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Dashboard);
    app.update();

    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::BrawlerDetails(candidate.id);
    app.update();
    assert!(
        app.world_mut()
            .resource_mut::<super::super::ClientProfileModel>()
            .select(candidate.id)
    );
    app.update();
    assert!(
        visible_text(&mut app)
            .iter()
            .any(|text| text == "SELECTING...")
    );

    membership.profile.selected_brawler_id = Some(selected_id);
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_snapshot_for_test(membership.profile);
    app.update();

    let copy = visible_text(&mut app);
    assert!(copy.iter().any(|text| text == "SELECT FOR PLAY"));
    assert!(!copy.iter().any(|text| text == "SELECTING..."));
    let world = app.world_mut();
    let mut buttons = world.query::<(&FlowButton, Has<InteractionDisabled>)>();
    let (_, disabled) = buttons
        .iter(world)
        .find(|(button, _)| button.action == FlowUiAction::SelectBrawler(candidate.id))
        .expect("selection button remains rendered after the outcome");
    assert!(!disabled);
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
        matches!(style, DashboardButtonStyle::Preview) && label == "Create your first brawler"
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
    let mut lobby = lobby_membership();
    let brawler_id = brawler::profiles::SavedBrawlerId::new(2).unwrap();
    lobby
        .profile
        .brawlers
        .push(brawler::profiles::SavedBrawler {
            id: brawler_id,
            creation_ordinal: 1,
            name: "Replay Brawler".into(),
            fighter_profile_id: brawler::profiles::FighterProfileId(1),
            weapon_base_id: brawler::profiles::WeaponBaseId(1),
            ultimate_id: brawler::builds::UltimateDefinitionId(1),
            passive_ids: [
                brawler::builds::PassiveDefinitionId(3),
                brawler::builds::PassiveDefinitionId(4),
            ],
            equipped_part_ids: [None; brawler::weapon_parts::WEAPON_PART_SLOT_COUNT],
            revision: brawler::profiles::ProfileRevision::INITIAL,
        });
    lobby.profile.selected_brawler_id = Some(brawler_id);
    lobby.profile.next_brawler_ordinal = 2;
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

    assert_eq!(app.world().resource::<FlowNavigation>().selected, 1);
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
fn queue_copy_uses_advertised_game_and_saved_brawler_recipe() {
    let builds = brawler::builds::BuildCatalog::embedded().unwrap();
    let recipe = brawler::builds::BrawlerBuildRecipe {
        weapon: brawler::builds::WeaponChoice::Preset(brawler::combat::WeaponPresetId(1)),
        ultimate: brawler::builds::UltimateDefinitionId(1),
        passives: [
            brawler::builds::PassiveDefinitionId(3),
            brawler::builds::PassiveDefinitionId(4),
        ],
    };
    let membership = brawler::lobby::QueueMembership {
        ticket_id: brawler::lobby::QueueTicketId::new(1).unwrap(),
        catalog_revision: brawler::lobby::CatalogRevision([1; 32]),
        game_type_id: brawler::lobby::GameTypeId::new("wipeout-2v2").unwrap(),
        game_type_configuration_revision: 1,
        brawler_id: brawler::profiles::SavedBrawlerId::new(1).unwrap(),
        brawler_revision: brawler::profiles::ProfileRevision::INITIAL,
        accepted_build: brawler::builds::AcceptedBuildSummary {
            canonical_recipe: recipe,
            identity: brawler::builds::SelectedBuild {
                recipe_fingerprint: brawler::builds::BuildRecipeFingerprint(1),
                revision: builds.balance_revision,
            },
            total_points: 10,
        },
        admitted_at_pool_state_revision: 2,
    };
    let copy = queue_membership_text(
        &super::super::ClientQueueModel::default(),
        &membership,
        Some(&lobby_membership()),
        &builds,
    );
    assert!(copy.contains("Wipeout 2v2"));
    assert!(copy.contains("Saved brawler"));
    assert!(copy.contains("Updating queue"));
    assert!(!copy.contains("points"));

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
        ("CANCELLING…", true)
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

    app.world_mut().resource_mut::<FlowNavigation>().selected = 7;
    app.world_mut()
        .resource_mut::<NextState<ClientFlow>>()
        .set(ClientFlow::Connecting);
    app.update();
    assert_eq!(count_flow_roots(&mut app), 1);
    assert_eq!(app.world().resource::<FlowNavigation>().selected, 0);

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
    assert_eq!(app.world().resource::<FlowNavigation>().selected, 1_000);

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
    assert_eq!(app.world().resource::<FlowNavigation>().selected, 2);

    let mut gamepad = Gamepad::default();
    gamepad.digital_mut().press(GamepadButton::DPadUp);
    let gamepad_entity = app.world_mut().spawn(gamepad).id();
    app.update();
    assert_eq!(app.world().resource::<FlowNavigation>().selected, 1);

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
        app.world().resource::<FlowNavigation>().selected,
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
        app.world_mut().resource_mut::<FlowNavigation>().selected = selected;
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
    let mut membership = lobby_membership_with_brawler();
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
    let mut app = configured_lobby_flow_app(lobby_membership_with_brawler());
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
    let mut membership = lobby_membership_with_brawler();
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
    let mut app = configured_lobby_flow_app(lobby_membership_with_brawler());
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
    let mut app = configured_lobby_flow_app(lobby_membership_with_brawler());
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
fn inline_rename_preserves_rejected_text_and_returns_to_details_on_acceptance() {
    let mut app = flow_action_test_app();
    let snapshot = lobby_membership_with_brawler().profile;
    let id = snapshot.brawlers[0].id;
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_snapshot_for_test(snapshot.clone());
    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::BrawlerDetails(id);
    inject_action(&mut app, FlowUiAction::BeginBrawlerNameEdit);
    {
        let mut draft = app.world_mut().resource_mut::<BrawlerEditDraft>();
        assert!(draft.editing_name && draft.inline_rename);
        draft.name = "Renamed build".into();
    }
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerDetails(id)
    );
    inject_action(&mut app, FlowUiAction::ConfirmBrawlerEdit);
    assert!(
        app.world()
            .resource::<super::super::ClientProfileModel>()
            .pending()
    );
    {
        let mut profile = app
            .world_mut()
            .resource_mut::<super::super::ClientProfileModel>();
        profile.set_snapshot_for_test(snapshot.clone());
        profile.set_decision_for_test(brawler::profiles::ProfileDecision::StorageFault);
    }
    app.update();
    let draft = app.world().resource::<BrawlerEditDraft>();
    assert_eq!(draft.name, "Renamed build");
    assert!(draft.editing_name && draft.inline_error.is_some());
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerDetails(id)
    );
    inject_action(&mut app, FlowUiAction::ConfirmBrawlerEdit);
    let mut accepted = snapshot;
    accepted.brawlers[0].name = "Renamed build".into();
    {
        let mut profile = app
            .world_mut()
            .resource_mut::<super::super::ClientProfileModel>();
        profile.set_snapshot_for_test(accepted);
        profile.set_decision_for_test(brawler::profiles::ProfileDecision::Accepted);
    }
    app.update();
    assert!(!app.world().resource::<BrawlerEditDraft>().inline_rename);
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerDetails(id)
    );
}

#[test]
fn shell_back_actions_share_the_same_role_and_preview_has_no_chrome() {
    use crate::ui::surface::Surface;
    for action in [
        FlowUiAction::Back,
        FlowUiAction::CloseBrawlerList,
        FlowUiAction::BackToBrawlerList,
        FlowUiAction::CancelWeaponEquipment,
        FlowUiAction::CancelBrawlerEdit,
        FlowUiAction::CancelCreateBrawler,
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
                action: FlowUiAction::OpenBrawlerList,
                error_action: false,
            },
            DashboardButtonStyle::Preview,
        ))
        .id();
    app.update();
    assert!(app.world().get::<Surface>(preview).is_none());
}

#[test]
fn creation_pickers_preserve_named_draft_and_only_submit_from_overview() {
    use super::screens::brawlers::CreationPicker;
    let membership = lobby_membership_with_brawler();
    let mut app = configured_customization_app(&membership);
    inject_action(&mut app, FlowUiAction::CreateBrawler);
    app.world_mut().resource_mut::<BrawlerCreationDraft>().name = "New Spark".into();
    inject_action(&mut app, FlowUiAction::OpenCreationUltimate);
    assert_eq!(
        app.world().resource::<BrawlerCreationDraft>().picker,
        Some(CreationPicker::Ultimate)
    );
    inject_action(&mut app, FlowUiAction::CancelCreateBrawler);
    assert_eq!(
        app.world().resource::<BrawlerCreationDraft>().name,
        "New Spark"
    );
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerCreation
    );
    inject_action(&mut app, FlowUiAction::OpenCreationWeapon);
    let id = app
        .world()
        .resource::<super::super::ClientProfileModel>()
        .catalog()
        .unwrap()
        .weapon_bases
        .last()
        .unwrap()
        .id;
    inject_action(&mut app, FlowUiAction::ChooseCreationWeapon(id));
    let draft = app.world().resource::<BrawlerCreationDraft>();
    assert_eq!(draft.weapon_base_id, id);
    assert!(draft.picker.is_none());
    assert!(
        !app.world()
            .resource::<super::super::ClientProfileModel>()
            .pending()
    );
    app.world_mut().resource_mut::<BrawlerCreationDraft>().name = String::new();
    inject_action(&mut app, FlowUiAction::ConfirmCreateBrawler);
    assert!(
        app.world()
            .resource::<BrawlerCreationDraft>()
            .inline_error
            .is_some()
    );
    assert!(
        !app.world()
            .resource::<super::super::ClientProfileModel>()
            .pending()
    );
    app.world_mut().resource_mut::<BrawlerCreationDraft>().name = "New Spark".into();
    inject_action(&mut app, FlowUiAction::ConfirmCreateBrawler);
    assert!(
        app.world()
            .resource::<super::super::ClientProfileModel>()
            .pending()
    );
    let commands = app
        .world()
        .resource::<super::super::ClientProfileModel>()
        .queued_commands_for_test();
    assert_eq!(commands.len(), 1);
    let brawler::profiles::ProfileCommand::CreateBrawler { draft, .. } = commands.front().unwrap()
    else {
        panic!("expected one atomic creation");
    };
    assert_eq!(draft.name, "New Spark");
    assert_eq!(draft.weapon_base_id, id);
    inject_action(&mut app, FlowUiAction::OpenCreationFighter);
    assert!(
        app.world()
            .resource::<BrawlerCreationDraft>()
            .picker
            .is_none()
    );
}

#[test]
fn ability_picker_waits_for_acceptance_and_keeps_loadout_on_rejection() {
    use super::screens::brawlers::AbilityPicker;
    let mut membership = lobby_membership_with_brawler();
    let saved = membership.profile.brawlers[0].clone();
    let mut app = configured_customization_app(&membership);
    inject_action(&mut app, FlowUiAction::OpenBrawlerEditor(saved.id));
    inject_action(&mut app, FlowUiAction::OpenBrawlerUltimate);
    let replacement = app
        .world()
        .resource::<super::super::ClientProfileModel>()
        .catalog()
        .unwrap()
        .ultimates
        .iter()
        .find(|u| u.id != saved.ultimate_id)
        .unwrap()
        .id;
    inject_action(&mut app, FlowUiAction::ChooseBrawlerUltimate(replacement));
    assert_eq!(
        app.world().resource::<BrawlerEditDraft>().ultimate_id,
        saved.ultimate_id
    );
    let profile = app.world().resource::<super::super::ClientProfileModel>();
    assert!(profile.pending());
    let brawler::profiles::ProfileCommand::EditBrawler { edit, .. } =
        profile.queued_commands_for_test().front().unwrap()
    else {
        panic!("expected edit")
    };
    assert_eq!(edit.name, saved.name);
    assert_eq!(edit.passive_ids, saved.passive_ids);
    assert_eq!(edit.ultimate_id, replacement);
    inject_action(&mut app, FlowUiAction::CancelBrawlerEdit);
    inject_action(
        &mut app,
        FlowUiAction::ChooseBrawlerUltimate(saved.ultimate_id),
    );
    assert_eq!(
        app.world().resource::<BrawlerEditDraft>().picker,
        Some(AbilityPicker::Ultimate)
    );
    assert_eq!(
        app.world()
            .resource::<super::super::ClientProfileModel>()
            .queued_commands_for_test()
            .len(),
        1
    );
    membership.profile.brawlers[0].ultimate_id = replacement;
    {
        let mut p = app
            .world_mut()
            .resource_mut::<super::super::ClientProfileModel>();
        p.set_snapshot_for_test(membership.profile.clone());
        p.set_decision_for_test(brawler::profiles::ProfileDecision::Accepted);
    }
    app.update();
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerEditor
    );
    assert!(app.world().resource::<BrawlerEditDraft>().picker.is_none());
    assert_eq!(
        app.world().resource::<BrawlerEditDraft>().ultimate_id,
        replacement
    );
    inject_action(&mut app, FlowUiAction::OpenBrawlerUltimate);
    inject_action(
        &mut app,
        FlowUiAction::ChooseBrawlerUltimate(saved.ultimate_id),
    );
    {
        let mut p = app
            .world_mut()
            .resource_mut::<super::super::ClientProfileModel>();
        p.set_snapshot_for_test(membership.profile.clone());
        p.set_decision_for_test(brawler::profiles::ProfileDecision::StorageFault);
    }
    app.update();
    let draft = app.world().resource::<BrawlerEditDraft>();
    assert_eq!(draft.ultimate_id, replacement);
    assert_eq!(draft.picker, Some(AbilityPicker::Ultimate));
    assert!(draft.inline_error.is_some());
}

#[test]
fn passive_picker_rejects_duplicates_and_fixed_frames_without_changing_other_slot() {
    let membership = lobby_membership_with_brawler();
    let saved = membership.profile.brawlers[0].clone();
    let mut app = configured_customization_app(&membership);
    inject_action(&mut app, FlowUiAction::OpenBrawlerEditor(saved.id));
    inject_action(&mut app, FlowUiAction::OpenBrawlerPassive(0));
    let catalog = app
        .world()
        .resource::<super::super::ClientProfileModel>()
        .catalog()
        .unwrap();
    let fixed = catalog
        .passives
        .iter()
        .find(|p| !p.saved_brawler_selectable)
        .unwrap()
        .id;
    let replacement = catalog
        .selectable_passives()
        .find(|p| !saved.passive_ids.contains(&p.id))
        .unwrap()
        .id;
    for invalid in [fixed, saved.passive_ids[1]] {
        inject_action(&mut app, FlowUiAction::ChooseBrawlerPassive(invalid));
        assert!(
            !app.world()
                .resource::<super::super::ClientProfileModel>()
                .pending()
        );
        assert!(
            app.world()
                .resource::<BrawlerEditDraft>()
                .inline_error
                .is_some()
        );
        assert_eq!(
            app.world().resource::<BrawlerEditDraft>().passive_ids,
            saved.passive_ids
        );
    }
    inject_action(&mut app, FlowUiAction::ChooseBrawlerPassive(replacement));
    let profile = app.world().resource::<super::super::ClientProfileModel>();
    let brawler::profiles::ProfileCommand::EditBrawler { edit, .. } =
        profile.queued_commands_for_test().front().unwrap()
    else {
        panic!("expected edit")
    };
    assert_eq!(edit.passive_ids, [replacement, saved.passive_ids[1]]);
    assert_eq!(edit.ultimate_id, saved.ultimate_id);
    assert_eq!(edit.name, saved.name);
}

#[test]
fn delete_confirmation_waits_for_authority_and_retains_rejected_brawler() {
    let mut membership = lobby_membership_with_brawler();
    let id = membership.profile.brawlers[0].id;
    let mut app = configured_customization_app(&membership);
    inject_action(&mut app, FlowUiAction::DeleteBrawler(id));
    assert_eq!(
        app.world()
            .resource::<screens::shared::FlowBurst>()
            .requested(),
        None
    );
    inject_action(&mut app, FlowUiAction::ConfirmDeleteBrawler);
    assert_eq!(
        app.world()
            .resource::<screens::shared::FlowBurst>()
            .requested(),
        Some(1),
        "the confirmed Delete bursts while authority decides"
    );
    inject_action(&mut app, FlowUiAction::ConfirmDeleteBrawler);
    inject_action(&mut app, FlowUiAction::CancelDeleteBrawler);
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::DeleteBrawlerConfirmation(id)
    );
    assert_eq!(
        app.world()
            .resource::<super::super::ClientProfileModel>()
            .queued_commands_for_test()
            .len(),
        1
    );
    {
        let mut p = app
            .world_mut()
            .resource_mut::<super::super::ClientProfileModel>();
        p.set_snapshot_for_test(membership.profile.clone());
        p.set_decision_for_test(brawler::profiles::ProfileDecision::StorageFault);
    }
    app.update();
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::DeleteBrawlerConfirmation(id)
    );
    assert!(
        app.world()
            .resource::<PendingDeletedBrawler>()
            .error
            .is_some()
    );
    assert_eq!(
        app.world()
            .resource::<super::super::ClientProfileModel>()
            .snapshot()
            .unwrap()
            .brawlers[0]
            .id,
        id
    );
    inject_action(&mut app, FlowUiAction::ConfirmDeleteBrawler);
    membership.profile.brawlers.clear();
    membership.profile.selected_brawler_id = None;
    {
        let mut p = app
            .world_mut()
            .resource_mut::<super::super::ClientProfileModel>();
        p.set_snapshot_for_test(membership.profile);
        p.set_decision_for_test(brawler::profiles::ProfileDecision::Accepted);
    }
    app.update();
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerList
    );
    assert!(
        app.world()
            .resource::<PendingDeletedBrawler>()
            .error
            .is_none()
    );
}

#[test]
fn confirmation_navigation_starts_safe_and_supports_both_axes() {
    let membership = lobby_membership_with_brawler();
    let id = membership.profile.brawlers[0].id;
    let mut app = configured_customization_app(&membership);
    inject_action(&mut app, FlowUiAction::DeleteBrawler(id));
    app.update();
    assert_eq!(app.world().resource::<FlowNavigation>().selected, 0);
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
        assert_eq!(app.world().resource::<FlowNavigation>().selected, expected);
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
        ClientOverlay::BrawlerDetails(id)
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
    let membership = lobby_membership_with_brawler();
    let brawler = membership.profile.brawlers[0].id;
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

    inject_action(&mut app, FlowUiAction::OpenBrawlerList);
    assert_phone_reachable(&mut app, "Brawlers");
    inject_action(&mut app, FlowUiAction::OpenBrawlerDetails(brawler));
    assert_phone_reachable(&mut app, "Brawler details");
    inject_action(&mut app, FlowUiAction::BackToBrawlerList);
    inject_action(&mut app, FlowUiAction::CreateBrawler);
    assert_phone_reachable(&mut app, "Create Brawler");
    inject_action(&mut app, FlowUiAction::CancelCreateBrawler);
    inject_action(&mut app, FlowUiAction::CloseBrawlerList);
    assert_phone_reachable(&mut app, "Dashboard after Brawlers");

    inject_action(&mut app, FlowUiAction::OpenGameTypeSelect);
    assert_phone_reachable(&mut app, "Choose Game");
    inject_action(&mut app, FlowUiAction::CancelGameType);
    inject_action(&mut app, FlowUiAction::OpenPracticeMaps);
    assert_phone_reachable(&mut app, "Practice maps");
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
                .insert_resource(super::super::ClientInputContext::Shell);
        });
        crate::ui::layout_harness::apply_viewport_fit(&mut app);
        app.world_mut().resource_mut::<UiScale>().0 *= ui_scale;
        app.world_mut()
            .resource_mut::<super::super::ClientProfileModel>()
            .set_snapshot_for_test(membership.profile.clone());
        app
    };
    for ui_scale in [1.0, MIN_UI_SCALE] {
        let membership = lobby_membership_with_brawler();
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
        let brawler = membership.profile.brawlers[0].id;
        inject_action(&mut app, FlowUiAction::OpenBrawlerList);
        assert_phone_touch_targets(&mut app, &at("Brawlers"));
        inject_action(&mut app, FlowUiAction::OpenBrawlerDetails(brawler));
        assert_phone_touch_targets(&mut app, &at("Brawler details"));
        inject_action(&mut app, FlowUiAction::BackToBrawlerList);
        inject_action(&mut app, FlowUiAction::CreateBrawler);
        assert_phone_touch_targets(&mut app, &at("Create Brawler"));
        inject_action(&mut app, FlowUiAction::CancelCreateBrawler);
        inject_action(&mut app, FlowUiAction::CloseBrawlerList);

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

/// BRL-0198: one shell screen reached from a fresh connected Dashboard.
struct BackCase {
    screen: &'static str,
    open: Vec<FlowUiAction>,
}

fn shell_back_cases(brawler: brawler::profiles::SavedBrawlerId) -> Vec<BackCase> {
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
        case("Brawlers", vec![FlowUiAction::OpenBrawlerList]),
        case(
            "Brawler details",
            vec![
                FlowUiAction::OpenBrawlerList,
                FlowUiAction::OpenBrawlerDetails(brawler),
            ],
        ),
        case(
            "Create Brawler",
            vec![FlowUiAction::OpenBrawlerList, FlowUiAction::CreateBrawler],
        ),
        case(
            "Customize Weapon",
            vec![
                FlowUiAction::OpenBrawlerList,
                FlowUiAction::OpenBrawlerDetails(brawler),
                FlowUiAction::OpenWeaponEquipment(brawler),
            ],
        ),
        case(
            "Delete Brawler confirmation",
            vec![
                FlowUiAction::OpenBrawlerList,
                FlowUiAction::OpenBrawlerDetails(brawler),
                FlowUiAction::DeleteBrawler(brawler),
            ],
        ),
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
    let membership = lobby_membership_with_brawler();
    let brawler = membership.profile.brawlers[0].id;
    for case in shell_back_cases(brawler) {
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
    let mut app = configured_customization_app(&lobby_membership_with_brawler());
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
            let membership = lobby_membership_with_brawler();
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
        .insert_resource(ClientOverlay::WeaponEquipment)
        .init_resource::<screens::shared::FlowBurst>()
        .add_systems(Update, screens::shared::apply_flow_burst);
    let button = |index, error_action| {
        (
            Surface::Card,
            FlowButton {
                index,
                action: FlowUiAction::SelectEquipmentSlot(index),
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
    let membership = lobby_membership_with_brawler();
    let brawler = membership.profile.brawlers[0].id;
    for case in shell_back_cases(brawler) {
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
    let mut app = configured_customization_app(&lobby_membership_with_brawler());
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
    app.world_mut().resource_mut::<FlowNavigation>().selected = DASHBOARD_PLAY_INDEX;

    press_key(&mut app, KeyCode::ArrowDown);
    assert_eq!(
        app.world().resource::<FlowNavigation>().selected,
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

#[test]
fn brawler_name_editor_commit_and_cancel_publish_feedback_and_a_rejected_name_is_silent() {
    use crate::ui::{feedback::UiFeedback, surface::Surface};
    let mut app = flow_action_test_app();
    record_feedback_in(&mut app);
    let snapshot = lobby_membership_with_brawler().profile;
    let id = snapshot.brawlers[0].id;
    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_snapshot_for_test(snapshot);
    *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::BrawlerDetails(id);
    let begin = |app: &mut App, name: &str| {
        inject_action(app, FlowUiAction::BeginBrawlerNameEdit);
        let mut draft = app.world_mut().resource_mut::<BrawlerEditDraft>();
        assert!(draft.editing_name);
        draft.name = name.into();
        draft.name_caret = draft.name.len();
    };

    begin(&mut app, "Ab\u{E000}cd");
    take_ui_feedback(&mut app);
    press_key(&mut app, KeyCode::Enter);
    assert!(app.world().resource::<BrawlerEditDraft>().editing_name);
    assert_eq!(
        take_ui_feedback(&mut app),
        Vec::new(),
        "a rejected name plays nothing"
    );

    press_escape(&mut app);
    assert!(!app.world().resource::<BrawlerEditDraft>().editing_name);
    assert_eq!(take_ui_feedback(&mut app), vec![UiFeedback::Back]);

    begin(&mut app, "Renamed build");
    take_ui_feedback(&mut app);
    press_key(&mut app, KeyCode::Enter);
    assert!(!app.world().resource::<BrawlerEditDraft>().editing_name);
    assert_eq!(
        take_ui_feedback(&mut app),
        vec![UiFeedback::Activated(Some(Surface::Primary))]
    );
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
    let membership = lobby_membership_with_brawler();
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
    let membership = lobby_membership_with_brawler();
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
    let mut app = configured_lobby_flow_app(lobby_membership_with_brawler());
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
    assert_eq!(app.world().resource::<FlowNavigation>().selected, 1_000);

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
        let membership = lobby_membership_with_brawler();
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
/// its action, from an idle connected Dashboard with one saved brawler.
#[cfg(feature = "automation")]
#[test]
fn shell_capture_route_steps_match_the_reducer() {
    let membership = lobby_membership_with_brawler();
    let brawler = membership.profile.selected_brawler_id;
    for route in ShellCaptureRoute::ALL {
        // The Map Maker editor loads palette images, so this harness carries an asset server.
        let mut app = configured_lobby_flow_app_with(membership.clone(), |app| {
            app.add_plugins(AssetPlugin::default())
                .init_asset::<Image>()
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
                .action(brawler)
                .unwrap_or_else(|| panic!("{} step {index}: no action", route.name()));
            inject_action(&mut app, action);
            app.update();
            if step
                .expected(brawler)
                .is_some_and(|(flow, _)| flow == ClientFlow::Queue)
            {
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
            assert_eq!(
                Some(shell_position(&app)),
                step.expected(brawler),
                "{} step {index}",
                route.name()
            );
        }
    }
}
