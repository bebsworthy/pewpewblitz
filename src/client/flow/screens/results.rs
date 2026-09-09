//! Match-completion cover and results screen ownership.

mod scroll;
mod view;
pub(in crate::client::flow) use scroll::scroll_results;
pub(in crate::client::flow) use view::spawn_results;

use crate::client::{
    ClientMatchResultState,
    flow::{
        model::ClientFlow,
        screens::shared::{flow_root_node, spawn_heading},
    },
};
use bevy::prelude::*;

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
        crate::matchplay::MatchResult::Cleared => "HORDE CLEARED".to_string(),
        crate::matchplay::MatchResult::Overrun => "TEAM OVERRUN".to_string(),
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

pub(in crate::client::flow) fn clear_results(mut result_state: ResMut<ClientMatchResultState>) {
    result_state.context = None;
}
