use super::{
    AppExit, Client, ClientJoinPhase, ClientJoinStatus, ClientLobbyFailure, ClientLobbyIdentity,
    ClientLobbyMembership, ClientNetworkConfig, ClientProfileIdentityState, Commands, Connected,
    Entity, LobbyHello, LobbyJoinOutcome, LobbyServerIdentity, MatchHello, MatchJoinOutcome,
    MatchJoinRejection, MessageReceiver, MessageSender, MessageWriter, NetworkEntityId,
    NetworkTransport, PendingClientConnect, PlayerId, ProtocolFingerprint, Query, Real, Res,
    ResMut, RoutedClientLifecycle, RoutedClientPhase, RoutedClientSession, RoutedClientSessionKind,
    RuntimeLobbyTarget, SessionChannel, String, Time, ToString, With, Without,
    connection_persistence, flow, format, info, warn,
};
use crate::protocol::LobbyJoinRejection;

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn send_client_hello(
    config: Res<ClientNetworkConfig>,
    fingerprint: Res<ProtocolFingerprint>,
    content_fingerprint: Res<crate::content::GameplayContentFingerprint>,
    time: Res<Time<Real>>,
    routed: Res<RoutedClientLifecycle>,
    mut query: Query<
        (
            &mut ClientJoinStatus,
            Option<&mut MessageSender<MatchHello>>,
            Option<&mut MessageSender<LobbyHello>>,
            Option<&RoutedClientSession>,
            Option<&RuntimeLobbyTarget>,
            Option<&ClientLobbyIdentity>,
        ),
        (With<Client>, With<Connected>),
    >,
) {
    for (mut status, match_sender, lobby_sender, routed_session, runtime_target, lobby_identity) in
        query.iter_mut()
    {
        if matches!(status.phase, ClientJoinPhase::Connecting) {
            if routed_session.is_some_and(|session| session.kind == RoutedClientSessionKind::Lobby)
            {
                let Some(identity) = lobby_identity else {
                    continue;
                };
                let Some(mut sender) = lobby_sender else {
                    continue;
                };
                sender.send::<SessionChannel>(LobbyHello {
                    protocol_version: config.expected_protocol_version,
                    build_version: config.expected_build_version.clone(),
                    registry_fingerprint: fingerprint.0,
                    content_fingerprint: *content_fingerprint,
                    account_id: identity.account_id,
                    proposed_display_name: runtime_target.map_or_else(
                        || crate::lobby::generated_display_name(config.client_id),
                        |target| target.proposed_display_name.clone(),
                    ),
                });
            } else {
                let Some(mut sender) = match_sender else {
                    continue;
                };
                sender.send::<SessionChannel>(MatchHello {
                    protocol_version: config.expected_protocol_version,
                    build_version: config.expected_build_version.clone(),
                    registry_fingerprint: fingerprint.0,
                    content_fingerprint: *content_fingerprint,
                });
            }
            status.phase = ClientJoinPhase::AwaitingOutcome;
            status.started_at = time.elapsed();
            info!("brawler client connected; awaiting compatibility outcome");
            if routed_session.is_some_and(|session| session.kind == RoutedClientSessionKind::Match)
                && let Some(request_id) = routed.current_request_id
            {
                // This marker intentionally contains only stable correlation IDs. It is emitted
                // at the Lightyear Connected boundary for the fresh match session; capabilities,
                // player identities, and manifests are never logged.
                let timestamp_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_millis());
                let marker = format!(
                    "brawler-client timing handoff-connected client_id={} request_id={} ts_ms={}\n",
                    config.client_id,
                    request_id.get(),
                    timestamp_ms,
                );
                // Both verification clients inherit one stderr file descriptor. Format the whole
                // bounded marker first and issue one write so process output cannot splice stable
                // IDs into an unparsable half-line.
                let _ = std::io::Write::write_all(&mut std::io::stderr().lock(), marker.as_bytes());
            }
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "the pre-hello identity transaction coordinates one bounded client-local persistence boundary"
)]
pub(super) fn process_lobby_server_identity(
    mut commands: Commands,
    config: Res<ClientNetworkConfig>,
    mut identity_state: ResMut<ClientProfileIdentityState>,
    mut persistence: Option<ResMut<flow::ConnectionPersistence>>,
    path: Option<Res<connection_persistence::ClientConnectionsPath>>,
    failures: Option<Res<flow::ClientLocalLoadFailures>>,
    mut clients: Query<
        (
            Entity,
            &mut ClientJoinStatus,
            &mut MessageReceiver<LobbyServerIdentity>,
            Option<&RuntimeLobbyTarget>,
            Option<&ClientLobbyIdentity>,
        ),
        (With<Client>, With<Connected>, Without<PendingClientConnect>),
    >,
    mut app_exit: MessageWriter<AppExit>,
) {
    for (entity, mut status, mut receiver, target, installed) in &mut clients {
        for announcement in receiver.receive() {
            if !lobby_identity_announcement_is_valid(announcement.logical_server_id, installed) {
                warn!(
                    announced_logical_server_id = announcement.logical_server_id,
                    installed_identity = installed.is_some(),
                    "lobby server identity announcement conflicted with the active session"
                );
                reject_lobby_identity(&mut commands, entity, &mut status, &config, &mut app_exit);
                continue;
            }
            let connections_failed = failures
                .as_deref()
                .is_some_and(|failures| failures.connections_failed);
            let account_id = resolve_lobby_account_id(
                announcement.logical_server_id,
                &config,
                &identity_state,
                target,
                persistence.as_deref_mut(),
                path.as_deref(),
                connections_failed,
            );
            let Some(account_id) = account_id else {
                warn!(
                    has_runtime_target = target.is_some(),
                    has_persistence = persistence.is_some(),
                    has_persistence_path = path.is_some(),
                    local_connections_failed = failures
                        .as_deref()
                        .is_some_and(|failures| failures.connections_failed),
                    "lobby account identity could not be established"
                );
                reject_lobby_identity(&mut commands, entity, &mut status, &config, &mut app_exit);
                continue;
            };
            identity_state.logical_server_id = Some(announcement.logical_server_id);
            identity_state.account_id = Some(account_id);
            commands.entity(entity).insert(ClientLobbyIdentity {
                logical_server_id: announcement.logical_server_id,
                account_id,
            });
        }
    }
}

