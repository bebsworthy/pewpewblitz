//! Shared, bounded wire contract for publishing a Map Maker map to a routed server (BRL-w9z1).
//!
//! The client sends only a typed authored candidate and a name. The lobby assigns the published
//! identity, re-validates the recipe with the authoritative validator, and stores it as server
//! data outside the gameplay content fingerprint and the advertised catalog revision.

use crate::lobby::{CatalogRevision, GameTypeId};
use serde::{Deserialize, Deserializer, Serialize};

/// Published recipes take `recipe_id = PUBLISHED_MAP_RECIPE_ID_BASE + published_map_id`, so a
/// published recipe never shares an identity with a built-in preset or a local draft ID.
pub const PUBLISHED_MAP_RECIPE_ID_BASE: u64 = 1 << 48;
/// Every accepted publication is stored as revision one of its server-assigned recipe identity.
pub const PUBLISHED_MAP_RECIPE_REVISION: u32 = 1;
/// Wire bound on a cooldown hint; the operator policy is validated against it.
pub const MAX_PUBLISH_COOLDOWN_SECONDS: u16 = 3_600;

macro_rules! nonzero_u64_id {
    ($name:ident, $description:literal) => {
        #[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            #[must_use]
            pub const fn new(value: u64) -> Option<Self> {
                if value == 0 { None } else { Some(Self(value)) }
            }

            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = u64::deserialize(deserializer)?;
                Self::new(value).ok_or_else(|| serde::de::Error::custom($description))
            }
        }
    };
}

nonzero_u64_id!(
    MapPublishRequestId,
    "map publish request ID must be nonzero"
);
nonzero_u64_id!(PublishedMapId, "published map ID must be nonzero");

impl PublishedMapId {
    /// The server-unique recipe identity a published map is stored and fingerprinted under.
    #[must_use]
    pub fn recipe_id(self) -> Option<crate::map::MapRecipeId> {
        PUBLISHED_MAP_RECIPE_ID_BASE
            .checked_add(self.0)
            .map(crate::map::MapRecipeId)
    }
}

fn deserialize_map_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    super::deserialize_validated_string(
        deserializer,
        crate::map::authoring::MAX_AUTHORED_MAP_NAME_BYTES,
        |value| {
            crate::map::authoring::normalize_map_name(value)
                .ok()
                .filter(|normalized| normalized == value)
        },
    )
}

/// Client-to-lobby request to publish one saved Map Maker draft revision for one game type.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MapPublishRequest {
    pub request_id: MapPublishRequestId,
    pub catalog_revision: CatalogRevision,
    pub game_type_id: GameTypeId,
    pub game_type_configuration_revision: u32,
    #[serde(deserialize_with = "deserialize_map_name")]
    pub name: String,
    pub candidate: crate::map::AuthoredMapCandidate,
}

/// The state a just-accepted submission entered: operator review or straight into rotation.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishedMapState {
    Pending,
    Approved,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapPublishRejection {
    InvalidName,
    InvalidMapRecipe,
    RosterMismatch,
    StaleCatalog,
    UnknownGameType,
    StaleGameConfiguration,
    CommunityMapsDisabled,
    PendingLimitReached,
    CoolingDown { retry_after_seconds: u16 },
    StoreFull,
    Busy,
    Internal,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapPublishDecision {
    Accepted {
        published_map_id: PublishedMapId,
        state: PublishedMapState,
    },
    Rejected(MapPublishRejection),
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapPublishOutcome {
    pub request_id: MapPublishRequestId,
    pub decision: MapPublishDecision,
}

impl<'de> Deserialize<'de> for MapPublishOutcome {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            request_id: MapPublishRequestId,
            decision: MapPublishDecision,
        }
        let wire = Wire::deserialize(deserializer)?;
        if let MapPublishDecision::Rejected(MapPublishRejection::CoolingDown {
            retry_after_seconds,
        }) = wire.decision
            && !(1..=MAX_PUBLISH_COOLDOWN_SECONDS).contains(&retry_after_seconds)
        {
            return Err(serde::de::Error::custom(
                "publish cooldown hint is outside its wire bound",
            ));
        }
        Ok(Self {
            request_id: wire.request_id,
            decision: wire.decision,
        })
    }
}

/// Author credit for a published map, carried by `ReservationStarted` for Match Loading.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublishedMapCredit {
    pub published_map_id: PublishedMapId,
    #[serde(deserialize_with = "deserialize_map_name")]
    pub name: String,
    #[serde(deserialize_with = "super::deserialize_accepted_player_name")]
    pub author_display_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_ids_and_unnormalized_names_fail_during_decode() {
        assert!(postcard::from_bytes::<MapPublishRequestId>(&[0]).is_err());
        assert!(postcard::from_bytes::<PublishedMapId>(&[0]).is_err());
        let credit = PublishedMapCredit {
            published_map_id: PublishedMapId::new(1).unwrap(),
            name: "  Spaced  ".to_string(),
            author_display_name: "Author".to_string(),
        };
        let encoded = postcard::to_allocvec(&credit).unwrap();
        assert!(postcard::from_bytes::<PublishedMapCredit>(&encoded).is_err());
        let credit = PublishedMapCredit {
            name: "Spaced".to_string(),
            ..credit
        };
        let encoded = postcard::to_allocvec(&credit).unwrap();
        assert_eq!(
            postcard::from_bytes::<PublishedMapCredit>(&encoded),
            Ok(credit)
        );
    }

    #[test]
    fn cooldown_hint_is_bounded_on_the_wire() {
        let request_id = MapPublishRequestId::new(1).unwrap();
        for retry_after_seconds in [0, MAX_PUBLISH_COOLDOWN_SECONDS + 1] {
            let outcome = MapPublishOutcome {
                request_id,
                decision: MapPublishDecision::Rejected(MapPublishRejection::CoolingDown {
                    retry_after_seconds,
                }),
            };
            let encoded = postcard::to_allocvec(&outcome).unwrap();
            assert!(postcard::from_bytes::<MapPublishOutcome>(&encoded).is_err());
        }
    }

    #[test]
    fn published_recipe_ids_never_overlap_draft_or_preset_ids() {
        let id = PublishedMapId::new(7).unwrap();
        assert_eq!(
            id.recipe_id(),
            Some(crate::map::MapRecipeId(PUBLISHED_MAP_RECIPE_ID_BASE + 7))
        );
    }
}
