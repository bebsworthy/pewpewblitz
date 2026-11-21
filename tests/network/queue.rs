use super::*;

#[derive(Resource, Default)]
struct CapturedQueueClientMessages(Vec<brawler::lobby::QueueClientMessage>);

fn capture_queue_client_messages(
    mut captured: ResMut<CapturedQueueClientMessages>,
    mut receivers: Query<&mut MessageReceiver<brawler::lobby::QueueClientMessage>>,
) {
    for mut receiver in &mut receivers {
        captured.0.extend(receiver.receive());
    }
}

#[test]
fn unified_queue_envelope_preserves_ack_then_command_order_over_crossbeam() {
    let mut harness = Harness::new(1);
    harness
        .server
        .init_resource::<CapturedQueueClientMessages>()
        .add_systems(bevy::prelude::Update, capture_queue_client_messages);
    harness.step_until(|harness| harness.active_server_sessions() == 1);

    let client_entity = harness.client_entities[0];
    let mut sender = harness.clients[0]
        .world_mut()
        .get_mut::<MessageSender<brawler::lobby::QueueClientMessage>>(client_entity)
        .expect("client queue sender is installed only in the allowed direction");
    let ack = brawler::lobby::QueueClientMessage::OutcomeAck {
        request_id: brawler::lobby::QueueRequestId::new(1).unwrap(),
    };
    let command = brawler::lobby::QueueClientMessage::Command {
        request_id: brawler::lobby::QueueRequestId::new(2).unwrap(),
        command: brawler::lobby::QueueCommand::Cancel(brawler::lobby::QueueCancelCommand {
            ticket_id: brawler::lobby::QueueTicketId::new(9).unwrap(),
        }),
    };
    sender.send::<SessionChannel>(ack.clone());
    sender.send::<SessionChannel>(command.clone());

    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CapturedQueueClientMessages>()
            .0
            .len()
            == 2
    });

    assert_eq!(
        harness
            .server
            .world()
            .resource::<CapturedQueueClientMessages>()
            .0,
        vec![ack, command]
    );
}

#[test]
fn queue_command_before_lobby_welcome_cannot_create_membership() {
    let mut harness = Harness::new_product_lobby(1);
    let client_entity = harness.client_entities[0];
    let mut sender = harness.clients[0]
        .world_mut()
        .get_mut::<MessageSender<brawler::lobby::QueueClientMessage>>(client_entity)
        .expect("client queue sender is installed");
    sender.send::<SessionChannel>(brawler::lobby::QueueClientMessage::Command {
        request_id: brawler::lobby::QueueRequestId::new(1).unwrap(),
        command: brawler::lobby::QueueCommand::Join(brawler::lobby::QueueJoinCommand {
            catalog_revision: brawler::lobby::CatalogRevision([1; 32]),
            game_type_id: brawler::lobby::GameTypeId::new("takedown-2v2").unwrap(),
            game_type_configuration_revision: 1,
            profile_revision: brawler::profiles::ProfileRevision::INITIAL,
        }),
    });

    for _ in 0..30 {
        harness.step();
    }

    assert_eq!(
        harness
            .server
            .world()
            .resource::<brawler::server::QueueState>()
            .ticket_count(),
        0
    );
    assert!(
        harness.clients[0]
            .world()
            .get::<brawler_client::ClientLobbyMembership>(client_entity)
            .is_none()
    );
}

pub(super) fn wait_for_product_lobby(harness: &mut Harness) {
    harness.step_until(|harness| {
        harness
            .clients
            .iter()
            .zip(&harness.client_entities)
            .all(|(client, entity)| {
                client
                    .world()
                    .get::<brawler_client::ClientLobbyMembership>(*entity)
                    .is_some()
                    && client
                        .world()
                        .resource::<brawler_client::ClientQueueModel>()
                        .snapshot()
                        .is_some()
                    && client
                        .world()
                        .resource::<brawler_client::ClientProfileModel>()
                        .snapshot()
                        .is_some()
            })
    });
}

