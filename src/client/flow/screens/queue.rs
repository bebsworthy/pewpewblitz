//! Queue-screen presentation and queue copy.

use crate::client::{
    ClientLobbyMembership, ClientQueueModel,
    flow::{
        actions::FlowUiAction,
        model::ClientFlow,
        screens::shared::{FlowButton, FlowNavigation, FlowRoot, flow_root_node, spawn_heading},
    },
};
use bevy::{prelude::*, ui::InteractionDisabled};
use lightyear::prelude::client::Client;

#[derive(Component)]
pub(in crate::client::flow) struct QueueStatusLabel;

#[derive(Component)]
pub(in crate::client::flow) struct QueueCancelButton;

#[derive(Component)]
pub(in crate::client::flow) struct QueueCancelLabel;

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::client::flow) fn spawn_queue(
    mut commands: Commands,
    queue: Res<ClientQueueModel>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    builds: Res<crate::builds::BuildCatalogResource>,
    mut navigation: ResMut<FlowNavigation>,
) {
    let Some(membership) = queue.membership() else {
        return;
    };
    navigation.selected = 0;
    let lobby = memberships.iter().next();
    commands
        .spawn((
            FlowRoot,
            DespawnOnExit(ClientFlow::Queue),
            flow_root_node(),
            BackgroundColor(Color::srgb(0.025, 0.04, 0.07)),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            spawn_heading(root, "QUEUE");
            root.spawn((
                QueueStatusLabel,
                Text::new(queue_membership_text(&queue, membership, lobby, &builds.0)),
                TextFont::from_font_size(20.0),
                TextColor(Color::srgb(0.86, 0.94, 1.0)),
                TextLayout::new(Justify::Center, LineBreak::WordBoundary),
            ));
            spawn_queue_cancel_button(root);
        });
}

pub(in crate::client::flow) fn queue_population(
    queue: &ClientQueueModel,
    game: &crate::lobby::AdvertisedGameType,
) -> String {
    queue
        .snapshot()
        .and_then(|snapshot| {
            snapshot
                .pools
                .iter()
                .find(|row| row.game_type_id == game.id)
        })
        .map_or_else(
            || "Updating queue".to_string(),
            |row| {
                format!(
                    "{} waiting - {} players per match",
                    row.queued, row.formation_size
                )
            },
        )
}

pub(in crate::client::flow) fn queue_membership_text(
    queue: &ClientQueueModel,
    membership: &crate::lobby::QueueMembership,
    lobby: Option<&ClientLobbyMembership>,
    builds: &crate::builds::BuildCatalog,
) -> String {
    let population = if queue.required_snapshot_is_fresh() {
        queue
            .raw_snapshot()
            .and_then(|snapshot| {
                snapshot
                    .pools
                    .iter()
                    .find(|row| row.game_type_id == membership.game_type_id)
            })
            .map_or_else(
                || "Updating queue".to_string(),
                |row| {
                    format!(
                        "{} waiting · {} players per match",
                        row.queued, row.formation_size
                    )
                },
            )
    } else {
        "Updating queue".to_string()
    };
    let game_name = lobby
        .and_then(|lobby| {
            lobby
                .game_types
                .iter()
                .find(|game| game.id == membership.game_type_id)
        })
        .map_or(membership.game_type_id.as_str(), |game| {
            game.display_name.as_str()
        });
    let recipe = membership.accepted_build.canonical_recipe;
    let ultimate = builds
        .ultimates
        .iter()
        .find(|definition| definition.id == recipe.ultimate)
        .map_or("Unknown ultimate", |definition| {
            definition.display_name.as_str()
        });
    let passives = recipe.passives.map(|id| {
        builds
            .passives
            .iter()
            .find(|definition| definition.id == id)
            .map_or("Unknown passive", |definition| {
                definition.display_name.as_str()
            })
    });
    format!(
        "{game_name}\n{population}\nSaved brawler accepted\n{ultimate} · {} / {}",
        passives[0], passives[1],
    )
}

fn spawn_queue_cancel_button(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            QueueCancelButton,
            FlowButton {
                index: 0,
                action: FlowUiAction::CancelQueue,
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
        .with_child((
            QueueCancelLabel,
            Text::new("CANCEL QUEUE"),
            TextFont::from_font_size(18.0),
        ));
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::client::flow) fn update_queue_cancel_button(
    mut commands: Commands,
    queue: Res<ClientQueueModel>,
    buttons: Query<Entity, With<QueueCancelButton>>,
    mut labels: Query<&mut Text, With<QueueCancelLabel>>,
) {
    let (label, cancelling) =
        queue_cancel_presentation(queue.pending().map(|pending| &pending.command));
    for entity in &buttons {
        if cancelling {
            commands.entity(entity).insert(InteractionDisabled);
        } else {
            commands.entity(entity).remove::<InteractionDisabled>();
        }
    }
    for mut text in &mut labels {
        text.0 = label.to_string();
    }
}

pub(in crate::client::flow) fn queue_cancel_presentation(
    pending: Option<&crate::lobby::QueueCommand>,
) -> (&'static str, bool) {
    if pending.is_some_and(|command| matches!(command, crate::lobby::QueueCommand::Cancel(_))) {
        ("CANCELLING…", true)
    } else {
        ("CANCEL QUEUE", false)
    }
}

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn update_queue_status(
    queue: Res<ClientQueueModel>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    builds: Res<crate::builds::BuildCatalogResource>,
    mut labels: Query<&mut Text, With<QueueStatusLabel>>,
) {
    let Some(membership) = queue.membership() else {
        return;
    };
    let lobby = memberships.iter().next();
    for mut label in &mut labels {
        label.0 = queue_membership_text(&queue, membership, lobby, &builds.0);
    }
}
