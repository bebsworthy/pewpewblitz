use super::{
    AccountId, BrawlerDraft, BrawlerEdit, FighterProfileId, MAX_BRAWLERS_PER_PROFILE,
    ProfileModelError, ProfileRevision, ProfileSnapshot, SavedBrawler, SavedBrawlerId,
    WeaponBaseId,
};
use crate::builds::{PassiveDefinitionId, UltimateDefinitionId};
use rusqlite::{Connection, OptionalExtension as _, Transaction, params};
use std::{
    path::{Path, PathBuf},
    sync::{
        Mutex,
        mpsc::{Receiver, SyncSender, TrySendError, sync_channel},
    },
    thread::{self, JoinHandle},
};

const APPLICATION_ID: i32 = 0x4252_574c;
const SCHEMA_VERSION: i32 = 1;
const EXECUTOR_BOUND: usize = 64;

#[derive(Debug)]
pub enum ProfileStorageError {
    Database(String),
    InvalidData(ProfileModelError),
    StaleRevision,
    MissingBrawler,
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
    ) -> Result<ProfileSnapshot, ProfileStorageError> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO profiles(account_id, revision, next_brawler_ordinal) VALUES(?1, 1, 1) ON CONFLICT(account_id) DO NOTHING",
            [account_id.to_bytes().as_slice()],
        )?;
        let snapshot = load_profile(&transaction, account_id)?;
        transaction.commit()?;
        Ok(snapshot)
    }

    pub fn create_brawler(
        &mut self,
        account_id: AccountId,
        expected_profile_revision: ProfileRevision,
        brawler_id: SavedBrawlerId,
        draft: BrawlerDraft,
    ) -> Result<ProfileSnapshot, ProfileStorageError> {
        let draft = draft.normalized()?;
        let transaction = self.connection.transaction()?;
        let current = load_profile(&transaction, account_id)?;
        require_revision(&current, expected_profile_revision)?;
        if current.brawlers.len() >= MAX_BRAWLERS_PER_PROFILE {
            return Err(ProfileStorageError::CapacityReached);
        }
        let ordinal = current.next_brawler_ordinal;
        transaction.execute(
            "INSERT INTO brawlers(account_id,brawler_id,creation_ordinal,name,fighter_profile_id,weapon_base_id,ultimate_id,passive_1_id,passive_2_id,revision) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,1)",
            params![account_id.to_bytes().as_slice(), brawler_id.to_bytes().as_slice(), to_i64(ordinal)?, draft.name, draft.fighter_profile_id.0, draft.weapon_base_id.0, draft.ultimate_id.0, draft.passive_ids[0].0, draft.passive_ids[1].0],
        )?;
        if current.brawlers.is_empty() {
            transaction.execute(
                "INSERT INTO profile_selection(account_id,brawler_id) VALUES(?1,?2)",
                params![
                    account_id.to_bytes().as_slice(),
                    brawler_id.to_bytes().as_slice()
                ],
            )?;
        }
        advance_profile(
            &transaction,
            account_id,
            expected_profile_revision,
            ordinal
                .checked_add(1)
                .ok_or(ProfileModelError::InvalidCreationOrdinal)?,
        )?;
        let snapshot = load_profile(&transaction, account_id)?;
        transaction.commit()?;
        Ok(snapshot)
    }

    pub fn edit_brawler(
        &mut self,
        account_id: AccountId,
        expected_profile_revision: ProfileRevision,
        brawler_id: SavedBrawlerId,
        expected_brawler_revision: ProfileRevision,
        edit: BrawlerEdit,
    ) -> Result<ProfileSnapshot, ProfileStorageError> {
        let edit = edit.normalized()?;
        let transaction = self.connection.transaction()?;
        let current = load_profile(&transaction, account_id)?;
        require_revision(&current, expected_profile_revision)?;
        let changed = transaction.execute(
            "UPDATE brawlers SET name=?1,ultimate_id=?2,passive_1_id=?3,passive_2_id=?4,revision=revision+1 WHERE account_id=?5 AND brawler_id=?6 AND revision=?7",
            params![edit.name, edit.ultimate_id.0, edit.passive_ids[0].0, edit.passive_ids[1].0, account_id.to_bytes().as_slice(), brawler_id.to_bytes().as_slice(), to_i64(expected_brawler_revision.get())?],
        )?;
        if changed != 1 {
            return if current.brawlers.iter().any(|item| item.id == brawler_id) {
                Err(ProfileStorageError::StaleRevision)
            } else {
                Err(ProfileStorageError::MissingBrawler)
            };
        }
        advance_profile(
            &transaction,
            account_id,
            expected_profile_revision,
            current.next_brawler_ordinal,
        )?;
        let snapshot = load_profile(&transaction, account_id)?;
        transaction.commit()?;
        Ok(snapshot)
    }

    pub fn select_brawler(
        &mut self,
        account_id: AccountId,
        expected_profile_revision: ProfileRevision,
        brawler_id: SavedBrawlerId,
    ) -> Result<ProfileSnapshot, ProfileStorageError> {
        let transaction = self.connection.transaction()?;
        let current = load_profile(&transaction, account_id)?;
        require_revision(&current, expected_profile_revision)?;
        if !current.brawlers.iter().any(|item| item.id == brawler_id) {
            return Err(ProfileStorageError::MissingBrawler);
        }
        transaction.execute(
            "INSERT INTO profile_selection(account_id,brawler_id) VALUES(?1,?2) ON CONFLICT(account_id) DO UPDATE SET brawler_id=excluded.brawler_id",
            params![account_id.to_bytes().as_slice(), brawler_id.to_bytes().as_slice()],
        )?;
        advance_profile(
            &transaction,
            account_id,
            expected_profile_revision,
            current.next_brawler_ordinal,
        )?;
        let snapshot = load_profile(&transaction, account_id)?;
        transaction.commit()?;
        Ok(snapshot)
    }

    pub fn delete_brawler(
        &mut self,
        account_id: AccountId,
        expected_profile_revision: ProfileRevision,
        brawler_id: SavedBrawlerId,
        expected_brawler_revision: ProfileRevision,
    ) -> Result<ProfileSnapshot, ProfileStorageError> {
        let transaction = self.connection.transaction()?;
        let current = load_profile(&transaction, account_id)?;
        require_revision(&current, expected_profile_revision)?;
        let changed = transaction.execute(
            "DELETE FROM brawlers WHERE account_id=?1 AND brawler_id=?2 AND revision=?3",
            params![
                account_id.to_bytes().as_slice(),
                brawler_id.to_bytes().as_slice(),
                to_i64(expected_brawler_revision.get())?
            ],
        )?;
        if changed != 1 {
            return if current.brawlers.iter().any(|item| item.id == brawler_id) {
                Err(ProfileStorageError::StaleRevision)
            } else {
                Err(ProfileStorageError::MissingBrawler)
            };
        }
        if current.selected_brawler_id == Some(brawler_id) {
            transaction.execute(
                "DELETE FROM profile_selection WHERE account_id=?1",
                [account_id.to_bytes().as_slice()],
            )?;
            let fallback: Option<Vec<u8>> = transaction.query_row(
                "SELECT brawler_id FROM brawlers WHERE account_id=?1 ORDER BY creation_ordinal LIMIT 1",
                [account_id.to_bytes().as_slice()],
                |row| row.get(0),
            ).optional()?;
            if let Some(fallback) = fallback {
                transaction.execute(
                    "INSERT INTO profile_selection(account_id,brawler_id) VALUES(?1,?2)",
                    params![account_id.to_bytes().as_slice(), fallback],
                )?;
            }
        }
        advance_profile(
            &transaction,
            account_id,
            expected_profile_revision,
            current.next_brawler_ordinal,
        )?;
        let snapshot = load_profile(&transaction, account_id)?;
        transaction.commit()?;
        Ok(snapshot)
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
            "CREATE TABLE profiles(account_id BLOB PRIMARY KEY CHECK(length(account_id)=16),revision INTEGER NOT NULL CHECK(revision>0),next_brawler_ordinal INTEGER NOT NULL CHECK(next_brawler_ordinal>0));
             CREATE TABLE brawlers(account_id BLOB NOT NULL CHECK(length(account_id)=16),brawler_id BLOB NOT NULL CHECK(length(brawler_id)=16),creation_ordinal INTEGER NOT NULL CHECK(creation_ordinal>0),name TEXT NOT NULL CHECK(length(name)<=96),fighter_profile_id INTEGER NOT NULL,weapon_base_id INTEGER NOT NULL,ultimate_id INTEGER NOT NULL,passive_1_id INTEGER NOT NULL,passive_2_id INTEGER NOT NULL,revision INTEGER NOT NULL CHECK(revision>0),PRIMARY KEY(account_id,brawler_id),UNIQUE(account_id,creation_ordinal),FOREIGN KEY(account_id) REFERENCES profiles(account_id) ON DELETE CASCADE);
             CREATE TABLE profile_selection(account_id BLOB PRIMARY KEY CHECK(length(account_id)=16),brawler_id BLOB NOT NULL CHECK(length(brawler_id)=16),FOREIGN KEY(account_id,brawler_id) REFERENCES brawlers(account_id,brawler_id) ON DELETE CASCADE);
             PRAGMA application_id=1112692556;
             PRAGMA user_version=1;",
        )?;
        transaction.commit()?;
    }
    Ok(())
}

