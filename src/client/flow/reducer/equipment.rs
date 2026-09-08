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
    if !matches!(action, FlowUiAction::OpenWeaponEquipment(_))
        && profile_changes_blocked(queue, practice, profile)
    {
        return;
    }
    match action {
        FlowUiAction::OpenWeaponEquipment(brawler_id) => {
            open_weapon_equipment(*brawler_id, queue, practice, profile, draft, commit);
        }
        FlowUiAction::SelectEquipmentSlot(slot) => {
            if *slot < crate::weapon_parts::WEAPON_PART_SLOT_COUNT {
                draft.selected_slot = *slot;
                draft.picker_open = true;
                commit.focus_index = Some(
                    draft.equipped_part_ids[*slot]
                        .and_then(|id| {
                            profile
                                .snapshot()
                                .and_then(|s| s.inventory.iter().position(|p| p.id == id))
                        })
                        .map_or(0, |i| i + 1),
                );
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
            save_equipment(profile, draft, commit);
        }
        FlowUiAction::UnequipWeaponPart => {
            draft.equipped_part_ids[draft.selected_slot] = None;
            save_equipment(profile, draft, commit);
        }
        FlowUiAction::CancelWeaponEquipment => {
            if draft.picker_open {
                draft.picker_open = false;
                draft.inline_error = None;
                commit.focus_index = Some(draft.selected_slot);
                return;
            }
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
            picker_open: false,
            awaiting_save: false,
            inline_error: None,
        };
        commit.overlay = Some(OverlayCommit::WeaponEquipment);
        commit.focus_index = Some(0);
    }
}

fn save_equipment(
    profile: &mut crate::client::ClientProfileModel,
    draft: &mut WeaponEquipmentDraft,
    commit: &mut FlowCommit,
) {
    let Some(id) = draft.brawler_id else {
        return;
    };
    if profile
        .snapshot()
        .and_then(|s| s.brawlers.iter().find(|b| b.id == id))
        .is_some_and(|b| b.equipped_part_ids == draft.equipped_part_ids)
    {
        draft.picker_open = false;
        draft.inline_error = None;
        commit.focus_index = Some(draft.selected_slot);
    } else if profile.equip_weapon_parts(id, draft.equipped_part_ids) {
        draft.awaiting_save = true;
        draft.inline_error = None;
    } else {
        draft.inline_error = Some("Could not send this change. Try again.".into());
    }
}
