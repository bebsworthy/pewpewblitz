//! Client mirror and command lifecycle for the server-owned saved-brawler profile.

use super::{Client, ClientLobbyMembership};
use bevy::prelude::*;
use lightyear::prelude::{MessageReceiver, MessageSender};
use std::collections::VecDeque;

#[derive(Resource, Clone, Debug, Default)]
pub struct ClientProfileModel {
    snapshot: Option<crate::profiles::ProfileSnapshot>,
    catalog: Option<crate::profiles::AdvertisedBrawlerCatalog>,
    pending_request_id: Option<u64>,
    pending_selection_id: Option<crate::profiles::SavedBrawlerId>,
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
    pub fn catalog(&self) -> Option<&crate::profiles::AdvertisedBrawlerCatalog> {
        self.catalog.as_ref()
    }

    #[must_use]
    pub const fn pending(&self) -> bool {
        self.pending_request_id.is_some()
    }

    #[must_use]
    pub fn selection_pending(&self, brawler_id: crate::profiles::SavedBrawlerId) -> bool {
        matches!(self.pending_selection_id, Some(pending_id) if pending_id == brawler_id)
    }

    pub fn take_decision(&mut self) -> Option<crate::profiles::ProfileDecision> {
        self.last_decision.take()
    }

    #[cfg(test)]
    pub(crate) fn set_snapshot_for_test(&mut self, snapshot: crate::profiles::ProfileSnapshot) {
        self.snapshot = Some(snapshot);
        self.pending_request_id = None;
        self.pending_selection_id = None;
        self.catalog = Some(
            crate::profiles::AdvertisedBrawlerCatalog::from_content(
                &crate::builds::BuildCatalog::embedded().expect("embedded build catalog"),
                &crate::combat::WeaponCatalog::embedded().expect("embedded weapon catalog"),
            )
            .expect("embedded brawler advertisement"),
        );
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
        self.pending_selection_id = Some(brawler_id);
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

    pub fn equip_weapon_parts(
        &mut self,
        brawler_id: crate::profiles::SavedBrawlerId,
        equipped_part_ids: [Option<crate::weapon_parts::WeaponPartInstanceId>;
            crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
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
            .push_back(crate::profiles::ProfileCommand::EquipWeaponParts {
                request_id,
                expected_profile_revision: profile_revision,
                brawler_id,
                expected_brawler_revision: brawler_revision,
                equipped_part_ids,
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
        self.pending_selection_id = None;
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
                create_automation_default_brawler,
            )
                .chain(),
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
        model.catalog = Some(membership.brawler_catalog.clone());
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
            model.pending_selection_id = None;
            model.last_decision = Some(outcome.decision.clone());
            if let Some(snapshot) = outcome.snapshot {
                let snapshot_valid = snapshot.validate_bounded().is_ok()
                    && model
                        .catalog
                        .as_ref()
                        .is_some_and(|catalog| catalog.validate_profile(&snapshot).is_ok());
                if !snapshot_valid {
                    model.last_decision = Some(crate::profiles::ProfileDecision::StorageFault);
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
fn create_automation_default_brawler(
    config: Res<crate::config::ClientNetworkConfig>,
    mut model: ResMut<ClientProfileModel>,
) {
    if !(config.headless || config.render_measurement.is_some())
        || model.pending()
        || model
            .snapshot()
            .is_none_or(|snapshot| !snapshot.brawlers.is_empty())
    {
        return;
    }
    let Some(catalog) = model.catalog() else {
        return;
    };
    let requested_weapon = config
        .weapon_preset
        .and_then(|id| catalog.weapon(crate::profiles::WeaponBaseId(id)))
        .or_else(|| catalog.weapon_bases.first());
    let Some((fighter, weapon, ultimate, passive_ids)) = catalog
        .fighter_profiles
        .first()
        .zip(requested_weapon)
        .zip(catalog.ultimates.first())
        .and_then(|((fighter, weapon), ultimate)| {
            let mut passives = catalog
                .selectable_passives()
                .map(|definition| definition.id);
            Some((
                fighter.id,
                weapon.id,
                ultimate.id,
                [passives.next()?, passives.next()?],
            ))
        })
    else {
        return;
    };
    let _ = model.create(crate::profiles::BrawlerDraft {
        name: "Automation Brawler".into(),
        fighter_profile_id: fighter,
        weapon_base_id: weapon,
        ultimate_id: ultimate,
        passive_ids,
    });
}
