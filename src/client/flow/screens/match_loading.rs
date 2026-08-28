//! Match-loading screen presentation.

use crate::client::{
    ClientMatchLoadingModel,
    flow::{
        actions::FlowUiAction,
        model::ClientFlow,
        screens::shared::{
            FlowNavigation, FlowRoot, flow_root_node, spawn_flow_button, spawn_heading,
        },
    },
};
use bevy::prelude::*;

#[derive(Component)]
pub(in crate::client::flow) struct MatchLoadingStatusLabel;

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::client::flow) fn spawn_match_loading(
    mut commands: Commands,
    loading: Res<ClientMatchLoadingModel>,
    mut navigation: ResMut<FlowNavigation>,
) {
    let Some(active) = loading.active() else {
        return;
    };
    navigation.selected = 0;
    commands
        .spawn((
            FlowRoot,
            DespawnOnExit(ClientFlow::MatchLoading),
            flow_root_node(),
            BackgroundColor(Color::srgb(0.025, 0.04, 0.07)),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            spawn_heading(root, "MATCH LOADING");
            root.spawn((
                MatchLoadingStatusLabel,
                Text::new(match_loading_text(active, loading.phase())),
                TextFont::from_font_size(20.0),
                TextColor(Color::srgb(0.86, 0.94, 1.0)),
                TextLayout::new(Justify::Center, LineBreak::WordBoundary),
            ));
            spawn_flow_button(
                root,
                0,
                FlowUiAction::RequestCancelMatchStart,
                "CANCEL MATCH START",
            );
        });
}

fn match_loading_text(
    active: &crate::lobby::ReservationStarted,
    phase: Option<crate::lobby::MatchLoadingPhase>,
) -> String {
    let phase = match phase.unwrap_or(crate::lobby::MatchLoadingPhase::Reserving) {
        crate::lobby::MatchLoadingPhase::Reserving => "Reserving roster",
        crate::lobby::MatchLoadingPhase::StartingServer => "Starting server",
        crate::lobby::MatchLoadingPhase::Connecting => "Connecting",
        crate::lobby::MatchLoadingPhase::Synchronizing => "Synchronizing map",
        crate::lobby::MatchLoadingPhase::WaitingForPlayers => "Waiting for players",
        crate::lobby::MatchLoadingPhase::Cancelling => "Cancelling",
        crate::lobby::MatchLoadingPhase::ReturningToQueue => "Returning to queue",
    };
    format!(
        "{phase}\n{}v{} · Map {}\nYour accepted build: {}/12 points",
        active.players_per_team,
        active.players_per_team,
        active.map_preset_id.0,
        active.accepted_build.total_points
    )
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::client::flow) fn update_match_loading(
    loading: Res<ClientMatchLoadingModel>,
    mut labels: Query<&mut Text, With<MatchLoadingStatusLabel>>,
) {
    let Some(active) = loading.active() else {
        return;
    };
    for mut label in &mut labels {
        label.0 = match_loading_text(active, loading.phase());
    }
}
