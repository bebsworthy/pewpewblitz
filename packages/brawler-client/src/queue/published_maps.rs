//! The author's own published maps on the connected server: list and withdraw (BRL-s3dh).
//!
//! The lobby owns every row and answers only for this session's account. The client keeps the
//! last listing for My Maps and never infers a state the server did not send.

use bevy::prelude::*;
use brawler::lobby::{
    PublishedMapId, PublishedMapListing, PublishedMapSummary, PublishedMapsAction,
    PublishedMapsRejection, PublishedMapsReply, PublishedMapsRequest, PublishedMapsRequestId,
    PublishedMapsResult,
};
use lightyear::prelude::{MessageReceiver, MessageSender};
use std::collections::VecDeque;

use crate::{Client, RoutedClientSession};

/// One in-flight request per lobby generation and the last listing the server sent.
#[derive(Resource, Clone, Debug, Default)]
pub struct ClientPublishedMapsModel {
    generation: Option<u64>,
    next_request_id: u64,
    pending: Option<(PublishedMapsRequestId, PublishedMapsAction)>,
    outbound: VecDeque<PublishedMapsRequest>,
    maps: Option<Vec<PublishedMapSummary>>,
    rejection: Option<PublishedMapsRejection>,
    withdrawn: Option<PublishedMapId>,
    changed: bool,
}

impl ClientPublishedMapsModel {
    pub(super) fn bind_generation(&mut self, generation: u64) {
        if self.generation != Some(generation) {
            *self = Self {
                generation: Some(generation),
                next_request_id: self.next_request_id,
                changed: true,
                ..Self::default()
            };
        }
    }

    fn start(&mut self, action: PublishedMapsAction) -> bool {
        if self.pending.is_some() || self.generation.is_none() {
            return false;
        }
        let Some(request_id) = self
            .next_request_id
            .checked_add(1)
            .and_then(PublishedMapsRequestId::new)
        else {
            return false;
        };
        self.next_request_id = request_id.get();
        self.pending = Some((request_id, action));
        self.outbound
            .push_back(PublishedMapsRequest { request_id, action });
        true
    }

    /// Ask for this account's submissions. False while a request is in flight or offline.
    pub fn request_list(&mut self) -> bool {
        self.start(PublishedMapsAction::List)
    }

    /// Withdraw one of this account's pending or approved maps; the reply lists the rest.
    pub fn withdraw(&mut self, published_map_id: PublishedMapId) -> bool {
        self.start(PublishedMapsAction::Withdraw { published_map_id })
    }

    /// A lobby session exists and nothing has been listed for it yet.
    #[must_use]
    pub fn needs_list(&self) -> bool {
        self.generation.is_some() && self.maps.is_none() && self.pending.is_none()
    }

    #[must_use]
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Whether a withdraw of this map is in flight.
    #[must_use]
    pub fn withdrawing(&self, published_map_id: PublishedMapId) -> bool {
        matches!(
            self.pending,
            Some((_, PublishedMapsAction::Withdraw { published_map_id: id })) if id == published_map_id
        )
    }

    /// The last listing, newest first; `None` until the server answered this session.
    #[must_use]
    pub fn maps(&self) -> Option<&[PublishedMapSummary]> {
        self.maps.as_deref()
    }

    #[must_use]
    pub fn map(&self, published_map_id: PublishedMapId) -> Option<&PublishedMapSummary> {
        self.maps
            .as_deref()?
            .iter()
            .find(|map| map.published_map_id == published_map_id)
    }

    /// True once after the listing or the in-flight state changed; presentation rebuilds then.
    pub fn take_changed(&mut self) -> bool {
        core::mem::take(&mut self.changed)
    }

    /// The last request's refusal, once.
    pub fn take_rejection(&mut self) -> Option<PublishedMapsRejection> {
        self.rejection.take()
    }

    /// The map the last successful withdraw removed, once.
    pub fn take_withdrawn(&mut self) -> Option<PublishedMapId> {
        self.withdrawn.take()
    }

