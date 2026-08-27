//! Server-selection screen ownership and bounded root replacement.

use super::super::{
    ClientFlow, ConnectionPersistence, FieldLabel, FlowCommit, FlowNavigation, FlowRoot,
    FlowUiAction, ServerSelectModel, flow_root_node, spawn_flow_button, spawn_heading,
};
use bevy::prelude::*;

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::client::flow) fn spawn_server_select(
    mut commands: Commands,
    model: Res<ServerSelectModel>,
    persistence: Res<ConnectionPersistence>,
    mut navigation: ResMut<FlowNavigation>,
) {
    spawn_server_select_root(&mut commands, &model, &persistence, &mut navigation, None);
}

pub(in crate::client::flow) fn spawn_server_select_root(
    commands: &mut Commands,
    model: &ServerSelectModel,
    persistence: &ConnectionPersistence,
    navigation: &mut FlowNavigation,
    requested_selection: Option<usize>,
) {
    navigation.selected = requested_selection.unwrap_or(2);
    commands
        .spawn((
            FlowRoot,
            DespawnOnExit(ClientFlow::ServerSelect),
            flow_root_node(),
            BackgroundColor(Color::srgb(0.025, 0.04, 0.07)),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            spawn_heading(root, "SERVER SELECT");
            spawn_flow_button(
                root,
                0,
                FlowUiAction::EditAddress,
                "",
                Some(FieldLabel::Address),
            );
            spawn_flow_button(root, 1, FlowUiAction::EditName, "", Some(FieldLabel::Name));
            spawn_flow_button(root, 2, FlowUiAction::Connect, "CONNECT", None);
            let mut index = 3;
            for favorite in &persistence.state.favorites {
                spawn_flow_button(
                    root,
                    index,
                    FlowUiAction::JoinSaved(favorite.address.clone()),
                    &format!("JOIN {} - {}", favorite.name, favorite.address),
                    None,
                );
                index += 1;
                spawn_flow_button(
                    root,
                    index,
                    FlowUiAction::RemoveFavorite(favorite.address.clone()),
                    &format!("REMOVE {}", favorite.name),
                    None,
                );
                index += 1;
            }
            for recent in &persistence.state.recents {
                spawn_flow_button(
                    root,
                    index,
                    FlowUiAction::JoinSaved(recent.address.clone()),
                    &format!("RECENT {} - {}", recent.server_name, recent.address),
                    None,
                );
                index += 1;
            }
            spawn_flow_button(root, index, FlowUiAction::OpenSettings, "SETTINGS", None);
            spawn_flow_button(root, index + 1, FlowUiAction::Quit, "QUIT", None);
            if let Some(error) = model.inline_error.as_ref() {
                root.spawn((
                    Text::new(error.clone()),
                    TextColor(Color::srgb(1.0, 0.55, 0.45)),
                ));
            }
        });
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "the bounded rebuild phase owns one complete server-select root replacement"
)]
pub(in crate::client::flow) fn refresh_server_select(
    mut commands: Commands,
    commit: Res<FlowCommit>,
    flow: Res<State<ClientFlow>>,
    roots: Query<Entity, With<FlowRoot>>,
    model: Res<ServerSelectModel>,
    persistence: Res<ConnectionPersistence>,
    mut navigation: ResMut<FlowNavigation>,
) {
    let Some(selection) = commit.refresh_server_select else {
        return;
    };
    if *flow.get() != ClientFlow::ServerSelect {
        return;
    }
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    spawn_server_select_root(
        &mut commands,
        &model,
        &persistence,
        &mut navigation,
        Some(selection),
    );
}
