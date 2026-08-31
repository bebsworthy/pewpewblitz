use super::*;
use crate::client::{
    ClientJoinPhase, ClientJoinStatus, ClientLobbyFailure, ClientLobbyMembership,
    ClientNetworkConfig, RoutedClientPhase, RoutedClientSession, RuntimeLobbyTarget,
    connection_persistence::ConnectionsFileV1, server_select::MAX_RESOLVED_CANDIDATES,
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
    dashboard::{
        DASHBOARD_BUILD_INDEX, DASHBOARD_GAME_INDEX, DASHBOARD_MENU_INDEX, DASHBOARD_PLAY_INDEX,
        DASHBOARD_PRACTICE_INDEX, DASHBOARD_SETTINGS_INDEX, DashboardButtonStyle,
        DashboardLayoutClass, DashboardNavigationDirection, dashboard_game_summary,
        dashboard_layout_class,
    },
    game_select::GameTypeSelectRoot,
    match_loading::match_loading_text,
    overlays::{FlowErrorRoot, RateLimitTryAgain},
    queue::{queue_cancel_presentation, queue_membership_text},
    results::MatchCompletionRoot,
    server_select::{EditingField, ServerSelectModel},
    shared::{FlowButton, FlowRoot, flow_button_background, flow_button_border},
};
use std::time::Duration;

fn flow_test_app() -> App {
    let mut app = App::new();
    let mut config = ClientNetworkConfig::new(0x1234);
    config.transport = crate::config::NetworkTransport::RoutedUdp;
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
            crate::modes::ModeRegistryPlugin,
            crate::modes::BuiltInModeRegistrationsPlugin,
        ))
        .add_plugins(ClientFlowPlugin);
    crate::test_app::finalize(&mut app);
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
    let mut app = flow_test_app();
    app.init_resource::<InjectedFlowActions>().add_systems(
        Update,
        inject_flow_actions
            .before(resolve_flow_action)
            .in_set(ClientFlowSet::ResolveFlowAction),
    );
    app
}

#[test]
fn client_flow_update_schedule_has_no_owned_ambiguities() {
    let mut app = flow_test_app();
    crate::test_app::reject_owned_schedule_ambiguities(&mut app, Update);
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
    let catalog = crate::profiles::AdvertisedBrawlerCatalog::from_content(
        &crate::builds::BuildCatalog::embedded().unwrap(),
        &crate::combat::WeaponCatalog::embedded().unwrap(),
    )
    .unwrap();

    assert_eq!(
        ultimate_name(&catalog, crate::builds::UltimateDefinitionId(5)),
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
        DASHBOARD_SETTINGS_INDEX,
        DASHBOARD_MENU_INDEX,
    ];
    assert_eq!(
        dashboard_focus_neighbor(
            DashboardLayoutClass::Wide,
            DASHBOARD_PLAY_INDEX,
            DashboardNavigationDirection::Left,
            &all,
        ),
        DASHBOARD_PRACTICE_INDEX
    );
    assert_eq!(
        dashboard_focus_neighbor(
            DashboardLayoutClass::Wide,
            DASHBOARD_PRACTICE_INDEX,
            DashboardNavigationDirection::Up,
            &all,
        ),
        DASHBOARD_BUILD_INDEX
    );
    assert_eq!(
        dashboard_focus_neighbor(
            DashboardLayoutClass::Compact,
            DASHBOARD_GAME_INDEX,
            DashboardNavigationDirection::Down,
            &all,
        ),
        DASHBOARD_PRACTICE_INDEX
    );
    assert_eq!(
        dashboard_focus_neighbor(
            DashboardLayoutClass::Compact,
            DASHBOARD_PLAY_INDEX,
            DashboardNavigationDirection::Up,
            &all,
        ),
        DASHBOARD_PRACTICE_INDEX
    );
}

