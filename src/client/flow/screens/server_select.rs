//! Server-selection screen ownership and bounded root replacement.

use crate::client::flow::{
    actions::{FlowCommit, FlowUiAction},
    input::edited_value,
    model::ClientFlow,
    persistence::ConnectionPersistence,
    screens::shared::{
        FlowButton, FlowNavigation, FlowRoot, flow_root_node, spawn_flow_button, spawn_heading,
    },
};
use bevy::prelude::*;

#[derive(Resource, Clone, Debug)]
pub(in crate::client::flow) struct ServerSelectModel {
    pub(in crate::client::flow) address: String,
    pub(in crate::client::flow) committed_name: String,
    pub(in crate::client::flow) name: String,
    pub(in crate::client::flow) editing: Option<EditingField>,
    pub(in crate::client::flow) caret: usize,
    pub(in crate::client::flow) inline_error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::client::flow) enum EditingField {
    Address,
    Name,
}

#[derive(Component, Clone, Copy)]
pub(in crate::client::flow) enum FieldLabel {
    Address,
    Name,
}

fn spawn_editor_button(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    field: FieldLabel,
) {
    parent
        .spawn((
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
        ))
        .with_child((field, Text::new(""), TextFont::from_font_size(18.0)));
}

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
            spawn_editor_button(root, 0, FlowUiAction::EditAddress, FieldLabel::Address);
            spawn_editor_button(root, 1, FlowUiAction::EditName, FieldLabel::Name);
            spawn_flow_button(root, 2, FlowUiAction::Connect, "CONNECT");
            let mut index = 3;
            for favorite in &persistence.state.favorites {
                spawn_flow_button(
                    root,
                    index,
                    FlowUiAction::JoinSaved(favorite.address.clone()),
                    &format!("JOIN {} - {}", favorite.name, favorite.address),
                );
                index += 1;
                spawn_flow_button(
                    root,
                    index,
                    FlowUiAction::RemoveFavorite(favorite.address.clone()),
                    &format!("REMOVE {}", favorite.name),
                );
                index += 1;
            }
            for recent in &persistence.state.recents {
                spawn_flow_button(
                    root,
                    index,
                    FlowUiAction::JoinSaved(recent.address.clone()),
                    &format!("RECENT {} - {}", recent.server_name, recent.address),
                );
                index += 1;
            }
            spawn_flow_button(root, index, FlowUiAction::OpenSettings, "SETTINGS");
            spawn_flow_button(root, index + 1, FlowUiAction::Quit, "QUIT");
            if let Some(error) = model.inline_error.as_ref() {
                root.spawn((
                    Text::new(error.clone()),
                    TextColor(Color::srgb(1.0, 0.55, 0.45)),
                ));
            }
        });
}

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn update_server_select_copy(
    model: Res<ServerSelectModel>,
    mut fields: Query<(&FieldLabel, &mut Text)>,
) {
    for (field, mut text) in &mut fields {
        text.0 = match field {
            FieldLabel::Address => format!(
                "ADDRESS: {}",
                render_editor_value(&model, EditingField::Address)
            ),
            FieldLabel::Name => {
                format!("NAME: {}", render_editor_value(&model, EditingField::Name))
            }
        };
    }
}

fn render_editor_value(model: &ServerSelectModel, field: EditingField) -> String {
    let value = edited_value(model, field);
    if model.editing != Some(field) {
        return value.to_string();
    }
    let caret = model.caret.min(value.len());
    format!("{}|{}", &value[..caret], &value[caret..])
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
