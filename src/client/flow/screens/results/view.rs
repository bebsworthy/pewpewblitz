//! Approved post-match presentation, driven entirely by the retained authority snapshot.
#[path = "objective.rs"]
mod objective;
use crate::{
    client::{
        ClientAssetHandles, ClientLobbyMembership, ClientMatchResultContext,
        ClientMatchResultState, RoutedClientLifecycle, RoutedClientSession,
        RoutedClientSessionKind,
        flow::{
            actions::FlowUiAction,
            model::{ClientFlow, SessionPurpose},
            screens::shared::{FlowButton, FlowNavigation, FlowRoot},
        },
        hud::mode_score_text,
        ui::{
            surface::{Surface, SurfaceState},
            theme::{CREAM, INK, font},
        },
    },
    combat::TeamId,
    matchplay::{MatchResult, WipeoutState},
};
use bevy::{prelude::*, ui::InteractionDisabled, window::PrimaryWindow};
use lightyear::prelude::client::Client;

#[derive(Component, PartialEq)]
pub(in crate::client::flow) struct ResultsView {
    context: ClientMatchResultContext,
    size: Vec2,
    replay: bool,
}
#[derive(bevy::ecs::system::SystemParam)]
pub(in crate::client::flow) struct ResultsAssets<'w, 's> {
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    scale: Option<Res<'w, UiScale>>,
    server: Option<Res<'w, AssetServer>>,
    handles: Option<Res<'w, ClientAssetHandles>>,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "Results render key and schedule-owned Bevy resources"
)]
pub(in crate::client::flow) fn spawn_results(
    mut commands: Commands,
    state: Res<ClientMatchResultState>,
    mut navigation: ResMut<FlowNavigation>,
    purpose: Res<SessionPurpose>,
    routed: Res<RoutedClientLifecycle>,
    memberships: Query<(&ClientLobbyMembership, &RoutedClientSession), With<Client>>,
    roots: Query<(Entity, &ResultsView)>,
    assets: ResultsAssets,
) {
    let Some(context) = state.context.as_ref() else {
        return;
    };
    let replay = context.game_type_id.as_ref().is_some_and(|id| {
        memberships.iter().any(|(member, session)| {
            session.kind == RoutedClientSessionKind::Lobby
                && session.generation == routed.generation
                && member.game_types.iter().any(|game| &game.id == id)
        })
    });
    let size = assets
        .windows
        .iter()
        .next()
        .map_or(Vec2::new(1440., 900.), |w| Vec2::new(w.width(), w.height()))
        / assets.scale.as_deref().map_or(1., |s| s.0);
    let key = ResultsView {
        context: context.clone(),
        size,
        replay,
    };
    if roots.iter().any(|(_, old)| *old == key) {
        return;
    }
    if roots.is_empty() {
        navigation.selected = usize::from(replay);
    }
    for (e, _) in &roots {
        commands.entity(e).despawn();
    }
    let painter = ResultsPainter {
        context,
        assets: assets.handles.as_deref(),
        compact: size.x < 1000.,
        stacked: size.x < 700.,
    };
    painter.draw(&mut commands, key, assets.server.as_deref(), *purpose);
}

