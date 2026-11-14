//! Replacement of a silent lobby session by a fresh login with the same account.
//!
//! An app that is killed sends no Netcode disconnect, so its lobby session would hold the account
//! until the 10 s session timeout and reject the relaunched app as `AccountInUse`. The account
//! credential is the device-held `AccountId`, never the display name, so a hello presenting the
//! same `AccountId` may replace the older session once that session has been silent for
//! [`STALE_LOBBY_SESSION_SILENCE`]. A live duplicate keeps sending and the newcomer is rejected.

use super::{
    LobbyClient, LobbySessionLosses, LobbyState, authenticated_netcode_id,
    profile_transactions::{
        PendingLobbyAdmission, PendingLobbyAdmissions, begin_admission,
        profile_authority_join_rejection,
    },
};
use crate::protocol::{LobbyHello, LobbyJoinOutcome, LobbyJoinRejection, SessionChannel};
use bevy::prelude::*;
use lightyear::prelude::{Connected, Disconnected, Link, LinkOf, MessageSender, RemoteId};
use std::{collections::BTreeMap, time::Duration};

/// How long a session holding an account must have sent nothing before a fresh login with the
/// same account replaces it. A live client sends Netcode keep-alives at 10 Hz, so this is twenty
/// missed keep-alives; it is also how long a newcomer waits before a live holder rejects it.
pub(crate) const STALE_LOBBY_SESSION_SILENCE: Duration = Duration::from_secs(2);

/// Real time at which the lobby last received any datagram on this client link.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LobbyLinkActivity(Duration);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TakeoverDecision {
    Replace,
    Wait,
    Reject,
}

/// Decide a newcomer's admission against the session currently holding its account.
fn decide_takeover(
    now: Duration,
    hello_received_at: Duration,
    holder_last_heard: Option<Duration>,
) -> TakeoverDecision {
    if holder_last_heard
        .is_some_and(|heard| now.saturating_sub(heard) >= STALE_LOBBY_SESSION_SILENCE)
    {
        TakeoverDecision::Replace
    } else if now.saturating_sub(hello_received_at) >= STALE_LOBBY_SESSION_SILENCE {
        TakeoverDecision::Reject
    } else {
        TakeoverDecision::Wait
    }
}

struct DeferredLogin {
    admission: PendingLobbyAdmission,
    received_at: Duration,
}

/// Hellos whose account is held by another session, awaiting [`resolve_account_takeovers`].
/// Keyed by the newcomer's Netcode client ID, so it holds at most one entry per connection.
#[derive(Resource, Default)]
pub(super) struct AccountTakeovers(BTreeMap<u64, DeferredLogin>);

impl AccountTakeovers {
    pub(super) fn hello(&self, client_id: u64) -> Option<&LobbyHello> {
        self.0.get(&client_id).map(|login| &login.admission.hello)
    }

    pub(super) fn defer(
        &mut self,
        client_id: u64,
        holder: u64,
        admission: PendingLobbyAdmission,
        received_at: Duration,
    ) {
        debug!(
            client_id,
            holder, "lobby hello waits for the session holding its account"
        );
        self.0.insert(
            client_id,
            DeferredLogin {
                admission,
                received_at,
            },
        );
    }
}

