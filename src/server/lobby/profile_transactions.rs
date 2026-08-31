//! Profile-backed lobby admission and mutation transaction bridge.

use super::{
    LobbyClient, LobbySessionError, LobbyState, QueueSnapshotPublication, QueueState,
    authenticated_netcode_id, catalog,
};
use crate::{
    protocol::{LobbyHello, LobbyJoinOutcome, LobbyJoinRejection, ProfileChannel, SessionChannel},
    server::{LobbyControlOutbox, RoutedPeer},
};
use bevy::prelude::*;
use brawler_routing::{LobbyAuthenticatedBody, PeerId, RouteId};
use lightyear::prelude::{Connected, Disconnected, MessageReceiver, MessageSender, RemoteId};
use std::collections::BTreeMap;

#[derive(Resource, Default)]
pub(super) struct PendingLobbyAdmissions(BTreeMap<u64, PendingLobbyAdmission>);

#[derive(Clone)]
struct PendingLobbyAdmission {
    entity: Entity,
    route_id: RouteId,
    peer_id: PeerId,
    hello: LobbyHello,
}

#[allow(
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "the query is the one bounded lobby authentication transaction"
)]
pub(super) fn lobby_receive_hellos(
    state: Res<LobbyState>,
    mut authority: ResMut<crate::profiles::ProfileAuthority>,
    mut pending: ResMut<PendingLobbyAdmissions>,
    mut receivers: Query<(
        Entity,
        &RemoteId,
        &mut MessageReceiver<LobbyHello>,
        &mut MessageSender<LobbyJoinOutcome>,
        Option<&RoutedPeer>,
        Has<Connected>,
        Has<Disconnected>,
    )>,
) {
    for (entity, remote_id, mut receiver, mut sender, routed_peer, connected, disconnected) in
        &mut receivers
    {
        if !connected || disconnected {
            continue;
        }
        let messages: Vec<_> = receiver.receive().collect();
        for hello in messages {
            let Some(client_id) = authenticated_netcode_id(remote_id) else {
                sender.send::<SessionChannel>(LobbyJoinOutcome::Rejected {
                    reason: LobbySessionError::InvalidClientId.rejection(),
                });
                continue;
            };
            let Some(peer) = routed_peer else {
                sender.send::<SessionChannel>(LobbyJoinOutcome::Rejected {
                    reason: LobbySessionError::NotRouted.rejection(),
                });
                continue;
            };
            if let Err(error) = state.validate_hello(&hello) {
                sender.send::<SessionChannel>(LobbyJoinOutcome::Rejected {
                    reason: error.rejection(),
                });
                continue;
            }
            if state.session_for_client(client_id).is_some() {
                continue;
            }
            if let Some(existing) = pending.0.get(&client_id) {
                if existing.hello != hello {
                    sender.send::<SessionChannel>(LobbyJoinOutcome::Rejected {
                        reason: LobbyJoinRejection::InvalidAccount,
                    });
                }
                continue;
            }
            match authority.begin_load(client_id, hello.account_id) {
                Ok(()) => {
                    pending.0.insert(
                        client_id,
                        PendingLobbyAdmission {
                            entity,
                            route_id: peer.route_id,
                            peer_id: peer.peer_id,
                            hello,
                        },
                    );
                }
                Err(error) => sender.send::<SessionChannel>(LobbyJoinOutcome::Rejected {
                    reason: profile_authority_join_rejection(&error),
                }),
            }
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "one ordered authority result pump atomically promotes pending sessions and publishes outcomes"
)]
pub(super) fn process_profile_storage_results(
    mut commands: Commands,
    mut state: ResMut<LobbyState>,
    mut authority: ResMut<crate::profiles::ProfileAuthority>,
    mut pending: ResMut<PendingLobbyAdmissions>,
    mut outbox: ResMut<LobbyControlOutbox>,
    catalog: Res<catalog::ResolvedLobbyCatalog>,
    mut publications: ResMut<QueueSnapshotPublication>,
    mut clients: Query<(
        &RemoteId,
        &mut MessageSender<LobbyJoinOutcome>,
        &mut MessageSender<crate::profiles::ProfileOutcome>,
        Has<Connected>,
        Has<Disconnected>,
    )>,
) {
    let (loads, mutations) = authority
        .poll_loads()
        .unwrap_or_else(|error| panic!("profile storage executor failed: {error:?}"));
    for completion in loads {
        let Some(admission) = pending.0.remove(&completion.client_key) else {
            authority.remove_client(completion.client_key);
            continue;
        };
        let Ok((remote_id, mut sender, _, connected, disconnected)) =
            clients.get_mut(admission.entity)
        else {
            authority.remove_client(completion.client_key);
            continue;
        };
        if !connected
            || disconnected
            || authenticated_netcode_id(remote_id) != Some(completion.client_key)
        {
            authority.remove_client(completion.client_key);
            continue;
        }
        let profile = match completion.result {
            Ok(profile) => profile,
            Err(decision) => {
                sender.send::<SessionChannel>(LobbyJoinOutcome::Rejected {
                    reason: profile_load_rejection(&decision),
                });
                authority.remove_client(completion.client_key);
                continue;
            }
        };
        let session = match state.accept_client(
            completion.client_key,
            admission.route_id,
            admission.peer_id,
            &admission.hello,
        ) {
            Ok(session) => session,
            Err(error) => {
                sender.send::<SessionChannel>(LobbyJoinOutcome::Rejected {
                    reason: error.rejection(),
                });
                authority.remove_client(completion.client_key);
                continue;
            }
        };
        commands.entity(admission.entity).insert(LobbyClient {
            client_id: session.netcode_client_id,
            lobby_session_id: session.lobby_session_id,
        });
        let _ = outbox.push_authenticated(LobbyAuthenticatedBody {
            route_id: session.route_id,
            peer_id: session.peer_id,
            lobby_session_id: session.lobby_session_id,
            netcode_client_id: session.netcode_client_id,
        });
        if state.mark_welcome_sent(session.netcode_client_id) {
            sender.send::<SessionChannel>(LobbyJoinOutcome::Accepted {
                logical_server_id: state.manifest.common.logical_server_id.get(),
                player_id: crate::protocol::PlayerId(session.player_id.get()),
                accepted_display_name: state
                    .accepted_name(session.netcode_client_id)
                    .expect("accepted session owns a name")
                    .to_string(),
                server_name: catalog.server_name.clone(),
                catalog_revision: catalog.revision,
                game_types: catalog.game_types.clone(),
                brawler_catalog: Box::new(catalog.brawler_catalog.clone()),
                profile: Box::new(profile),
            });
            publications.initial_pending = true;
        }
    }
    for (client_key, outcome) in mutations {
        for (remote_id, _, mut sender, connected, disconnected) in &mut clients {
            if connected && !disconnected && authenticated_netcode_id(remote_id) == Some(client_key)
            {
                sender.send::<ProfileChannel>(outcome.clone());
                break;
            }
        }
    }
}

#[allow(
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "the bounded profile command receiver serializes mutations before queue admission"
)]
pub(super) fn collect_profile_commands(
    mut authority: ResMut<crate::profiles::ProfileAuthority>,
    queue: Res<QueueState>,
    mut clients: Query<(
        &LobbyClient,
        &mut MessageReceiver<crate::profiles::ProfileCommand>,
        &mut MessageSender<crate::profiles::ProfileOutcome>,
        Has<Disconnected>,
    )>,
) {
    for (client, mut receiver, mut sender, disconnected) in &mut clients {
        if disconnected {
            continue;
        }
        let queue_locked = queue.ticket_for_client(client.client_id).is_some();
        for command in receiver.receive().take(4) {
            match authority.submit_command(client.client_id.get(), command.clone(), queue_locked) {
                Ok(crate::profiles::ProfileMutationSubmission::Pending) => {}
                Ok(crate::profiles::ProfileMutationSubmission::Immediate(outcome)) => {
                    sender.send::<ProfileChannel>(outcome);
                }
                Err(crate::profiles::ProfileAuthorityError::StorageStopped) => {
                    panic!("profile storage executor stopped")
                }
                Err(error) => sender.send::<ProfileChannel>(crate::profiles::ProfileOutcome {
                    request_id: profile_command_request_id(&command),
                    decision: profile_command_error_decision(&error)
                        .expect("StorageStopped is handled as the fatal system boundary"),
                    snapshot: None,
                }),
            }
        }
    }
}

