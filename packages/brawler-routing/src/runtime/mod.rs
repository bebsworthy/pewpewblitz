//! Plain Mio supervisor shell.
//!
//! This owner loop routes opaque datagrams and framed records only.  It deliberately has no Bevy,
//! Lightyear, Netcode, or gameplay dependency: those protocols stay at their worker/client seams.

use std::{
    collections::HashMap,
    io,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use mio::{Events, Interest, Poll, Token, Waker, net::UdpSocket};

use crate::{
    AllocateRequestBody, AllocationId, AllocationPolicy, Capability, ControlBody,
    ControlSequenceTracker, CoreConfig, IpcChannel, IpcIoError, LifecycleEvent, MatchId,
    MonotonicMillis, PeerId, PrivateRuntimeDir, ProcessSupervisor, ProcessSupervisorConfig,
    RequestId, RouteId, RouteRegistration, RoutingErrorCategory, RoutingMetrics,
    SourceIngressLimiter, UnixWorkerChannels, UnixWorkerListeners, WorkerId, WorkerRegistration,
};

mod capacity;
mod reporting;
mod routing_io;
mod worker_lifecycle;

use reporting::{report_lifecycle_event, report_runtime_observations};

#[cfg(test)]
use crate::Generation;

const PUBLIC_TOKEN: Token = Token(0);
const WAKE_TOKEN: Token = Token(1);
const MAX_TRACKED_ALLOCATIONS: usize = 8;
const ALLOCATION_REJECT_INVALID: u16 = 1;
const ALLOCATION_REJECT_CAPACITY: u16 = 2;
const ALLOCATION_REJECT_INTERNAL: u16 = 3;
const ALLOCATION_REJECT_CONFLICT: u16 = 4;
/// A Result starts a packet-drain phase.  The worker must half-close its packet write direction;
/// if that EOF does not arrive within this bounded interval, the worker is failed and its routes
/// are revoked.  This is a safety deadline only; normal completion is event-driven by EOF.
const RESULT_PACKET_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

/// Bounded owner-loop settings.  The defaults match the M01 burst and readiness contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub public_bind: SocketAddr,
    pub core: CoreConfig,
    pub poll_interval: Duration,
    pub udp_burst: usize,
    pub packet_burst: usize,
    pub control_burst: usize,
    /// Explicit Bevy-free match policy. No match worker can be spawned without it.
    pub allocation_policy: Option<AllocationPolicy>,
    /// Worker executable used for supervisor-created match workers.
    pub worker_executable: Option<PathBuf>,
    /// Protocol registry fingerprint carried by match manifests.
    pub protocol_registry_fingerprint: Option<u64>,
    /// Optional child lifecycle owner. When configured, this runtime owns the `Mio` streams and
    /// `ProcessSupervisor` owns only child status, deadlines, and typed control state.
    pub process_supervisor: Option<ProcessSupervisorConfig>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            public_bind: SocketAddr::from(([127, 0, 0, 1], 0)),
            core: CoreConfig::default(),
            poll_interval: Duration::from_millis(10),
            udp_burst: 64,
            packet_burst: 64,
            control_burst: 16,
            allocation_policy: None,
            worker_executable: None,
            protocol_registry_fingerprint: None,
            process_supervisor: None,
        }
    }
}

#[derive(Debug)]
pub enum RuntimeError {
    Io(io::Error),
    Lifecycle(crate::LifecycleError),
    Routing(RoutingErrorCategory),
    Ipc {
        worker_id: WorkerId,
        channel: IpcChannel,
        error: IpcIoError,
    },
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "supervisor I/O error: {error}"),
            Self::Lifecycle(error) => write!(formatter, "worker lifecycle error: {error}"),
            Self::Routing(category) => write!(formatter, "supervisor routing error: {category:?}"),
            Self::Ipc {
                worker_id,
                channel,
                error,
            } => write!(
                formatter,
                "worker {worker_id} {channel:?} IPC error: {error}"
            ),
        }
    }
}

impl std::error::Error for RuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Lifecycle(error) => Some(error),
            Self::Ipc { error, .. } => error.source(),
            Self::Routing(_) => None,
        }
    }
}

impl From<io::Error> for RuntimeError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Work performed in one bounded poll turn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimePollReport {
    pub public_received: usize,
    pub public_dropped: usize,
    pub packets_to_workers: usize,
    pub packets_to_public: usize,
    pub controls_to_workers: usize,
    pub workers_cleaned: usize,
    pub routes_torn_down: usize,
    pub lifecycle_events: Vec<LifecycleEvent>,
    /// Cross-process timing facts owned by the supervisor. These carry only stable IDs and are
    /// rendered as redacted log markers by the production owner; no capability or manifest bytes
    /// cross this observability boundary.
    pub timing_events: Vec<RuntimeTimingEvent>,
}

