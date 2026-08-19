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