fn profile_command_request_id(command: &crate::profiles::ProfileCommand) -> u64 {
    match command {
        crate::profiles::ProfileCommand::CreateBrawler { request_id, .. }
        | crate::profiles::ProfileCommand::EditBrawler { request_id, .. }
        | crate::profiles::ProfileCommand::SelectBrawler { request_id, .. }
        | crate::profiles::ProfileCommand::DeleteBrawler { request_id, .. }
        | crate::profiles::ProfileCommand::EquipWeaponParts { request_id, .. } => *request_id,
    }
}

fn profile_load_rejection(decision: &crate::profiles::ProfileDecision) -> LobbyJoinRejection {
    match decision {
        crate::profiles::ProfileDecision::StorageFault => LobbyJoinRejection::StorageUnavailable,
        _ => LobbyJoinRejection::InvalidAccount,
    }
}

fn profile_command_error_decision(
    error: &crate::profiles::ProfileAuthorityError,
) -> Option<crate::profiles::ProfileDecision> {
    match error {
        crate::profiles::ProfileAuthorityError::StorageStopped => None,
        crate::profiles::ProfileAuthorityError::QueueLocked => {
            Some(crate::profiles::ProfileDecision::QueueLocked)
        }
        crate::profiles::ProfileAuthorityError::InvalidRequest
        | crate::profiles::ProfileAuthorityError::UnknownSession => {
            Some(crate::profiles::ProfileDecision::InvalidRequest)
        }
        _ => Some(crate::profiles::ProfileDecision::TemporarilyUnavailable),
    }
}

