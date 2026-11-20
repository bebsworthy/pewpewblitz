//! Focused queue state-machine and automation characterization tests.

use super::*;
use crate::ClientLobbyMembership;
use std::time::Duration;

fn game() -> brawler::lobby::AdvertisedGameType {
    brawler::lobby::AdvertisedGameType {
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
            active_limit_ticks: 1_000,
        },
    }
}

#[cfg(feature = "automation")]
#[test]
fn automation_can_target_an_exact_game_instead_of_the_first_matching_roster() {
    let wipeout = game();
    let mut hot_zone = wipeout.clone();
    hot_zone.id = brawler::lobby::GameTypeId::new("hot-zone-2v2").unwrap();
    hot_zone.mode_definition_id = brawler::map::HOT_ZONE_MODE_DEFINITION;
    let games = [wipeout, hot_zone];
    assert_eq!(
        automation_game_type(&games, 2, None).unwrap().id.as_str(),
        "wipeout-2v2"
    );
    let requested = brawler::lobby::GameTypeId::new("hot-zone-2v2").unwrap();
    assert_eq!(
        automation_game_type(&games, 2, Some(&requested))
            .unwrap()
            .id
            .as_str(),
        "hot-zone-2v2"
    );
}

fn membership() -> ClientLobbyMembership {
    let catalog = brawler::profiles::AdvertisedCharacterCatalog::embedded().unwrap();
    ClientLobbyMembership {
        logical_server_id: 1,
        player_id: brawler::protocol::PlayerId(1),
        accepted_display_name: "Player".to_string(),
        server_name: "Server".to_string(),
        catalog_revision: brawler::lobby::CatalogRevision([1; 32]),
        game_types: vec![game()],
        profile: catalog.profile_from_stored(
            brawler::profiles::AccountId::new(1).unwrap(),
            brawler::profiles::ProfileRevision::INITIAL,
            None,
            [],
        ),
        character_catalog: catalog,
    }
}

fn snapshot(revision: u64, queued: u16) -> brawler::lobby::QueuePoolSnapshot {
    brawler::lobby::QueuePoolSnapshot {
        catalog_revision: brawler::lobby::CatalogRevision([1; 32]),
        state_revision: revision,
        formation_availability: brawler::lobby::FormationAvailability::Available,
        pools: vec![brawler::lobby::QueuePoolRow {
            game_type_id: brawler::lobby::GameTypeId::new("wipeout-2v2").unwrap(),
            game_type_configuration_revision: 1,
            queued,
            formation_size: 4,
        }],
    }
}

fn joined_membership(game_type_id: &str, ticket_id: u128) -> brawler::lobby::QueueMembership {
    let catalogs = brawler::characters::EmbeddedLoadoutCatalogs::load().unwrap();
    let choice = catalogs.characters.default_choice();
    let resolved = catalogs.view().resolve(choice).unwrap();
    brawler::lobby::QueueMembership {
        ticket_id: brawler::lobby::QueueTicketId::new(ticket_id).unwrap(),
        catalog_revision: brawler::lobby::CatalogRevision([1; 32]),
        game_type_id: brawler::lobby::GameTypeId::new(game_type_id).unwrap(),
        game_type_configuration_revision: 1,
        profile_revision: brawler::profiles::ProfileRevision::INITIAL,
        accepted_build: brawler::builds::AcceptedBuildSummary {
            character: choice,
            identity: resolved.identity,
        },
        admitted_at_pool_state_revision: 2,
    }
}

#[test]
fn practice_request_is_single_flight_and_rejection_clears_it() {
    let mut model = ClientPracticeModel::default();
    model.bind_generation(7);
    let selected = super::super::flow::SelectedGameType {
        catalog_revision: Some(brawler::lobby::CatalogRevision([1; 32])),
        game_type_id: Some(brawler::lobby::GameTypeId::new("hot-zone-3v3").unwrap()),
        configuration_revision: Some(1),
    };
    let revision = brawler::profiles::ProfileRevision::INITIAL;
    assert!(model.start(&selected, revision));
    assert!(model.pending());
    assert!(!model.start(&selected, revision));
    let request = model.outbound.pop_front().unwrap();
    assert_eq!(request.game_type_id.as_str(), "hot-zone-3v3");
    model.accept_rejection(
        request.request_id,
        brawler::lobby::PracticeStartRejection::CapacityUnavailable,
    );
    assert!(!model.pending());
    assert_eq!(
        model.take_rejection(),
        Some(brawler::lobby::PracticeStartRejection::CapacityUnavailable)
    );
}

