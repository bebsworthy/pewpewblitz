//! Match-completion cover and results screen ownership.

mod scroll;
mod view;
pub(in crate::flow) use scroll::scroll_results;
pub(in crate::flow) use view::spawn_results;

use crate::{
    ClientMatchResultState,
    flow::{
        model::ClientFlow,
        screens::shared::{flow_root_node, spawn_heading},
    },
};
use bevy::prelude::*;

#[derive(Component)]
pub(in crate::flow) struct MatchCompletionRoot;

#[allow(clippy::needless_pass_by_value)]
pub(in crate::flow) fn present_match_completion(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    matches: Query<&brawler::matchplay::MatchState, With<brawler::matchplay::MatchRoot>>,
    roots: Query<Entity, With<MatchCompletionRoot>>,
) {
    if *flow.get() != ClientFlow::Match || roots.iter().next().is_some() {
        return;
    }
    let Some(result) = matches.iter().find_map(|state| match state.phase {
        brawler::matchplay::MatchPhase::Completed { result, .. } => Some(result),
        _ => None,
    }) else {
        return;
    };
    let result = match result {
        brawler::matchplay::MatchResult::TeamVictory { team } => {
            format!("TEAM {} WINS", team.0 + 1)
        }
        brawler::matchplay::MatchResult::Draw => "DRAW".to_string(),
        brawler::matchplay::MatchResult::Cleared => "HORDE CLEARED".to_string(),
        brawler::matchplay::MatchResult::Overrun => "TEAM OVERRUN".to_string(),
        brawler::matchplay::MatchResult::Forfeit { winner, .. } => {
            format!("TEAM {} WINS BY FORFEIT", winner.0 + 1)
        }
    };
    commands
        .spawn((
            MatchCompletionRoot,
            DespawnOnExit(ClientFlow::Match),
            flow_root_node(),
            crate::ui::theme::shell_background(),
            GlobalZIndex(450),
        ))
        .with_children(|root| {
            spawn_heading(root, "MATCH COMPLETE");
            root.spawn((
                Text::new(result),
                TextFont::from_font_size(28.0),
                TextColor(crate::ui::theme::TEXT),
            ));
            root.spawn((
                Text::new("RETURNING TO LOBBY…"),
                TextFont::from_font_size(18.0),
                TextColor(crate::ui::theme::TEXT.with_alpha(0.75)),
            ));
        });
}

pub(in crate::flow) fn clear_results(mut result_state: ResMut<ClientMatchResultState>) {
    result_state.context = None;
}
