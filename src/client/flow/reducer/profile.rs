//! Brawler profile action resolution and profile-decision reconciliation.

use crate::client::flow::{
    actions::{FlowCommit, FlowUiAction, OverlayCommit},
    model::ClientOverlay,
    screens::{
        brawlers::{BrawlerCreationDraft, BrawlerEditDraft},
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

pub(super) fn resolve_profile_decision(
    profile: &mut crate::client::ClientProfileModel,
    dashboard_notice: &mut DashboardNotice,
    pending_created_brawler: &mut PendingCreatedBrawler,
    pending_edited_brawler: &mut PendingEditedBrawler,
    creation_draft: &mut BrawlerCreationDraft,
    brawler_edit: &mut BrawlerEditDraft,
    commit: &mut FlowCommit,
) {
    let Some(decision) = profile.take_decision() else {
        return;
    };
    let accepted = matches!(decision, crate::profiles::ProfileDecision::Accepted);
    dashboard_notice.0 = Some(match decision {
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
    });
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
        commit.overlay = Some(OverlayCommit::BrawlerDetails(brawler_id));
        commit.focus_index = Some(1);
    } else if !accepted {
        if pending_created_brawler.0.take().is_some() {
            creation_draft.inline_error.clone_from(&dashboard_notice.0);
            commit.overlay = Some(OverlayCommit::BrawlerCreation);
        }
        if pending_edited_brawler.0.take().is_some() {
            brawler_edit.inline_error.clone_from(&dashboard_notice.0);
            commit.overlay = Some(OverlayCommit::BrawlerEditor);
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
        | FlowUiAction::CycleCreationProfile
        | FlowUiAction::CycleCreationWeapon
        | FlowUiAction::CycleCreationUltimate
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
        FlowUiAction::CancelBrawlerEdit => {
            commit.overlay = brawler_edit
                .brawler_id
                .map_or(Some(OverlayCommit::BrawlerList), |id| {
                    Some(OverlayCommit::BrawlerDetails(id))
                });
        }
        FlowUiAction::CancelDeleteBrawler => {
            let details = match overlay {
                ClientOverlay::DeleteBrawlerConfirmation(id) => OverlayCommit::BrawlerDetails(*id),
                _ => OverlayCommit::BrawlerList,
            };
            commit.overlay = Some(details);
        }
        FlowUiAction::SelectBrawler(brawler_id) => {
            select_brawler(*brawler_id, queue, practice, profile, commit);
        }
        FlowUiAction::OpenBrawlerEditor(brawler_id) => {
            open_brawler_editor(*brawler_id, queue, practice, profile, brawler_edit, commit);
        }
        FlowUiAction::BeginBrawlerNameEdit => {
            brawler_edit.editing_name = true;
            brawler_edit.name_caret = brawler_edit.name.len();
            brawler_edit.inline_error = None;
        }
        FlowUiAction::CycleBrawlerUltimate => cycle_brawler_ultimate(profile, brawler_edit),
        FlowUiAction::CycleBrawlerPassiveOne => cycle_brawler_passive(profile, brawler_edit, 0),
        FlowUiAction::CycleBrawlerPassiveTwo => cycle_brawler_passive(profile, brawler_edit, 1),
        FlowUiAction::ConfirmBrawlerEdit => {
            confirm_brawler_edit(profile, brawler_edit, pending_edited_brawler);
        }
        FlowUiAction::DeleteBrawler(brawler_id) => {
            if profile_changes_blocked(queue, practice, profile) {
                return;
            }
            commit.overlay = Some(OverlayCommit::DeleteBrawlerConfirmation(*brawler_id));
        }
        FlowUiAction::ConfirmDeleteBrawler => {
            let ClientOverlay::DeleteBrawlerConfirmation(brawler_id) = overlay else {
                return;
            };
            let _ = profile.delete(*brawler_id);
            commit.overlay = Some(OverlayCommit::BrawlerList);
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
    match action {
        FlowUiAction::CreateBrawler => open_brawler_creation(
            queue,
            practice,
            profile,
            creation_draft,
            dashboard_notice,
            commit,
        ),
        FlowUiAction::CycleCreationProfile => {
            creation_draft.inline_error = None;
            let Some(catalog) = profile.catalog() else {
                return;
            };
            let index = catalog
                .fighter_profiles
                .iter()
                .position(|entry| entry.id == creation_draft.fighter_profile_id)
                .unwrap_or(0);
            creation_draft.fighter_profile_id =
                catalog.fighter_profiles[(index + 1) % catalog.fighter_profiles.len()].id;
        }
        FlowUiAction::CycleCreationWeapon => {
            creation_draft.inline_error = None;
            let Some(catalog) = profile.catalog() else {
                return;
            };
            let index = catalog
                .weapon_bases
                .iter()
                .position(|entry| entry.id == creation_draft.weapon_base_id)
                .unwrap_or(0);
            creation_draft.weapon_base_id =
                catalog.weapon_bases[(index + 1) % catalog.weapon_bases.len()].id;
        }
        FlowUiAction::CycleCreationUltimate => {
            creation_draft.inline_error = None;
            let Some(catalog) = profile.catalog() else {
                return;
            };
            let current = catalog
                .ultimates
                .iter()
                .position(|definition| definition.id == creation_draft.ultimate)
                .unwrap_or(0);
            creation_draft.ultimate = catalog.ultimates[(current + 1) % catalog.ultimates.len()].id;
        }
        FlowUiAction::CancelCreateBrawler => {
            commit.overlay = Some(OverlayCommit::BrawlerList);
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
        inline_error: None,
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
        || profile_changes_blocked(queue, practice, profile)
    {
        return;
    }
    let Some(snapshot) = profile.snapshot() else {
        return;
    };
    let ordinal = snapshot.next_brawler_ordinal;
    draft.inline_error = None;
    let Some((passive_one, passive_two)) = profile.catalog().and_then(|catalog| {
        let mut passives = catalog.selectable_passives().map(|entry| entry.id);
        Some((passives.next()?, passives.next()?))
    }) else {
        return;
    };
    if profile.create(crate::profiles::BrawlerDraft {
        name: format!("Brawler {ordinal}"),
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
            inline_error: None,
        };
        commit.overlay = Some(OverlayCommit::BrawlerEditor);
    }
}

fn cycle_brawler_ultimate(
    profile: &crate::client::ClientProfileModel,
    draft: &mut BrawlerEditDraft,
) {
    let Some(catalog) = profile.catalog() else {
        return;
    };
    let index = catalog
        .ultimates
        .iter()
        .position(|definition| definition.id == draft.ultimate_id)
        .unwrap_or(0);
    draft.ultimate_id = catalog.ultimates[(index + 1) % catalog.ultimates.len()].id;
}

fn cycle_brawler_passive(
    profile: &crate::client::ClientProfileModel,
    draft: &mut BrawlerEditDraft,
    slot: usize,
) {
    let Some(catalog) = profile.catalog() else {
        return;
    };
    let options: Vec<_> = catalog
        .selectable_passives()
        .map(|entry| entry.id)
        .collect();
    let other_slot = 1 - slot;
    let index = options
        .iter()
        .position(|id| *id == draft.passive_ids[slot])
        .unwrap_or(0);
    draft.passive_ids[slot] = options[(index + 1) % options.len()];
    if draft.passive_ids[slot] == draft.passive_ids[other_slot] {
        draft.passive_ids[other_slot] = options[(index + 2) % options.len()];
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
