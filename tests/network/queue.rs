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
            game_type_id: brawler::lobby::GameTypeId::new("wipeout-2v2").unwrap(),
            game_type_configuration_revision: 1,
            brawler_id: brawler::profiles::SavedBrawlerId::new(1).unwrap(),
            brawler_revision: brawler::profiles::ProfileRevision::INITIAL,
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
            .get::<brawler::client::ClientLobbyMembership>(client_entity)
            .is_none()
    );
}

fn wait_for_product_lobby(harness: &mut Harness) {
    harness.step_until(|harness| {
        harness
            .clients
            .iter()
            .zip(&harness.client_entities)
            .all(|(client, entity)| {
                client
                    .world()
                    .get::<brawler::client::ClientLobbyMembership>(*entity)
                    .is_some()
                    && client
                        .world()
                        .resource::<brawler::client::ClientQueueModel>()
                        .snapshot()
                        .is_some()
                    && client
                        .world()
                        .resource::<brawler::client::ClientProfileModel>()
                        .snapshot()
                        .is_some_and(|profile| {
                            profile.selected_brawler_id.is_some() && !profile.brawlers.is_empty()
                        })
            })
    });
}

fn selected_brawler(
    harness: &Harness,
    client_index: usize,
) -> (
    brawler::profiles::SavedBrawlerId,
    brawler::profiles::ProfileRevision,
) {
    let profile = harness.clients[client_index]
        .world()
        .resource::<brawler::client::ClientProfileModel>()
        .snapshot()
        .expect("product lobby has delivered the authoritative profile");
    let selected_id = profile
        .selected_brawler_id
        .expect("headless product client has created and selected a default brawler");
    let selected = profile
        .brawlers
        .iter()
        .find(|brawler| brawler.id == selected_id)
        .expect("selected brawler belongs to the profile");
    (selected.id, selected.revision)
}

fn start_product_join(harness: &mut Harness, client_index: usize, game_index: usize) {
    let entity = harness.client_entities[client_index];
    let lobby = harness.clients[client_index]
        .world()
        .get::<brawler::client::ClientLobbyMembership>(entity)
        .unwrap()
        .clone();
    let game = lobby.game_types[game_index].clone();
    let selection = brawler::client::SelectedGameType {
        catalog_revision: Some(lobby.catalog_revision),
        game_type_id: Some(game.id),
        configuration_revision: Some(game.configuration_revision),
    };
    let (brawler_id, brawler_revision) = selected_brawler(harness, client_index);
    assert!(
        harness.clients[client_index]
            .world_mut()
            .resource_mut::<brawler::client::ClientQueueModel>()
            .start_join(
                &selection,
                brawler_id,
                brawler_revision,
                std::time::Duration::ZERO,
            )
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
        .get::<brawler::client::ClientLobbyMembership>(client_entity)
        .unwrap()
        .clone();
    let game = lobby
        .game_types
        .iter()
        .find(|game| game.id.as_str() == "hot-zone-3v3")
        .unwrap();
    let selected = brawler::client::SelectedGameType {
        catalog_revision: Some(lobby.catalog_revision),
        game_type_id: Some(game.id.clone()),
        configuration_revision: Some(game.configuration_revision),
    };
    let (brawler_id, brawler_revision) = selected_brawler(&harness, 0);
    assert!(
        harness.clients[0]
            .world_mut()
            .resource_mut::<brawler::client::ClientPracticeModel>()
            .start(&selected, brawler_id, brawler_revision,)
    );

    harness.step_until(|harness| {
        harness.clients[0]
            .world()
            .resource::<brawler::client::ClientMatchLoadingModel>()
            .active()
            .is_some()
    });
    let started = harness.clients[0]
        .world()
        .resource::<brawler::client::ClientMatchLoadingModel>()
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
                .resource::<brawler::client::ClientQueueModel>();
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
            .resource_mut::<brawler::client::ClientQueueModel>()
            .start_cancel(std::time::Duration::from_secs(1))
    );
    harness.step_until(|harness| {
        let first = harness.clients[0]
            .world()
            .resource::<brawler::client::ClientQueueModel>();
        let second = harness.clients[1]
            .world()
            .resource::<brawler::client::ClientQueueModel>();
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
                .resource::<brawler::client::ClientQueueModel>();
            queue.membership().is_some()
                && queue.snapshot().is_some_and(|snapshot| {
                    snapshot.pools[0].queued == 1 && snapshot.pools[1].queued == 1
                })
        })
    });
    let first = harness.clients[0]
        .world()
        .resource::<brawler::client::ClientQueueModel>();
    let second = harness.clients[1]
        .world()
        .resource::<brawler::client::ClientQueueModel>();
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
