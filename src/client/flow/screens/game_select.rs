//! Mode cards and exact advertised variant selection; confirmation remains flow-owned.
use super::scroll::{clamp_scroll_offset, normalized_wheel_delta, offset_keeping_interval_visible};
use super::shared::{FlowButton, FlowNavigation, FlowRoot};
use crate::client::ui::theme::{CREAM, INK, font};
use crate::client::{
    ClientLobbyMembership,
    flow::{actions::FlowUiAction, model::ClientFlow},
};
use crate::lobby::{AdvertisedGameType, AdvertisedRulesSummary};
use bevy::{
    input::mouse::MouseWheel,
    prelude::*,
    ui::{InteractionDisabled, ScrollPosition},
};
use lightyear::prelude::client::Client;

pub(in crate::client::flow) const GAME_TYPE_CONFIRM_INDEX: usize = 1_000;
#[derive(Resource, Default, Clone, Debug, PartialEq, Eq)]
pub(in crate::client::flow) struct GameTypeSelectionDraft {
    pub(in crate::client::flow) selected_index: Option<usize>,
    pub(in crate::client::flow) unavailable_previous: bool,
}
#[derive(Component)]
pub(in crate::client::flow) struct GameTypeSelectRoot;
#[derive(Component)]
pub(in crate::client::flow) struct GameSelectionColumns(pub usize);
#[derive(Component)]
pub(in crate::client::flow) struct RenderedSelection {
    draft: GameTypeSelectionDraft,
    games: Vec<AdvertisedGameType>,
    width: f32,
    height: f32,
}

