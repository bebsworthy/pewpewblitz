//! Server-selection screen ownership and bounded root replacement.

use crate::client::flow::{
    actions::{FlowCommit, FlowUiAction},
    input::edited_value,
    model::ClientFlow,
    persistence::ConnectionPersistence,
    screens::{
        connection_shell::{
            action_node, root_background, root_node, spawn_action, spawn_background, spawn_logo,
            spawn_text,
        },
        shared::{FlowButton, FlowNavigation, FlowRoot},
    },
};
use crate::client::{
    ClientAssetHandles,
    ui::{
        surface::{Surface, SurfaceState},
        theme::{INK, ORANGE, font},
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
    assets: Option<&ClientAssetHandles>,
    index: usize,
    action: FlowUiAction,
    field: FieldLabel,
    label: &str,
) {
    parent
        .spawn((
            Button,
            AccessibleLabel::new(label),
            FlowButton {
                index,
                action,
                error_action: false,
            },
            Surface::Field,
            SurfaceState::default(),
            Node {
                width: percent(100),
                min_height: px(72),
                padding: UiRect::axes(px(30), px(14)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: px(24),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
        ))
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                font(assets, 22.0),
                TextColor(INK),
                Node {
                    flex_shrink: 0.0,
                    ..default()
                },
            ));
            row.spawn((
                field,
                Text::new(""),
                font(assets, 22.0),
                TextColor(INK),
                TextLayout::new(Justify::Right, LineBreak::AnyCharacter),
                Node {
                    min_width: px(0),
                    ..default()
                },
            ));
        });
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
    assets: Option<Res<ClientAssetHandles>>,
    server: Option<Res<AssetServer>>,
) {
    spawn_server_select_root(
        &mut commands,
        &model,
        &persistence,
        &mut navigation,
        None,
        assets.as_deref(),
        server.as_deref(),
    );
}

pub(in crate::client::flow) fn spawn_server_select_root(
    commands: &mut Commands,
    model: &ServerSelectModel,
    persistence: &ConnectionPersistence,
    navigation: &mut FlowNavigation,
    requested_selection: Option<usize>,
    assets: Option<&ClientAssetHandles>,
    server: Option<&AssetServer>,
) {
    navigation.selected = requested_selection.unwrap_or(2);
    commands
        .spawn((
            FlowRoot,
            DespawnOnExit(ClientFlow::ServerSelect),
            root_node(),
            root_background(),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            spawn_background(root, server);
            root.spawn(Node {
                width: percent(100),
                max_width: px(1120),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: px(24),
                ..default()
            })
            .with_children(|header| {
                spawn_text(header, assets, "SELECT SERVER", 54.0);
                spawn_logo(header, assets, 220.0);
            });
            root.spawn(Node {
                width: percent(100),
                max_width: px(980),
                flex_direction: FlexDirection::Column,
                row_gap: px(12),
                ..default()
            })
            .with_children(|content| {
                spawn_editor_button(
                    content,
                    assets,
                    0,
                    FlowUiAction::EditAddress,
                    FieldLabel::Address,
                    "SERVER ADDRESS",
                );
                spawn_editor_button(
                    content,
                    assets,
                    1,
                    FlowUiAction::EditName,
                    FieldLabel::Name,
                    "PLAYER NAME",
                );
                spawn_action(
                    content,
                    assets,
                    2,
                    FlowUiAction::Connect,
                    "CONNECT",
                    Some(Surface::Primary),
                    false,
                    action_node(percent(100), 72.0),
                );
                if let Some(error) = model.inline_error.as_ref() {
                    content.spawn((
                        Text::new(error.clone()),
                        font(assets, 18.0),
                        TextColor(ORANGE),
                        TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                    ));
                }
                spawn_saved_servers(content, assets, persistence);
            });
        });
}

fn spawn_saved_servers(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&ClientAssetHandles>,
    persistence: &ConnectionPersistence,
) {
    let has_saved =
        !persistence.state.favorites.is_empty() || !persistence.state.recents.is_empty();
    if has_saved {
        spawn_text(parent, assets, "SAVED SERVERS", 24.0);
    }
    parent
        .spawn(Node {
            width: percent(100),
            flex_wrap: FlexWrap::Wrap,
            column_gap: percent(2),
            row_gap: px(10),
            ..default()
        })
        .with_children(|cards| {
            let mut index = 3;
            for favorite in &persistence.state.favorites {
                let join_label = format!("{}  ·  {}", favorite.name, favorite.address);
                spawn_action(
                    cards,
                    assets,
                    index,
                    FlowUiAction::JoinSaved(favorite.address.clone()),
                    &join_label,
                    Some(Surface::Row),
                    false,
                    action_node(percent(74), 62.0),
                );
                index += 1;
                spawn_action(
                    cards,
                    assets,
                    index,
                    FlowUiAction::RemoveFavorite(favorite.address.clone()),
                    "REMOVE",
                    Some(Surface::Quiet),
                    false,
                    action_node(percent(24), 62.0),
                );
                index += 1;
            }
            for recent in &persistence.state.recents {
                let label = format!("{}  ·  {}", recent.server_name, recent.address);
                spawn_action(
                    cards,
                    assets,
                    index,
                    FlowUiAction::JoinSaved(recent.address.clone()),
                    &label,
                    Some(Surface::Row),
                    false,
                    action_node(percent(49), 62.0),
                );
                index += 1;
            }
            spawn_action(
                cards,
                assets,
                index,
                FlowUiAction::OpenSettings,
                "SETTINGS",
                Some(Surface::Quiet),
                false,
                action_node(auto(), 54.0),
            );
            spawn_action(
                cards,
                assets,
                index + 1,
                FlowUiAction::Quit,
                "QUIT",
                Some(Surface::Quiet),
                false,
                action_node(auto(), 54.0),
            );
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
    assets: Option<Res<ClientAssetHandles>>,
    server: Option<Res<AssetServer>>,
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
        assets.as_deref(),
        server.as_deref(),
    );
}