/// Correlated supervisor timing facts. Client-side markers use the same `RequestId` and wall-clock
/// timestamp domain, so the evidence harness can measure a handoff across process stderr streams
/// without changing BRPK or BRCT wire layouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeTimingEvent {
    AllocationAccepted {
        request_id: RequestId,
        worker_id: WorkerId,
    },
}

/// Cheap cloneable stop signal suitable for a signal-handler closure.  The owner still wakes its
/// Mio poller through the paired `Waker`; the atomic makes a request visible even if it arrives
/// immediately before a poll turn.
#[derive(Clone)]
pub struct StopHandle {
    requested: Arc<AtomicBool>,
    waker: Arc<Waker>,
}

impl StopHandle {
    pub fn request(&self) -> io::Result<()> {
        self.requested.store(true, Ordering::Release);
        self.waker.wake()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadyTarget {
    PacketListener(WorkerId),
    ControlListener(WorkerId),
    Packet(WorkerId),
    Control(WorkerId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkerControlDisposition {
    Continue,
    ExitReceived,
    Failed,
}

struct RuntimeWorker {
    registration: WorkerRegistration,
    listeners: Option<UnixWorkerListeners>,
    pending_packet: Option<mio::net::UnixStream>,
    pending_control: Option<mio::net::UnixStream>,
    channels: Option<UnixWorkerChannels>,
    packet_token: Option<Token>,
    control_token: Option<Token>,
    packet_listener_token: Option<Token>,
    control_listener_token: Option<Token>,
    control_sequences: ControlSequenceTracker,
    next_control_sequence: u64,
    /// The lobby route is intentionally held here until both IPC streams are attached and the
    /// worker has passed the typed Ready handshake.  Installing it at process spawn would let
    /// public traffic enter a route whose worker channel is still `None`.
    pending_default_route: Option<RouteRegistration>,
    /// Set only after the worker packet stream has delivered EOF.  This is the cross-stream
    /// quiescence barrier used to order Result-triggered route teardown.
    packet_eof: bool,
    /// A Result has been accepted for this match.  Incoming public packets are dropped during
    /// the drain phase while worker-to-public packets remain routable until `packet_eof`.
    result_received: bool,
    result_drain_deadline: Option<Instant>,
    result_teardown_started: bool,
    match_slot_limit: Option<u16>,
    last_free_match_slots: Option<u8>,
}

struct AllocationParticipant {
    source: crate::AllocateParticipant,
    route_id: RouteId,
    peer_id: PeerId,
    capability: Capability,
}

struct AllocationRecord {
    request: AllocateRequestBody,
    lobby_worker_id: WorkerId,
    allocation_id: Option<AllocationId>,
    match_id: Option<MatchId>,
    match_worker_id: Option<WorkerId>,
    participants: Vec<AllocationParticipant>,
    response: Option<ControlBody>,
    response_queued: bool,
    result: Option<crate::ResultBody>,
}

struct PendingPublicDatagram {
    bytes: Vec<u8>,
    destination: SocketAddr,
    inner_bytes: usize,
    match_worker: bool,
    worker_packet_started: Instant,
}

impl RuntimeWorker {
    fn new(registration: WorkerRegistration) -> Self {
        Self {
            registration,
            listeners: None,
            pending_packet: None,
            pending_control: None,
            channels: None,
            packet_token: None,
            control_token: None,
            packet_listener_token: None,
            control_listener_token: None,
            control_sequences: ControlSequenceTracker::default(),
            next_control_sequence: 1,
            pending_default_route: None,
            packet_eof: false,
            result_received: false,
            result_drain_deadline: None,
            result_teardown_started: false,
            match_slot_limit: None,
            last_free_match_slots: None,
        }
    }
}

pub struct SupervisorRuntime {
    config: RuntimeConfig,
    poll: Poll,
    waker: Arc<Waker>,
    public: UdpSocket,
    events: Events,
    core: crate::SupervisorCore,
    workers: HashMap<WorkerId, RuntimeWorker>,
    targets: HashMap<Token, ReadyTarget>,
    next_token: usize,
    incoming: Vec<u8>,
    ingress: SourceIngressLimiter,
    outgoing: std::collections::VecDeque<PendingPublicDatagram>,
    started: Instant,
    outgoing_bytes: usize,
    metrics: RoutingMetrics,
    stop_requested: bool,
    /// Set before the lifecycle owner queues shutdown `Stop` frames.  Once set, runtime-owned
    /// control records must not be appended after those lifecycle frames: they share one BRCT
    /// sequence space, but the lifecycle owner is the only writer allowed to advance it during
    /// shutdown.
    shutting_down: bool,
    stop_flag: Arc<AtomicBool>,
    runtime_dir: Option<PrivateRuntimeDir>,
    processes: Option<ProcessSupervisor>,
    last_process_poll: Option<Instant>,
    allocations: HashMap<RequestId, AllocationRecord>,
    packet_enqueue_started: HashMap<RouteId, std::collections::VecDeque<Instant>>,
}

impl SupervisorRuntime {
    #[cfg(test)]
    fn bind(public_bind: SocketAddr) -> io::Result<Self> {
        let logical_server_id =
            crate::LogicalServerId::new(1).expect("constant identity is nonzero");
        let generation = Generation::new(1).expect("constant generation is nonzero");
        Self::new(RuntimeConfig {
            public_bind,
            core: CoreConfig::with_identity(logical_server_id, generation, 1, 0),
            ..RuntimeConfig::default()
        })
    }

    pub fn new(config: RuntimeConfig) -> io::Result<Self> {
        if !config.core.has_identity() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "supervisor runtime requires logical server, generation, network protocol, and content identity",
            ));
        }
        if let Some(policy) = config.allocation_policy {
            policy.validate().map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid allocation policy: {error}"),
                )
            })?;
        }
        let poll = Poll::new()?;
        let waker = Arc::new(Waker::new(poll.registry(), WAKE_TOKEN)?);
        let mut public = UdpSocket::bind(config.public_bind)?;
        poll.registry()
            .register(&mut public, PUBLIC_TOKEN, Interest::READABLE)?;
        let runtime_dir = PrivateRuntimeDir::create()?;
        let core_config = config.core;
        let processes = config
            .process_supervisor
            .clone()
            .map(ProcessSupervisor::without_runtime_dir);
        Ok(Self {
            config,
            poll,
            waker,
            public,
            events: Events::with_capacity(128),
            core: crate::SupervisorCore::new(core_config),
            workers: HashMap::new(),
            targets: HashMap::new(),
            next_token: 2,
            incoming: vec![0; crate::PUBLIC_MAX_DATAGRAM_BYTES + 1],
            ingress: SourceIngressLimiter::default(),
            outgoing: std::collections::VecDeque::new(),
            started: Instant::now(),
            outgoing_bytes: 0,
            metrics: RoutingMetrics::default(),
            stop_requested: false,
            shutting_down: false,
            stop_flag: Arc::new(AtomicBool::new(false)),
            runtime_dir: Some(runtime_dir),
            processes,
            last_process_poll: None,
            allocations: HashMap::new(),
            packet_enqueue_started: HashMap::new(),
        })
    }

    pub fn public_addr(&self) -> io::Result<SocketAddr> {
        self.public.local_addr()
    }

    #[must_use]
    pub const fn core(&self) -> &crate::SupervisorCore {
        &self.core
    }

    pub fn core_mut(&mut self) -> &mut crate::SupervisorCore {
        &mut self.core
    }

    /// Return the bounded routing metrics collected at the supervisor's public/inner/IPC
    /// boundaries. Values contain no addresses, IDs, or secret capabilities.
    #[must_use]
    pub const fn metrics(&self) -> &RoutingMetrics {
        &self.metrics
    }

    /// Return the current admitted registration for a lifecycle-owned worker. This is a small
    /// observability seam for restart evidence; manifests and capabilities still stay on IPC.
    #[must_use]
    pub fn worker_registration(&self, worker_id: WorkerId) -> Option<WorkerRegistration> {
        self.processes
            .as_ref()
            .and_then(|processes| processes.worker_registration(worker_id))
    }

    /// Number of child lifecycles still owned by this runtime. This narrow inspector lets
    /// process-isolation evidence distinguish an empty route registry from a leaked OS child
    /// after bounded shutdown; it exposes no child handles or process-local state.
    #[must_use]
    pub fn process_worker_count(&self) -> usize {
        self.processes
            .as_ref()
            .map_or(0, ProcessSupervisor::worker_count)
    }

    #[must_use]
    pub fn waker(&self) -> &Waker {
        &self.waker
    }

    #[must_use]
    pub fn stop_handle(&self) -> StopHandle {
        StopHandle {
            requested: Arc::clone(&self.stop_flag),
            waker: Arc::clone(&self.waker),
        }
    }

    #[must_use]
    pub fn runtime_dir(&self) -> Option<&std::path::Path> {
        self.runtime_dir.as_ref().map(PrivateRuntimeDir::path)
    }

    pub fn register_worker(
        &mut self,
        registration: WorkerRegistration,
    ) -> Result<(), RuntimeError> {
        self.core
            .register_worker(registration)
            .map_err(RuntimeError::Routing)?;
        self.workers
            .insert(registration.worker_id, RuntimeWorker::new(registration));
        Ok(())
    }
}

