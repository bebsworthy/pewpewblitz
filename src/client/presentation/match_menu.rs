//! Shared shell surfaces for the live match menu.
use bevy::prelude::*;

use super::MatchMenuState;
use crate::client::{
    PauseOverlay,
    ui::{
        surface::{Surface, SurfaceState},
        theme::{CREAM, FONT_PATH, INK, TEAL},
    },
};

#[derive(Component)]
pub(super) struct MatchMenuButton(pub(super) usize);

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are passed by value"
)]
pub(crate) fn spawn_pause_overlay(mut commands: Commands, assets: Res<AssetServer>) {
    let font = assets.load(FONT_PATH);
    commands
        .spawn((
            PauseOverlay,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(px(24)),
                ..default()
            },
            BackgroundColor(CREAM),
            GlobalZIndex(300),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                ImageNode {
                    image_mode: NodeImageMode::Stretch,
                    ..ImageNode::new(assets.load("brawler/ui/shell/game-selection-background.png"))
                },
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                ZIndex(-1),
            ));
            root.spawn(Node {
                width: percent(100),
                max_width: px(640),
                height: percent(100),
                max_height: px(680),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(16),
                ..default()
            })
            .with_children(|column| {
                label(column, &font, "MATCH MENU", 48., INK);
                label(column, &font, "Match continues", 24., TEAL);
                for (index, title) in ["RESUME", "SETTINGS", "SCOREBOARD", "LEAVE MATCH"]
                    .into_iter()
                    .enumerate()
                {
                    column
                        .spawn((
                            Button,
                            MatchMenuButton(index),
                            if index == 0 {
                                Surface::Primary
                            } else {
                                Surface::Action
                            },
                            SurfaceState::default(),
                            Node {
                                width: percent(100),
                                height: px(88),
                                min_height: px(52),
                                padding: UiRect::axes(px(40), px(8)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                        ))
                        .with_children(|button| label(button, &font, title, 28., INK));
                }
            });
        });
}

fn label(
    parent: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    text: &str,
    size: f32,
    color: Color,
) {
    parent.spawn((
        Text::new(text),
        TextFont {
            font: font.clone().into(),
            font_size: size.into(),
            ..default()
        },
        TextColor(color),
        TextLayout::justify(Justify::Center),
    ));
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are passed by value"
)]
pub(super) fn update_match_menu_chrome(
    menu: Res<MatchMenuState>,
    mut buttons: Query<(&MatchMenuButton, &mut SurfaceState)>,
) {
    for (button, mut state) in &mut buttons {
        state.focused = button.0 == menu.selected;
    }
}
