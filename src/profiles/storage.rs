use super::{AccountId, ProfileModelError, ProfileRevision};
use crate::builds::{PassiveDefinitionId, UltimateDefinitionId};
use crate::characters::{CharacterChoice, CharacterId, MAX_CHARACTERS};
use rusqlite::{Connection, OptionalExtension as _, Transaction, params};
use std::{
    path::{Path, PathBuf},
    sync::{
        Mutex,
        mpsc::{Receiver, SyncSender, TrySendError, sync_channel},
    },
    thread::{self, JoinHandle},
};

mod published_maps;

pub use published_maps::{
    AccountPublishedMaps, MAX_PENDING_PUBLISHED_MAPS, MAX_PUBLISHED_MAP_REASON_BYTES,
    MAX_ROTATION_MAPS_PER_GAME_TYPE, MAX_STORED_PUBLISHED_MAPS, PublishedMapAction,
    PublishedMapLimits, PublishedMapRecord, PublishedMapStatus, PublishedMapSubmission,
    PublishedMapSubmissionOutcome, PublishedMapWithdrawal,
};

const APPLICATION_ID: i32 = 0x4252_574c;
/// Schema 4 (BRL-egn6): character picks replace saved brawlers and the part inventory.
const SCHEMA_VERSION: i32 = 5;
/// The first schema with published maps; older test databases fail closed.
const OLDEST_RETIRABLE_SCHEMA_VERSION: i32 = 3;
const EXECUTOR_BOUND: usize = 64;

const CREATE_CHARACTER_TABLES: &str = "CREATE TABLE character_choices(account_id BLOB NOT NULL CHECK(length(account_id)=16),character_id INTEGER NOT NULL CHECK(character_id>0),ultimate_id INTEGER NOT NULL CHECK(ultimate_id>0),passive_id INTEGER NOT NULL CHECK(passive_id>0),PRIMARY KEY(account_id,character_id),FOREIGN KEY(account_id) REFERENCES profiles(account_id) ON DELETE CASCADE);";

#[derive(Debug)]
pub enum ProfileStorageError {
    Database(String),
    InvalidData(ProfileModelError),
    StaleRevision,
    /// A published-map rotation is full (operator approval).
    CapacityReached,
    QueueFull,
    ExecutorStopped,
}

impl std::fmt::Display for ProfileStorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl From<rusqlite::Error> for ProfileStorageError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.to_string())
    }
}

impl From<ProfileModelError> for ProfileStorageError {
    fn from(error: ProfileModelError) -> Self {
        Self::InvalidData(error)
    }
}

/// One account's stored picks, before the authority normalizes them against the catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredProfile {
    pub account_id: AccountId,
    pub revision: ProfileRevision,
    pub selected_character_id: Option<CharacterId>,
    /// Ascending by character.
    pub choices: Vec<CharacterChoice>,
}

pub struct ProfileStorage {
    connection: Connection,
}

impl ProfileStorage {
    pub fn open(path: &Path) -> Result<Self, ProfileStorageError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| ProfileStorageError::Database(error.to_string()))?;
        }
        let mut connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.busy_timeout(std::time::Duration::from_secs(2))?;
        migrate(&mut connection)?;
        validate_database(&connection)?;
        Ok(Self { connection })
    }

    pub fn load_or_create(
        &mut self,
        account_id: AccountId,
    ) -> Result<StoredProfile, ProfileStorageError> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO profiles(account_id, revision) VALUES(?1, 1) ON CONFLICT(account_id) DO NOTHING",
            [account_id.to_bytes().as_slice()],
        )?;
        let profile = load_profile(&transaction, account_id)?;
        transaction.commit()?;
        Ok(profile)
    }

    /// Select `choice` and remember it for its character. The caller validated it against the
    /// character catalog; storage keeps only the structural bounds.
    pub fn select_character(
        &mut self,
        account_id: AccountId,
        expected_profile_revision: ProfileRevision,
        choice: CharacterChoice,
    ) -> Result<StoredProfile, ProfileStorageError> {
        if choice.character_id.0 == 0 || choice.ultimate_id.0 == 0 || choice.passive_id.0 == 0 {
            return Err(ProfileModelError::InvalidChoice.into());
        }
        let transaction = self.connection.transaction()?;
        let current = load_profile(&transaction, account_id)?;
        if current.revision != expected_profile_revision {
            return Err(ProfileStorageError::StaleRevision);
        }
        if current.choices.len() >= MAX_CHARACTERS
            && !current
                .choices
                .iter()
                .any(|stored| stored.character_id == choice.character_id)
        {
            return Err(ProfileModelError::TooManyChoices.into());
        }
        transaction.execute(
            "INSERT INTO character_choices(account_id,character_id,ultimate_id,passive_id) VALUES(?1,?2,?3,?4) ON CONFLICT(account_id,character_id) DO UPDATE SET ultimate_id=excluded.ultimate_id,passive_id=excluded.passive_id",
            params![account_id.to_bytes().as_slice(), choice.character_id.0, choice.ultimate_id.0, choice.passive_id.0],
        )?;
        let changed = transaction.execute(
            "UPDATE profiles SET revision=revision+1,selected_character_id=?1 WHERE account_id=?2 AND revision=?3",
            params![choice.character_id.0, account_id.to_bytes().as_slice(), to_i64(expected_profile_revision.get())?],
        )?;
        if changed != 1 {
            return Err(ProfileStorageError::StaleRevision);
        }
        let profile = load_profile(&transaction, account_id)?;
        transaction.commit()?;
        Ok(profile)
    }
}