mod allocation;

impl SupervisorRuntime {
    pub fn submit_allocation_request(
        &mut self,
        lobby_worker_id: WorkerId,
        request: AllocateRequestBody,
    ) -> Result<Vec<LifecycleEvent>, RuntimeError> {
        let mut report = RuntimePollReport::default();
        if let Err(error) = self.accept_allocation_request(lobby_worker_id, request, &mut report) {
            if let Some(category) = runtime_worker_failure_category(&error) {
                self.fail_worker_control(lobby_worker_id, category, &mut report)?;
            }
            return Err(error);
        }
        Ok(report.lifecycle_events)
    }

    #[must_use]
    pub const fn stop_requested(&self) -> bool {
        self.stop_requested
    }

    /// Run until the owner receives a wake request.  Ctrl-C handlers should only call
    /// [`Self::request_stop`] (or wake the returned [`Self::waker`]).
    pub fn run(&mut self) -> Result<(), RuntimeError> {
        while !self.stop_requested {
            let report = self.poll_once(Some(self.config.poll_interval))?;
            report_runtime_observations(&report, self.started.elapsed());
        }
        self.shutdown_processes()?;
        Ok(())
    }

    pub fn poll_once(
        &mut self,
        timeout: Option<Duration>,
    ) -> Result<RuntimePollReport, RuntimeError> {
        let timeout = timeout
            .unwrap_or(self.config.poll_interval)
            .min(self.config.poll_interval);
        if let Err(error) = self.poll.poll(&mut self.events, Some(timeout)) {
            if error.kind() == io::ErrorKind::Interrupted {
                if self.stop_flag.load(Ordering::Acquire) {
                    self.stop_requested = true;
                }
                return Ok(RuntimePollReport::default());
            }
            return Err(RuntimeError::Io(error));
        }
        let ready = self
            .events
            .iter()
            .map(|event| (event.token(), event.is_readable(), event.is_writable()))
            .collect::<Vec<_>>();
        let mut report = RuntimePollReport::default();
        self.queue_external_controls();
        for (token, readable, writable) in ready {
            if token == WAKE_TOKEN {
                self.stop_requested = true;
            } else if token == PUBLIC_TOKEN && !self.shutting_down {
                self.receive_public(&mut report)?;
                if writable {
                    self.flush_public(&mut report)?;
                }
            } else if let Some(target) = self.targets.get(&token).copied() {
                match target {
                    ReadyTarget::PacketListener(worker) if readable => {
                        self.accept_stream(worker, true)?;
                    }
                    ReadyTarget::ControlListener(worker) if readable => {
                        self.accept_stream(worker, false)?;
                    }
                    ReadyTarget::Packet(worker) => {
                        self.handle_packet(worker, readable, writable, &mut report)?;
                    }
                    ReadyTarget::Control(worker) => {
                        self.handle_control(worker, readable, writable, &mut report)?;
                    }
                    _ => {}
                }
            }
        }
        // Readable worker controls are handled before child reconciliation. A worker is allowed
        // to send Exit and close its control stream in one write/exit turn; try_wait must not
        // classify that as a missing-exit crash before the typed Exit body is observed.
        self.poll_processes(&mut report)?;
        if !self.shutting_down {
            self.activate_ready_routes(&mut report);
            self.finalize_ready_allocations(&mut report)?;
        }
        if !self.shutting_down {
            self.queue_allocation_responses();
            self.refresh_lobby_capacity();
        }
        self.expire_result_packet_drains();
        if self.stop_flag.load(Ordering::Acquire) {
            self.stop_requested = true;
        }
        self.expire(&mut report);
        if self.shutting_down {
            // Do not append pre-shutdown runtime controls/packets after lifecycle Stop. They are
            // abandoned with the worker during bounded shutdown; the worker's existing channel
            // queue still flushes in its original order before Stop.
            let _ = self.core.drain_controls(self.config.control_burst);
            let _ = self.core.drain_packets(self.config.packet_burst);
        } else {
            self.dispatch_queues(&mut report)?;
        }
        self.flush_workers(&mut report)?;
        if !self.shutting_down {
            self.flush_public(&mut report)?;
        }
        Ok(report)
    }

