//! Connecting-screen presentation.

use crate::client::{
    ClientAssetHandles,
    flow::{
        actions::FlowUiAction,
        connection::{PendingConnection, connection_presentation},
        model::ClientFlow,
        screens::shared::{
            FlowNavigation, FlowRoot, flow_root_node, spawn_flow_button, spawn_heading,
        },
    },
};
use bevy::prelude::*;

#[derive(Component)]
pub(in crate::client::flow) struct ConnectingLabel;

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::client::flow) fn spawn_connecting(
    mut commands: Commands,
    pending: Option<Res<PendingConnection>>,
    mut navigation: ResMut<FlowNavigation>,
    assets: Option<Res<ClientAssetHandles>>,
) {
    navigation.selected = 0;
    let address = pending.as_ref().map_or("server", |pending| {
        pending.target.logical_address.canonical()
    });
    commands
        .spawn((
            FlowRoot,
            DespawnOnExit(ClientFlow::Connecting),
            flow_root_node(),
            BackgroundColor(Color::srgb(0.025, 0.04, 0.07)),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            if let Some(assets) = assets.as_deref() {
                root.spawn((
                    ImageNode::new(assets.loading_logo.clone()),
                    Node {
                        width: percent(62),
                        max_width: px(560),
                        height: auto(),
                        margin: UiRect::bottom(px(18)),
                        ..default()
                    },
                ));
            } else {
                spawn_heading(root, "PEWPEW BLITZ");
            }
            root.spawn((
                Node {
                    width: percent(88),
                    max_width: px(720),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(18),
                    padding: UiRect::all(px(28)),
                    border: UiRect::all(px(2)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.055, 0.08, 0.12)),
                BorderColor::all(Color::srgb(0.12, 0.32, 0.42)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    ConnectingLabel,
                    Text::new(format!("PREPARING CONNECTION\n{address}")),
                    TextFont::from_font_size(22.0),
                    TextColor(Color::srgb(0.9, 0.95, 1.0)),
                    TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                ));
                spawn_flow_button(panel, 0, FlowUiAction::Cancel, "CANCEL");
                spawn_flow_button(panel, 1, FlowUiAction::OpenSettings, "SETTINGS");
                spawn_flow_button(panel, 2, FlowUiAction::Quit, "QUIT");
                panel.spawn((
                    Text::new("ESC / PAD EAST  -  CANCEL"),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::srgb(0.58, 0.66, 0.74)),
                ));
            });
        });
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::client::flow) fn update_connecting_copy(
    time: Res<Time<Real>>,
    flow: Res<State<ClientFlow>>,
    pending: Option<Res<PendingConnection>>,
    mut labels: Query<&mut Text, With<ConnectingLabel>>,
) {
    if *flow.get() != ClientFlow::Connecting {
        return;
    }
    let Some(pending) = pending else {
        return;
    };
    for mut label in &mut labels {
        label.0 = connection_presentation(&pending, time.elapsed());
    }
}