#[test]
fn dashboard_navigation_skips_disabled_targets_and_repairs_focus() {
    let available = [
        DASHBOARD_GAME_INDEX,
        DASHBOARD_BUILD_INDEX,
        DASHBOARD_SETTINGS_INDEX,
        DASHBOARD_MENU_INDEX,
    ];
    assert_eq!(
        dashboard_focus_neighbor(
            DashboardLayoutClass::Wide,
            DASHBOARD_PLAY_INDEX,
            DashboardNavigationDirection::Left,
            &available,
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
            DASHBOARD_SETTINGS_INDEX,
            DashboardNavigationDirection::Left,
            &available,
        ),
        DASHBOARD_SETTINGS_INDEX
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
}

fn lobby_membership() -> ClientLobbyMembership {
    let account_id = crate::profiles::AccountId::new(1).unwrap();
    ClientLobbyMembership {
        logical_server_id: 1,
        player_id: crate::protocol::PlayerId(1),
        accepted_display_name: "Player".to_string(),
        server_name: "Test Lobby".to_string(),
        catalog_revision: crate::lobby::CatalogRevision([1; 32]),
        game_types: vec![crate::lobby::AdvertisedGameType {
            id: crate::lobby::GameTypeId::new("wipeout-2v2").unwrap(),
            configuration_revision: 1,
            display_name: "Wipeout 2v2".to_string(),
            mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
            map_preset_ids: vec![crate::map::MapPresetId(1)],
            team_count: 2,
            players_per_team: 2,
            rules_summary: crate::lobby::AdvertisedRulesSummary::Wipeout {
                target_score: 10,
                active_limit_ticks: 3_600,
            },
        }],
        brawler_catalog: crate::profiles::AdvertisedBrawlerCatalog::from_content(
            &crate::builds::BuildCatalog::embedded().unwrap(),
            &crate::combat::WeaponCatalog::embedded().unwrap(),
        )
        .unwrap(),
        profile: crate::profiles::ProfileSnapshot::empty(account_id),
    }
}

fn lobby_membership_with_brawler() -> ClientLobbyMembership {
    let mut membership = lobby_membership();
    let brawler_id = crate::profiles::SavedBrawlerId::new(2).unwrap();
    membership
        .profile
        .brawlers
        .push(crate::profiles::SavedBrawler {
            id: brawler_id,
            creation_ordinal: 1,
            name: "Test Brawler".into(),
            fighter_profile_id: crate::profiles::FighterProfileId(1),
            weapon_base_id: crate::profiles::WeaponBaseId(1),
            ultimate_id: crate::builds::UltimateDefinitionId(1),
            passive_ids: [
                crate::builds::PassiveDefinitionId(3),
                crate::builds::PassiveDefinitionId(4),
            ],
            equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
            revision: crate::profiles::ProfileRevision::INITIAL,
        });
    membership.profile.selected_brawler_id = Some(brawler_id);
    membership.profile.next_brawler_ordinal = 2;
    membership
}

fn lobby_membership_with_two_brawlers() -> ClientLobbyMembership {
    let mut membership = lobby_membership_with_brawler();
    let mut second = membership.profile.brawlers[0].clone();
    second.id = crate::profiles::SavedBrawlerId::new(3).unwrap();
    second.creation_ordinal = 2;
    second.name = "Second Brawler".into();
    second.revision = crate::profiles::ProfileRevision::INITIAL;
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
        .set_decision_for_test(crate::profiles::ProfileDecision::InvalidRequest);
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
    let mut app = flow_action_test_app();
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
    game_type_id: crate::lobby::GameTypeId,
) -> super::super::ClientMatchResultContext {
    super::super::ClientMatchResultContext {
        result: crate::matchplay::MatchResult::Draw,
        local_team: None,
        game_type_id: Some(game_type_id),
        game_name: None,
        final_score: None,
    }
}

fn queue_membership(membership: &ClientLobbyMembership) -> crate::lobby::QueueMembership {
    let brawler = &membership.profile.brawlers[0];
    let recipe = crate::builds::BrawlerBuildRecipe {
        weapon: crate::builds::WeaponChoice::Preset(crate::combat::WeaponPresetId(1)),
        ultimate: brawler.ultimate_id,
        passives: brawler.passive_ids,
    };
    crate::lobby::QueueMembership {
        ticket_id: crate::lobby::QueueTicketId::new(1).unwrap(),
        catalog_revision: membership.catalog_revision,
        game_type_id: membership.game_types[0].id.clone(),
        game_type_configuration_revision: membership.game_types[0].configuration_revision,
        brawler_id: brawler.id,
        brawler_revision: brawler.revision,
        accepted_build: crate::builds::AcceptedBuildSummary {
            canonical_recipe: recipe,
            identity: crate::builds::SelectedBuild {
                recipe_fingerprint: crate::builds::BuildRecipeFingerprint(1),
                revision: crate::builds::BuildCatalog::embedded()
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
        SessionObservation::QueueOutcome(crate::lobby::QueueCommandOutcome {
            request_id: crate::lobby::QueueRequestId::new(1).unwrap(),
            decision: crate::lobby::QueueDecision::Joined(joined.clone()),
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
        SessionObservation::QueueOutcome(crate::lobby::QueueCommandOutcome {
            request_id: crate::lobby::QueueRequestId::new(2).unwrap(),
            decision: crate::lobby::QueueDecision::Cancelled {
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
        SessionObservation::QueueOutcome(crate::lobby::QueueCommandOutcome {
            request_id: crate::lobby::QueueRequestId::new(3).unwrap(),
            decision: crate::lobby::QueueDecision::Rejected(
                crate::lobby::QueueRejection::StaleCatalog,
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
        SessionObservation::PracticeRejected(crate::lobby::PracticeStartRejection::StaleCatalog),
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
        SessionObservation::PracticeRejected(crate::lobby::PracticeStartRejection::Busy),
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
    assert_eq!(error.message, "The match server stopped unexpectedly");
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

#[test]
fn practice_queue_again_uses_fresh_generation_game_and_selected_brawler() {
    let membership = lobby_membership_with_brawler();
    let game_type_id = membership.game_types[0].id.clone();
    let selected_brawler = membership.profile.brawlers[0].clone();
    let mut app = configured_lobby_flow_app(membership);
    let mut stale_membership = lobby_membership_with_brawler();
    stale_membership.game_types[0].id = crate::lobby::GameTypeId::new("stale-wipeout").unwrap();
    stale_membership.profile.brawlers[0].id = crate::profiles::SavedBrawlerId::new(77).unwrap();
    stale_membership.profile.brawlers[0].revision =
        crate::profiles::ProfileRevision::new(9).unwrap();
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
    brawler_id: crate::profiles::SavedBrawlerId,
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
                crate::profiles::BrawlerEdit {
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
    let occupied_part = crate::weapon_parts::WeaponPartInstanceId::new(41).unwrap();
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
        FlowUiAction::SelectEquipmentSlot(crate::weapon_parts::WEAPON_PART_SLOT_COUNT),
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

    let free_part = crate::weapon_parts::WeaponPartInstanceId::new(42).unwrap();
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
    inject_action(&mut app, FlowUiAction::UnequipWeaponPart);
    assert_eq!(
        app.world()
            .resource::<WeaponEquipmentDraft>()
            .equipped_part_ids[1],
        None
    );
    inject_action(&mut app, FlowUiAction::EquipWeaponPart(free_part));
    inject_action(&mut app, FlowUiAction::ConfirmWeaponEquipment);
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerDetails(target)
    );
    assert!(
        app.world()
            .resource::<super::super::ClientProfileModel>()
            .pending()
    );

    app.world_mut()
        .resource_mut::<super::super::ClientProfileModel>()
        .set_snapshot_for_test(membership.profile.clone());
    inject_action(&mut app, FlowUiAction::OpenWeaponEquipment(target));
    inject_action(&mut app, FlowUiAction::CancelWeaponEquipment);
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerDetails(target)
    );
    *app.world_mut().resource_mut::<WeaponEquipmentDraft>() = WeaponEquipmentDraft::default();
    inject_action(&mut app, FlowUiAction::CancelWeaponEquipment);
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerList
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
        .set_decision_for_test(crate::profiles::ProfileDecision::Accepted);

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
        .set_decision_for_test(crate::profiles::ProfileDecision::Accepted);
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
        .set_decision_for_test(crate::profiles::ProfileDecision::InvalidRequest);
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
        .set_decision_for_test(crate::profiles::ProfileDecision::StaleRevision);
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
            .any(|text| text.contains("Default · Pulse Sidearm"))
    );
    assert!(
        dashboard_copy
            .iter()
            .any(|text| text.contains("Dash · Adrenal Response + Close Quarters"))
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
        visible_text(&mut app)
            .iter()
            .any(|text| text.contains("SELECTED FOR PLAY"))
    );
    assert!(
        visible_text(&mut app)
            .iter()
            .any(|text| text.contains("Dash"))
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
        let mut previews = world.query_filtered::<Entity, With<BrawlerDetailsPreviewHost>>();
        assert_eq!(previews.iter(world).count(), 1);
    }
    assert!(
        visible_text(&mut app)
            .iter()
            .any(|text| text.contains("Pulse Sidearm"))
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
    let (scroll_area, save_parent, save_disabled) = {
        let world = app.world_mut();
        let mut roots =
            world.query_filtered::<(Entity, &Node, &BackgroundColor), With<WeaponEquipmentRoot>>();
        let (_, node, background) = roots.single(world).unwrap();
        assert_eq!(
            (node.left, node.right, node.top, node.bottom),
            (px(0), px(0), px(0), px(0))
        );
        assert!((background.0.to_srgba().alpha - 1.0).abs() <= f32::EPSILON);
        let mut areas = world.query_filtered::<Entity, With<WeaponEquipmentScrollArea>>();
        let area = areas.single(world).unwrap();
        let mut buttons = world.query::<(&FlowButton, &ChildOf, Has<InteractionDisabled>)>();
        let (parent, disabled) = buttons
            .iter(world)
            .find(|(button, _, _)| button.action == FlowUiAction::ConfirmWeaponEquipment)
            .map(|(_, child_of, disabled)| (child_of.parent(), disabled))
            .expect("equipment Save button is rendered");
        (area, parent, disabled)
    };
    assert_ne!(save_parent, scroll_area, "Save remains in the fixed footer");
    assert!(!save_disabled, "a valid equipment preview can be saved");

    app.world_mut().write_message(MouseWheel {
        unit: MouseScrollUnit::Line,
        x: 0.0,
        y: -2.0,
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    app.update();
    assert!(
        (app.world().get::<ScrollPosition>(scroll_area).unwrap().0.y - 48.0).abs() <= f32::EPSILON
    );

    press_flow_button(&mut app, &FlowUiAction::ConfirmWeaponEquipment);
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::BrawlerDetails(brawler_id)
    );
    assert!(
        app.world()
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
    candidate.id = crate::profiles::SavedBrawlerId::new(3).unwrap();
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
fn dashboard_mode_card_separates_title_and_pool_without_claiming_a_selected_map() {
    let game = lobby_membership().game_types.remove(0);
    let summary = dashboard_game_summary(&game);
    assert!(game.display_name.contains("Wipeout"));
    assert!(summary.contains("First to"));
    assert!(summary.contains("Map pool:"));
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
    assert_eq!(labels.len(), 7);
    assert!(labels.iter().all(|(_, label)| !label.trim().is_empty()));
    assert!(labels.iter().any(|(style, label)| {
        matches!(style, DashboardButtonStyle::Preview) && label == "Create your first brawler"
    }));
    assert!(labels.iter().any(|(style, label)| {
        matches!(style, DashboardButtonStyle::Mode)
            && label.contains("Map pool:")
            && !label.contains("Selected map")
    }));
    assert!(
        labels.iter().any(|(style, label)| {
            matches!(style, DashboardButtonStyle::Play) && label == "Play"
        })
    );
}

#[test]
fn dashboard_fighter_preview_stays_transparent_during_every_interaction() {
    for interaction in [
        Interaction::None,
        Interaction::Hovered,
        Interaction::Pressed,
    ] {
        assert_eq!(
            flow_button_background(
                false,
                interaction,
                false,
                false,
                Some(DashboardButtonStyle::Preview),
            ),
            Color::NONE
        );
    }
    assert_ne!(
        flow_button_border(
            false,
            Interaction::Hovered,
            false,
            false,
            Some(DashboardButtonStyle::Preview),
        ),
        Color::NONE
    );
}

#[test]
fn dashboard_action_hover_colors_are_visibly_distinct_from_rest() {
    for style in [
        DashboardButtonStyle::Header,
        DashboardButtonStyle::Build,
        DashboardButtonStyle::Mode,
        DashboardButtonStyle::Practice,
        DashboardButtonStyle::Play,
    ] {
        assert_ne!(
            flow_button_background(false, Interaction::None, false, false, Some(style)),
            flow_button_background(false, Interaction::Hovered, false, false, Some(style)),
            "{style:?} must have a visible hover fill"
        );
    }
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
            crate::matchplay::MatchRoot,
            crate::matchplay::MatchState {
                match_id: crate::matchplay::MatchId(9),
                mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
                phase: crate::matchplay::MatchPhase::Completed {
                    completed_at_tick: 12,
                    restart_unlocked_at_tick: 72,
                    result: crate::matchplay::MatchResult::Draw,
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
                player_id: crate::protocol::PlayerId(1),
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
    let brawler_id = crate::profiles::SavedBrawlerId::new(2).unwrap();
    lobby.profile.brawlers.push(crate::profiles::SavedBrawler {
        id: brawler_id,
        creation_ordinal: 1,
        name: "Replay Brawler".into(),
        fighter_profile_id: crate::profiles::FighterProfileId(1),
        weapon_base_id: crate::profiles::WeaponBaseId(1),
        ultimate_id: crate::builds::UltimateDefinitionId(1),
        passive_ids: [
            crate::builds::PassiveDefinitionId(3),
            crate::builds::PassiveDefinitionId(4),
        ],
        equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
        revision: crate::profiles::ProfileRevision::INITIAL,
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
                player_id: crate::protocol::PlayerId(1),
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
            result: crate::matchplay::MatchResult::Draw,
            local_team: None,
            game_type_id: Some(game_type_id.clone()),
            game_name: None,
            final_score: None,
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
        crate::lobby::QueueCommand::Join(command) if command.game_type_id == game_type_id
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
    second.id = crate::lobby::GameTypeId::new("hot-zone-2v2").unwrap();
    second.configuration_revision = 2;
    second.display_name = "Hot Zone 2v2".to_string();
    second.mode_definition_id = crate::map::HOT_ZONE_MODE_DEFINITION;
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
    lobby.game_types = (0..crate::lobby::MAX_GAME_TYPES)
        .map(|index| {
            let mut game = prototype.clone();
            game.id = crate::lobby::GameTypeId::new(format!("test-game-{index}"))
                .expect("bounded test game ID");
            game.display_name = format!("Test Game {index}");
            game
        })
        .collect();
    app.world_mut().spawn((Client, lobby));
    *app.world_mut().resource_mut::<GameTypeSelectionDraft>() = GameTypeSelectionDraft {
        selected_index: Some(crate::lobby::MAX_GAME_TYPES - 1),
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
    second.id = crate::lobby::GameTypeId::new("hot-zone-2v2").unwrap();
    second.configuration_revision = 2;
    second.display_name = "Hot Zone 2v2".to_string();
    second.mode_definition_id = crate::map::HOT_ZONE_MODE_DEFINITION;
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
        result: crate::matchplay::MatchResult::Draw,
        local_team: None,
        game_type_id: Some(crate::lobby::GameTypeId::new("retired-mode").unwrap()),
        game_name: Some("Retired Mode".to_string()),
        final_score: None,
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
}

#[test]
fn queue_copy_uses_advertised_game_and_saved_brawler_recipe() {
    let builds = crate::builds::BuildCatalog::embedded().unwrap();
    let recipe = crate::builds::BrawlerBuildRecipe {
        weapon: crate::builds::WeaponChoice::Preset(crate::combat::WeaponPresetId(1)),
        ultimate: crate::builds::UltimateDefinitionId(1),
        passives: [
            crate::builds::PassiveDefinitionId(3),
            crate::builds::PassiveDefinitionId(4),
        ],
    };
    let membership = crate::lobby::QueueMembership {
        ticket_id: crate::lobby::QueueTicketId::new(1).unwrap(),
        catalog_revision: crate::lobby::CatalogRevision([1; 32]),
        game_type_id: crate::lobby::GameTypeId::new("wipeout-2v2").unwrap(),
        game_type_configuration_revision: 1,
        brawler_id: crate::profiles::SavedBrawlerId::new(1).unwrap(),
        brawler_revision: crate::profiles::ProfileRevision::INITIAL,
        accepted_build: crate::builds::AcceptedBuildSummary {
            canonical_recipe: recipe,
            identity: crate::builds::SelectedBuild {
                recipe_fingerprint: crate::builds::BuildRecipeFingerprint(1),
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
        &crate::lobby::ReservationStarted {
            reservation_id: crate::lobby::MatchReservationId::new(1).unwrap(),
            ticket_id: Some(membership.ticket_id),
            game_type_id: membership.game_type_id.clone(),
            map_preset_id: crate::map::MapPresetId(1),
            team_count: 2,
            players_per_team: 2,
            accepted_build: membership.accepted_build,
            loading_deadline_millis: 30_000,
        },
        Some(crate::lobby::MatchLoadingPhase::Synchronizing),
    );
    assert!(loading_copy.contains("Synchronizing map"));
    assert!(loading_copy.contains("Saved brawler accepted"));
    assert!(!loading_copy.contains("points"));
}

#[test]
fn cancel_pending_copy_is_explicit_and_disables_only_cancel() {
    let cancel = crate::lobby::QueueCommand::Cancel(crate::lobby::QueueCancelCommand {
        ticket_id: crate::lobby::QueueTicketId::new(1).unwrap(),
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
        crate::protocol::LobbyJoinRejection::InvalidName,
    ));
    assert_eq!(
        invalid_name.actions,
        [Some(FlowErrorAction::EditName), Some(FlowErrorAction::Back)]
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