fn migrate(connection: &mut Connection) -> Result<(), ProfileStorageError> {
    let application_id: i32 =
        connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
    let version: i32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if application_id != 0 && application_id != APPLICATION_ID {
        return Err(ProfileStorageError::Database(
            "unexpected SQLite application ID".into(),
        ));
    }
    if version > SCHEMA_VERSION {
        return Err(ProfileStorageError::Database(
            "profile schema is newer than this binary".into(),
        ));
    }
    if version == 0 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "CREATE TABLE profiles(account_id BLOB PRIMARY KEY CHECK(length(account_id)=16),revision INTEGER NOT NULL CHECK(revision>0),selected_character_id INTEGER CHECK(selected_character_id IS NULL OR selected_character_id>0));
             PRAGMA application_id=1112692556;",
        )?;
        transaction.execute_batch(CREATE_CHARACTER_TABLES)?;
        published_maps::create_published_map_schema(&transaction)?;
        transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        transaction.commit()?;
        return Ok(());
    }
    if version < OLDEST_RETIRABLE_SCHEMA_VERSION {
        return Err(ProfileStorageError::Database(
            "profile schema predates published maps; delete this test database".into(),
        ));
    }
    if version == OLDEST_RETIRABLE_SCHEMA_VERSION {
        // Saved brawlers and the part inventory are retired without migration (local test data):
        // account rows and published maps stay; every account starts at the default pick.
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "DROP TABLE brawler_part_slots;
             DROP TABLE profile_selection;
             DROP TABLE weapon_part_instances;
             DROP TABLE brawlers;
             ALTER TABLE profiles DROP COLUMN starter_part_set;
             ALTER TABLE profiles DROP COLUMN next_brawler_ordinal;
             ALTER TABLE profiles ADD COLUMN selected_character_id INTEGER CHECK(selected_character_id IS NULL OR selected_character_id>0);",
        )?;
        transaction.execute_batch(CREATE_CHARACTER_TABLES)?;
        transaction.pragma_update(None, "user_version", 4)?;
        transaction.commit()?;
    }
    let version: i32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version == 4 {
        // BRL-wg28: one-time cutover of published maps to the renamed game-type IDs.
        let transaction = connection.transaction()?;
        transaction.execute_batch(RENAME_GAME_TYPE_IDS)?;
        transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        transaction.commit()?;
    }
    Ok(())
}

/// Schema 5 (BRL-wg28) renames the stored game-type IDs of published maps in place.
const RENAME_GAME_TYPE_IDS: &str = "UPDATE published_maps SET game_type_id='takedown-'||substr(game_type_id,9) WHERE game_type_id LIKE 'wipeout-%';
UPDATE published_maps SET game_type_id='turf-hold-'||substr(game_type_id,10) WHERE game_type_id LIKE 'hot-zone-%';
UPDATE published_maps SET game_type_id='vault-break-'||substr(game_type_id,7) WHERE game_type_id LIKE 'heist-%';";

