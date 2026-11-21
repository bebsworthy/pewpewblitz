//! BRL-w9z1: a player publishes a Map Maker recipe over the routed lobby session, the lobby
//! re-validates and stores it, an operator decision puts it into a game type's queue rotation,
//! and a queue match carries the stored recipe to the match worker, which every client loads.
//! BRL-s3dh: an author lists and withdraws only their own maps; a withdrawn approved map leaves
//! rotation. BRL-1sgj: the reservation's map credit stays with the client through the handoff.

use super::queue::{
    apply_lobby_control, grant_pending_allocation, start_product_join, wait_for_product_lobby,
};
use super::*;
use brawler::lobby::{
    MapPublishDecision, MapPublishRejection, PublishedMapId, PublishedMapListing,
    PublishedMapState, PublishedMapSummary, PublishedMapsRejection,
};

const COMMUNITY_GAME: &str = "takedown-1v1";

fn advertised_game(
    harness: &Harness,
    index: usize,
    id: &str,
) -> brawler::lobby::AdvertisedGameType {
    harness.clients[index]
        .world()
        .get::<brawler_client::ClientLobbyMembership>(harness.client_entities[index])
        .unwrap()
        .game_types
        .iter()
        .find(|game| game.id.as_str() == id)
        .unwrap_or_else(|| panic!("{id} is advertised"))
        .clone()
}

fn game_index(harness: &Harness, id: &str) -> usize {
    harness.clients[0]
        .world()
        .get::<brawler_client::ClientLobbyMembership>(harness.client_entities[0])
        .unwrap()
        .game_types
        .iter()
        .position(|game| game.id.as_str() == id)
        .unwrap()
}

fn preset_recipe(preset: brawler::map::MapPresetId) -> brawler::map::MapRecipe {
    brawler::map::MapContentCatalog::embedded()
        .unwrap()
        .preset(preset)
        .unwrap()
        .recipe
        .clone()
}

/// Send one publish request from a client and return the server's decision.
fn publish(
    harness: &mut Harness,
    index: usize,
    game_id: &str,
    name: &str,
    recipe: brawler::map::MapRecipe,
) -> MapPublishDecision {
    let game = advertised_game(harness, index, game_id);
    let catalog_revision = harness.clients[index]
        .world()
        .get::<brawler_client::ClientLobbyMembership>(harness.client_entities[index])
        .unwrap()
        .catalog_revision;
    assert!(
        harness.clients[index]
            .world_mut()
            .resource_mut::<brawler_client::ClientMapPublishModel>()
            .start(
                catalog_revision,
                &game,
                name.to_string(),
                brawler::map::AuthoredMapCandidate {
                    draft_id: 5,
                    recipe,
                },
            )
    );
    let mut decision = None;
    harness.step_until(|harness| {
        decision = harness.clients[index]
            .world_mut()
            .resource_mut::<brawler_client::ClientMapPublishModel>()
            .take_outcome();
        decision.is_some()
    });
    decision.unwrap()
}

/// Send one "my published maps" request from a client (list, or withdraw first) and return the
/// server's listing or refusal.
fn published_maps(
    harness: &mut Harness,
    index: usize,
    withdraw: Option<PublishedMapId>,
) -> Result<Vec<PublishedMapSummary>, PublishedMapsRejection> {
    {
        let mut model = harness.clients[index]
            .world_mut()
            .resource_mut::<brawler_client::ClientPublishedMapsModel>();
        let started = match withdraw {
            Some(id) => model.withdraw(id),
            None => model.request_list(),
        };
        assert!(started, "one request at a time");
    }
    harness.step_until(|harness| {
        !harness.clients[index]
            .world()
            .resource::<brawler_client::ClientPublishedMapsModel>()
            .pending()
    });
    let mut model = harness.clients[index]
        .world_mut()
        .resource_mut::<brawler_client::ClientPublishedMapsModel>();
    model
        .take_rejection()
        .map_or_else(|| Ok(model.maps().unwrap().to_vec()), Err)
}

fn accepted_id(decision: MapPublishDecision) -> PublishedMapId {
    let MapPublishDecision::Accepted {
        published_map_id, ..
    } = decision
    else {
        panic!("a valid recipe is accepted: {decision:?}");
    };
    published_map_id
}

fn profile_storage(harness: &Harness) -> brawler::profiles::ProfileStorage {
    let path = harness
        .server
        .world()
        .resource::<brawler::server::LobbyState>()
        .manifest()
        .profile_database_path
        .clone();
    brawler::profiles::ProfileStorage::open(std::path::Path::new(&path)).unwrap()
}

