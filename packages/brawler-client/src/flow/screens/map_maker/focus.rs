//! Keyboard and controller focus in the Map Maker editor's chrome (BRL-q5sm).
//!
//! The editor has two input modes. In map-cursor mode (the default) the arrows and D-pad move
//! the map cursor and Enter or South places, as before. Q or the controller's Select switches to
//! chrome-focus mode: the arrows and D-pad move the flat focus ring through the top bar, the
//! side panel, its asset and Check rows and the chip, and Enter, Space or South presses the
//! focused control. Esc or East returns to the map cursor. Every other shortcut works in both
//! modes. A badge in the arena's corner names the mode; like the ring, it hides after a touch
//! press (`FocusVisibility`).

use super::{
    EditorTool, MapMakerRuntime,
    editor::index,
    library::spatial_neighbor,
    widgets::{FlatControl, Kit, RADIUS, bar_fill, row},
};
use crate::{
    flow::{
        actions::{FlowUiAction, PendingFlowActions},
        input::queue_ui_action,
        screens::shared::{FlowButton, FlowNavigation},
    },
    ui::{feedback::UiFeedback, focus_visibility::FocusVisibility, theme::ACCENT},
};
use bevy::{prelude::*, ui::InteractionDisabled};

/// Keyboard key that switches between the map cursor and the editor chrome.
pub(super) const CHROME_FOCUS_KEY: KeyCode = KeyCode::KeyQ;
/// Controller button that switches between the map cursor and the editor chrome.
pub(super) const CHROME_FOCUS_BUTTON: GamepadButton = GamepadButton::Select;

/// The corner badge naming the editor's input mode.
#[derive(Component)]
pub(in crate::flow) struct MapEditorFocusBadge;

/// The badge's copy for the current mode.
pub(super) fn focus_mode_copy(chrome_focus: bool) -> &'static str {
    if chrome_focus {
        "Controls · Q / Select: back to map"
    } else {
        "Map cursor · Q / Select: controls"
    }
}

/// The input-mode badge at the arena's bottom-right corner, left of the side panel. It is a
/// flat control too, so a click switches modes and chrome focus can reach it.
pub(super) fn focus_badge(
    root: &mut ChildSpawnerCommands,
    kit: &Kit,
    runtime: &MapMakerRuntime,
    right: f32,
) {
    root.spawn((
        MapEditorFocusBadge,
        super::editor::MapMakerChrome,
        Node {
            position_type: PositionType::Absolute,
            right: px(right + 8.0),
            bottom: px(8),
            padding: UiRect::all(px(2)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(RADIUS + 2.0)),
            ..row(4.0)
        },
        BackgroundColor(bar_fill()),
        BorderColor::all(ACCENT.with_alpha(if runtime.chrome_focus { 0.9 } else { 0.35 })),
    ))
    .with_children(|badge| {
        let copy = focus_mode_copy(runtime.chrome_focus);
        let id = kit.button(
            badge,
            super::widgets::ButtonSpec::new(
                index::FOCUS_MODE,
                FlowUiAction::MapMakerToggleChromeFocus,
                super::widgets::Tone::Quiet,
            )
            .icon(if runtime.chrome_focus {
                "menu"
            } else {
                "crosshair"
            })
            .label(copy)
            .named(copy),
        );
        badge
            .commands()
            .entity(id)
            .entry::<Node>()
            .and_modify(|mut node| node.justify_content = JustifyContent::FlexStart);
    });
}

/// Shows the mode badge only while focus is visible: a touch press hides it with the ring.
#[allow(clippy::needless_pass_by_value, reason = "Bevy system parameters")]
pub(in crate::flow) fn show_map_editor_focus_badge(
    focus: Option<Res<FocusVisibility>>,
    mut badges: Query<&mut Node, With<MapEditorFocusBadge>>,
) {
    let display = if focus.as_deref().is_none_or(|focus| focus.visible) {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in &mut badges {
        if node.display != display {
            node.display = display;
        }
    }
}

/// The control chrome focus starts on: the active tool's segment.
fn default_focus(runtime: &MapMakerRuntime) -> usize {
    match runtime.tool {
        EditorTool::Place => index::PLACE,
        EditorTool::Select => index::SELECT,
        EditorTool::Erase => index::ERASE,
    }
}

/// Moves and activates chrome focus in chrome-focus mode. Mode switching and Esc/East are read
/// by `map_maker_input`; while asset search is typed, keys belong to the search field.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy system parameters"
)]
pub(in crate::flow) fn navigate_map_editor(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    runtime: Res<MapMakerRuntime>,
    mut navigation: ResMut<FlowNavigation>,
    mut actions: ResMut<PendingFlowActions>,
    mut feedback: MessageWriter<UiFeedback>,
    buttons: Query<
        (
            &FlowButton,
            &Interaction,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            Has<InteractionDisabled>,
        ),
        With<FlatControl>,
    >,
) {
    if !runtime.chrome_focus || runtime.palette_search_active {
        return;
    }
    let candidates: Vec<_> = buttons
        .iter()
        .filter(|(_, _, computed, _, visible, disabled)| {
            visible.get() && !*disabled && !computed.is_empty()
        })
        .map(|(button, interaction, computed, transform, ..)| {
            (
                button,
                *interaction,
                transform.translation * computed.inverse_scale_factor(),
            )
        })
        .collect();
    if candidates.is_empty() {
        return;
    }
    // Focus follows a pointer press.
    if let Some((button, ..)) = candidates
        .iter()
        .find(|(_, interaction, _)| *interaction == Interaction::Pressed)
    {
        navigation.focused = button.index;
    }
    if !candidates
        .iter()
        .any(|(button, ..)| button.index == navigation.focused)
    {
        let preferred = default_focus(&runtime);
        navigation.focused = if candidates
            .iter()
            .any(|(button, ..)| button.index == preferred)
        {
            preferred
        } else {
            index::BACK
        };
    }
    let pad = |button| gamepads.iter().any(|pad| pad.just_pressed(button));
    let direction = if keyboard.just_pressed(KeyCode::ArrowLeft) || pad(GamepadButton::DPadLeft) {
        Some(Vec2::NEG_X)
    } else if keyboard.just_pressed(KeyCode::ArrowRight) || pad(GamepadButton::DPadRight) {
        Some(Vec2::X)
    } else if keyboard.just_pressed(KeyCode::ArrowUp) || pad(GamepadButton::DPadUp) {
        Some(Vec2::NEG_Y)
    } else if keyboard.just_pressed(KeyCode::ArrowDown) || pad(GamepadButton::DPadDown) {
        Some(Vec2::Y)
    } else {
        None
    };
    let positions: Vec<_> = candidates
        .iter()
        .map(|(button, _, position)| (button.index, *position))
        .collect();
    if let Some(direction) = direction
        && let Some(next) = spatial_neighbor(navigation.focused, direction, &positions)
        && next != navigation.focused
    {
        navigation.focused = next;
        feedback.write(UiFeedback::Moved);
    }
    if (keyboard.just_pressed(KeyCode::Enter)
        || keyboard.just_pressed(KeyCode::Space)
        || pad(GamepadButton::South))
        && let Some((button, ..)) = candidates
            .iter()
            .find(|(button, ..)| button.index == navigation.focused)
    {
        queue_ui_action(&mut actions, button.action.clone());
        feedback.write(UiFeedback::Activated(None));
    }
}
