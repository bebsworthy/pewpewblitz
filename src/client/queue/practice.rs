//! Practice-start request state and transport.

use bevy::prelude::*;
use lightyear::prelude::MessageSender;
use std::collections::VecDeque;

use crate::client::Client;

#[derive(Resource, Clone, Debug, Default)]
pub struct ClientPracticeModel {
    pub(super) generation: Option<u64>,
    pub(super) next_request_id: u64,
    pub(super) pending: Option<crate::lobby::PracticeStartRequest>,
    pub(super) outbound: VecDeque<crate::lobby::PracticeStartRequest>,
    pub(super) rejection: Option<crate::lobby::PracticeStartRejection>,
}

impl ClientPracticeModel {
    pub(super) fn bind_generation(&mut self, generation: u64) {
        if self.generation != Some(generation) {
            *self = Self {
                generation: Some(generation),
                next_request_id: self.next_request_id,
                ..Self::default()
            };
        }
    }

    #[cfg(test)]
    pub(crate) fn bind_generation_for_test(&mut self, generation: u64) {
        self.bind_generation(generation);
    }

    pub fn start(
        &mut self,
        selected: &crate::client::flow::SelectedGameType,
        brawler_id: crate::profiles::SavedBrawlerId,
        brawler_revision: crate::profiles::ProfileRevision,
    ) -> bool {
        let (Some(catalog_revision), Some(game_type_id), Some(configuration_revision)) = (
            selected.catalog_revision,
            selected.game_type_id.clone(),
            selected.configuration_revision,
        ) else {
            return false;
        };
        if self.pending.is_some() || self.generation.is_none() {
            return false;
        }
        let Some(request_id) = self
            .next_request_id
            .checked_add(1)
            .and_then(crate::lobby::PracticeRequestId::new)
        else {
            return false;
        };
        self.next_request_id = request_id.get();
        let request = crate::lobby::PracticeStartRequest {
            request_id,
            catalog_revision,
            game_type_id,
            game_type_configuration_revision: configuration_revision,
            brawler_id,
            brawler_revision,
        };
        self.pending = Some(request.clone());
        self.outbound.push_back(request);
        self.rejection = None;
        true
    }

    pub(super) fn accept_started(&mut self) {
        self.pending = None;
        self.rejection = None;
    }

    pub(super) fn accept_rejection(
        &mut self,
        request_id: crate::lobby::PracticeRequestId,
        reason: crate::lobby::PracticeStartRejection,
    ) {
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.request_id == request_id)
        {
            self.pending = None;
            self.rejection = Some(reason);
        }
    }

    pub fn take_rejection(&mut self) -> Option<crate::lobby::PracticeStartRejection> {
        self.rejection.take()
    }

    #[must_use]
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
}

pub(super) fn send_practice_messages(
    mut model: ResMut<ClientPracticeModel>,
    mut senders: Query<&mut MessageSender<crate::lobby::PracticeStartRequest>, With<Client>>,
) {
    let Ok(mut sender) = senders.single_mut() else {
        return;
    };
    while let Some(message) = model.outbound.pop_front() {
        sender.send::<crate::protocol::SessionChannel>(message);
    }
}