#[test]
fn cancelled_match_start_clears_loading_and_returns_to_game_select_observation() {
    let joined = joined_membership("wipeout-2v2", 7);
    let reservation_id = brawler::lobby::MatchReservationId::new(11).unwrap();
    let mut model = ClientMatchLoadingModel {
        active: Some(brawler::lobby::ReservationStarted {
            reservation_id,
            ticket_id: Some(joined.ticket_id),
            game_type_id: joined.game_type_id,
            mode_definition_id: brawler::map::WIPEOUT_MODE_DEFINITION,
            map_preset_id: Some(brawler::map::MapPresetId(1)),
            map_recipe_id: brawler::map::MapRecipeId(1),
            map_recipe_revision: 1,
            map_recipe_fingerprint: brawler::map::MapRecipeFingerprint(1),
            team_count: 2,
            players_per_team: 2,
            accepted_build: joined.accepted_build,
            loading_deadline_millis: 30_000,
            published_map: None,
        }),
        ..default()
    };

    assert!(model.request_cancel());
    let message = model.outbound.pop_front().expect("one cancel intent");
    assert!(matches!(
        message.action,
        brawler::lobby::MatchmakingClientAction::Cancel {
            reservation_id: id,
            generation: 1,
        } if id == reservation_id
    ));
    assert!(model.match_cancel_requested());

    model.observe_match_cancellation(true);
    assert!(!model.match_cancel_requested());
    assert!(model.outbound.is_empty());
    assert!(model.active().is_none());
    assert_eq!(
        model.phase(),
        Some(brawler::lobby::MatchLoadingPhase::ReturningToQueue)
    );
    assert!(model.take_returned());
    assert!(!model.take_returned());
}

#[test]
fn fresh_lobby_generation_discards_completed_match_loading_state() {
    let joined = joined_membership("wipeout-2v2", 7);
    let mut model = ClientMatchLoadingModel {
        lobby_generation: Some(1),
        expected_sequence: 4,
        active: Some(brawler::lobby::ReservationStarted {
            reservation_id: brawler::lobby::MatchReservationId::new(11).unwrap(),
            ticket_id: Some(joined.ticket_id),
            game_type_id: joined.game_type_id,
            mode_definition_id: brawler::map::WIPEOUT_MODE_DEFINITION,
            map_preset_id: Some(brawler::map::MapPresetId(1)),
            map_recipe_id: brawler::map::MapRecipeId(1),
            map_recipe_revision: 1,
            map_recipe_fingerprint: brawler::map::MapRecipeFingerprint(1),
            team_count: 2,
            players_per_team: 2,
            accepted_build: joined.accepted_build,
            loading_deadline_millis: 30_000,
            published_map: None,
        }),
        phase: Some(brawler::lobby::MatchLoadingPhase::WaitingForPlayers),
        protocol_failure: true,
        ..default()
    };

    model.reset_for_lobby_generation(3);

    assert_eq!(model.lobby_generation, Some(3));
    assert_eq!(model.expected_sequence, 0);
    assert!(model.active().is_none());
    assert_eq!(model.phase(), None);
    assert!(!model.protocol_failure);
}

#[test]
fn equal_snapshot_refreshes_freshness_older_does_not_and_conflict_fails() {
    let membership = membership();
    let mut model = ClientQueueModel {
        generation: Some(1),
        ..default()
    };
    model.accept_snapshot(snapshot(2, 1), &membership, Duration::ZERO);
    model.update_time(Duration::from_secs(4));
    assert!(model.snapshot().is_none());
    assert_eq!(model.freshness_aged, 1);
    model.accept_snapshot(snapshot(1, 0), &membership, Duration::from_secs(4));
    assert!(model.snapshot().is_none());
    model.accept_snapshot(snapshot(2, 1), &membership, Duration::from_secs(4));
    assert!(model.snapshot().is_some());
    assert_eq!(model.freshness_restored, 2);
    model.accept_snapshot(snapshot(2, 2), &membership, Duration::from_secs(5));
    assert!(model.protocol_failure());
}