    fn allocate_target(&mut self, target: ReadyTarget) -> Token {
        let token = Token(self.next_token);
        self.next_token = self.next_token.saturating_add(1);
        self.targets.insert(token, target);
        token
    }

    fn now(&self) -> MonotonicMillis {
        MonotonicMillis(u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX))
    }
}

mod control_io;

impl SupervisorRuntime {
    fn cleanup_worker(&mut self, worker_id: WorkerId) {
        self.reject_allocation_for_worker(worker_id, ALLOCATION_REJECT_INTERNAL);
        if let Some(processes) = self.processes.as_mut()
            && processes.worker_phase(worker_id).is_some()
        {
            let _ = processes.fail_worker(worker_id, RoutingErrorCategory::IpcPacketClosed);
        }
        let Some(mut worker) = self.workers.remove(&worker_id) else {
            return;
        };
        if let Some(mut channels) = worker.channels.take() {
            if let Some(token) = worker.packet_token {
                let _ = self
                    .poll
                    .registry()
                    .deregister(channels.packet_source_mut());
                self.targets.remove(&token);
            }
            if let Some(token) = worker.control_token {
                let _ = self
                    .poll
                    .registry()
                    .deregister(channels.control_source_mut());
                self.targets.remove(&token);
            }
        }
        if let Some(mut listeners) = worker.listeners.take() {
            if let Some(token) = worker.packet_listener_token {
                let _ = self
                    .poll
                    .registry()
                    .deregister(listeners.packet_listener_mut());
                self.targets.remove(&token);
            }
            if let Some(token) = worker.control_listener_token {
                let _ = self
                    .poll
                    .registry()
                    .deregister(listeners.control_listener_mut());
                self.targets.remove(&token);
            }
        }
        self.targets.retain(|_, target| {
            !matches!(
                target,
                ReadyTarget::Packet(id)
                    | ReadyTarget::Control(id)
                    | ReadyTarget::PacketListener(id)
                    | ReadyTarget::ControlListener(id)
                    if *id == worker_id
            )
        });
        let _ = self.core.cleanup_worker(worker_id);
    }
}

