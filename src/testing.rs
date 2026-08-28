//! Narrow integration-test support surface for the combined client/server feature graph.

use crate::{
    config::{ClientNetworkConfig, ServerNetworkConfig},
    protocol::FighterInput,
};
use bevy::prelude::{Entity, World};
use lightyear::{
    input::{
        input_buffer::InputBuffer,
        input_message::{ActionStateSequence, InputMessage, InputTarget, PerTargetData},
    },
    prelude::{
        MessageSender, Tick,
        input::{
            InputChannel,
            native::{ActionState, NativeStateSequence},
        },
    },
};

pub use crate::combat::testing::{
    CaptureCombatCues, TestDummy, TestDummyFixture, TestDummyResetDeadline,
};
pub use crate::logging::{
    ExpectedLateInputDiagnostics, capture_expected_late_input_diagnostics,
    install_network_test_logger,
};

/// Native-input message shape used only by authority-forgery integration tests.
pub type TestNativeInputMessage = InputMessage<NativeStateSequence<FighterInput>>;

/// Send one deliberately forged native input through the real Lightyear input channel.
pub fn send_forged_native_input(
    sender: &mut MessageSender<TestNativeInputMessage>,
    target: InputTarget,
    end_tick: u32,
    input: FighterInput,
) {
    let mut buffer = InputBuffer::default();
    buffer.set(Tick(end_tick), ActionState(input));
    let states = NativeStateSequence::build_from_input_buffer(&buffer, 1, Tick(end_tick))
        .expect("forged test input sequence should contain one state");
    let mut message = InputMessage::new(Tick(end_tick));
    message.inputs.push(PerTargetData { target, states });
    sender.send::<InputChannel>(message);
}

/// Spawn one deterministic Crossbeam client endpoint for a separate-App test.
#[allow(
    clippy::needless_pass_by_value,
    reason = "the test endpoint consumes its owned configuration and Crossbeam IO"
)]
pub fn spawn_crossbeam_client(
    world: &mut World,
    config: ClientNetworkConfig,
    io: lightyear::crossbeam::CrossbeamIo,
) -> Entity {
    crate::client::spawn_crossbeam_client(world, config, io)
}

/// Spawn one deterministic Crossbeam server endpoint for a separate-App test.
pub fn spawn_crossbeam_server(world: &mut World, config: &ServerNetworkConfig) -> Entity {
    crate::server::spawn_crossbeam_server(world, config)
}

/// Attach one deterministic Crossbeam link to a test server endpoint.
pub fn spawn_crossbeam_link(
    world: &mut World,
    server: Entity,
    io: lightyear::crossbeam::CrossbeamIo,
) -> Entity {
    crate::server::spawn_crossbeam_link(world, server, io)
}

/// Request graceful Lightyear shutdown for a test server endpoint.
pub fn request_server_stop(world: &mut World, server: Entity) {
    crate::server::request_stop(world, server);
}
