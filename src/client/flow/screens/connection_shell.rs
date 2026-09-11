//! Shared presentation primitives for the connection screen family.

use super::shared::{FlowButton, surface_role};
use crate::client::{
    ClientAssetHandles,
    flow::actions::FlowUiAction,
    ui::{
        surface::{Surface, SurfaceState},
        theme::{CREAM, INK, font},
    },
};
use bevy::prelude::*;

pub(super) fn root_node() -> Node {
    Node {
        width: percent(100),
        height: percent(100),
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: px(18),
        padding: UiRect::axes(px(42), px(30)),
        overflow: Overflow::scroll_y(),
        ..default()
    }
}

pub(super) fn spawn_background(root: &mut ChildSpawnerCommands, server: Option<&AssetServer>) {
    let Some(server) = server else {
        return;
    };
    root.spawn((
        ImageNode {
            image_mode: NodeImageMode::Stretch,
            ..ImageNode::new(server.load("brawler/ui/shell/game-selection-background.png"))
        },
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            left: px(0),
            top: px(0),
            ..default()
        },
        ZIndex(-1),
    ));
}

pub(super) fn spawn_logo(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&ClientAssetHandles>,
    width: f32,
) {
    if let Some(assets) = assets {
        parent.spawn((
            ImageNode::new(assets.loading_logo.clone()),
            Node {
                width: px(width),
                max_width: percent(70),
                height: auto(),
                flex_shrink: 1.0,
                ..default()
            },
        ));
    } else {
        spawn_text(parent, assets, "PEWPEW BLITZ", 34.0);
    }
}

pub(super) fn spawn_text(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&ClientAssetHandles>,
    copy: impl Into<String>,
    size: f32,
) -> Entity {
    parent
        .spawn((
            Text::new(copy),
            font(assets, size),
            TextColor(INK),
            TextLayout::new(Justify::Center, LineBreak::WordBoundary),
        ))
        .id()
}

#[allow(
    clippy::too_many_arguments,
    reason = "a shell action has one owner, semantic role, scope, label and bounded geometry"
)]
pub(super) fn spawn_action(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&ClientAssetHandles>,
    index: usize,
    action: FlowUiAction,
    label: &str,
    role: Option<Surface>,
    error_action: bool,
    node: Node,
) -> Entity {
    let role = role.unwrap_or_else(|| surface_role(&action));
    parent
        .spawn((
            Button,
            AccessibleLabel::new(label),
            FlowButton {
                index,
                action,
                error_action,
            },
            role,
            SurfaceState::default(),
            node,
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
        ))
        .with_child((
            Text::new(label),
            font(assets, 22.0),
            TextColor(INK),
            TextLayout::new(Justify::Center, LineBreak::WordBoundary),
        ))
        .id()
}

pub(super) fn action_node(width: Val, min_height: f32) -> Node {
    Node {
        width,
        min_height: px(min_height),
        flex_shrink: 0.0,
        padding: UiRect::axes(px(28), px(13)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

pub(super) fn root_background() -> BackgroundColor {
    BackgroundColor(CREAM)
}
