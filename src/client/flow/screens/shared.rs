//! Shared flow-screen roots, buttons, navigation, and interaction presentation.

use super::{dashboard::DashboardButtonStyle, game_select::GameTypeSelectionDraft};
use crate::client::{
    ClientLobbyMembership,
    flow::{actions::FlowUiAction, input::overlay_allows_button, model::ClientOverlay},
};
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
    if let Some(style) = dashboard_style {
        use super::dashboard::{CREAM, INK, ORANGE, TEAL};
        if matches!(style, DashboardButtonStyle::Preview) {
            return Color::NONE;
        }
        if disabled {
            return Color::srgb(0.69, 0.71, 0.66);
        }
        if focused && matches!(style, DashboardButtonStyle::Header) {
            return if interaction == Interaction::Pressed {
                CREAM.mix(&INK, 0.15)
            } else {
                CREAM
            };
        }
        let base = match style {
            DashboardButtonStyle::Play | DashboardButtonStyle::Header => ORANGE,
            DashboardButtonStyle::Build | DashboardButtonStyle::Practice => TEAL,
            DashboardButtonStyle::Mode => CREAM,
            DashboardButtonStyle::Preview => unreachable!(),
        };
        return match interaction {
            Interaction::Pressed => base.mix(&INK, 0.15),
            Interaction::Hovered => base.mix(&Color::WHITE, 0.16),
            Interaction::None => base,
        };
    }
    if disabled {
        return Color::srgb(0.1, 0.1, 0.12);
    }
    match interaction {
        Interaction::Pressed => Color::srgb(0.08, 0.48, 0.58),
        Interaction::Hovered => Color::srgb(0.12, 0.32, 0.42),
        Interaction::None if focused => Color::srgb(0.12, 0.32, 0.42),
        Interaction::None if selected => Color::srgb(0.12, 0.24, 0.34),
        Interaction::None => Color::srgb(0.09, 0.14, 0.2),
    }
}

pub(in crate::client::flow) fn flow_button_border(
    disabled: bool,
    interaction: Interaction,
    focused: bool,
    selected: bool,
    dashboard_style: Option<DashboardButtonStyle>,
) -> Color {
    if matches!(dashboard_style, Some(DashboardButtonStyle::Preview)) {
        return Color::NONE;
    }
    if dashboard_style.is_some() {
        if disabled {
            return Color::srgb(0.4, 0.45, 0.44);
        }
        if focused || interaction == Interaction::Hovered {
            return Color::srgb(0.0, 0.37, 0.40);
        }
        return if matches!(dashboard_style, Some(DashboardButtonStyle::Preview)) {
            Color::NONE
        } else {
            super::dashboard::INK
        };
    }
    if disabled {
        Color::NONE
    } else if focused {
        Color::WHITE
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
    overlay: Res<ClientOverlay>,
    assets: Option<Res<crate::client::ClientAssetHandles>>,
    game_draft: Res<GameTypeSelectionDraft>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    mut buttons: Query<(
        &FlowButton,
        &Interaction,
        Has<InteractionDisabled>,
        &mut BackgroundColor,
        &mut BorderColor,
        Option<&DashboardButtonStyle>,
        Option<&mut ImageNode>,
    )>,
) {
    let selected_brawler_id = memberships
        .iter()
        .next()
        .and_then(|membership| membership.profile.selected_brawler_id);
    for (button, interaction, disabled, mut background, mut border, dashboard_style, image) in
        &mut buttons
    {
        // Use the same owner gate as input collection: indices are local to each surface.
        let active = overlay_allows_button(&overlay, button);
        let focused = active && button.index == navigation.selected;
        let interaction = if active {
            *interaction
        } else {
            Interaction::None
        };
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
            flow_button_background(disabled, interaction, focused, selected, dashboard_style);
        border.set_all(flow_button_border(
            disabled,
            interaction,
            focused,
            selected,
            dashboard_style,
        ));
        if let Some(mut image) = image.filter(|_| dashboard_style.is_some()) {
            background.0 = Color::NONE;
            border.set_all(Color::NONE);
            if let (Some(assets), Some(style)) = (assets.as_deref(), dashboard_style) {
                let index = match style {
                    DashboardButtonStyle::Mode => 0,
                    DashboardButtonStyle::Play => 1,
                    DashboardButtonStyle::Practice => 2,
                    DashboardButtonStyle::Build => 3,
                    DashboardButtonStyle::Header => 4,
                    DashboardButtonStyle::Preview => continue,
                };
                image.image = if !disabled && focused {
                    assets.dashboard_focus_panels[index].clone()
                } else {
                    assets.dashboard_panels[index].clone()
                };
            }

            image.color = if disabled {
                Color::srgba(0.65, 0.65, 0.65, 0.7)
            } else if interaction == Interaction::Pressed {
                Color::srgb(0.82, 0.86, 0.86)
            } else if focused || interaction == Interaction::Hovered {
                Color::srgb(1.08, 1.08, 1.08)
            } else {
                Color::WHITE
            };
        }
    }
}
