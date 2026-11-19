//! Published Map Maker maps stored beside the profiles they belong to (BRL-w9z1, schema 3).
//!
//! The lobby's storage executor is the only in-process writer; `brawler-profile-admin maps`
//! opens the same WAL database for operator decisions. Server-wide capacity ceilings live here in
//! code; per-account policy arrives from the validated operator catalog.

use super::{ProfileStorage, ProfileStorageError, from_i64, to_i64};
use crate::lobby::{GameTypeId, MapPublishRejection, PublishedMapId, PublishedMapState};
use crate::profiles::AccountId;
use rusqlite::{OptionalExtension as _, Transaction, params};

/// Pending plus approved rows a server keeps at once.
pub const MAX_STORED_PUBLISHED_MAPS: u32 = 256;
/// Submissions awaiting an operator decision across every account.
pub const MAX_PENDING_PUBLISHED_MAPS: u32 = 64;
/// Approved maps one game type's queue rotation may hold.
pub const MAX_ROTATION_MAPS_PER_GAME_TYPE: u32 = 64;
/// Bound on an operator's rejection or removal reason.
pub const MAX_PUBLISHED_MAP_REASON_BYTES: usize = 256;

pub(super) fn create_schema_v3(transaction: &Transaction<'_>) -> Result<(), ProfileStorageError> {
    transaction.execute_batch(
        "CREATE TABLE published_maps(published_map_id INTEGER PRIMARY KEY AUTOINCREMENT CHECK(published_map_id>0),account_id BLOB NOT NULL CHECK(length(account_id)=16),author_display_name TEXT NOT NULL CHECK(length(author_display_name)<=64),name TEXT NOT NULL CHECK(length(name)<=96),game_type_id TEXT NOT NULL CHECK(length(game_type_id)<=32),game_type_revision INTEGER NOT NULL CHECK(game_type_revision>0),status INTEGER NOT NULL CHECK(status BETWEEN 0 AND 3),recipe BLOB NOT NULL CHECK(length(recipe)<=262144),submitted_at_unix_ms INTEGER NOT NULL CHECK(submitted_at_unix_ms>=0),decided_at_unix_ms INTEGER CHECK(decided_at_unix_ms IS NULL OR decided_at_unix_ms>=0),reason TEXT CHECK(reason IS NULL OR length(reason)<=256),FOREIGN KEY(account_id) REFERENCES profiles(account_id) ON DELETE CASCADE);
         CREATE INDEX published_maps_by_account ON published_maps(account_id,status);
         CREATE INDEX published_maps_by_status ON published_maps(status,decided_at_unix_ms,published_map_id);
         PRAGMA user_version=3;",
    )?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PublishedMapStatus {
    Pending,
    Approved,
    Rejected,
    Removed,
}

impl PublishedMapStatus {
    const fn code(self) -> i64 {
        match self {
            Self::Pending => 0,
            Self::Approved => 1,
            Self::Rejected => 2,
            Self::Removed => 3,
        }
    }

    fn from_code(code: i64) -> Result<Self, ProfileStorageError> {
        match code {
            0 => Ok(Self::Pending),
            1 => Ok(Self::Approved),
            2 => Ok(Self::Rejected),
            3 => Ok(Self::Removed),
            _ => Err(ProfileStorageError::Database(
                "unknown published map status".into(),
            )),
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Removed => "removed",
        }
    }

    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        [Self::Pending, Self::Approved, Self::Rejected, Self::Removed]
            .into_iter()
            .find(|status| status.label() == label)
    }
}

/// Per-account operator policy applied inside the submission transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublishedMapLimits {
    pub pending_per_account: u8,
    pub submit_cooldown_millis: u64,
    /// `Automatic` moderation stores a valid submission as approved instead of pending.
    pub approve_immediately: bool,
}

