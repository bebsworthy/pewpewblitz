//! Error and confirmation overlay presentation.

use crate::client::{
    ClientQueueModel,
    flow::{
        actions::FlowUiAction,
        model::{ClientFlow, ClientOverlay, FlowError, FlowErrorAction},
        screens::shared::{FlowButton, FlowNavigation, spawn_flow_error_button, spawn_heading},
    },
};
use bevy::{prelude::*, ui::InteractionDisabled};
use std::time::Duration;

const ERROR_BUTTON_BASE: usize = 1_000;

#[derive(Component)]
pub(in crate::client::flow) struct FlowErrorRoot(FlowError);

#[derive(Component)]
pub(in crate::client::flow) struct RateLimitTryAgain;

#[derive(Component)]
pub(in crate::client::flow) struct RateLimitTryAgainLabel;

#[derive(Component)]
pub(in crate::client::flow) struct CancelConfirmationRoot;

#[derive(Component)]
pub(in crate::client::flow) struct LeaveConfirmationRoot;

#[derive(Component)]
pub(in crate::client::flow) struct ChangeServerConfirmationRoot;

fn spawn_rate_limit_try_again_button(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
) {
    parent
        .spawn((
            Button,
            InteractionDisabled,
            RateLimitTryAgain,
            FlowButton {
                index,
                action,
                error_action: true,
            },
            Node {
                width: percent(92),
                min_height: px(44),
                padding: UiRect::axes(px(12), px(8)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(2)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.12)),
            BorderColor::all(Color::NONE),
        ))
        .with_child((
            RateLimitTryAgainLabel,
            Text::new("TRY AGAIN"),
            TextFont::from_font_size(18.0),
        ));
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::client::flow) fn present_flow_error_overlay(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    roots: Query<(Entity, &FlowErrorRoot)>,
    mut navigation: ResMut<FlowNavigation>,
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
    commands
        .spawn((
            FlowErrorRoot(error.clone()),
            DespawnOnExit(error.return_flow),
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
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(500),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(88),
                    max_width: px(720),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(12),
                    padding: UiRect::all(px(24)),
                    border: UiRect::all(px(2)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.055, 0.08, 0.12)),
                BorderColor::all(Color::srgb(0.85, 0.3, 0.25)),
            ))
            .with_children(|panel| {
                spawn_heading(panel, error.kind.title());
                panel.spawn((
                    Text::new(error.message.clone()),
                    TextColor(Color::srgb(1.0, 0.72, 0.65)),
                ));
                for (offset, action) in error.actions.into_iter().flatten().enumerate() {
                    let (ui_action, label) = flow_error_action_button(action);
                    if action == FlowErrorAction::TryAgainQueue {
                        spawn_rate_limit_try_again_button(
                            panel,
                            ERROR_BUTTON_BASE + offset,
                            ui_action,
                        );
                    } else {
                        spawn_flow_error_button(
                            panel,
                            ERROR_BUTTON_BASE + offset,
                            ui_action,
                            label,
                        );
                    }
                }
            });
        });
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy presentation reads runtime-owned time and queue state"
)]
pub(in crate::client::flow) fn update_rate_limit_try_again(
    mut commands: Commands,
    time: Res<Time<Real>>,
    queue: Res<ClientQueueModel>,
    buttons: Query<Entity, With<RateLimitTryAgain>>,
    mut labels: Query<&mut Text, With<RateLimitTryAgainLabel>>,
) {
    let remaining = queue
        .pending()
        .and_then(|pending| pending.rate_limited_until)
        .map_or(Duration::ZERO, |deadline| {
            deadline.saturating_sub(time.elapsed())
        });
    let enabled = remaining.is_zero();
    for entity in &buttons {
        if enabled {
            commands.entity(entity).remove::<InteractionDisabled>();
        } else {
            commands.entity(entity).insert(InteractionDisabled);
        }
    }
    for mut label in &mut labels {
        label.0 = if enabled {
            "TRY AGAIN".to_string()
        } else {
            format!("TRY AGAIN IN {:.1}s", remaining.as_secs_f32())
        };
    }
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
pub(in crate::client::flow) fn present_cancel_confirmation(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    roots: Query<Entity, With<CancelConfirmationRoot>>,
    mut navigation: ResMut<FlowNavigation>,
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
    commands
        .spawn((
            CancelConfirmationRoot,
            DespawnOnExit(ClientFlow::MatchLoading),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(500),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(88),
                    max_width: px(640),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(12),
                    padding: UiRect::all(px(24)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.055, 0.08, 0.12)),
            ))
            .with_children(|panel| {
                spawn_heading(panel, "CANCEL MATCH START?");
                spawn_flow_error_button(panel, 0, FlowUiAction::KeepLoading, "KEEP LOADING");
                spawn_flow_error_button(
                    panel,
                    1,
                    FlowUiAction::ConfirmCancelMatchStart,
                    "CANCEL MATCH START",
                );
            });
        });
}

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn present_leave_confirmation(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    roots: Query<Entity, With<LeaveConfirmationRoot>>,
    mut navigation: ResMut<FlowNavigation>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::LeaveConfirmation) {
        for entity in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    if roots.iter().next().is_some() || *flow.get() != ClientFlow::Match {
        return;
    }
    navigation.selected = 0;
    commands
        .spawn((
            LeaveConfirmationRoot,
            DespawnOnExit(ClientFlow::Match),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(500),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(88),
                    max_width: px(640),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(12),
                    padding: UiRect::all(px(24)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.055, 0.08, 0.12)),
            ))
            .with_children(|panel| {
                spawn_heading(panel, "LEAVE MATCH?");
                spawn_flow_error_button(panel, 0, FlowUiAction::KeepPlaying, "KEEP PLAYING");
                spawn_flow_error_button(panel, 1, FlowUiAction::ConfirmLeaveMatch, "LEAVE MATCH");
            });
        });
}

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn present_change_server_confirmation(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    roots: Query<Entity, With<ChangeServerConfirmationRoot>>,
    mut navigation: ResMut<FlowNavigation>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::ChangeServerConfirmation) {
        for entity in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    if roots.iter().next().is_some() || *flow.get() != ClientFlow::Dashboard {
        return;
    }
    navigation.selected = 0;
    commands
        .spawn((
            ChangeServerConfirmationRoot,
            DespawnOnExit(ClientFlow::Dashboard),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(500),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(88),
                    max_width: px(640),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(12),
                    padding: UiRect::all(px(24)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.055, 0.08, 0.12)),
            ))
            .with_children(|panel| {
                spawn_heading(panel, "CHANGE SERVER?");
                panel.spawn((
                    Text::new("This disconnects from the current lobby."),
                    TextColor(Color::srgb(0.75, 0.84, 0.9)),
                ));
                spawn_flow_error_button(panel, 0, FlowUiAction::KeepServer, "STAY CONNECTED");
                spawn_flow_error_button(
                    panel,
                    1,
                    FlowUiAction::ConfirmChangeServer,
                    "CHANGE SERVER",
                );
            });
        });
}
