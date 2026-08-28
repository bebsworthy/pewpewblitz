//! Shared flow-screen roots, buttons, navigation, and interaction presentation.

use super::{dashboard::DashboardButtonStyle, game_select::GameTypeSelectionDraft};
use crate::client::{ClientLobbyMembership, flow::actions::FlowUiAction};
use bevy::{prelude::*, ui::InteractionDisabled};
use lightyear::prelude::client::Client;

#[derive(Resource, Default)]
pub(in crate::client::flow) struct FlowNavigation {
    pub(in crate::client::flow) selected: usize,
}

#[derive(Component)]
pub(in crate::client::flow) struct FlowRoot;

#[derive(Component, Clone, Debug)]
pub(in crate::client::flow) struct FlowButton {
    pub(in crate::client::flow) index: usize,
    pub(in crate::client::flow) action: FlowUiAction,
    pub(in crate::client::flow) error_action: bool,
}

pub(in crate::client::flow) fn flow_root_node() -> Node {
    Node {
        width: percent(100),
        height: percent(100),
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        row_gap: px(10),
        padding: UiRect::all(px(20)),
        overflow: Overflow::scroll_y(),
        ..default()
    }
}

pub(in crate::client::flow) fn spawn_heading(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text),
        TextFont::from_font_size(38.0),
        TextColor(Color::srgb(0.25, 0.9, 1.0)),
    ));
}

pub(in crate::client::flow) fn spawn_flow_button(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    label: &str,
) {
    spawn_flow_button_disabled(parent, index, action, label, false);
}

