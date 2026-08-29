//! Focused queue state-machine and automation characterization tests.

use super::*;
use crate::client::ClientLobbyMembership;
use std::time::Duration;

fn game() -> crate::lobby::AdvertisedGameType {
    crate::lobby::AdvertisedGameType {
        id: crate::lobby::GameTypeId::new("wipeout-2v2").unwrap(),
        configuration_revision: 1,
        display_name: "Wipeout 2v2".to_string(),
        mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
        map_preset_ids: vec![crate::map::MapPresetId(1)],
        team_count: 2,
        players_per_team: 2,
        rules_summary: crate::lobby::AdvertisedRulesSummary::Wipeout {
            target_score: 10,
            active_limit_ticks: 1_000,
        },
    }
}

#[test]
fn automation_can_target_an_exact_game_instead_of_the_first_matching_roster() {
    let wipeout = game();
    let mut hot_zone = wipeout.clone();
    hot_zone.id = crate::lobby::GameTypeId::new("hot-zone-2v2").unwrap();
    hot_zone.mode_definition_id = crate::map::HOT_ZONE_MODE_DEFINITION;
    let games = [wipeout, hot_zone];
    assert_eq!(
        automation_game_type(&games, 2, None).unwrap().id.as_str(),
        "wipeout-2v2"
    );
    let requested = crate::lobby::GameTypeId::new("hot-zone-2v2").unwrap();
    assert_eq!(
        automation_game_type(&games, 2, Some(&requested))
            .unwrap()
            .id
            .as_str(),
        "hot-zone-2v2"
    );
}

fn membership() -> ClientLobbyMembership {
    ClientLobbyMembership {
        logical_server_id: 1,
        player_id: crate::protocol::PlayerId(1),
        accepted_display_name: "Player".to_string(),
        server_name: "Server".to_string(),
        catalog_revision: crate::lobby::CatalogRevision([1; 32]),
        game_types: vec![game()],
        brawler_catalog: crate::profiles::AdvertisedBrawlerCatalog::from_content(
            &crate::builds::BuildCatalog::embedded().unwrap(),
            &crate::combat::WeaponCatalog::embedded().unwrap(),
        )
        .unwrap(),
        profile: crate::profiles::ProfileSnapshot::empty(
            crate::profiles::AccountId::new(1).unwrap(),
        ),
    }
}

fn snapshot(revision: u64, queued: u16) -> crate::lobby::QueuePoolSnapshot {
    crate::lobby::QueuePoolSnapshot {
        catalog_revision: crate::lobby::CatalogRevision([1; 32]),
        state_revision: revision,
        formation_availability: crate::lobby::FormationAvailability::Available,
        pools: vec![crate::lobby::QueuePoolRow {
            game_type_id: crate::lobby::GameTypeId::new("wipeout-2v2").unwrap(),
            game_type_configuration_revision: 1,
            queued,
            formation_size: 4,
        }],
    }
}

fn joined_membership(game_type_id: &str, ticket_id: u128) -> crate::lobby::QueueMembership {
    let builds = crate::builds::BuildCatalog::embedded().unwrap();
    let weapons = crate::combat::WeaponCatalog::embedded().unwrap();
    let fighter = crate::combat::FighterDefinitions::default().entries[0];
    let brawler = crate::profiles::SavedBrawler {
        id: crate::profiles::SavedBrawlerId::new(1).unwrap(),
        creation_ordinal: 1,
        name: "Queue Brawler".into(),
        fighter_profile_id: crate::profiles::FighterProfileId(1),
        weapon_base_id: crate::profiles::WeaponBaseId(1),
        ultimate_id: crate::builds::UltimateDefinitionId(1),
        passive_ids: [
            crate::builds::PassiveDefinitionId(3),
            crate::builds::PassiveDefinitionId(4),
        ],
        equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
        revision: crate::profiles::ProfileRevision::INITIAL,
    };
    let resolved = brawler
        .resolve_loadout(&builds, &weapons, &fighter)
        .unwrap();
    crate::lobby::QueueMembership {
        ticket_id: crate::lobby::QueueTicketId::new(ticket_id).unwrap(),
        catalog_revision: crate::lobby::CatalogRevision([1; 32]),
        game_type_id: crate::lobby::GameTypeId::new(game_type_id).unwrap(),
        game_type_configuration_revision: 1,
        brawler_id: crate::profiles::SavedBrawlerId::new(1).unwrap(),
        brawler_revision: crate::profiles::ProfileRevision::INITIAL,
        accepted_build: crate::builds::AcceptedBuildSummary {
            canonical_recipe: crate::builds::BrawlerBuildRecipe {
                weapon: crate::builds::WeaponChoice::Preset(crate::combat::WeaponPresetId(1)),
                ultimate: crate::builds::UltimateDefinitionId(1),
                passives: [
                    crate::builds::PassiveDefinitionId(3),
                    crate::builds::PassiveDefinitionId(4),
                ],
            },
            identity: resolved.identity,
            total_points: 0,
        },
        admitted_at_pool_state_revision: 2,
    }
}