/// One already-validated submission. Storage assigns the identity and rewrites the recipe's
/// `recipe_id` and `revision` from it before encoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishedMapSubmission {
    pub account_id: AccountId,
    pub author_display_name: String,
    pub name: String,
    pub game_type_id: GameTypeId,
    pub game_type_revision: u32,
    pub recipe: crate::map::MapRecipe,
    pub now_unix_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishedMapSubmissionOutcome {
    Accepted {
        published_map_id: PublishedMapId,
        state: PublishedMapState,
    },
    Rejected(MapPublishRejection),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishedMapRecord {
    pub published_map_id: PublishedMapId,
    pub account_id: AccountId,
    pub author_display_name: String,
    pub name: String,
    pub game_type_id: GameTypeId,
    pub game_type_revision: u32,
    pub status: PublishedMapStatus,
    /// Canonical postcard bytes of the stored recipe, already bearing its published identity.
    pub recipe: Vec<u8>,
    pub submitted_at_unix_ms: u64,
    pub decided_at_unix_ms: Option<u64>,
    pub reason: Option<String>,
}

/// An operator decision on one stored row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublishedMapAction {
    Approve,
    Reject { reason: String },
    Remove { reason: Option<String> },
}

fn count(
    transaction: &Transaction<'_>,
    sql: &str,
    parameters: impl rusqlite::Params,
) -> Result<u32, ProfileStorageError> {
    let value: i64 = transaction.query_row(sql, parameters, |row| row.get(0))?;
    u32::try_from(value).map_err(|_| ProfileStorageError::Database("count overflow".into()))
}

fn validate_reason(reason: &str) -> Result<(), ProfileStorageError> {
    if reason.len() > MAX_PUBLISHED_MAP_REASON_BYTES || reason.chars().any(char::is_control) {
        return Err(ProfileStorageError::Database(format!(
            "reason must be at most {MAX_PUBLISHED_MAP_REASON_BYTES} bytes without control characters"
        )));
    }
    Ok(())
}

const RECORD_COLUMNS: &str = "published_map_id,account_id,author_display_name,name,game_type_id,game_type_revision,status,recipe,submitted_at_unix_ms,decided_at_unix_ms,reason";

fn record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawRecord> {
    Ok(RawRecord {
        published_map_id: row.get(0)?,
        account_id: row.get(1)?,
        author_display_name: row.get(2)?,
        name: row.get(3)?,
        game_type_id: row.get(4)?,
        game_type_revision: row.get(5)?,
        status: row.get(6)?,
        recipe: row.get(7)?,
        submitted_at_unix_ms: row.get(8)?,
        decided_at_unix_ms: row.get(9)?,
        reason: row.get(10)?,
    })
}

struct RawRecord {
    published_map_id: i64,
    account_id: Vec<u8>,
    author_display_name: String,
    name: String,
    game_type_id: String,
    game_type_revision: i64,
    status: i64,
    recipe: Vec<u8>,
    submitted_at_unix_ms: i64,
    decided_at_unix_ms: Option<i64>,
    reason: Option<String>,
}

impl RawRecord {
    fn decode(self) -> Result<PublishedMapRecord, ProfileStorageError> {
        let invalid =
            |what: &str| ProfileStorageError::Database(format!("invalid published map {what}"));
        let account_bytes: [u8; 16] = self
            .account_id
            .as_slice()
            .try_into()
            .map_err(|_| invalid("account"))?;
        Ok(PublishedMapRecord {
            published_map_id: PublishedMapId::new(from_i64(self.published_map_id)?)
                .ok_or_else(|| invalid("identity"))?,
            account_id: AccountId::from_bytes(account_bytes)?,
            author_display_name: self.author_display_name,
            name: self.name,
            game_type_id: GameTypeId::new(self.game_type_id).map_err(|_| invalid("game type"))?,
            game_type_revision: u32::try_from(self.game_type_revision)
                .map_err(|_| invalid("game-type revision"))?,
            status: PublishedMapStatus::from_code(self.status)?,
            recipe: self.recipe,
            submitted_at_unix_ms: from_i64(self.submitted_at_unix_ms)?,
            decided_at_unix_ms: self.decided_at_unix_ms.map(from_i64).transpose()?,
            reason: self.reason,
        })
    }
}

