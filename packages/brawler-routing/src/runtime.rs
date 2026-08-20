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
    AllocateRequestBody, AllocationGrant, AllocationGrantedBody, AllocationId, AllocationPolicy,
    AllocationRejectedBody, CONTROL_VERSION_CURRENT, Capability, CapabilityBinding, ControlBody,
    ControlFrame, ControlSequenceTracker, CoreConfig, Generation, IpcChannel, IpcIoError,
    LifecycleEvent, ManifestBody, ManifestCommon, MatchId, MatchManifestParticipant,
    MatchManifestV1, MonotonicMillis, PACKET_VERSION_V1, PacketDirection, PacketRecord, PeerId,
    PrivateRuntimeDir, ProcessId, ProcessSupervisor, ProcessSupervisorConfig, PublicEnvelope,
    ROUTE_VERSION_V1, RequestId, RouteId, RouteRegistration, RoutingErrorCategory, RoutingMetrics,
    SeedPolicy, SourceIngressLimiter, StopId, UnixWorkerChannels, UnixWorkerListeners, WorkerId,
    WorkerKind, WorkerLaunchSpec, WorkerRegistration, WorkerRole,
};

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

    #[allow(clippy::too_many_lines)]
    fn accept_allocation_request(
        &mut self,
        lobby_worker_id: WorkerId,
        request: AllocateRequestBody,
        report: &mut RuntimePollReport,
    ) -> Result<(), RuntimeError> {
        if self.shutting_down {
            return Ok(());
        }
        if let Some(existing) = self.allocations.get(&request.request_id) {
            if existing.request == request && existing.lobby_worker_id == lobby_worker_id {
                return Ok(());
            }
            self.queue_control_body(
                lobby_worker_id,
                ControlBody::AllocationRejected(AllocationRejectedBody {
                    request_id: request.request_id,
                    reason: ALLOCATION_REJECT_CONFLICT,
                    retry_after_ms: 0,
                }),
            );
            return Ok(());
        }
        if self
            .workers
            .get(&lobby_worker_id)
            .is_none_or(|worker| worker.registration.kind != WorkerKind::Lobby)
        {
            return Ok(());
        }
        if self.allocations.len() >= MAX_TRACKED_ALLOCATIONS {
            self.queue_control_body(
                lobby_worker_id,
                ControlBody::AllocationRejected(AllocationRejectedBody {
                    request_id: request.request_id,
                    reason: ALLOCATION_REJECT_CAPACITY,
                    retry_after_ms: 1_000,
                }),
            );
            return Ok(());
        }
        if crate::validate_m01_request(&request).is_err() {
            self.insert_rejected_allocation(request, lobby_worker_id, ALLOCATION_REJECT_INVALID);
            return Ok(());
        }
        let Some(policy) = self.config.allocation_policy else {
            self.insert_rejected_allocation(request, lobby_worker_id, ALLOCATION_REJECT_INTERNAL);
            return Ok(());
        };
        let Some(executable) = self.config.worker_executable.clone() else {
            self.insert_rejected_allocation(request, lobby_worker_id, ALLOCATION_REJECT_INTERNAL);
            return Ok(());
        };
        let Some(protocol_registry_fingerprint) = self.config.protocol_registry_fingerprint else {
            self.insert_rejected_allocation(request, lobby_worker_id, ALLOCATION_REJECT_INTERNAL);
            return Ok(());
        };
        let Some(processes) = self.processes.as_ref() else {
            self.insert_rejected_allocation(request, lobby_worker_id, ALLOCATION_REJECT_INTERNAL);
            return Ok(());
        };
        if processes.worker_count() >= self.config.core.max_workers {
            self.insert_rejected_allocation(request, lobby_worker_id, ALLOCATION_REJECT_CAPACITY);
            return Ok(());
        }
        let logical_server_id = self
            .config
            .core
            .logical_server_id
            .ok_or(RuntimeError::Routing(
                RoutingErrorCategory::ManifestIncompatible,
            ))?;
        let network_protocol = self
            .config
            .core
            .network_protocol
            .ok_or(RuntimeError::Routing(
                RoutingErrorCategory::ManifestIncompatible,
            ))?;
        let content_fingerprint =
            self.config
                .core
                .content_fingerprint
                .ok_or(RuntimeError::Routing(
                    RoutingErrorCategory::ManifestIncompatible,
                ))?;
        let Ok(allocation_id) = random_id128(AllocationId::new) else {
            self.insert_rejected_allocation(request, lobby_worker_id, ALLOCATION_REJECT_INTERNAL);
            return Ok(());
        };
        let Ok(match_id) = random_id128(MatchId::new) else {
            self.insert_rejected_allocation(request, lobby_worker_id, ALLOCATION_REJECT_INTERNAL);
            return Ok(());
        };
        let Ok(match_worker_id) = self.fresh_worker_id() else {
            self.insert_rejected_allocation(request, lobby_worker_id, ALLOCATION_REJECT_INTERNAL);
            return Ok(());
        };
        let Ok(process_id) = random_id128(ProcessId::new) else {
            self.insert_rejected_allocation(request, lobby_worker_id, ALLOCATION_REJECT_INTERNAL);
            return Ok(());
        };
        let generation = Generation::new(1).expect("constant generation is nonzero");
        let seed = match policy.seed_policy {
            SeedPolicy::OsRandom => {
                let Ok(seed) = random_u64() else {
                    self.insert_rejected_allocation(
                        request,
                        lobby_worker_id,
                        ALLOCATION_REJECT_INTERNAL,
                    );
                    return Ok(());
                };
                seed
            }
        };
        let mut participants = Vec::with_capacity(request.participants.len());
        let mut manifest_participants = Vec::with_capacity(request.participants.len());
        for source in &request.participants {
            let route_id = Self::fresh_route_id(&participants)?;
            let peer_id = Self::fresh_peer_id(&participants)?;
            let Ok(capability) = Capability::generate() else {
                self.insert_rejected_allocation(
                    request,
                    lobby_worker_id,
                    ALLOCATION_REJECT_INTERNAL,
                );
                return Ok(());
            };
            manifest_participants.push(MatchManifestParticipant {
                lobby_session_id: source.lobby_session_id,
                player_id: source.player_id,
                netcode_client_id: source.netcode_client_id,
                peer_id,
                team: source.team,
                display_name: source.display_name,
                source_build_preset: source.source_build_preset,
                recipe_fingerprint: source.recipe_fingerprint,
                revision: source.build_revision,
                build_snapshot: source.build_snapshot,
            });
            participants.push(AllocationParticipant {
                source: *source,
                route_id,
                peer_id,
                capability,
            });
        }
        let manifest = MatchManifestV1 {
            common: ManifestCommon {
                manifest_version: 2,
                role: WorkerRole::Match,
                logical_server_id,
                process_id,
                worker_id: match_worker_id,
                generation,
                network_protocol,
                protocol_registry_fingerprint,
                content_fingerprint,
                route_version: ROUTE_VERSION_V1,
                packet_version: PACKET_VERSION_V1,
                control_version: CONTROL_VERSION_CURRENT,
                flags: 0,
            },
            request_id: request.request_id,
            match_id,
            allocation_id,
            mode: request.mode,
            map_preset: request.map_preset,
            map_revision: request.map_revision,
            rules_profile: request.rules_profile,
            objective_target: request.objective_target,
            match_duration_ticks: request.match_duration_ticks,
            countdown_ticks: request.countdown_ticks,
            respawn_ticks: request.respawn_ticks,
            reserved: 0,
            seed,
            participants: manifest_participants,
            heartbeat_ms: policy.heartbeat_ms,
            nonce: random_u128()
                .map_err(|_| RuntimeError::Routing(RoutingErrorCategory::SupervisorInternal))?,
            digest: [0; 32],
        };
        let mut spec = WorkerLaunchSpec::new(
            executable,
            WorkerRegistration {
                worker_id: match_worker_id,
                process_id,
                generation,
                kind: WorkerKind::Match,
            },
            ManifestBody::from_match(&manifest)
                .map_err(|_| RuntimeError::Routing(RoutingErrorCategory::ManifestMalformed))?,
        );
        // The paired evidence launcher opts into one role-local authoritative window marker.
        // Keep this as a narrow environment seam: the worker manifest and argv remain unchanged,
        // while the match worker writes its first Active->Completed interval to a unique path.
        if let Some(window_dir) = std::env::var_os("BRAWLER_ROUTED_WINDOW_DIR") {
            let window_path = PathBuf::from(window_dir).join("match.window");
            spec = spec
                .with_environment("BRAWLER_DIAGNOSTICS_WINDOW_FILE", window_path)
                .with_environment("BRAWLER_DIAGNOSTICS_ROLE", "match");
        }
        let request_id = request.request_id;
        self.allocations.insert(
            request_id,
            AllocationRecord {
                request: request.clone(),
                lobby_worker_id,
                allocation_id: Some(allocation_id),
                match_id: Some(match_id),
                match_worker_id: Some(match_worker_id),
                participants,
                response: None,
                response_queued: false,
                result: None,
            },
        );
        // This is the handoff gate's origin: the request has passed every policy/capacity check,
        // its immutable record is committed, and the cold match-worker spawn has not begun yet.
        // Emitting at grant delivery would incorrectly exclude spawn and validated Ready latency.
        report
            .timing_events
            .push(RuntimeTimingEvent::AllocationAccepted {
                request_id,
                worker_id: match_worker_id,
            });
        if let Ok(events) = self.spawn_worker(spec) {
            report.lifecycle_events.extend(events);
        } else {
            self.cleanup_worker(match_worker_id);
            self.reject_allocation(request_id, ALLOCATION_REJECT_INTERNAL);
        }
        Ok(())
    }

    fn insert_rejected_allocation(
        &mut self,
        request: AllocateRequestBody,
        lobby_worker_id: WorkerId,
        reason: u16,
    ) {
        let request_id = request.request_id;
        self.allocations.insert(
            request_id,
            AllocationRecord {
                request,
                lobby_worker_id,
                allocation_id: None,
                match_id: None,
                match_worker_id: None,
                participants: Vec::new(),
                response: Some(ControlBody::AllocationRejected(AllocationRejectedBody {
                    request_id,
                    reason,
                    retry_after_ms: 0,
                })),
                response_queued: false,
                result: None,
            },
        );
    }

    fn reject_allocation(&mut self, request_id: RequestId, reason: u16) {
        let Some(record) = self.allocations.get_mut(&request_id) else {
            return;
        };
        if record.response.is_some() {
            return;
        }
        record.response = Some(ControlBody::AllocationRejected(AllocationRejectedBody {
            request_id,
            reason,
            retry_after_ms: 0,
        }));
        record.response_queued = false;
        record.match_worker_id = None;
    }

    fn reject_allocation_for_worker(&mut self, worker_id: WorkerId, reason: u16) {
        let ids = self
            .allocations
            .iter()
            .filter_map(|(request_id, record)| {
                (record.match_worker_id == Some(worker_id)
                    && record.response.is_none()
                    && record.result.is_none())
                .then_some(*request_id)
            })
            .collect::<Vec<_>>();
        for request_id in ids {
            self.reject_allocation(request_id, reason);
        }
    }

    fn finalize_ready_allocations(
        &mut self,
        report: &mut RuntimePollReport,
    ) -> Result<(), RuntimeError> {
        let ready_workers = report
            .lifecycle_events
            .iter()
            .filter_map(|event| match event {
                LifecycleEvent::Ready { worker_id } => Some(*worker_id),
                _ => None,
            })
            .collect::<Vec<_>>();
        for worker_id in ready_workers {
            let Some(request_id) = self.allocations.iter().find_map(|(request_id, record)| {
                (record.match_worker_id == Some(worker_id) && record.response.is_none())
                    .then_some(*request_id)
            }) else {
                continue;
            };
            self.finalize_allocation(request_id, worker_id)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn finalize_allocation(
        &mut self,
        request_id: RequestId,
        match_worker_id: WorkerId,
    ) -> Result<(), RuntimeError> {
        let Some(record) = self.allocations.get(&request_id) else {
            return Ok(());
        };
        let Some(allocation_id) = record.allocation_id else {
            return Ok(());
        };
        let Some(match_id) = record.match_id else {
            return Ok(());
        };
        let Some(registration) = self
            .processes
            .as_ref()
            .and_then(|processes| processes.worker_registration(match_worker_id))
        else {
            self.reject_allocation(request_id, ALLOCATION_REJECT_INTERNAL);
            return Ok(());
        };
        let logical_server_id = self
            .config
            .core
            .logical_server_id
            .ok_or(RuntimeError::Routing(
                RoutingErrorCategory::ManifestIncompatible,
            ))?;
        let supervisor_generation =
            self.config
                .core
                .supervisor_generation
                .ok_or(RuntimeError::Routing(
                    RoutingErrorCategory::ManifestIncompatible,
                ))?;
        let network_protocol = self
            .config
            .core
            .network_protocol
            .ok_or(RuntimeError::Routing(
                RoutingErrorCategory::ManifestIncompatible,
            ))?;
        let content_fingerprint =
            self.config
                .core
                .content_fingerprint
                .ok_or(RuntimeError::Routing(
                    RoutingErrorCategory::ManifestIncompatible,
                ))?;
        let now = self.now();
        let expiry = unix_now_millis().saturating_add(crate::CAPABILITY_HARD_LIFETIME_MILLIS);
        let activation_expiry = unix_now_millis().saturating_add(crate::CAPABILITY_PENDING_MILLIS);
        let participants = record
            .participants
            .iter()
            .map(|participant| {
                (
                    participant.source,
                    participant.route_id,
                    participant.peer_id,
                    participant.capability.clone(),
                    AllocationGrant {
                        lobby_session_id: participant.source.lobby_session_id,
                        route_id: participant.route_id,
                        peer_id: participant.peer_id,
                        capability: participant.capability.clone(),
                        activation_expiry_unix_ms: activation_expiry,
                        route_expiry_unix_ms: expiry,
                    },
                )
            })
            .collect::<Vec<_>>();
        for (_, route_id, peer_id, _, _) in &participants {
            if let Err(category) = self.core.register_route(RouteRegistration {
                route_id: *route_id,
                worker_id: match_worker_id,
                peer_id: *peer_id,
                is_default_lobby: false,
            }) {
                self.cleanup_worker(match_worker_id);
                self.reject_allocation(request_id, allocation_rejection_for(category));
                return Ok(());
            }
        }
        for (source, route_id, peer_id, capability, _) in &participants {
            let binding = CapabilityBinding {
                logical_server_id,
                supervisor_generation,
                worker_id: match_worker_id,
                worker_generation: registration.generation,
                route_id: *route_id,
                peer_id: *peer_id,
                lobby_session_id: source.lobby_session_id,
                allocation_id,
                match_id,
                network_protocol,
                content_fingerprint,
            };
            if let Err(category) = self.core.bind_capability(capability.clone(), binding, now) {
                self.cleanup_worker(match_worker_id);
                self.reject_allocation(request_id, allocation_rejection_for(category));
                return Ok(());
            }
        }
        let grants = participants
            .into_iter()
            .map(|(_, _, _, _, grant)| grant)
            .collect();
        if let Some(record) = self.allocations.get_mut(&request_id) {
            record.response = Some(ControlBody::AllocationGranted(AllocationGrantedBody {
                request_id,
                allocation_id,
                match_id,
                worker_id: match_worker_id,
                grants,
            }));
            record.response_queued = false;
        }
        Ok(())
    }

    fn queue_allocation_responses(&mut self) {
        let mut ids = self.allocations.keys().copied().collect::<Vec<_>>();
        ids.sort_by_key(|id| id.get());
        for request_id in ids {
            let Some((worker_id, response)) =
                self.allocations.get(&request_id).and_then(|record| {
                    (!record.response_queued)
                        .then(|| {
                            record
                                .response
                                .clone()
                                .map(|response| (record.lobby_worker_id, response))
                        })
                        .flatten()
                })
            else {
                continue;
            };
            if self.queue_control_body(worker_id, response)
                && let Some(record) = self.allocations.get_mut(&request_id)
            {
                record.response_queued = true;
            }
        }
        // Rejections have no match worker whose Result still needs to be correlated. Once the
        // response is safely in the bounded supervisor queue, release the request record so the
        // bound describes concurrent work rather than the process's entire request history.
        self.allocations
            .retain(|_, record| record.match_worker_id.is_some() || !record.response_queued);
    }

    fn queue_control_body(&mut self, worker_id: WorkerId, body: ControlBody) -> bool {
        // ProcessSupervisor owns the shutdown Stop frame.  A runtime response queued after it
        // would be physically ordered after Stop while still carrying the pre-Stop runtime
        // cursor, so the worker would reject it as stale (and a later retry could duplicate the
        // sequence).  Shutdown deliberately drops these non-lifecycle controls.
        if self.shutting_down {
            return false;
        }
        let Some(worker) = self.workers.get_mut(&worker_id) else {
            return false;
        };
        let sequence = worker.next_control_sequence;
        worker.next_control_sequence = worker.next_control_sequence.saturating_add(1);
        let Ok(frame) = ControlFrame::from_raw_sequence(
            sequence,
            worker.registration.process_id,
            worker_id,
            body,
        ) else {
            return false;
        };
        let Ok(record) = frame.encode() else {
            return false;
        };
        self.core.enqueue_control(worker_id, record).is_ok()
    }

    fn fresh_worker_id(&self) -> Result<WorkerId, RuntimeError> {
        for _ in 0..16 {
            let id = random_id128(WorkerId::new)?;
            if self.workers.contains_key(&id)
                || self
                    .allocations
                    .values()
                    .any(|record| record.match_worker_id == Some(id))
            {
                continue;
            }
            return Ok(id);
        }
        Err(RuntimeError::Routing(
            RoutingErrorCategory::SupervisorInternal,
        ))
    }

    fn fresh_route_id(participants: &[AllocationParticipant]) -> Result<RouteId, RuntimeError> {
        for _ in 0..16 {
            let id = random_id128(RouteId::new)?;
            if participants
                .iter()
                .any(|participant| participant.route_id == id)
            {
                continue;
            }
            return Ok(id);
        }
        Err(RuntimeError::Routing(
            RoutingErrorCategory::SupervisorInternal,
        ))
    }

    fn fresh_peer_id(participants: &[AllocationParticipant]) -> Result<PeerId, RuntimeError> {
        for _ in 0..16 {
            let id = random_id128(PeerId::new)?;
            if participants
                .iter()
                .any(|participant| participant.peer_id == id)
            {
                continue;
            }
            return Ok(id);
        }
        Err(RuntimeError::Routing(
            RoutingErrorCategory::SupervisorInternal,
        ))
    }

    pub fn register_worker_listener(
        &mut self,
        registration: WorkerRegistration,
        mut listeners: UnixWorkerListeners,
    ) -> Result<(), RuntimeError> {
        self.register_worker(registration)?;
        let packet_token =
            self.allocate_target(ReadyTarget::PacketListener(registration.worker_id));
        let control_token =
            self.allocate_target(ReadyTarget::ControlListener(registration.worker_id));
        self.poll.registry().register(
            listeners.packet_listener_mut(),
            packet_token,
            Interest::READABLE,
        )?;
        self.poll.registry().register(
            listeners.control_listener_mut(),
            control_token,
            Interest::READABLE,
        )?;
        let worker = self
            .workers
            .get_mut(&registration.worker_id)
            .expect("worker inserted above");
        worker.packet_listener_token = Some(packet_token);
        worker.control_listener_token = Some(control_token);
        worker.listeners = Some(listeners);
        Ok(())
    }

    pub fn attach_worker_channels(
        &mut self,
        worker_id: WorkerId,
        mut channels: UnixWorkerChannels,
    ) -> Result<(), RuntimeError> {
        let packet_token = self.allocate_target(ReadyTarget::Packet(worker_id));
        let control_token = self.allocate_target(ReadyTarget::Control(worker_id));
        let (packet_token, control_token) = {
            let worker = self
                .workers
                .get_mut(&worker_id)
                .ok_or(RuntimeError::Routing(
                    RoutingErrorCategory::ManifestIdentity,
                ))?;
            if worker.channels.is_some() {
                return Err(RuntimeError::Routing(
                    RoutingErrorCategory::WorkerProtocolConflict,
                ));
            }
            self.poll.registry().register(
                channels.packet_source_mut(),
                packet_token,
                Interest::READABLE,
            )?;
            self.poll.registry().register(
                channels.control_source_mut(),
                control_token,
                Interest::READABLE,
            )?;
            worker.packet_token = Some(packet_token);
            worker.control_token = Some(control_token);
            worker.channels = Some(channels);
            (packet_token, control_token)
        };
        let _ = (packet_token, control_token);
        Ok(())
    }

    /// Spawn one worker through the process lifecycle contract while retaining sole ownership of
    /// its listeners and accepted streams in this Mio runtime.
    pub fn spawn_worker(
        &mut self,
        spec: WorkerLaunchSpec,
    ) -> Result<Vec<LifecycleEvent>, RuntimeError> {
        let worker_id = spec.registration.worker_id;
        let registration = spec.registration;
        let manifest_body = spec.manifest.clone();
        let (pending_default_route, match_slot_limit) = if registration.kind == WorkerKind::Lobby {
            let manifest = crate::LobbyManifest::decode(&manifest_body.manifest)
                .map_err(|_| RuntimeError::Routing(RoutingErrorCategory::ManifestMalformed))?;
            (
                Some(RouteRegistration {
                    route_id: manifest.default_route_id,
                    worker_id,
                    peer_id: PeerId::new(manifest.default_route_id.get()).ok_or(
                        RuntimeError::Routing(RoutingErrorCategory::ManifestIdentity),
                    )?,
                    is_default_lobby: true,
                }),
                Some(manifest.active_matches),
            )
        } else {
            (None, None)
        };
        let runtime = self.runtime_dir.as_ref().ok_or(RuntimeError::Routing(
            RoutingErrorCategory::SupervisorShutdown,
        ))?;
        let listeners = UnixWorkerListeners::bind(runtime, worker_id)?;
        let processes = self.processes.as_mut().ok_or(RuntimeError::Routing(
            RoutingErrorCategory::SupervisorInternal,
        ))?;
        let events = processes
            .spawn_with_listeners(spec, listeners)
            .map_err(RuntimeError::Lifecycle)?;
        let listeners = processes
            .take_worker_listeners(worker_id)
            .map_err(RuntimeError::Lifecycle)?;
        self.register_worker_listener(registration, listeners)?;
        if let Some(worker) = self.workers.get_mut(&worker_id) {
            worker.pending_default_route = pending_default_route;
            worker.match_slot_limit = match_slot_limit;
        }
        Ok(events)
    }

    pub fn request_stop(&self) -> io::Result<()> {
        self.waker.wake()
    }

    /// Queue one idempotent worker Stop through the lifecycle owner. The actual control record is
    /// delivered on the next bounded owner turn, preserving the single Mio stream owner.
    pub fn stop_worker(
        &mut self,
        worker_id: WorkerId,
        stop_id: StopId,
        reason: u16,
    ) -> Result<bool, RuntimeError> {
        let next_sequence = self
            .workers
            .get(&worker_id)
            .map(|worker| worker.next_control_sequence)
            .ok_or(RuntimeError::Routing(
                RoutingErrorCategory::SupervisorInternal,
            ))?;
        if let Some(processes) = self.processes.as_mut() {
            processes
                .sync_external_next_sequence(worker_id, next_sequence)
                .map_err(RuntimeError::Lifecycle)?;
        }
        self.processes
            .as_mut()
            .ok_or(RuntimeError::Routing(
                RoutingErrorCategory::SupervisorInternal,
            ))?
            .stop_worker(worker_id, stop_id, reason)
            .inspect(|&queued| {
                if queued && let Some(worker) = self.workers.get_mut(&worker_id) {
                    // The lifecycle owner consumed this sequence for Stop.  Keep the runtime
                    // cursor past it so any later owner-side inspection cannot reuse the frame.
                    worker.next_control_sequence = worker.next_control_sequence.saturating_add(1);
                }
            })
            .map_err(RuntimeError::Lifecycle)
    }

    /// Submit one already-decoded lobby request through the same bounded orchestration seam used
    /// by the control-stream owner. This keeps integration adapters from reconstructing manifests
    /// or capabilities outside the supervisor.
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

    fn shutdown_processes(&mut self) -> Result<(), RuntimeError> {
        if self.processes.is_none() {
            return Ok(());
        }
        self.shutting_down = true;
        // Runtime-owned lobby controls and lifecycle-owned Stop share one BRCT sequence space.
        // Synchronize every live worker before ProcessSupervisor allocates shutdown frames.
        let already_stopping = self
            .workers
            .keys()
            .copied()
            .filter(|worker_id| {
                self.processes
                    .as_ref()
                    .is_some_and(|processes| processes.worker_is_stopping(*worker_id))
            })
            .collect::<std::collections::HashSet<_>>();
        let sequence_cursors = self
            .workers
            .iter()
            .map(|(worker_id, worker)| (*worker_id, worker.next_control_sequence))
            .collect::<Vec<_>>();
        if let Some(processes) = self.processes.as_mut() {
            for (worker_id, next_sequence) in sequence_cursors {
                processes
                    .sync_external_next_sequence(worker_id, next_sequence)
                    .map_err(RuntimeError::Lifecycle)?;
            }
        }
        let initial_events = self
            .processes
            .as_mut()
            .expect("process supervisor exists")
            .begin_shutdown_at(Instant::now())
            .map_err(RuntimeError::Lifecycle)?;
        for event in &initial_events {
            report_lifecycle_event(event, self.started.elapsed());
        }
        // `begin_shutdown_at` queues Stop through ProcessSupervisor directly, rather than via
        // `stop_worker`, so advance the runtime cursor for each newly requested lifecycle Stop.
        // This is intentionally event-based: a worker that was already stopping did not consume
        // another sequence during this shutdown pass.
        for worker_id in initial_events.iter().filter_map(|event| match event {
            LifecycleEvent::StopRequested { worker_id, .. } => Some(*worker_id),
            _ => None,
        }) {
            if !already_stopping.contains(&worker_id)
                && let Some(worker) = self.workers.get_mut(&worker_id)
            {
                worker.next_control_sequence = worker.next_control_sequence.saturating_add(1);
            }
        }
        let started = Instant::now();
        while self
            .processes
            .as_ref()
            .is_some_and(|processes| processes.worker_count() > 0)
            && started.elapsed() < Duration::from_secs(5)
        {
            let report = self.poll_once(Some(self.config.poll_interval))?;
            report_runtime_observations(&report, self.started.elapsed());
        }
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

    fn poll_processes(&mut self, report: &mut RuntimePollReport) -> Result<(), RuntimeError> {
        let now = Instant::now();
        if self
            .last_process_poll
            .is_some_and(|last| now.duration_since(last) < Duration::from_millis(100))
        {
            return Ok(());
        }
        self.last_process_poll = Some(now);
        if self.processes.is_none() {
            return Ok(());
        }
        let events = self
            .processes
            .as_mut()
            .expect("process supervisor exists")
            .poll_at(now)
            .map_err(RuntimeError::Lifecycle)?;
        for event in events {
            let terminal = matches!(
                &event,
                LifecycleEvent::Failed { .. }
                    | LifecycleEvent::Stopped { .. }
                    | LifecycleEvent::ChildReaped { .. }
                    | LifecycleEvent::Cleaned { .. }
            );
            let worker_id = match &event {
                LifecycleEvent::Spawned { worker_id, .. }
                | LifecycleEvent::ManifestSent { worker_id }
                | LifecycleEvent::Ready { worker_id }
                | LifecycleEvent::HeartbeatSuspect { worker_id }
                | LifecycleEvent::HeartbeatRecovered { worker_id }
                | LifecycleEvent::ExitReceived { worker_id, .. }
                | LifecycleEvent::ChildReaped { worker_id, .. }
                | LifecycleEvent::Failed { worker_id, .. }
                | LifecycleEvent::StopRequested { worker_id, .. }
                | LifecycleEvent::StopSent { worker_id, .. }
                | LifecycleEvent::ForcedStop { worker_id }
                | LifecycleEvent::Stopped { worker_id, .. }
                | LifecycleEvent::RestartScheduled { worker_id, .. }
                | LifecycleEvent::RestartExhausted { worker_id }
                | LifecycleEvent::Cleaned { worker_id }
                | LifecycleEvent::Control { worker_id, .. }
                | LifecycleEvent::ResultReceived { worker_id, .. } => *worker_id,
            };
            report.lifecycle_events.push(event);
            if terminal {
                self.cleanup_worker(worker_id);
                self.reclaim_terminal_allocations(worker_id);
            }
        }
        if !self.shutting_down {
            let external_restart_ids = self
                .processes
                .as_ref()
                .expect("process supervisor exists")
                .due_external_restart_ids(now);
            for worker_id in external_restart_ids {
                let spec = self
                    .processes
                    .as_mut()
                    .expect("process supervisor exists")
                    .take_due_external_restart(worker_id, now)
                    .map_err(RuntimeError::Lifecycle)?;
                if let Some(spec) = spec {
                    let events = self.spawn_worker(spec)?;
                    report.lifecycle_events.extend(events);
                }
            }
        }
        Ok(())
    }

    /// Fail a worker that announced a terminal Result but never supplied the packet-stream EOF
    /// barrier.  Normal completion never waits on a sleep: `handle_packet` invokes
    /// `maybe_complete_match_result` as soon as EOF is observed.  This bounded deadline only
    /// prevents a stuck worker from retaining routes/capabilities indefinitely.
    fn expire_result_packet_drains(&mut self) {
        let now = Instant::now();
        let expired = self
            .workers
            .iter()
            .filter_map(|(worker_id, worker)| {
                worker
                    .result_drain_deadline
                    .is_some_and(|deadline| now >= deadline && !worker.packet_eof)
                    .then_some(*worker_id)
            })
            .collect::<Vec<_>>();
        for worker_id in expired {
            self.core.note_error(RoutingErrorCategory::IpcPacketClosed);
            self.cleanup_worker(worker_id);
        }
    }

    /// Release request records once their worker lifecycle is terminal. Rejected records are
    /// reclaimed earlier, after their response enters the bounded control queue; successful
    /// records remain only until the match Result/Exit handshake has been reconciled.
    fn reclaim_terminal_allocations(&mut self, worker_id: WorkerId) {
        let lobby_requests = self
            .allocations
            .iter()
            .filter_map(|(request_id, record)| {
                (record.lobby_worker_id == worker_id)
                    .then_some((*request_id, record.match_worker_id))
            })
            .collect::<Vec<_>>();
        if !lobby_requests.is_empty() {
            // A dead lobby cannot receive a pending response. Any match it launched is orphaned
            // and is stopped through the same bounded cleanup path before its record is dropped.
            let orphaned_matches = lobby_requests
                .iter()
                .filter_map(|(_, match_worker_id)| *match_worker_id)
                .collect::<Vec<_>>();
            for (request_id, _) in lobby_requests {
                self.allocations.remove(&request_id);
            }
            for match_worker_id in orphaned_matches {
                self.cleanup_worker(match_worker_id);
            }
        }

        let match_requests = self
            .allocations
            .iter()
            .filter_map(|(request_id, record)| {
                (record.match_worker_id == Some(worker_id)).then_some(*request_id)
            })
            .collect::<Vec<_>>();
        for request_id in match_requests {
            let Some(record) = self.allocations.get(&request_id) else {
                continue;
            };
            if record.result.is_some() || record.response_queued {
                self.allocations.remove(&request_id);
            } else {
                self.reject_allocation_terminal(request_id, ALLOCATION_REJECT_INTERNAL);
            }
        }
    }

    fn reject_allocation_terminal(&mut self, request_id: RequestId, reason: u16) {
        let Some(record) = self.allocations.get_mut(&request_id) else {
            return;
        };
        if record.response_queued || record.result.is_some() {
            return;
        }
        record.response = Some(ControlBody::AllocationRejected(AllocationRejectedBody {
            request_id,
            reason,
            retry_after_ms: 0,
        }));
        record.response_queued = false;
        record.match_worker_id = None;
        record.allocation_id = None;
        record.match_id = None;
    }

    /// Publish the default lobby route only after the process supervisor has accepted Ready.
    /// `Ready` can only be observed through the attached control stream, so this also establishes
    /// the IPC-attachment half of the admission invariant. Early public datagrams therefore fail
    /// closed in `SupervisorCore` instead of reaching `dispatch_queues` with missing channels.
    fn activate_ready_routes(&mut self, report: &mut RuntimePollReport) {
        let ready_workers = report
            .lifecycle_events
            .iter()
            .filter_map(|event| match event {
                LifecycleEvent::Ready { worker_id } => Some(*worker_id),
                _ => None,
            })
            .collect::<Vec<_>>();
        for worker_id in ready_workers {
            let Some(route) = self
                .workers
                .get_mut(&worker_id)
                .and_then(|worker| worker.pending_default_route.take())
            else {
                continue;
            };
            if let Err(category) = self.core.register_route(route) {
                self.core.note_error(category);
                self.cleanup_worker(worker_id);
            }
        }
    }

    fn queue_external_controls(&mut self) {
        if self.processes.is_none() {
            return;
        }
        let worker_ids = self.workers.keys().copied().collect::<Vec<_>>();
        for worker_id in worker_ids {
            if self
                .workers
                .get(&worker_id)
                .is_none_or(|worker| worker.channels.is_none())
            {
                continue;
            }
            let records = self
                .processes
                .as_mut()
                .expect("process supervisor exists")
                .take_external_control_records(worker_id);
            for record in records {
                let Some(worker) = self.workers.get_mut(&worker_id) else {
                    continue;
                };
                let Some(channels) = worker.channels.as_mut() else {
                    continue;
                };
                if channels.enqueue_control(&record).is_err() {
                    self.cleanup_worker(worker_id);
                    break;
                }
                self.metrics
                    .ipc_to_worker
                    .observe_ipc_frame(record.len().saturating_add(4));
            }
        }
    }

    /// Publish only the scalar capacity needed by the lobby. Match workers remain counted until
    /// their runtime entry is removed after the lifecycle owner observes actual child reap.
    fn refresh_lobby_capacity(&mut self) {
        let match_workers = self.processes.as_ref().map_or_else(
            || {
                self.workers
                    .values()
                    .filter(|worker| worker.registration.kind == WorkerKind::Match)
                    .count()
            },
            |processes| processes.worker_count().saturating_sub(1),
        );
        let Some((lobby_worker_id, manifest_limit, previous, ready)) =
            self.workers.iter().find_map(|(worker_id, worker)| {
                if worker.registration.kind != WorkerKind::Lobby {
                    return None;
                }
                Some((
                    *worker_id,
                    usize::from(worker.match_slot_limit?),
                    worker.last_free_match_slots,
                    worker.pending_default_route.is_none(),
                ))
            })
        else {
            return;
        };
        if !ready {
            return;
        }
        let host_limit = self.config.core.max_workers.saturating_sub(1);
        let free = u8::try_from(
            manifest_limit
                .min(host_limit)
                .saturating_sub(match_workers)
                .min(usize::from(u8::MAX)),
        )
        .expect("free slot value was clamped to u8");
        if previous == Some(free) {
            return;
        }
        if self.queue_control_body(
            lobby_worker_id,
            ControlBody::LobbyCapacity(crate::LobbyCapacityBody {
                free_match_slots: free,
            }),
        ) && let Some(worker) = self.workers.get_mut(&lobby_worker_id)
        {
            worker.last_free_match_slots = Some(free);
        }
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

    fn expire(&mut self, report: &mut RuntimePollReport) {
        let now = self.now();
        self.ingress.expire(now);
        let teardowns = self.core.expire(now);
        if self.shutting_down {
            // Route expiry still revokes routes/capabilities above, but its PeerClose controls
            // must not be appended after lifecycle-owned Stop during global shutdown.
            report.routes_torn_down += teardowns.len();
            return;
        }
        for teardown in teardowns {
            report.routes_torn_down += 1;
            let mut failed = false;
            if let Some(worker) = self.workers.get_mut(&teardown.worker_id)
                && let Some(channels) = worker.channels.as_mut()
            {
                let sequence = worker.next_control_sequence;
                worker.next_control_sequence = worker.next_control_sequence.saturating_add(1);
                let frame = ControlFrame::from_raw_sequence(
                    sequence,
                    worker.registration.process_id,
                    teardown.worker_id,
                    ControlBody::PeerClose(crate::PeerCloseBody {
                        route_id: teardown.route_id,
                        peer_id: teardown.peer_id,
                        reason: 1,
                    }),
                )
                .and_then(|frame| frame.encode());
                match frame {
                    Ok(frame) => {
                        failed = channels.enqueue_control(&frame).is_err();
                        if !failed {
                            self.metrics
                                .ipc_to_worker
                                .observe_ipc_frame(frame.len().saturating_add(4));
                        }
                    }
                    Err(_) => failed = true,
                }
            }
            if failed {
                self.cleanup_worker(teardown.worker_id);
            }
        }
    }

    fn receive_public(&mut self, report: &mut RuntimePollReport) -> Result<(), RuntimeError> {
        for _ in 0..self.config.udp_burst {
            let (length, source) = match self.public.recv_from(&mut self.incoming) {
                Ok(value) => value,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(RuntimeError::Io(error)),
            };
            report.public_received += 1;
            let public_received_at = Instant::now();
            self.metrics.public_ingress.observe_datagram(length);
            let now = self.now();
            if self.ingress.is_suppressed(source, now) {
                report.public_dropped += 1;
                self.core.note_error(RoutingErrorCategory::SourceLimited);
                continue;
            }
            let envelope = match PublicEnvelope::decode(&self.incoming[..length]) {
                Ok(envelope) => envelope,
                Err(error) => {
                    report.public_dropped += 1;
                    let category = match error {
                        crate::CodecError::Oversize => RoutingErrorCategory::PublicOversize,
                        crate::CodecError::UnsupportedVersion(_)
                        | crate::CodecError::UnsupportedType(_) => {
                            RoutingErrorCategory::PublicUnsupported
                        }
                        _ => RoutingErrorCategory::PublicMalformed,
                    };
                    self.core.note_error(category);
                    if self.ingress.record_malformed(source, now)
                        == crate::IngressDecision::Suppressed
                    {
                        self.core.note_error(RoutingErrorCategory::SourceLimited);
                    }
                    continue;
                }
            };
            self.metrics.public_ingress.observe_frame();
            self.metrics
                .inner_ingress
                .observe_datagram(envelope.payload().len());
            self.metrics.inner_ingress.observe_frame();
            // The lobby route is published only after the worker's Ready handshake.  A client
            // may legitimately send Netcode handshake retries in that startup window; dropping
            // them here keeps the pre-auth limiter reserved for packets that can actually reach
            // an admitted route, while retaining the exact 8-datagram/9-KiB budget thereafter.
            // Boundary counters still include these valid envelopes so public/inner accounting
            // remains an exact per-datagram relation even when readiness races the first retry.
            if matches!(envelope.selector(), crate::RouteSelector::DefaultLobby)
                && !self.core.default_lobby_ready()
            {
                report.public_dropped += 1;
                continue;
            }
            if matches!(envelope.selector(), crate::RouteSelector::DefaultLobby)
                && self.ingress.admit_default(source, length, now)
                    != crate::IngressDecision::Allowed
            {
                report.public_dropped += 1;
                self.core.note_error(RoutingErrorCategory::SourceLimited);
                continue;
            }
            let capability_selector =
                matches!(envelope.selector(), crate::RouteSelector::Capability(_));
            let routed = self.core.route_public(&envelope, source, now);
            if let Ok(route) = routed {
                if self
                    .workers
                    .get(&route.worker_id)
                    .is_some_and(|worker| worker.registration.kind == WorkerKind::Match)
                {
                    self.metrics
                        .match_inner_ingress
                        .observe_datagram(envelope.payload().len());
                    self.metrics.match_inner_ingress.observe_frame();
                }
                if capability_selector {
                    self.ingress.promote_authenticated(source, now);
                }
                self.packet_enqueue_started
                    .entry(route.route_id)
                    .or_default()
                    .push_back(public_received_at);
            } else {
                report.public_dropped += 1;
            }
        }
        Ok(())
    }

    fn accept_stream(&mut self, worker_id: WorkerId, packet: bool) -> Result<(), RuntimeError> {
        let accepted = {
            let worker = self
                .workers
                .get_mut(&worker_id)
                .ok_or(RuntimeError::Routing(
                    RoutingErrorCategory::ManifestIdentity,
                ))?;
            let Some(listeners) = worker.listeners.as_mut() else {
                return Ok(());
            };
            let listener = if packet {
                listeners.packet_listener_mut()
            } else {
                listeners.control_listener_mut()
            };
            match listener.accept() {
                Ok((stream, _)) => Some(stream),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => None,
                Err(error) => return Err(RuntimeError::Io(error)),
            }
        };
        if let Some(stream) = accepted {
            let pair = {
                let worker = self.workers.get_mut(&worker_id).expect("worker exists");
                if packet {
                    worker.pending_packet = Some(stream);
                } else {
                    worker.pending_control = Some(stream);
                }
                worker
                    .pending_packet
                    .as_ref()
                    .zip(worker.pending_control.as_ref())
                    .is_some()
            };
            if pair {
                let (packet_stream, control_stream) = {
                    let worker = self.workers.get_mut(&worker_id).expect("worker exists");
                    (
                        worker.pending_packet.take().expect("pair checked"),
                        worker.pending_control.take().expect("pair checked"),
                    )
                };
                self.attach_worker_channels(
                    worker_id,
                    UnixWorkerChannels::new(packet_stream, control_stream),
                )?;
                self.queue_worker_manifest(worker_id)?;
            }
        }
        Ok(())
    }

    fn queue_worker_manifest(&mut self, worker_id: WorkerId) -> Result<(), RuntimeError> {
        let Some(manifest) = self
            .processes
            .as_ref()
            .and_then(|processes| processes.worker_manifest(worker_id))
        else {
            return Ok(());
        };
        let Some(worker) = self.workers.get_mut(&worker_id) else {
            return Err(RuntimeError::Routing(
                RoutingErrorCategory::ManifestIdentity,
            ));
        };
        let Some(channels) = worker.channels.as_mut() else {
            return Err(RuntimeError::Routing(
                RoutingErrorCategory::IpcControlClosed,
            ));
        };
        channels
            .enqueue_control(&manifest)
            .map_err(|error| RuntimeError::Ipc {
                worker_id,
                channel: IpcChannel::Control,
                error,
            })?;
        self.metrics
            .ipc_to_worker
            .observe_ipc_frame(manifest.len().saturating_add(4));
        // ProcessSupervisor's immutable Manifest always owns sequence 1. Runtime-generated
        // controls share that same supervisor-to-worker sequence space, so the first grant,
        // rejection, peer close, or stop must begin at 2.
        worker.next_control_sequence = worker.next_control_sequence.max(2);
        self.processes
            .as_mut()
            .expect("process supervisor exists for worker manifest")
            .mark_manifest_sent(worker_id)
            .map_err(RuntimeError::Lifecycle)?;
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn handle_packet(
        &mut self,
        worker_id: WorkerId,
        readable: bool,
        writable: bool,
        report: &mut RuntimePollReport,
    ) -> Result<(), RuntimeError> {
        let mut failed = false;
        let mut packet_eof = false;
        if readable {
            let match_worker = self
                .workers
                .get(&worker_id)
                .is_some_and(|worker| worker.registration.kind == WorkerKind::Match);
            let lifecycle_owned = self
                .processes
                .as_ref()
                .is_some_and(|processes| processes.worker_phase(worker_id).is_some());
            let result = {
                let worker = self
                    .workers
                    .get_mut(&worker_id)
                    .expect("token worker exists");
                worker
                    .channels
                    .as_mut()
                    .ok_or(RuntimeError::Routing(RoutingErrorCategory::IpcPacketClosed))?
                    .packet_read_ready(self.config.packet_burst)
            };
            match result {
                Ok(progress) => {
                    self.metrics
                        .ipc_from_worker
                        .observe_ipc_read(progress.bytes_read, progress.records.len());
                    // For lifecycle-owned workers, process reconciliation is the authority for
                    // EOF versus a valid Exit. Do not kill a child merely because its packet
                    // half closes in the same turn as its typed Exit control frame.
                    failed = progress.eof && !lifecycle_owned;
                    for raw in progress.records {
                        let worker_packet_started = Instant::now();
                        let packet =
                            match PacketRecord::decode(&raw, PacketDirection::WorkerToSupervisor) {
                                Ok(packet) => packet,
                                Err(error) => {
                                    self.core.note_error(RoutingErrorCategory::IpcMalformed);
                                    let _ = error;
                                    failed = true;
                                    break;
                                }
                            };
                        if packet.worker_id != worker_id {
                            self.core.note_error(RoutingErrorCategory::ManifestIdentity);
                            failed = true;
                            break;
                        }
                        let Ok(destination) = self.core.accept_worker_packet(&packet) else {
                            failed = true;
                            break;
                        };
                        let Some(selector) = self.core.public_selector_for_route(packet.route_id)
                        else {
                            self.core.note_error(RoutingErrorCategory::Binding);
                            failed = true;
                            break;
                        };
                        let inner_bytes = packet.payload.len();
                        let envelope = PublicEnvelope::new(selector, packet.payload)
                            .map_err(|_| RuntimeError::Routing(RoutingErrorCategory::IpcMalformed))?
                            .encode()
                            .map_err(|_| {
                                RuntimeError::Routing(RoutingErrorCategory::IpcMalformed)
                            })?;
                        if self.enqueue_public_datagram_with_metadata(
                            envelope,
                            destination,
                            inner_bytes,
                            match_worker,
                            worker_packet_started,
                        ) {
                            report.packets_to_public += 1;
                        }
                    }
                    if progress.eof {
                        // EOF is the explicit worker-side packet drain barrier.  A partial
                        // framed record cannot be silently accepted as a terminal success.
                        let buffered = self
                            .workers
                            .get(&worker_id)
                            .and_then(|worker| worker.channels.as_ref())
                            .map_or(0, UnixWorkerChannels::packet_buffered_bytes);
                        if buffered != 0 {
                            self.core.note_error(RoutingErrorCategory::IpcMalformed);
                            failed = true;
                        } else {
                            packet_eof = true;
                        }
                    }
                }
                Err(error) => {
                    let _ = error;
                    failed = true;
                }
            }
        }
        if packet_eof && !failed {
            if let Some(worker) = self.workers.get_mut(&worker_id) {
                worker.packet_eof = true;
            }
            self.maybe_complete_match_result(worker_id)?;
        }
        if writable && !failed {
            let result = self
                .workers
                .get_mut(&worker_id)
                .expect("token worker exists")
                .channels
                .as_mut()
                .expect("packet channel exists")
                .flush_packet(self.config.packet_burst);
            if result.is_err() {
                failed = true;
            }
        }
        if failed {
            self.cleanup_worker(worker_id);
        } else {
            self.update_worker_interest(worker_id)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn handle_control(
        &mut self,
        worker_id: WorkerId,
        readable: bool,
        writable: bool,
        report: &mut RuntimePollReport,
    ) -> Result<(), RuntimeError> {
        let mut failed = false;
        if readable {
            let lifecycle_owned = self
                .processes
                .as_ref()
                .is_some_and(|processes| processes.worker_phase(worker_id).is_some());
            let result = {
                let worker = self
                    .workers
                    .get_mut(&worker_id)
                    .expect("token worker exists");
                worker
                    .channels
                    .as_mut()
                    .ok_or(RuntimeError::Routing(
                        RoutingErrorCategory::IpcControlClosed,
                    ))?
                    .control_read_ready(self.config.control_burst)
            };
            match result {
                Ok(progress) => {
                    self.metrics
                        .ipc_from_worker
                        .observe_ipc_read(progress.bytes_read, progress.records.len());
                    failed = progress.eof && !lifecycle_owned;
                    let mut received_exit = false;
                    for raw in progress.records {
                        let frame = {
                            let worker = self.workers.get(&worker_id).expect("worker exists");
                            ControlFrame::decode_for(
                                &raw,
                                worker.registration.process_id,
                                worker_id,
                            )
                        };
                        let Ok(frame) = frame else {
                            self.core.note_error(RoutingErrorCategory::IpcMalformed);
                            failed = true;
                            break;
                        };
                        let allocation_request = match &frame.body {
                            ControlBody::AllocateRequest(request) => Some(request.clone()),
                            _ => None,
                        };
                        let result_body = match &frame.body {
                            ControlBody::Result(result) => Some(result.clone()),
                            _ => None,
                        };
                        let peer_close = match &frame.body {
                            ControlBody::PeerClose(close) => Some(*close),
                            _ => None,
                        };
                        let lobby_authenticated = match &frame.body {
                            ControlBody::LobbyAuthenticated(fact) => Some(*fact),
                            _ => None,
                        };
                        let lobby_netcode_authenticated = match &frame.body {
                            ControlBody::LobbyNetcodeAuthenticated(fact) => Some(*fact),
                            _ => None,
                        };
                        let cancel_activation = match &frame.body {
                            ControlBody::CancelActivation(fact) => Some(*fact),
                            _ => None,
                        };
                        let activated = match &frame.body {
                            ControlBody::Activated(fact) => Some(*fact),
                            _ => None,
                        };
                        let start_failed = match &frame.body {
                            ControlBody::StartFailed(fact) => Some(*fact),
                            _ => None,
                        };
                        if let Some(processes) = self.processes.as_mut() {
                            match processes.observe_control_frame(worker_id, &frame, Instant::now())
                            {
                                Ok(events) => {
                                    received_exit |= events.iter().any(|event| {
                                        matches!(event, LifecycleEvent::ExitReceived { .. })
                                    });
                                    report.lifecycle_events.extend(events);
                                }
                                Err(error) => {
                                    let Some(category) = worker_control_failure_category(&error)
                                    else {
                                        // Unknown workers, owner violations, and supervisor
                                        // invariants must still fail the owner loop.  Only a
                                        // decoded worker fact with a stable worker-scoped
                                        // semantic error may be isolated below.
                                        return Err(RuntimeError::Lifecycle(error));
                                    };
                                    self.fail_worker_control(worker_id, category, report)?;
                                    failed = true;
                                    break;
                                }
                            }
                        } else {
                            let worker = self.workers.get_mut(&worker_id).expect("worker exists");
                            if worker.control_sequences.observe(frame).is_err() {
                                self.core.note_error(RoutingErrorCategory::IpcMalformed);
                                failed = true;
                                break;
                            }
                        }
                        if !self.shutting_down
                            && let Some(request) = allocation_request
                            && let Err(error) =
                                self.accept_allocation_request(worker_id, request, report)
                        {
                            let Some(category) = runtime_worker_failure_category(&error) else {
                                // A missing supervisor identity/entropy is an owner invariant,
                                // not a bad worker fact. Preserve the hard failure boundary.
                                return Err(error);
                            };
                            self.fail_worker_control(worker_id, category, report)?;
                            failed = true;
                            break;
                        }
                        if let Some(result) = result_body
                            && let Err(error) = self.complete_match_result(worker_id, result)
                        {
                            let Some(category) = runtime_worker_failure_category(&error) else {
                                return Err(error);
                            };
                            self.fail_worker_control(worker_id, category, report)?;
                            failed = true;
                            break;
                        }
                        if let Some(close) = peer_close {
                            let teardown = match self.core.close_route_from_worker(
                                worker_id,
                                close.route_id,
                                close.peer_id,
                            ) {
                                Ok(teardown) => teardown,
                                Err(category) => {
                                    self.fail_worker_control(worker_id, category, report)?;
                                    failed = true;
                                    break;
                                }
                            };
                            if teardown.is_some() {
                                report.routes_torn_down += 1;
                            }
                        }
                        if let Some(fact) = lobby_authenticated {
                            let source = match self.core.authenticated_lobby_source(worker_id, fact)
                            {
                                Ok(source) => source,
                                Err(category) => {
                                    self.fail_worker_control(worker_id, category, report)?;
                                    failed = true;
                                    break;
                                }
                            };
                            self.ingress.promote_authenticated(source, self.now());
                        }
                        if let Some(fact) = lobby_netcode_authenticated {
                            let source = match self
                                .core
                                .authenticated_lobby_netcode_source(worker_id, fact)
                            {
                                Ok(source) => source,
                                Err(category) => {
                                    self.fail_worker_control(worker_id, category, report)?;
                                    failed = true;
                                    break;
                                }
                            };
                            self.ingress.promote_authenticated(source, self.now());
                        }
                        if let Some(fact) = cancel_activation {
                            let allocation =
                                self.allocations.get(&fact.request_id).and_then(|record| {
                                    (record.allocation_id == Some(fact.allocation_id)
                                        && record.match_id == Some(fact.match_id)
                                        && (record.lobby_worker_id == worker_id
                                            || record.match_worker_id == Some(worker_id)))
                                    .then(|| {
                                        record.match_worker_id.map(|match_worker_id| {
                                            (record.lobby_worker_id, match_worker_id)
                                        })
                                    })
                                    .flatten()
                                });
                            let Some((lobby_worker_id, match_worker_id)) = allocation else {
                                self.fail_worker_control(
                                    worker_id,
                                    RoutingErrorCategory::WorkerProtocolConflict,
                                    report,
                                )?;
                                failed = true;
                                break;
                            };
                            let dissolved = ControlBody::ActivationDissolved(fact);
                            if !self.queue_control_body(lobby_worker_id, dissolved.clone())
                                || !self.queue_control_body(match_worker_id, dissolved)
                            {
                                failed = true;
                                break;
                            }
                        }
                        if let Some(fact) = activated {
                            let lobby_worker_id =
                                self.allocations.get(&fact.request_id).and_then(|record| {
                                    (record.match_worker_id == Some(worker_id)
                                        && record.allocation_id == Some(fact.allocation_id)
                                        && record.match_id == Some(fact.match_id))
                                    .then_some(record.lobby_worker_id)
                                });
                            let Some(lobby_worker_id) = lobby_worker_id else {
                                self.fail_worker_control(
                                    worker_id,
                                    RoutingErrorCategory::WorkerProtocolConflict,
                                    report,
                                )?;
                                failed = true;
                                break;
                            };
                            if !self
                                .queue_control_body(lobby_worker_id, ControlBody::Activated(fact))
                            {
                                failed = true;
                                break;
                            }
                        }
                        if let Some(fact) = start_failed {
                            let lobby_worker_id =
                                self.allocations.get(&fact.request_id).and_then(|record| {
                                    (record.match_worker_id == Some(worker_id)
                                        && record.allocation_id == Some(fact.allocation_id)
                                        && record.match_id == Some(fact.match_id))
                                    .then_some(record.lobby_worker_id)
                                });
                            let Some(lobby_worker_id) = lobby_worker_id else {
                                self.fail_worker_control(
                                    worker_id,
                                    RoutingErrorCategory::WorkerProtocolConflict,
                                    report,
                                )?;
                                failed = true;
                                break;
                            };
                            let dissolved = ControlBody::ActivationDissolved(fact);
                            if !self.queue_control_body(lobby_worker_id, dissolved.clone())
                                || !self.queue_control_body(worker_id, dissolved)
                            {
                                failed = true;
                                break;
                            }
                        }
                    }
                    // Keep the channels registered for one more owner turn after a valid Exit.
                    // ProcessSupervisor reconciles the already-observed body with try_wait; an
                    // EOF without Exit remains an immediate IPC failure.
                    if received_exit {
                        failed = false;
                    }
                }
                Err(error) => {
                    let _ = error;
                    failed = true;
                }
            }
        }
        if writable && !failed {
            let result = self
                .workers
                .get_mut(&worker_id)
                .expect("token worker exists")
                .channels
                .as_mut()
                .expect("control channel exists")
                .flush_control(self.config.control_burst);
            if result.is_err() {
                failed = true;
            }
        }
        if failed {
            self.cleanup_worker(worker_id);
        } else {
            self.update_worker_interest(worker_id)?;
        }
        report.controls_to_workers += 0;
        Ok(())
    }

    /// A validated match Result is the worker's terminal gameplay fact. Tear down only that
    /// match's routes/capabilities, then send one graceful Stop through the lifecycle owner so
    /// the worker can emit Exit after Result. The lobby and any unrelated worker stay live.
    fn complete_match_result(
        &mut self,
        worker_id: WorkerId,
        result: crate::ResultBody,
    ) -> Result<(), RuntimeError> {
        let Some(request_id) = self.allocations.iter().find_map(|(request_id, record)| {
            (record.match_worker_id == Some(worker_id)
                && record.match_id == Some(result.match_id)
                && record.allocation_id == Some(result.allocation_id))
            .then_some(*request_id)
        }) else {
            return Err(RuntimeError::Routing(
                RoutingErrorCategory::WorkerProtocolConflict,
            ));
        };
        let record = self
            .allocations
            .get_mut(&request_id)
            .expect("allocation request was just found");
        if record.result.is_some() {
            return Err(RuntimeError::Routing(
                RoutingErrorCategory::WorkerProtocolConflict,
            ));
        }
        record.result = Some(result);
        let worker = self
            .workers
            .get_mut(&worker_id)
            .ok_or(RuntimeError::Routing(
                RoutingErrorCategory::WorkerProtocolConflict,
            ))?;
        worker.result_received = true;
        worker.result_drain_deadline = Some(Instant::now() + RESULT_PACKET_DRAIN_TIMEOUT);
        // The Result control stream and gameplay packet stream are independent. Keep the route
        // and capability registry alive until the worker packet write half closes; otherwise a
        // control frame that overtakes a final BRPK frame would discard that gameplay packet.
        self.maybe_complete_match_result(worker_id)
    }

    fn maybe_complete_match_result(&mut self, worker_id: WorkerId) -> Result<(), RuntimeError> {
        let should_teardown = self.workers.get(&worker_id).is_some_and(|worker| {
            worker.packet_eof && worker.result_received && !worker.result_teardown_started
        });
        if !should_teardown {
            return Ok(());
        }
        let worker = self
            .workers
            .get_mut(&worker_id)
            .expect("result worker exists");
        worker.result_teardown_started = true;
        // EOF is the explicit drain barrier: every complete BRPK frame written by the worker is
        // already readable and has been routed before this registry cleanup drops the route.
        let _ = self.core.cleanup_worker(worker_id);
        // Bevy-free in-memory runtimes have no lifecycle owner to receive Stop. They still need
        // the same route teardown semantics for deterministic transport tests.
        if self.processes.is_none() || self.shutting_down {
            return Ok(());
        }
        let stop_id = StopId::new(random_u64()?).ok_or(RuntimeError::Routing(
            RoutingErrorCategory::SupervisorInternal,
        ))?;
        self.stop_worker(worker_id, stop_id, 0)?;
        Ok(())
    }

    /// Isolate a decoded worker fact whose semantics do not match this worker's admitted
    /// identity.  The process lifecycle owner still receives the exact failure category so a
    /// lobby can follow its bounded restart policy; route and capability cleanup is restricted to
    /// this worker.  Supervisor invariants intentionally do not use this path.
    fn fail_worker_control(
        &mut self,
        worker_id: WorkerId,
        category: RoutingErrorCategory,
        report: &mut RuntimePollReport,
    ) -> Result<(), RuntimeError> {
        self.core.note_error(category);
        if let Some(processes) = self.processes.as_mut()
            && processes.worker_phase(worker_id).is_some()
        {
            let events = processes
                .fail_worker(worker_id, category)
                .map_err(RuntimeError::Lifecycle)?;
            report.lifecycle_events.extend(events);
        }
        self.cleanup_worker(worker_id);
        Ok(())
    }

    fn dispatch_queues(&mut self, report: &mut RuntimePollReport) -> Result<(), RuntimeError> {
        for packet in self.core.drain_packets(self.config.packet_burst) {
            let worker_id = packet.worker_id;
            let public_received_at = self.take_packet_enqueue_started(packet.route_id);
            if self
                .workers
                .get(&worker_id)
                .is_some_and(|worker| worker.result_received)
            {
                // A terminal Result stops new client intent immediately, but worker-to-public
                // packets remain routable until the packet EOF drain barrier is observed.
                continue;
            }
            if self.core.is_default_template(packet.route_id) {
                self.core.note_error(RoutingErrorCategory::Binding);
                continue;
            }
            let encoded = packet
                .encode()
                .map_err(|_| RuntimeError::Routing(RoutingErrorCategory::IpcMalformed))?;
            let Some(channels) = self
                .workers
                .get_mut(&worker_id)
                .and_then(|worker| worker.channels.as_mut())
            else {
                // Route publication is readiness-gated, but keep this owner loop fail-closed if
                // a future route transition races cleanup: an early packet is dropped rather
                // than turning a missing IPC attachment into a supervisor-wide poll failure.
                self.core.note_error(RoutingErrorCategory::IpcPacketClosed);
                continue;
            };
            let result = channels.enqueue_packet(&encoded);
            if result.is_err() {
                self.cleanup_worker(worker_id);
            } else {
                self.metrics
                    .ipc_to_worker
                    .observe_ipc_frame(encoded.len().saturating_add(4));
                if let Some(public_received_at) = public_received_at {
                    self.metrics
                        .public_receive_to_packet_ipc_enqueue
                        .observe(public_received_at.elapsed());
                }
                report.packets_to_workers += 1;
            }
        }
        for (worker_id, record) in self.core.drain_controls(self.config.control_burst) {
            let Some(channels) = self
                .workers
                .get_mut(&worker_id)
                .and_then(|worker| worker.channels.as_mut())
            else {
                self.core.note_error(RoutingErrorCategory::IpcControlClosed);
                continue;
            };
            let result = channels.enqueue_control(&record);
            if result.is_err() {
                self.cleanup_worker(worker_id);
            } else {
                self.metrics
                    .ipc_to_worker
                    .observe_ipc_frame(record.len().saturating_add(4));
                report.controls_to_workers += 1;
            }
        }
        Ok(())
    }

    fn take_packet_enqueue_started(&mut self, route_id: RouteId) -> Option<Instant> {
        let started = self
            .packet_enqueue_started
            .get_mut(&route_id)
            .and_then(std::collections::VecDeque::pop_front);
        if self
            .packet_enqueue_started
            .get(&route_id)
            .is_some_and(std::collections::VecDeque::is_empty)
        {
            self.packet_enqueue_started.remove(&route_id);
        }
        started
    }

    fn flush_workers(&mut self, _report: &mut RuntimePollReport) -> Result<(), RuntimeError> {
        let worker_ids = self.workers.keys().copied().collect::<Vec<_>>();
        for worker_id in worker_ids {
            let (failed, control_drained) = {
                let Some(worker) = self.workers.get_mut(&worker_id) else {
                    continue;
                };
                let Some(channels) = worker.channels.as_mut() else {
                    continue;
                };
                let failed = channels.flush_packet(self.config.packet_burst).is_err()
                    || channels.flush_control(self.config.control_burst).is_err();
                (failed, !channels.control_pending())
            };
            if failed {
                self.cleanup_worker(worker_id);
            } else if self.workers.contains_key(&worker_id) {
                if control_drained
                    && let Some(processes) = self.processes.as_mut()
                    && processes
                        .mark_external_stop_sent(worker_id)
                        .map_err(RuntimeError::Lifecycle)?
                {
                    // The lifecycle owner queues StopSent behind StopRequested and publishes both
                    // on its next poll, preserving the actual causal order in evidence logs.
                }
                self.update_worker_interest(worker_id)?;
            }
        }
        Ok(())
    }

    fn flush_public(&mut self, _report: &mut RuntimePollReport) -> Result<(), RuntimeError> {
        for _ in 0..self.config.udp_burst {
            let Some(pending) = self.outgoing.front() else {
                break;
            };
            match self.public.send_to(&pending.bytes, pending.destination) {
                Ok(_) => {
                    if let Some(pending) = self.outgoing.pop_front() {
                        self.outgoing_bytes =
                            self.outgoing_bytes.saturating_sub(pending.bytes.len());
                        self.metrics
                            .public_egress
                            .observe_datagram(pending.bytes.len());
                        self.metrics.public_egress.observe_frame();
                        self.metrics
                            .inner_egress
                            .observe_datagram(pending.inner_bytes);
                        self.metrics.inner_egress.observe_frame();
                        if pending.match_worker {
                            self.metrics
                                .match_inner_egress
                                .observe_datagram(pending.inner_bytes);
                            self.metrics.match_inner_egress.observe_frame();
                        }
                        self.metrics
                            .worker_packet_to_public_send
                            .observe(pending.worker_packet_started.elapsed());
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(RuntimeError::Io(error)),
            }
        }
        let interest = if self.outgoing.is_empty() {
            Interest::READABLE
        } else {
            Interest::READABLE.add(Interest::WRITABLE)
        };
        self.poll
            .registry()
            .reregister(&mut self.public, PUBLIC_TOKEN, interest)?;
        Ok(())
    }

    #[cfg(test)]
    fn enqueue_public_datagram(&mut self, bytes: Vec<u8>, destination: SocketAddr) -> bool {
        self.enqueue_public_datagram_with_metadata(bytes, destination, 0, false, Instant::now())
    }

    fn enqueue_public_datagram_with_metadata(
        &mut self,
        bytes: Vec<u8>,
        destination: SocketAddr,
        inner_bytes: usize,
        match_worker: bool,
        worker_packet_started: Instant,
    ) -> bool {
        if self.outgoing.len() >= crate::GLOBAL_PACKET_QUEUE_FRAMES
            || self.outgoing_bytes.saturating_add(bytes.len()) > crate::GLOBAL_PACKET_QUEUE_BYTES
        {
            self.core.note_error(RoutingErrorCategory::PacketQueueFull);
            return false;
        }
        self.outgoing_bytes = self.outgoing_bytes.saturating_add(bytes.len());
        self.outgoing.push_back(PendingPublicDatagram {
            bytes,
            destination,
            inner_bytes,
            match_worker,
            worker_packet_started,
        });
        true
    }

    fn update_worker_interest(&mut self, worker_id: WorkerId) -> Result<(), RuntimeError> {
        let Some(worker) = self.workers.get_mut(&worker_id) else {
            return Ok(());
        };
        let Some(channels) = worker.channels.as_mut() else {
            return Ok(());
        };
        if let Some(token) = worker.packet_token {
            let interest = if channels.packet_pending() {
                Interest::READABLE.add(Interest::WRITABLE)
            } else {
                Interest::READABLE
            };
            self.poll
                .registry()
                .reregister(channels.packet_source_mut(), token, interest)?;
        }
        if let Some(token) = worker.control_token {
            let interest = if channels.control_pending() {
                Interest::READABLE.add(Interest::WRITABLE)
            } else {
                Interest::READABLE
            };
            self.poll
                .registry()
                .reregister(channels.control_source_mut(), token, interest)?;
        }
        Ok(())
    }

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

fn report_runtime_observations(report: &RuntimePollReport, elapsed: Duration) {
    for event in &report.lifecycle_events {
        report_lifecycle_event(event, elapsed);
    }
    for event in &report.timing_events {
        report_timing_event(event, elapsed);
    }
}

fn wall_clock_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

fn report_timing_event(event: &RuntimeTimingEvent, elapsed: Duration) {
    let timestamp_ms = wall_clock_millis();
    match event {
        RuntimeTimingEvent::AllocationAccepted {
            request_id,
            worker_id,
        } => {
            // Stable correlation IDs are sufficient for evidence. Capabilities and player
            // manifests deliberately never cross this logging boundary.
            eprintln!(
                "brawler-supervisor timing allocation-accepted request_id={} worker={} ts_ms={} elapsed_ms={}",
                request_id.get(),
                worker_id.get(),
                timestamp_ms,
                elapsed.as_millis(),
            );
        }
    }
}

fn report_lifecycle_event(event: &LifecycleEvent, elapsed: Duration) {
    let timestamp_ms = wall_clock_millis();
    let elapsed_ms = elapsed.as_millis();
    match event {
        LifecycleEvent::Spawned { worker_id, pid } => {
            eprintln!(
                "brawler-supervisor worker spawned worker={worker_id} pid={pid} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
            );
        }
        LifecycleEvent::Ready { worker_id } => {
            eprintln!(
                "brawler-supervisor worker ready worker={worker_id} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
            );
        }
        LifecycleEvent::HeartbeatSuspect { worker_id } => {
            eprintln!(
                "brawler-supervisor worker heartbeat-suspect worker={worker_id} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
            );
        }
        LifecycleEvent::HeartbeatRecovered { worker_id } => {
            eprintln!(
                "brawler-supervisor worker heartbeat-recovered worker={worker_id} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
            );
        }
        LifecycleEvent::ExitReceived { worker_id, .. } => {
            eprintln!(
                "brawler-supervisor worker exit-received worker={worker_id} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
            );
        }
        LifecycleEvent::ChildReaped { worker_id, status } => {
            eprintln!(
                "brawler-supervisor worker reaped worker={worker_id} success={} code={:?} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}",
                status.success, status.code,
            );
        }
        LifecycleEvent::Failed {
            worker_id,
            category,
        } => {
            eprintln!(
                "brawler-supervisor worker failed worker={worker_id} category={category:?} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
            );
        }
        LifecycleEvent::ForcedStop { worker_id } => {
            eprintln!(
                "brawler-supervisor worker forced-stop worker={worker_id} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
            );
        }
        LifecycleEvent::StopRequested { worker_id, stop_id } => {
            eprintln!(
                "brawler-supervisor worker stop-requested worker={worker_id} stop_id={} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}",
                stop_id.get(),
            );
        }
        LifecycleEvent::StopSent { worker_id, stop_id } => {
            eprintln!(
                "brawler-supervisor worker stop-sent worker={worker_id} stop_id={} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}",
                stop_id.get(),
            );
        }
        LifecycleEvent::Stopped { worker_id, forced } => {
            eprintln!(
                "brawler-supervisor worker stopped worker={worker_id} forced={forced} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
            );
        }
        LifecycleEvent::RestartScheduled { worker_id, after } => {
            eprintln!(
                "brawler-supervisor worker restart-scheduled worker={worker_id} after_ms={} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}",
                after.as_millis(),
            );
        }
        LifecycleEvent::RestartExhausted { worker_id } => {
            eprintln!(
                "brawler-supervisor worker restart-exhausted worker={worker_id} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
            );
        }
        LifecycleEvent::Cleaned { worker_id } => {
            eprintln!(
                "brawler-supervisor worker cleaned worker={worker_id} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
            );
        }
        LifecycleEvent::ManifestSent { .. } | LifecycleEvent::Control { .. } => {}
        LifecycleEvent::ResultReceived { worker_id, .. } => {
            // Result payload bytes and all route/capability material remain redacted; this marker
            // is only a bounded lifecycle assertion for process smoke logs.
            eprintln!(
                "brawler-supervisor worker result-received worker={worker_id} elapsed_ms={elapsed_ms} ts_ms={timestamp_ms}"
            );
        }
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
mod tests {
    use std::{
        net::UdpSocket,
        time::{Duration, Instant},
    };

    use mio::net::UnixStream;

    use crate::{
        AllocateRequestBody, AllocationId, GameMode, Generation, LobbySessionId, MatchId,
        MonotonicMillis, PacketDirection, PacketRecord, PeerCloseBody, PeerId, ProcessId,
        PublicEnvelope, RequestId, ResultBody, RouteId, RouteRegistration, RouteSelector,
        UnixWorkerChannels, WorkerId, WorkerKind, WorkerRegistration,
    };

    use super::*;

    fn id128(value: u128) -> WorkerId {
        WorkerId::new(value).unwrap()
    }

    fn registration(worker_id: u128, kind: WorkerKind) -> WorkerRegistration {
        WorkerRegistration {
            worker_id: id128(worker_id),
            process_id: ProcessId::new(worker_id + 100).unwrap(),
            generation: Generation::new(1).unwrap(),
            kind,
        }
    }

    fn route(route_id: u128, worker_id: WorkerId, default: bool) -> RouteRegistration {
        RouteRegistration {
            route_id: RouteId::new(route_id).unwrap(),
            worker_id,
            peer_id: PeerId::new(route_id + 1000).unwrap(),
            is_default_lobby: default,
        }
    }

    fn attach_worker(
        runtime: &mut SupervisorRuntime,
        worker: WorkerRegistration,
    ) -> UnixWorkerChannels {
        let (supervisor_packet, worker_packet) = UnixStream::pair().unwrap();
        let (supervisor_control, worker_control) = UnixStream::pair().unwrap();
        runtime
            .attach_worker_channels(
                worker.worker_id,
                UnixWorkerChannels::new(supervisor_packet, supervisor_control),
            )
            .unwrap();
        UnixWorkerChannels::new(worker_packet, worker_control)
    }

    fn send_worker_control(
        worker: &mut UnixWorkerChannels,
        registration: WorkerRegistration,
        sequence: u64,
        body: ControlBody,
    ) {
        let frame = ControlFrame::from_raw_sequence(
            sequence,
            registration.process_id,
            registration.worker_id,
            body,
        )
        .unwrap()
        .encode_framed()
        .unwrap();
        worker.enqueue_control(&frame).unwrap();
        worker.flush_control(1).unwrap();
    }

    #[test]
    fn public_udp_routes_opaque_payload_to_packet_ipc_and_back() {
        let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let worker = registration(1, WorkerKind::Lobby);
        let default_route = route(2, worker.worker_id, true);
        runtime.register_worker(worker).unwrap();
        runtime.core_mut().register_route(default_route).unwrap();
        let (supervisor_packet, worker_packet) = UnixStream::pair().unwrap();
        let (supervisor_control, worker_control) = UnixStream::pair().unwrap();
        runtime
            .attach_worker_channels(
                worker.worker_id,
                UnixWorkerChannels::new(supervisor_packet, supervisor_control),
            )
            .unwrap();
        let mut worker_channels = UnixWorkerChannels::new(worker_packet, worker_control);
        let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        client
            .set_nonblocking(true)
            .expect("client socket should be nonblocking");
        let public = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![7, 8, 9])
            .unwrap()
            .encode()
            .unwrap();
        client
            .send_to(&public, runtime.public_addr().unwrap())
            .unwrap();
        runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        let worker_packet = worker_channels
            .packet_read_ready(1)
            .unwrap()
            .records
            .pop()
            .expect("packet reached worker");
        let decoded =
            PacketRecord::decode(&worker_packet, PacketDirection::SupervisorToWorker).unwrap();
        assert_eq!(decoded.payload, vec![7, 8, 9]);
        let response = PacketRecord::new(
            PacketDirection::WorkerToSupervisor,
            worker.worker_id,
            decoded.route_id,
            decoded.peer_id,
            vec![4, 5],
        )
        .unwrap()
        .encode()
        .unwrap();
        worker_channels.enqueue_packet(&response).unwrap();
        worker_channels.flush_packet(1).unwrap();
        for _ in 0..3 {
            runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        }
        let mut received = [0; crate::PUBLIC_MAX_DATAGRAM_BYTES];
        let (length, _) = client.recv_from(&mut received).unwrap();
        assert_eq!(
            PublicEnvelope::decode(&received[..length])
                .unwrap()
                .payload(),
            &[4, 5]
        );
        let routing = runtime.metrics();
        assert_eq!(routing.public_ingress.datagrams, 1);
        assert_eq!(routing.public_ingress.frames, 1);
        assert_eq!(routing.public_ingress.bytes, public.len() as u64);
        assert_eq!(routing.inner_ingress.datagrams, 1);
        assert_eq!(routing.inner_ingress.frames, 1);
        assert_eq!(routing.inner_ingress.bytes, 3);
        assert_eq!(routing.ipc_to_worker.frames, 1);
        assert_eq!(
            routing.ipc_to_worker.bytes,
            (worker_packet.len() + 4) as u64
        );
        assert_eq!(routing.ipc_from_worker.frames, 1);
        assert_eq!(routing.ipc_from_worker.bytes, (response.len() + 4) as u64);
        assert_eq!(routing.public_egress.datagrams, 1);
        assert_eq!(routing.public_egress.frames, 1);
        assert_eq!(routing.public_egress.bytes, received[..length].len() as u64);
        assert_eq!(routing.inner_egress.datagrams, 1);
        assert_eq!(routing.inner_egress.frames, 1);
        assert_eq!(routing.inner_egress.bytes, 2);
        assert_eq!(routing.public_receive_to_packet_ipc_enqueue.count(), 1);
        assert_eq!(routing.worker_packet_to_public_send.count(), 1);
    }

    #[test]
    fn public_ipv6_udp_routes_dynamic_source_and_opaque_payload_to_packet_ipc_and_back() {
        let mut runtime =
            SupervisorRuntime::bind(SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], 0))).unwrap();
        let worker = registration(1, WorkerKind::Lobby);
        let default_route = route(2, worker.worker_id, true);
        runtime.register_worker(worker).unwrap();
        runtime.core_mut().register_route(default_route).unwrap();
        let (supervisor_packet, worker_packet) = UnixStream::pair().unwrap();
        let (supervisor_control, worker_control) = UnixStream::pair().unwrap();
        runtime
            .attach_worker_channels(
                worker.worker_id,
                UnixWorkerChannels::new(supervisor_packet, supervisor_control),
            )
            .unwrap();
        let mut worker_channels = UnixWorkerChannels::new(worker_packet, worker_control);
        let client = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], 0))).unwrap();
        client
            .set_nonblocking(true)
            .expect("IPv6 client socket should be nonblocking");
        let public = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![7, 8, 9])
            .unwrap()
            .encode()
            .unwrap();
        client
            .send_to(&public, runtime.public_addr().unwrap())
            .unwrap();
        runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        let worker_packet = worker_channels
            .packet_read_ready(1)
            .unwrap()
            .records
            .pop()
            .expect("IPv6 packet reached worker");
        let decoded =
            PacketRecord::decode(&worker_packet, PacketDirection::SupervisorToWorker).unwrap();
        assert_eq!(decoded.payload, vec![7, 8, 9]);
        assert_eq!(
            runtime.core().source_for_route(decoded.route_id),
            Some(client.local_addr().unwrap())
        );
        assert!(
            runtime
                .core()
                .source_for_route(decoded.route_id)
                .is_some_and(|source| source.is_ipv6())
        );
        let response = PacketRecord::new(
            PacketDirection::WorkerToSupervisor,
            worker.worker_id,
            decoded.route_id,
            decoded.peer_id,
            vec![4, 5],
        )
        .unwrap()
        .encode()
        .unwrap();
        worker_channels.enqueue_packet(&response).unwrap();
        worker_channels.flush_packet(1).unwrap();
        for _ in 0..3 {
            runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        }
        let mut received = [0; crate::PUBLIC_MAX_DATAGRAM_BYTES];
        let (length, source) = client.recv_from(&mut received).unwrap();
        assert!(source.is_ipv6());
        assert_eq!(source, runtime.public_addr().unwrap());
        assert_eq!(
            PublicEnvelope::decode(&received[..length])
                .unwrap()
                .payload(),
            &[4, 5]
        );
        assert_eq!(
            runtime.core().source_for_route(decoded.route_id),
            Some(client.local_addr().unwrap())
        );
    }

    #[test]
    fn public_traffic_before_lobby_ready_is_dropped_without_poll_failure() {
        let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let worker = registration(1, WorkerKind::Lobby);
        runtime.register_worker(worker).unwrap();
        let (supervisor_packet, _worker_packet) = UnixStream::pair().unwrap();
        let (supervisor_control, _worker_control) = UnixStream::pair().unwrap();
        runtime
            .attach_worker_channels(
                worker.worker_id,
                UnixWorkerChannels::new(supervisor_packet, supervisor_control),
            )
            .unwrap();
        let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let envelope = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![1, 2, 3])
            .unwrap()
            .encode()
            .unwrap();
        client
            .send_to(&envelope, runtime.public_addr().unwrap())
            .unwrap();

        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        assert_eq!(report.public_received, 1);
        assert_eq!(report.public_dropped, 1);
        assert_eq!(runtime.core().route_count(), 0);
        assert_eq!(runtime.core().metrics().packet_current.frames, 0);
    }

    #[test]
    fn lobby_startup_retries_do_not_spend_preauth_budget_before_ready() {
        let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let worker = registration(1, WorkerKind::Lobby);
        runtime.register_worker(worker).unwrap();
        let (supervisor_packet, _worker_packet) = UnixStream::pair().unwrap();
        let (supervisor_control, _worker_control) = UnixStream::pair().unwrap();
        runtime
            .attach_worker_channels(
                worker.worker_id,
                UnixWorkerChannels::new(supervisor_packet, supervisor_control),
            )
            .unwrap();
        let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let envelope = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![1; 1])
            .unwrap()
            .encode()
            .unwrap();

        // The route is not published until Ready. These retries are dropped without consuming
        // the source's 8-datagram/9-KiB pre-auth allowance.
        for _ in 0..crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW {
            client
                .send_to(&envelope, runtime.public_addr().unwrap())
                .unwrap();
        }
        let mut received = 0;
        let mut dropped = 0;
        for _ in 0..4 {
            let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
            received += report.public_received;
            dropped += report.public_dropped;
            if received == crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW {
                break;
            }
        }
        assert_eq!(received, crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW);
        assert_eq!(dropped, crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW);
        assert_eq!(runtime.core().metrics().source_limited, 0);
        assert_eq!(
            runtime.metrics().public_ingress.datagrams,
            crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW as u64
        );
        assert_eq!(
            runtime.metrics().inner_ingress.datagrams,
            crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW as u64
        );
        assert!(!runtime.core().default_lobby_ready());

        // Once Ready publishes the template, the same source receives the normal bounded budget.
        runtime
            .core_mut()
            .register_route(route(2, worker.worker_id, true))
            .unwrap();
        assert!(runtime.core().default_lobby_ready());
        for _ in 0..=crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW {
            client
                .send_to(&envelope, runtime.public_addr().unwrap())
                .unwrap();
        }
        let expected = crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW + 1;
        let mut received = 0;
        let mut dropped = 0;
        for _ in 0..4 {
            let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
            received += report.public_received;
            dropped += report.public_dropped;
            if received == expected {
                break;
            }
        }
        assert_eq!(received, expected);
        assert_eq!(dropped, 1);
        assert_eq!(runtime.core().metrics().source_limited, 1);
    }

    #[test]
    fn queued_rejection_is_reclaimed_so_request_ids_can_restart() {
        let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let worker = registration(1, WorkerKind::Lobby);
        runtime.register_worker(worker).unwrap();
        let request = AllocateRequestBody {
            request_id: RequestId::new(77).unwrap(),
            lobby_session_id: LobbySessionId::new(88).unwrap(),
            mode: GameMode::Wipeout,
            map_preset: 1,
            map_revision: 1,
            rules_profile: 1,
            objective_target: 10,
            match_duration_ticks: 10_800,
            countdown_ticks: 180,
            respawn_ticks: 180,
            team_count: 2,
            players_per_team: 2,
            participants: Vec::new(),
        };
        runtime.allocations.insert(
            request.request_id,
            AllocationRecord {
                request: request.clone(),
                lobby_worker_id: worker.worker_id,
                allocation_id: None,
                match_id: None,
                match_worker_id: None,
                participants: Vec::new(),
                response: Some(ControlBody::AllocationRejected(AllocationRejectedBody {
                    request_id: request.request_id,
                    reason: ALLOCATION_REJECT_INVALID,
                    retry_after_ms: 0,
                })),
                response_queued: false,
                result: None,
            },
        );
        runtime.queue_allocation_responses();
        assert!(runtime.allocations.is_empty());

        // A later session may safely reuse the same request ID after the prior response crossed
        // the bounded supervisor queue.
        runtime.allocations.insert(
            request.request_id,
            AllocationRecord {
                request,
                lobby_worker_id: worker.worker_id,
                allocation_id: None,
                match_id: None,
                match_worker_id: None,
                participants: Vec::new(),
                response: None,
                response_queued: false,
                result: None,
            },
        );
        assert_eq!(runtime.allocations.len(), 1);
    }

    #[test]
    fn shutdown_suppresses_pending_runtime_controls_but_still_expires_routes() {
        let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let worker = registration(1, WorkerKind::Lobby);
        runtime.register_worker(worker).unwrap();
        runtime
            .core_mut()
            .register_route(route(2, worker.worker_id, true))
            .unwrap();
        let envelope = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![1, 2, 3]).unwrap();
        let source = SocketAddr::from(([127, 0, 0, 1], 4000));
        runtime
            .core_mut()
            .route_public(&envelope, source, MonotonicMillis(1))
            .unwrap();
        assert_eq!(runtime.core().route_count(), 2);

        // Leave a runtime-owned response in the bounded control queue, then enter shutdown.
        // The lifecycle Stop is the only control allowed to be appended after this point.
        let response = ControlBody::AllocationRejected(AllocationRejectedBody {
            request_id: RequestId::new(77).unwrap(),
            reason: ALLOCATION_REJECT_INVALID,
            retry_after_ms: 0,
        });
        assert!(runtime.queue_control_body(worker.worker_id, response));
        let next_before_shutdown = runtime
            .workers
            .get(&worker.worker_id)
            .expect("worker is registered")
            .next_control_sequence;
        runtime.shutting_down = true;
        runtime.started = Instant::now()
            .checked_sub(Duration::from_millis(
                crate::PUBLIC_LOBBY_ROUTE_IDLE_MILLIS + 1,
            ))
            .expect("idle-route duration fits before the current instant");

        // Expiry revokes the dynamic route and queued packets, but does not emit PeerClose after
        // the lifecycle Stop boundary or advance the runtime control cursor.
        let mut report = RuntimePollReport::default();
        runtime.expire(&mut report);
        assert_eq!(report.routes_torn_down, 1);
        assert_eq!(runtime.core().route_count(), 1);
        assert_eq!(runtime.core().metrics().control_current.frames, 1);
        assert_eq!(
            runtime
                .workers
                .get(&worker.worker_id)
                .expect("worker remains registered")
                .next_control_sequence,
            next_before_shutdown
        );

        let pending_request = AllocateRequestBody {
            request_id: RequestId::new(78).unwrap(),
            lobby_session_id: LobbySessionId::new(88).unwrap(),
            mode: GameMode::Wipeout,
            map_preset: 1,
            map_revision: 1,
            rules_profile: 1,
            objective_target: 10,
            match_duration_ticks: 10_800,
            countdown_ticks: 180,
            respawn_ticks: 180,
            team_count: 2,
            players_per_team: 2,
            participants: Vec::new(),
        };
        runtime.allocations.insert(
            pending_request.request_id,
            AllocationRecord {
                request: pending_request.clone(),
                lobby_worker_id: worker.worker_id,
                allocation_id: None,
                match_id: None,
                match_worker_id: None,
                participants: Vec::new(),
                response: Some(ControlBody::AllocationRejected(AllocationRejectedBody {
                    request_id: pending_request.request_id,
                    reason: ALLOCATION_REJECT_INVALID,
                    retry_after_ms: 0,
                })),
                response_queued: false,
                result: None,
            },
        );

        // A pending allocation response is retained for bounded bookkeeping, but it cannot be
        // moved into the core queue once shutdown owns the stream ordering.
        runtime.queue_allocation_responses();
        assert_eq!(runtime.core().metrics().control_current.frames, 1);
    }

    #[test]
    fn malformed_public_datagram_is_dropped_without_worker_work() {
        let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let worker = registration(1, WorkerKind::Lobby);
        runtime.register_worker(worker).unwrap();
        runtime
            .core_mut()
            .register_route(route(2, worker.worker_id, true))
            .unwrap();
        let (supervisor_packet, _worker_packet) = UnixStream::pair().unwrap();
        let (supervisor_control, _worker_control) = UnixStream::pair().unwrap();
        runtime
            .attach_worker_channels(
                worker.worker_id,
                UnixWorkerChannels::new(supervisor_packet, supervisor_control),
            )
            .unwrap();
        let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        client
            .send_to(
                &[0xff; crate::PUBLIC_MAX_DATAGRAM_BYTES + 1],
                runtime.public_addr().unwrap(),
            )
            .unwrap();
        let mut dropped = 0;
        for _ in 0..3 {
            dropped += runtime
                .poll_once(Some(Duration::from_millis(10)))
                .unwrap()
                .public_dropped;
            if dropped == 1 {
                break;
            }
        }
        assert_eq!(dropped, 1);
        assert_eq!(runtime.core().metrics().packet_current.frames, 0);
    }

    #[test]
    fn public_default_flood_is_limited_before_route_queue_and_does_not_spawn_workers() {
        let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let worker = registration(1, WorkerKind::Lobby);
        runtime.register_worker(worker).unwrap();
        runtime
            .core_mut()
            .register_route(route(2, worker.worker_id, true))
            .unwrap();
        let (supervisor_packet, _worker_packet) = UnixStream::pair().unwrap();
        let (supervisor_control, _worker_control) = UnixStream::pair().unwrap();
        runtime
            .attach_worker_channels(
                worker.worker_id,
                UnixWorkerChannels::new(supervisor_packet, supervisor_control),
            )
            .unwrap();
        let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let envelope = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![1; 1])
            .unwrap()
            .encode()
            .unwrap();
        for _ in 0..=crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW {
            client
                .send_to(&envelope, runtime.public_addr().unwrap())
                .unwrap();
        }
        let expected = crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW + 1;
        let mut received = 0;
        let mut dropped = 0;
        for _ in 0..4 {
            let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
            received += report.public_received;
            dropped += report.public_dropped;
            if received == expected {
                break;
            }
        }
        assert_eq!(received, expected);
        assert_eq!(dropped, 1);
        assert_eq!(runtime.core().metrics().source_limited, 1);
        assert_eq!(runtime.core().route_count(), 2);
        assert_eq!(runtime.core().worker_count(), 1);
    }

    #[test]
    fn malformed_source_is_suppressed_without_allocating_workers_or_replies() {
        let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        client.set_nonblocking(true).unwrap();
        let malformed = [0xff; crate::PUBLIC_MAX_DATAGRAM_BYTES + 1];
        for _ in 0..crate::PUBLIC_MALFORMED_PER_WINDOW {
            client
                .send_to(&malformed, runtime.public_addr().unwrap())
                .unwrap();
        }
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        assert_eq!(report.public_received, crate::PUBLIC_MALFORMED_PER_WINDOW);
        assert_eq!(report.public_dropped, crate::PUBLIC_MALFORMED_PER_WINDOW);
        assert!(runtime.core().metrics().source_limited >= 1);
        assert_eq!(runtime.core().worker_count(), 0);
        let mut response = [0_u8; crate::PUBLIC_MAX_DATAGRAM_BYTES];
        assert!(client.recv_from(&mut response).is_err());
    }

    #[test]
    fn worker_to_public_queue_drops_newest_at_global_frame_bound() {
        let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let destination = SocketAddr::from(([127, 0, 0, 1], 9));
        for _ in 0..crate::GLOBAL_PACKET_QUEUE_FRAMES {
            assert!(runtime.enqueue_public_datagram(vec![1], destination));
        }
        assert!(!runtime.enqueue_public_datagram(vec![2], destination));
        assert_eq!(runtime.outgoing.len(), crate::GLOBAL_PACKET_QUEUE_FRAMES);
        assert_eq!(
            runtime.core().metrics().error_counts[&RoutingErrorCategory::PacketQueueFull],
            1
        );
    }

    #[test]
    fn invalid_peer_close_isolated_from_sibling_lobby() {
        let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let lobby = registration(1, WorkerKind::Lobby);
        let match_worker = registration(2, WorkerKind::Match);
        runtime.register_worker(lobby).unwrap();
        runtime.register_worker(match_worker).unwrap();
        let lobby_route = route(3, lobby.worker_id, true);
        let match_route = route(4, match_worker.worker_id, false);
        runtime.core_mut().register_route(lobby_route).unwrap();
        runtime.core_mut().register_route(match_route).unwrap();
        let lobby_io = attach_worker(&mut runtime, lobby);
        let mut match_io = attach_worker(&mut runtime, match_worker);

        send_worker_control(
            &mut match_io,
            match_worker,
            1,
            ControlBody::PeerClose(PeerCloseBody {
                route_id: match_route.route_id,
                peer_id: PeerId::new(match_route.peer_id.get() + 1).unwrap(),
                reason: 1,
            }),
        );
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();

        assert!(report.lifecycle_events.is_empty());
        assert_eq!(runtime.core().worker_count(), 1);
        assert_eq!(runtime.core().route_count(), 1);
        assert!(
            runtime
                .core()
                .public_selector_for_route(lobby_route.route_id)
                .is_some()
        );
        // Keep the surviving channel live so the test also proves its Mio registration was not
        // torn down with the failed match.
        drop(lobby_io);
    }

    #[test]
    fn invalid_lobby_authentication_isolated_from_sibling_match() {
        let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let lobby = registration(1, WorkerKind::Lobby);
        let match_worker = registration(2, WorkerKind::Match);
        runtime.register_worker(lobby).unwrap();
        runtime.register_worker(match_worker).unwrap();
        let lobby_route = route(3, lobby.worker_id, true);
        let match_route = route(4, match_worker.worker_id, false);
        runtime.core_mut().register_route(lobby_route).unwrap();
        runtime.core_mut().register_route(match_route).unwrap();
        let mut lobby_io = attach_worker(&mut runtime, lobby);
        let _match_io = attach_worker(&mut runtime, match_worker);

        send_worker_control(
            &mut lobby_io,
            lobby,
            1,
            ControlBody::LobbyAuthenticated(crate::LobbyAuthenticatedBody {
                route_id: lobby_route.route_id,
                peer_id: PeerId::new(lobby_route.peer_id.get() + 1).unwrap(),
                lobby_session_id: LobbySessionId::new(7).unwrap(),
                netcode_client_id: crate::NetcodeClientId::new(8).unwrap(),
            }),
        );
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();

        assert!(report.lifecycle_events.is_empty());
        assert_eq!(runtime.core().worker_count(), 1);
        assert_eq!(runtime.core().route_count(), 1);
    }

    #[test]
    fn invalid_match_result_isolated_from_sibling_lobby() {
        let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let lobby = registration(1, WorkerKind::Lobby);
        let match_worker = registration(2, WorkerKind::Match);
        runtime.register_worker(lobby).unwrap();
        runtime.register_worker(match_worker).unwrap();
        let lobby_route = route(3, lobby.worker_id, true);
        let match_route = route(4, match_worker.worker_id, false);
        runtime.core_mut().register_route(lobby_route).unwrap();
        runtime.core_mut().register_route(match_route).unwrap();
        let _lobby_io = attach_worker(&mut runtime, lobby);
        let mut match_io = attach_worker(&mut runtime, match_worker);

        send_worker_control(
            &mut match_io,
            match_worker,
            1,
            ControlBody::Result(
                ResultBody::new(
                    MatchId::new(99).unwrap(),
                    AllocationId::new(100).unwrap(),
                    vec![1],
                )
                .unwrap(),
            ),
        );
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();

        assert!(report.lifecycle_events.is_empty());
        assert_eq!(runtime.core().worker_count(), 1);
        assert_eq!(runtime.core().route_count(), 1);
    }

    #[test]
    fn lobby_capacity_is_scalar_idempotent_and_tracks_registered_match_workers() {
        let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let lobby = registration(1, WorkerKind::Lobby);
        runtime.register_worker(lobby).unwrap();
        {
            let worker = runtime.workers.get_mut(&lobby.worker_id).unwrap();
            worker.match_slot_limit = Some(3);
            worker.pending_default_route = None;
        }
        runtime.refresh_lobby_capacity();
        let first = runtime.core_mut().drain_controls(4);
        assert_eq!(first.len(), 1);
        assert!(matches!(
            ControlFrame::decode(&first[0].1).unwrap().body,
            ControlBody::LobbyCapacity(crate::LobbyCapacityBody {
                free_match_slots: 3
            })
        ));
        runtime.refresh_lobby_capacity();
        assert!(runtime.core_mut().drain_controls(4).is_empty());

        runtime
            .register_worker(registration(2, WorkerKind::Match))
            .unwrap();
        runtime.refresh_lobby_capacity();
        let second = runtime.core_mut().drain_controls(4);
        assert!(matches!(
            ControlFrame::decode(&second[0].1).unwrap().body,
            ControlBody::LobbyCapacity(crate::LobbyCapacityBody {
                free_match_slots: 2
            })
        ));
    }
}