#[test]
fn pending_timeout_retry_keeps_request_and_rate_limit_try_again_changes_it() {
    let mut model = ClientQueueModel {
        generation: Some(1),
        ..default()
    };
    let selected = super::super::flow::SelectedGameType {
        catalog_revision: Some(brawler::lobby::CatalogRevision([1; 32])),
        game_type_id: Some(brawler::lobby::GameTypeId::new("wipeout-2v2").unwrap()),
        configuration_revision: Some(1),
    };
    assert!(model.start_join(
        &selected,
        brawler::profiles::ProfileRevision::INITIAL,
        Duration::ZERO,
    ));
    let first = model.pending().unwrap().request_id;
    model.update_time(Duration::from_secs(10));
    assert!(model.pending().unwrap().timed_out);
    assert!(model.take_timeout_notice());
    assert!(!model.take_timeout_notice());
    assert!(model.retry_pending(Duration::from_secs(10)));
    assert_eq!(model.pending().unwrap().request_id, first);
    model.accept_outcome(
        brawler::lobby::QueueCommandOutcome {
            request_id: first,
            decision: brawler::lobby::QueueDecision::Rejected(
                brawler::lobby::QueueRejection::RateLimited {
                    retry_after_millis: 500,
                },
            ),
        },
        Duration::from_secs(11),
    );
    assert!(!model.try_again_after_rate_limit(Duration::from_millis(11_499)));
    assert!(model.try_again_after_rate_limit(Duration::from_millis(11_500)));
    assert!(model.pending().unwrap().request_id > first);
}

#[test]
fn cancellation_revision_hides_an_older_fresh_snapshot_until_replacement_arrives() {
    let lobby = membership();
    let ticket_id = brawler::lobby::QueueTicketId::new(9).unwrap();
    let mut model = ClientQueueModel {
        generation: Some(1),
        membership: Some(brawler::lobby::QueueMembership {
            ticket_id,
            catalog_revision: lobby.catalog_revision,
            game_type_id: lobby.game_types[0].id.clone(),
            game_type_configuration_revision: 1,
            profile_revision: brawler::profiles::ProfileRevision::INITIAL,
            accepted_build: brawler::builds::AcceptedBuildSummary {
                character: lobby.profile.selection(),
                identity: brawler::builds::SelectedBuild {
                    recipe_fingerprint: brawler::builds::BuildRecipeFingerprint(1),
                    revision: brawler::builds::BuildRevision(1),
                },
            },
            admitted_at_pool_state_revision: 2,
        }),
        ..default()
    };
    model.accept_snapshot(snapshot(2, 1), &lobby, Duration::ZERO);
    assert!(model.start_cancel(Duration::ZERO));
    let request_id = model.pending().unwrap().request_id;
    model.accept_outcome(
        brawler::lobby::QueueCommandOutcome {
            request_id,
            decision: brawler::lobby::QueueDecision::Cancelled {
                ticket_id,
                resulting_pool_state_revision: 3,
            },
        },
        Duration::ZERO,
    );

    assert!(model.snapshot().is_none());
    assert!(model.raw_snapshot().is_some());
    model.accept_snapshot(snapshot(3, 0), &lobby, Duration::from_millis(1));
    assert!(model.snapshot().is_some());
}

#[test]
fn late_outcome_remains_authoritative_after_timeout_notice() {
    let mut model = ClientQueueModel {
        generation: Some(1),
        ..default()
    };
    let selected = super::super::flow::SelectedGameType {
        catalog_revision: Some(brawler::lobby::CatalogRevision([1; 32])),
        game_type_id: Some(brawler::lobby::GameTypeId::new("wipeout-2v2").unwrap()),
        configuration_revision: Some(1),
    };
    assert!(model.start_join(
        &selected,
        brawler::profiles::ProfileRevision::INITIAL,
        Duration::ZERO,
    ));
    let request_id = model.pending().unwrap().request_id;
    model.update_time(Duration::from_secs(10));
    assert!(model.take_timeout_notice());
    model.accept_outcome(
        brawler::lobby::QueueCommandOutcome {
            request_id,
            decision: brawler::lobby::QueueDecision::Joined(joined_membership("wipeout-2v2", 9)),
        },
        Duration::from_secs(11),
    );
    assert!(model.pending().is_none());
    assert!(model.membership().is_some());
    assert!(matches!(
        model.take_outcome().unwrap().decision,
        brawler::lobby::QueueDecision::Joined(_)
    ));
}

