use super::{
    AccountId, AccountPublishedMaps, AdvertisedCharacterCatalog, MatchCharacterSnapshot,
    ProfileCommand, ProfileDecision, ProfileOutcome, ProfileSnapshot, ProfileStorageCommand,
    ProfileStorageError, ProfileStorageExecutor, ProfileStorageReply, PublishedMapLimits,
    PublishedMapRecord, PublishedMapSubmission, PublishedMapSubmissionOutcome, StoredProfile,
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
        command: ProfileCommand,
    },
    PublishedMap,
}

/// A published-map storage completion, keyed by the storage request ID its submission returned.
/// The lobby's publishing owner interprets it; profile sessions never observe it.
#[derive(Debug)]
pub enum PublishedMapStorageCompletion {
    Submission {
        storage_request_id: u64,
        result: Result<PublishedMapSubmissionOutcome, ProfileStorageError>,
    },
    Rotation {
        storage_request_id: u64,
        result: Result<Vec<PublishedMapRecord>, ProfileStorageError>,
    },
    /// A "my published maps" list or withdrawal for one account (BRL-s3dh).
    Account {
        storage_request_id: u64,
        result: Result<AccountPublishedMaps, ProfileStorageError>,
    },
}

#[derive(Resource)]
pub struct ProfileAuthority {
    storage: ProfileStorageExecutor,
    catalog: AdvertisedCharacterCatalog,
    next_storage_request_id: u64,
    sessions: BTreeMap<u64, AcceptedProfileSession>,
    active_accounts: BTreeMap<AccountId, u64>,
    pending_accounts: BTreeMap<AccountId, u64>,
    pending: BTreeMap<u64, PendingOperation>,
    published_map_completions: Vec<PublishedMapStorageCompletion>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileAuthorityError {
    AccountInUse,
    AlreadyPending,
    UnknownSession,
    QueueLocked,
    InvalidRequest,
    IncompatibleBuild,
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
        let catalog =
            AdvertisedCharacterCatalog::embedded().map_err(ProfileStorageError::Database)?;
        Self::start_with_catalog(path, catalog)
    }

