//! Durable logical-server-local player profiles and saved brawlers.

#[cfg(feature = "server")]
mod authority;
mod model;
#[cfg(feature = "server")]
mod storage;

#[cfg(feature = "server")]
pub use authority::{
    ProfileAuthority, ProfileAuthorityError, ProfileLoadCompletion, ProfileMutationSubmission,
};
pub use model::{
    AccountId, BrawlerDraft, BrawlerEdit, FighterProfileId, MAX_BRAWLERS_PER_PROFILE,
    MAX_PROFILE_SNAPSHOT_BYTES, MatchBuildSnapshotV2, ProfileCommand, ProfileDecision,
    ProfileModelError, ProfileOutcome, ProfileRevision, ProfileSnapshot, SavedBrawler,
    SavedBrawlerId, WeaponBaseId,
};
#[cfg(feature = "server")]
pub use storage::{
    ProfileStorage, ProfileStorageCommand, ProfileStorageError, ProfileStorageExecutor,
    ProfileStorageResult, backup_database,
};

#[cfg(test)]
mod tests;
