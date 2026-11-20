//! Durable logical-server-local player profiles: the selected roster character and the pick
//! remembered for each character.

#[cfg(feature = "server")]
mod authority;
mod catalog;
mod model;
#[cfg(feature = "server")]
mod storage;

#[cfg(feature = "server")]
pub use authority::{
    ProfileAuthority, ProfileAuthorityError, ProfileLoadCompletion, ProfileMutationSubmission,
    PublishedMapStorageCompletion,
};
pub use catalog::{
    AdvertisedCharacter, AdvertisedCharacterCatalog, AdvertisedPassive, AdvertisedUltimate,
    AdvertisedWeaponBase, CharacterCatalogRevision, MAX_ADVERTISED_CHARACTER_CATALOG_BYTES,
    MAX_ADVERTISED_CHARACTERS, MAX_ADVERTISED_PASSIVES, MAX_ADVERTISED_ULTIMATES,
    MAX_ADVERTISED_WEAPON_BASES,
};
pub use model::{
    AccountId, MAX_PROFILE_SNAPSHOT_BYTES, MatchCharacterSnapshot, ProfileCommand, ProfileDecision,
    ProfileModelError, ProfileOutcome, ProfileRevision, ProfileSnapshot,
};
#[cfg(feature = "server")]
pub use storage::{
    AccountPublishedMaps, MAX_PENDING_PUBLISHED_MAPS, MAX_PUBLISHED_MAP_REASON_BYTES,
    MAX_ROTATION_MAPS_PER_GAME_TYPE, MAX_STORED_PUBLISHED_MAPS, ProfileStorage,
    ProfileStorageCommand, ProfileStorageError, ProfileStorageExecutor, ProfileStorageReply,
    ProfileStorageResult, PublishedMapAction, PublishedMapLimits, PublishedMapRecord,
    PublishedMapStatus, PublishedMapSubmission, PublishedMapSubmissionOutcome,
    PublishedMapWithdrawal, StoredProfile, backup_database,
};

#[cfg(test)]
mod tests;
