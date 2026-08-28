use super::{
    AppExit, Client, ClientCombatEvidenceStatus, ClientJoinPhase, ClientJoinStatus,
    ClientNetworkConfig, ClientShutdown, Commands, Connecting, Disconnect, Disconnected, Entity,
    Fighter, Has, HeadlessAutomation, MessageWriter, Messages, NetworkEntityId, NetworkTransport,
    PendingClientConnect, PlayerId, Query, Real, Remote, Res, ResMut, RosterLogState,
    RoutedClientLifecycle, RoutedClientPhase, Time, ToString, Vec, With, Without, error, format,
    info, record_client_failure, warn,
};

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(in crate::client) fn observe_client_lifecycle(
    config: Res<ClientNetworkConfig>,
    routed: Res<RoutedClientLifecycle>,
    mut query: Query<
        (
            &mut ClientJoinStatus,
            Option<&Disconnected>,
            Has<Connecting>,
        ),
        (With<Client>, Without<PendingClientConnect>),
    >,
    diagnostics: Option<Res<crate::diagnostics::ProcessDiagnosticsSettings>>,
    mut classification: ResMut<crate::diagnostics::ProcessExitClassification>,
    mut app_exit: MessageWriter<AppExit>,
) {
    // Routed sessions recover through their explicit Unlinked/deferred-teardown state machine;
    // treating the deliberate lobby->match disconnect as a terminal direct-UDP failure would
    // incorrectly exit the client.
    if config.transport == NetworkTransport::RoutedUdp
        && matches!(
            routed.phase,
            RoutedClientPhase::AwaitingLobbyUnlink
                | RoutedClientPhase::AwaitingLobbyRetryUnlink
                | RoutedClientPhase::AwaitingMatchUnlink
        )
    {
        return;
    }
    for (mut status, disconnected, connecting) in query.iter_mut() {
        if disconnected.is_some()
            && !connecting
            && !matches!(
                status.phase,
                ClientJoinPhase::Rejected(_) | ClientJoinPhase::Disconnected
            )
        {
            let reason = disconnected.map(|disconnected| disconnected.reason.to_string());
            warn!(?reason, "brawler client disconnected");
            record_client_failure(
                diagnostics.as_ref(),
                &mut classification,
                crate::diagnostics::FailureCategory::ShutdownIncomplete,
                format!("client disconnected: {reason:?}"),
            );
            status.phase = ClientJoinPhase::Disconnected;
            if !config.presents_product_shell() {
                app_exit.write(AppExit::error());
            }
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn log_replicated_roster(
    config: Res<ClientNetworkConfig>,
    routed: Res<RoutedClientLifecycle>,
    automation: Res<HeadlessAutomation>,
    combat_evidence: Option<Res<ClientCombatEvidenceStatus>>,
    mut roster_state: ResMut<RosterLogState>,
    status_query: Query<&ClientJoinStatus, With<Client>>,
    roster: Query<(&PlayerId, &NetworkEntityId), (With<Remote>, With<Fighter>)>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if config.transport == NetworkTransport::RoutedUdp && routed.phase != RoutedClientPhase::Match {
        return;
    }
    if config.exit_after_lobby_return {
        return;
    }
    let mut current: Vec<_> = roster
        .iter()
        .map(|(player, entity)| (*player, *entity))
        .collect();
    current.sort_by_key(|(player, entity)| (player.0, entity.0));
    if current != roster_state.0 {
        info!(
            roster = ?current.iter().map(|(player, entity)| (player.0, entity.0)).collect::<Vec<_>>(),
            "brawler replicated roster changed"
        );
        roster_state.0.clone_from(&current);
    }
    if let Some(target) = config.exit_after_roster
        && status_query
            .iter()
            .any(|status| matches!(status.phase, ClientJoinPhase::Active { .. }))
        && current.len() >= target
        && automation
            .simulation_ticks
            .is_none_or(|limit| automation.elapsed_ticks >= limit)
        && combat_evidence.is_none_or(|status| status.permits_exit())
    {
        app_exit.write(AppExit::Success);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn enforce_client_timeout(
    config: Res<ClientNetworkConfig>,
    time: Res<Time<Real>>,
    status_query: Query<&ClientJoinStatus, With<Client>>,
    roster: Query<(), (With<Remote>, With<Fighter>)>,
    diagnostics: Option<Res<crate::diagnostics::ProcessDiagnosticsSettings>>,
    mut classification: ResMut<crate::diagnostics::ProcessExitClassification>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if config.transport == NetworkTransport::RoutedUdp {
        return;
    }
    let now = time.elapsed();
    let roster_count = roster.iter().count();
    for status in status_query.iter() {
        let connection_timed_out = matches!(
            status.phase,
            ClientJoinPhase::Connecting | ClientJoinPhase::AwaitingOutcome
        ) && now
            >= status.started_at.saturating_add(config.connect_timeout);
        let roster_timed_out = config.exit_after_roster.is_some_and(|target| {
            matches!(status.phase, ClientJoinPhase::Active { .. })
                && roster_count < target
                && now >= status.started_at.saturating_add(config.connect_timeout)
        });
        if connection_timed_out || roster_timed_out {
            error!("brawler client connection timed out");
            record_client_failure(
                diagnostics.as_ref(),
                &mut classification,
                crate::diagnostics::FailureCategory::Timeout,
                "client connection timed out".to_string(),
            );
            app_exit.write(AppExit::error());
        }
    }
}

pub(in crate::client) fn forward_app_exit_to_client_disconnect(
    mut app_exits: ResMut<Messages<AppExit>>,
    mut shutdown: ResMut<ClientShutdown>,
    mut commands: Commands,
    query: Query<(Entity, Option<&Disconnected>), With<Client>>,
) {
    if shutdown.requested_exit.is_some() {
        return;
    }
    let exits: Vec<_> = app_exits.drain().collect();
    let Some(exit) = exits
        .iter()
        .find(|exit| exit.is_error())
        .or_else(|| exits.first())
        .cloned()
    else {
        return;
    };
    shutdown.requested_exit = Some(exit);
    for (entity, disconnected) in query.iter() {
        if disconnected.is_none() {
            commands.trigger(Disconnect { entity });
        }
    }
}

pub(in crate::client) fn finish_client_shutdown(
    mut app_exits: ResMut<Messages<AppExit>>,
    mut shutdown: ResMut<ClientShutdown>,
    query: Query<Option<&Disconnected>, With<Client>>,
) {
    let mut any_client = false;
    let all_disconnected = query.iter().all(|disconnected| {
        any_client = true;
        disconnected.is_some()
    });
    if any_client
        && all_disconnected
        && let Some(exit) = shutdown.requested_exit.take()
    {
        app_exits.write(exit);
    }
}