pub(in crate::client::flow) fn spawn_flow_button_disabled(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    label: &str,
    disabled: bool,
) {
    let mut entity = parent.spawn((
        Button,
        FlowButton {
            index,
            action,
            error_action: false,
        },
        Node {
            width: percent(88),
            max_width: px(820),
            min_height: px(42),
            padding: UiRect::axes(px(12), px(8)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(px(2)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.09, 0.14, 0.2)),
        BorderColor::all(Color::NONE),
    ));
    if disabled {
        entity.insert(InteractionDisabled);
    }
    entity.with_child((Text::new(label), TextFont::from_font_size(18.0)));
}

pub(in crate::client::flow) fn spawn_flow_error_button(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    label: &str,
) {
    spawn_flow_error_button_disabled(parent, index, action, label, false);
}

pub(in crate::client::flow) fn spawn_flow_error_button_disabled(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    label: &str,
    disabled: bool,
) {
    let mut entity = parent.spawn((
        Button,
        FlowButton {
            index,
            action,
            error_action: true,
        },
        Node {
            width: percent(92),
            min_height: px(44),
            padding: UiRect::axes(px(12), px(8)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(px(2)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.16, 0.12, 0.15)),
        BorderColor::all(Color::NONE),
    ));
    if disabled {
        entity.insert(InteractionDisabled);
    }
    entity.with_child((Text::new(label), TextFont::from_font_size(18.0)));
}

pub(in crate::client::flow) fn flow_button_background(
    disabled: bool,
    interaction: Interaction,
    focused: bool,
    selected: bool,
    dashboard_style: Option<DashboardButtonStyle>,
) -> Color {
    if matches!(dashboard_style, Some(DashboardButtonStyle::Preview)) {
        return Color::NONE;
    }
    if disabled {
        return Color::srgb(0.1, 0.1, 0.12);
    }
    match (interaction, dashboard_style) {
        (Interaction::Pressed, Some(DashboardButtonStyle::Play)) => Color::srgb(0.92, 0.48, 0.02),
        (
            Interaction::Pressed,
            Some(
                DashboardButtonStyle::Build
                | DashboardButtonStyle::Mode
                | DashboardButtonStyle::Practice,
            ),
        ) => Color::srgb(0.78, 0.87, 0.98),
        (Interaction::Pressed, Some(DashboardButtonStyle::Header)) => {
            Color::srgb(0.025, 0.22, 0.58)
        }
        (Interaction::Pressed, None) => Color::srgb(0.08, 0.48, 0.58),
        (Interaction::Hovered, Some(DashboardButtonStyle::Play)) => Color::srgb(1.0, 0.7, 0.08),
        (
            Interaction::Hovered,
            Some(
                DashboardButtonStyle::Build
                | DashboardButtonStyle::Mode
                | DashboardButtonStyle::Practice,
            ),
        ) => Color::srgb(0.86, 0.93, 1.0),
        (Interaction::Hovered, Some(DashboardButtonStyle::Header)) => Color::srgb(0.06, 0.4, 0.9),
        (Interaction::Hovered, None) => Color::srgb(0.12, 0.32, 0.42),
        (_, Some(DashboardButtonStyle::Play)) => Color::srgb(1.0, 0.62, 0.04),
        (
            _,
            Some(
                DashboardButtonStyle::Build
                | DashboardButtonStyle::Mode
                | DashboardButtonStyle::Practice,
            ),
        ) => Color::srgb(0.92, 0.95, 1.0),
        (_, Some(DashboardButtonStyle::Header)) => Color::srgb(0.035, 0.3, 0.76),
        (_, None) if focused => Color::srgb(0.12, 0.32, 0.42),
        (_, None) if selected => Color::srgb(0.12, 0.24, 0.34),
        (_, None) => Color::srgb(0.09, 0.14, 0.2),
        (_, Some(DashboardButtonStyle::Preview)) => unreachable!("handled above"),
    }
}

pub(in crate::client::flow) fn flow_button_border(
    disabled: bool,
    interaction: Interaction,
    focused: bool,
    selected: bool,
    dashboard_style: Option<DashboardButtonStyle>,
) -> Color {
    if disabled {
        Color::NONE
    } else if matches!(dashboard_style, Some(DashboardButtonStyle::Preview))
        && (interaction == Interaction::Hovered || focused)
    {
        Color::srgb(0.25, 0.9, 1.0)
    } else if focused {
        Color::WHITE
    } else if matches!(dashboard_style, Some(DashboardButtonStyle::Play)) {
        Color::srgb(1.0, 0.86, 0.35)
    } else if matches!(dashboard_style, Some(DashboardButtonStyle::Header)) {
        Color::srgb(0.18, 0.58, 1.0)
    } else if matches!(
        dashboard_style,
        Some(
            DashboardButtonStyle::Build
                | DashboardButtonStyle::Mode
                | DashboardButtonStyle::Practice
        )
    ) {
        Color::srgb(0.48, 0.66, 0.9)
    } else if selected {
        Color::srgb(0.25, 0.9, 1.0)
    } else {
        Color::NONE
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "shared button chrome reads selection state but owns no screen copy or layout"
)]
pub(in crate::client::flow) fn update_flow_button_chrome(
    navigation: Res<FlowNavigation>,
    game_draft: Res<GameTypeSelectionDraft>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    mut buttons: Query<(
        &FlowButton,
        &Interaction,
        Has<InteractionDisabled>,
        &mut BackgroundColor,
        &mut BorderColor,
        Option<&DashboardButtonStyle>,
    )>,
) {
    let selected_brawler_id = memberships
        .iter()
        .next()
        .and_then(|membership| membership.profile.selected_brawler_id);
    for (button, interaction, disabled, mut background, mut border, dashboard_style) in &mut buttons
    {
        let focused = button.index == navigation.selected;
        let selected_game = match button.action {
            FlowUiAction::SelectGameTypeDraft(index) => game_draft.selected_index == Some(index),
            _ => false,
        };
        let selected_brawler = matches!(
            button.action,
            FlowUiAction::OpenBrawlerDetails(id) if Some(id) == selected_brawler_id
        );
        let selected = selected_game || selected_brawler;
        let dashboard_style = dashboard_style.copied();
        background.0 =
            flow_button_background(disabled, *interaction, focused, selected, dashboard_style);
        border.set_all(flow_button_border(
            disabled,
            *interaction,
            focused,
            selected,
            dashboard_style,
        ));
    }
}
