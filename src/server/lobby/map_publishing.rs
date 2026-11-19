//! Lobby-owned map publishing and community-map rotation (BRL-w9z1).
//!
//! A player publishes a saved Map Maker recipe over the lobby session. The lobby validates it
//! with the same authoritative acceptance path Practice uses, then the profile storage executor
//! applies quota, cooldown, and capacity and stores it beside the profiles. Approved maps are
//! re-read on a bounded interval, re-validated against the current content and game type, and
//! appended to that game type's queue rotation after its built-in maps. None of this changes the
//! gameplay content fingerprint or the advertised catalog revision.

use super::{LobbyClient, LobbyState, catalog};
use crate::{
    lobby::{
        AdvertisedGameType, GameTypeId, MapPublishDecision, MapPublishOutcome, MapPublishRejection,
        MapPublishRequest, MapPublishRequestId, PublishedMapCredit, PublishedMapId,
    },
    profiles::{
        ProfileAuthority, PublishedMapLimits, PublishedMapRecord, PublishedMapStorageCompletion,
        PublishedMapSubmission, PublishedMapSubmissionOutcome,
    },
    protocol::SessionChannel,
};
use bevy::prelude::*;
use brawler_routing::LobbySessionId;
use lightyear::prelude::{Connected, Disconnected, MessageReceiver, MessageSender};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

/// How often the lobby re-reads approved maps so operator CLI decisions reach rotation without
/// a restart. The lobby's own automatic approvals refresh immediately.
pub(super) const COMMUNITY_ROTATION_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

/// A player-authored recipe that passed the authoritative validator and the game type's exact
/// roster capacity check.
pub(super) struct AcceptedAuthoredMap {
    pub recipe_bytes: Vec<u8>,
    pub identity: crate::map::ResolvedMapIdentity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AuthoredMapRejection {
    InvalidRecipe,
    RosterMismatch,
}

/// The one lobby acceptance path for player-authored recipes: Practice test-play, publishing,
/// and community rotation all run exactly this check before a recipe reaches a match manifest.
pub(super) fn accept_authored_map(
    recipe: &crate::map::MapRecipe,
    game: &AdvertisedGameType,
    maps: &crate::map::MapContentCatalog,
) -> Result<AcceptedAuthoredMap, AuthoredMapRejection> {
    let resolved = crate::map::validate_authored_recipe(
        recipe,
        game.mode_definition_id,
        maps,
        crate::map::MapInstanceId(1),
    )
    .map_err(|_| AuthoredMapRejection::InvalidRecipe)?;
    let capacity = crate::matchplay::ResolvedMatchCapacity {
        team_slots: (0..game.team_count)
            .map(|team_slot| crate::matchplay::TeamSlotCapacity {
                team_slot,
                minimum_participants: game.players_per_team,
                maximum_participants: game.players_per_team,
            })
            .collect(),
        maximum_active_fighters: game.team_count * game.players_per_team,
    };
    capacity
        .validate_against_spawn_catalog(&crate::map::SpawnPointCatalog(
            resolved.spawn_points_by_team.clone(),
        ))
        .map_err(|_| AuthoredMapRejection::RosterMismatch)?;
    Ok(AcceptedAuthoredMap {
        recipe_bytes: postcard::to_allocvec(recipe)
            .map_err(|_| AuthoredMapRejection::InvalidRecipe)?,
        identity: resolved.snapshot.identity,
    })
}

/// One approved published map, re-validated for the game type whose rotation holds it.
#[derive(Debug, PartialEq, Eq)]
pub struct CommunityMap {
    pub published_map_id: PublishedMapId,
    pub name: String,
    pub author_display_name: String,
    pub recipe_bytes: Vec<u8>,
    pub recipe_id: crate::map::MapRecipeId,
    pub recipe_revision: u32,
    pub recipe_fingerprint: crate::map::MapRecipeFingerprint,
}

impl CommunityMap {
    #[must_use]
    pub fn credit(&self) -> PublishedMapCredit {
        PublishedMapCredit {
            published_map_id: self.published_map_id,
            name: self.name.clone(),
            author_display_name: self.author_display_name.clone(),
        }
    }
}

/// Approved community maps per opted-in game type, in stable approval order. Queue formation
/// reads it; only the refresh below replaces it.
#[derive(Resource, Debug, Default)]
pub struct CommunityMapRotation {
    maps: BTreeMap<GameTypeId, Vec<Arc<CommunityMap>>>,
    next_refresh: Duration,
    refresh_request: Option<u64>,
    refresh_soon: bool,
}

impl CommunityMapRotation {
    #[must_use]
    pub fn maps_for(&self, game_type_id: &GameTypeId) -> &[Arc<CommunityMap>] {
        self.maps.get(game_type_id).map_or(&[], Vec::as_slice)
    }

