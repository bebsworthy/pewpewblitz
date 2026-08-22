use super::{
    AccountId, MatchBuildSnapshotV2, ProfileCommand, ProfileDecision, ProfileOutcome,
    ProfileSnapshot, ProfileStorageCommand, ProfileStorageError, ProfileStorageExecutor,
    SavedBrawlerId,
};
use bevy::prelude::Resource;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug)]
struct AcceptedProfileSession {
    account_id: AccountId,
    snapshot: ProfileSnapshot,
    in_flight: bool,
    last_command: Option<ProfileCommand>,
    last_outcome: Option<ProfileOutcome>,
}

#[derive(Clone, Debug)]
enum PendingOperation {
    Load {
        client_key: u64,
        account_id: AccountId,
    },
    Mutation {
        client_key: u64,
        request_id: u64,
        command: ProfileCommand,
    },
}

#[derive(Resource)]
pub struct ProfileAuthority {
    storage: ProfileStorageExecutor,
    next_storage_request_id: u64,
    sessions: BTreeMap<u64, AcceptedProfileSession>,
    active_accounts: BTreeMap<AccountId, u64>,
    pending_accounts: BTreeMap<AccountId, u64>,
    pending: BTreeMap<u64, PendingOperation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileAuthorityError {
    AccountInUse,
    AlreadyPending,
    UnknownSession,
    QueueLocked,
    InvalidRequest,
    TemporarilyUnavailable,
    StorageStopped,
    IdentifierExhausted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileLoadCompletion {
    pub client_key: u64,
    pub account_id: AccountId,
    pub result: Result<ProfileSnapshot, ProfileDecision>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileMutationSubmission {
    Pending,
    Immediate(ProfileOutcome),
}

type ProfilePollResult = (Vec<ProfileLoadCompletion>, Vec<(u64, ProfileOutcome)>);

impl ProfileAuthority {
    pub fn start(path: PathBuf) -> Result<Self, ProfileStorageError> {
        Ok(Self {
            storage: ProfileStorageExecutor::start(path)?,
            next_storage_request_id: 1,
            sessions: BTreeMap::new(),
            active_accounts: BTreeMap::new(),
            pending_accounts: BTreeMap::new(),
            pending: BTreeMap::new(),
        })
    }

    pub fn begin_load(
        &mut self,
        client_key: u64,
        account_id: AccountId,
    ) -> Result<(), ProfileAuthorityError> {
        if self.sessions.contains_key(&client_key)
            || self
                .pending
                .values()
                .any(|pending| matches!(pending, PendingOperation::Load { client_key: pending_key, .. } if *pending_key == client_key))
        {
            return Err(ProfileAuthorityError::AlreadyPending);
        }
        if self.active_accounts.contains_key(&account_id)
            || self.pending_accounts.contains_key(&account_id)
        {
            return Err(ProfileAuthorityError::AccountInUse);
        }
        let storage_request_id = self.next_storage_request_id()?;
        self.storage
            .try_submit(ProfileStorageCommand::LoadOrCreate {
                request_id: storage_request_id,
                account_id,
            })
            .map_err(|error| map_submission_error(&error))?;
        self.pending_accounts.insert(account_id, client_key);
        self.pending.insert(
            storage_request_id,
            PendingOperation::Load {
                client_key,
                account_id,
            },
        );
        Ok(())
    }

    pub fn poll_loads(&mut self) -> Result<ProfilePollResult, ProfileAuthorityError> {
        let mut loads = Vec::new();
        let mut mutations = Vec::new();
        while let Some(completion) = self
            .storage
            .try_result()
            .map_err(|_| ProfileAuthorityError::StorageStopped)?
        {
            let Some(pending) = self.pending.remove(&completion.request_id) else {
                continue;
            };
            match pending {
                PendingOperation::Load {
                    client_key,
                    account_id,
                } => {
                    self.pending_accounts.remove(&account_id);
                    let result = completion.result.map_err(|error| storage_decision(&error));
                    if let Ok(snapshot) = &result {
                        self.active_accounts.insert(account_id, client_key);
                        self.sessions.insert(
                            client_key,
                            AcceptedProfileSession {
                                account_id,
                                snapshot: snapshot.clone(),
                                in_flight: false,
                                last_command: None,
                                last_outcome: None,
                            },
                        );
                    }
                    loads.push(ProfileLoadCompletion {
                        client_key,
                        account_id,
                        result,
                    });
                }
                PendingOperation::Mutation {
                    client_key,
                    request_id,
                    command,
                } => {
                    let decision = completion
                        .result
                        .as_ref()
                        .map_or_else(storage_decision, |_| ProfileDecision::Accepted);
                    let snapshot = completion.result.ok();
                    let outcome = ProfileOutcome {
                        request_id,
                        decision,
                        snapshot: snapshot.clone(),
                    };
                    if let Some(session) = self.sessions.get_mut(&client_key) {
                        session.in_flight = false;
                        if let Some(snapshot) = snapshot {
                            session.snapshot = snapshot;
                        }
                        session.last_command = Some(command);
                        session.last_outcome = Some(outcome.clone());
                    }
                    mutations.push((client_key, outcome));
                }
            }
        }
        Ok((loads, mutations))
    }

    pub fn submit_command(
        &mut self,
        client_key: u64,
        command: ProfileCommand,
        queue_locked: bool,
    ) -> Result<ProfileMutationSubmission, ProfileAuthorityError> {
        let session = self
            .sessions
            .get(&client_key)
            .ok_or(ProfileAuthorityError::UnknownSession)?;
        if queue_locked {
            return Ok(ProfileMutationSubmission::Immediate(rejected_outcome(
                &command,
                ProfileDecision::QueueLocked,
            )));
        }
        if command_request_id(&command) == 0 {
            return Ok(ProfileMutationSubmission::Immediate(rejected_outcome(
                &command,
                ProfileDecision::InvalidRequest,
            )));
        }
        if let Some(last) = session.last_command.as_ref()
            && command_request_id(&command) <= command_request_id(last)
        {
            return Ok(ProfileMutationSubmission::Immediate(if last == &command {
                session
                    .last_outcome
                    .clone()
                    .expect("remembered command owns an outcome")
            } else {
                rejected_outcome(&command, ProfileDecision::StaleRevision)
            }));
        }
        if session.in_flight {
            return Ok(ProfileMutationSubmission::Immediate(rejected_outcome(
                &command,
                ProfileDecision::TemporarilyUnavailable,
            )));
        }
        let account_id = session.account_id;
        let storage_request_id = self.next_storage_request_id()?;
        let storage_command = translate_command(account_id, storage_request_id, &command)?;
        self.storage
            .try_submit(storage_command)
            .map_err(|error| map_submission_error(&error))?;
        self.sessions
            .get_mut(&client_key)
            .expect("validated session remains installed")
            .in_flight = true;
        self.pending.insert(
            storage_request_id,
            PendingOperation::Mutation {
                client_key,
                request_id: command_request_id(&command),
                command,
            },
        );
        Ok(ProfileMutationSubmission::Pending)
    }

    #[must_use]
    pub fn snapshot(&self, client_key: u64) -> Option<&ProfileSnapshot> {
        self.sessions
            .get(&client_key)
            .map(|session| &session.snapshot)
    }

    #[must_use]
    pub fn mutation_in_flight(&self, client_key: u64) -> bool {
        self.sessions
            .get(&client_key)
            .is_some_and(|session| session.in_flight)
    }

    pub fn admitted_snapshot(
        &self,
        client_key: u64,
        expected_brawler_id: SavedBrawlerId,
        expected_brawler_revision: super::ProfileRevision,
        builds: &crate::builds::BuildCatalog,
        weapons: &crate::combat::WeaponCatalog,
        fighter: &crate::combat::FighterDefinition,
    ) -> Result<MatchBuildSnapshotV2, ProfileAuthorityError> {
        let session = self
            .sessions
            .get(&client_key)
            .ok_or(ProfileAuthorityError::UnknownSession)?;
        if session.in_flight {
            return Err(ProfileAuthorityError::TemporarilyUnavailable);
        }
        let brawler = session
            .snapshot
            .brawlers
            .iter()
            .find(|brawler| {
                brawler.id == expected_brawler_id
                    && brawler.revision == expected_brawler_revision
                    && session.snapshot.selected_brawler_id == Some(brawler.id)
            })
            .ok_or(ProfileAuthorityError::InvalidRequest)?;
        MatchBuildSnapshotV2::from_brawler(brawler, builds, weapons, fighter)
            .map_err(|_| ProfileAuthorityError::InvalidRequest)
    }

    pub fn remove_client(&mut self, client_key: u64) {
        if let Some(session) = self.sessions.remove(&client_key) {
            self.active_accounts.remove(&session.account_id);
        }
        self.pending_accounts
            .retain(|_, pending_client| *pending_client != client_key);
        self.pending.retain(|_, operation| match operation {
            PendingOperation::Load {
                client_key: pending_client,
                ..
            }
            | PendingOperation::Mutation {
                client_key: pending_client,
                ..
            } => *pending_client != client_key,
        });
    }

    fn next_storage_request_id(&mut self) -> Result<u64, ProfileAuthorityError> {
        let current = self.next_storage_request_id;
        if current == 0 {
            return Err(ProfileAuthorityError::IdentifierExhausted);
        }
        self.next_storage_request_id = current
            .checked_add(1)
            .ok_or(ProfileAuthorityError::IdentifierExhausted)?;
        Ok(current)
    }
}

fn command_request_id(command: &ProfileCommand) -> u64 {
    match command {
        ProfileCommand::CreateBrawler { request_id, .. }
        | ProfileCommand::EditBrawler { request_id, .. }
        | ProfileCommand::SelectBrawler { request_id, .. }
        | ProfileCommand::DeleteBrawler { request_id, .. } => *request_id,
    }
}

fn translate_command(
    account_id: AccountId,
    storage_request_id: u64,
    command: &ProfileCommand,
) -> Result<ProfileStorageCommand, ProfileAuthorityError> {
    Ok(match command {
        ProfileCommand::CreateBrawler {
            expected_profile_revision,
            draft,
            ..
        } => ProfileStorageCommand::Create {
            request_id: storage_request_id,
            account_id,
            expected_profile_revision: *expected_profile_revision,
            brawler_id: SavedBrawlerId::random()
                .map_err(|_| ProfileAuthorityError::IdentifierExhausted)?,
            draft: draft.clone(),
        },
        ProfileCommand::EditBrawler {
            expected_profile_revision,
            brawler_id,
            expected_brawler_revision,
            edit,
            ..
        } => ProfileStorageCommand::Edit {
            request_id: storage_request_id,
            account_id,
            expected_profile_revision: *expected_profile_revision,
            brawler_id: *brawler_id,
            expected_brawler_revision: *expected_brawler_revision,
            edit: edit.clone(),
        },
        ProfileCommand::SelectBrawler {
            expected_profile_revision,
            brawler_id,
            ..
        } => ProfileStorageCommand::Select {
            request_id: storage_request_id,
            account_id,
            expected_profile_revision: *expected_profile_revision,
            brawler_id: *brawler_id,
        },
        ProfileCommand::DeleteBrawler {
            expected_profile_revision,
            brawler_id,
            expected_brawler_revision,
            ..
        } => ProfileStorageCommand::Delete {
            request_id: storage_request_id,
            account_id,
            expected_profile_revision: *expected_profile_revision,
            brawler_id: *brawler_id,
            expected_brawler_revision: *expected_brawler_revision,
        },
    })
}

fn rejected_outcome(command: &ProfileCommand, decision: ProfileDecision) -> ProfileOutcome {
    ProfileOutcome {
        request_id: command_request_id(command),
        decision,
        snapshot: None,
    }
}

fn map_submission_error(error: &ProfileStorageError) -> ProfileAuthorityError {
    match error {
        ProfileStorageError::QueueFull => ProfileAuthorityError::TemporarilyUnavailable,
        _ => ProfileAuthorityError::StorageStopped,
    }
}

fn storage_decision(error: &ProfileStorageError) -> ProfileDecision {
    match error {
        ProfileStorageError::InvalidData(_) => ProfileDecision::InvalidRequest,
        ProfileStorageError::StaleRevision => ProfileDecision::StaleRevision,
        ProfileStorageError::MissingBrawler => ProfileDecision::MissingBrawler,
        ProfileStorageError::CapacityReached => ProfileDecision::CapacityReached,
        ProfileStorageError::QueueFull => ProfileDecision::TemporarilyUnavailable,
        ProfileStorageError::Database(_) | ProfileStorageError::ExecutorStopped => {
            ProfileDecision::StorageFault
        }
    }
}
