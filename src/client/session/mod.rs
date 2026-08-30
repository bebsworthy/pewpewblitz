//! Client connection, compatibility handshake, selection, roster, and shutdown lifecycle.
#[cfg(test)]
use super::On;
use super::{
    ACTION_INTERACT, Added, App, AppExit, ApplyDeferred, Authentication, AuthoritativeTick, Client,
    ClientCombatEvidenceStatus, ClientInputContext, ClientInputSettings, ClientJoinPhase,
    ClientJoinStatus, ClientLobbyFailure, ClientLobbyIdentity, ClientLobbyMembership,
    ClientMatchLoadingModel, ClientMatchResultContext, ClientMatchResultState, ClientNetworkConfig,
    ClientPlayableGate, ClientProfileIdentityState, ClientSettingsUiSet, ClientShutdown, Commands,
    Component, Connect, Connected, Connecting, Controlled, ControllerDemoGamepad, Disconnect,
    Disconnected, Duration, Entity, FallbackErrorHandler, Fighter, FixedPreUpdate, FixedUpdate,
    Gamepad, GamepadAxis, GamepadButton, Has, HeadlessAutomation, InputDeviceActivity,
    InputSettingsSelection, InputSystems, IntoScheduleConfigs, Last, Link, LinkMtu, LiveInputTrace,
    LobbyHello, LobbyJoinOutcome, LobbyServerIdentity, LocalAddr, MatchCommand,
    MatchCommandOutcome, MatchCommandRequest, MatchCommandState, MatchHello, MatchJoinOutcome,
    MatchJoinRejection, MatchLoadingClientAction, MatchLoadingClientMessage,
    MatchLoadingServerMessage, MatchLoadingStatus, MatchParticipant, MatchPhase, MatchRoot,
    MatchRouteGrant, MatchState, MessageReceiver, MessageSender, MessageWriter, Messages, Name,
    NetcodeClient, NetcodeConfig, NetworkEntityId, NetworkTransport, PeerAddr, PendingLocalActions,
    PingManager, PlayerId, Plugin, Position, ProtocolFingerprint, Query, ROUTED_LINK_MTU, Real,
    Remote, ReplicationReceiver, Res, ResMut, Resource, Result, RosterLogState,
    RoutedClientLifecycle, RoutedClientPhase, RoutedClientSession, RoutedClientSessionKind,
    RoutedUdpIo, RoutedUdpPlugin, RunFixedMainLoop, RunFixedMainLoopSystems, RuntimeLobbyTarget,
    SelectedGameType, SessionChannel, Startup, String, SystemSet, Time, ToOwned, ToString, UdpIo,
    Unlink, UnlinkReason, Unlinked, Update, VERSION, Vec, Vec2, With, Without,
    add_controlled_input_marker, adjust_input_settings_from_pause_keys,
    advance_headless_automation, apply_headless_input, connection_persistence, default, error,
    flow, format, hud, info, resolve_targeted_ultimate_input, sample_local_input,
    trace_client_interpolation_history, trace_client_interpolation_sync,
    update_input_settings_overlay, warn, write_client_input,
};

mod admission;
mod automation;
mod connection;
mod match_commands;
mod observation;
mod routing;

use admission::{
    process_join_outcome, process_lobby_server_identity, record_client_failure, send_client_hello,
};
use automation::{spawn_controller_demo_gamepad, update_controller_demo_gamepad};
pub(in crate::client) use connection::{ProductLobbyAttempt, spawn_product_lobby_connection};
use connection::{
    connect_spawned_clients, finish_spawned_client_connect, spawn_client_connection,
    spawn_client_entity,
};
use match_commands::{
    MatchLoadingCommandState, drive_match_loading_check_in, finish_product_match_smoke,
    process_match_command_outcomes, send_match_command,
};
use observation::{enforce_client_timeout, log_replicated_roster};
use routing::{
    advance_routed_transition, disconnect_rejected_client, observe_routed_transition,
    process_match_route_grant,
};

#[cfg(test)]
pub(super) use admission::join_rejection_category;
#[cfg(test)]
pub(super) use match_commands::{
    automatic_match_command_enabled, should_rearm_headless_match_command,
};
pub(super) use observation::{
    finish_client_shutdown, forward_app_exit_to_client_disconnect, observe_client_lifecycle,
};
#[cfg(test)]
use routing::owns_automatic_routed_recovery;
pub(super) use routing::{
    drive_routed_transition, enforce_routed_timeout, observe_completed_match,
    observe_fresh_lobby_return,
};

/// Installs client connection, compatibility-handshake, session, input, and status systems.
pub struct ClientNetworkPlugin;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ClientSessionSet {
    MaterializeConnection,
    Handshake,
    Transition,
    EnforceTransition,
    MatchCommands,
    Observe,
    EnforceSession,
}

fn configure_client_session_schedule(app: &mut App) {
    app.configure_sets(
        Update,
        (
            ClientSessionSet::MaterializeConnection,
            ClientSessionSet::Handshake,
            ClientSessionSet::Transition,
            ClientSessionSet::EnforceTransition,
            ClientSessionSet::MatchCommands,
            ClientSessionSet::Observe,
            ClientSessionSet::EnforceSession,
        )
            .chain(),
    );
}