#[test]
fn joined_outcome_must_match_the_frozen_join_target() {
    let mut model = ClientQueueModel {
        generation: Some(1),
        ..default()
    };
    let selected = super::super::flow::SelectedGameType {
        catalog_revision: Some(brawler::lobby::CatalogRevision([1; 32])),
        game_type_id: Some(brawler::lobby::GameTypeId::new("wipeout-2v2").unwrap()),
        configuration_revision: Some(1),
    };
    assert!(model.start_join(
        &selected,
        brawler::profiles::ProfileRevision::INITIAL,
        Duration::ZERO,
    ));
    let request_id = model.pending().unwrap().request_id;

    model.accept_outcome(
        brawler::lobby::QueueCommandOutcome {
            request_id,
            decision: brawler::lobby::QueueDecision::Joined(joined_membership("hot-zone-2v2", 9)),
        },
        Duration::ZERO,
    );

    assert!(model.protocol_failure());
    assert!(model.membership().is_none());
    assert!(model.pending().is_some());
    assert_eq!(
        model.outbound.len(),
        1,
        "invalid outcome is not acknowledged"
    );
}

#[test]
fn joined_outcome_cannot_replace_membership_while_cancel_is_pending() {
    let current = joined_membership("wipeout-2v2", 8);
    let mut model = ClientQueueModel {
        generation: Some(1),
        membership: Some(current.clone()),
        ..default()
    };
    assert!(model.start_cancel(Duration::ZERO));
    let request_id = model.pending().unwrap().request_id;

    model.accept_outcome(
        brawler::lobby::QueueCommandOutcome {
            request_id,
            decision: brawler::lobby::QueueDecision::Joined(joined_membership("wipeout-2v2", 9)),
        },
        Duration::ZERO,
    );

    assert!(model.protocol_failure());
    assert_eq!(model.membership(), Some(&current));
    assert!(model.pending().is_some());
}

#[test]
fn practice_selection_survives_replay_but_cannot_leak_to_another_game() {
    let game = brawler::lobby::GameTypeId::new("wipeout-2v2").unwrap();
    let mut model = ClientPracticeModel::default();
    model.bind_generation(1);
    model.selected_map = Some((game.clone(), brawler::map::MapPresetId(10)));
    let mut selected = crate::flow::SelectedGameType {
        catalog_revision: Some(brawler::lobby::CatalogRevision([1; 32])),
        game_type_id: Some(game),
        configuration_revision: Some(1),
    };
    assert!(model.start(&selected, brawler::profiles::ProfileRevision::INITIAL));
    assert_eq!(
        model.pending.as_ref().unwrap().map_preset_id,
        Some(brawler::map::MapPresetId(10))
    );
    assert!(!model.start(&selected, brawler::profiles::ProfileRevision::INITIAL));
    model.bind_generation(2);
    assert!(model.start(&selected, brawler::profiles::ProfileRevision::INITIAL));
    assert_eq!(
        model.pending.as_ref().unwrap().map_preset_id,
        Some(brawler::map::MapPresetId(10))
    );
    model.bind_generation(3);
    selected.game_type_id = Some(brawler::lobby::GameTypeId::new("heist-2v2").unwrap());
    assert!(model.start(&selected, brawler::profiles::ProfileRevision::INITIAL));
    assert_eq!(model.pending.as_ref().unwrap().map_preset_id, None);
}

