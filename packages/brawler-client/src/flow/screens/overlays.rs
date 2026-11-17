//! Error and confirmation overlay presentation.

use crate::{
    ClientAssetHandles, ClientQueueModel,
    flow::{
        actions::FlowUiAction,
        model::{ClientFlow, ClientOverlay, FlowError, FlowErrorAction, FlowErrorKind},
        screens::{
            connection_shell::{
                action_node, root_background, root_node, spawn_action, spawn_background,
                spawn_logo, spawn_text,
            },
            shared::FlowNavigation,
        },
    },
    ui::{
        surface::Surface,
        theme::{INK, TEXT, font},
    },
};
use bevy::{prelude::*, ui::InteractionDisabled};
use std::time::Duration;

const ERROR_BUTTON_BASE: usize = 1_000;

#[derive(Component)]
pub(in crate::flow) struct FlowErrorRoot(FlowError);

#[derive(Component)]
pub(in crate::flow) struct RateLimitTryAgain;

#[derive(Component)]
pub(in crate::flow) struct CancelConfirmationRoot;

/// The dimmed full-screen scrim every in-flow overlay sits on.
fn overlay_scrim() -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            display: Display::Flex,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::all(px(24)),
            ..default()
        },
        BackgroundColor(INK.with_alpha(0.72)),
        GlobalZIndex(500),
    )
}

/// The shell panel an overlay's copy and actions sit in; `Surface` owns its chrome.
fn overlay_panel() -> impl Bundle {
    (
        Surface::Panel,
        Node {
            width: percent(88),
            max_width: px(720),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(14),
            padding: UiRect::axes(px(40), px(28)),
            ..default()
        },
        BackgroundColor(Color::NONE),
        BorderColor::all(Color::NONE),
    )
}

/// A column that keeps overlay actions one width, as on the Connection Lost surface.
fn overlay_actions() -> Node {
    Node {
        width: percent(100),
        max_width: px(560),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: px(10),
        margin: UiRect::top(px(6)),
        ..default()
    }
}

fn overlay_action_node() -> Node {
    action_node(percent(100), 56.0)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::flow) fn present_flow_error_overlay(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    roots: Query<(Entity, &FlowErrorRoot)>,
    mut navigation: ResMut<FlowNavigation>,
    assets: Option<Res<ClientAssetHandles>>,
    server: Option<Res<AssetServer>>,
) {
    let ClientOverlay::Error(error) = overlay.as_ref() else {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    };
    if error.return_flow != *flow.get() {
        return;
    }
    let matches_current = roots.iter().any(|(_, rendered)| rendered.0 == *error);
    if matches_current && roots.iter().count() == 1 {
        return;
    }
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    navigation.selected = ERROR_BUTTON_BASE;
    if error.kind == FlowErrorKind::Connection && error.return_flow == ClientFlow::ServerSelect {
        spawn_connection_recovery(&mut commands, error, assets.as_deref(), server.as_deref());
        return;
    }
    commands
        .spawn((
            FlowErrorRoot(error.clone()),
            DespawnOnExit(error.return_flow),
            overlay_scrim(),
        ))
        .with_children(|root| {
            root.spawn(overlay_panel()).with_children(|panel| {
                spawn_text(panel, assets.as_deref(), error.kind.title(), 40.0);
                panel.spawn((
                    Text::new(error.message.clone()),
                    font(assets.as_deref(), 22.0),
                    TextColor(TEXT),
                    TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                    Node {
                        max_width: px(600),
                        ..default()
                    },
                ));
                panel.spawn(overlay_actions()).with_children(|actions| {
                    spawn_flow_error_actions(actions, error, assets.as_deref());
                });
            });
        });
}