impl Drop for SupervisorRuntime {
    fn drop(&mut self) {
        let workers = self.workers.keys().copied().collect::<Vec<_>>();
        for worker in workers {
            self.cleanup_worker(worker);
        }
        if let Some(runtime_dir) = self.runtime_dir.take() {
            let _ = runtime_dir.cleanup();
        }
    }
}

fn random_u128() -> Result<u128, RuntimeError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|_| RuntimeError::Routing(RoutingErrorCategory::SupervisorInternal))?;
    Ok(u128::from_be_bytes(bytes).max(1))
}

fn random_u64() -> Result<u64, RuntimeError> {
    let mut bytes = [0_u8; 8];
    getrandom::fill(&mut bytes)
        .map_err(|_| RuntimeError::Routing(RoutingErrorCategory::SupervisorInternal))?;
    Ok(u64::from_be_bytes(bytes).max(1))
}

fn random_id128<T>(constructor: fn(u128) -> Option<T>) -> Result<T, RuntimeError> {
    constructor(random_u128()?).ok_or(RuntimeError::Routing(
        RoutingErrorCategory::SupervisorInternal,
    ))
}

fn unix_now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

fn allocation_rejection_for(category: RoutingErrorCategory) -> u16 {
    if category == RoutingErrorCategory::AllocationCapacity {
        ALLOCATION_REJECT_CAPACITY
    } else {
        ALLOCATION_REJECT_INTERNAL
    }
}

/// Return a worker-scoped category for a lifecycle error raised while validating a control
/// record.  Errors that indicate an owner violation or a broken supervisor invariant remain
/// errors from `poll_once` and are never downgraded to worker cleanup.
fn worker_control_failure_category(error: &crate::LifecycleError) -> Option<RoutingErrorCategory> {
    match error {
        crate::LifecycleError::Codec(_) => Some(RoutingErrorCategory::IpcMalformed),
        crate::LifecycleError::Routing(category) => worker_scoped_category(*category),
        _ => None,
    }
}

fn runtime_worker_failure_category(error: &RuntimeError) -> Option<RoutingErrorCategory> {
    match error {
        RuntimeError::Routing(category) => worker_scoped_category(*category),
        RuntimeError::Lifecycle(error) => worker_control_failure_category(error),
        RuntimeError::Ipc { .. } | RuntimeError::Io(_) => None,
    }
}

fn worker_scoped_category(category: RoutingErrorCategory) -> Option<RoutingErrorCategory> {
    (!matches!(
        category,
        RoutingErrorCategory::SupervisorInternal | RoutingErrorCategory::SupervisorShutdown
    ))
    .then_some(category)
}

#[cfg(test)]
mod tests;
