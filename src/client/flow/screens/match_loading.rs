//! Approved loading composition; reservation state remains the sole source of progress.
use super::{
    dashboard::DashboardPreviewHost,
    shared::{FlowButton, FlowNavigation, FlowRoot},
};
use crate::client::ui::theme::{CREAM, INK, font};
use crate::client::{
    ClientMatchLoadingModel,
    flow::{ClientFlow, actions::FlowUiAction},
};
use bevy::{prelude::*, ui::InteractionDisabled};

#[derive(Component)]
pub(in crate::client::flow) struct MatchLoadingStatusLabel;
#[derive(Component)]
pub(in crate::client::flow) struct MatchLoadingRoot {
    width: f32,
    height: f32,
}
#[derive(Component)]
pub(in crate::client::flow) struct LoadingActivity(u8);
#[derive(Component)]
pub(in crate::client::flow) struct LoadingCancel;

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "loading snapshot reads viewport, assets and immutable reservation"
)]
pub(in crate::client::flow) fn spawn_match_loading(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    loading: Res<ClientMatchLoadingModel>,
    mut navigation: ResMut<FlowNavigation>,
    assets: Option<Res<crate::client::ClientAssetHandles>>,
    server: Option<Res<AssetServer>>,
    modes: Res<crate::modes::ModeRegistry>,
    windows: Query<&Window>,
    scale: Option<Res<UiScale>>,
    roots: Query<(Entity, &MatchLoadingRoot)>,
) {
    if *flow.get() != ClientFlow::MatchLoading {
        return;
    }
    let Some(active) = loading.active() else {
        return;
    };
    let scale = scale.map_or(1.0, |s| s.0);
    let width = windows.iter().next().map_or(1600.0, Window::width) / scale;
    let height = windows.iter().next().map_or(900.0, Window::height) / scale;
    if roots
        .iter()
        .any(|(_, r)| (r.width - width).abs() < 0.5 && (r.height - height).abs() < 0.5)
    {
        return;
    }
    if roots.is_empty() {
        navigation.selected = 0;
    }
    for (e, _) in &roots {
        commands.entity(e).despawn();
    }
    let catalog = crate::map::MapContentCatalog::embedded().ok();
    let mode = catalog
        .as_ref()
        .and_then(|c| c.preset(active.map_preset_id))
        .and_then(|p| modes.descriptor_for_definition(p.recipe.mode_definition_id));
    let title = mode
        .and_then(|m| m.presentation)
        .map_or("MATCH", |p| p.selection_label)
        .split(" ·")
        .next()
        .unwrap_or("MATCH")
        .to_uppercase();
    let badge = mode.map_or("wipeout", |m| m.key);
    let factor = (width / 1600.0).min(height / 900.0).clamp(0.5, 1.25);
    let view = LoadingView {
        assets: assets.as_deref(),
        server: server.as_deref(),
        factor,
    };
    commands
        .spawn((
            FlowRoot,
            MatchLoadingRoot { width, height },
            DespawnOnExit(ClientFlow::MatchLoading),
            GlobalZIndex(410),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            BackgroundColor(CREAM),
        ))
        .with_children(|root| {
            view.background(root);
            root.spawn((
                DashboardPreviewHost,
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(2),
                    top: percent(18),
                    width: percent(56),
                    height: percent(78),
                    ..default()
                },
            ));
            if let Some(a) = view.assets {
                root.spawn((
                    ImageNode::new(a.dashboard_logo.clone()),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(32.0 * factor),
                        top: px(32.0 * factor),
                        width: px(220.0 * factor),
                        ..default()
                    },
                ));
            }
            root.spawn(Node {
                position_type: PositionType::Absolute,
                left: percent(60),
                top: percent(3),
                width: percent(37),
                height: percent(93),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|copy| {
                view.mode(copy, &title, badge, active);
                view.status(copy, active, loading.phase());
                view.cancel(copy);
            });
        });
}
struct LoadingView<'a> {
    assets: Option<&'a crate::client::ClientAssetHandles>,
    server: Option<&'a AssetServer>,
    factor: f32,
}
impl LoadingView<'_> {
    fn label(
        &self,
        parent: &mut ChildSpawnerCommands,
        text: &str,
        size: f32,
        color: Color,
    ) -> Entity {
        parent
            .spawn((
                Text::new(text),
                font(self.assets, size * self.factor),
                TextColor(color),
                TextLayout::new(Justify::Center, LineBreak::WordBoundary),
            ))
            .id()
    }
    fn background(&self, root: &mut ChildSpawnerCommands) {
        if let Some(s) = self.server {
            root.spawn((
                ImageNode {
                    image_mode: NodeImageMode::Stretch,
                    ..ImageNode::new(s.load("brawler/ui/shell/game-selection-background.png"))
                },
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                ZIndex(-1),
            ));
        }
    }
    fn mode(
        &self,
        root: &mut ChildSpawnerCommands,
        title: &str,
        badge: &str,
        active: &crate::lobby::ReservationStarted,
    ) {
        root.spawn(Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(10.0 * self.factor),
            ..default()
        })
        .with_children(|group| {
            if let Some(s) = self.server {
                let path = if badge == "wipeout" {
                    "brawler/ui/shell/wipeout.png".to_owned()
                } else {
                    format!("brawler/ui/shell/game-badge-{badge}.png")
                };
                group.spawn((
                    ImageNode::new(s.load(path)),
                    Node {
                        width: px(130.0 * self.factor),
                        height: px(160.0 * self.factor),
                        ..default()
                    },
                ));
            }
            self.label(group, title, 78.0, INK);
            let topology = if active.team_count == 1 {
                format!("{} PLAYERS", active.players_per_team)
            } else {
                format!("{}v{}", active.players_per_team, active.players_per_team)
            };
            self.label(group, &topology, 38.0, INK);
            group.spawn((
                Node {
                    width: percent(96),
                    height: px(3),
                    margin: UiRect::top(px(18.0 * self.factor)),
                    ..default()
                },
                BackgroundColor(INK),
            ));
        });
    }
    fn status(
        &self,
        root: &mut ChildSpawnerCommands,
        active: &crate::lobby::ReservationStarted,
        phase: Option<crate::lobby::MatchLoadingPhase>,
    ) {
        root.spawn(Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(20.0 * self.factor),
            ..default()
        })
        .with_children(|group| {
            self.label(group, "MATCH LOADING", 48.0, Color::srgb(0.9, 0.32, 0.02));
            let id = self.label(group, &match_loading_text(active, phase), 30.0, INK);
            group.commands().entity(id).insert(MatchLoadingStatusLabel);
            group
                .spawn(Node {
                    column_gap: px(10.0 * self.factor),
                    ..default()
                })
                .with_children(|dots| {
                    for index in 0..3 {
                        dots.spawn((
                            LoadingActivity(index),
                            Node {
                                width: px(9.0 * self.factor),
                                height: px(9.0 * self.factor),
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BackgroundColor(INK),
                        ));
                    }
                });
        });
    }
    fn cancel(&self, root: &mut ChildSpawnerCommands) {
        root.spawn((
            Button,
            LoadingCancel,
            AccessibleLabel::new("Cancel match start"),
            FlowButton {
                index: 0,
                action: FlowUiAction::RequestCancelMatchStart,
                error_action: false,
            },
            Node {
                width: percent(100),
                height: px(92.0 * self.factor),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(px(12.0 * self.factor)),
                ..default()
            },
        ))
        .with_children(|c| {
            self.label(c, "CANCEL MATCH START", 30.0, INK);
        });
    }
}