fn lobby_identity_announcement_is_valid(
    announced_logical_server_id: u128,
    installed: Option<&ClientLobbyIdentity>,
) -> bool {
    announced_logical_server_id != 0
        && installed
            .is_none_or(|identity| identity.logical_server_id == announced_logical_server_id)
}

#[allow(clippy::too_many_arguments)]
fn resolve_lobby_account_id(
    logical_server_id: u128,
    config: &ClientNetworkConfig,
    identity_state: &ClientProfileIdentityState,
    target: Option<&RuntimeLobbyTarget>,
    persistence: Option<&mut flow::ConnectionPersistence>,
    path: Option<&connection_persistence::ClientConnectionsPath>,
    connections_failed: bool,
) -> Option<crate::profiles::AccountId> {
    if identity_state.logical_server_id == Some(logical_server_id) {
        return identity_state.account_id;
    }
    let (Some(target), Some(persistence), Some(path)) = (target, persistence, path) else {
        return crate::profiles::AccountId::new(u128::from(config.client_id)).ok();
    };
    if connections_failed {
        return None;
    }
    let logical_server_key = format!("{logical_server_id:032x}");
    let account_id = match persistence
        .state
        .account_for_server(&logical_server_key, &target.logical_address)
    {
        Ok(account_id) => account_id,
        Err(error) => {
            warn!(%error, "could not bind an account identity to the announced logical server");
            persistence.dirty_error = Some(error);
            return None;
        }
    };
    if let Err(error) = connection_persistence::save_connections(&path.0, &persistence.state) {
        warn!(%error, "could not persist the account identity for the announced logical server");
        persistence.dirty_error = Some(error);
        return None;
    }
    Some(account_id)
}

