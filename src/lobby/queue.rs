//! Shared, bounded queue wire contract.

use crate::{
    builds::{AcceptedBuildSummary, BUILD_POINT_BUDGET},
    lobby::{CatalogRevision, GameTypeId, MAX_GAME_TYPES},
};
use serde::{Deserialize, Deserializer, Serialize, de::SeqAccess, de::Visitor};

pub const MAX_QUEUE_OUTCOME_BYTES: usize = 512;
pub const MAX_QUEUE_TICKETS: u16 = 32;
pub const MAX_QUEUE_FORMATION_SIZE: u8 = 8;
pub const MAX_QUEUE_RETRY_AFTER_MILLIS: u16 = 1_000;

macro_rules! nonzero_id {
    ($name:ident, $repr:ty, $description:literal) => {
        #[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name($repr);

        impl $name {
            #[must_use]
            pub const fn new(value: $repr) -> Option<Self> {
                if value == 0 { None } else { Some(Self(value)) }
            }

            #[must_use]
            pub const fn get(self) -> $repr {
                self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = <$repr>::deserialize(deserializer)?;
                Self::new(value).ok_or_else(|| serde::de::Error::custom($description))
            }
        }
    };
}

nonzero_id!(QueueRequestId, u64, "queue request ID must be nonzero");
nonzero_id!(
    PracticeRequestId,
    u64,
    "practice request ID must be nonzero"
);
nonzero_id!(QueueTicketId, u128, "queue ticket ID must be nonzero");
nonzero_id!(
    MatchReservationId,
    u128,
    "match reservation ID must be nonzero"
);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchLoadingPhase {
    Reserving,
    StartingServer,
    Connecting,
    Synchronizing,
    WaitingForPlayers,
    Cancelling,
    ReturningToQueue,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ReservationStarted {
    pub reservation_id: MatchReservationId,
    pub ticket_id: Option<QueueTicketId>,
    pub game_type_id: GameTypeId,
    pub map_preset_id: crate::map::MapPresetId,
    pub team_count: u8,
    pub players_per_team: u8,
    pub accepted_build: AcceptedBuildSummary,
    pub loading_deadline_millis: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BeginMatchConnect {
    pub reservation_id: MatchReservationId,
    pub generation: u32,
    pub grant: crate::protocol::MatchRouteGrant,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum MatchmakingServerPhase {
    ReservationStarted(ReservationStarted),
    BeginMatchConnect(BeginMatchConnect),
    Status {
        reservation_id: MatchReservationId,
        generation: u32,
        phase: MatchLoadingPhase,
        connected: u8,
        checked_in: u8,
        participant_count: u8,
    },
    Removed {
        reservation_id: MatchReservationId,
        ticket_id: Option<QueueTicketId>,
        reason: MatchStartFailure,
    },
    PracticeRejected {
        request_id: PracticeRequestId,
        reason: PracticeStartRejection,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PracticeStartRequest {
    pub request_id: PracticeRequestId,
    pub catalog_revision: CatalogRevision,
    pub game_type_id: GameTypeId,
    pub game_type_configuration_revision: u32,
    pub brawler_id: crate::profiles::SavedBrawlerId,
    pub brawler_revision: crate::profiles::ProfileRevision,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PracticeStartRejection {
    StaleCatalog,
    UnknownGameType,
    StaleGameConfiguration,
    InvalidBuild,
    IncompatibleBuild,
    Busy,
    CapacityUnavailable,
    Internal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MatchmakingServerMessage {
    pub sequence: u64,
    pub phase: MatchmakingServerPhase,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchmakingClientAction {
    Cancel {
        reservation_id: MatchReservationId,
        generation: u32,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchmakingClientMessage {
    pub sequence: u64,
    pub action: MatchmakingClientAction,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchStartFailure {
    Cancelled,
    ParticipantLost,
    WorkerFailed,
    TimedOut,
    CapacityOccupied,
    Expired,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum QueueClientMessage {
    Command {
        request_id: QueueRequestId,
        command: QueueCommand,
    },
    OutcomeAck {
        request_id: QueueRequestId,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum QueueCommand {
    Join(QueueJoinCommand),
    Cancel(QueueCancelCommand),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct QueueJoinCommand {
    pub catalog_revision: CatalogRevision,
    pub game_type_id: GameTypeId,
    pub game_type_configuration_revision: u32,
    pub brawler_id: crate::profiles::SavedBrawlerId,
    pub brawler_revision: crate::profiles::ProfileRevision,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueCancelCommand {
    pub ticket_id: QueueTicketId,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct QueueCommandOutcome {
    pub request_id: QueueRequestId,
    pub decision: QueueDecision,
}

impl<'de> Deserialize<'de> for QueueCommandOutcome {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            request_id: QueueRequestId,
            decision: QueueDecision,
        }
        let wire = Wire::deserialize(deserializer)?;
        match &wire.decision {
            QueueDecision::Cancelled {
                resulting_pool_state_revision,
                ..
            } if *resulting_pool_state_revision == 0 => {
                return Err(serde::de::Error::custom(
                    "cancelled queue revision must be nonzero",
                ));
            }
            QueueDecision::Rejected(QueueRejection::OverBudget { used, budget })
                if *budget != BUILD_POINT_BUDGET || *used <= *budget =>
            {
                return Err(serde::de::Error::custom(
                    "over-budget outcome must carry the canonical exceeded budget",
                ));
            }
            QueueDecision::Rejected(QueueRejection::RateLimited { retry_after_millis })
                if !(1..=MAX_QUEUE_RETRY_AFTER_MILLIS).contains(retry_after_millis) =>
            {
                return Err(serde::de::Error::custom(
                    "queue retry delay is outside its wire bound",
                ));
            }
            _ => {}
        }
        Ok(Self {
            request_id: wire.request_id,
            decision: wire.decision,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum QueueDecision {
    Joined(QueueMembership),
    Cancelled {
        ticket_id: QueueTicketId,
        resulting_pool_state_revision: u64,
    },
    Rejected(QueueRejection),
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct QueueMembership {
    pub ticket_id: QueueTicketId,
    pub catalog_revision: CatalogRevision,
    pub game_type_id: GameTypeId,
    pub game_type_configuration_revision: u32,
    pub brawler_id: crate::profiles::SavedBrawlerId,
    pub brawler_revision: crate::profiles::ProfileRevision,
    pub accepted_build: AcceptedBuildSummary,
    pub admitted_at_pool_state_revision: u64,
}

impl<'de> Deserialize<'de> for QueueMembership {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            ticket_id: QueueTicketId,
            catalog_revision: CatalogRevision,
            game_type_id: GameTypeId,
            game_type_configuration_revision: u32,
            brawler_id: crate::profiles::SavedBrawlerId,
            brawler_revision: crate::profiles::ProfileRevision,
            accepted_build: AcceptedBuildSummary,
            admitted_at_pool_state_revision: u64,
        }
        let wire = Wire::deserialize(deserializer)?;
        if wire.game_type_configuration_revision == 0
            || wire.admitted_at_pool_state_revision == 0
            || wire.accepted_build.total_points > BUILD_POINT_BUDGET
        {
            return Err(serde::de::Error::custom(
                "queue membership contains an impossible revision or point total",
            ));
        }
        Ok(Self {
            ticket_id: wire.ticket_id,
            catalog_revision: wire.catalog_revision,
            game_type_id: wire.game_type_id,
            game_type_configuration_revision: wire.game_type_configuration_revision,
            brawler_id: wire.brawler_id,
            brawler_revision: wire.brawler_revision,
            accepted_build: wire.accepted_build,
            admitted_at_pool_state_revision: wire.admitted_at_pool_state_revision,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum QueueRejection {
    IncompatiblePassives,
    OverBudget { used: u8, budget: u8 },
    StaleCatalog,
    StaleGameConfiguration,
    UnknownGameType,
    MustCancelFirst,
    TicketMismatch,
    StaleRequest,
    TemporarilyUnavailable,
    ServerMatchCapacityOccupied,
    InternalBuildResolution,
    RateLimited { retry_after_millis: u16 },
    ProtocolFailure,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct QueuePoolSnapshot {
    pub catalog_revision: CatalogRevision,
    pub state_revision: u64,
    pub formation_availability: FormationAvailability,
    pub pools: Vec<QueuePoolRow>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FormationAvailability {
    #[default]
    Available,
    ProductMatchOccupied,
}

impl<'de> Deserialize<'de> for QueuePoolSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            catalog_revision: CatalogRevision,
            state_revision: u64,
            formation_availability: FormationAvailability,
            #[serde(deserialize_with = "deserialize_pool_rows")]
            pools: Vec<QueuePoolRow>,
        }
        let wire = Wire::deserialize(deserializer)?;
        if wire.state_revision == 0 || wire.pools.is_empty() {
            return Err(serde::de::Error::custom(
                "queue snapshot revision and pools must be nonzero",
            ));
        }
        Ok(Self {
            catalog_revision: wire.catalog_revision,
            state_revision: wire.state_revision,
            formation_availability: wire.formation_availability,
            pools: wire.pools,
        })
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct QueuePoolRow {
    pub game_type_id: GameTypeId,
    pub game_type_configuration_revision: u32,
    pub queued: u16,
    pub formation_size: u8,
}

impl<'de> Deserialize<'de> for QueuePoolRow {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            game_type_id: GameTypeId,
            game_type_configuration_revision: u32,
            queued: u16,
            formation_size: u8,
        }
        let wire = Wire::deserialize(deserializer)?;
        if wire.game_type_configuration_revision == 0
            || wire.queued > MAX_QUEUE_TICKETS
            || !(1..=MAX_QUEUE_FORMATION_SIZE).contains(&wire.formation_size)
        {
            return Err(serde::de::Error::custom(
                "queue pool row exceeds its revision, count, or topology bound",
            ));
        }
        Ok(Self {
            game_type_id: wire.game_type_id,
            game_type_configuration_revision: wire.game_type_configuration_revision,
            queued: wire.queued,
            formation_size: wire.formation_size,
        })
    }
}

fn deserialize_pool_rows<'de, D>(deserializer: D) -> Result<Vec<QueuePoolRow>, D::Error>
where
    D: Deserializer<'de>,
{
    struct RowsVisitor;
    impl<'de> Visitor<'de> for RowsVisitor {
        type Value = Vec<QueuePoolRow>;

        fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(
                formatter,
                "between one and {MAX_GAME_TYPES} queue pool rows"
            )
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            if sequence
                .size_hint()
                .is_some_and(|length| length > MAX_GAME_TYPES)
            {
                return Err(serde::de::Error::invalid_length(
                    sequence.size_hint().unwrap_or(MAX_GAME_TYPES + 1),
                    &self,
                ));
            }
            let mut rows = Vec::new();
            while let Some(row) = sequence.next_element()? {
                if rows.len() == MAX_GAME_TYPES {
                    return Err(serde::de::Error::invalid_length(rows.len() + 1, &self));
                }
                rows.push(row);
            }
            if rows.is_empty() {
                return Err(serde::de::Error::invalid_length(0, &self));
            }
            Ok(rows)
        }
    }
    deserializer.deserialize_seq(RowsVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(index: u8) -> QueuePoolRow {
        QueuePoolRow {
            game_type_id: GameTypeId::new(format!("game-{index}")).unwrap(),
            game_type_configuration_revision: 1,
            queued: 0,
            formation_size: 4,
        }
    }

    #[test]
    fn zero_request_and_ticket_ids_fail_during_decode() {
        assert!(postcard::from_bytes::<QueueRequestId>(&[0]).is_err());
        assert!(postcard::from_bytes::<QueueTicketId>(&[0]).is_err());
    }

    #[test]
    fn snapshot_rejects_zero_revision_empty_and_overbound_rows() {
        let revision = CatalogRevision([1; 32]);
        for snapshot in [
            QueuePoolSnapshot {
                catalog_revision: revision,
                state_revision: 0,
                formation_availability: FormationAvailability::Available,
                pools: vec![row(0)],
            },
            QueuePoolSnapshot {
                catalog_revision: revision,
                state_revision: 1,
                formation_availability: FormationAvailability::Available,
                pools: Vec::new(),
            },
            QueuePoolSnapshot {
                catalog_revision: revision,
                state_revision: 1,
                formation_availability: FormationAvailability::Available,
                pools: (0..=u8::try_from(MAX_GAME_TYPES).unwrap())
                    .map(row)
                    .collect(),
            },
        ] {
            let encoded = postcard::to_allocvec(&snapshot).unwrap();
            assert!(postcard::from_bytes::<QueuePoolSnapshot>(&encoded).is_err());
        }
    }

    #[test]
    fn pool_rows_reject_impossible_counts_revisions_and_topology() {
        for invalid in [
            QueuePoolRow {
                queued: MAX_QUEUE_TICKETS + 1,
                ..row(0)
            },
            QueuePoolRow {
                game_type_configuration_revision: 0,
                ..row(0)
            },
            QueuePoolRow {
                formation_size: 0,
                ..row(0)
            },
            QueuePoolRow {
                formation_size: MAX_QUEUE_FORMATION_SIZE + 1,
                ..row(0)
            },
        ] {
            let encoded = postcard::to_allocvec(&invalid).unwrap();
            assert!(postcard::from_bytes::<QueuePoolRow>(&encoded).is_err());
        }
        let maximum = QueuePoolRow {
            queued: MAX_QUEUE_TICKETS,
            formation_size: MAX_QUEUE_FORMATION_SIZE,
            ..row(0)
        };
        let encoded = postcard::to_allocvec(&maximum).unwrap();
        assert_eq!(postcard::from_bytes::<QueuePoolRow>(&encoded), Ok(maximum));
    }

    #[test]
    fn outcomes_reject_zero_revisions_and_invalid_bounded_details() {
        let request_id = QueueRequestId::new(1).unwrap();
        let ticket_id = QueueTicketId::new(1).unwrap();
        for decision in [
            QueueDecision::Cancelled {
                ticket_id,
                resulting_pool_state_revision: 0,
            },
            QueueDecision::Rejected(QueueRejection::RateLimited {
                retry_after_millis: 0,
            }),
            QueueDecision::Rejected(QueueRejection::RateLimited {
                retry_after_millis: MAX_QUEUE_RETRY_AFTER_MILLIS + 1,
            }),
            QueueDecision::Rejected(QueueRejection::OverBudget {
                used: BUILD_POINT_BUDGET,
                budget: BUILD_POINT_BUDGET,
            }),
            QueueDecision::Rejected(QueueRejection::OverBudget {
                used: BUILD_POINT_BUDGET + 1,
                budget: BUILD_POINT_BUDGET - 1,
            }),
        ] {
            let outcome = QueueCommandOutcome {
                request_id,
                decision,
            };
            let encoded = postcard::to_allocvec(&outcome).unwrap();
            assert!(postcard::from_bytes::<QueueCommandOutcome>(&encoded).is_err());
        }
        let maximum = QueueCommandOutcome {
            request_id,
            decision: QueueDecision::Rejected(QueueRejection::RateLimited {
                retry_after_millis: MAX_QUEUE_RETRY_AFTER_MILLIS,
            }),
        };
        let encoded = postcard::to_allocvec(&maximum).unwrap();
        assert_eq!(
            postcard::from_bytes::<QueueCommandOutcome>(&encoded),
            Ok(maximum)
        );
    }
}