/// The selected character pick and the profile revision a queue or Practice request names.
pub(super) fn selected_pick(
    harness: &Harness,
    client_index: usize,
) -> (
    brawler::characters::CharacterChoice,
    brawler::profiles::ProfileRevision,
) {
    let profile = harness.clients[client_index]
        .world()
        .resource::<brawler_client::ClientProfileModel>()
        .snapshot()
        .expect("product lobby has delivered the authoritative profile");
    (profile.selection(), profile.revision)
}

pub(super) fn start_product_join(harness: &mut Harness, client_index: usize, game_index: usize) {
    let entity = harness.client_entities[client_index];
    let lobby = harness.clients[client_index]
        .world()
        .get::<brawler_client::ClientLobbyMembership>(entity)
        .unwrap()
        .clone();
    let game = lobby.game_types[game_index].clone();
    let selection = brawler_client::SelectedGameType {
        catalog_revision: Some(lobby.catalog_revision),
        game_type_id: Some(game.id),
        configuration_revision: Some(game.configuration_revision),
    };
    let (_, profile_revision) = selected_pick(harness, client_index);
    assert!(
        harness.clients[client_index]
            .world_mut()
            .resource_mut::<brawler_client::ClientQueueModel>()
            .start_join(&selection, profile_revision, std::time::Duration::ZERO)
    );
}

#[test]
fn practice_request_bypasses_queue_and_starts_one_human_three_v_three_reservation() {
    let mut harness = Harness::new_product_lobby(1);
    wait_for_product_lobby(&mut harness);
    let capacity = brawler_routing::ControlFrame::from_raw_sequence(
        1,
        brawler_routing::ProcessId::new(2).unwrap(),
        brawler_routing::WorkerId::new(3).unwrap(),
        brawler_routing::ControlBody::LobbyCapacity(brawler_routing::LobbyCapacityBody {
            free_match_slots: 1,
        }),
    )
    .unwrap();
    harness
        .server
        .world_mut()
        .resource_mut::<brawler::server::LobbyState>()
        .apply_control_frame(capacity)
        .unwrap();
    let client_entity = harness.client_entities[0];
    let lobby = harness.clients[0]
        .world()
        .get::<brawler_client::ClientLobbyMembership>(client_entity)
        .unwrap()
        .clone();
    let game = lobby
        .game_types
        .iter()
        .find(|game| game.id.as_str() == "turf-hold-3v3")
        .unwrap();
    let selected = brawler_client::SelectedGameType {
        catalog_revision: Some(lobby.catalog_revision),
        game_type_id: Some(game.id.clone()),
        configuration_revision: Some(game.configuration_revision),
    };
    let (_, profile_revision) = selected_pick(&harness, 0);
    assert!(
        harness.clients[0]
            .world_mut()
            .resource_mut::<brawler_client::ClientPracticeModel>()
            .start(&selected, profile_revision)
    );

    harness.step_until(|harness| {
        harness.clients[0]
            .world()
            .resource::<brawler_client::ClientMatchLoadingModel>()
            .active()
            .is_some()
    });
    let started = harness.clients[0]
        .world()
        .resource::<brawler_client::ClientMatchLoadingModel>()
        .active()
        .unwrap();
    assert_eq!(started.ticket_id, None);
    assert_eq!((started.team_count, started.players_per_team), (2, 3));
    assert_eq!(
        harness
            .server
            .world()
            .resource::<brawler::server::QueueState>()
            .ticket_count(),
        0
    );
}

