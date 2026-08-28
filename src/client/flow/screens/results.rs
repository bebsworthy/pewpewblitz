//! Match-completion cover and results screen ownership.

use crate::client::{
    ClientLobbyMembership, ClientMatchResultState, RoutedClientLifecycle, RoutedClientSession,
    RoutedClientSessionKind,
    flow::{
        actions::FlowUiAction,
        model::{ClientFlow, SessionPurpose},
        screens::shared::{
            FlowNavigation, FlowRoot, flow_root_node, spawn_flow_button,
            spawn_flow_button_disabled, spawn_heading,
        },
    },
    hud::mode_score_text,
};
use bevy::prelude::*;
use lightyear::prelude::client::Client;

#[derive(Component)]
pub(in crate::client::flow) struct MatchCompletionRoot;

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn present_match_completion(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    matches: Query<&crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>,
    roots: Query<Entity, With<MatchCompletionRoot>>,
) {
    if *flow.get() != ClientFlow::Match || roots.iter().next().is_some() {
        return;
    }
    let Some(result) = matches.iter().find_map(|state| match state.phase {
        crate::matchplay::MatchPhase::Completed { result, .. } => Some(result),
        _ => None,
    }) else {
        return;
    };
    let result = match result {
        crate::matchplay::MatchResult::TeamVictory { team } => {
            format!("TEAM {} WINS", team.0 + 1)
        }
        crate::matchplay::MatchResult::Draw => "DRAW".to_string(),
        crate::matchplay::MatchResult::Forfeit { winner, .. } => {
            format!("TEAM {} WINS BY FORFEIT", winner.0 + 1)
        }
    };
    commands
        .spawn((
            MatchCompletionRoot,
            DespawnOnExit(ClientFlow::Match),
            flow_root_node(),
            BackgroundColor(Color::srgba(0.025, 0.04, 0.07, 0.96)),
            GlobalZIndex(450),
        ))
        .with_children(|root| {
            spawn_heading(root, "MATCH COMPLETE");
            root.spawn((
                Text::new(result),
                TextFont::from_font_size(28.0),
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
            ));
            root.spawn((
                Text::new("RETURNING TO LOBBY…"),
                TextFont::from_font_size(18.0),
                TextColor(Color::srgb(0.58, 0.66, 0.74)),
            ));
        });
}

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn spawn_results(
    mut commands: Commands,
    result_state: Res<ClientMatchResultState>,
    mut navigation: ResMut<FlowNavigation>,
    purpose: Res<SessionPurpose>,
    routed: Res<RoutedClientLifecycle>,
    memberships: Query<(&ClientLobbyMembership, &RoutedClientSession), With<Client>>,
) {
    let Some(context) = result_state.context.as_ref() else {
        return;
    };
    let replay_available = context.game_type_id.as_ref().is_some_and(|game_type_id| {
        memberships.iter().any(|(membership, session)| {
            session.kind == RoutedClientSessionKind::Lobby
                && session.generation == routed.generation
                && membership
                    .game_types
                    .iter()
                    .any(|game| &game.id == game_type_id)
        })
    });
    navigation.selected = 0;
    let outcome = match context.result {
        crate::matchplay::MatchResult::TeamVictory { team } => {
            if context.local_team == Some(team) {
                "VICTORY".to_string()
            } else if context.local_team.is_some() {
                "DEFEAT".to_string()
            } else {
                format!("TEAM {} WINS", team.0 + 1)
            }
        }
        crate::matchplay::MatchResult::Draw => "DRAW".to_string(),
        crate::matchplay::MatchResult::Forfeit { winner, .. } => {
            if context.local_team == Some(winner) {
                "VICTORY BY FORFEIT".to_string()
            } else if context.local_team.is_some() {
                "DEFEAT BY FORFEIT".to_string()
            } else {
                format!("TEAM {} WINS BY FORFEIT", winner.0 + 1)
            }
        }
    };
    commands
        .spawn((
            FlowRoot,
            DespawnOnExit(ClientFlow::Results),
            flow_root_node(),
            BackgroundColor(Color::srgb(0.025, 0.04, 0.07)),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            spawn_heading(root, "RESULTS");
            root.spawn((
                Text::new(outcome),
                TextFont::from_font_size(30.0),
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
            ));
            if let Some(name) = context.game_name.as_deref() {
                root.spawn((
                    Text::new(name.to_string()),
                    TextColor(Color::srgb(0.68, 0.78, 0.86)),
                ));
            }
            if let Some(team) = context.local_team {
                root.spawn((
                    Text::new(format!("YOU — T{}", team.0 + 1)),
                    TextColor(Color::srgb(0.85, 0.9, 0.96)),
                ));
            }
            if let Some(score) = context.final_score {
                root.spawn((
                    Text::new(mode_score_text(score)),
                    TextFont::from_font_size(22.0),
                    TextColor(Color::WHITE),
                    TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                ));
            }
            if !replay_available {
                root.spawn((
                    Text::new("The previous game is not available on this server."),
                    TextColor(Color::srgb(1.0, 0.72, 0.28)),
                ));
            }
            spawn_flow_button_disabled(
                root,
                0,
                FlowUiAction::QueueAgain,
                if !replay_available {
                    "REPLAY UNAVAILABLE"
                } else if *purpose == SessionPurpose::Practice {
                    "PRACTICE AGAIN"
                } else {
                    "PLAY AGAIN"
                },
                !replay_available,
            );
            spawn_flow_button(root, 1, FlowUiAction::ReturnToDashboard, "DASHBOARD");
        });
}

pub(in crate::client::flow) fn clear_results(mut result_state: ResMut<ClientMatchResultState>) {
    result_state.context = None;
}