fn validate_database(connection: &Connection) -> Result<(), ProfileStorageError> {
    let integrity: String = connection.pragma_query_value(None, "quick_check", |row| row.get(0))?;
    if integrity != "ok" {
        return Err(ProfileStorageError::Database(format!(
            "SQLite quick check failed: {integrity}"
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
) -> Result<ProfileSnapshot, ProfileStorageError> {
    let (revision, next): (i64, i64) = transaction.query_row(
        "SELECT revision,next_brawler_ordinal FROM profiles WHERE account_id=?1",
        [account_id.to_bytes().as_slice()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let selected_bytes: Option<Vec<u8>> = transaction
        .query_row(
            "SELECT brawler_id FROM profile_selection WHERE account_id=?1",
            [account_id.to_bytes().as_slice()],
            |row| row.get(0),
        )
        .optional()?;
    let selected_brawler_id = selected_bytes.map(|bytes| decode_id(&bytes)).transpose()?;
    let mut statement = transaction.prepare(
        "SELECT brawler_id,creation_ordinal,name,fighter_profile_id,weapon_base_id,ultimate_id,passive_1_id,passive_2_id,revision FROM brawlers WHERE account_id=?1 ORDER BY creation_ordinal",
    )?;
    let rows = statement.query_map([account_id.to_bytes().as_slice()], |row| {
        Ok((
            row.get::<_, Vec<u8>>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, u16>(3)?,
            row.get::<_, u16>(4)?,
            row.get::<_, u16>(5)?,
            row.get::<_, u16>(6)?,
            row.get::<_, u16>(7)?,
            row.get::<_, i64>(8)?,
        ))
    })?;
    let mut brawlers = Vec::new();
    for row in rows {
        let (id, ordinal, name, fighter, weapon, ultimate, passive_1, passive_2, brawler_revision) =
            row?;
        brawlers.push(SavedBrawler {
            id: decode_id(&id)?,
            creation_ordinal: from_i64(ordinal)?,
            name,
            fighter_profile_id: FighterProfileId(fighter),
            weapon_base_id: WeaponBaseId(weapon),
            ultimate_id: UltimateDefinitionId(ultimate),
            passive_ids: [
                PassiveDefinitionId(passive_1),
                PassiveDefinitionId(passive_2),
            ],
            revision: ProfileRevision::new(from_i64(brawler_revision)?)?,
        });
    }
    drop(statement);
    let snapshot = ProfileSnapshot {
        account_id,
        revision: ProfileRevision::new(from_i64(revision)?)?,
        next_brawler_ordinal: from_i64(next)?,
        selected_brawler_id,
        brawlers,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

fn require_revision(
    snapshot: &ProfileSnapshot,
    expected: ProfileRevision,
) -> Result<(), ProfileStorageError> {
    if snapshot.revision == expected {
        Ok(())
    } else {
        Err(ProfileStorageError::StaleRevision)
    }
}

fn advance_profile(
    transaction: &Transaction<'_>,
    account_id: AccountId,
    expected: ProfileRevision,
    next: u64,
) -> Result<(), ProfileStorageError> {
    let changed = transaction.execute(
        "UPDATE profiles SET revision=revision+1,next_brawler_ordinal=?1 WHERE account_id=?2 AND revision=?3",
        params![to_i64(next)?, account_id.to_bytes().as_slice(), to_i64(expected.get())?],
    )?;
    if changed == 1 {
        Ok(())
    } else {
        Err(ProfileStorageError::StaleRevision)
    }
}

fn decode_id<T>(bytes: &[u8]) -> Result<T, ProfileStorageError>
where
    T: FromIdBytes,
{
    let bytes: [u8; 16] = bytes
        .try_into()
        .map_err(|_| ProfileModelError::MalformedId)?;
    T::from_id_bytes(bytes).map_err(Into::into)
}

trait FromIdBytes: Sized {
    fn from_id_bytes(bytes: [u8; 16]) -> Result<Self, ProfileModelError>;
}

impl FromIdBytes for SavedBrawlerId {
    fn from_id_bytes(bytes: [u8; 16]) -> Result<Self, ProfileModelError> {
        Self::from_bytes(bytes)
    }
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
    Create {
        request_id: u64,
        account_id: AccountId,
        expected_profile_revision: ProfileRevision,
        brawler_id: SavedBrawlerId,
        draft: BrawlerDraft,
    },
    Edit {
        request_id: u64,
        account_id: AccountId,
        expected_profile_revision: ProfileRevision,
        brawler_id: SavedBrawlerId,
        expected_brawler_revision: ProfileRevision,
        edit: BrawlerEdit,
    },
    Select {
        request_id: u64,
        account_id: AccountId,
        expected_profile_revision: ProfileRevision,
        brawler_id: SavedBrawlerId,
    },
    Delete {
        request_id: u64,
        account_id: AccountId,
        expected_profile_revision: ProfileRevision,
        brawler_id: SavedBrawlerId,
        expected_brawler_revision: ProfileRevision,
    },
    Shutdown,
}

pub struct ProfileStorageResult {
    pub request_id: u64,
    pub result: Result<ProfileSnapshot, ProfileStorageError>,
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
                        } => (request_id, storage.load_or_create(account_id)),
                        ProfileStorageCommand::Create {
                            request_id,
                            account_id,
                            expected_profile_revision,
                            brawler_id,
                            draft,
                        } => (
                            request_id,
                            storage.create_brawler(
                                account_id,
                                expected_profile_revision,
                                brawler_id,
                                draft,
                            ),
                        ),
                        ProfileStorageCommand::Edit {
                            request_id,
                            account_id,
                            expected_profile_revision,
                            brawler_id,
                            expected_brawler_revision,
                            edit,
                        } => (
                            request_id,
                            storage.edit_brawler(
                                account_id,
                                expected_profile_revision,
                                brawler_id,
                                expected_brawler_revision,
                                edit,
                            ),
                        ),
                        ProfileStorageCommand::Select {
                            request_id,
                            account_id,
                            expected_profile_revision,
                            brawler_id,
                        } => (
                            request_id,
                            storage.select_brawler(
                                account_id,
                                expected_profile_revision,
                                brawler_id,
                            ),
                        ),
                        ProfileStorageCommand::Delete {
                            request_id,
                            account_id,
                            expected_profile_revision,
                            brawler_id,
                            expected_brawler_revision,
                        } => (
                            request_id,
                            storage.delete_brawler(
                                account_id,
                                expected_profile_revision,
                                brawler_id,
                                expected_brawler_revision,
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
