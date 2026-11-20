//! Client mirror and command lifecycle for the server-owned character pick.

use super::{Client, ClientLobbyMembership};
use bevy::prelude::*;
use lightyear::prelude::{MessageReceiver, MessageSender};
use std::collections::VecDeque;

#[cfg(feature = "automation")]
mod automation;
#[cfg(feature = "automation")]
pub(crate) use automation::automation_character_ready;

#[derive(Resource, Clone, Debug, Default)]
pub struct ClientProfileModel {
    snapshot: Option<brawler::profiles::ProfileSnapshot>,
    catalog: Option<brawler::profiles::AdvertisedCharacterCatalog>,
    pending_request_id: Option<u64>,
    pending_choice: Option<brawler::characters::CharacterChoice>,
    next_request_id: u64,
    outbound: VecDeque<brawler::profiles::ProfileCommand>,
    last_decision: Option<brawler::profiles::ProfileDecision>,
}

impl ClientProfileModel {
    #[must_use]
    pub fn snapshot(&self) -> Option<&brawler::profiles::ProfileSnapshot> {
        self.snapshot.as_ref()
    }

    #[must_use]
    pub fn catalog(&self) -> Option<&brawler::profiles::AdvertisedCharacterCatalog> {
        self.catalog.as_ref()
    }

    #[must_use]
    pub const fn pending(&self) -> bool {
        self.pending_request_id.is_some()
    }

    /// The pick a sent selection is waiting on.
    #[must_use]
    pub const fn pending_choice(&self) -> Option<brawler::characters::CharacterChoice> {
        self.pending_choice
    }

    pub fn take_decision(&mut self) -> Option<brawler::profiles::ProfileDecision> {
        self.last_decision.take()
    }

    #[cfg(test)]
    pub(crate) fn set_snapshot_for_test(&mut self, snapshot: brawler::profiles::ProfileSnapshot) {
        self.snapshot = Some(snapshot);
        self.pending_request_id = None;
        self.pending_choice = None;
        self.catalog = Some(
            brawler::profiles::AdvertisedCharacterCatalog::embedded()
                .expect("embedded character advertisement"),
        );
    }

    #[cfg(test)]
    pub(crate) fn set_decision_for_test(&mut self, decision: brawler::profiles::ProfileDecision) {
        self.last_decision = Some(decision);
    }

    #[cfg(test)]
    pub(crate) fn queued_commands_for_test(&self) -> &VecDeque<brawler::profiles::ProfileCommand> {
        &self.outbound
    }

    /// Select `choice` (a character with one of its ultimates and one of its passives). The
    /// server remembers the pick for that character.
    pub fn select(&mut self, choice: brawler::characters::CharacterChoice) -> bool {
        let Some(revision) = self.snapshot.as_ref().map(|snapshot| snapshot.revision) else {
            return false;
        };
        if self
            .catalog
            .as_ref()
            .is_none_or(|catalog| !catalog.accepts(choice))
        {
            return false;
        }
        let Some(request_id) = self.begin_request() else {
            return false;
        };
        self.pending_choice = Some(choice);
        self.outbound.push_back(brawler::profiles::ProfileCommand {
            request_id,
            expected_profile_revision: revision,
            choice,
        });
        true
    }

    fn begin_request(&mut self) -> Option<u64> {
        if self.pending_request_id.is_some() {
            return None;
        }
        let request_id = self.next_request_id.checked_add(1)?;
        self.next_request_id = request_id;
        self.pending_request_id = Some(request_id);
        self.pending_choice = None;
        self.last_decision = None;
        Some(request_id)
    }
}

pub struct ClientProfilePlugin;

impl Plugin for ClientProfilePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientProfileModel>().add_systems(
            Update,
            (
                clear_profile_when_membership_ends,
                bind_profile_snapshot,
                receive_profile_outcomes,
                send_profile_commands,
            )
                .chain(),
        );
        // Automation-only: headless and render-evidence clients select the `--character` pick
        // so they can queue without the interactive screen. Queued after outbound drain so the command leaves on the following
        // frame, as it did inside the former chain tail.
        #[cfg(feature = "automation")]
        app.add_systems(
            Update,
            automation::prepare_automation_character.after(send_profile_commands),
        );
    }
}

fn clear_profile_when_membership_ends(
    mut removed: RemovedComponents<ClientLobbyMembership>,
    mut model: ResMut<ClientProfileModel>,
) {
    if removed.read().next().is_some() {
        *model = ClientProfileModel::default();
    }
}

fn bind_profile_snapshot(
    memberships: Query<&ClientLobbyMembership, (With<Client>, Changed<ClientLobbyMembership>)>,
    mut model: ResMut<ClientProfileModel>,
) {
    for membership in &memberships {
        model.snapshot = Some(membership.profile.clone());
        model.catalog = Some(membership.character_catalog.clone());
    }
}

fn receive_profile_outcomes(
    mut model: ResMut<ClientProfileModel>,
    mut clients: Query<
        (
            &mut ClientLobbyMembership,
            &mut MessageReceiver<brawler::profiles::ProfileOutcome>,
        ),
        With<Client>,
    >,
) {
    for (mut membership, mut receiver) in &mut clients {
        for outcome in receiver.receive().take(4) {
            if model.pending_request_id != Some(outcome.request_id) {
                continue;
            }
            model.pending_request_id = None;
            model.pending_choice = None;
            model.last_decision = Some(outcome.decision.clone());
            if let Some(snapshot) = outcome.snapshot {
                let snapshot_valid = snapshot.validate_bounded().is_ok()
                    && model
                        .catalog
                        .as_ref()
                        .is_some_and(|catalog| catalog.validate_profile(&snapshot).is_ok());
                if !snapshot_valid {
                    model.last_decision = Some(brawler::profiles::ProfileDecision::StorageFault);
                    continue;
                }
                membership.profile = snapshot.clone();
                model.snapshot = Some(snapshot);
            }
        }
    }
}

fn send_profile_commands(
    mut model: ResMut<ClientProfileModel>,
    mut senders: Query<&mut MessageSender<brawler::profiles::ProfileCommand>, With<Client>>,
) {
    let Ok(mut sender) = senders.single_mut() else {
        return;
    };
    while let Some(command) = model.outbound.pop_front() {
        sender.send::<brawler::protocol::ProfileChannel>(command);
    }
}
