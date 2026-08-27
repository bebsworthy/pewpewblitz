//! Pure focus navigation, overlay filtering, and action-priority policy for flow input.

use super::{
    BrawlerEditDraft, ClientOverlay, DASHBOARD_BUILD_INDEX, DASHBOARD_GAME_INDEX,
    DASHBOARD_MENU_INDEX, DASHBOARD_PLAY_INDEX, DASHBOARD_PRACTICE_INDEX, DASHBOARD_SETTINGS_INDEX,
    DashboardLayoutClass, DashboardNavigationDirection, EditingField, FlowButton, FlowUiAction,
    PendingFlowActions, ServerSelectModel,
};
use unicode_segmentation::UnicodeSegmentation as _;

pub(super) fn repair_dashboard_focus(current: usize, available: &[usize]) -> usize {
    if available.contains(&current) {
        return current;
    }
    [
        DASHBOARD_PLAY_INDEX,
        DASHBOARD_PRACTICE_INDEX,
        DASHBOARD_GAME_INDEX,
        DASHBOARD_BUILD_INDEX,
        DASHBOARD_SETTINGS_INDEX,
        DASHBOARD_MENU_INDEX,
    ]
    .into_iter()
    .find(|index| available.contains(index))
    .unwrap_or(current)
}

pub(super) fn dashboard_focus_neighbor(
    class: DashboardLayoutClass,
    current: usize,
    direction: DashboardNavigationDirection,
    available: &[usize],
) -> usize {
    let raw_neighbor = |index| match direction {
        DashboardNavigationDirection::Left => match index {
            DASHBOARD_PLAY_INDEX => Some(DASHBOARD_PRACTICE_INDEX),
            DASHBOARD_PRACTICE_INDEX => Some(DASHBOARD_GAME_INDEX),
            DASHBOARD_MENU_INDEX => Some(DASHBOARD_SETTINGS_INDEX),
            _ => None,
        },
        DashboardNavigationDirection::Right => match index {
            DASHBOARD_GAME_INDEX => Some(DASHBOARD_PRACTICE_INDEX),
            DASHBOARD_PRACTICE_INDEX => Some(DASHBOARD_PLAY_INDEX),
            DASHBOARD_SETTINGS_INDEX => Some(DASHBOARD_MENU_INDEX),
            _ => None,
        },
        DashboardNavigationDirection::Up => match (class, index) {
            (DashboardLayoutClass::Compact, DASHBOARD_PLAY_INDEX) => Some(DASHBOARD_PRACTICE_INDEX),
            (DashboardLayoutClass::Compact, DASHBOARD_PRACTICE_INDEX) => Some(DASHBOARD_GAME_INDEX),
            (_, DASHBOARD_PLAY_INDEX | DASHBOARD_PRACTICE_INDEX | DASHBOARD_GAME_INDEX) => {
                Some(DASHBOARD_BUILD_INDEX)
            }
            (_, DASHBOARD_BUILD_INDEX) => Some(DASHBOARD_SETTINGS_INDEX),
            _ => None,
        },
        DashboardNavigationDirection::Down => match (class, index) {
            (_, DASHBOARD_SETTINGS_INDEX | DASHBOARD_MENU_INDEX) => Some(DASHBOARD_BUILD_INDEX),
            (_, DASHBOARD_BUILD_INDEX) => Some(DASHBOARD_GAME_INDEX),
            (DashboardLayoutClass::Compact, DASHBOARD_GAME_INDEX) => Some(DASHBOARD_PRACTICE_INDEX),
            (DashboardLayoutClass::Compact, DASHBOARD_PRACTICE_INDEX) => Some(DASHBOARD_PLAY_INDEX),
            _ => None,
        },
    };
    let mut candidate = current;
    while let Some(next) = raw_neighbor(candidate) {
        if available.contains(&next) {
            return next;
        }
        candidate = next;
    }
    current
}

pub(super) fn overlay_allows_button(overlay: &ClientOverlay, button: &FlowButton) -> bool {
    match overlay {
        ClientOverlay::Error(_)
        | ClientOverlay::Confirmation(_)
        | ClientOverlay::BrawlerCreation
        | ClientOverlay::BrawlerEditor
        | ClientOverlay::BrawlerList
        | ClientOverlay::BrawlerDetails(_)
        | ClientOverlay::WeaponEquipment
        | ClientOverlay::DeleteBrawlerConfirmation(_)
        | ClientOverlay::DashboardMenu
        | ClientOverlay::ChangeServerConfirmation
        | ClientOverlay::LeaveConfirmation => button.error_action,
        ClientOverlay::Settings | ClientOverlay::Credits => false,
        ClientOverlay::None => !button.error_action,
    }
}

pub(super) fn queue_ui_action(actions: &mut PendingFlowActions, action: FlowUiAction) {
    if matches!(
        action,
        FlowUiAction::Cancel | FlowUiAction::Disconnect | FlowUiAction::ConfirmChangeServer
    ) {
        actions.explicit = Some(action);
    } else {
        actions.ordinary = Some(action);
    }
}

pub(super) fn edited_value(model: &ServerSelectModel, field: EditingField) -> &str {
    match field {
        EditingField::Address => &model.address,
        EditingField::Name => &model.name,
    }
}

pub(super) fn edited_value_mut(model: &mut ServerSelectModel, field: EditingField) -> &mut String {
    match field {
        EditingField::Address => &mut model.address,
        EditingField::Name => &mut model.name,
    }
}

pub(super) fn previous_caret(value: &str, caret: usize, field: EditingField) -> usize {
    if caret == 0 {
        return 0;
    }
    match field {
        EditingField::Address => caret.saturating_sub(1),
        EditingField::Name => value[..caret]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(index, _)| index),
    }
}

pub(super) fn next_caret(value: &str, caret: usize, field: EditingField) -> usize {
    if caret >= value.len() {
        return value.len();
    }
    match field {
        EditingField::Address => caret + 1,
        EditingField::Name => value[caret..]
            .grapheme_indices(true)
            .nth(1)
            .map_or(value.len(), |(index, _)| caret + index),
    }
}

pub(super) fn insert_editor_text(model: &mut ServerSelectModel, field: EditingField, text: &str) {
    let allowed = match field {
        EditingField::Address => text.is_ascii() && !text.chars().any(char::is_control),
        EditingField::Name => !text.chars().any(|character| {
            character.is_control() || matches!(character, '\u{2028}' | '\u{2029}')
        }),
    };
    let maximum = match field {
        EditingField::Address => 255,
        EditingField::Name => 64,
    };
    if !allowed || edited_value(model, field).len().saturating_add(text.len()) > maximum {
        model.inline_error = Some("Text exceeds this field's bounds".to_string());
        return;
    }
    let caret = model.caret;
    edited_value_mut(model, field).insert_str(caret, text);
    model.caret = caret + text.len();
    model.inline_error = None;
}

pub(super) fn insert_brawler_name_text(draft: &mut BrawlerEditDraft, text: &str) {
    let allowed = !text
        .chars()
        .any(|character| character.is_control() || matches!(character, '\u{2028}' | '\u{2029}'));
    if !allowed || draft.name.len().saturating_add(text.len()) > 64 {
        draft.inline_error = Some("Name exceeds this field's bounds".to_string());
        return;
    }
    draft.name.insert_str(draft.name_caret, text);
    draft.name_caret += text.len();
    draft.inline_error = None;
}