impl ProfileStorage {
    /// Apply quota, cooldown, and server capacity, then store one validated submission and
    /// assign its server-unique identity, all in one transaction.
    pub fn submit_published_map(
        &mut self,
        submission: PublishedMapSubmission,
        limits: PublishedMapLimits,
    ) -> Result<PublishedMapSubmissionOutcome, ProfileStorageError> {
        use PublishedMapSubmissionOutcome::Rejected;
        let transaction = self.connection.transaction()?;
        let account = submission.account_id.to_bytes();
        let now = to_i64(submission.now_unix_ms)?;
        let last: Option<i64> = transaction.query_row(
            "SELECT MAX(submitted_at_unix_ms) FROM published_maps WHERE account_id=?1",
            [account.as_slice()],
            |row| row.get(0),
        )?;
        if let Some(last) = last {
            let elapsed = u64::try_from(now.saturating_sub(last)).unwrap_or(0);
            if elapsed < limits.submit_cooldown_millis {
                let remaining = limits.submit_cooldown_millis - elapsed;
                let seconds = remaining
                    .div_ceil(1_000)
                    .clamp(1, u64::from(crate::lobby::MAX_PUBLISH_COOLDOWN_SECONDS));
                return Ok(Rejected(MapPublishRejection::CoolingDown {
                    retry_after_seconds: u16::try_from(seconds).unwrap_or(u16::MAX),
                }));
            }
        }
        let pending = PublishedMapStatus::Pending.code();
        let approved = PublishedMapStatus::Approved.code();
        if !limits.approve_immediately
            && count(
                &transaction,
                "SELECT COUNT(*) FROM published_maps WHERE account_id=?1 AND status=?2",
                params![account.as_slice(), pending],
            )? >= u32::from(limits.pending_per_account)
        {
            return Ok(Rejected(MapPublishRejection::PendingLimitReached));
        }
        let stored = count(
            &transaction,
            "SELECT COUNT(*) FROM published_maps WHERE status IN (?1,?2)",
            params![pending, approved],
        )?;
        let full = if limits.approve_immediately {
            count(
                &transaction,
                "SELECT COUNT(*) FROM published_maps WHERE status=?1 AND game_type_id=?2",
                params![approved, submission.game_type_id.as_str()],
            )? >= MAX_ROTATION_MAPS_PER_GAME_TYPE
        } else {
            count(
                &transaction,
                "SELECT COUNT(*) FROM published_maps WHERE status=?1",
                [pending],
            )? >= MAX_PENDING_PUBLISHED_MAPS
        };
        if full || stored >= MAX_STORED_PUBLISHED_MAPS {
            return Ok(Rejected(MapPublishRejection::StoreFull));
        }
        let (status, decided_at, state) = if limits.approve_immediately {
            (approved, Some(now), PublishedMapState::Approved)
        } else {
            (pending, None, PublishedMapState::Pending)
        };
        transaction.execute(
            "INSERT INTO published_maps(account_id,author_display_name,name,game_type_id,game_type_revision,status,recipe,submitted_at_unix_ms,decided_at_unix_ms) VALUES(?1,?2,?3,?4,?5,?6,X'',?7,?8)",
            params![
                account.as_slice(),
                submission.author_display_name,
                submission.name,
                submission.game_type_id.as_str(),
                i64::from(submission.game_type_revision),
                status,
                now,
                decided_at,
            ],
        )?;
        let published_map_id = PublishedMapId::new(from_i64(transaction.last_insert_rowid())?)
            .ok_or_else(|| ProfileStorageError::Database("zero published map ID".into()))?;
        let mut recipe = submission.recipe;
        recipe.recipe_id = published_map_id
            .recipe_id()
            .ok_or_else(|| ProfileStorageError::Database("published map ID overflow".into()))?;
        recipe.revision = crate::lobby::PUBLISHED_MAP_RECIPE_REVISION;
        let bytes = postcard::to_allocvec(&recipe)
            .map_err(|error| ProfileStorageError::Database(error.to_string()))?;
        transaction.execute(
            "UPDATE published_maps SET recipe=?1 WHERE published_map_id=?2",
            params![bytes, to_i64(published_map_id.get())?],
        )?;
        transaction.commit()?;
        Ok(PublishedMapSubmissionOutcome::Accepted {
            published_map_id,
            state,
        })
    }