    /// Bind a lobby generation and answer the in-flight (or a new) request with one server
    /// listing, for flow tests without a session.
    #[cfg(test)]
    pub(crate) fn listed_for_test(&mut self, generation: u64, maps: Vec<PublishedMapSummary>) {
        self.bind_generation(generation);
        if self.pending.is_none() {
            assert!(self.request_list());
        }
        let request_id = self.pending.expect("a request is in flight").0;
        self.outbound.clear();
        self.accept_reply(PublishedMapsReply {
            request_id,
            result: PublishedMapsResult::Listed(maps),
        });
    }

    /// The requests waiting for the transport, for flow tests.
    #[cfg(test)]
    pub(crate) fn outbound_for_test(&self) -> Vec<PublishedMapsRequest> {
        self.outbound.iter().copied().collect()
    }

    pub(super) fn accept_reply(&mut self, reply: PublishedMapsReply) {
        let Some((request_id, action)) = self.pending else {
            return;
        };
        if request_id != reply.request_id {
            return;
        }
        self.pending = None;
        self.changed = true;
        match reply.result {
            PublishedMapsResult::Listed(maps) => {
                self.maps = Some(maps);
                if let PublishedMapsAction::Withdraw { published_map_id } = action {
                    self.withdrawn = Some(published_map_id);
                }
            }
            PublishedMapsResult::Rejected(reason) => self.rejection = Some(reason),
        }
    }
}

/// The short state chip of one published map.
#[must_use]
pub fn published_map_chip(listing: &PublishedMapListing) -> &'static str {
    match listing {
        PublishedMapListing::Pending => "PENDING",
        PublishedMapListing::Approved { in_rotation: true } => "IN ROTATION",
        PublishedMapListing::Approved { in_rotation: false } => "APPROVED",
        PublishedMapListing::Rejected { .. } => "REJECTED",
        PublishedMapListing::Removed { .. } => "REMOVED",
    }
}

/// The one-line explanation under a published map's chip.
#[must_use]
pub fn published_map_detail(listing: &PublishedMapListing) -> String {
    match listing {
        PublishedMapListing::Pending => "Waiting for the server operator's review.".to_string(),
        PublishedMapListing::Approved { in_rotation: true } => {
            "Plays in this server's match rotation.".to_string()
        }
        PublishedMapListing::Approved { in_rotation: false } => {
            "Approved, but not in rotation right now.".to_string()
        }
        PublishedMapListing::Rejected { reason } if reason.is_empty() => {
            "The operator rejected it.".to_string()
        }
        PublishedMapListing::Rejected { reason } => format!("Rejected: {reason}"),
        PublishedMapListing::Removed {
            reason: Some(reason),
        } => reason.clone(),
        PublishedMapListing::Removed { reason: None } => {
            "The operator took it out of rotation.".to_string()
        }
    }
}

/// Player-facing text for a refused list or withdraw request.
#[must_use]
pub fn published_maps_rejection_text(reason: PublishedMapsRejection) -> &'static str {
    match reason {
        PublishedMapsRejection::NotFound => "That map is not one of yours on this server.",
        PublishedMapsRejection::NotWithdrawable => {
            "That map is already out of review and rotation."
        }
        PublishedMapsRejection::Busy | PublishedMapsRejection::Internal => {
            "The server could not load your published maps. Try again."
        }
    }
}

#[allow(
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "one lobby-session query owns the published-maps request transport"
)]
pub(super) fn exchange_published_maps_messages(
    mut model: ResMut<ClientPublishedMapsModel>,
    lifecycle: Res<crate::RoutedClientLifecycle>,
    mut clients: Query<
        (
            &RoutedClientSession,
            Option<&mut MessageReceiver<PublishedMapsReply>>,
            Option<&mut MessageSender<PublishedMapsRequest>>,
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
            for reply in receiver.receive() {
                model.accept_reply(reply);
            }
        }
        if let Some(mut sender) = sender {
            while let Some(request) = model.outbound.pop_front() {
                sender.send::<brawler::protocol::SessionChannel>(request);
            }
        }
    }
}