fn mode_index(game: &AdvertisedGameType) -> usize {
    match game.rules_summary {
        AdvertisedRulesSummary::Wipeout { .. } => 0,
        AdvertisedRulesSummary::HotZone { .. } => 1,
        AdvertisedRulesSummary::Heist { .. } => 2,
        AdvertisedRulesSummary::Zombie { .. } => 3,
    }
}
fn mode_candidate(
    games: &[AdvertisedGameType],
    mode: usize,
    selected: Option<usize>,
) -> Option<usize> {
    let current = selected.and_then(|i| games.get(i));
    if let Some(i) = selected.filter(|i| games.get(*i).is_some_and(|g| mode_index(g) == mode)) {
        return Some(i);
    }
    games
        .iter()
        .position(|g| {
            mode_index(g) == mode
                && current.is_some_and(|c| c.players_per_team == g.players_per_team)
        })
        .or_else(|| games.iter().position(|g| mode_index(g) == mode))
}
fn rules_text(game: &AdvertisedGameType) -> String {
    let (objective, ticks) = match game.rules_summary {
        AdvertisedRulesSummary::Wipeout {
            target_score,
            active_limit_ticks,
        } => (
            format!("First to {target_score} eliminations"),
            active_limit_ticks,
        ),
        AdvertisedRulesSummary::HotZone {
            target_progress_ticks,
            active_limit_ticks,
        } => (
            format!(
                "Hold for {} sec",
                crate::timing::simulation_whole_seconds(u64::from(target_progress_ticks))
            ),
            active_limit_ticks,
        ),
        AdvertisedRulesSummary::Heist {
            safe_maximum_health,
            active_limit_ticks,
        } => (
            format!("Enemy safe: {safe_maximum_health} HP"),
            active_limit_ticks,
        ),
        AdvertisedRulesSummary::Zombie {
            pressure_stage_count,
            active_limit_ticks,
        } => (
            format!("{pressure_stage_count} pressure stages"),
            active_limit_ticks,
        ),
    };
    {
        let seconds = crate::timing::simulation_whole_seconds(ticks);
        let duration = if seconds.is_multiple_of(60) {
            format!("{} min", seconds / 60)
        } else {
            format!("{seconds} sec")
        };
        format!("{objective} · {duration}")
    }
}
fn map_text(game: &AdvertisedGameType) -> String {
    let catalog = crate::map::MapContentCatalog::embedded().ok();
    game.map_preset_ids
        .iter()
        .map(|id| {
            catalog
                .as_ref()
                .and_then(|c| c.presets.iter().find(|p| p.id == *id))
                .map_or_else(|| format!("Map {}", id.0), |p| p.display_name.clone())
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "screen snapshot reads assets and authenticated selection without mutating gameplay"
)]
pub(in crate::client::flow) fn spawn_game_type_select(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    mut navigation: ResMut<FlowNavigation>,
    draft: Res<GameTypeSelectionDraft>,
    assets: Option<Res<crate::client::ClientAssetHandles>>,
    modes: Res<crate::modes::ModeRegistry>,
    server: Option<Res<AssetServer>>,
    windows: Query<&Window>,
    scale: Option<Res<UiScale>>,
    roots: Query<(Entity, &RenderedSelection), With<GameTypeSelectRoot>>,
) {
    if *flow.get() != ClientFlow::GameTypeSelect {
        return;
    }
    let Some(membership) = memberships.iter().next() else {
        return;
    };
    let height =
        windows.iter().next().map_or(900.0, Window::height) / scale.as_ref().map_or(1.0, |s| s.0);
    let width = windows.iter().next().map_or(1600.0, Window::width) / scale.map_or(1.0, |s| s.0);
    if roots.iter().any(|(_, r)| {
        r.draft == *draft
            && r.games == membership.game_types
            && (r.width - width).abs() < 0.5
            && (r.height - height).abs() < 0.5
    }) {
        return;
    }
    let entering = roots.is_empty();
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    let selected = draft
        .selected_index
        .and_then(|i| membership.game_types.get(i));
    if entering {
        navigation.selected = selected.map_or(0, mode_index);
    }
    let factor = (width / 1600.0).min(height / 900.0).clamp(0.6, 1.25);
    commands
        .spawn((
            FlowRoot,
            GameTypeSelectRoot,
            GameSelectionColumns(if width < 1000.0 { 2 } else { 4 }),
            RenderedSelection {
                draft: draft.clone(),
                games: membership.game_types.clone(),
                width,
                height,
            },
            DespawnOnExit(ClientFlow::GameTypeSelect),
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(28.0 * factor)),
                row_gap: px(20.0 * factor),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition::default(),
            BackgroundColor(CREAM),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            spawn_background(root, server.as_deref());
            spawn_header(root, assets.as_deref(), factor);
            if draft.unavailable_previous {
                label(
                    root,
                    assets.as_deref(),
                    "Previous game unavailable. Choose another game.",
                    20.0 * factor,
                );
            }
            spawn_cards(
                root,
                &membership.game_types,
                draft.selected_index,
                assets.as_deref(),
                server.as_deref(),
                &modes,
                factor,
                width < 1000.0,
            );
            spawn_variants(
                root,
                &membership.game_types,
                draft.selected_index,
                assets.as_deref(),
                server.as_deref(),
                factor,
            );
        });
}
fn spawn_background(root: &mut ChildSpawnerCommands, server: Option<&AssetServer>) {
    if let Some(server) = server {
        root.spawn((
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
}

fn spawn_header(
    root: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    factor: f32,
) {
    root.spawn(Node {
        width: percent(100),
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|header| {
        label(header, assets, "CHOOSE GAME", 64.0 * factor);
        if let Some(a) = assets {
            header.spawn((
                ImageNode::new(a.dashboard_logo.clone()),
                Node {
                    width: px(200.0 * factor),
                    ..default()
                },
            ));
        }
    });
}

fn label(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    text: &str,
    size: f32,
) {
    parent.spawn((Text::new(text), font(assets, size), TextColor(INK)));
}
struct Choice<'a> {
    index: usize,
    action: FlowUiAction,
    selected: bool,
    card: bool,
    label: &'a str,
}
fn choice(
    parent: &mut ChildSpawnerCommands,
    _server: Option<&AssetServer>,
    node: Node,
    choice: &Choice<'_>,
) -> Entity {
    parent
        .spawn((
            Button,
            AccessibleLabel::new(choice.label),
            if choice.card {
                crate::client::ui::surface::Surface::Card
            } else {
                super::shared::surface_role(&choice.action)
            },
            crate::client::ui::surface::SurfaceState {
                selected: choice.selected,
                ..default()
            },
            FlowButton {
                index: choice.index,
                action: choice.action.clone(),
                error_action: false,
            },
            node,
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
        ))
        .id()
}
#[allow(
    clippy::too_many_arguments,
    reason = "card construction receives one screen snapshot"
)]
fn spawn_cards(
    parent: &mut ChildSpawnerCommands,
    games: &[AdvertisedGameType],
    selected: Option<usize>,
    assets: Option<&crate::client::ClientAssetHandles>,
    server: Option<&AssetServer>,
    modes: &crate::modes::ModeRegistry,
    factor: f32,
    compact: bool,
) {
    let objectives = [
        "Eliminate the enemy team",
        "Hold the zone",
        "Destroy the enemy safe",
        "Survive together",
    ];
    parent
        .spawn(Node {
            width: percent(100),
            flex_wrap: FlexWrap::Wrap,
            column_gap: percent(1),
            row_gap: px(12),
            ..default()
        })
        .with_children(|row| {
            for (mode, objective) in objectives.iter().enumerate() {
                let Some(index) = mode_candidate(games, mode, selected) else {
                    continue;
                };
                let name = modes
                    .descriptor_for_definition(games[index].mode_definition_id)
                    .and_then(|m| m.presentation)
                    .map_or("GAME", |p| p.selection_label)
                    .split(" ·")
                    .next()
                    .unwrap_or("GAME")
                    .to_uppercase();
                let active = selected
                    .and_then(|i| games.get(i))
                    .is_some_and(|g| mode_index(g) == mode);
                let id = choice(
                    row,
                    server,
                    Node {
                        width: percent(if compact { 49.0 } else { 24.0 }),
                        min_height: px(if compact { 300.0 } else { 440.0 } * factor),
                        padding: UiRect::axes(px(22.0 * factor), px(28.0 * factor)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        flex_direction: FlexDirection::Column,
                        row_gap: px(12.0 * factor),
                        ..default()
                    },
                    &Choice {
                        index: mode,
                        action: FlowUiAction::SelectGameTypeDraft(index),
                        selected: active,
                        card: true,
                        label: &name,
                    },
                );
                row.commands().entity(id).with_children(|card| {
                    if let Some(a) = assets {
                        card.spawn((
                            ImageNode::new(if mode == 0 {
                                a.dashboard_badges[0].clone()
                            } else {
                                server.map_or_else(
                                    || a.dashboard_badges[mode].clone(),
                                    |s| {
                                        s.load(format!(
                                            "brawler/ui/shell/game-badge-{}.png",
                                            ["wipeout", "hot-zone", "heist", "zombie"][mode]
                                        ))
                                    },
                                )
                            }),
                            Node {
                                width: px(140.0 * factor),
                                height: px(174.0 * factor),
                                ..default()
                            },
                        ));
                    }
                    label(card, assets, &name, 38.0 * factor);
                    label(card, assets, objective, 19.0 * factor);
                });
            }
        });
}
fn spawn_variants(
    parent: &mut ChildSpawnerCommands,
    games: &[AdvertisedGameType],
    selected: Option<usize>,
    assets: Option<&crate::client::ClientAssetHandles>,
    server: Option<&AssetServer>,
    factor: f32,
) {
    parent
        .spawn(Node {
            width: percent(100),
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            row_gap: px(12),
            ..default()
        })
        .with_children(|footer| {
            footer
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(12),
                    max_width: percent(100),
                    ..default()
                })
                .with_children(|details| {
                    if let Some(game) = selected.and_then(|i| games.get(i)) {
                        label(
                            details,
                            assets,
                            if mode_index(game) == 3 {
                                "PLAYERS · CO-OP"
                            } else {
                                "TEAM SIZE"
                            },
                            23.0 * factor,
                        );
                        details
                            .spawn(Node {
                                flex_wrap: FlexWrap::Wrap,
                                column_gap: px(10),
                                row_gap: px(10),
                                ..default()
                            })
                            .with_children(|sizes| {
                                spawn_size_choices(
                                    sizes, games, selected, assets, server, factor, game,
                                );
                            });
                        label(details, assets, &rules_text(game), 20.0 * factor);
                        label(
                            details,
                            assets,
                            &format!("Map: {}", map_text(game)),
                            20.0 * factor,
                        );
                    } else {
                        label(details, assets, "Choose a mode", 26.0 * factor);
                    }
                });
            let id = choice(
                footer,
                server,
                Node {
                    min_width: px(340.0 * factor),
                    height: px(100.0 * factor),
                    padding: UiRect::all(px(22)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                &Choice {
                    index: GAME_TYPE_CONFIRM_INDEX,
                    action: FlowUiAction::ConfirmGameType,
                    selected: true,
                    card: false,
                    label: "SELECT GAME",
                },
            );
            let mut commands = footer.commands();
            let mut button = commands.entity(id);
            if selected.and_then(|i| games.get(i)).is_none() {
                button.insert(InteractionDisabled);
            }
            button.with_children(|c| label(c, assets, "SELECT GAME  ›", 34.0 * factor));
        });
}
fn spawn_size_choices(
    sizes: &mut ChildSpawnerCommands,
    games: &[AdvertisedGameType],
    selected: Option<usize>,
    assets: Option<&crate::client::ClientAssetHandles>,
    server: Option<&AssetServer>,
    factor: f32,
    game: &AdvertisedGameType,
) {
    let mut variants = games
        .iter()
        .enumerate()
        .filter(|(_, g)| g.mode_definition_id == game.mode_definition_id)
        .collect::<Vec<_>>();
    variants.sort_by_key(|(i, g)| (g.players_per_team, *i));
    for (position, (index, variant)) in variants.into_iter().enumerate() {
        let duplicates = games
            .iter()
            .filter(|g| {
                g.mode_definition_id == game.mode_definition_id
                    && g.players_per_team == variant.players_per_team
            })
            .count()
            > 1;
        let text = if duplicates {
            variant.display_name.clone()
        } else if mode_index(game) == 3 {
            format!("{}", variant.players_per_team)
        } else {
            format!("{0}v{0}", variant.players_per_team)
        };
        let id = choice(
            sizes,
            server,
            Node {
                min_width: px(112.0 * factor),
                min_height: px(64.0 * factor),
                padding: UiRect::all(px(14)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            &Choice {
                index: 100 + position,
                action: FlowUiAction::SelectGameTypeDraft(index),
                selected: selected == Some(index),
                card: false,
                label: &text,
            },
        );
        sizes.commands().entity(id).with_children(|c| {
            label(c, assets, &text, 25.0 * factor);
        });
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::client::flow) fn scroll_game_type_select(
    flow: Res<State<ClientFlow>>,
    mut wheel: MessageReader<MouseWheel>,
    mut roots: Query<(&ComputedNode, &mut ScrollPosition), With<GameTypeSelectRoot>>,
) {
    if *flow.get() != ClientFlow::GameTypeSelect {
        return;
    }
    let delta = normalized_wheel_delta(wheel.read(), 24.0);
    if delta.abs() <= f32::EPSILON {
        return;
    }
    for (node, mut position) in &mut roots {
        position.0.y = clamp_scroll_offset(
            position.0.y - delta,
            node.content_size().y,
            node.size().y,
            node.inverse_scale_factor(),
        );
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "computed UI bounds are available only after Bevy's layout pass"
)]
pub(in crate::client::flow) fn keep_game_type_focus_visible(
    flow: Res<State<ClientFlow>>,
    navigation: Res<FlowNavigation>,
    buttons: Query<(&FlowButton, &ComputedNode, &UiGlobalTransform)>,
    mut roots: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            &mut ScrollPosition,
        ),
        With<GameTypeSelectRoot>,
    >,
    mut prior: Local<Option<(Entity, usize)>>,
) {
    if *flow.get() != ClientFlow::GameTypeSelect {
        *prior = None;
        return;
    }
    let Some((root_entity, _, _, _)) = roots.iter_mut().next() else {
        *prior = None;
        return;
    };
    let focus_key = (root_entity, navigation.selected);
    if prior.as_ref() == Some(&focus_key) {
        return;
    }
    *prior = Some(focus_key);
    let Some((_, focused_node, focused_transform)) = buttons
        .iter()
        .find(|(button, _, _)| button.index == navigation.selected)
    else {
        return;
    };
    if focused_node.is_empty() {
        return;
    }
    let (_, _, focused_center) = focused_transform.to_scale_angle_translation();
    let focused_half_height = focused_node.size().y * 0.5;
    for (_, root_node, root_transform, mut scroll) in &mut roots {
        if root_node.is_empty() {
            continue;
        }
        let (_, _, root_center) = root_transform.to_scale_angle_translation();
        let root_half_height = root_node.size().y * 0.5;
        let visible_top = root_center.y - root_half_height + 8.0;
        let visible_bottom = root_center.y + root_half_height - 8.0;
        let focused_top = focused_center.y - focused_half_height;
        let focused_bottom = focused_center.y + focused_half_height;
        let offset = offset_keeping_interval_visible(
            scroll.0.y,
            visible_top..visible_bottom,
            focused_top..focused_bottom,
            root_node.inverse_scale_factor(),
        );
        scroll.0.y = clamp_scroll_offset(
            offset,
            root_node.content_size().y,
            root_node.size().y,
            root_node.inverse_scale_factor(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(mode: usize, size: u8) -> AdvertisedGameType {
        AdvertisedGameType {
            id: crate::lobby::GameTypeId::new(format!("mode-{mode}-{size}")).unwrap(),
            configuration_revision: 1,
            display_name: format!("Mode {mode}"),
            mode_definition_id: if mode == 0 {
                crate::map::WIPEOUT_MODE_DEFINITION
            } else {
                crate::map::ZOMBIE_MODE_DEFINITION
            },
            map_preset_ids: vec![],
            practice_map_preset_ids: vec![],
            team_count: if mode == 0 { 2 } else { 1 },
            players_per_team: size,
            rules_summary: if mode == 0 {
                AdvertisedRulesSummary::Wipeout {
                    target_score: 10,
                    active_limit_ticks: 10800,
                }
            } else {
                AdvertisedRulesSummary::Zombie {
                    pressure_stage_count: 4,
                    active_limit_ticks: 10800,
                }
            },
        }
    }

    #[test]
    fn mode_change_preserves_size_and_exact_current_variant() {
        let games = vec![game(0, 1), game(0, 2), game(0, 3), game(3, 2), game(3, 3)];
        assert_eq!(mode_candidate(&games, 3, Some(2)), Some(4));
        assert_eq!(mode_candidate(&games, 3, Some(0)), Some(3));
        assert_eq!(mode_candidate(&games, 0, Some(1)), Some(1));
        assert_eq!(mode_candidate(&games, 1, Some(0)), None);
        assert_eq!(mode_candidate(&games, 0, Some(99)), Some(0));
    }
}
