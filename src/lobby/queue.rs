//! Shared, bounded queue wire contract.

use crate::{
    builds::{AcceptedBuildSummary, BuildCandidate},
    lobby::{CatalogRevision, GameTypeId, MAX_GAME_TYPES},
};
use serde::{Deserialize, Deserializer, Serialize, de::SeqAccess, de::Visitor};

pub const MAX_QUEUE_OUTCOME_BYTES: usize = 512;

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
nonzero_id!(QueueTicketId, u128, "queue ticket ID must be nonzero");

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
    pub build: BuildCandidate,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueCancelCommand {
    pub ticket_id: QueueTicketId,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct QueueCommandOutcome {
    pub request_id: QueueRequestId,
    pub decision: QueueDecision,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct QueueMembership {
    pub ticket_id: QueueTicketId,
    pub catalog_revision: CatalogRevision,
    pub game_type_id: GameTypeId,
    pub game_type_configuration_revision: u32,
    pub accepted_build: AcceptedBuildSummary,
    pub admitted_at_pool_state_revision: u64,
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
    InternalBuildResolution,
    RateLimited { retry_after_millis: u16 },
    ProtocolFailure,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct QueuePoolSnapshot {
    pub catalog_revision: CatalogRevision,
    pub state_revision: u64,
    pub pools: Vec<QueuePoolRow>,
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
            pools: wire.pools,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct QueuePoolRow {
    pub game_type_id: GameTypeId,
    pub game_type_configuration_revision: u32,
    pub queued: u16,
    pub formation_size: u8,
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
                pools: vec![row(0)],
            },
            QueuePoolSnapshot {
                catalog_revision: revision,
                state_revision: 1,
                pools: Vec::new(),
            },
            QueuePoolSnapshot {
                catalog_revision: revision,
                state_revision: 1,
                pools: (0..=8).map(row).collect(),
            },
        ] {
            let encoded = postcard::to_allocvec(&snapshot).unwrap();
            assert!(postcard::from_bytes::<QueuePoolSnapshot>(&encoded).is_err());
        }
    }
}
