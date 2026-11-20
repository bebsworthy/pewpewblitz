//! Queue-screen presentation and queue copy.

use crate::{
    ClientAssetHandles, ClientLobbyMembership, ClientQueueModel,
    flow::{
        actions::FlowUiAction,
        model::ClientFlow,
        screens::{
            connection_shell::{
                action_node, root_background, root_node, spawn_action, spawn_background, spawn_text,
            },
            dashboard::{dashboard_badge_index, dashboard_mode_title},
            shared::{FlowNavigation, FlowRoot},
        },
    },
    ui::{
        surface::Surface,
        theme::{ACCENT, TEXT, font},
    },
};
use bevy::{prelude::*, ui::InteractionDisabled};
use lightyear::prelude::client::Client;

/// One live line of the queue panel, rewritten from [`queue_copy`] while the screen is open.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::flow) enum QueueLabel {
    Mode,
    Game,
    Population,
    Brawler,
    Build,
}

#[derive(Component)]
pub(in crate::flow) struct QueueCancelButton;

/// The queue panel's copy for one membership snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::flow) struct QueueCopy {
    pub(in crate::flow) mode: String,
    pub(in crate::flow) game: String,
    pub(in crate::flow) population: String,
    pub(in crate::flow) brawler: String,
    pub(in crate::flow) build: String,
}