#[test]
fn practice_request_is_single_flight_and_rejection_clears_it() {
    let mut model = ClientPracticeModel::default();
    model.bind_generation(7);
    let selected = super::super::flow::SelectedGameType {
        catalog_revision: Some(crate::lobby::CatalogRevision([1; 32])),
        game_type_id: Some(crate::lobby::GameTypeId::new("hot-zone-3v3").unwrap()),
        configuration_revision: Some(1),
    };
    let brawler_id = crate::profiles::SavedBrawlerId::new(1).unwrap();
    let revision = crate::profiles::ProfileRevision::INITIAL;
    assert!(model.start(&selected, brawler_id, revision));
    assert!(model.pending());
    assert!(!model.start(&selected, brawler_id, revision));
    let request = model.outbound.pop_front().unwrap();
    assert_eq!(request.game_type_id.as_str(), "hot-zone-3v3");
    model.accept_rejection(
        request.request_id,
        crate::lobby::PracticeStartRejection::CapacityUnavailable,
    );
    assert!(!model.pending());
    assert_eq!(
        model.take_rejection(),
        Some(crate::lobby::PracticeStartRejection::CapacityUnavailable)
    );
}

#[test]
fn cancelled_match_start_clears_loading_and_returns_to_game_select_observation() {
    let joined = joined_membership("wipeout-2v2", 7);
    let reservation_id = crate::lobby::MatchReservationId::new(11).unwrap();
    let mut model = ClientMatchLoadingModel {
        active: Some(crate::lobby::ReservationStarted {
            reservation_id,
            ticket_id: Some(joined.ticket_id),
            game_type_id: joined.game_type_id,
            map_preset_id: crate::map::MapPresetId(1),
            team_count: 2,
            players_per_team: 2,
            accepted_build: joined.accepted_build,
            loading_deadline_millis: 30_000,
        }),
        ..default()
    };

    assert!(model.request_cancel());
    let message = model.outbound.pop_front().expect("one cancel intent");
    assert!(matches!(
        message.action,
        crate::lobby::MatchmakingClientAction::Cancel {
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
        Some(crate::lobby::MatchLoadingPhase::ReturningToQueue)
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
        active: Some(crate::lobby::ReservationStarted {
            reservation_id: crate::lobby::MatchReservationId::new(11).unwrap(),
            ticket_id: Some(joined.ticket_id),
            game_type_id: joined.game_type_id,
            map_preset_id: crate::map::MapPresetId(1),
            team_count: 2,
            players_per_team: 2,
            accepted_build: joined.accepted_build,
            loading_deadline_millis: 30_000,
        }),
        phase: Some(crate::lobby::MatchLoadingPhase::WaitingForPlayers),
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
        catalog_revision: Some(crate::lobby::CatalogRevision([1; 32])),
        game_type_id: Some(crate::lobby::GameTypeId::new("wipeout-2v2").unwrap()),
        configuration_revision: Some(1),
    };
    assert!(model.start_join(
        &selected,
        crate::profiles::SavedBrawlerId::new(1).unwrap(),
        crate::profiles::ProfileRevision::INITIAL,
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
        crate::lobby::QueueCommandOutcome {
            request_id: first,
            decision: crate::lobby::QueueDecision::Rejected(
                crate::lobby::QueueRejection::RateLimited {
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
    let ticket_id = crate::lobby::QueueTicketId::new(9).unwrap();
    let mut model = ClientQueueModel {
        generation: Some(1),
        membership: Some(crate::lobby::QueueMembership {
            ticket_id,
            catalog_revision: lobby.catalog_revision,
            game_type_id: lobby.game_types[0].id.clone(),
            game_type_configuration_revision: 1,
            brawler_id: crate::profiles::SavedBrawlerId::new(1).unwrap(),
            brawler_revision: crate::profiles::ProfileRevision::INITIAL,
            accepted_build: crate::builds::AcceptedBuildSummary {
                canonical_recipe: crate::builds::BrawlerBuildRecipe {
                    weapon: crate::builds::WeaponChoice::Preset(crate::combat::WeaponPresetId(1)),
                    ultimate: crate::builds::UltimateDefinitionId(1),
                    passives: [
                        crate::builds::PassiveDefinitionId(3),
                        crate::builds::PassiveDefinitionId(4),
                    ],
                },
                identity: crate::builds::SelectedBuild {
                    recipe_fingerprint: crate::builds::BuildRecipeFingerprint(1),
                    revision: crate::builds::BuildRevision(1),
                },
                total_points: 10,
            },
            admitted_at_pool_state_revision: 2,
        }),
        ..default()
    };
    model.accept_snapshot(snapshot(2, 1), &lobby, Duration::ZERO);
    assert!(model.start_cancel(Duration::ZERO));
    let request_id = model.pending().unwrap().request_id;
    model.accept_outcome(
        crate::lobby::QueueCommandOutcome {
            request_id,
            decision: crate::lobby::QueueDecision::Cancelled {
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
        catalog_revision: Some(crate::lobby::CatalogRevision([1; 32])),
        game_type_id: Some(crate::lobby::GameTypeId::new("wipeout-2v2").unwrap()),
        configuration_revision: Some(1),
    };
    assert!(model.start_join(
        &selected,
        crate::profiles::SavedBrawlerId::new(1).unwrap(),
        crate::profiles::ProfileRevision::INITIAL,
        Duration::ZERO,
    ));
    let request_id = model.pending().unwrap().request_id;
    model.update_time(Duration::from_secs(10));
    assert!(model.take_timeout_notice());
    model.accept_outcome(
        crate::lobby::QueueCommandOutcome {
            request_id,
            decision: crate::lobby::QueueDecision::Joined(joined_membership("wipeout-2v2", 9)),
        },
        Duration::from_secs(11),
    );
    assert!(model.pending().is_none());
    assert!(model.membership().is_some());
    assert!(matches!(
        model.take_outcome().unwrap().decision,
        crate::lobby::QueueDecision::Joined(_)
    ));
}

#[test]
fn joined_outcome_must_match_the_frozen_join_target() {
    let mut model = ClientQueueModel {
        generation: Some(1),
        ..default()
    };
    let selected = super::super::flow::SelectedGameType {
        catalog_revision: Some(crate::lobby::CatalogRevision([1; 32])),
        game_type_id: Some(crate::lobby::GameTypeId::new("wipeout-2v2").unwrap()),
        configuration_revision: Some(1),
    };
    assert!(model.start_join(
        &selected,
        crate::profiles::SavedBrawlerId::new(1).unwrap(),
        crate::profiles::ProfileRevision::INITIAL,
        Duration::ZERO,
    ));
    let request_id = model.pending().unwrap().request_id;

    model.accept_outcome(
        crate::lobby::QueueCommandOutcome {
            request_id,
            decision: crate::lobby::QueueDecision::Joined(joined_membership("hot-zone-2v2", 9)),
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
        crate::lobby::QueueCommandOutcome {
            request_id,
            decision: crate::lobby::QueueDecision::Joined(joined_membership("wipeout-2v2", 9)),
        },
        Duration::ZERO,
    );

    assert!(model.protocol_failure());
    assert_eq!(model.membership(), Some(&current));
    assert!(model.pending().is_some());
}
