//! Map publish request state and transport over the lobby session (BRL-w9z1).

use bevy::prelude::*;
use lightyear::prelude::{MessageReceiver, MessageSender};
use std::collections::VecDeque;

use crate::{Client, RoutedClientSession};

/// One in-flight publish request per lobby generation, and the last outcome for presentation.
#[derive(Resource, Clone, Debug, Default)]
pub struct ClientMapPublishModel {
    generation: Option<u64>,
    next_request_id: u64,
    pending: Option<brawler::lobby::MapPublishRequestId>,
    outbound: VecDeque<brawler::lobby::MapPublishRequest>,
    outcome: Option<brawler::lobby::MapPublishDecision>,
}

impl ClientMapPublishModel {
    pub(super) fn bind_generation(&mut self, generation: u64) {
        if self.generation != Some(generation) {
            *self = Self {
                generation: Some(generation),
                next_request_id: self.next_request_id,
                ..Self::default()
            };
        }
    }

    /// Queue one publish request for the selected game type. Returns false while another
    /// request is in flight or before a lobby session exists.
    pub fn start(
        &mut self,
        catalog_revision: brawler::lobby::CatalogRevision,
        game: &brawler::lobby::AdvertisedGameType,
        name: String,
        candidate: brawler::map::AuthoredMapCandidate,
    ) -> bool {
        if self.pending.is_some() || self.generation.is_none() {
            return false;
        }
        let Some(request_id) = self
            .next_request_id
            .checked_add(1)
            .and_then(brawler::lobby::MapPublishRequestId::new)
        else {
            return false;
        };
        self.next_request_id = request_id.get();
        self.pending = Some(request_id);
        self.outcome = None;
        self.outbound.push_back(brawler::lobby::MapPublishRequest {
            request_id,
            catalog_revision,
            game_type_id: game.id.clone(),
            game_type_configuration_revision: game.configuration_revision,
            name,
            candidate,
        });
        true
    }

    #[must_use]
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    /// The decision for the last request, once; presentation consumes it.
    pub fn take_outcome(&mut self) -> Option<brawler::lobby::MapPublishDecision> {
        self.outcome.take()
    }

    pub(super) fn accept_outcome(&mut self, outcome: brawler::lobby::MapPublishOutcome) {
        if self.pending == Some(outcome.request_id) {
            self.pending = None;
            self.outcome = Some(outcome.decision);
        }
    }
}

/// Player-facing text for a publish decision.
#[must_use]
pub fn map_publish_decision_text(decision: brawler::lobby::MapPublishDecision) -> String {
    use brawler::lobby::{MapPublishDecision, MapPublishRejection, PublishedMapState};
    match decision {
        MapPublishDecision::Accepted {
            state: PublishedMapState::Pending,
            ..
        } => "Published. The server operator reviews it before it enters rotation.".to_string(),
        MapPublishDecision::Accepted {
            state: PublishedMapState::Approved,
            ..
        } => "Published. It is now in this server's match rotation.".to_string(),
        MapPublishDecision::Rejected(reason) => match reason {
            MapPublishRejection::InvalidName => "Publish failed: rename the map first.".to_string(),
            MapPublishRejection::InvalidMapRecipe => {
                "Publish failed: the server could not validate this map.".to_string()
            }
            MapPublishRejection::RosterMismatch => {
                "Publish failed: add enough spawns for every player on each team.".to_string()
            }
            MapPublishRejection::StaleCatalog | MapPublishRejection::StaleGameConfiguration => {
                "Publish failed: the server's game list changed. Reconnect and try again."
                    .to_string()
            }
            MapPublishRejection::UnknownGameType | MapPublishRejection::CommunityMapsDisabled => {
                "This server no longer takes community maps for this mode.".to_string()
            }
            MapPublishRejection::PendingLimitReached => {
                "You already have the most maps waiting for review on this server.".to_string()
            }
            MapPublishRejection::CoolingDown {
                retry_after_seconds,
            } => format!("Publish again in {retry_after_seconds} s."),
            MapPublishRejection::StoreFull => {
                "This server's community map store is full.".to_string()
            }
            MapPublishRejection::Busy | MapPublishRejection::Internal => {
                "The server could not publish right now. Try again.".to_string()
            }
        },
    }
}

#[allow(
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "one lobby-session query owns the publish request transport"
)]
pub(super) fn exchange_map_publish_messages(
    mut model: ResMut<ClientMapPublishModel>,
    lifecycle: Res<crate::RoutedClientLifecycle>,
    mut clients: Query<
        (
            &RoutedClientSession,
            Option<&mut MessageReceiver<brawler::lobby::MapPublishOutcome>>,
            Option<&mut MessageSender<brawler::lobby::MapPublishRequest>>,
        ),
        With<Client>,
    >,
) {
    for (session, receiver, sender) in &mut clients {
        if session.kind != crate::RoutedClientSessionKind::Lobby
            || session.generation != lifecycle.generation
        {
            continue;
        }
        model.bind_generation(session.generation);
        if let Some(mut receiver) = receiver {
            for outcome in receiver.receive() {
                model.accept_outcome(outcome);
            }
        }
        if let Some(mut sender) = sender {
            while let Some(request) = model.outbound.pop_front() {
                sender.send::<brawler::protocol::SessionChannel>(request);
            }
        }
    }
}
