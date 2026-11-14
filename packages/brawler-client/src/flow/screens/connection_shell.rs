//! Shared presentation primitives for the connection screen family.

use super::shared::{FlowButton, surface_role};
use crate::{
    ClientAssetHandles,
    flow::actions::FlowUiAction,
    ui::{
        surface::{Surface, SurfaceState},
        theme::{TEXT, font, shell_background},
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
        crate::ui::motifs::ShellBackdrop(crate::ui::motifs::MotifLayout::Edges),
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

/// The real logo lockup. A screen spawned before `ClientAssetHandles` exists (the first
/// Connecting frame) loads it directly rather than drawing substitute text; only headless
/// schedule tests without an asset server fall back to display-font text.
pub(super) fn spawn_logo(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&ClientAssetHandles>,
    server: Option<&AssetServer>,
    width: f32,
) {
    let logo = assets
        .map(|assets| assets.loading_logo.clone())
        .or_else(|| server.map(|server| server.load(crate::assets::LOADING_LOGO_PATH)));
    if let Some(logo) = logo {
        parent.spawn((
            ImageNode::new(logo),
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
            TextColor(TEXT),
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
            TextColor(TEXT),
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

pub(super) fn root_background() -> impl Bundle {
    shell_background()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BRL-51zd: a connection screen spawned before `ClientAssetHandles` exists still shows the
    /// real logo, never fallback text in the default font.
    #[test]
    fn logo_loads_directly_before_the_handle_resource_exists() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>();
        let server = app.world().resource::<AssetServer>().clone();
        let root = app.world_mut().spawn(Node::default()).id();
        app.world_mut()
            .commands()
            .entity(root)
            .with_children(|parent| spawn_logo(parent, None, Some(&server), 300.0));
        app.update();
        let mut images = app.world_mut().query::<&ImageNode>();
        assert_eq!(images.iter(app.world()).count(), 1);
        let mut texts = app.world_mut().query::<&Text>();
        assert_eq!(texts.iter(app.world()).count(), 0);
    }
}