#[test]
fn product_lobby_two_client_fifo_cancel_and_aggregate_convergence() {
    let mut harness = Harness::new_product_lobby(2);
    wait_for_product_lobby(&mut harness);
    start_product_join(&mut harness, 0, 0);
    start_product_join(&mut harness, 1, 0);
    harness.step_until(|harness| {
        harness.clients.iter().all(|client| {
            let queue = client
                .world()
                .resource::<brawler_client::ClientQueueModel>();
            queue.membership().is_some()
                && queue
                    .snapshot()
                    .is_some_and(|snapshot| snapshot.pools[0].queued == 2)
        })
    });

    let queue = harness
        .server
        .world()
        .resource::<brawler::server::QueueState>();
    let first = queue
        .ticket_for_client(brawler_routing::NetcodeClientId::new(1).unwrap())
        .unwrap();
    let second = queue
        .ticket_for_client(brawler_routing::NetcodeClientId::new(2).unwrap())
        .unwrap();
    assert_eq!(
        first.admission_order < second.admission_order,
        first.player_id < second.player_id,
        "same-update admissions use the documented stable PlayerId tie-break",
    );
    assert_ne!(first.ticket_id, second.ticket_id);

    assert!(
        harness.clients[0]
            .world_mut()
            .resource_mut::<brawler_client::ClientQueueModel>()
            .start_cancel(std::time::Duration::from_secs(1))
    );
    harness.step_until(|harness| {
        let first = harness.clients[0]
            .world()
            .resource::<brawler_client::ClientQueueModel>();
        let second = harness.clients[1]
            .world()
            .resource::<brawler_client::ClientQueueModel>();
        first.membership().is_none()
            && first
                .snapshot()
                .is_some_and(|snapshot| snapshot.pools[0].queued == 1)
            && second.membership().is_some()
            && second
                .snapshot()
                .is_some_and(|snapshot| snapshot.pools[0].queued == 1)
    });
    assert_eq!(
        harness
            .server
            .world()
            .resource::<brawler::server::QueueState>()
            .ticket_count(),
        1
    );
}

#[test]
fn product_lobby_cross_pool_snapshots_are_shared_but_membership_is_private() {
    let mut harness = Harness::new_product_lobby(2);
    wait_for_product_lobby(&mut harness);
    start_product_join(&mut harness, 0, 0);
    start_product_join(&mut harness, 1, 1);
    harness.step_until(|harness| {
        harness.clients.iter().all(|client| {
            let queue = client
                .world()
                .resource::<brawler_client::ClientQueueModel>();
            queue.membership().is_some()
                && queue.snapshot().is_some_and(|snapshot| {
                    snapshot.pools[0].queued == 1 && snapshot.pools[1].queued == 1
                })
        })
    });
    let first = harness.clients[0]
        .world()
        .resource::<brawler_client::ClientQueueModel>();
    let second = harness.clients[1]
        .world()
        .resource::<brawler_client::ClientQueueModel>();
    assert_ne!(
        first.membership().unwrap().game_type_id,
        second.membership().unwrap().game_type_id
    );
    assert_eq!(first.snapshot(), second.snapshot());

    let disconnected = harness.client_entities[0];
    harness.clients[0].world_mut().trigger(Disconnect {
        entity: disconnected,
    });
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<brawler::server::QueueState>()
            .ticket_count()
            == 1
    });
    assert!(
        harness
            .server
            .world()
            .resource::<brawler::server::QueueState>()
            .ticket_for_client(brawler_routing::NetcodeClientId::new(2).unwrap())
            .is_some()
    );
}

/// Feed one supervisor control body to the product lobby, as its control stream would.
pub(super) fn apply_lobby_control(
    harness: &mut Harness,
    sequence: u64,
    body: brawler_routing::ControlBody,
) {
    let frame = brawler_routing::ControlFrame::from_raw_sequence(
        sequence,
        brawler_routing::ProcessId::new(2).unwrap(),
        brawler_routing::WorkerId::new(3).unwrap(),
        body,
    )
    .unwrap();
    harness
        .server
        .world_mut()
        .resource_mut::<brawler::server::LobbyState>()
        .apply_control_frame(frame)
        .unwrap();
}

