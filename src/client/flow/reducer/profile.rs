//! Brawler profile action resolution and profile-decision reconciliation.

use crate::client::flow::{
    actions::{FlowCommit, FlowUiAction, OverlayCommit},
    model::ClientOverlay,
    screens::{
        brawlers::{AbilityPicker, BrawlerCreationDraft, BrawlerEditDraft},
        dashboard::{DASHBOARD_BUILD_INDEX, DashboardNotice},
    },
};
use bevy::prelude::Resource;

#[derive(Resource, Default)]
pub(in crate::client::flow) struct PendingCreatedBrawler(pub(in crate::client::flow) Option<u64>);

#[derive(Resource, Default)]
pub(in crate::client::flow) struct PendingEditedBrawler(
    pub(in crate::client::flow) Option<crate::profiles::SavedBrawlerId>,
);

#[derive(Resource, Default)]
pub(in crate::client::flow) struct PendingDeletedBrawler {
    pub(in crate::client::flow) id: Option<crate::profiles::SavedBrawlerId>,
    pub(in crate::client::flow) error: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_profile_decision(
    profile: &mut crate::client::ClientProfileModel,
    dashboard_notice: &mut DashboardNotice,
    pending_created_brawler: &mut PendingCreatedBrawler,
    pending_edited_brawler: &mut PendingEditedBrawler,
    pending_deleted_brawler: &mut PendingDeletedBrawler,
    creation_draft: &mut BrawlerCreationDraft,
    brawler_edit: &mut BrawlerEditDraft,
    equipment: &mut crate::client::flow::screens::brawlers::WeaponEquipmentDraft,
    commit: &mut FlowCommit,
) {
    let Some(decision) = profile.take_decision() else {
        return;
    };
    let accepted = matches!(decision, crate::profiles::ProfileDecision::Accepted);
    dashboard_notice.0 = Some(profile_decision_message(&decision));
    if let Some(id) = pending_deleted_brawler.id.take() {
        pending_deleted_brawler.error = if accepted {
            None
        } else {
            dashboard_notice.0.take()
        };
        dashboard_notice.0 = None;
        commit.overlay = Some(if accepted {
            OverlayCommit::BrawlerList
        } else {
            OverlayCommit::DeleteBrawlerConfirmation(id)
        });
        commit.focus_index = Some(0);
        return;
    }
    if equipment.awaiting_save {
        equipment.awaiting_save = false;
        if let Some(saved) = profile.snapshot().and_then(|s| {
            s.brawlers
                .iter()
                .find(|b| Some(b.id) == equipment.brawler_id)
        }) {
            equipment.equipped_part_ids = saved.equipped_part_ids;
        }
        equipment.inline_error = if accepted {
            None
        } else {
            dashboard_notice.0.take()
        };
        dashboard_notice.0 = None;
        equipment.picker_open = !accepted;
        commit.overlay = Some(OverlayCommit::WeaponEquipment);
        commit.focus_index = Some(if accepted { equipment.selected_slot } else { 0 });
        return;
    }
    if accepted
        && let Some(ordinal) = pending_created_brawler.0.take()
        && let Some(created) = profile.snapshot().and_then(|snapshot| {
            snapshot
                .brawlers
                .iter()
                .find(|brawler| brawler.creation_ordinal == ordinal)
        })
    {
        dashboard_notice.0 = Some(format!("Created {}.", created.name));
        commit.overlay = Some(OverlayCommit::BrawlerDetails(created.id));
        commit.focus_index = Some(0);
    } else if accepted && let Some(brawler_id) = pending_edited_brawler.0.take() {
        accept_brawler_edit(profile, brawler_edit, brawler_id, commit);
    } else if !accepted {
        if pending_created_brawler.0.take().is_some() {
            creation_draft.inline_error.clone_from(&dashboard_notice.0);
            commit.overlay = Some(OverlayCommit::BrawlerCreation);
        }
        if pending_edited_brawler.0.take().is_some() {
            brawler_edit.inline_error.clone_from(&dashboard_notice.0);
            commit.overlay = if brawler_edit.inline_rename {
                brawler_edit.editing_name = true;
                brawler_edit.brawler_id.map(OverlayCommit::BrawlerDetails)
            } else {
                Some(OverlayCommit::BrawlerEditor)
            };
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_profile_action(
    action: &FlowUiAction,
    overlay: &ClientOverlay,
    queue: &crate::client::ClientQueueModel,
    practice: &crate::client::ClientPracticeModel,
    profile: &mut crate::client::ClientProfileModel,
    creation_draft: &mut BrawlerCreationDraft,
    brawler_edit: &mut BrawlerEditDraft,
    pending_created_brawler: &mut PendingCreatedBrawler,
    pending_edited_brawler: &mut PendingEditedBrawler,
    pending_deleted_brawler: &mut PendingDeletedBrawler,
    dashboard_notice: &mut DashboardNotice,
    commit: &mut FlowCommit,
) {
    match action {
        FlowUiAction::OpenBrawlerList | FlowUiAction::BackToBrawlerList => {
            commit.overlay = Some(OverlayCommit::BrawlerList);
            commit.focus_index = Some(0);
        }
        FlowUiAction::CloseBrawlerList => {
            commit.overlay = Some(OverlayCommit::Clear);
            commit.focus_index = Some(DASHBOARD_BUILD_INDEX);
        }
        FlowUiAction::OpenBrawlerDetails(brawler_id) => {
            commit.overlay = Some(OverlayCommit::BrawlerDetails(*brawler_id));
            commit.focus_index = Some(0);
        }
        action @ (FlowUiAction::CreateBrawler
        | FlowUiAction::OpenCreationFighter
        | FlowUiAction::OpenCreationWeapon
        | FlowUiAction::ChooseCreationFighter(_)
        | FlowUiAction::ChooseCreationWeapon(_)
        | FlowUiAction::ChooseCreationUltimate(_)
        | FlowUiAction::EditCreationName
        | FlowUiAction::OpenCreationUltimate
        | FlowUiAction::CancelCreateBrawler
        | FlowUiAction::ConfirmCreateBrawler) => resolve_brawler_creation_action(
            action,
            overlay,
            queue,
            practice,
            profile,
            creation_draft,
            pending_created_brawler,
            dashboard_notice,
            commit,
        ),
        FlowUiAction::CancelBrawlerEdit => cancel_brawler_edit(profile, brawler_edit, commit),
        FlowUiAction::CancelDeleteBrawler => {
            cancel_brawler_deletion(overlay, profile, pending_deleted_brawler, commit);
        }
        FlowUiAction::SelectBrawler(brawler_id) => {
            select_brawler(*brawler_id, queue, practice, profile, commit);
        }
        FlowUiAction::OpenBrawlerEditor(brawler_id) => {
            open_brawler_editor(*brawler_id, queue, practice, profile, brawler_edit, commit);
        }
        FlowUiAction::BeginBrawlerNameEdit => {
            if let ClientOverlay::BrawlerDetails(id) = overlay {
                open_brawler_editor(*id, queue, practice, profile, brawler_edit, commit);
                if commit.overlay.is_none() {
                    return;
                }
                commit.overlay = Some(OverlayCommit::BrawlerDetails(*id));
                brawler_edit.inline_rename = true;
            }
            brawler_edit.editing_name = true;
            brawler_edit.name_caret = brawler_edit.name.len();
            brawler_edit.inline_error = None;
        }
        action @ (FlowUiAction::OpenBrawlerUltimate
        | FlowUiAction::OpenBrawlerPassive(_)
        | FlowUiAction::ChooseBrawlerUltimate(_)
        | FlowUiAction::ChooseBrawlerPassive(_)) => {
            if !matches!(overlay, ClientOverlay::BrawlerEditor)
                || profile_changes_blocked(queue, practice, profile)
            {
                return;
            }
            resolve_ability_action(
                action,
                profile,
                brawler_edit,
                pending_edited_brawler,
                commit,
            );
        }
        FlowUiAction::ConfirmBrawlerEdit => {
            confirm_brawler_edit(profile, brawler_edit, pending_edited_brawler);
            if brawler_edit.inline_rename && profile.pending() {
                brawler_edit.editing_name = false;
            }
        }
        FlowUiAction::DeleteBrawler(brawler_id) => {
            if profile_changes_blocked(queue, practice, profile) {
                return;
            }
            pending_deleted_brawler.error = None;
            commit.overlay = Some(OverlayCommit::DeleteBrawlerConfirmation(*brawler_id));
            commit.focus_index = Some(0);
        }
        FlowUiAction::ConfirmDeleteBrawler => {
            if !profile_changes_blocked(queue, practice, profile) {
                submit_brawler_deletion(overlay, profile, pending_deleted_brawler);
            }
        }
        _ => unreachable!("flow action was routed to the wrong profile reducer"),
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_brawler_creation_action(
    action: &FlowUiAction,
    overlay: &ClientOverlay,
    queue: &crate::client::ClientQueueModel,
    practice: &crate::client::ClientPracticeModel,
    profile: &mut crate::client::ClientProfileModel,
    creation_draft: &mut BrawlerCreationDraft,
    pending_created_brawler: &mut PendingCreatedBrawler,
    dashboard_notice: &mut DashboardNotice,
    commit: &mut FlowCommit,
) {
    if !matches!(action, FlowUiAction::CreateBrawler)
        && (!matches!(overlay, ClientOverlay::BrawlerCreation)
            || profile_changes_blocked(queue, practice, profile))
    {
        return;
    }
    if !matches!(action, FlowUiAction::EditCreationName) {
        creation_draft.editing_name = false;
    }
    match action {
        FlowUiAction::CreateBrawler => open_brawler_creation(
            queue,
            practice,
            profile,
            creation_draft,
            dashboard_notice,
            commit,
        ),
        FlowUiAction::OpenCreationFighter => {
            creation_draft.picker = Some(super::super::screens::brawlers::CreationPicker::Fighter);
        }
        FlowUiAction::OpenCreationWeapon => {
            creation_draft.picker = Some(super::super::screens::brawlers::CreationPicker::Weapon);
        }
        FlowUiAction::OpenCreationUltimate => {
            creation_draft.picker = Some(super::super::screens::brawlers::CreationPicker::Ultimate);
        }
        FlowUiAction::EditCreationName => {
            creation_draft.editing_name = true;
            creation_draft.name_caret = creation_draft.name.len();
        }
        FlowUiAction::ChooseCreationFighter(id) => {
            if profile.catalog().is_some_and(|c| c.fighter(*id).is_some()) {
                creation_draft.fighter_profile_id = *id;
                creation_draft.picker = None;
                commit.focus_index = Some(0);
            }
        }
        FlowUiAction::ChooseCreationWeapon(id) => {
            if profile.catalog().is_some_and(|c| c.weapon(*id).is_some()) {
                creation_draft.weapon_base_id = *id;
                creation_draft.picker = None;
                commit.focus_index = Some(1);
            }
        }
        FlowUiAction::ChooseCreationUltimate(id) => {
            if profile.catalog().is_some_and(|c| c.ultimate(*id).is_some()) {
                creation_draft.ultimate = *id;
                creation_draft.picker = None;
                commit.focus_index = Some(2);
            }
        }
        FlowUiAction::CancelCreateBrawler => {
            if creation_draft.picker.take().is_none() {
                commit.overlay = Some(OverlayCommit::BrawlerList);
            }
        }
        FlowUiAction::ConfirmCreateBrawler => confirm_brawler_creation(
            overlay,
            queue,
            practice,
            profile,
            creation_draft,
            pending_created_brawler,
        ),
        _ => unreachable!("flow action was routed to the wrong creation reducer"),
    }
}

fn open_brawler_creation(
    queue: &crate::client::ClientQueueModel,
    practice: &crate::client::ClientPracticeModel,
    profile: &crate::client::ClientProfileModel,
    draft: &mut BrawlerCreationDraft,
    dashboard_notice: &mut DashboardNotice,
    commit: &mut FlowCommit,
) {
    if profile_changes_blocked(queue, practice, profile) {
        return;
    }
    let Some(snapshot) = profile.snapshot() else {
        return;
    };
    let Some(catalog) = profile.catalog() else {
        return;
    };
    if snapshot.brawlers.len() >= usize::from(catalog.limits.maximum_saved_brawlers) {
        dashboard_notice.0 = Some(format!(
            "Brawler limit reached ({}).",
            catalog.limits.maximum_saved_brawlers
        ));
        commit.overlay = Some(OverlayCommit::Clear);
        return;
    }
    let (Some(fighter), Some(weapon), Some(ultimate)) = (
        catalog.fighter_profiles.first(),
        catalog.weapon_bases.first(),
        catalog.ultimates.first(),
    ) else {
        return;
    };
    *draft = BrawlerCreationDraft {
        fighter_profile_id: fighter.id,
        weapon_base_id: weapon.id,
        ultimate: ultimate.id,
        name: format!("Brawler {}", snapshot.next_brawler_ordinal),
        ..BrawlerCreationDraft::default()
    };
    commit.overlay = Some(OverlayCommit::BrawlerCreation);
}

fn confirm_brawler_creation(
    overlay: &ClientOverlay,
    queue: &crate::client::ClientQueueModel,
    practice: &crate::client::ClientPracticeModel,
    profile: &mut crate::client::ClientProfileModel,
    draft: &mut BrawlerCreationDraft,
    pending: &mut PendingCreatedBrawler,
) {
    if !matches!(overlay, ClientOverlay::BrawlerCreation)
        || draft.picker.is_some()
        || profile_changes_blocked(queue, practice, profile)
    {
        return;
    }
    let Some(snapshot) = profile.snapshot() else {
        return;
    };
    match crate::lobby::normalize_proposed_display_name(&draft.name) {
        Ok(name) => draft.name = name,
        Err(error) => {
            draft.inline_error = Some(format!("Invalid name: {error}"));
            return;
        }
    }
    let ordinal = snapshot.next_brawler_ordinal;
    draft.inline_error = None;
    let Some((passive_one, passive_two)) = profile.catalog().and_then(|catalog| {
        let mut passives = catalog.selectable_passives().map(|entry| entry.id);
        Some((passives.next()?, passives.next()?))
    }) else {
        return;
    };
    if profile.create(crate::profiles::BrawlerDraft {
        name: draft.name.clone(),
        fighter_profile_id: draft.fighter_profile_id,
        weapon_base_id: draft.weapon_base_id,
        ultimate_id: draft.ultimate,
        passive_ids: [passive_one, passive_two],
    }) {
        pending.0 = Some(ordinal);
    }
}

pub(super) fn profile_changes_blocked(
    queue: &crate::client::ClientQueueModel,
    practice: &crate::client::ClientPracticeModel,
    profile: &crate::client::ClientProfileModel,
) -> bool {
    queue.membership().is_some()
        || queue.pending().is_some()
        || practice.pending()
        || profile.pending()
}

fn select_brawler(
    brawler_id: crate::profiles::SavedBrawlerId,
    queue: &crate::client::ClientQueueModel,
    practice: &crate::client::ClientPracticeModel,
    profile: &mut crate::client::ClientProfileModel,
    commit: &mut FlowCommit,
) {
    if profile_changes_blocked(queue, practice, profile) {
        return;
    }
    let already_selected = profile
        .snapshot()
        .is_some_and(|snapshot| snapshot.selected_brawler_id == Some(brawler_id));
    if already_selected || profile.select(brawler_id) {
        commit.overlay = Some(OverlayCommit::Clear);
        commit.focus_index = Some(DASHBOARD_BUILD_INDEX);
    }
}

fn open_brawler_editor(
    brawler_id: crate::profiles::SavedBrawlerId,
    queue: &crate::client::ClientQueueModel,
    practice: &crate::client::ClientPracticeModel,
    profile: &crate::client::ClientProfileModel,
    draft: &mut BrawlerEditDraft,
    commit: &mut FlowCommit,
) {
    if profile_changes_blocked(queue, practice, profile) {
        return;
    }
    let selected = profile
        .snapshot()
        .and_then(|snapshot| {
            snapshot
                .brawlers
                .iter()
                .find(|brawler| brawler.id == brawler_id)
        })
        .cloned();
    if let Some(brawler) = selected {
        *draft = BrawlerEditDraft {
            brawler_id: Some(brawler.id),
            name_caret: brawler.name.len(),
            name: brawler.name,
            fighter_profile_id: brawler.fighter_profile_id,
            weapon_base_id: brawler.weapon_base_id,
            ultimate_id: brawler.ultimate_id,
            passive_ids: brawler.passive_ids,
            editing_name: false,
            inline_rename: false,
            picker: None,
            inline_error: None,
        };
        commit.overlay = Some(OverlayCommit::BrawlerEditor);
    }
}

fn ability_slot_index(picker: AbilityPicker) -> usize {
    match picker {
        AbilityPicker::Ultimate => 0,
        AbilityPicker::Passive(slot) => slot + 1,
    }
}

fn resolve_ability_action(
    action: &FlowUiAction,
    profile: &mut crate::client::ClientProfileModel,
    draft: &mut BrawlerEditDraft,
    pending: &mut PendingEditedBrawler,
    commit: &mut FlowCommit,
) {
    let Some(catalog) = profile.catalog() else {
        return;
    };
    let picker = match action {
        FlowUiAction::OpenBrawlerUltimate => Some(AbilityPicker::Ultimate),
        FlowUiAction::OpenBrawlerPassive(slot) if *slot < 2 => Some(AbilityPicker::Passive(*slot)),
        _ => None,
    };
    if let Some(picker) = picker {
        draft.picker = Some(picker);
        draft.inline_error = None;
        commit.focus_index = Some(
            match picker {
                AbilityPicker::Ultimate => catalog
                    .ultimates
                    .iter()
                    .position(|u| u.id == draft.ultimate_id),
                AbilityPicker::Passive(slot) => catalog
                    .passives
                    .iter()
                    .position(|p| p.id == draft.passive_ids[slot]),
            }
            .unwrap_or(0),
        );
        return;
    }
    let mut edit = crate::profiles::BrawlerEdit {
        name: draft.name.clone(),
        ultimate_id: draft.ultimate_id,
        passive_ids: draft.passive_ids,
    };
    match (action, draft.picker) {
        (FlowUiAction::ChooseBrawlerUltimate(id), Some(AbilityPicker::Ultimate)) => {
            edit.ultimate_id = *id;
        }
        (FlowUiAction::ChooseBrawlerPassive(id), Some(AbilityPicker::Passive(slot))) => {
            edit.passive_ids[slot] = *id;
        }
        _ => return,
    }
    if let Err(error) = catalog.validate_edit(&edit) {
        draft.inline_error = Some(
            match error {
                crate::profiles::ProfileModelError::DuplicatePassive => {
                    "This passive is already used in the other slot."
                }
                _ => "This ability is unavailable for this brawler.",
            }
            .into(),
        );
        return;
    }
    if edit.ultimate_id == draft.ultimate_id && edit.passive_ids == draft.passive_ids {
        if let Some(picker) = draft.picker.take() {
            commit.focus_index = Some(ability_slot_index(picker));
        }
        draft.inline_error = None;
        return;
    }
    if let Some(id) = draft.brawler_id
        && profile.edit(id, edit)
    {
        pending.0 = Some(id);
        draft.inline_error = None;
    }
}

fn confirm_brawler_edit(
    profile: &mut crate::client::ClientProfileModel,
    draft: &mut BrawlerEditDraft,
    pending: &mut PendingEditedBrawler,
) {
    let Ok(name) = crate::lobby::normalize_proposed_display_name(&draft.name) else {
        draft.inline_error = Some("Enter a valid brawler name.".to_string());
        return;
    };
    let Some(brawler_id) = draft.brawler_id else {
        return;
    };
    if profile.edit(
        brawler_id,
        crate::profiles::BrawlerEdit {
            name,
            ultimate_id: draft.ultimate_id,
            passive_ids: draft.passive_ids,
        },
    ) {
        pending.0 = Some(brawler_id);
        draft.inline_error = None;
    }
}

fn accept_brawler_edit(
    profile: &crate::client::ClientProfileModel,
    brawler_edit: &mut BrawlerEditDraft,
    brawler_id: crate::profiles::SavedBrawlerId,
    commit: &mut FlowCommit,
) {
    if let Some(picker) = brawler_edit.picker.take() {
        if let Some(saved) = profile
            .snapshot()
            .and_then(|s| s.brawlers.iter().find(|b| b.id == brawler_id))
        {
            brawler_edit.ultimate_id = saved.ultimate_id;
            brawler_edit.passive_ids = saved.passive_ids;
        }
        commit.overlay = Some(OverlayCommit::BrawlerEditor);
        commit.focus_index = Some(ability_slot_index(picker));
    } else {
        brawler_edit.inline_rename = false;
        brawler_edit.editing_name = false;
        commit.overlay = Some(OverlayCommit::BrawlerDetails(brawler_id));
        commit.focus_index = Some(1);
    }
}

fn cancel_brawler_edit(
    profile: &crate::client::ClientProfileModel,
    brawler_edit: &mut BrawlerEditDraft,
    commit: &mut FlowCommit,
) {
    if profile.pending() {
        return;
    }
    if let Some(picker) = brawler_edit.picker.take() {
        brawler_edit.inline_error = None;
        commit.focus_index = Some(ability_slot_index(picker));
        return;
    }
    commit.overlay = brawler_edit
        .brawler_id
        .map_or(Some(OverlayCommit::BrawlerList), |id| {
            Some(OverlayCommit::BrawlerDetails(id))
        });
}

fn profile_decision_message(decision: &crate::profiles::ProfileDecision) -> String {
    match decision {
        crate::profiles::ProfileDecision::Accepted => "Profile saved.".to_string(),
        crate::profiles::ProfileDecision::InvalidRequest => {
            "That brawler change is not valid.".to_string()
        }
        crate::profiles::ProfileDecision::StaleRevision => {
            "The profile changed; review it and try again.".to_string()
        }
        crate::profiles::ProfileDecision::MissingBrawler => {
            "That brawler no longer exists.".to_string()
        }
        crate::profiles::ProfileDecision::CapacityReached => {
            "Brawler limit reached (16).".to_string()
        }
        crate::profiles::ProfileDecision::QueueLocked => {
            "Leave the queue before changing a brawler.".to_string()
        }
        crate::profiles::ProfileDecision::TemporarilyUnavailable => {
            "Profile storage is temporarily unavailable; try again.".to_string()
        }
        crate::profiles::ProfileDecision::StorageFault => {
            "The profile could not be saved safely; owned data was preserved.".to_string()
        }
        crate::profiles::ProfileDecision::MissingPart => {
            "That weapon part is no longer in this inventory.".to_string()
        }
        crate::profiles::ProfileDecision::PartAlreadyEquipped => {
            "That physical part is already equipped on a brawler.".to_string()
        }
        crate::profiles::ProfileDecision::IncompatibleWeapon => {
            "Those parts do not form a valid weapon configuration.".to_string()
        }
        crate::profiles::ProfileDecision::IncompatibleBuild => {
            "Choose only one elemental resistance passive for this brawler.".to_string()
        }
    }
}

fn cancel_brawler_deletion(
    overlay: &ClientOverlay,
    profile: &crate::client::ClientProfileModel,
    pending: &mut PendingDeletedBrawler,
    commit: &mut FlowCommit,
) {
    if profile.pending() {
        return;
    }
    pending.error = None;
    commit.overlay = Some(match overlay {
        ClientOverlay::DeleteBrawlerConfirmation(id) => OverlayCommit::BrawlerDetails(*id),
        _ => OverlayCommit::BrawlerList,
    });
}

fn submit_brawler_deletion(
    overlay: &ClientOverlay,
    profile: &mut crate::client::ClientProfileModel,
    pending: &mut PendingDeletedBrawler,
) {
    if let ClientOverlay::DeleteBrawlerConfirmation(id) = overlay
        && profile.delete(*id)
    {
        pending.id = Some(*id);
        pending.error = None;
    }
}