fn profile_authority_join_rejection(
    error: &crate::profiles::ProfileAuthorityError,
) -> LobbyJoinRejection {
    match error {
        crate::profiles::ProfileAuthorityError::AccountInUse => LobbyJoinRejection::AccountInUse,
        crate::profiles::ProfileAuthorityError::StorageStopped => {
            LobbyJoinRejection::StorageUnavailable
        }
        _ => LobbyJoinRejection::InvalidAccount,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brawler_id() -> crate::profiles::SavedBrawlerId {
        crate::profiles::SavedBrawlerId::new(1).unwrap()
    }

    fn draft() -> crate::profiles::BrawlerDraft {
        crate::profiles::BrawlerDraft {
            name: "Test".into(),
            fighter_profile_id: crate::profiles::FighterProfileId(1),
            weapon_base_id: crate::profiles::WeaponBaseId(1),
            ultimate_id: crate::builds::UltimateDefinitionId(1),
            passive_ids: [
                crate::builds::PassiveDefinitionId(3),
                crate::builds::PassiveDefinitionId(4),
            ],
        }
    }

    #[test]
    fn profile_command_request_ids_cover_every_variant() {
        let revision = crate::profiles::ProfileRevision::INITIAL;
        let commands = [
            crate::profiles::ProfileCommand::CreateBrawler {
                request_id: 1,
                expected_profile_revision: revision,
                draft: draft(),
            },
            crate::profiles::ProfileCommand::EditBrawler {
                request_id: 2,
                expected_profile_revision: revision,
                brawler_id: brawler_id(),
                expected_brawler_revision: revision,
                edit: crate::profiles::BrawlerEdit {
                    name: "Edited".into(),
                    ultimate_id: crate::builds::UltimateDefinitionId(1),
                    passive_ids: [
                        crate::builds::PassiveDefinitionId(3),
                        crate::builds::PassiveDefinitionId(4),
                    ],
                },
            },
            crate::profiles::ProfileCommand::SelectBrawler {
                request_id: 3,
                expected_profile_revision: revision,
                brawler_id: brawler_id(),
            },
            crate::profiles::ProfileCommand::DeleteBrawler {
                request_id: 4,
                expected_profile_revision: revision,
                brawler_id: brawler_id(),
                expected_brawler_revision: revision,
            },
            crate::profiles::ProfileCommand::EquipWeaponParts {
                request_id: 5,
                expected_profile_revision: revision,
                brawler_id: brawler_id(),
                expected_brawler_revision: revision,
                equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
            },
        ];

        assert_eq!(
            commands
                .iter()
                .map(profile_command_request_id)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn profile_load_and_authority_errors_keep_exact_wire_mappings() {
        use crate::profiles::{ProfileAuthorityError as Error, ProfileDecision as Decision};

        for decision in [
            Decision::Accepted,
            Decision::InvalidRequest,
            Decision::StaleRevision,
            Decision::MissingBrawler,
            Decision::CapacityReached,
            Decision::QueueLocked,
            Decision::TemporarilyUnavailable,
            Decision::MissingPart,
            Decision::PartAlreadyEquipped,
            Decision::IncompatibleWeapon,
            Decision::IncompatibleBuild,
        ] {
            assert_eq!(
                profile_load_rejection(&decision),
                LobbyJoinRejection::InvalidAccount
            );
        }
        assert_eq!(
            profile_load_rejection(&Decision::StorageFault),
            LobbyJoinRejection::StorageUnavailable
        );

        for (error, expected) in [
            (Error::QueueLocked, Some(Decision::QueueLocked)),
            (Error::InvalidRequest, Some(Decision::InvalidRequest)),
            (Error::UnknownSession, Some(Decision::InvalidRequest)),
            (Error::AccountInUse, Some(Decision::TemporarilyUnavailable)),
            (
                Error::AlreadyPending,
                Some(Decision::TemporarilyUnavailable),
            ),
            (
                Error::IncompatibleBuild,
                Some(Decision::TemporarilyUnavailable),
            ),
            (
                Error::TemporarilyUnavailable,
                Some(Decision::TemporarilyUnavailable),
            ),
            (
                Error::IdentifierExhausted,
                Some(Decision::TemporarilyUnavailable),
            ),
            (Error::StorageStopped, None),
        ] {
            assert_eq!(profile_command_error_decision(&error), expected);
        }

        for (error, expected) in [
            (Error::AccountInUse, LobbyJoinRejection::AccountInUse),
            (
                Error::StorageStopped,
                LobbyJoinRejection::StorageUnavailable,
            ),
            (Error::AlreadyPending, LobbyJoinRejection::InvalidAccount),
            (Error::UnknownSession, LobbyJoinRejection::InvalidAccount),
            (Error::QueueLocked, LobbyJoinRejection::InvalidAccount),
            (Error::InvalidRequest, LobbyJoinRejection::InvalidAccount),
            (Error::IncompatibleBuild, LobbyJoinRejection::InvalidAccount),
            (
                Error::TemporarilyUnavailable,
                LobbyJoinRejection::InvalidAccount,
            ),
            (
                Error::IdentifierExhausted,
                LobbyJoinRejection::InvalidAccount,
            ),
        ] {
            assert_eq!(profile_authority_join_rejection(&error), expected);
        }
    }
}
