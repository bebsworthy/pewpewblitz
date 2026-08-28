//! Headless queue, Practice, product-match, and requeue automation.

use bevy::prelude::*;
use std::time::Duration;

use crate::client::{
    Client, ClientLobbyMembership, ClientPracticeModel, ClientQueueModel, RoutedClientSession,
};

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum HeadlessQueueSmokeStage {
    #[default]
    AwaitingInitialSnapshot,
    Joining,
    AwaitingJoinedSnapshot,
    Cancelling,
    AwaitingCancelledSnapshot,
    Complete,
}

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum HeadlessRequeueSmokeStage {
    #[default]
    AwaitingFreshLobby,
    AwaitingJoined,
    Complete,
}

#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
pub(super) fn drive_headless_requeue_smoke(
    time: Res<Time<Real>>,
    config: Res<crate::client::ClientNetworkConfig>,
    lobbies: Query<(&ClientLobbyMembership, &RoutedClientSession), With<Client>>,
    mut model: ResMut<ClientQueueModel>,
    mut stage: ResMut<HeadlessRequeueSmokeStage>,
    mut exit: MessageWriter<AppExit>,
) {
    if !config.product_requeue_smoke || *stage == HeadlessRequeueSmokeStage::Complete {
        return;
    }
    if *stage == HeadlessRequeueSmokeStage::AwaitingJoined && model.membership().is_some() {
        info!("brawler product requeue smoke accepted a fresh queue Join");
        *stage = HeadlessRequeueSmokeStage::Complete;
        exit.write(AppExit::Success);
        return;
    }
    let Some((lobby, session)) = lobbies.iter().find(|(_, session)| {
        session.kind == crate::client::RoutedClientSessionKind::Lobby && session.generation >= 3
    }) else {
        return;
    };
    let Some(game_type_id) = model.last_accepted_game_type_id().cloned() else {
        return;
    };
    if model.start_requeue_join(session.generation, lobby, &game_type_id, time.elapsed()) {
        *stage = HeadlessRequeueSmokeStage::AwaitingJoined;
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(super) fn drive_headless_queue_smoke(
    time: Res<Time<Real>>,
    config: Res<crate::client::ClientNetworkConfig>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    mut model: ResMut<ClientQueueModel>,
    mut practice: ResMut<ClientPracticeModel>,
    mut stage: ResMut<HeadlessQueueSmokeStage>,
    mut exit: MessageWriter<AppExit>,
) {
    if (!config.product_queue_smoke && !config.product_match_smoke)
        || *stage == HeadlessQueueSmokeStage::Complete
    {
        return;
    }
    if model.protocol_failure() {
        error!("brawler product queue smoke observed incompatible queue state");
        exit.write(AppExit::error());
        *stage = HeadlessQueueSmokeStage::Complete;
        return;
    }
    if let Some(outcome) = model.take_outcome()
        && let crate::lobby::QueueDecision::Rejected(reason) = outcome.decision
    {
        error!(?reason, "brawler product queue smoke command was rejected");
        exit.write(AppExit::error());
        *stage = HeadlessQueueSmokeStage::Complete;
        return;
    }
    if let Some(reason) = practice.take_rejection() {
        error!(
            ?reason,
            "brawler product Practice smoke request was rejected"
        );
        exit.write(AppExit::error());
        *stage = HeadlessQueueSmokeStage::Complete;
        return;
    }
    let Some(lobby) = memberships.iter().next() else {
        return;
    };
    match *stage {
        HeadlessQueueSmokeStage::AwaitingInitialSnapshot => {
            if let Some(next) = start_headless_product_request(
                &config,
                lobby,
                &mut model,
                &mut practice,
                time.elapsed(),
            ) {
                *stage = next;
            }
        }
        HeadlessQueueSmokeStage::Joining => {
            if model.membership().is_some() {
                debug!("product match automation received queue membership");
                *stage = HeadlessQueueSmokeStage::AwaitingJoinedSnapshot;
            }
        }
        HeadlessQueueSmokeStage::AwaitingJoinedSnapshot => {
            if config.product_match_smoke {
                return;
            }
            if model.required_snapshot_is_fresh() && model.start_cancel(time.elapsed()) {
                *stage = HeadlessQueueSmokeStage::Cancelling;
            }
        }
        HeadlessQueueSmokeStage::Cancelling => {
            if model.membership().is_none() && model.pending().is_none() {
                *stage = HeadlessQueueSmokeStage::AwaitingCancelledSnapshot;
            }
        }
        HeadlessQueueSmokeStage::AwaitingCancelledSnapshot => {
            if model.required_snapshot_is_fresh() {
                let marker = format!(
                    "brawler-client queue-evidence admissions=1 cancellations=1 freshness_aged={} freshness_restored={}\n",
                    model.freshness_aged, model.freshness_restored
                );
                let _ = std::io::Write::write_all(&mut std::io::stderr().lock(), marker.as_bytes());
                exit.write(AppExit::Success);
                *stage = HeadlessQueueSmokeStage::Complete;
            }
        }
        HeadlessQueueSmokeStage::Complete => {}
    }
}

fn start_headless_product_request(
    config: &crate::client::ClientNetworkConfig,
    lobby: &ClientLobbyMembership,
    queue: &mut ClientQueueModel,
    practice: &mut ClientPracticeModel,
    now: Duration,
) -> Option<HeadlessQueueSmokeStage> {
    let game = automation_game_type(
        &lobby.game_types,
        config.product_match_players_per_team,
        config.product_match_game_type.as_ref(),
    )?;
    queue.snapshot()?;
    let selection = crate::client::flow::SelectedGameType {
        catalog_revision: Some(lobby.catalog_revision),
        game_type_id: Some(game.id.clone()),
        configuration_revision: Some(game.configuration_revision),
    };
    let brawler = lobby.profile.selected_brawler_id.and_then(|id| {
        lobby
            .profile
            .brawlers
            .iter()
            .find(|brawler| brawler.id == id)
    })?;
    if config.product_practice_smoke && practice.start(&selection, brawler.id, brawler.revision) {
        debug!(
            game_type = game.id.as_str(),
            "product Practice automation requested match"
        );
        Some(HeadlessQueueSmokeStage::AwaitingJoinedSnapshot)
    } else if !config.product_practice_smoke
        && queue.start_join(&selection, brawler.id, brawler.revision, now)
    {
        debug!(
            game_type = game.id.as_str(),
            "product match automation joined queue"
        );
        Some(HeadlessQueueSmokeStage::Joining)
    } else {
        None
    }
}

pub(super) fn automation_game_type<'a>(
    games: &'a [crate::lobby::AdvertisedGameType],
    players_per_team: u8,
    requested: Option<&crate::lobby::GameTypeId>,
) -> Option<&'a crate::lobby::AdvertisedGameType> {
    games.iter().find(|game| {
        requested.map_or_else(
            || game.players_per_team == players_per_team,
            |requested| &game.id == requested,
        )
    })
}