/// Stamp every client link that received a datagram this frame. Runs after the transports fill
/// the link buffers and before Netcode drains them, so keep-alives count as activity.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(super) fn record_lobby_link_activity(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut links: Query<(Entity, &Link, Option<&mut LobbyLinkActivity>), With<LinkOf>>,
) {
    let now = time.elapsed();
    for (entity, link, activity) in &mut links {
        if link.recv.len() == 0 {
            continue;
        }
        match activity {
            Some(mut activity) => activity.0 = now,
            None => {
                commands.entity(entity).insert(LobbyLinkActivity(now));
            }
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "one ordered admission transaction replaces the holder and starts the newcomer's load"
)]
pub(super) fn resolve_account_takeovers(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut takeovers: ResMut<AccountTakeovers>,
    mut authority: ResMut<crate::profiles::ProfileAuthority>,
    mut pending: ResMut<PendingLobbyAdmissions>,
    mut losses: ResMut<LobbySessionLosses>,
    mut state: ResMut<LobbyState>,
    mut clients: Query<(
        Entity,
        &RemoteId,
        Option<&LobbyClient>,
        Option<&LobbyLinkActivity>,
        &mut MessageSender<LobbyJoinOutcome>,
        Has<Connected>,
        Has<Disconnected>,
    )>,
) {
    if takeovers.0.is_empty() {
        return;
    }
    let now = time.elapsed();
    let deferred: Vec<u64> = takeovers.0.keys().copied().collect();
    for client_id in deferred {
        let login = &takeovers.0[&client_id];
        let newcomer = login.admission.entity;
        let newcomer_live =
            clients
                .get(newcomer)
                .is_ok_and(|(_, remote_id, _, _, _, connected, disconnected)| {
                    connected
                        && !disconnected
                        && authenticated_netcode_id(remote_id) == Some(client_id)
                });
        if !newcomer_live {
            takeovers.0.remove(&client_id);
            continue;
        }
        let holder = authority.account_holder(login.admission.hello.account_id);
        let holder_session = holder.and_then(|holder| {
            clients.iter().find_map(
                |(entity, remote_id, client, activity, _, connected, disconnected)| {
                    (authenticated_netcode_id(remote_id) == Some(holder)).then(|| HolderSession {
                        entity,
                        client: client.copied(),
                        last_heard: activity.map(|activity| activity.0),
                        live: connected && !disconnected,
                    })
                },
            )
        });
        let decision = match (holder, holder_session) {
            (None, _) => None,
            // A holder without a live connection is already leaving.
            (Some(_), None) => Some(TakeoverDecision::Replace),
            (Some(_), Some(session)) if !session.live => Some(TakeoverDecision::Replace),
            (Some(_), Some(session)) => {
                Some(decide_takeover(now, login.received_at, session.last_heard))
            }
        };
        match (decision, holder) {
            (Some(TakeoverDecision::Wait), _) => continue,
            (Some(TakeoverDecision::Reject), _) => {
                takeovers.0.remove(&client_id);
                reject(&mut clients, newcomer, LobbyJoinRejection::AccountInUse);
                continue;
            }
            (Some(TakeoverDecision::Replace), Some(holder)) => {
                replace_holder(
                    &mut commands,
                    &mut authority,
                    &mut pending,
                    &mut losses,
                    &mut state,
                    holder,
                    holder_session,
                );
                info!(
                    replaced_client_id = holder,
                    client_id,
                    silent_ms = holder_session
                        .and_then(|session| session.last_heard)
                        .map(|heard| now.saturating_sub(heard).as_millis()),
                    "lobby replaced a silent session of the same account"
                );
            }
            _ => {}
        }
        let login = takeovers
            .0
            .remove(&client_id)
            .expect("the deferred login is still present");
        if let Err(error) =
            begin_admission(&mut authority, &mut pending, client_id, login.admission)
        {
            reject(
                &mut clients,
                newcomer,
                profile_authority_join_rejection(&error),
            );
        }
    }
}

#[derive(Clone, Copy)]
struct HolderSession {
    entity: Entity,
    client: Option<LobbyClient>,
    last_heard: Option<Duration>,
    live: bool,
}

#[allow(
    clippy::type_complexity,
    reason = "shares the resolve system's single client query"
)]
fn reject(
    clients: &mut Query<(
        Entity,
        &RemoteId,
        Option<&LobbyClient>,
        Option<&LobbyLinkActivity>,
        &mut MessageSender<LobbyJoinOutcome>,
        Has<Connected>,
        Has<Disconnected>,
    )>,
    newcomer: Entity,
    reason: LobbyJoinRejection,
) {
    if let Ok((_, _, _, _, mut sender, ..)) = clients.get_mut(newcomer) {
        sender.send::<SessionChannel>(LobbyJoinOutcome::Rejected { reason });
    }
}

/// Retire the holder's profile session and connection. The lobby-session loss lets
/// `cleanup_disconnected_sessions` cancel its queue ticket, reservation, or Practice formation
/// exactly as for a disconnect. Lightyear 0.29 has no public server-side Netcode kick, so the dead
/// connection is marked `Disconnected` (no further messages are processed) and left to time out.
fn replace_holder(
    commands: &mut Commands,
    authority: &mut crate::profiles::ProfileAuthority,
    pending: &mut PendingLobbyAdmissions,
    losses: &mut LobbySessionLosses,
    state: &mut LobbyState,
    holder: u64,
    holder_session: Option<HolderSession>,
) {
    authority.remove_client(holder);
    // Release the holder's roster entry and accepted name now: the newcomer's profile load can
    // complete later in this same frame, before `cleanup_disconnected_sessions` runs, and must not
    // be suffixed against the session it replaces.
    state.detach_client(holder);
    let pending_entity = pending.remove(holder);
    if let Some(client) = holder_session.and_then(|session| session.client) {
        losses.0.insert(client.lobby_session_id, client.client_id);
    }
    if let Some(entity) = holder_session
        .map(|session| session.entity)
        .or(pending_entity)
    {
        commands.entity(entity).try_insert(Disconnected::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    #[test]
    fn a_holder_silent_for_the_bound_is_replaced_at_once() {
        assert_eq!(
            decide_takeover(ms(10_000), ms(10_000), Some(ms(8_000))),
            TakeoverDecision::Replace
        );
    }

    #[test]
    fn a_recently_heard_holder_makes_the_newcomer_wait_then_reject() {
        assert_eq!(
            decide_takeover(ms(10_000), ms(10_000), Some(ms(9_900))),
            TakeoverDecision::Wait
        );
        // The holder kept sending during the whole wait: it is live.
        assert_eq!(
            decide_takeover(ms(12_000), ms(10_000), Some(ms(11_900))),
            TakeoverDecision::Reject
        );
    }

    #[test]
    fn a_holder_that_falls_silent_while_the_newcomer_waits_is_replaced() {
        // A killed app: the relaunch's hello arrives 0.5 s after the last datagram.
        let heard = ms(10_000);
        let hello = ms(10_500);
        assert_eq!(
            decide_takeover(ms(11_999), hello, Some(heard)),
            TakeoverDecision::Wait
        );
        assert_eq!(
            decide_takeover(heard + STALE_LOBBY_SESSION_SILENCE, hello, Some(heard)),
            TakeoverDecision::Replace
        );
    }

    #[test]
    fn a_holder_without_activity_evidence_is_never_replaced() {
        assert_eq!(
            decide_takeover(ms(1_000), ms(1_000), None),
            TakeoverDecision::Wait
        );
        assert_eq!(
            decide_takeover(ms(1_000) + STALE_LOBBY_SESSION_SILENCE, ms(1_000), None),
            TakeoverDecision::Reject
        );
    }
}
