//! Character-select action resolution and profile-decision reconciliation.

use crate::flow::{
    actions::{FlowCommit, FlowUiAction, OverlayCommit},
    model::ClientOverlay,
    screens::{
        brawlers::{BrawlersNotice, focus_index_for_character},
        dashboard::{DASHBOARD_BUILD_INDEX, DashboardNotice},
    },
};

/// Reconciles the server's decision on the last sent pick: a refusal is shown on the Brawlers
/// screen (or the Dashboard), an acceptance celebrates the pressed control.
pub(super) fn resolve_profile_decision(
    overlay: &ClientOverlay,
    profile: &mut crate::ClientProfileModel,
    dashboard_notice: &mut DashboardNotice,
    brawlers_notice: &mut BrawlersNotice,
    commit: &mut FlowCommit,
) {
    let Some(decision) = profile.take_decision() else {
        return;
    };
    let accepted = matches!(decision, brawler::profiles::ProfileDecision::Accepted);
    let message = (!accepted).then(|| profile_decision_message(&decision));
    if matches!(overlay, ClientOverlay::Brawlers) {
        brawlers_notice.0 = message;
        commit.celebrate = accepted;
    } else if message.is_some() {
        dashboard_notice.0 = message;
    }
}

pub(super) fn resolve_profile_action(
    action: &FlowUiAction,
    overlay: &ClientOverlay,
    queue: &crate::ClientQueueModel,
    practice: &crate::ClientPracticeModel,
    profile: &mut crate::ClientProfileModel,
    brawlers_notice: &mut BrawlersNotice,
    commit: &mut FlowCommit,
) {
    match action {
        FlowUiAction::OpenBrawlers => {
            brawlers_notice.0 = None;
            commit.overlay = Some(OverlayCommit::Brawlers);
            commit.focus_index = Some(profile.snapshot().zip(profile.catalog()).map_or(
                0,
                |(snapshot, catalog)| {
                    focus_index_for_character(catalog, snapshot.selected_character_id)
                },
            ));
        }
        FlowUiAction::CloseBrawlers => {
            commit.overlay = Some(OverlayCommit::Clear);
            commit.focus_index = Some(DASHBOARD_BUILD_INDEX);
        }
        FlowUiAction::PickCharacter(_)
        | FlowUiAction::PickUltimate(_)
        | FlowUiAction::PickPassive(_) => {
            if !matches!(overlay, ClientOverlay::Brawlers)
                || profile_changes_blocked(queue, practice, profile)
            {
                return;
            }
            let Some(choice) = picked_choice(action, profile) else {
                return;
            };
            if profile.select(choice) {
                brawlers_notice.0 = None;
            }
        }
        _ => unreachable!("flow action was routed to the wrong profile reducer"),
    }
}

/// The pick one press asks for, or `None` when it would not change the selection.
fn picked_choice(
    action: &FlowUiAction,
    profile: &crate::ClientProfileModel,
) -> Option<brawler::characters::CharacterChoice> {
    let (snapshot, catalog) = (profile.snapshot()?, profile.catalog()?);
    let current = snapshot.selection();
    let choice = match *action {
        FlowUiAction::PickCharacter(id) => catalog.choice_for(snapshot, id)?,
        FlowUiAction::PickUltimate(ultimate_id) => brawler::characters::CharacterChoice {
            ultimate_id,
            ..current
        },
        FlowUiAction::PickPassive(passive_id) => brawler::characters::CharacterChoice {
            passive_id,
            ..current
        },
        _ => return None,
    };
    (choice != current && catalog.accepts(choice)).then_some(choice)
}

/// A pick waits while one is in flight and is refused while the player is queued or starting
/// Practice (the server would answer `QueueLocked`).
pub(super) fn profile_changes_blocked(
    queue: &crate::ClientQueueModel,
    practice: &crate::ClientPracticeModel,
    profile: &crate::ClientProfileModel,
) -> bool {
    profile.pending()
        || queue.membership().is_some()
        || queue.pending().is_some()
        || practice.pending()
}

fn profile_decision_message(decision: &brawler::profiles::ProfileDecision) -> String {
    match decision {
        brawler::profiles::ProfileDecision::Accepted => "Pick saved.".to_string(),
        brawler::profiles::ProfileDecision::InvalidRequest => {
            "That character does not offer that pick.".to_string()
        }
        brawler::profiles::ProfileDecision::StaleRevision => {
            "The profile changed; try again.".to_string()
        }
        brawler::profiles::ProfileDecision::QueueLocked => {
            "Leave the queue before changing your character.".to_string()
        }
        brawler::profiles::ProfileDecision::TemporarilyUnavailable => {
            "Profile storage is temporarily unavailable; try again.".to_string()
        }
        brawler::profiles::ProfileDecision::StorageFault => {
            "The pick could not be saved safely; your profile was preserved.".to_string()
        }
    }
}