fn spawn_flow_error_actions(
    actions: &mut ChildSpawnerCommands,
    error: &FlowError,
    assets: Option<&ClientAssetHandles>,
) {
    for (offset, action) in error.actions.into_iter().flatten().enumerate() {
        let (ui_action, label) = flow_error_action_button(action);
        let role = if offset == 0 && leads_forward(action) {
            Surface::Primary
        } else {
            Surface::Secondary
        };
        let button = spawn_action(
            actions,
            assets,
            ERROR_BUTTON_BASE + offset,
            ui_action,
            label,
            Some(role),
            true,
            overlay_action_node(),
        );
        if action == FlowErrorAction::TryAgainQueue {
            // Enabled by `update_rate_limit_try_again` once the rate-limit window passes.
            actions
                .commands()
                .entity(button)
                .insert((RateLimitTryAgain, InteractionDisabled));
        }
    }
}

fn spawn_connection_recovery(
    commands: &mut Commands,
    error: &FlowError,
    assets: Option<&ClientAssetHandles>,
    server: Option<&AssetServer>,
) {
    commands
        .spawn((
            FlowErrorRoot(error.clone()),
            DespawnOnExit(error.return_flow),
            root_node(),
            root_background(),
            GlobalZIndex(500),
        ))
        .with_children(|root| {
            spawn_background(root, server);
            spawn_logo(root, assets, server, 330.0);
            root.spawn((
                Surface::Panel,
                Node {
                    width: percent(88),
                    max_width: px(820),
                    min_height: px(390),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: px(20),
                    padding: UiRect::axes(px(48), px(38)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
            ))
            .with_children(|panel| {
                spawn_connection_recovery_content(panel, error, assets);
            });
        });
}

fn spawn_connection_recovery_content(
    panel: &mut ChildSpawnerCommands,
    error: &FlowError,
    assets: Option<&ClientAssetHandles>,
) {
    let retryable = error
        .actions
        .contains(&Some(FlowErrorAction::RetryConnection));
    spawn_text(
        panel,
        assets,
        if retryable {
            "CONNECTION LOST"
        } else {
            "CONNECTION ERROR"
        },
        52.0,
    );
    panel.spawn((
        Text::new(error.message.clone()),
        font(assets, 22.0),
        TextColor(TEXT),
        TextLayout::new(Justify::Center, LineBreak::WordBoundary),
        Node {
            max_width: px(640),
            ..default()
        },
    ));
    panel
        .spawn(Node {
            width: percent(100),
            max_width: px(620),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: px(10),
            ..default()
        })
        .with_children(|actions| spawn_connection_recovery_actions(actions, error, assets));
}

fn spawn_connection_recovery_actions(
    actions: &mut ChildSpawnerCommands,
    error: &FlowError,
    assets: Option<&ClientAssetHandles>,
) {
    for (offset, action) in error.actions.into_iter().flatten().enumerate() {
        let (ui_action, default_label) = flow_error_action_button(action);
        let label = if action == FlowErrorAction::Back {
            "CHANGE SERVER"
        } else {
            default_label
        };
        let role = if offset == 0 && action == FlowErrorAction::RetryConnection {
            Surface::Primary
        } else {
            Surface::Secondary
        };
        spawn_action(
            actions,
            assets,
            ERROR_BUTTON_BASE + offset,
            ui_action,
            label,
            Some(role),
            true,
            action_node(percent(100), 64.0),
        );
    }
    spawn_action(
        actions,
        assets,
        ERROR_BUTTON_BASE + error.actions.iter().flatten().count(),
        FlowUiAction::Quit,
        "QUIT",
        Some(Surface::Quiet),
        true,
        action_node(auto(), 54.0),
    );
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy presentation reads runtime-owned time and queue state"
)]
pub(in crate::flow) fn update_rate_limit_try_again(
    mut commands: Commands,
    time: Res<Time<Real>>,
    queue: Res<ClientQueueModel>,
    buttons: Query<(Entity, &Children), With<RateLimitTryAgain>>,
    mut labels: Query<&mut Text>,
) {
    let remaining = queue
        .pending()
        .and_then(|pending| pending.rate_limited_until)
        .map_or(Duration::ZERO, |deadline| {
            deadline.saturating_sub(time.elapsed())
        });
    let enabled = remaining.is_zero();
    let copy = if enabled {
        "TRY AGAIN".to_string()
    } else {
        format!("TRY AGAIN IN {:.1}s", remaining.as_secs_f32())
    };
    for (entity, children) in &buttons {
        if enabled {
            commands.entity(entity).remove::<InteractionDisabled>();
        } else {
            commands.entity(entity).insert(InteractionDisabled);
        }
        let mut labels = labels.iter_many_mut(children);
        while let Some(mut label) = labels.fetch_next() {
            if label.0 != copy {
                label.0.clone_from(&copy);
            }
        }
    }
}

/// Actions that retry or continue the player's path lead the panel; dismissals stay secondary.
fn leads_forward(action: FlowErrorAction) -> bool {
    matches!(
        action,
        FlowErrorAction::RetryConnection
            | FlowErrorAction::RetrySave
            | FlowErrorAction::RetryQueue
            | FlowErrorAction::TryAgainQueue
            | FlowErrorAction::ContinueWithDefaults
    )
}

fn flow_error_action_button(action: FlowErrorAction) -> (FlowUiAction, &'static str) {
    match action {
        FlowErrorAction::RetryConnection => (FlowUiAction::Retry, "RETRY"),
        FlowErrorAction::EditName => (FlowUiAction::EditName, "EDIT NAME"),
        FlowErrorAction::Back => (FlowUiAction::DismissError, "BACK"),
        FlowErrorAction::RetrySave => (FlowUiAction::RetrySave, "RETRY SAVE"),
        FlowErrorAction::ContinueWithoutSaving => (
            FlowUiAction::ContinueWithoutSaving,
            "CONTINUE WITHOUT SAVING",
        ),
        FlowErrorAction::ContinueWithDefaults => {
            (FlowUiAction::DismissError, "CONTINUE WITH DEFAULTS")
        }
        FlowErrorAction::RetryQueue => (FlowUiAction::RetryQueue, "RETRY"),
        FlowErrorAction::TryAgainQueue => (FlowUiAction::TryAgainQueue, "TRY AGAIN"),
        FlowErrorAction::Disconnect => (FlowUiAction::Disconnect, "DISCONNECT"),
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::flow) fn present_cancel_confirmation(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    roots: Query<Entity, With<CancelConfirmationRoot>>,
    mut navigation: ResMut<FlowNavigation>,
    assets: Option<Res<ClientAssetHandles>>,
) {
    let ClientOverlay::Confirmation(_) = overlay.as_ref() else {
        for entity in &roots {
            commands.entity(entity).despawn();
        }
        return;
    };
    if roots.iter().next().is_some() || *flow.get() != ClientFlow::MatchLoading {
        return;
    }
    navigation.selected = 0;
    let assets = assets.as_deref();
    commands
        .spawn((
            CancelConfirmationRoot,
            DespawnOnExit(ClientFlow::MatchLoading),
            overlay_scrim(),
        ))
        .with_children(|root| {
            root.spawn(overlay_panel()).with_children(|panel| {
                spawn_text(panel, assets, "CANCEL MATCH START?", 40.0);
                panel.spawn(overlay_actions()).with_children(|actions| {
                    for (index, action, label, role) in [
                        (
                            0,
                            FlowUiAction::KeepLoading,
                            "KEEP LOADING",
                            Surface::Primary,
                        ),
                        (
                            1,
                            FlowUiAction::ConfirmCancelMatchStart,
                            "CANCEL MATCH START",
                            Surface::Secondary,
                        ),
                    ] {
                        spawn_action(
                            actions,
                            assets,
                            index,
                            action,
                            label,
                            Some(role),
                            true,
                            overlay_action_node(),
                        );
                    }
                });
            });
        });
}