    /// Approved maps in stable rotation order: decision time, then identity.
    pub fn load_published_rotation(
        &mut self,
    ) -> Result<Vec<PublishedMapRecord>, ProfileStorageError> {
        self.list_published_maps(Some(PublishedMapStatus::Approved))
    }

    pub fn list_published_maps(
        &mut self,
        status: Option<PublishedMapStatus>,
    ) -> Result<Vec<PublishedMapRecord>, ProfileStorageError> {
        let sql = format!(
            "SELECT {RECORD_COLUMNS} FROM published_maps WHERE ?1 IS NULL OR status=?1 ORDER BY status, COALESCE(decided_at_unix_ms, submitted_at_unix_ms), published_map_id"
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement
            .query_map([status.map(PublishedMapStatus::code)], record_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(RawRecord::decode).collect()
    }

    /// Apply one operator decision. Approve and reject act only on a pending row; remove acts on
    /// a pending or approved row. Returns the updated record.
    pub fn decide_published_map(
        &mut self,
        published_map_id: PublishedMapId,
        action: &PublishedMapAction,
        now_unix_ms: u64,
    ) -> Result<PublishedMapRecord, ProfileStorageError> {
        let transaction = self.connection.transaction()?;
        let id = to_i64(published_map_id.get())?;
        let current = transaction
            .query_row(
                &format!("SELECT {RECORD_COLUMNS} FROM published_maps WHERE published_map_id=?1"),
                [id],
                record_from_row,
            )
            .optional()?
            .ok_or_else(|| ProfileStorageError::Database("no such published map".into()))?
            .decode()?;
        let (allowed, next, reason) = match action {
            PublishedMapAction::Approve => (
                current.status == PublishedMapStatus::Pending,
                PublishedMapStatus::Approved,
                None,
            ),
            PublishedMapAction::Reject { reason } => {
                validate_reason(reason)?;
                (
                    current.status == PublishedMapStatus::Pending,
                    PublishedMapStatus::Rejected,
                    Some(reason.clone()),
                )
            }
            PublishedMapAction::Remove { reason } => {
                if let Some(reason) = reason {
                    validate_reason(reason)?;
                }
                (
                    matches!(
                        current.status,
                        PublishedMapStatus::Pending | PublishedMapStatus::Approved
                    ),
                    PublishedMapStatus::Removed,
                    reason.clone(),
                )
            }
        };
        if !allowed {
            return Err(ProfileStorageError::Database(format!(
                "published map {} is {}",
                published_map_id.get(),
                current.status.label()
            )));
        }
        if next == PublishedMapStatus::Approved
            && count(
                &transaction,
                "SELECT COUNT(*) FROM published_maps WHERE status=?1 AND game_type_id=?2",
                params![
                    PublishedMapStatus::Approved.code(),
                    current.game_type_id.as_str()
                ],
            )? >= MAX_ROTATION_MAPS_PER_GAME_TYPE
        {
            return Err(ProfileStorageError::CapacityReached);
        }
        transaction.execute(
            "UPDATE published_maps SET status=?1,decided_at_unix_ms=?2,reason=?3 WHERE published_map_id=?4",
            params![next.code(), to_i64(now_unix_ms)?, reason, id],
        )?;
        transaction.commit()?;
        Ok(PublishedMapRecord {
            status: next,
            decided_at_unix_ms: Some(now_unix_ms),
            reason,
            ..current
        })
    }
}
