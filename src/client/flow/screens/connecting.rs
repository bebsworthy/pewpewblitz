//! Connecting-screen presentation.

use crate::client::{
    ClientAssetHandles,
    flow::{
        actions::FlowUiAction,
        connection::{ConnectionStage, PendingConnection},
        model::ClientFlow,
        screens::{
            connection_shell::{
                action_node, root_background, root_node, spawn_action, spawn_background,
                spawn_logo, spawn_text,
            },
            shared::{FlowNavigation, FlowRoot},
        },
    },
    ui::{
        surface::Surface,
        theme::{INK, ORANGE, TEAL, font},
    },
};
use bevy::prelude::*;

#[derive(Component)]
pub(in crate::client::flow) struct ConnectingPhaseLabel;

#[derive(Component)]
pub(in crate::client::flow) struct ConnectingDetailLabel;

#[derive(Component)]
pub(in crate::client::flow) struct ConnectingProgressSegment(usize);

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::client::flow) fn spawn_connecting(
    mut commands: Commands,
    pending: Option<Res<PendingConnection>>,
    mut navigation: ResMut<FlowNavigation>,
    assets: Option<Res<ClientAssetHandles>>,
    server: Option<Res<AssetServer>>,
) {
    navigation.selected = 0;
    let address = pending.as_ref().map_or("server", |pending| {
        pending.target.logical_address.canonical()
    });
    commands
        .spawn((
            FlowRoot,
            DespawnOnExit(ClientFlow::Connecting),
            root_node(),
            root_background(),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            spawn_background(root, server.as_deref());
            spawn_logo(root, assets.as_deref(), 360.0);
            root.spawn((
                Surface::Panel,
                Node {
                    width: percent(88),
                    max_width: px(820),
                    min_height: px(390),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: px(20),
                    padding: UiRect::axes(px(50), px(36)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
            ))
            .with_children(|panel| {
                spawn_text(panel, assets.as_deref(), "CONNECTING", 54.0);
                panel.spawn((
                    Text::new(address),
                    font(assets.as_deref(), 22.0),
                    TextColor(TEAL),
                    TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                ));
                panel
                    .spawn((Node {
                        width: percent(78),
                        max_width: px(520),
                        height: px(12),
                        column_gap: px(8),
                        ..default()
                    },))
                    .with_children(|progress| {
                        for step in 0..3 {
                            progress.spawn((
                                ConnectingProgressSegment(step),
                                Node {
                                    height: percent(100),
                                    flex_grow: 1.0,
                                    ..default()
                                },
                                BackgroundColor(if step == 0 {
                                    ORANGE
                                } else {
                                    INK.with_alpha(0.18)
                                }),
                            ));
                        }
                    });
                panel.spawn((
                    ConnectingPhaseLabel,
                    Text::new("PREPARING CONNECTION"),
                    font(assets.as_deref(), 28.0),
                    TextColor(INK),
                    TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                ));
                panel.spawn((
                    ConnectingDetailLabel,
                    Text::new("Starting connection"),
                    font(assets.as_deref(), 18.0),
                    TextColor(INK),
                    TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                ));
                spawn_action(
                    panel,
                    assets.as_deref(),
                    0,
                    FlowUiAction::Cancel,
                    "CANCEL",
                    Some(Surface::Secondary),
                    false,
                    action_node(percent(70), 64.0),
                );
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
    mut phase_labels: Query<
        &mut Text,
        (With<ConnectingPhaseLabel>, Without<ConnectingDetailLabel>),
    >,
    mut detail_labels: Query<
        &mut Text,
        (With<ConnectingDetailLabel>, Without<ConnectingPhaseLabel>),
    >,
    mut segments: Query<(&ConnectingProgressSegment, &mut BackgroundColor)>,
) {
    if *flow.get() != ClientFlow::Connecting {
        return;
    }
    let Some(pending) = pending else {
        return;
    };
    let now = time.elapsed();
    let remaining = pending.overall_deadline.saturating_sub(now);
    let remaining_seconds = remaining
        .as_secs()
        .saturating_add(u64::from(remaining.subsec_nanos() != 0));
    let (step, phase, detail) = match pending.stage {
        ConnectionStage::ResolvingAddress => (
            0,
            "RESOLVING SERVER",
            format!("Looking up the server address · up to {remaining_seconds}s"),
        ),
        ConnectionStage::ContactingServer { current, total } => (
            1,
            "CONTACTING SERVER",
            format!("Connection candidate {current} of {total} · up to {remaining_seconds}s"),
        ),
        ConnectionStage::JoiningLobby => (
            2,
            "JOINING LOBBY",
            format!("Checking compatibility and game list · up to {remaining_seconds}s"),
        ),
    };
    for mut label in &mut phase_labels {
        label.0 = phase.to_string();
    }
    for mut label in &mut detail_labels {
        label.0.clone_from(&detail);
    }
    for (segment, mut color) in &mut segments {
        color.0 = match segment.0.cmp(&step) {
            std::cmp::Ordering::Less => TEAL,
            std::cmp::Ordering::Equal => ORANGE,
            std::cmp::Ordering::Greater => INK.with_alpha(0.18),
        };
    }
}