#[test]
fn publishing_validates_with_the_authoritative_rules_and_stores_a_pending_map() {
    let mut harness = Harness::new_product_lobby(1);
    wait_for_product_lobby(&mut harness);

    // A Turf Hold recipe cannot become a Takedown community map.
    let wrong_mode = preset_recipe(brawler::map::FEATURE_YARD_TURF_HOLD_PRESET);
    assert_eq!(
        publish(&mut harness, 0, COMMUNITY_GAME, "Wrong Mode", wrong_mode),
        MapPublishDecision::Rejected(MapPublishRejection::InvalidMapRecipe)
    );
    // A recipe outside the Map Maker authoring limits is refused before storage.
    let mut oversized = preset_recipe(brawler::map::FEATURE_YARD_TAKEDOWN_PRESET);
    oversized.dimensions.width = brawler::map::AUTHORING_LIMITS.maximum_axis_cells + 1;
    assert_eq!(
        publish(&mut harness, 0, COMMUNITY_GAME, "Oversized", oversized),
        MapPublishDecision::Rejected(MapPublishRejection::InvalidMapRecipe)
    );
    // Game types the operator did not open to community maps refuse publication.
    assert_eq!(
        publish(
            &mut harness,
            0,
            "takedown-2v2",
            "Closed Pool",
            preset_recipe(brawler::map::FEATURE_YARD_TAKEDOWN_PRESET),
        ),
        MapPublishDecision::Rejected(MapPublishRejection::CommunityMapsDisabled)
    );

    let MapPublishDecision::Accepted {
        published_map_id,
        state,
    } = publish(
        &mut harness,
        0,
        COMMUNITY_GAME,
        "Yard Remix",
        preset_recipe(brawler::map::FEATURE_YARD_TAKEDOWN_PRESET),
    )
    else {
        panic!("a valid recipe is accepted");
    };
    assert_eq!(
        state,
        PublishedMapState::Pending,
        "the checked-in server pre-approves"
    );
    // The two-minute per-account cooldown starts at an accepted submission.
    assert!(matches!(
        publish(
            &mut harness,
            0,
            COMMUNITY_GAME,
            "Yard Remix Two",
            preset_recipe(brawler::map::FEATURE_YARD_TAKEDOWN_PRESET),
        ),
        MapPublishDecision::Rejected(MapPublishRejection::CoolingDown { .. })
    ));

    let mut storage = profile_storage(&harness);
    let pending = storage
        .list_published_maps(Some(brawler::profiles::PublishedMapStatus::Pending))
        .unwrap();
    assert_eq!(pending.len(), 1);
    let row = &pending[0];
    assert_eq!(row.published_map_id, published_map_id);
    assert_eq!(row.name, "Yard Remix");
    assert_eq!(row.game_type_id.as_str(), COMMUNITY_GAME);
    let stored: brawler::map::MapRecipe = postcard::from_bytes(&row.recipe).unwrap();
    assert_eq!(
        Some(stored.recipe_id),
        published_map_id.recipe_id(),
        "the server assigns the published identity; the client's draft ID never reaches storage"
    );
    assert!(
        harness
            .server
            .world()
            .resource::<brawler::server::CommunityMapRotation>()
            .maps_for(&advertised_game(&harness, 0, COMMUNITY_GAME).id)
            .is_empty(),
        "a pending map never enters rotation"
    );
}

/// Publish from client 0, approve as the operator, and wait for the lobby's rotation reload.
fn publish_and_approve(harness: &mut Harness) -> PublishedMapId {
    let MapPublishDecision::Accepted {
        published_map_id, ..
    } = publish(
        harness,
        0,
        COMMUNITY_GAME,
        "Yard Remix",
        preset_recipe(brawler::map::FEATURE_YARD_TAKEDOWN_PRESET),
    )
    else {
        panic!("a valid recipe is accepted");
    };
    // The operator approves from the CLI's storage path while the lobby runs.
    profile_storage(harness)
        .decide_published_map(
            published_map_id,
            &brawler::profiles::PublishedMapAction::Approve,
            1,
        )
        .unwrap();
    let game = advertised_game(harness, 0, COMMUNITY_GAME);
    for _ in 0..(8 * 60) {
        harness.step();
        if !harness
            .server
            .world()
            .resource::<brawler::server::CommunityMapRotation>()
            .maps_for(&game.id)
            .is_empty()
        {
            return published_map_id;
        }
    }
    panic!("the lobby re-reads approvals on its bounded interval");
}

fn open_one_match_slot(harness: &mut Harness, sequence: u64) {
    apply_lobby_control(
        harness,
        sequence,
        brawler_routing::ControlBody::LobbyCapacity(brawler_routing::LobbyCapacityBody {
            free_match_slots: 1,
        }),
    );
}