fn validate_database(connection: &Connection) -> Result<(), ProfileStorageError> {
    let application_id: i32 =
        connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
    if application_id != APPLICATION_ID {
        return Err(ProfileStorageError::Database(
            "unexpected SQLite application ID".into(),
        ));
    }
    let version: i32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version != SCHEMA_VERSION {
        return Err(ProfileStorageError::Database(
            "unexpected profile schema version".into(),
        ));
    }
    let integrity: String =
        connection.pragma_query_value(None, "integrity_check", |row| row.get(0))?;
    if integrity != "ok" {
        return Err(ProfileStorageError::Database(format!(
            "SQLite integrity check failed: {integrity}"
        )));
    }
    let foreign_key_fault: Option<i64> = connection
        .query_row(
            "SELECT rowid FROM pragma_foreign_key_check LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if foreign_key_fault.is_some() {
        return Err(ProfileStorageError::Database(
            "SQLite foreign key check failed".into(),
        ));
    }
    Ok(())
}

fn load_profile(
    transaction: &Transaction<'_>,
    account_id: AccountId,
) -> Result<StoredProfile, ProfileStorageError> {
    let (revision, selected): (i64, Option<u16>) = transaction.query_row(
        "SELECT revision,selected_character_id FROM profiles WHERE account_id=?1",
        [account_id.to_bytes().as_slice()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let mut statement = transaction.prepare(
        "SELECT character_id,ultimate_id,passive_id FROM character_choices WHERE account_id=?1 ORDER BY character_id LIMIT ?2",
    )?;
    let rows = statement.query_map(
        params![
            account_id.to_bytes().as_slice(),
            to_i64(MAX_CHARACTERS as u64)?
        ],
        |row| {
            Ok(CharacterChoice {
                character_id: CharacterId(row.get(0)?),
                ultimate_id: UltimateDefinitionId(row.get(1)?),
                passive_id: PassiveDefinitionId(row.get(2)?),
            })
        },
    )?;
    let choices = rows.collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    Ok(StoredProfile {
        account_id,
        revision: ProfileRevision::new(from_i64(revision)?)?,
        selected_character_id: selected.map(CharacterId),
        choices,
    })
}

fn to_i64(value: u64) -> Result<i64, ProfileStorageError> {
    i64::try_from(value).map_err(|_| ProfileModelError::InvalidRevision.into())
}

fn from_i64(value: i64) -> Result<u64, ProfileStorageError> {
    u64::try_from(value).map_err(|_| ProfileModelError::InvalidRevision.into())
}

pub enum ProfileStorageCommand {
    LoadOrCreate {
        request_id: u64,
        account_id: AccountId,
    },
    SelectCharacter {
        request_id: u64,
        account_id: AccountId,
        expected_profile_revision: ProfileRevision,
        choice: CharacterChoice,
    },
    SubmitPublishedMap {
        request_id: u64,
        submission: Box<PublishedMapSubmission>,
        limits: PublishedMapLimits,
    },
    LoadPublishedRotation {
        request_id: u64,
    },
    /// One account's own rows, after an optional author withdrawal (BRL-s3dh).
    AccountPublishedMaps {
        request_id: u64,
        account_id: AccountId,
        withdraw: Option<crate::lobby::PublishedMapId>,
        now_unix_ms: u64,
    },
    Shutdown,
}

/// One storage-owner completion; the variant matches the command family that produced it.
pub enum ProfileStorageReply {
    Profile(Result<StoredProfile, ProfileStorageError>),
    PublishedMapSubmission(Result<PublishedMapSubmissionOutcome, ProfileStorageError>),
    PublishedRotation(Result<Vec<PublishedMapRecord>, ProfileStorageError>),
    AccountPublishedMaps(Result<AccountPublishedMaps, ProfileStorageError>),
}

pub struct ProfileStorageResult {
    pub request_id: u64,
    pub result: ProfileStorageReply,
}

pub struct ProfileStorageExecutor {
    commands: SyncSender<ProfileStorageCommand>,
    results: Mutex<Receiver<ProfileStorageResult>>,
    join: Option<JoinHandle<()>>,
}

impl ProfileStorageExecutor {
    pub fn start(path: PathBuf) -> Result<Self, ProfileStorageError> {
        let (commands, command_rx) = sync_channel(EXECUTOR_BOUND);
        let (result_tx, results) = sync_channel(EXECUTOR_BOUND);
        let (ready_tx, ready_rx) = sync_channel(1);
        let join = thread::Builder::new()
            .name("brawler-profile-storage".into())
            .spawn(move || {
                let mut storage = match ProfileStorage::open(&path) {
                    Ok(storage) => {
                        let _ = ready_tx.send(Ok(()));
                        storage
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };
                while let Ok(command) = command_rx.recv() {
                    let (request_id, result) = match command {
                        ProfileStorageCommand::LoadOrCreate {
                            request_id,
                            account_id,
                        } => (
                            request_id,
                            ProfileStorageReply::Profile(storage.load_or_create(account_id)),
                        ),
                        ProfileStorageCommand::SelectCharacter {
                            request_id,
                            account_id,
                            expected_profile_revision,
                            choice,
                        } => (
                            request_id,
                            ProfileStorageReply::Profile(storage.select_character(
                                account_id,
                                expected_profile_revision,
                                choice,
                            )),
                        ),
                        ProfileStorageCommand::SubmitPublishedMap {
                            request_id,
                            submission,
                            limits,
                        } => (
                            request_id,
                            ProfileStorageReply::PublishedMapSubmission(
                                storage.submit_published_map(*submission, limits),
                            ),
                        ),
                        ProfileStorageCommand::LoadPublishedRotation { request_id } => (
                            request_id,
                            ProfileStorageReply::PublishedRotation(
                                storage.load_published_rotation(),
                            ),
                        ),
                        ProfileStorageCommand::AccountPublishedMaps {
                            request_id,
                            account_id,
                            withdraw,
                            now_unix_ms,
                        } => (
                            request_id,
                            ProfileStorageReply::AccountPublishedMaps(
                                storage.account_published_maps(account_id, withdraw, now_unix_ms),
                            ),
                        ),
                        ProfileStorageCommand::Shutdown => break,
                    };
                    if result_tx
                        .send(ProfileStorageResult { request_id, result })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .map_err(|error| ProfileStorageError::Database(error.to_string()))?;
        ready_rx
            .recv()
            .map_err(|_| ProfileStorageError::ExecutorStopped)??;
        Ok(Self {
            commands,
            results: Mutex::new(results),
            join: Some(join),
        })
    }

    pub fn try_submit(&self, command: ProfileStorageCommand) -> Result<(), ProfileStorageError> {
        self.commands
            .try_send(command)
            .map_err(|error| match error {
                TrySendError::Full(_) => ProfileStorageError::QueueFull,
                TrySendError::Disconnected(_) => ProfileStorageError::ExecutorStopped,
            })
    }

    pub fn try_result(&self) -> Result<Option<ProfileStorageResult>, ProfileStorageError> {
        let results = self
            .results
            .lock()
            .map_err(|_| ProfileStorageError::ExecutorStopped)?;
        match results.try_recv() {
            Ok(result) => Ok(Some(result)),
            Err(std::sync::mpsc::TryRecvError::Empty) => Ok(None),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Err(ProfileStorageError::ExecutorStopped)
            }
        }
    }
}

impl Drop for ProfileStorageExecutor {
    fn drop(&mut self) {
        let mut shutdown = ProfileStorageCommand::Shutdown;
        loop {
            match self.commands.try_send(shutdown) {
                Ok(()) | Err(TrySendError::Disconnected(_)) => break,
                Err(TrySendError::Full(command)) => {
                    shutdown = command;
                    if let Ok(results) = self.results.lock() {
                        while results.try_recv().is_ok() {}
                    }
                    thread::yield_now();
                }
            }
        }
        if let Some(join) = self.join.take() {
            while !join.is_finished() {
                if let Ok(results) = self.results.lock() {
                    while results.try_recv().is_ok() {}
                }
                thread::yield_now();
            }
            let _ = join.join();
        }
    }
}

pub fn backup_database(database: &Path, output: &Path) -> Result<(), ProfileStorageError> {
    if output.exists() {
        return Err(ProfileStorageError::Database(
            "backup output already exists".into(),
        ));
    }
    let source = ProfileStorage::open(database)?;
    let parent = output
        .parent()
        .ok_or_else(|| ProfileStorageError::Database("backup output has no parent".into()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| ProfileStorageError::Database(error.to_string()))?;
    let temporary = output.with_extension("tmp");
    if temporary.exists() {
        return Err(ProfileStorageError::Database(
            "backup temporary path already exists".into(),
        ));
    }
    let mut destination = Connection::open(&temporary)?;
    {
        let backup = rusqlite::backup::Backup::new(&source.connection, &mut destination)?;
        backup.run_to_completion(64, std::time::Duration::from_millis(10), None)?;
    }
    validate_database(&destination)?;
    drop(destination);
    std::fs::rename(&temporary, output)
        .map_err(|error| ProfileStorageError::Database(error.to_string()))
}
