//! Pure focus navigation, overlay filtering, and action-priority policy for flow input.

#![allow(clippy::wildcard_imports)]

use super::*;
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
    mut actions: ResMut<PendingFlowActions>,
    mut brawler_edit: ResMut<BrawlerEditDraft>,
) {
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
    if matches!(overlay.as_ref(), ClientOverlay::BrawlerEditor) && brawler_edit.editing_name {
        if keyboard.just_pressed(KeyCode::Home) {
            brawler_edit.name_caret = 0;
        } else if keyboard.just_pressed(KeyCode::End) {
            brawler_edit.name_caret = brawler_edit.name.len();
        } else if keyboard.just_pressed(KeyCode::ArrowLeft) {
            brawler_edit.name_caret = previous_caret(
                &brawler_edit.name,
                brawler_edit.name_caret,
                EditingField::Name,
            );
        } else if keyboard.just_pressed(KeyCode::ArrowRight) {
            brawler_edit.name_caret = next_caret(
                &brawler_edit.name,
                brawler_edit.name_caret,
                EditingField::Name,
            );
        }
        for event in keyboard_events.read() {
            if event.state != ButtonState::Pressed {
                continue;
            }
            if event.key_code == KeyCode::Backspace {
                let previous = previous_caret(
                    &brawler_edit.name,
                    brawler_edit.name_caret,
                    EditingField::Name,
                );
                let caret = brawler_edit.name_caret;
                brawler_edit.name.replace_range(previous..caret, "");
                brawler_edit.name_caret = previous;
            } else if event.key_code == KeyCode::Delete {
                let next = next_caret(
                    &brawler_edit.name,
                    brawler_edit.name_caret,
                    EditingField::Name,
                );
                let caret = brawler_edit.name_caret;
                brawler_edit.name.replace_range(caret..next, "");
            } else if let Some(text) = event.text.as_deref() {
                insert_brawler_name_text(&mut brawler_edit, text);
            }
        }
        if keyboard.just_pressed(KeyCode::Enter) || pad_pressed(GamepadButton::South) {
            match crate::lobby::normalize_proposed_display_name(&brawler_edit.name) {
                Ok(name) => {
                    brawler_edit.name = name;
                    brawler_edit.editing_name = false;
                    brawler_edit.inline_error = None;
                }
                Err(error) => brawler_edit.inline_error = Some(format!("Invalid name: {error}")),
            }
        } else if keyboard.just_pressed(KeyCode::Escape) || pad_pressed(GamepadButton::East) {
            brawler_edit.editing_name = false;
        }
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
