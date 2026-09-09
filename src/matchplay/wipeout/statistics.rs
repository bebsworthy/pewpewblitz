//! Bounded scoring snapshot: player attribution and team totals travel together.
use super::WipeoutState;
use crate::{
    combat::TeamId,
    protocol::{NetworkEntityId, PlayerId},
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WipeoutPlayerStats {
    pub player_id: PlayerId,
    pub network_id: NetworkEntityId,
    pub team: TeamId,
    pub display_name: String,
    pub kills: u16,
    pub deaths: u16,
}

impl WipeoutState {
    #[cfg(feature = "server")]
    pub(super) fn remember_player(&mut self, player: WipeoutPlayerStats) {
        if self
            .players
            .iter()
            .flatten()
            .any(|p| p.player_id == player.player_id)
        {
            return;
        }
        if let Some(slot) = self.players.iter_mut().find(|p| p.is_none()) {
            *slot = Some(player);
        }
    }

    #[cfg(feature = "server")]
    pub(super) fn record_defeat(
        &mut self,
        target: NetworkEntityId,
        source: Option<NetworkEntityId>,
        credited_team: Option<TeamId>,
    ) {
        for player in self.players.iter_mut().flatten() {
            if player.network_id == target {
                player.deaths = player.deaths.saturating_add(1);
            }
            if Some(player.network_id) == source && Some(player.team) == credited_team {
                player.kills = player.kills.saturating_add(1);
            }
        }
    }
}

#[cfg(feature = "server")]
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy roster projection queries"
)]
pub(super) fn remember_roster(
    mut roots: bevy::prelude::Query<(&crate::matchplay::MatchState, &mut WipeoutState)>,
    fighters: bevy::prelude::Query<
        (
            &PlayerId,
            &NetworkEntityId,
            &TeamId,
            &crate::matchplay::MatchParticipant,
            Option<&crate::matchplay::FighterDisplayName>,
        ),
        bevy::prelude::With<crate::protocol::Fighter>,
    >,
) {
    for (state, mut scores) in &mut roots {
        if scores.match_id != Some(state.match_id) {
            scores.match_id = Some(state.match_id);
            scores.players = Default::default();
            scores.completed_result = None;
        }
        for (player_id, network_id, team, participant, name) in &fighters {
            if participant.match_id != state.match_id
                || scores
                    .players
                    .iter()
                    .flatten()
                    .any(|p| p.player_id == *player_id)
            {
                continue;
            }
            scores.remember_player(WipeoutPlayerStats {
                player_id: *player_id,
                network_id: *network_id,
                team: *team,
                display_name: name
                    .map_or_else(|| format!("Player {}", player_id.0), |name| name.0.clone()),
                kills: 0,
                deaths: 0,
            });
        }
    }
}

#[cfg(feature = "server")]
pub(super) fn publish_completion(
    mut roots: bevy::prelude::Query<(&crate::matchplay::MatchState, &mut WipeoutState)>,
) {
    for (state, mut scores) in &mut roots {
        if let crate::matchplay::MatchPhase::Completed { result, .. } = state.phase
            && scores.match_id == Some(state.match_id)
            && scores.completed_result != Some(result)
        {
            scores.completed_result = Some(result);
        }
    }
}