    pub fn start_with_catalog(
        path: PathBuf,
        catalog: AdvertisedCharacterCatalog,
    ) -> Result<Self, ProfileStorageError> {
        catalog.validate().map_err(ProfileStorageError::Database)?;
        Ok(Self {
            storage: ProfileStorageExecutor::start(path)?,
            catalog,
            next_storage_request_id: 1,
            sessions: BTreeMap::new(),
            active_accounts: BTreeMap::new(),
            pending_accounts: BTreeMap::new(),
            pending: BTreeMap::new(),
            published_map_completions: Vec::new(),
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
            let Some(completion_result) =
                self.take_profile_reply(completion.request_id, completion.result)
            else {
                continue;
            };
            let completion_result = completion_result.map(|stored| self.normalize(stored));
            match pending {
                PendingOperation::PublishedMap => {}
                PendingOperation::Load {
                    client_key,
                    account_id,
                } => {
                    self.pending_accounts.remove(&account_id);
                    let result = completion_result.map_err(|error| storage_decision(&error));
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
                    command,
                } => {
                    let decision = completion_result
                        .as_ref()
                        .map_or_else(storage_decision, |_| ProfileDecision::Accepted);
                    let snapshot = completion_result.ok();
                    let outcome = ProfileOutcome {
                        request_id: command.request_id,
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

    /// The wire profile for stored rows, normalized against the advertised roster.
    fn normalize(&self, stored: StoredProfile) -> ProfileSnapshot {
        self.catalog.profile_from_stored(
            stored.account_id,
            stored.revision,
            stored.selected_character_id,
            stored.choices,
        )
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
        if command.request_id == 0 || !self.catalog.accepts(command.choice) {
            return Ok(ProfileMutationSubmission::Immediate(rejected_outcome(
                &command,
                ProfileDecision::InvalidRequest,
            )));
        }
        if let Some(last) = session.last_command.as_ref()
            && command.request_id <= last.request_id
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
        self.storage
            .try_submit(ProfileStorageCommand::SelectCharacter {
                request_id: storage_request_id,
                account_id,
                expected_profile_revision: command.expected_profile_revision,
                choice: command.choice,
            })
            .map_err(|error| map_submission_error(&error))?;
        self.sessions
            .get_mut(&client_key)
            .expect("validated session remains installed")
            .in_flight = true;
        self.pending.insert(
            storage_request_id,
            PendingOperation::Mutation {
                client_key,
                command,
            },
        );
        Ok(ProfileMutationSubmission::Pending)
    }

    /// A profile reply for profile sessions; a published-map reply is set aside for the lobby.
    fn take_profile_reply(
        &mut self,
        storage_request_id: u64,
        reply: ProfileStorageReply,
    ) -> Option<Result<StoredProfile, ProfileStorageError>> {
        let completion = match reply {
            ProfileStorageReply::Profile(result) => return Some(result),
            ProfileStorageReply::PublishedMapSubmission(result) => {
                PublishedMapStorageCompletion::Submission {
                    storage_request_id,
                    result,
                }
            }
            ProfileStorageReply::PublishedRotation(result) => {
                PublishedMapStorageCompletion::Rotation {
                    storage_request_id,
                    result,
                }
            }
            ProfileStorageReply::AccountPublishedMaps(result) => {
                PublishedMapStorageCompletion::Account {
                    storage_request_id,
                    result,
                }
            }
        };
        self.published_map_completions.push(completion);
        None
    }

    /// Queue one validated published-map submission on the storage owner (BRL-w9z1). Returns
    /// the storage request ID its `PublishedMapStorageCompletion` will carry.
    pub fn submit_published_map(
        &mut self,
        submission: PublishedMapSubmission,
        limits: PublishedMapLimits,
    ) -> Result<u64, ProfileAuthorityError> {
        let storage_request_id = self.next_storage_request_id()?;
        self.storage
            .try_submit(ProfileStorageCommand::SubmitPublishedMap {
                request_id: storage_request_id,
                submission: Box::new(submission),
                limits,
            })
            .map_err(|error| map_submission_error(&error))?;
        self.pending
            .insert(storage_request_id, PendingOperation::PublishedMap);
        Ok(storage_request_id)
    }

    /// Queue a read of the approved published maps in rotation order.
    pub fn request_published_rotation(&mut self) -> Result<u64, ProfileAuthorityError> {
        let storage_request_id = self.next_storage_request_id()?;
        self.storage
            .try_submit(ProfileStorageCommand::LoadPublishedRotation {
                request_id: storage_request_id,
            })
            .map_err(|error| map_submission_error(&error))?;
        self.pending
            .insert(storage_request_id, PendingOperation::PublishedMap);
        Ok(storage_request_id)
    }

    /// Queue one account's own published-map listing, withdrawing `withdraw` first when given
    /// (BRL-s3dh). The caller passes the session's own account; storage scopes every query to it.
    pub fn request_account_published_maps(
        &mut self,
        account_id: AccountId,
        withdraw: Option<crate::lobby::PublishedMapId>,
        now_unix_ms: u64,
    ) -> Result<u64, ProfileAuthorityError> {
        let storage_request_id = self.next_storage_request_id()?;
        self.storage
            .try_submit(ProfileStorageCommand::AccountPublishedMaps {
                request_id: storage_request_id,
                account_id,
                withdraw,
                now_unix_ms,
            })
            .map_err(|error| map_submission_error(&error))?;
        self.pending
            .insert(storage_request_id, PendingOperation::PublishedMap);
        Ok(storage_request_id)
    }

    /// Published-map completions collected by the last `poll_loads`.
    pub fn take_published_map_completions(&mut self) -> Vec<PublishedMapStorageCompletion> {
        std::mem::take(&mut self.published_map_completions)
    }

    /// The client key whose accepted or loading profile session currently holds `account_id`.
    #[must_use]
    pub fn account_holder(&self, account_id: AccountId) -> Option<u64> {
        self.active_accounts
            .get(&account_id)
            .or_else(|| self.pending_accounts.get(&account_id))
            .copied()
    }

    /// The account of an accepted profile session.
    #[must_use]
    pub fn account_for_client(&self, client_key: u64) -> Option<AccountId> {
        self.sessions
            .get(&client_key)
            .map(|session| session.account_id)
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

    /// The selected pick of a settled profile at the revision the client saw, as the immutable
    /// snapshot routing carries into the match.
    pub fn admitted_snapshot(
        &self,
        client_key: u64,
        expected_profile_revision: super::ProfileRevision,
        catalogs: crate::characters::LoadoutCatalogs<'_>,
    ) -> Result<MatchCharacterSnapshot, ProfileAuthorityError> {
        let session = self
            .sessions
            .get(&client_key)
            .ok_or(ProfileAuthorityError::UnknownSession)?;
        if session.in_flight {
            return Err(ProfileAuthorityError::TemporarilyUnavailable);
        }
        if session.snapshot.revision != expected_profile_revision {
            return Err(ProfileAuthorityError::InvalidRequest);
        }
        MatchCharacterSnapshot::from_choice(session.snapshot.selection(), catalogs)
            .map_err(|_| ProfileAuthorityError::IncompatibleBuild)
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
            PendingOperation::PublishedMap => true,
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

const fn rejected_outcome(command: &ProfileCommand, decision: ProfileDecision) -> ProfileOutcome {
    ProfileOutcome {
        request_id: command.request_id,
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
        ProfileStorageError::InvalidData(_) | ProfileStorageError::CapacityReached => {
            ProfileDecision::InvalidRequest
        }
        ProfileStorageError::StaleRevision => ProfileDecision::StaleRevision,
        ProfileStorageError::QueueFull => ProfileDecision::TemporarilyUnavailable,
        ProfileStorageError::Database(_) | ProfileStorageError::ExecutorStopped => {
            ProfileDecision::StorageFault
        }
    }
}