pub(super) fn reject_lobby_identity(
    commands: &mut Commands,
    entity: Entity,
    status: &mut ClientJoinStatus,
    config: &ClientNetworkConfig,
    app_exit: &mut MessageWriter<AppExit>,
) {
    status.phase = ClientJoinPhase::Disconnected;
    if config.presents_product_shell() {
        commands
            .entity(entity)
            .insert(ClientLobbyFailure::InvalidWelcome);
    } else {
        app_exit.write(AppExit::error());
    }
}

/// Map one server join rejection to the stable failure category its evidence belongs to.
pub(in crate::client) fn join_rejection_category(
    reason: &MatchJoinRejection,
) -> crate::diagnostics::FailureCategory {
    match reason {
        MatchJoinRejection::ProtocolVersionMismatch
        | MatchJoinRejection::BuildVersionMismatch
        | MatchJoinRejection::RegistryMismatch => {
            crate::diagnostics::FailureCategory::ProtocolMismatch
        }
        MatchJoinRejection::ContentMismatch => crate::diagnostics::FailureCategory::ContentMismatch,
        MatchJoinRejection::HandshakeTimeout => crate::diagnostics::FailureCategory::Timeout,
        MatchJoinRejection::ServerFull
        | MatchJoinRejection::MatchFull
        | MatchJoinRejection::MatchInProgress
        | MatchJoinRejection::IdentifierExhausted => {
            crate::diagnostics::FailureCategory::ShutdownIncomplete
        }
    }
}