/// Clients 0 and 1 form, and hand off, the first 1v1 reservation; it plays the built-in map.
fn play_first_rotation_slot(harness: &mut Harness, index: usize) {
    open_one_match_slot(harness, 1);
    start_product_join(harness, 0, index);
    start_product_join(harness, 1, index);
    grant_pending_allocation(harness, 2);
    harness.step_until(|harness| {
        harness.clients[..2].iter().all(|client| {
            client
                .world()
                .resource::<brawler_client::RoutedClientLifecycle>()
                .accepted_grant
                .is_some()
        })
    });
    let first = harness.clients[0]
        .world()
        .resource::<brawler_client::ClientMatchLoadingModel>()
        .active()
        .cloned()
        .unwrap();
    assert!(first.map_preset_id.is_some() && first.published_map.is_none());
}

fn next_allocation_request(harness: &mut Harness) -> brawler_routing::AllocateRequestBody {
    for _ in 0..600 {
        harness.step();
        if let Some(request) = harness
            .server
            .world_mut()
            .resource_mut::<brawler::server::LobbyState>()
            .pending_allocate_request()
            .unwrap()
        {
            return request;
        }
    }
    panic!("the roster requests an allocation");
}

/// A production match worker built from the allocation's map fields admits it, and every
/// participant rebuilds exactly the published map from replication.
fn assert_match_load(
    request: &brawler_routing::AllocateRequestBody,
    published_map_id: PublishedMapId,
) {
    let participants = [
        brawler::testing::TestRoutedParticipant {
            player_id: 7,
            netcode_client_id: 8,
            peer_id: 9,
            team: 0,
        },
        brawler::testing::TestRoutedParticipant {
            player_id: 10,
            netcode_client_id: 11,
            peer_id: 12,
            team: 1,
        },
    ];
    let mut matchplay = Harness::new_routed_match_with(&participants, |manifest| {
        manifest.map_preset = request.map_preset;
        manifest.map_revision = request.map_revision;
        manifest.map_recipe_fingerprint = request.map_recipe_fingerprint;
        manifest.map_recipe.clone_from(&request.map_recipe);
    });
    for participant in participants {
        matchplay.add_routed_match_client(participant.netcode_client_id, participant.peer_id);
    }
    matchplay.step_until(|harness| {
        (0..2).all(|index| {
            let world = harness.clients[index].world_mut();
            let mut query =
                world.query_filtered::<&brawler::map::ResolvedMapIdentity, With<MapRoot>>();
            let replicated = query.iter(world).any(|identity| {
                identity.recipe_fingerprint.0 == request.map_recipe_fingerprint
                    && Some(identity.recipe_id) == published_map_id.recipe_id()
            });
            // The client re-resolves the snapshot itself before it presents the map.
            replicated
                && world.get_resource::<brawler::map::ClientMapReadiness>()
                    == Some(&brawler::map::ClientMapReadiness::Ready)
        })
    });
}

#[test]
fn an_approved_map_follows_the_built_in_rotation_and_every_client_loads_it() {
    let mut harness = Harness::new_product_lobby(4);
    wait_for_product_lobby(&mut harness);
    let published_map_id = publish_and_approve(&mut harness);
    let index = game_index(&harness, COMMUNITY_GAME);
    play_first_rotation_slot(&mut harness, index);

    // The next reservation of the same game type plays the approved community map.
    open_one_match_slot(&mut harness, 3);
    start_product_join(&mut harness, 2, index);
    start_product_join(&mut harness, 3, index);
    let request = next_allocation_request(&mut harness);
    assert_eq!(
        request.map_preset, 0,
        "a published map has no preset provenance"
    );
    let recipe: brawler::map::MapRecipe = postcard::from_bytes(&request.map_recipe).unwrap();
    assert_eq!(Some(recipe.recipe_id), published_map_id.recipe_id());
    assert_eq!(request.map_revision, recipe.revision);
    harness.step_until(|harness| {
        harness.clients[2..].iter().all(|client| {
            client
                .world()
                .resource::<brawler_client::ClientMatchLoadingModel>()
                .active()
                .is_some()
        })
    });
    let author = harness
        .server
        .world()
        .resource::<brawler::server::LobbyState>()
        .accepted_name(brawler_routing::NetcodeClientId::new(1).unwrap())
        .unwrap()
        .to_string();
    for client in &harness.clients[2..] {
        let started = client
            .world()
            .resource::<brawler_client::ClientMatchLoadingModel>()
            .active()
            .unwrap();
        let credit = started
            .published_map
            .as_ref()
            .expect("Match Loading credit");
        assert_eq!(credit.published_map_id, published_map_id);
        assert_eq!(credit.name, "Yard Remix");
        assert_eq!(credit.author_display_name, author);
        assert_eq!(started.map_preset_id, None);
        assert_eq!(
            started.map_recipe_fingerprint.0,
            request.map_recipe_fingerprint
        );
    }
    // BRL-1sgj: the credit stays with the reservation through the match handoff, where the
    // completed match copies it into the Results context.
    grant_pending_allocation(&mut harness, 4);
    harness.step_until(|harness| {
        harness.clients[2..].iter().all(|client| {
            client
                .world()
                .resource::<brawler_client::RoutedClientLifecycle>()
                .accepted_grant
                .is_some()
        })
    });
    for client in &harness.clients[2..] {
        let kept = client
            .world()
            .resource::<brawler_client::ClientMatchLoadingModel>()
            .active()
            .and_then(|active| active.published_map.as_ref())
            .map(|credit| credit.published_map_id);
        assert_eq!(kept, Some(published_map_id));
    }
    assert_match_load(&request, published_map_id);
}