/// Deadline for an intentional routed session teardown. A normal teardown reaches `Unlinked`
/// quickly; if lifecycle markers never arrive, this bounded fallback lets the next generation
/// recover instead of leaving the client permanently in an awaiting phase.
#[derive(Component, Clone, Copy, Debug)]
pub(super) struct RoutedTransitionDeadline(pub(super) Duration);

/// Defers `Connect` until the freshly spawned client entity and all transport components are
/// materialized in the `World`. Product-shell connections are created from an `Update` system;
/// triggering `Connect` in that same deferred spawn boundary can run transport observers before
/// their query can see the new `RoutedUdpIo`, leaving the first attempt without a bound socket.
#[derive(Component)]
pub(super) struct PendingClientConnect;

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        if app.world().resource::<ClientNetworkConfig>().transport == NetworkTransport::RoutedUdp {
            app.add_plugins(RoutedUdpPlugin);
        }
        configure_client_settings_ui(app);
        configure_client_session_schedule(app);
        install_session_state(app);
        configure_input_and_automation(app);
        configure_network_session_systems(app);
        configure_terminal_session_systems(app);
        app.add_observer(add_controlled_input_marker);
    }
}

fn install_session_state(app: &mut App) {
    app.insert_resource(FallbackErrorHandler(error))
        .init_resource::<RosterLogState>()
        .init_resource::<ClientShutdown>()
        .init_resource::<PendingLocalActions>()
        .init_resource::<LiveInputTrace>()
        .init_resource::<HeadlessAutomation>()
        .init_resource::<InputDeviceActivity>()
        .init_resource::<ClientInputContext>()
        .init_resource::<ClientPlayableGate>()
        .init_resource::<MatchCommandState>()
        .init_resource::<MatchLoadingCommandState>()
        .init_resource::<ClientInputSettings>()
        .init_resource::<InputSettingsSelection>()
        .init_resource::<RoutedClientLifecycle>()
        .init_resource::<ClientMatchResultState>()
        .init_resource::<ClientProfileIdentityState>()
        .init_resource::<crate::diagnostics::ProcessExitClassification>();
}

fn configure_input_and_automation(app: &mut App) {
    app.add_systems(
        Startup,
        (spawn_client_connection, spawn_controller_demo_gamepad).chain(),
    )
    .add_systems(
        RunFixedMainLoop,
        (
            update_controller_demo_gamepad,
            sample_local_input,
            resolve_targeted_ultimate_input,
            apply_headless_input,
        )
            .chain()
            .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
    )
    .add_systems(
        FixedPreUpdate,
        write_client_input.in_set(InputSystems::WriteClientInputs),
    )
    .add_systems(
        FixedUpdate,
        advance_headless_automation.in_set(crate::gameplay::GameplaySet::Finalize),
    );
}

fn configure_network_session_systems(app: &mut App) {
    app.add_systems(
        Update,
        (connect_spawned_clients, finish_spawned_client_connect)
            .chain()
            .in_set(ClientSessionSet::MaterializeConnection),
    )
    .add_systems(
        Update,
        (
            process_lobby_server_identity,
            ApplyDeferred,
            send_client_hello,
            process_join_outcome,
            observe_fresh_lobby_return,
            process_match_route_grant,
            observe_completed_match,
        )
            .chain()
            .in_set(ClientSessionSet::Handshake),
    )
    .add_systems(
        Update,
        (
            drive_routed_transition,
            observe_routed_transition,
            advance_routed_transition,
        )
            .chain()
            .in_set(ClientSessionSet::Transition),
    )
    .add_systems(
        Update,
        enforce_routed_timeout.in_set(ClientSessionSet::EnforceTransition),
    )
    .add_systems(
        Update,
        (
            drive_match_loading_check_in,
            process_match_command_outcomes,
            send_match_command,
            finish_product_match_smoke,
            disconnect_rejected_client,
        )
            .chain()
            .in_set(ClientSessionSet::MatchCommands),
    )
    .add_systems(
        Update,
        (
            (observe_client_lifecycle, log_replicated_roster).chain(),
            (
                trace_client_interpolation_sync,
                trace_client_interpolation_history,
            )
                .chain(),
        )
            .in_set(ClientSessionSet::Observe),
    )
    .add_systems(
        Update,
        enforce_client_timeout.in_set(ClientSessionSet::EnforceSession),
    );
}

fn configure_terminal_session_systems(app: &mut App) {
    app.add_systems(
        Last,
        (
            forward_app_exit_to_client_disconnect,
            finish_client_shutdown,
        )
            .chain()
            // Order before the terminal observation set so closeout observations and the final
            // report see post-shutdown counts and the re-emitted exit.
            .before(crate::diagnostics::TerminalObservationSet),
    );
}

fn configure_client_settings_ui(app: &mut App) {
    app.configure_sets(
        Update,
        (
            ClientSettingsUiSet::Capture,
            ClientSettingsUiSet::Shell,
            ClientSettingsUiSet::Present,
        )
            .chain(),
    )
    .add_systems(
        Update,
        adjust_input_settings_from_pause_keys.in_set(ClientSettingsUiSet::Capture),
    )
    .add_systems(
        Update,
        update_input_settings_overlay.in_set(ClientSettingsUiSet::Present),
    );
}

#[cfg(test)]
mod tests;