/// Wait for the lobby's allocation request and grant every participant a route, as the
/// supervisor would after starting the match worker.
pub(super) fn grant_pending_allocation(harness: &mut Harness, sequence: u64) {
    let mut request = None;
    for _ in 0..600 {
        harness.step();
        request = harness
            .server
            .world_mut()
            .resource_mut::<brawler::server::LobbyState>()
            .pending_allocate_request()
            .unwrap();
        if request.is_some() {
            break;
        }
    }
    let request = request.expect("the reservation requests an allocation");
    let grants = request
        .participants
        .iter()
        .zip(1_u8..)
        .map(|(participant, index)| brawler_routing::AllocationGrant {
            lobby_session_id: participant.lobby_session_id,
            route_id: brawler_routing::RouteId::new(500 + u128::from(index)).unwrap(),
            peer_id: brawler_routing::PeerId::new(600 + u128::from(index)).unwrap(),
            capability: brawler_routing::Capability::from_bytes([index; 32]).unwrap(),
            activation_expiry_unix_ms: 1,
            route_expiry_unix_ms: 2,
        })
        .collect();
    apply_lobby_control(
        harness,
        sequence,
        brawler_routing::ControlBody::AllocationGranted(brawler_routing::AllocationGrantedBody {
            request_id: request.request_id,
            allocation_id: brawler_routing::AllocationId::new(70).unwrap(),
            match_id: brawler_routing::MatchId::new(71).unwrap(),
            worker_id: brawler_routing::WorkerId::new(72).unwrap(),
            grants,
        }),
    );
}

/// BRL-k0tr: once every participant has its `BeginMatchConnect`, each client closes its lobby
/// session on purpose. A participant whose lobby session closes first must not cancel the running
/// activation or send `Removed` to a participant whose lobby session closes a few frames later
/// (a phone on Wi-Fi), which pulled that client's flow back to the Dashboard under its match.
#[test]
fn a_handed_off_lobby_session_closing_first_does_not_remove_the_other_participant() {
    let mut harness = Harness::new_product_lobby(2);
    wait_for_product_lobby(&mut harness);
    apply_lobby_control(
        &mut harness,
        1,
        brawler_routing::ControlBody::LobbyCapacity(brawler_routing::LobbyCapacityBody {
            free_match_slots: 1,
        }),
    );
    let game_index = harness.clients[0]
        .world()
        .get::<brawler_client::ClientLobbyMembership>(harness.client_entities[0])
        .unwrap()
        .game_types
        .iter()
        .position(|game| game.id.as_str() == "takedown-1v1")
        .expect("the 1v1 Takedown game is advertised");
    start_product_join(&mut harness, 0, game_index);
    start_product_join(&mut harness, 1, game_index);
    grant_pending_allocation(&mut harness, 2);

    // Both clients accept their BeginMatchConnect grant.
    harness.step_until(|harness| {
        harness.clients.iter().all(|client| {
            client
                .world()
                .resource::<brawler_client::RoutedClientLifecycle>()
                .accepted_grant
                .is_some()
        })
    });

    // The first participant's lobby session closes at its handoff; the second one stays open
    // for a while longer, as a slower phone's does.
    let first = harness.client_entities[0];
    harness.clients[0]
        .world_mut()
        .trigger(Disconnect { entity: first });
    harness.step_until(|harness| {
        harness.clients[0]
            .world()
            .get::<Disconnected>(first)
            .is_some()
    });
    for _ in 0..30 {
        harness.step();
    }

    assert_eq!(
        brawler::testing::lobby_activation_cancel_count(harness.server.world()),
        0,
        "a completed handoff never cancels the activation of the match it started"
    );
    let loading = harness.clients[1]
        .world()
        .resource::<brawler_client::ClientMatchLoadingModel>();
    assert!(
        loading.active().is_some(),
        "the second participant is still on its way to the match"
    );
    assert_eq!(
        loading.phase(),
        Some(brawler::lobby::MatchLoadingPhase::Connecting)
    );
}
