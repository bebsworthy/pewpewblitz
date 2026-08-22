//! Client mirror and command lifecycle for the server-owned saved-brawler profile.

use super::{Client, ClientLobbyMembership};
use bevy::prelude::*;
use lightyear::prelude::{MessageReceiver, MessageSender};
use std::collections::VecDeque;

#[derive(Resource, Clone, Debug, Default)]
pub struct ClientProfileModel {
    snapshot: Option<crate::profiles::ProfileSnapshot>,
    pending_request_id: Option<u64>,
    next_request_id: u64,
    outbound: VecDeque<crate::profiles::ProfileCommand>,
    last_decision: Option<crate::profiles::ProfileDecision>,
}

impl ClientProfileModel {
    #[must_use]
    pub fn snapshot(&self) -> Option<&crate::profiles::ProfileSnapshot> {
        self.snapshot.as_ref()
    }

    #[must_use]
    pub const fn pending(&self) -> bool {
        self.pending_request_id.is_some()
    }

    pub fn take_decision(&mut self) -> Option<crate::profiles::ProfileDecision> {
        self.last_decision.take()
    }

    pub fn create(&mut self, draft: crate::profiles::BrawlerDraft) -> bool {
        let Some(revision) = self.snapshot.as_ref().map(|snapshot| snapshot.revision) else {
            return false;
        };
        let Some(request_id) = self.begin_request() else {
            return false;
        };
        self.outbound
            .push_back(crate::profiles::ProfileCommand::CreateBrawler {
                request_id,
                expected_profile_revision: revision,
                draft,
            });
        true
    }

    pub fn edit(
        &mut self,
        brawler_id: crate::profiles::SavedBrawlerId,
        edit: crate::profiles::BrawlerEdit,
    ) -> bool {
        let Some(snapshot) = self.snapshot.as_ref() else {
            return false;
        };
        let Some(brawler_revision) = snapshot
            .brawlers
            .iter()
            .find(|brawler| brawler.id == brawler_id)
            .map(|brawler| brawler.revision)
        else {
            return false;
        };
        let profile_revision = snapshot.revision;
        let Some(request_id) = self.begin_request() else {
            return false;
        };
        self.outbound
            .push_back(crate::profiles::ProfileCommand::EditBrawler {
                request_id,
                expected_profile_revision: profile_revision,
                brawler_id,
                expected_brawler_revision: brawler_revision,
                edit,
            });
        true
    }

    pub fn select(&mut self, brawler_id: crate::profiles::SavedBrawlerId) -> bool {
        let Some(revision) = self.snapshot.as_ref().map(|snapshot| snapshot.revision) else {
            return false;
        };
        let Some(request_id) = self.begin_request() else {
            return false;
        };
        self.outbound
            .push_back(crate::profiles::ProfileCommand::SelectBrawler {
                request_id,
                expected_profile_revision: revision,
                brawler_id,
            });
        true
    }

    pub fn delete(&mut self, brawler_id: crate::profiles::SavedBrawlerId) -> bool {
        let Some(snapshot) = self.snapshot.as_ref() else {
            return false;
        };
        let Some(brawler_revision) = snapshot
            .brawlers
            .iter()
            .find(|brawler| brawler.id == brawler_id)
            .map(|brawler| brawler.revision)
        else {
            return false;
        };
        let profile_revision = snapshot.revision;
        let Some(request_id) = self.begin_request() else {
            return false;
        };
        self.outbound
            .push_back(crate::profiles::ProfileCommand::DeleteBrawler {
                request_id,
                expected_profile_revision: profile_revision,
                brawler_id,
                expected_brawler_revision: brawler_revision,
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
                bind_profile_snapshot,
                receive_profile_outcomes,
                send_profile_commands,
                create_headless_default_brawler,
            )
                .chain(),
        );
    }
}

fn bind_profile_snapshot(
    memberships: Query<&ClientLobbyMembership, (With<Client>, Changed<ClientLobbyMembership>)>,
    mut model: ResMut<ClientProfileModel>,
) {
    for membership in &memberships {
        model.snapshot = Some(membership.profile.clone());
        model.pending_request_id = None;
    }
}

fn receive_profile_outcomes(
    mut model: ResMut<ClientProfileModel>,
    mut clients: Query<
        (
            &mut ClientLobbyMembership,
            &mut MessageReceiver<crate::profiles::ProfileOutcome>,
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
            model.last_decision = Some(outcome.decision.clone());
            if let Some(snapshot) = outcome.snapshot {
                membership.profile = snapshot.clone();
                model.snapshot = Some(snapshot);
            }
        }
    }
}

fn send_profile_commands(
    mut model: ResMut<ClientProfileModel>,
    mut senders: Query<&mut MessageSender<crate::profiles::ProfileCommand>, With<Client>>,
) {
    let Ok(mut sender) = senders.single_mut() else {
        return;
    };
    while let Some(command) = model.outbound.pop_front() {
        sender.send::<crate::protocol::ProfileChannel>(command);
    }
}

#[allow(clippy::needless_pass_by_value)]
fn create_headless_default_brawler(
    config: Res<crate::config::ClientNetworkConfig>,
    mut model: ResMut<ClientProfileModel>,
) {
    if !config.headless
        || model.pending()
        || model
            .snapshot()
            .is_none_or(|snapshot| !snapshot.brawlers.is_empty())
    {
        return;
    }
    let weapon = config.build_preset.unwrap_or(1).clamp(1, 4);
    let _ = model.create(crate::profiles::BrawlerDraft {
        name: "Automation Brawler".into(),
        fighter_profile_id: crate::profiles::FighterProfileId(1),
        weapon_base_id: crate::profiles::WeaponBaseId(weapon),
        ultimate_id: crate::builds::UltimateDefinitionId(1),
        passive_ids: [
            crate::builds::PassiveDefinitionId(3),
            crate::builds::PassiveDefinitionId(4),
        ],
    });
}
