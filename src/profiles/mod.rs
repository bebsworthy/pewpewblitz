//! Durable logical-server-local player profiles and saved brawlers.

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
    AdvertisedBrawlerCatalog, AdvertisedBrawlerLimits, AdvertisedFighterProfile, AdvertisedPassive,
    AdvertisedUltimate, AdvertisedWeaponBase, BrawlerCatalogRevision,
    MAX_ADVERTISED_BRAWLER_CATALOG_BYTES, MAX_ADVERTISED_FIGHTER_PROFILES, MAX_ADVERTISED_PASSIVES,
    MAX_ADVERTISED_ULTIMATES, MAX_ADVERTISED_WEAPON_BASES,
};
pub use model::{
    AccountId, BrawlerDraft, BrawlerEdit, FighterProfileId, MAX_BRAWLERS_PER_PROFILE,
    MAX_PROFILE_SNAPSHOT_BYTES, MatchBuildSnapshotV3, ProfileCommand, ProfileDecision,
    ProfileModelError, ProfileOutcome, ProfileRevision, ProfileSnapshot, SavedBrawler,
    SavedBrawlerId, WeaponBaseId,
};
#[cfg(feature = "server")]
pub use storage::{
    MAX_PENDING_PUBLISHED_MAPS, MAX_PUBLISHED_MAP_REASON_BYTES, MAX_ROTATION_MAPS_PER_GAME_TYPE,
    MAX_STORED_PUBLISHED_MAPS, ProfileStorage, ProfileStorageCommand, ProfileStorageError,
    ProfileStorageExecutor, ProfileStorageReply, ProfileStorageResult, PublishedMapAction,
    PublishedMapLimits, PublishedMapRecord, PublishedMapStatus, PublishedMapSubmission,
    PublishedMapSubmissionOutcome, backup_database,
};

#[cfg(test)]
mod tests;
