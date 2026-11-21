//! Headless queue, Practice, product-match, and requeue automation.

use bevy::prelude::*;
use std::fmt::Write as _;
use std::time::Duration;

use crate::{
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
    config: Res<crate::ClientAutomationConfig>,
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
        session.kind == crate::RoutedClientSessionKind::Lobby && session.generation >= 3
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
    config: Res<crate::ClientAutomationConfig>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    mut model: ResMut<ClientQueueModel>,
    mut practice: ResMut<ClientPracticeModel>,
    mut stage: ResMut<HeadlessQueueSmokeStage>,
    mut exit: MessageWriter<AppExit>,
) {
    if (!config.product_queue_smoke && !config.product_match_smoke)
        || config.product_publish_map_smoke
        || config.product_published_maps_smoke
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
        && let brawler::lobby::QueueDecision::Rejected(reason) = outcome.decision
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
    config: &crate::ClientAutomationConfig,
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
    let selection = crate::flow::SelectedGameType {
        catalog_revision: Some(lobby.catalog_revision),
        game_type_id: Some(game.id.clone()),
        configuration_revision: Some(game.configuration_revision),
    };
    // `--character` waits until profile automation has selected the requested character, so the
    // server resolves that character's kit for the match.
    if !crate::profile::automation_character_ready(config, &lobby.profile) {
        return None;
    }
    let profile_revision = lobby.profile.revision;
    if let Some(map) = config.product_practice_map {
        practice.selected_map = Some((game.id.clone(), map));
    }
    if config.product_practice_smoke && practice.start(&selection, profile_revision) {
        debug!(
            game_type = game.id.as_str(),
            "product Practice automation requested match"
        );
        Some(HeadlessQueueSmokeStage::AwaitingJoinedSnapshot)
    } else if !config.product_practice_smoke && queue.start_join(&selection, profile_revision, now)
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

/// Headless BRL-w9z1 evidence: publish one copy of the selected game type's first built-in map
/// and exit once the server decides. Acceptance prints a parseable marker for the e2e script.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(super) fn drive_headless_publish_smoke(
    config: Res<crate::ClientAutomationConfig>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    mut publish: ResMut<crate::ClientMapPublishModel>,
    mut started: Local<bool>,
    mut exit: MessageWriter<AppExit>,
) {
    if !config.product_publish_map_smoke {
        return;
    }
    if *started {
        match publish.take_outcome() {
            Some(brawler::lobby::MapPublishDecision::Accepted {
                published_map_id,
                state,
            }) => {
                let marker = format!(
                    "brawler-client published-map id={} state={state:?}\n",
                    published_map_id.get()
                );
                let _ = std::io::Write::write_all(&mut std::io::stderr().lock(), marker.as_bytes());
                exit.write(AppExit::Success);
            }
            Some(brawler::lobby::MapPublishDecision::Rejected(reason)) => {
                error!(?reason, "brawler product publish smoke was rejected");
                exit.write(AppExit::error());
            }
            None => {}
        }
        return;
    }
    let Some(lobby) = memberships.iter().next() else {
        return;
    };
    let Some(game) = automation_game_type(
        &lobby.game_types,
        config.product_match_players_per_team,
        config.product_match_game_type.as_ref(),
    ) else {
        return;
    };
    let Some(recipe) = brawler::map::MapContentCatalog::embedded()
        .ok()
        .and_then(|maps| {
            game.map_preset_ids
                .first()
                .and_then(|id| maps.preset(*id))
                .map(|preset| preset.recipe.clone())
        })
    else {
        error!("brawler product publish smoke found no built-in map to copy");
        exit.write(AppExit::error());
        return;
    };
    *started = publish.start(
        lobby.catalog_revision,
        game,
        config
            .publish_map_name
            .clone()
            .unwrap_or_else(|| "E2E Published Yard".to_string()),
        brawler::map::AuthoredMapCandidate {
            draft_id: 1,
            recipe,
        },
    );
}

/// Headless BRL-s3dh evidence: list (or first withdraw one of) this account's published maps,
/// print one parseable row per map, and exit once the server answers.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(super) fn drive_headless_published_maps_smoke(
    config: Res<crate::ClientAutomationConfig>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    mut published: ResMut<crate::ClientPublishedMapsModel>,
    mut started: Local<bool>,
    mut exit: MessageWriter<AppExit>,
) {
    if !config.product_published_maps_smoke {
        return;
    }
    if !*started {
        if memberships.iter().next().is_some() {
            *started = match config.withdraw_published_map {
                Some(id) => published.withdraw(id),
                None => published.request_list(),
            };
        }
        return;
    }
    if published.pending() {
        return;
    }
    if let Some(reason) = published.take_rejection() {
        error!(?reason, "brawler published maps smoke was rejected");
        exit.write(AppExit::error());
        return;
    }
    let mut report = String::new();
    for map in published.maps().unwrap_or_default() {
        let state = match &map.listing {
            brawler::lobby::PublishedMapListing::Pending => "pending",
            brawler::lobby::PublishedMapListing::Approved { in_rotation: true } => {
                "approved-in-rotation"
            }
            brawler::lobby::PublishedMapListing::Approved { in_rotation: false } => "approved",
            brawler::lobby::PublishedMapListing::Rejected { .. } => "rejected",
            brawler::lobby::PublishedMapListing::Removed { .. } => "removed",
        };
        let _ = writeln!(
            report,
            "brawler-client published-map-row id={} state={state} game_type={} name=\"{}\" detail=\"{}\"",
            map.published_map_id.get(),
            map.game_type_id.as_str(),
            map.name,
            crate::published_map_detail(&map.listing),
        );
    }
    let _ = std::io::Write::write_all(&mut std::io::stderr().lock(), report.as_bytes());
    exit.write(AppExit::Success);
}

/// Headless BRL-w9z1 evidence: print the Match Loading credit of a published map once.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(super) fn report_published_map_credit(
    config: Res<crate::ClientAutomationConfig>,
    loading: Res<crate::ClientMatchLoadingModel>,
    mut reported: Local<bool>,
) {
    if !config.expect_published_map || *reported {
        return;
    }
    if let Some(credit) = loading
        .active()
        .and_then(|active| active.published_map.as_ref())
    {
        *reported = true;
        let marker = format!(
            "brawler-client match-loading credit=\"{} — by {}\" published_map_id={}\n",
            credit.name,
            credit.author_display_name,
            credit.published_map_id.get()
        );
        let _ = std::io::Write::write_all(&mut std::io::stderr().lock(), marker.as_bytes());
    }
}

pub(super) fn automation_game_type<'a>(
    games: &'a [brawler::lobby::AdvertisedGameType],
    players_per_team: u8,
    requested: Option<&brawler::lobby::GameTypeId>,
) -> Option<&'a brawler::lobby::AdvertisedGameType> {
    games.iter().find(|game| {
        requested.map_or_else(
            || game.players_per_team == players_per_team,
            |requested| &game.id == requested,
        )
    })
}
