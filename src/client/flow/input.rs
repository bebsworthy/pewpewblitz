//! Pure focus navigation, overlay filtering, and action-priority policy for flow input.

use crate::client::{
    connection_persistence::{ClientConnectionsPath, save_connections},
    flow::{
        actions::{FlowUiAction, PendingFlowActions},
        model::{ClientFlow, ClientOverlay},
        persistence::ConnectionPersistence,
        screens::{
            brawlers::{BrawlerCreationDraft, BrawlerEditDraft},
            dashboard::{
                DASHBOARD_BUILD_INDEX, DASHBOARD_GAME_INDEX, DASHBOARD_MENU_INDEX,
                DASHBOARD_PLAY_INDEX, DASHBOARD_PRACTICE_INDEX, DashboardLayoutClass,
                DashboardNavigationDirection, DashboardRoot,
            },
            server_select::{EditingField, ServerSelectModel},
            shared::{FlowButton, FlowNavigation},
        },
    },
};
use bevy::{
    input::{ButtonState, keyboard::KeyboardInput},
    prelude::*,
    ui::InteractionDisabled,
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
            DASHBOARD_GAME_INDEX | DASHBOARD_PLAY_INDEX | DASHBOARD_PRACTICE_INDEX => {
                Some(DASHBOARD_BUILD_INDEX)
            }
            _ => None,
        },
        DashboardNavigationDirection::Right => match index {
            DASHBOARD_BUILD_INDEX => Some(DASHBOARD_PLAY_INDEX),
            _ => None,
        },
        DashboardNavigationDirection::Up => match index {
            DASHBOARD_PRACTICE_INDEX => Some(DASHBOARD_PLAY_INDEX),
            DASHBOARD_PLAY_INDEX => Some(DASHBOARD_GAME_INDEX),
            DASHBOARD_GAME_INDEX if class == DashboardLayoutClass::Compact => {
                Some(DASHBOARD_BUILD_INDEX)
            }
            DASHBOARD_GAME_INDEX | DASHBOARD_BUILD_INDEX => Some(DASHBOARD_MENU_INDEX),
            _ => None,
        },
        DashboardNavigationDirection::Down => match index {
            DASHBOARD_MENU_INDEX if class == DashboardLayoutClass::Compact => {
                Some(DASHBOARD_BUILD_INDEX)
            }
            DASHBOARD_MENU_INDEX | DASHBOARD_BUILD_INDEX => Some(DASHBOARD_GAME_INDEX),
            DASHBOARD_GAME_INDEX => Some(DASHBOARD_PLAY_INDEX),
            DASHBOARD_PLAY_INDEX => Some(DASHBOARD_PRACTICE_INDEX),
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

pub(super) fn game_selection_neighbor(
    current: usize,
    direction: DashboardNavigationDirection,
    available: &[usize],
    columns: usize,
) -> usize {
    let cards = available
        .iter()
        .copied()
        .filter(|i| *i < 100)
        .collect::<Vec<_>>();
    let row = |i: usize| {
        if i < 100 {
            cards.iter().position(|c| *c == i).unwrap_or(0) / columns
        } else if i < 1000 {
            cards.len().div_ceil(columns)
        } else {
            cards.len().div_ceil(columns) + 1
        }
    };
    let same = available
        .iter()
        .copied()
        .filter(|i| row(*i) == row(current))
        .collect::<Vec<_>>();
    let position = same.iter().position(|i| *i == current).unwrap_or(0);
    let target_row = match direction {
        DashboardNavigationDirection::Left => {
            return same
                .get(position.saturating_sub(1))
                .copied()
                .unwrap_or(current);
        }
        DashboardNavigationDirection::Right => {
            return same
                .get((position + 1).min(same.len().saturating_sub(1)))
                .copied()
                .unwrap_or(current);
        }
        DashboardNavigationDirection::Up => row(current).checked_sub(1),
        DashboardNavigationDirection::Down => Some(row(current) + 1),
    };
    let targets = available
        .iter()
        .copied()
        .filter(|i| Some(row(*i)) == target_row)
        .collect::<Vec<_>>();
    targets
        .get(position.min(targets.len().saturating_sub(1)))
        .copied()
        .unwrap_or(current)
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

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "the bounded flow input phase keeps field and navigation precedence visible"
)]
pub(super) fn collect_flow_input(
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut keyboard_events: MessageReader<KeyboardInput>,
    mut model: ResMut<ServerSelectModel>,
    mut persistence: ResMut<ConnectionPersistence>,
    path: Res<ClientConnectionsPath>,
    mut navigation: ResMut<FlowNavigation>,
    buttons: Query<(&FlowButton, &Interaction, Has<InteractionDisabled>)>,
    dashboard_layout: Query<&DashboardLayoutClass, With<DashboardRoot>>,
    game_columns: Query<&super::screens::game_select::GameSelectionColumns>,
    mut actions: ResMut<PendingFlowActions>,
    mut brawler_edit: ResMut<BrawlerEditDraft>,
    mut creation: ResMut<BrawlerCreationDraft>,
) {
    if *flow.get() == ClientFlow::PracticeMapSelect && matches!(*overlay, ClientOverlay::None) {
        return;
    }
    for (button, interaction, disabled) in &buttons {
        if !disabled
            && *interaction == Interaction::Pressed
            && overlay_allows_button(&overlay, button)
        {
            navigation.selected = button.index;
            queue_ui_action(&mut actions, button.action.clone());
        }
    }
    let pad_pressed = |button| gamepads.iter().any(|pad| pad.just_pressed(button));
    if matches!(
        overlay.as_ref(),
        ClientOverlay::BrawlerEditor | ClientOverlay::BrawlerDetails(_)
    ) && brawler_edit.editing_name
    {
        let edit = &mut *brawler_edit;
        edit_name_input(
            &mut edit.name,
            &mut edit.name_caret,
            &mut edit.editing_name,
            &mut edit.inline_error,
            &keyboard,
            &mut keyboard_events,
            pad_pressed(GamepadButton::South),
            pad_pressed(GamepadButton::East),
        );
        if !edit.editing_name && edit.inline_rename {
            if keyboard.just_pressed(KeyCode::Escape) || pad_pressed(GamepadButton::East) {
                edit.inline_rename = false;
            } else {
                queue_ui_action(&mut actions, FlowUiAction::ConfirmBrawlerEdit);
            }
        }
        return;
    }
    if matches!(*overlay, ClientOverlay::BrawlerCreation) && creation.editing_name {
        let edit = &mut *creation;
        edit_name_input(
            &mut edit.name,
            &mut edit.name_caret,
            &mut edit.editing_name,
            &mut edit.inline_error,
            &keyboard,
            &mut keyboard_events,
            pad_pressed(GamepadButton::South),
            pad_pressed(GamepadButton::East),
        );
        return;
    }
    if let Some(field) = model.editing {
        if keyboard.just_pressed(KeyCode::Home) {
            model.caret = 0;
        } else if keyboard.just_pressed(KeyCode::End) {
            model.caret = edited_value(&model, field).len();
        } else if keyboard.just_pressed(KeyCode::ArrowLeft) {
            model.caret = previous_caret(edited_value(&model, field), model.caret, field);
        } else if keyboard.just_pressed(KeyCode::ArrowRight) {
            model.caret = next_caret(edited_value(&model, field), model.caret, field);
        }
        if keyboard.just_pressed(KeyCode::KeyV)
            && keyboard.any_pressed([
                KeyCode::ControlLeft,
                KeyCode::ControlRight,
                KeyCode::SuperLeft,
                KeyCode::SuperRight,
            ])
        {
            match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
                Ok(text) => insert_editor_text(&mut model, field, &text),
                Err(error) => {
                    model.inline_error = Some(format!("Clipboard text is unavailable: {error}"));
                }
            }
        }
        for event in keyboard_events.read() {
            if event.state != ButtonState::Pressed {
                continue;
            }
            if event.key_code == KeyCode::Backspace {
                let previous = previous_caret(edited_value(&model, field), model.caret, field);
                let caret = model.caret;
                edited_value_mut(&mut model, field).replace_range(previous..caret, "");
                model.caret = previous;
            } else if event.key_code == KeyCode::Delete {
                let next = next_caret(edited_value(&model, field), model.caret, field);
                let caret = model.caret;
                edited_value_mut(&mut model, field).replace_range(caret..next, "");
            } else if let Some(text) = event.text.as_deref() {
                insert_editor_text(&mut model, field, text);
            }
        }
        if keyboard.just_pressed(KeyCode::Enter) || pad_pressed(GamepadButton::South) {
            if field == EditingField::Name {
                match crate::lobby::normalize_proposed_display_name(&model.name) {
                    Ok(name) => {
                        model.name.clone_from(&name);
                        model.committed_name.clone_from(&name);
                        persistence.state.preferred_display_name = Some(name);
                        if let Err(error) = save_connections(&path.0, &persistence.state) {
                            persistence.dirty_error = Some(error);
                        }
                        model.inline_error = None;
                    }
                    Err(error) => model.inline_error = Some(format!("Invalid name: {error}")),
                }
            }
            model.editing = None;
        } else if keyboard.just_pressed(KeyCode::Escape) || pad_pressed(GamepadButton::East) {
            if field == EditingField::Name {
                model.name = model.committed_name.clone();
            }
            model.editing = None;
        }
        return;
    }

    let mut available = buttons
        .iter()
        .filter(|(button, _, disabled)| !*disabled && overlay_allows_button(&overlay, button))
        .map(|(button, _, _)| button.index)
        .collect::<Vec<_>>();
    available.sort_unstable();
    available.dedup();
    if matches!(*overlay, ClientOverlay::BrawlerDetails(_)) {
        available.sort_by_key(|index| [4, 5, 3, 2, 1, 0].iter().position(|entry| entry == index));
    }
    if !available.is_empty() {
        if *flow.get() == ClientFlow::Dashboard && matches!(*overlay, ClientOverlay::None) {
            navigation.selected = repair_dashboard_focus(navigation.selected, &available);
            let direction = if keyboard.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS])
                || pad_pressed(GamepadButton::DPadDown)
            {
                Some(DashboardNavigationDirection::Down)
            } else if keyboard.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW])
                || pad_pressed(GamepadButton::DPadUp)
            {
                Some(DashboardNavigationDirection::Up)
            } else if keyboard.any_just_pressed([KeyCode::ArrowLeft, KeyCode::KeyA])
                || pad_pressed(GamepadButton::DPadLeft)
            {
                Some(DashboardNavigationDirection::Left)
            } else if keyboard.any_just_pressed([KeyCode::ArrowRight, KeyCode::KeyD])
                || pad_pressed(GamepadButton::DPadRight)
            {
                Some(DashboardNavigationDirection::Right)
            } else {
                None
            };
            if let Some(direction) = direction {
                let class = dashboard_layout.iter().next().copied().unwrap_or_default();
                navigation.selected =
                    dashboard_focus_neighbor(class, navigation.selected, direction, &available);
            }
        } else if (*flow.get() == ClientFlow::GameTypeSelect
            && matches!(*overlay, ClientOverlay::None))
            || matches!(
                *overlay,
                ClientOverlay::WeaponEquipment
                    | ClientOverlay::BrawlerList
                    | ClientOverlay::BrawlerCreation
                    | ClientOverlay::BrawlerEditor
            )
        {
            if !available.contains(&navigation.selected) {
                navigation.selected = available[0];
            }
            let direction = if keyboard.any_just_pressed([KeyCode::ArrowLeft, KeyCode::KeyA])
                || pad_pressed(GamepadButton::DPadLeft)
            {
                Some(DashboardNavigationDirection::Left)
            } else if keyboard.any_just_pressed([KeyCode::ArrowRight, KeyCode::KeyD])
                || pad_pressed(GamepadButton::DPadRight)
            {
                Some(DashboardNavigationDirection::Right)
            } else if keyboard.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW])
                || pad_pressed(GamepadButton::DPadUp)
            {
                Some(DashboardNavigationDirection::Up)
            } else if keyboard.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS])
                || pad_pressed(GamepadButton::DPadDown)
            {
                Some(DashboardNavigationDirection::Down)
            } else {
                None
            };
            if let Some(direction) = direction {
                let columns = game_columns.iter().next().map_or(4, |c| c.0);
                navigation.selected = if matches!(
                    *overlay,
                    ClientOverlay::WeaponEquipment
                        | ClientOverlay::BrawlerList
                        | ClientOverlay::BrawlerCreation
                        | ClientOverlay::BrawlerEditor
                ) {
                    equipment_neighbor(navigation.selected, direction, &available, columns)
                } else {
                    game_selection_neighbor(navigation.selected, direction, &available, columns)
                };
            }
        } else {
            let position = available
                .iter()
                .position(|index| *index == navigation.selected)
                .unwrap_or(0);
            if keyboard.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS])
                || pad_pressed(GamepadButton::DPadDown)
            {
                navigation.selected = available[(position + 1).min(available.len() - 1)];
            }
            if keyboard.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW])
                || pad_pressed(GamepadButton::DPadUp)
            {
                navigation.selected = available[position.saturating_sub(1)];
            }
        }
    }
    if (keyboard.any_just_pressed([KeyCode::Enter, KeyCode::Space])
        || pad_pressed(GamepadButton::South))
        && let Some((button, _, _)) = buttons
            .iter()
            .filter(|(button, _, disabled)| {
                !*disabled
                    && button.index == navigation.selected
                    && overlay_allows_button(&overlay, button)
            })
            .min_by_key(|(button, _, _)| button.index)
    {
        queue_ui_action(&mut actions, button.action.clone());
    }
    if keyboard.just_pressed(KeyCode::Escape) || pad_pressed(GamepadButton::East) {
        let action = if matches!(overlay.as_ref(), ClientOverlay::ChangeServerConfirmation) {
            FlowUiAction::KeepServer
        } else if matches!(overlay.as_ref(), ClientOverlay::DashboardMenu) {
            FlowUiAction::CloseDashboardMenu
        } else if matches!(overlay.as_ref(), ClientOverlay::BrawlerList) {
            FlowUiAction::CloseBrawlerList
        } else if matches!(overlay.as_ref(), ClientOverlay::BrawlerDetails(_)) {
            FlowUiAction::BackToBrawlerList
        } else if matches!(overlay.as_ref(), ClientOverlay::BrawlerCreation) {
            FlowUiAction::CancelCreateBrawler
        } else if matches!(overlay.as_ref(), ClientOverlay::BrawlerEditor) {
            FlowUiAction::CancelBrawlerEdit
        } else if matches!(overlay.as_ref(), ClientOverlay::WeaponEquipment) {
            FlowUiAction::CancelWeaponEquipment
        } else if matches!(
            overlay.as_ref(),
            ClientOverlay::DeleteBrawlerConfirmation(_)
        ) {
            FlowUiAction::CancelDeleteBrawler
        } else if matches!(overlay.as_ref(), ClientOverlay::Confirmation(_)) {
            FlowUiAction::KeepLoading
        } else {
            match *flow.get() {
                ClientFlow::Connecting => FlowUiAction::Cancel,
                ClientFlow::GameTypeSelect => FlowUiAction::CancelGameType,
                ClientFlow::PracticeMapSelect => FlowUiAction::CancelPracticeMaps,
                ClientFlow::Queue => FlowUiAction::CancelQueue,
                ClientFlow::MatchLoading => FlowUiAction::RequestCancelMatchStart,
                ClientFlow::Results => FlowUiAction::ReturnToDashboard,
                ClientFlow::Match => {
                    if matches!(overlay.as_ref(), ClientOverlay::LeaveConfirmation) {
                        FlowUiAction::KeepPlaying
                    } else {
                        return;
                    }
                }
                ClientFlow::ServerSelect => FlowUiAction::Back,
                ClientFlow::Dashboard => return,
            }
        };
        queue_ui_action(&mut actions, action);
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

fn insert_name_text(name: &mut String, caret: &mut usize, error: &mut Option<String>, text: &str) {
    if text
        .chars()
        .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
        || name.len().saturating_add(text.len()) > 64
    {
        *error = Some("Name exceeds this field's bounds".into());
        return;
    }
    name.insert_str(*caret, text);
    *caret += text.len();
    *error = None;
}

fn equipment_neighbor(
    current: usize,
    direction: DashboardNavigationDirection,
    available: &[usize],
    columns: usize,
) -> usize {
    if current == 1000 {
        return if direction == DashboardNavigationDirection::Down {
            available
                .iter()
                .copied()
                .find(|i| *i != 1000)
                .unwrap_or(current)
        } else {
            current
        };
    }
    let wanted = match direction {
        DashboardNavigationDirection::Up => {
            if current < columns {
                1000
            } else {
                current - columns
            }
        }
        DashboardNavigationDirection::Down => {
            let next_row = (current / columns + 1) * columns;
            available
                .iter()
                .copied()
                .filter(|i| *i >= next_row && *i < next_row + columns && *i < 1000)
                .min_by_key(|i| i.abs_diff(current + columns))
                .unwrap_or(current)
        }
        DashboardNavigationDirection::Left => current.saturating_sub(1),
        DashboardNavigationDirection::Right => current + 1,
    };
    if available.contains(&wanted) {
        wanted
    } else {
        current
    }
}

#[cfg(test)]
mod equipment_navigation_tests {
    use super::{DashboardNavigationDirection as D, equipment_neighbor};
    #[test]
    fn grid_columns_reach_back_and_inventory_beyond_one_hundred() {
        let mut available = (0..129).collect::<Vec<_>>();
        available.push(1000);
        assert_eq!(equipment_neighbor(0, D::Down, &available, 3), 3);
        assert_eq!(equipment_neighbor(1, D::Up, &available, 3), 1000);
        assert_eq!(equipment_neighbor(1000, D::Down, &available, 3), 0);
        assert_eq!(equipment_neighbor(99, D::Down, &available, 3), 102);
        assert_eq!(equipment_neighbor(128, D::Right, &available, 3), 128);
        assert_eq!(equipment_neighbor(128, D::Up, &available, 3), 125);
        assert_eq!(equipment_neighbor(6, D::Down, &available, 6), 12);
        assert_eq!(equipment_neighbor(9, D::Down, &available, 5), 14);
        assert_eq!(equipment_neighbor(5, D::Up, &available, 6), 1000);
        assert_eq!(
            equipment_neighbor(11, D::Down, &(0..13).collect::<Vec<_>>(), 6),
            12
        );
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Shared bounded name-field input, borrowed by creation and editing"
)]
fn edit_name_input(
    name: &mut String,
    caret: &mut usize,
    editing: &mut bool,
    error: &mut Option<String>,
    keyboard: &ButtonInput<KeyCode>,
    keyboard_events: &mut MessageReader<KeyboardInput>,
    confirm: bool,
    cancel: bool,
) {
    if keyboard.just_pressed(KeyCode::Home) {
        *caret = 0;
    } else if keyboard.just_pressed(KeyCode::End) {
        *caret = name.len();
    } else if keyboard.just_pressed(KeyCode::ArrowLeft) {
        *caret = previous_caret(&*name, *caret, EditingField::Name);
    } else if keyboard.just_pressed(KeyCode::ArrowRight) {
        *caret = next_caret(&*name, *caret, EditingField::Name);
    }
    for event in keyboard_events.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        if event.key_code == KeyCode::Backspace {
            let previous = previous_caret(&*name, *caret, EditingField::Name);
            let current_caret = *caret;
            name.replace_range(previous..current_caret, "");
            *caret = previous;
        } else if event.key_code == KeyCode::Delete {
            let next = next_caret(&*name, *caret, EditingField::Name);
            let current_caret = *caret;
            name.replace_range(current_caret..next, "");
        } else if let Some(text) = event.text.as_deref() {
            insert_name_text(name, caret, error, text);
        }
    }
    if keyboard.just_pressed(KeyCode::Enter) || confirm {
        match crate::lobby::normalize_proposed_display_name(&*name) {
            Ok(normalized) => {
                *name = normalized;
                *editing = false;
                *error = None;
            }
            Err(reason) => *error = Some(format!("Invalid name: {reason}")),
        }
    } else if keyboard.just_pressed(KeyCode::Escape) || cancel {
        *editing = false;
    }
}