#[test]
fn authors_list_and_withdraw_only_their_own_published_maps() {
    let mut harness = Harness::new_product_lobby(2);
    wait_for_product_lobby(&mut harness);
    let yard = || preset_recipe(brawler::map::FEATURE_YARD_TAKEDOWN_PRESET);
    let own = accepted_id(publish(
        &mut harness,
        0,
        COMMUNITY_GAME,
        "Yard Remix",
        yard(),
    ));
    let foreign = accepted_id(publish(
        &mut harness,
        1,
        COMMUNITY_GAME,
        "Other Yard",
        yard(),
    ));

    let listed = published_maps(&mut harness, 0, None).unwrap();
    assert_eq!(
        listed.len(),
        1,
        "only the caller's own submissions are listed"
    );
    assert_eq!(listed[0].published_map_id, own);
    assert_eq!(listed[0].name, "Yard Remix");
    assert_eq!(listed[0].game_type_id.as_str(), COMMUNITY_GAME);
    assert_eq!(listed[0].listing, PublishedMapListing::Pending);

    // Another account's map answers exactly like a missing one, and stays pending.
    assert_eq!(
        published_maps(&mut harness, 0, Some(foreign)),
        Err(PublishedMapsRejection::NotFound)
    );
    let pending = profile_storage(&harness)
        .list_published_maps(Some(brawler::profiles::PublishedMapStatus::Pending))
        .unwrap();
    assert!(pending.iter().any(|row| row.published_map_id == foreign));

    let withdrawn = published_maps(&mut harness, 0, Some(own)).unwrap();
    assert_eq!(
        withdrawn[0].listing,
        PublishedMapListing::Removed {
            reason: Some(brawler::lobby::WITHDRAWN_BY_AUTHOR_REASON.to_string()),
        }
    );
    assert_eq!(
        published_maps(&mut harness, 0, Some(own)),
        Err(PublishedMapsRejection::NotWithdrawable)
    );
    assert_eq!(
        published_maps(&mut harness, 1, None).unwrap()[0].listing,
        PublishedMapListing::Pending,
        "the other author's map is untouched"
    );
}

#[test]
fn a_withdrawn_approved_map_leaves_rotation_at_once_and_stays_out() {
    let mut harness = Harness::new_product_lobby(1);
    wait_for_product_lobby(&mut harness);
    let published_map_id = publish_and_approve(&mut harness);
    let game = advertised_game(&harness, 0, COMMUNITY_GAME);
    let listed = published_maps(&mut harness, 0, None).unwrap();
    assert_eq!(
        listed[0].listing,
        PublishedMapListing::Approved { in_rotation: true }
    );

    let withdrawn = published_maps(&mut harness, 0, Some(published_map_id)).unwrap();
    assert!(matches!(
        withdrawn[0].listing,
        PublishedMapListing::Removed { .. }
    ));
    let rotation_is_empty = |harness: &Harness| {
        harness
            .server
            .world()
            .resource::<brawler::server::CommunityMapRotation>()
            .maps_for(&game.id)
            .is_empty()
    };
    assert!(
        rotation_is_empty(&harness),
        "withdrawal leaves rotation now"
    );
    // The bounded rotation reload re-reads storage, where the row is removed like an
    // operator `remove`, so the map never returns.
    for _ in 0..(8 * 60) {
        harness.step();
        assert!(rotation_is_empty(&harness));
    }
    let row = profile_storage(&harness)
        .list_published_maps(Some(brawler::profiles::PublishedMapStatus::Removed))
        .unwrap();
    assert_eq!(row.len(), 1);
    assert_eq!(row[0].published_map_id, published_map_id);
}