pub(in crate::client::flow) fn match_loading_text(
    _active: &crate::lobby::ReservationStarted,
    phase: Option<crate::lobby::MatchLoadingPhase>,
) -> String {
    match phase.unwrap_or(crate::lobby::MatchLoadingPhase::Reserving) {
        crate::lobby::MatchLoadingPhase::Reserving => "Reserving roster",
        crate::lobby::MatchLoadingPhase::StartingServer => "Starting server",
        crate::lobby::MatchLoadingPhase::Connecting => "Connecting",
        crate::lobby::MatchLoadingPhase::Synchronizing => "Synchronizing map",
        crate::lobby::MatchLoadingPhase::WaitingForPlayers => "Waiting for players",
        crate::lobby::MatchLoadingPhase::Cancelling => "Cancelling",
        crate::lobby::MatchLoadingPhase::ReturningToQueue => "Returning to queue",
    }
    .to_owned()
}
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy loading presentation resources"
)]
pub(in crate::client::flow) fn update_match_loading(
    mut commands: Commands,
    loading: Res<ClientMatchLoadingModel>,
    time: Res<Time>,
    settings: Option<Res<crate::client::ClientShellSettings>>,
    mut labels: Query<&mut Text, With<MatchLoadingStatusLabel>>,
    mut dots: Query<(&LoadingActivity, &mut BackgroundColor)>,
    buttons: Query<Entity, With<LoadingCancel>>,
) {
    let Some(active) = loading.active() else {
        return;
    };
    for mut label in &mut labels {
        label.0 = match_loading_text(active, loading.phase());
    }
    let reduced = settings.is_some_and(|s| s.reduced_motion);
    for (index, mut color) in &mut dots {
        let alpha = if reduced {
            1.0
        } else {
            0.35 + 0.65 * ((time.elapsed_secs() * 5.0 - f32::from(index.0) * 1.2).sin() * 0.5 + 0.5)
        };
        color.0 = INK.with_alpha(alpha);
    }
    let disabled = matches!(
        loading.phase(),
        Some(
            crate::lobby::MatchLoadingPhase::Cancelling
                | crate::lobby::MatchLoadingPhase::ReturningToQueue
        )
    );
    for e in &buttons {
        if disabled {
            commands.entity(e).insert(InteractionDisabled);
        } else {
            commands.entity(e).remove::<InteractionDisabled>();
        }
    }
}