/// BRL-s3dh: one published-maps request in flight per lobby generation; a reply for another
/// request is ignored; a successful withdraw reports its map once; a new generation forgets the
/// listing so My Maps asks again.
#[test]
fn published_maps_keep_one_request_in_flight_and_the_last_server_listing() {
    use brawler::lobby::{
        PublishedMapId, PublishedMapListing, PublishedMapSummary, PublishedMapsRejection,
        PublishedMapsReply, PublishedMapsRequestId, PublishedMapsResult,
    };
    let mut model = ClientPublishedMapsModel::default();
    assert!(!model.request_list(), "no lobby session yet");
    model.bind_generation(1);
    assert!(model.needs_list());
    assert!(model.request_list());
    assert!(!model.withdraw(PublishedMapId::new(1).unwrap()));
    let map = PublishedMapSummary {
        published_map_id: PublishedMapId::new(1).unwrap(),
        name: "Yard".to_string(),
        game_type_id: brawler::lobby::GameTypeId::new("wipeout-1v1").unwrap(),
        listing: PublishedMapListing::Pending,
    };
    let reply = |id: u64, result| PublishedMapsReply {
        request_id: PublishedMapsRequestId::new(id).unwrap(),
        result,
    };
    model.accept_reply(reply(9, PublishedMapsResult::Listed(Vec::new())));
    assert!(model.pending(), "a reply to another request is ignored");
    model.accept_reply(reply(1, PublishedMapsResult::Listed(vec![map.clone()])));
    assert!(!model.pending() && !model.needs_list());
    assert!(model.take_changed());
    assert_eq!(model.maps(), Some(&[map.clone()][..]));

    assert!(model.withdraw(map.published_map_id));
    assert!(model.withdrawing(map.published_map_id));
    let removed = PublishedMapSummary {
        listing: PublishedMapListing::Removed { reason: None },
        ..map.clone()
    };
    model.accept_reply(reply(2, PublishedMapsResult::Listed(vec![removed])));
    assert_eq!(model.take_withdrawn(), Some(map.published_map_id));
    assert_eq!(model.take_withdrawn(), None);
    assert!(model.withdraw(map.published_map_id));
    model.accept_reply(reply(
        3,
        PublishedMapsResult::Rejected(PublishedMapsRejection::NotWithdrawable),
    ));
    assert_eq!(
        model.take_rejection(),
        Some(PublishedMapsRejection::NotWithdrawable)
    );
    assert_eq!(
        model.take_withdrawn(),
        None,
        "a refused withdraw removed nothing"
    );

    model.bind_generation(2);
    assert!(model.needs_list(), "a new lobby session lists again");
    assert_eq!(
        published_map_chip(&PublishedMapListing::Approved { in_rotation: true }),
        "IN ROTATION"
    );
}

#[test]
fn map_publish_allows_one_request_per_lobby_generation_and_reports_its_outcome_once() {
    let mut model = ClientMapPublishModel::default();
    let mut game = game();
    game.community_maps = true;
    let revision = brawler::lobby::CatalogRevision([1; 32]);
    let candidate = || brawler::map::AuthoredMapCandidate {
        draft_id: 3,
        recipe: brawler::map::MapContentCatalog::embedded()
            .unwrap()
            .preset(brawler::map::FEATURE_YARD_WIPEOUT_PRESET)
            .unwrap()
            .recipe
            .clone(),
    };
    assert!(
        !model.start(revision, &game, "Yard".into(), candidate()),
        "no lobby session yet"
    );
    model.bind_generation(1);
    assert!(model.start(revision, &game, "Yard".into(), candidate()));
    assert!(model.pending());
    assert!(!model.start(revision, &game, "Yard".into(), candidate()));
    let request_id = brawler::lobby::MapPublishRequestId::new(1).unwrap();
    let accepted = brawler::lobby::MapPublishDecision::Accepted {
        published_map_id: brawler::lobby::PublishedMapId::new(4).unwrap(),
        state: brawler::lobby::PublishedMapState::Pending,
    };
    // An outcome for another request is ignored.
    model.accept_outcome(brawler::lobby::MapPublishOutcome {
        request_id: brawler::lobby::MapPublishRequestId::new(9).unwrap(),
        decision: accepted,
    });
    assert!(model.pending());
    model.accept_outcome(brawler::lobby::MapPublishOutcome {
        request_id,
        decision: accepted,
    });
    assert!(!model.pending());
    assert_eq!(model.take_outcome(), Some(accepted));
    assert_eq!(model.take_outcome(), None);
    assert!(map_publish_decision_text(accepted).contains("review"));
    assert_eq!(
        map_publish_decision_text(brawler::lobby::MapPublishDecision::Rejected(
            brawler::lobby::MapPublishRejection::CoolingDown {
                retry_after_seconds: 42
            }
        )),
        "Publish again in 42 s."
    );
}
