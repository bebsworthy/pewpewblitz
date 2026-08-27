//! Connection-attempt state, deadlines, DNS resolution, and candidate spawning.

use super::{
    ClientNetworkConfig, LogicalServerAddress, MAX_RESOLVED_CANDIDATES, ProductLobbyAttempt,
    RoutedClientLifecycle, ServerAddressHost, SessionObservation, spawn_product_lobby_connection,
};
use crate::client::server_select::parse_server_address;
use bevy::prelude::{Commands, Entity, Resource};
use bevy::tasks::{IoTaskPool, Task};
use std::{
    collections::BTreeSet,
    net::{SocketAddr, ToSocketAddrs as _},
    time::Duration,
};

const DNS_DEADLINE: Duration = Duration::from_secs(5);
const ATTEMPT_DEADLINE: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ValidatedConnectionTarget {
    pub(super) logical_address: LogicalServerAddress,
    pub(super) proposed_display_name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ConnectionStage {
    ResolvingAddress,
    ContactingServer { current: usize, total: usize },
    JoiningLobby,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AttemptDeadlineExpiry {
    Dns,
    Overall,
    Candidate,
}

#[derive(Resource, Clone, Debug)]
pub(super) struct PendingConnection {
    pub(super) generation: u64,
    pub(super) target: ValidatedConnectionTarget,
    pub(super) candidates: Vec<SocketAddr>,
    pub(super) current_candidate: usize,
    pub(super) overall_deadline: Duration,
    pub(super) dns_deadline: Option<Duration>,
    pub(super) candidate_deadline: Option<Duration>,
    pub(super) current_entity: Option<Entity>,
    pub(super) stage: ConnectionStage,
}

#[derive(Resource, Default)]
pub(super) struct ConnectionGeneration(pub(super) u64);

pub(super) struct ResolverTask {
    pub(super) generation: u64,
    pub(super) task: Task<Result<Vec<SocketAddr>, String>>,
}

#[derive(Resource, Default)]
pub(super) struct ResolverState {
    pub(super) task: Option<ResolverTask>,
}

pub(super) fn validate_target(
    address: &str,
    name: &str,
) -> Result<ValidatedConnectionTarget, String> {
    Ok(ValidatedConnectionTarget {
        logical_address: parse_server_address(address)
            .map_err(|error| format!("Invalid server address: {error:?}"))?,
        proposed_display_name: crate::lobby::normalize_proposed_display_name(name)
            .map_err(|error| format!("Invalid display name: {error}"))?,
    })
}

pub(super) fn attempt_deadline_expiry(
    now: Duration,
    pending: &PendingConnection,
) -> Option<AttemptDeadlineExpiry> {
    if pending.dns_deadline.is_some_and(|deadline| now > deadline) {
        Some(AttemptDeadlineExpiry::Dns)
    } else if now > pending.overall_deadline {
        Some(AttemptDeadlineExpiry::Overall)
    } else if pending
        .candidate_deadline
        .is_some_and(|deadline| now > deadline)
    {
        Some(AttemptDeadlineExpiry::Candidate)
    } else {
        None
    }
}

pub(super) fn observation_for_expiry(expiry: AttemptDeadlineExpiry) -> SessionObservation {
    match expiry {
        AttemptDeadlineExpiry::Dns => SessionObservation::DnsTimedOut,
        AttemptDeadlineExpiry::Overall => SessionObservation::TimedOut,
        AttemptDeadlineExpiry::Candidate => SessionObservation::CandidateTimedOut,
    }
}

pub(super) fn accepted_observation(
    now: Duration,
    pending: &PendingConnection,
    disconnected: bool,
) -> SessionObservation {
    if disconnected {
        SessionObservation::UnexpectedLoss
    } else if let Some(expiry) = attempt_deadline_expiry(now, pending) {
        observation_for_expiry(expiry)
    } else {
        SessionObservation::Accepted
    }
}

pub(super) fn has_next_candidate(pending: &PendingConnection) -> bool {
    pending.current_candidate.saturating_add(1) < pending.candidates.len()
}

pub(super) fn candidate_time_share(remaining: Duration, remaining_candidates: u32) -> Duration {
    debug_assert!(remaining_candidates > 0);
    remaining.div_f64(f64::from(remaining_candidates.max(1)))
}

pub(super) fn netcode_timeout_ceiling(remaining: Duration) -> Duration {
    Duration::from_secs(
        remaining
            .as_secs()
            .saturating_add(u64::from(remaining.subsec_nanos() != 0))
            .max(1),
    )
}

pub(super) fn bound_resolved_candidates(
    candidates: impl IntoIterator<Item = SocketAddr>,
) -> Vec<SocketAddr> {
    let mut unique = BTreeSet::new();
    candidates
        .into_iter()
        .filter(|address| unique.insert(*address))
        .take(MAX_RESOLVED_CANDIDATES)
        .collect()
}

pub(super) fn begin_connection_target(
    commands: &mut Commands,
    config: &ClientNetworkConfig,
    now: Duration,
    generation: &mut ConnectionGeneration,
    resolver: &mut ResolverState,
    routed: &mut RoutedClientLifecycle,
    target: ValidatedConnectionTarget,
) -> Result<(), String> {
    generation.0 = generation.0.saturating_add(1).max(1);
    let mut connection = PendingConnection {
        generation: generation.0,
        target,
        candidates: Vec::new(),
        current_candidate: 0,
        overall_deadline: now.saturating_add(ATTEMPT_DEADLINE),
        dns_deadline: None,
        candidate_deadline: None,
        current_entity: None,
        stage: ConnectionStage::ResolvingAddress,
    };
    if let Some(socket) = connection.target.logical_address.numeric_socket() {
        connection.candidates.push(socket);
        connection.stage = ConnectionStage::ContactingServer {
            current: 1,
            total: 1,
        };
        spawn_current_candidate(commands, config, now, routed, &mut connection);
    } else {
        if resolver.task.is_some() {
            return Err("A previous operating-system address lookup is still busy".to_string());
        }
        let ServerAddressHost::Dns(host) = &connection.target.logical_address.host else {
            unreachable!("non-numeric logical server host is DNS")
        };
        let task_generation = connection.generation;
        let query = format!("{}:{}", host, connection.target.logical_address.port);
        resolver.task = Some(ResolverTask {
            generation: task_generation,
            task: IoTaskPool::get().spawn(async move {
                Ok(bound_resolved_candidates(query.to_socket_addrs().map_err(
                    |error| format!("Address resolution failed: {error}"),
                )?))
            }),
        });
        connection.dns_deadline = Some(now.saturating_add(DNS_DEADLINE));
    }
    commands.insert_resource(connection);
    Ok(())
}

pub(super) fn spawn_current_candidate(
    commands: &mut Commands,
    config: &ClientNetworkConfig,
    now: Duration,
    routed: &mut RoutedClientLifecycle,
    pending: &mut PendingConnection,
) {
    let Some(server_addr) = pending.candidates.get(pending.current_candidate).copied() else {
        return;
    };
    let remaining_candidates = pending
        .candidates
        .len()
        .saturating_sub(pending.current_candidate)
        .max(1);
    let remaining = pending.overall_deadline.saturating_sub(now);
    let divisor = u32::try_from(remaining_candidates).expect("candidate bound fits u32");
    let share = candidate_time_share(remaining, divisor);
    pending.candidate_deadline = Some(now.saturating_add(share));
    pending.stage = ConnectionStage::ContactingServer {
        current: pending.current_candidate + 1,
        total: pending.candidates.len(),
    };
    pending.current_entity = spawn_product_lobby_connection(
        commands,
        config,
        routed,
        ProductLobbyAttempt {
            started_at: now,
            server_addr,
            logical_address: pending.target.logical_address.canonical().to_string(),
            proposed_display_name: pending.target.proposed_display_name.clone(),
            netcode_timeout: netcode_timeout_ceiling(remaining),
        },
    )
    .ok();
}

pub(super) fn connection_presentation(pending: &PendingConnection, now: Duration) -> String {
    let dots = "."
        .repeat(usize::try_from((now.as_millis() / 350) % 3 + 1).expect("pulse width is bounded"));
    let remaining = pending.overall_deadline.saturating_sub(now);
    let remaining_seconds = remaining
        .as_secs()
        .saturating_add(u64::from(remaining.subsec_nanos() != 0));
    let address = pending.target.logical_address.canonical();
    match pending.stage {
        ConnectionStage::ResolvingAddress => format!(
            "STEP 1 OF 3\nResolving server address{dots}\n{address}\nUp to {remaining_seconds}s remaining"
        ),
        ConnectionStage::ContactingServer { current, total } => format!(
            "STEP 2 OF 3\nOpening routed connection{dots}\n{address}\nCandidate {current} of {total}  -  up to {remaining_seconds}s remaining"
        ),
        ConnectionStage::JoiningLobby => format!(
            "STEP 3 OF 3\nChecking compatibility and game list{dots}\n{address}\nUp to {remaining_seconds}s remaining"
        ),
    }
}