struct ResultsPainter<'a> {
    context: &'a ClientMatchResultContext,
    assets: Option<&'a ClientAssetHandles>,
    compact: bool,
    stacked: bool,
}
impl ResultsPainter<'_> {
    fn draw(
        &self,
        commands: &mut Commands,
        key: ResultsView,
        server: Option<&AssetServer>,
        purpose: SessionPurpose,
    ) {
        let replay = key.replay;
        commands
            .spawn((
                key,
                ScrollPosition::default(),
                FlowRoot,
                DespawnOnExit(ClientFlow::Results),
                GlobalZIndex(410),
                BackgroundColor(CREAM),
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(if self.compact { 24. } else { 40. })),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
            ))
            .with_children(|root| {
                if let Some(server) = server {
                    root.spawn((
                        ImageNode {
                            image_mode: NodeImageMode::Stretch,
                            ..ImageNode::new(
                                server.load("brawler/ui/shell/game-selection-background.png"),
                            )
                        },
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0),
                            top: px(0),
                            width: percent(100),
                            height: percent(100),
                            ..default()
                        },
                        ZIndex(-1),
                    ));
                }
                root.spawn(Node {
                    width: percent(100),
                    min_height: percent(100),
                    flex_shrink: 0.,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: px(12),
                    ..default()
                })
                .with_children(|body| {
                    self.header(body);
                    if let Some(scores) = &self.context.wipeout {
                        self.teams(body, scores);
                    } else if let Some(objective) = self.context.objective {
                        self.objective(body, objective.totals);
                    } else if let Some(score) = self.context.final_score {
                        self.text(body, &mode_score_text(score), 32.);
                    }
                    if !replay {
                        self.text(
                            body,
                            "The previous game is not available on this server.",
                            20.,
                        );
                    }
                    self.actions(body, replay, purpose);
                });
            });
    }

    fn actions(&self, body: &mut ChildSpawnerCommands, replay: bool, purpose: SessionPurpose) {
        body.spawn(Node {
            width: percent(100),
            max_width: px(840),
            margin: UiRect::top(px(24)),
            flex_shrink: 0.,
            column_gap: px(24),
            row_gap: px(12),
            flex_direction: if self.stacked {
                FlexDirection::Column
            } else {
                FlexDirection::Row
            },
            ..default()
        })
        .with_children(|actions| {
            self.action(
                actions,
                0,
                FlowUiAction::ReturnToDashboard,
                "DASHBOARD",
                Surface::Action,
                false,
            );
            self.action(
                actions,
                1,
                FlowUiAction::QueueAgain,
                if !replay {
                    "REPLAY UNAVAILABLE"
                } else if purpose == SessionPurpose::Practice {
                    "PRACTICE AGAIN"
                } else {
                    "PLAY AGAIN"
                },
                Surface::Primary,
                !replay,
            );
        });
    }

    fn text(&self, p: &mut ChildSpawnerCommands, text: &str, size: f32) {
        p.spawn((
            Text::new(text),
            font(self.assets, size),
            TextColor(INK),
            TextLayout::justify(Justify::Center),
            Node {
                max_width: percent(100),
                flex_shrink: 0.,
                ..default()
            },
        ));
    }
    fn header(&self, p: &mut ChildSpawnerCommands) {
        trophy(p, if self.compact { 0.6 } else { 0.85 });
        self.text(
            p,
            &outcome(self.context.result, self.context.local_team),
            if self.compact { 44. } else { 72. },
        );
        let title = self.context.wipeout.as_ref().map_or_else(
            || {
                self.context.objective.map_or_else(
                    || self.context.game_name.clone().unwrap_or_default(),
                    |o| match o.totals {
                        crate::matchplay::ObjectiveTotals::HotZone { .. } => "HOT ZONE".to_owned(),
                        crate::matchplay::ObjectiveTotals::Heist { .. } => "HEIST".to_owned(),
                    },
                )
            },
            |scores| {
                let counts = [0, 1].map(|t| {
                    scores
                        .players
                        .iter()
                        .flatten()
                        .filter(|p| p.team.0 == t)
                        .count()
                });
                format!("WIPEOUT · {}v{}", counts[0], counts[1])
            },
        );
        if !title.is_empty() {
            self.text(p, &title, if self.compact { 24. } else { 30. });
        }
    }
    fn teams(&self, p: &mut ChildSpawnerCommands, scores: &WipeoutState) {
        p.spawn(Node {
            width: percent(100),
            max_width: px(1360),
            flex_shrink: 0.,
            column_gap: px(28),
            row_gap: px(22),
            flex_direction: if self.stacked {
                FlexDirection::Column
            } else {
                FlexDirection::Row
            },
            margin: UiRect::top(px(8)),
            ..default()
        })
        .with_children(|teams| {
            let first = self.context.local_team.unwrap_or(TeamId(0));
            for team in [first, TeamId(1 - first.0.min(1))] {
                self.team(teams, scores, team);
            }
        });
    }
    fn team(&self, p: &mut ChildSpawnerCommands, scores: &WipeoutState, team: TeamId) {
        p.spawn(Node {
            width: if self.stacked { percent(100) } else { px(0) },
            min_width: px(0),
            flex_grow: 1.,
            flex_direction: FlexDirection::Column,
            row_gap: px(8),
            ..default()
        })
        .with_children(|column| {
            let ours = self.context.local_team == Some(team);
            column
                .spawn((
                    Surface::Action,
                    SurfaceState {
                        selected: ours,
                        ..default()
                    },
                    Node {
                        width: percent(100),
                        min_height: px(if self.compact { 78. } else { 112. }),
                        padding: UiRect::axes(px(28), px(12)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        ..default()
                    },
                ))
                .with_children(|banner| {
                    self.text(
                        banner,
                        if ours {
                            "YOUR TEAM"
                        } else if self.context.local_team.is_some() {
                            "OPPONENTS"
                        } else if team.0 == 0 {
                            "TEAM 1"
                        } else {
                            "TEAM 2"
                        },
                        if self.compact { 22. } else { 32. },
                    );
                    self.text(
                        banner,
                        &scores.team_scores[usize::from(team.0)].to_string(),
                        if self.compact { 48. } else { 76. },
                    );
                });
            self.row(column, "", "KILLS", "DEATHS", false, true);
            let mut players: Vec<_> = scores
                .players
                .iter()
                .flatten()
                .filter(|p| p.team == team)
                .collect();
            players.sort_by_key(|p| (std::cmp::Reverse(p.kills), p.deaths, p.player_id.0));
            for player in players {
                self.row(
                    column,
                    &player.display_name,
                    &player.kills.to_string(),
                    &player.deaths.to_string(),
                    Some(player.player_id) == self.context.local_player,
                    false,
                );
            }
        });
    }
    fn row(
        &self,
        p: &mut ChildSpawnerCommands,
        name: &str,
        kills: &str,
        deaths: &str,
        selected: bool,
        heading: bool,
    ) {
        let mut row = p.spawn(Node {
            width: percent(100),
            min_height: px(if heading {
                28.
            } else if self.compact {
                50.
            } else {
                62.
            }),
            padding: UiRect::axes(px(if self.compact { 18. } else { 28. }), px(9)),
            align_items: AlignItems::Center,
            ..default()
        });
        if !heading {
            row.insert((
                Surface::Panel,
                SurfaceState {
                    selected,
                    ..default()
                },
            ));
        }
        row.with_children(|cells| {
            for (value, width) in [(name, 54.), (kills, 23.), (deaths, 23.)] {
                cells
                    .spawn(Node {
                        width: percent(width),
                        min_width: px(0),
                        justify_content: if width > 50. {
                            JustifyContent::FlexStart
                        } else {
                            JustifyContent::Center
                        },
                        padding: UiRect::horizontal(px(3)),
                        ..default()
                    })
                    .with_children(|cell| {
                        self.text(
                            cell,
                            value,
                            if heading {
                                if self.compact { 15. } else { 18. }
                            } else if self.compact {
                                21.
                            } else {
                                28.
                            },
                        );
                    });
            }
        });
    }
    fn action(
        &self,
        p: &mut ChildSpawnerCommands,
        index: usize,
        action: FlowUiAction,
        title: &str,
        role: Surface,
        disabled: bool,
    ) {
        let mut e = p.spawn((
            Button,
            super::scroll::ResultsAction,
            FlowButton {
                index,
                action,
                error_action: false,
            },
            role,
            SurfaceState::default(),
            Node {
                width: if self.stacked { percent(100) } else { px(0) },
                flex_grow: 1.,
                min_height: px(76),
                padding: UiRect::axes(px(30), px(14)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
        ));
        if disabled {
            e.insert(InteractionDisabled);
        }
        e.with_children(|button| self.text(button, title, if self.compact { 23. } else { 30. }));
    }
}
fn outcome(result: MatchResult, local: Option<TeamId>) -> String {
    match result {
        MatchResult::TeamVictory { team } | MatchResult::Forfeit { winner: team, .. } => {
            let label = if local == Some(team) {
                "VICTORY".to_owned()
            } else if local.is_some() {
                "DEFEAT".to_owned()
            } else {
                format!("TEAM {} WINS", team.0 + 1)
            };
            if matches!(result, MatchResult::Forfeit { .. }) {
                format!("{label} BY FORFEIT")
            } else {
                label
            }
        }
        MatchResult::Draw => "DRAW".to_owned(),
        MatchResult::Cleared => "HORDE CLEARED".to_owned(),
        MatchResult::Overrun => "TEAM OVERRUN".to_owned(),
    }
}
fn trophy(p: &mut ChildSpawnerCommands, scale: f32) {
    p.spawn(Node {
        width: px(88. * scale),
        height: px(80. * scale),
        flex_shrink: 0.,
        ..default()
    })
    .with_children(|cup| {
        for (x, y, w, h, r, border) in [
            (8., 12., 72., 42., 20., true),
            (22., 0., 44., 54., 22., false),
            (39., 45., 10., 23., 0., false),
            (26., 66., 36., 7., 3., false),
        ] {
            cup.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(x * scale),
                    top: px(y * scale),
                    width: px(w * scale),
                    height: px(h * scale),
                    border: UiRect::all(px(if border { 6. * scale } else { 0. })),
                    border_radius: BorderRadius::bottom(px(r * scale)),
                    ..default()
                },
                BackgroundColor(if border { Color::NONE } else { INK }),
                BorderColor::all(INK),
            ));
        }
    });
}