impl QueueCopy {
    fn line(&self, label: QueueLabel) -> &str {
        match label {
            QueueLabel::Mode => &self.mode,
            QueueLabel::Game => &self.game,
            QueueLabel::Population => &self.population,
            QueueLabel::Brawler => &self.brawler,
            QueueLabel::Build => &self.build,
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::flow) fn spawn_queue(
    mut commands: Commands,
    queue: Res<ClientQueueModel>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    builds: Res<brawler::builds::BuildCatalogResource>,
    mut navigation: ResMut<FlowNavigation>,
    assets: Option<Res<ClientAssetHandles>>,
    server: Option<Res<AssetServer>>,
) {
    let Some(membership) = queue.membership() else {
        return;
    };
    navigation.focused = 0;
    let lobby = memberships.iter().next();
    let copy = queue_copy(&queue, membership, lobby, &builds.0);
    let assets = assets.as_deref();
    let pennant = queued_game(membership, lobby)
        .zip(assets)
        .map(|(game, assets)| assets.dashboard_badges[dashboard_badge_index(game)].clone());
    let ultimate_badge = builds
        .0
        .ultimates
        .iter()
        .find(|definition| definition.id == membership.accepted_build.character.ultimate_id)
        .zip(server.as_deref())
        .map(|(definition, server)| {
            server.load(crate::ui::icons::path(crate::ui::icons::ultimate_icon(
                definition.kind,
            )))
        });
    let (cancel_label, cancelling) =
        queue_cancel_presentation(queue.pending().map(|pending| &pending.command));
    commands
        .spawn((
            FlowRoot,
            DespawnOnExit(ClientFlow::Queue),
            Node {
                justify_content: JustifyContent::Center,
                ..root_node()
            },
            root_background(),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            spawn_background(root, server.as_deref());
            root.spawn((
                Surface::Panel,
                Node {
                    width: percent(88),
                    max_width: px(900),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(18),
                    padding: UiRect::axes(px(44), px(30)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
            ))
            .with_children(|panel| {
                spawn_text(panel, assets, "FINDING A MATCH", 46.0);
                spawn_label(panel, assets, &copy, QueueLabel::Population, 24.0, ACCENT);
                panel
                    .spawn(Node {
                        width: percent(100),
                        column_gap: px(16),
                        align_items: AlignItems::Stretch,
                        ..default()
                    })
                    .with_children(|tiles| {
                        spawn_tile(
                            tiles,
                            assets,
                            &copy,
                            pennant.map(|image| (image, Vec2::new(64.0, 80.0))),
                            [QueueLabel::Mode, QueueLabel::Game],
                        );
                        spawn_tile(
                            tiles,
                            assets,
                            &copy,
                            ultimate_badge.map(|image| (image, Vec2::new(84.0, 70.0))),
                            [QueueLabel::Brawler, QueueLabel::Build],
                        );
                    });
                let cancel = spawn_action(
                    panel,
                    assets,
                    0,
                    FlowUiAction::CancelQueue,
                    cancel_label,
                    Some(Surface::Secondary),
                    false,
                    Node {
                        max_width: px(520),
                        margin: UiRect::top(px(4)),
                        ..action_node(percent(70), 60.0)
                    },
                );
                let mut commands = panel.commands();
                let mut cancel = commands.entity(cancel);
                cancel.insert(QueueCancelButton);
                if cancelling {
                    cancel.insert(InteractionDisabled);
                }
            });
        });
}

/// One half of the queue panel: an emblem beside a title line and a detail line.
fn spawn_tile(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&ClientAssetHandles>,
    copy: &QueueCopy,
    emblem: Option<(Handle<Image>, Vec2)>,
    [title, detail]: [QueueLabel; 2],
) {
    parent
        .spawn((
            Surface::Dark,
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                min_height: px(118),
                column_gap: px(18),
                align_items: AlignItems::Center,
                padding: UiRect::axes(px(22), px(14)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
        ))
        .with_children(|tile| {
            if let Some((image, size)) = emblem {
                tile.spawn((
                    ImageNode::new(image),
                    Node {
                        width: px(size.x),
                        height: px(size.y),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
            }
            tile.spawn(Node {
                flex_direction: FlexDirection::Column,
                flex_shrink: 1.0,
                row_gap: px(4),
                ..default()
            })
            .with_children(|lines| {
                spawn_label(lines, assets, copy, title, 32.0, TEXT);
                spawn_label(lines, assets, copy, detail, 20.0, TEXT.with_alpha(0.82));
            });
        });
}

fn spawn_label(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&ClientAssetHandles>,
    copy: &QueueCopy,
    label: QueueLabel,
    size: f32,
    color: Color,
) {
    parent.spawn((
        label,
        Text::new(copy.line(label)),
        font(assets, size),
        TextColor(color),
        TextLayout::new(
            if label == QueueLabel::Population {
                Justify::Center
            } else {
                Justify::Left
            },
            LineBreak::WordBoundary,
        ),
    ));
}

fn queued_game<'a>(
    membership: &brawler::lobby::QueueMembership,
    lobby: Option<&'a ClientLobbyMembership>,
) -> Option<&'a brawler::lobby::AdvertisedGameType> {
    lobby.and_then(|lobby| {
        lobby
            .game_types
            .iter()
            .find(|game| game.id == membership.game_type_id)
    })
}

pub(in crate::flow) fn queue_copy(
    queue: &ClientQueueModel,
    membership: &brawler::lobby::QueueMembership,
    lobby: Option<&ClientLobbyMembership>,
    builds: &brawler::builds::BuildCatalog,
) -> QueueCopy {
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
    let game = queued_game(membership, lobby);
    let pick = membership.accepted_build.character;
    let brawler = lobby
        .and_then(|lobby| lobby.character_catalog.character(pick.character_id))
        .map_or_else(
            || "Brawler".to_string(),
            |character| character.display_name.clone(),
        );
    let ultimate = builds
        .ultimate(pick.ultimate_id)
        .map_or("Unknown ultimate", |definition| {
            definition.display_name.as_str()
        });
    let passive = builds
        .passive(pick.passive_id)
        .map_or("Unknown passive", |definition| {
            definition.display_name.as_str()
        });
    QueueCopy {
        mode: game.map_or("GAME", dashboard_mode_title).to_string(),
        game: game
            .map_or(membership.game_type_id.as_str(), |game| {
                game.display_name.as_str()
            })
            .to_string(),
        population,
        brawler,
        build: format!("{ultimate} · {passive}"),
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::flow) fn update_queue_cancel_button(
    mut commands: Commands,
    queue: Res<ClientQueueModel>,
    buttons: Query<(Entity, &Children), With<QueueCancelButton>>,
    mut labels: Query<&mut Text>,
) {
    let (label, cancelling) =
        queue_cancel_presentation(queue.pending().map(|pending| &pending.command));
    for (entity, children) in &buttons {
        if cancelling {
            commands.entity(entity).insert(InteractionDisabled);
        } else {
            commands.entity(entity).remove::<InteractionDisabled>();
        }
        for child in children.iter() {
            if let Ok(mut text) = labels.get_mut(child)
                && text.0 != label
            {
                label.clone_into(&mut text.0);
            }
        }
    }
}

pub(in crate::flow) fn queue_cancel_presentation(
    pending: Option<&brawler::lobby::QueueCommand>,
) -> (&'static str, bool) {
    if pending.is_some_and(|command| matches!(command, brawler::lobby::QueueCommand::Cancel(_))) {
        ("CANCELLING...", true)
    } else {
        ("CANCEL QUEUE", false)
    }
}

#[allow(clippy::needless_pass_by_value)]
pub(in crate::flow) fn update_queue_status(
    queue: Res<ClientQueueModel>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    builds: Res<brawler::builds::BuildCatalogResource>,
    mut labels: Query<(&mut Text, &QueueLabel)>,
) {
    let Some(membership) = queue.membership() else {
        return;
    };
    let copy = queue_copy(&queue, membership, memberships.iter().next(), &builds.0);
    for (mut text, label) in &mut labels {
        let line = copy.line(*label);
        if text.0 != line {
            line.clone_into(&mut text.0);
        }
    }
}
