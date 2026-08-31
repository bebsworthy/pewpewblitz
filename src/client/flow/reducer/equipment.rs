//! Weapon-equipment action resolution.

use super::profile::profile_changes_blocked;
use crate::client::flow::{
    actions::{FlowCommit, FlowUiAction, OverlayCommit},
    screens::brawlers::WeaponEquipmentDraft,
};

pub(super) fn resolve_equipment_action(
    action: &FlowUiAction,
    queue: &crate::client::ClientQueueModel,
    practice: &crate::client::ClientPracticeModel,
    profile: &mut crate::client::ClientProfileModel,
    draft: &mut WeaponEquipmentDraft,
    commit: &mut FlowCommit,
) {
    match action {
        FlowUiAction::OpenWeaponEquipment(brawler_id) => {
            open_weapon_equipment(*brawler_id, queue, practice, profile, draft, commit);
        }
        FlowUiAction::SelectEquipmentSlot(slot) => {
            if *slot < crate::weapon_parts::WEAPON_PART_SLOT_COUNT {
                draft.selected_slot = *slot;
                draft.inline_error = None;
            }
        }
        FlowUiAction::EquipWeaponPart(part_id) => {
            let Some(snapshot) = profile.snapshot() else {
                return;
            };
            let Some(brawler_id) = draft.brawler_id else {
                return;
            };
            if snapshot.brawlers.iter().any(|brawler| {
                brawler.id != brawler_id && brawler.equipped_part_ids.contains(&Some(*part_id))
            }) {
                draft.inline_error =
                    Some("That physical part is equipped on another brawler.".into());
                return;
            }
            for slot in &mut draft.equipped_part_ids {
                if *slot == Some(*part_id) {
                    *slot = None;
                }
            }
            draft.equipped_part_ids[draft.selected_slot] = Some(*part_id);
            draft.inline_error = None;
        }
        FlowUiAction::UnequipWeaponPart => {
            draft.equipped_part_ids[draft.selected_slot] = None;
            draft.inline_error = None;
        }
        FlowUiAction::ConfirmWeaponEquipment => {
            let Some(brawler_id) = draft.brawler_id else {
                return;
            };
            if profile.equip_weapon_parts(brawler_id, draft.equipped_part_ids) {
                commit.overlay = Some(OverlayCommit::BrawlerDetails(brawler_id));
            }
        }
        FlowUiAction::CancelWeaponEquipment => {
            commit.overlay = draft
                .brawler_id
                .map_or(Some(OverlayCommit::BrawlerList), |id| {
                    Some(OverlayCommit::BrawlerDetails(id))
                });
        }
        _ => unreachable!("flow action was routed to the wrong equipment reducer"),
    }
}

fn open_weapon_equipment(
    brawler_id: crate::profiles::SavedBrawlerId,
    queue: &crate::client::ClientQueueModel,
    practice: &crate::client::ClientPracticeModel,
    profile: &crate::client::ClientProfileModel,
    draft: &mut WeaponEquipmentDraft,
    commit: &mut FlowCommit,
) {
    if profile_changes_blocked(queue, practice, profile) {
        return;
    }
    let selected = profile.snapshot().and_then(|snapshot| {
        snapshot
            .brawlers
            .iter()
            .find(|brawler| brawler.id == brawler_id)
    });
    if let Some(brawler) = selected {
        *draft = WeaponEquipmentDraft {
            brawler_id: Some(brawler.id),
            equipped_part_ids: brawler.equipped_part_ids,
            selected_slot: 0,
            inline_error: None,
        };
        commit.overlay = Some(OverlayCommit::WeaponEquipment);
    }
}