    /// Replace the rotation from stored approved rows. A row whose game type no longer opts in,
    /// changed its configuration revision, or whose recipe no longer validates against the
    /// current content is left out (it stays stored and recoverable) and logged.
    pub(super) fn replace_from_records(
        &mut self,
        records: Vec<PublishedMapRecord>,
        catalog: &catalog::ResolvedLobbyCatalog,
        maps: &crate::map::MapContentCatalog,
    ) {
        let mut rotation: BTreeMap<GameTypeId, Vec<Arc<CommunityMap>>> = BTreeMap::new();
        for record in records {
            let id = record.published_map_id.get();
            let Some(game) = catalog
                .game_types
                .iter()
                .find(|game| game.id == record.game_type_id && game.community_maps)
            else {
                warn!(
                    published_map_id = id,
                    "published map game type no longer takes community maps"
                );
                continue;
            };
            if game.configuration_revision != record.game_type_revision {
                warn!(
                    published_map_id = id,
                    "published map game type configuration changed"
                );
                continue;
            }
            let Ok(recipe) = postcard::from_bytes::<crate::map::MapRecipe>(&record.recipe) else {
                warn!(
                    published_map_id = id,
                    "published map recipe no longer decodes"
                );
                continue;
            };
            if Some(recipe.recipe_id) != record.published_map_id.recipe_id() {
                warn!(
                    published_map_id = id,
                    "published map recipe identity mismatch"
                );
                continue;
            }
            let accepted = match accept_authored_map(&recipe, game, maps) {
                Ok(accepted) => accepted,
                Err(reason) => {
                    warn!(
                        published_map_id = id,
                        ?reason,
                        "published map no longer validates"
                    );
                    continue;
                }
            };
            let entry = rotation.entry(game.id.clone()).or_default();
            if entry.len()
                >= usize::try_from(crate::profiles::MAX_ROTATION_MAPS_PER_GAME_TYPE)
                    .unwrap_or(usize::MAX)
            {
                continue;
            }
            entry.push(Arc::new(CommunityMap {
                published_map_id: record.published_map_id,
                name: record.name,
                author_display_name: record.author_display_name,
                recipe_bytes: accepted.recipe_bytes,
                recipe_id: accepted.identity.recipe_id,
                recipe_revision: accepted.identity.recipe_revision,
                recipe_fingerprint: accepted.identity.recipe_fingerprint,
            }));
        }
        if rotation != self.maps {
            info!(
                community_maps = rotation.values().map(Vec::len).sum::<usize>(),
                "community map rotation refreshed"
            );
        }
        self.maps = rotation;
    }
}

/// Publish requests waiting on the storage owner, at most one per lobby session.
#[derive(Resource, Debug, Default)]
pub(super) struct PendingMapPublications {
    by_storage_request: BTreeMap<u64, (LobbySessionId, MapPublishRequestId)>,
    sessions: BTreeSet<LobbySessionId>,
}

fn unix_now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Everything about a publish request the lobby can decide without storage.
fn validate_publish_request(
    request: &MapPublishRequest,
    catalog: &catalog::ResolvedLobbyCatalog,
    maps: &crate::map::MapContentCatalog,
) -> Result<GameTypeId, MapPublishRejection> {
    if request.catalog_revision != catalog.revision {
        return Err(MapPublishRejection::StaleCatalog);
    }
    let game = catalog
        .game_types
        .iter()
        .find(|game| game.id == request.game_type_id)
        .ok_or(MapPublishRejection::UnknownGameType)?;
    if game.configuration_revision != request.game_type_configuration_revision {
        return Err(MapPublishRejection::StaleGameConfiguration);
    }
    if !game.community_maps {
        return Err(MapPublishRejection::CommunityMapsDisabled);
    }
    if crate::map::authoring::normalize_map_name(&request.name).as_deref()
        != Ok(request.name.as_str())
    {
        return Err(MapPublishRejection::InvalidName);
    }
    if request.candidate.draft_id == 0 {
        return Err(MapPublishRejection::InvalidMapRecipe);
    }
    accept_authored_map(&request.candidate.recipe, game, maps).map_err(|reason| match reason {
        AuthoredMapRejection::InvalidRecipe => MapPublishRejection::InvalidMapRecipe,
        AuthoredMapRejection::RosterMismatch => MapPublishRejection::RosterMismatch,
    })?;
    Ok(game.id.clone())
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "one bounded receiver query owns the per-session publish request transaction"
)]
pub(super) fn apply_map_publish_requests(
    state: Res<LobbyState>,
    catalog: Res<catalog::ResolvedLobbyCatalog>,
    mut authority: ResMut<ProfileAuthority>,
    mut pending: ResMut<PendingMapPublications>,
    mut clients: Query<(
        &LobbyClient,
        &mut MessageReceiver<MapPublishRequest>,
        &mut MessageSender<MapPublishOutcome>,
        Has<Connected>,
        Has<Disconnected>,
    )>,
) {
    let mut maps = None;
    for (client, mut receiver, mut sender, connected, disconnected) in &mut clients {
        // Lightyear's draining iterator discards anything beyond the one request per update.
        let Some(request) = receiver.receive().next() else {
            continue;
        };
        if !connected || disconnected {
            continue;
        }
        let reject = |sender: &mut MessageSender<MapPublishOutcome>, reason| {
            sender.send::<SessionChannel>(MapPublishOutcome {
                request_id: request.request_id,
                decision: MapPublishDecision::Rejected(reason),
            });
        };
        if pending.sessions.contains(&client.lobby_session_id) {
            reject(&mut sender, MapPublishRejection::Busy);
            continue;
        }
        let (Some(account_id), Some(author)) = (
            authority.account_for_client(client.client_id.get()),
            state.accepted_name(client.client_id),
        ) else {
            reject(&mut sender, MapPublishRejection::Busy);
            continue;
        };
        if maps.is_none() {
            maps = crate::map::MapContentCatalog::embedded().ok();
        }
        let Some(maps) = maps.as_ref() else {
            reject(&mut sender, MapPublishRejection::Internal);
            continue;
        };
        let game_type_id = match validate_publish_request(&request, &catalog, maps) {
            Ok(game_type_id) => game_type_id,
            Err(reason) => {
                info!(?reason, "map publish request rejected");
                reject(&mut sender, reason);
                continue;
            }
        };
        let policy = catalog.map_publishing();
        let submission = PublishedMapSubmission {
            account_id,
            author_display_name: author.to_string(),
            name: request.name.clone(),
            game_type_id,
            game_type_revision: request.game_type_configuration_revision,
            recipe: request.candidate.recipe.clone(),
            now_unix_ms: unix_now_millis(),
        };
        let limits = PublishedMapLimits {
            pending_per_account: policy.pending_per_account,
            submit_cooldown_millis: u64::try_from(policy.submit_cooldown.as_millis())
                .unwrap_or(u64::MAX),
            approve_immediately: policy.moderation == catalog::MapModeration::Automatic,
        };
        match authority.submit_published_map(submission, limits) {
            Ok(storage_request_id) => {
                pending.sessions.insert(client.lobby_session_id);
                pending.by_storage_request.insert(
                    storage_request_id,
                    (client.lobby_session_id, request.request_id),
                );
            }
            Err(error) => {
                warn!(?error, "map publish storage submission failed");
                reject(&mut sender, MapPublishRejection::Busy);
            }
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(super) fn apply_published_map_completions(
    catalog: Res<catalog::ResolvedLobbyCatalog>,
    mut authority: ResMut<ProfileAuthority>,
    mut pending: ResMut<PendingMapPublications>,
    mut rotation: ResMut<CommunityMapRotation>,
    mut clients: Query<(&LobbyClient, &mut MessageSender<MapPublishOutcome>)>,
) {
    for completion in authority.take_published_map_completions() {
        match completion {
            PublishedMapStorageCompletion::Submission {
                storage_request_id,
                result,
            } => {
                let Some((session_id, request_id)) =
                    pending.by_storage_request.remove(&storage_request_id)
                else {
                    continue;
                };
                pending.sessions.remove(&session_id);
                let decision = match result {
                    Ok(PublishedMapSubmissionOutcome::Accepted {
                        published_map_id,
                        state,
                    }) => {
                        info!(
                            published_map_id = published_map_id.get(),
                            ?state,
                            "map published"
                        );
                        if state == crate::lobby::PublishedMapState::Approved {
                            rotation.refresh_soon = true;
                        }
                        MapPublishDecision::Accepted {
                            published_map_id,
                            state,
                        }
                    }
                    Ok(PublishedMapSubmissionOutcome::Rejected(reason)) => {
                        MapPublishDecision::Rejected(reason)
                    }
                    Err(error) => {
                        warn!(?error, "map publish storage failed");
                        MapPublishDecision::Rejected(MapPublishRejection::Internal)
                    }
                };
                if let Some((_, mut sender)) = clients
                    .iter_mut()
                    .find(|(client, _)| client.lobby_session_id == session_id)
                {
                    sender.send::<SessionChannel>(MapPublishOutcome {
                        request_id,
                        decision,
                    });
                }
            }
            PublishedMapStorageCompletion::Rotation {
                storage_request_id,
                result,
            } => {
                if rotation.refresh_request != Some(storage_request_id) {
                    continue;
                }
                rotation.refresh_request = None;
                match (result, crate::map::MapContentCatalog::embedded()) {
                    (Ok(records), Ok(maps)) => {
                        rotation.replace_from_records(records, &catalog, &maps);
                    }
                    (Err(error), _) => warn!(?error, "community map rotation read failed"),
                    (_, Err(error)) => warn!(%error, "embedded map catalog failed"),
                }
            }
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(super) fn refresh_community_rotation(
    time: Res<Time<Real>>,
    catalog: Res<catalog::ResolvedLobbyCatalog>,
    mut authority: ResMut<ProfileAuthority>,
    mut rotation: ResMut<CommunityMapRotation>,
) {
    if rotation.refresh_request.is_some()
        || !catalog.game_types.iter().any(|game| game.community_maps)
    {
        return;
    }
    let now = time.elapsed();
    if !rotation.refresh_soon && now < rotation.next_refresh {
        return;
    }
    if let Ok(storage_request_id) = authority.request_published_rotation() {
        rotation.refresh_request = Some(storage_request_id);
        rotation.refresh_soon = false;
        rotation.next_refresh = now.saturating_add(COMMUNITY_ROTATION_REFRESH_INTERVAL);
    }
}