/// Classify a client error exit and append the bounded local failure record when the
/// `BRAWLER_FAILURE_REPORT` control selects one, so client failures keep the same stable
/// categories the dedicated server already records.
pub(super) fn record_client_failure(
    diagnostics: Option<&Res<crate::diagnostics::ProcessDiagnosticsSettings>>,
    classification: &mut crate::diagnostics::ProcessExitClassification,
    category: crate::diagnostics::FailureCategory,
    message: String,
) {
    classification.record_error_exit(category.into());
    if let Some(settings) = diagnostics
        && let Some(path) = settings.failure_record_path()
    {
        crate::diagnostics::write_failure_record(
            &path,
            &crate::diagnostics::ProcessFailureRecordV1::new(category, message),
        );
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "the system coordinates the ordered match and lobby receive transactions"
)]
pub(super) fn process_join_outcome(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &mut ClientJoinStatus,
            Option<&mut MessageReceiver<MatchJoinOutcome>>,
            Option<&mut MessageReceiver<LobbyJoinOutcome>>,
            Option<&RoutedClientSession>,
            Option<&ClientLobbyMembership>,
            Option<&ClientLobbyIdentity>,
        ),
        (With<Client>, Without<PendingClientConnect>),
    >,
    config: Res<ClientNetworkConfig>,
    routed: Res<RoutedClientLifecycle>,
    diagnostics: Option<Res<crate::diagnostics::ProcessDiagnosticsSettings>>,
    mut classification: ResMut<crate::diagnostics::ProcessExitClassification>,
    mut app_exit: MessageWriter<AppExit>,
) {
    for (
        entity,
        mut status,
        match_receiver,
        lobby_receiver,
        routed_session,
        membership,
        lobby_identity,
    ) in query.iter_mut()
    {
        if let Some(mut receiver) = match_receiver {
            process_match_join_outcomes(
                &mut receiver,
                &mut status,
                &config,
                &routed,
                diagnostics.as_ref(),
                &mut classification,
                &mut app_exit,
            );
        }
        if !routed_session.is_some_and(|session| session.kind == RoutedClientSessionKind::Lobby) {
            continue;
        }
        let Some(mut receiver) = lobby_receiver else {
            continue;
        };
        process_lobby_join_outcomes(
            entity,
            &mut receiver,
            &mut status,
            membership,
            lobby_identity,
            &mut commands,
            &config,
            diagnostics.as_ref(),
            &mut classification,
            &mut app_exit,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn process_match_join_outcomes(
    receiver: &mut MessageReceiver<MatchJoinOutcome>,
    status: &mut ClientJoinStatus,
    config: &ClientNetworkConfig,
    routed: &RoutedClientLifecycle,
    diagnostics: Option<&Res<crate::diagnostics::ProcessDiagnosticsSettings>>,
    classification: &mut crate::diagnostics::ProcessExitClassification,
    app_exit: &mut MessageWriter<AppExit>,
) {
    for outcome in receiver.receive() {
        match outcome {
            MatchJoinOutcome::Accepted {
                player_id,
                network_entity_id,
            } => accept_match_join(status, config, player_id, network_entity_id),
            MatchJoinOutcome::Rejected { reason }
                if routed_join_rejection_is_stale(config, routed) =>
            {
                warn!(
                    ?reason,
                    "ignoring join rejection during routed session teardown"
                );
            }
            MatchJoinOutcome::Rejected { reason } => {
                warn!(?reason, "brawler client rejected");
                record_client_failure(
                    diagnostics,
                    classification,
                    join_rejection_category(&reason),
                    format!("join rejected: {reason:?}"),
                );
                status.phase = ClientJoinPhase::Rejected(reason);
                app_exit.write(AppExit::error());
            }
        }
    }
}

fn accept_match_join(
    status: &mut ClientJoinStatus,
    config: &ClientNetworkConfig,
    player_id: PlayerId,
    network_entity_id: NetworkEntityId,
) {
    info!(
        player_id = player_id.0,
        network_entity_id = network_entity_id.0,
        "brawler client accepted"
    );
    if config.render_measurement.is_some() {
        eprintln!(
            "brawler-client timing match-accepted client_id={} player_id={} ts_ms={}",
            config.client_id,
            player_id.0,
            crate::diagnostics::unix_micros_now() / 1_000
        );
    }
    status.phase = ClientJoinPhase::Active {
        player_id,
        network_entity_id,
    };
}

fn routed_join_rejection_is_stale(
    config: &ClientNetworkConfig,
    routed: &RoutedClientLifecycle,
) -> bool {
    config.transport == NetworkTransport::RoutedUdp
        && matches!(
            routed.phase,
            RoutedClientPhase::AwaitingLobbyUnlink
                | RoutedClientPhase::AwaitingLobbyRetryUnlink
                | RoutedClientPhase::AwaitingMatchUnlink
        )
}

#[derive(Clone, Copy, Debug, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the named flags preserve precise validation diagnostics for one bounded welcome"
)]
struct LobbyWelcomeValidation {
    catalog_invalid: bool,
    catalog_revision_mismatch: bool,
    rejected_this_batch: bool,
    installed_membership_conflict: bool,
    batch_membership_conflict: bool,
    identity_mismatch: bool,
    profile_invalid: bool,
}

impl LobbyWelcomeValidation {
    fn failed(self) -> bool {
        self.catalog_invalid
            || self.catalog_revision_mismatch
            || self.rejected_this_batch
            || self.installed_membership_conflict
            || self.batch_membership_conflict
            || self.identity_mismatch
            || self.profile_invalid
    }
}

fn validate_lobby_welcome(
    accepted: &ClientLobbyMembership,
    declared_catalog_revision: crate::lobby::CatalogRevision,
    rejected_this_batch: bool,
    installed: Option<&ClientLobbyMembership>,
    accepted_this_batch: Option<&ClientLobbyMembership>,
    identity: Option<&ClientLobbyIdentity>,
) -> LobbyWelcomeValidation {
    LobbyWelcomeValidation {
        catalog_invalid: crate::lobby::validate_catalog(&accepted.game_types).is_err(),
        catalog_revision_mismatch: crate::lobby::catalog_revision(&accepted.game_types).ok()
            != Some(declared_catalog_revision),
        rejected_this_batch,
        installed_membership_conflict: installed.is_some_and(|value| value != accepted),
        batch_membership_conflict: accepted_this_batch.is_some_and(|value| value != accepted),
        identity_mismatch: identity.is_none_or(|value| {
            value.logical_server_id != accepted.logical_server_id
                || value.account_id != accepted.profile.account_id
        }),
        profile_invalid: accepted.profile.validate_bounded().is_err()
            || accepted.brawler_catalog.validate().is_err()
            || accepted
                .brawler_catalog
                .validate_profile(&accepted.profile)
                .is_err(),
    }
}

#[allow(clippy::too_many_arguments)]
fn process_lobby_join_outcomes(
    entity: Entity,
    receiver: &mut MessageReceiver<LobbyJoinOutcome>,
    status: &mut ClientJoinStatus,
    membership: Option<&ClientLobbyMembership>,
    lobby_identity: Option<&ClientLobbyIdentity>,
    commands: &mut Commands,
    config: &ClientNetworkConfig,
    diagnostics: Option<&Res<crate::diagnostics::ProcessDiagnosticsSettings>>,
    classification: &mut crate::diagnostics::ProcessExitClassification,
    app_exit: &mut MessageWriter<AppExit>,
) {
    let mut accepted_this_batch: Option<ClientLobbyMembership> = None;
    let mut rejected_this_batch = false;
    for outcome in receiver.receive() {
        match outcome {
            LobbyJoinOutcome::Accepted {
                logical_server_id,
                player_id,
                accepted_display_name,
                server_name,
                catalog_revision,
                game_types,
                brawler_catalog,
                profile,
            } => {
                let accepted = ClientLobbyMembership {
                    logical_server_id,
                    player_id,
                    accepted_display_name,
                    server_name,
                    catalog_revision,
                    game_types,
                    brawler_catalog: *brawler_catalog,
                    profile: *profile,
                };
                let validation = validate_lobby_welcome(
                    &accepted,
                    catalog_revision,
                    rejected_this_batch,
                    membership,
                    accepted_this_batch.as_ref(),
                    lobby_identity,
                );
                if validation.failed() {
                    warn!(
                        catalog_invalid = validation.catalog_invalid,
                        catalog_revision_mismatch = validation.catalog_revision_mismatch,
                        rejected_this_batch = validation.rejected_this_batch,
                        installed_membership_conflict = validation.installed_membership_conflict,
                        batch_membership_conflict = validation.batch_membership_conflict,
                        identity_mismatch = validation.identity_mismatch,
                        profile_invalid = validation.profile_invalid,
                        "lobby welcome failed client consistency validation"
                    );
                    reject_invalid_lobby_welcome(
                        entity,
                        status,
                        commands,
                        config,
                        diagnostics,
                        classification,
                        app_exit,
                    );
                    continue;
                }
                if membership == Some(&accepted) || accepted_this_batch.as_ref() == Some(&accepted)
                {
                    continue;
                }
                commands.entity(entity).insert(accepted.clone());
                accepted_this_batch = Some(accepted);
                status.phase = ClientJoinPhase::LobbyActive { player_id };
                info!(player_id = player_id.0, "brawler lobby client accepted");
                if config.exit_after_lobby_welcome {
                    app_exit.write(AppExit::Success);
                }
            }
            LobbyJoinOutcome::Rejected { reason } => {
                if membership.is_some() || accepted_this_batch.is_some() {
                    reject_conflicting_lobby_outcome(entity, status, commands, config, app_exit);
                    continue;
                }
                rejected_this_batch = true;
                warn!(?reason, "brawler lobby client rejected");
                reject_lobby_join(
                    entity,
                    reason,
                    status,
                    commands,
                    config,
                    diagnostics,
                    classification,
                    app_exit,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn reject_invalid_lobby_welcome(
    entity: Entity,
    status: &mut ClientJoinStatus,
    commands: &mut Commands,
    config: &ClientNetworkConfig,
    diagnostics: Option<&Res<crate::diagnostics::ProcessDiagnosticsSettings>>,
    classification: &mut crate::diagnostics::ProcessExitClassification,
    app_exit: &mut MessageWriter<AppExit>,
) {
    if config.presents_product_shell() {
        commands
            .entity(entity)
            .insert(ClientLobbyFailure::InvalidWelcome);
        status.phase = ClientJoinPhase::Disconnected;
    } else {
        record_client_failure(
            diagnostics,
            classification,
            crate::diagnostics::FailureCategory::ProtocolMismatch,
            "lobby advertised an invalid or conflicting welcome".to_string(),
        );
        app_exit.write(AppExit::error());
    }
}

fn reject_conflicting_lobby_outcome(
    entity: Entity,
    status: &mut ClientJoinStatus,
    commands: &mut Commands,
    config: &ClientNetworkConfig,
    app_exit: &mut MessageWriter<AppExit>,
) {
    if config.presents_product_shell() {
        commands
            .entity(entity)
            .insert(ClientLobbyFailure::InvalidWelcome);
        status.phase = ClientJoinPhase::Disconnected;
    } else {
        app_exit.write(AppExit::error());
    }
}

#[allow(clippy::too_many_arguments)]
fn reject_lobby_join(
    entity: Entity,
    reason: LobbyJoinRejection,
    status: &mut ClientJoinStatus,
    commands: &mut Commands,
    config: &ClientNetworkConfig,
    diagnostics: Option<&Res<crate::diagnostics::ProcessDiagnosticsSettings>>,
    classification: &mut crate::diagnostics::ProcessExitClassification,
    app_exit: &mut MessageWriter<AppExit>,
) {
    if config.presents_product_shell() {
        commands
            .entity(entity)
            .insert(ClientLobbyFailure::Rejected(reason));
        status.phase = ClientJoinPhase::Disconnected;
    } else {
        record_client_failure(
            diagnostics,
            classification,
            crate::diagnostics::FailureCategory::ProtocolMismatch,
            format!("lobby join rejected: {reason:?}"),
        );
        app_exit.write(AppExit::error());
    }
}

#[cfg(test)]
mod tests {
    use super::{lobby_identity_announcement_is_valid, routed_join_rejection_is_stale};
    use crate::client::{
        ClientLobbyIdentity, ClientNetworkConfig, NetworkTransport, RoutedClientLifecycle,
        RoutedClientPhase,
    };

    #[test]
    fn lobby_identity_validation_rejects_zero_and_conflicting_servers() {
        let identity = ClientLobbyIdentity {
            logical_server_id: 7,
            account_id: crate::profiles::AccountId::new(9).unwrap(),
        };
        assert!(!lobby_identity_announcement_is_valid(0, None));
        assert!(lobby_identity_announcement_is_valid(7, Some(&identity)));
        assert!(!lobby_identity_announcement_is_valid(8, Some(&identity)));
    }

    #[test]
    fn only_routed_teardown_suppresses_a_late_match_rejection() {
        let mut config = ClientNetworkConfig::new(1);
        config.transport = NetworkTransport::RoutedUdp;
        let mut routed = RoutedClientLifecycle {
            phase: RoutedClientPhase::AwaitingLobbyUnlink,
            ..Default::default()
        };
        assert!(routed_join_rejection_is_stale(&config, &routed));

        routed.phase = RoutedClientPhase::Match;
        assert!(!routed_join_rejection_is_stale(&config, &routed));
        config.transport = NetworkTransport::Udp;
        routed.phase = RoutedClientPhase::AwaitingMatchUnlink;
        assert!(!routed_join_rejection_is_stale(&config, &routed));
    }
}
