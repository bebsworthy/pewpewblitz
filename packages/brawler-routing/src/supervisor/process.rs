//! Safe child-process supervision for routed workers.
//!
//! This module deliberately owns only process and IPC lifecycle.  It does not know about Bevy,
//! Lightyear, Netcode, or gameplay state.  A caller supplies a validated manifest and receives
//! lifecycle events; route/capability cleanup remains an explicit `SupervisorCore` operation via
//! [`ProcessSupervisor::poll_with_core`].

use std::{
    collections::{HashMap, VecDeque},
    ffi::OsString,
    fmt, io,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::{
    CodecError, ControlBody, ControlFrame, ControlSequenceTracker, ExitBody, Generation,
    IpcIoError, ManifestBody, ManifestCommon, MatchManifestV1, PrivateRuntimeDir, ProcessId,
    ResultBody, RoutingErrorCategory, SequenceDisposition, StopBody, StopId, SupervisorCore,
    UnixWorkerChannels, UnixWorkerListeners, WorkerId, WorkerKind, WorkerRegistration, WorkerRole,
};

// Keep this import local to the Unix-only IPC implementation.  The package already uses Unix
// domain sockets in `ipc.rs`; no platform-specific process signalling or unsafe code is needed.
use mio::net::UnixStream;

/// How a supervised worker's stderr is connected.  Worker stdout is always redirected to null;
/// it is never an IPC channel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StderrPolicy {
    /// Inherit the supervisor's operator-visible stderr.
    #[default]
    Inherit,
    /// Discard stderr for tests or deliberately quiet launchers.
    Null,
}

/// Descriptor policy applied to every child.  Keeping this as a named value makes the invariant
/// easy to inspect in tests and prevents a future worker launch from accidentally using stdout as
/// a binary transport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DescriptorPolicy {
    pub stdin_null: bool,
    pub stdout_null: bool,
    pub stderr: StderrPolicy,
}

impl DescriptorPolicy {
    #[must_use]
    pub const fn strict(stderr: StderrPolicy) -> Self {
        Self {
            stdin_null: true,
            stdout_null: true,
            stderr,
        }
    }
}

/// Settings for one process supervisor.  The durations are bounded by validation so callers
/// cannot accidentally turn a readiness/health contract into an unbounded wait.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessSupervisorConfig {
    pub logical_server_id: crate::LogicalServerId,
    pub supervisor_generation: Generation,
    pub network_protocol: u64,
    pub content_fingerprint: u64,
    pub max_workers: usize,
    pub poll_interval: Duration,
    pub ready_timeout: Duration,
    pub heartbeat_suspect_after: Duration,
    pub heartbeat_fail_after: Duration,
    pub graceful_stop: Duration,
    pub forced_reap: Duration,
    pub shutdown_deadline: Duration,
    pub stderr: StderrPolicy,
}

impl ProcessSupervisorConfig {
    #[must_use]
    pub fn new(
        logical_server_id: crate::LogicalServerId,
        supervisor_generation: Generation,
        network_protocol: u64,
        content_fingerprint: u64,
    ) -> Self {
        Self {
            logical_server_id,
            supervisor_generation,
            network_protocol,
            content_fingerprint,
            max_workers: crate::MAX_WORKERS,
            poll_interval: Duration::from_millis(10),
            ready_timeout: Duration::from_secs(5),
            heartbeat_suspect_after: Duration::from_secs(3),
            heartbeat_fail_after: Duration::from_secs(5),
            graceful_stop: Duration::from_secs(2),
            forced_reap: Duration::from_secs(1),
            shutdown_deadline: Duration::from_secs(5),
            stderr: StderrPolicy::Inherit,
        }
    }

    fn validate(&self) -> Result<(), LifecycleError> {
        if self.max_workers == 0
            || self.poll_interval.is_zero()
            || self.poll_interval > Duration::from_millis(100)
            || self.ready_timeout.is_zero()
            || self.heartbeat_suspect_after.is_zero()
            || self.heartbeat_fail_after < self.heartbeat_suspect_after
            || self.graceful_stop.is_zero()
            || self.forced_reap.is_zero()
            || self.shutdown_deadline < self.graceful_stop.saturating_add(self.forced_reap)
        {
            return Err(LifecycleError::InvalidConfiguration(
                "process lifecycle bounds are inconsistent",
            ));
        }
        Ok(())
    }
}

/// Immutable child launch description.  No arbitrary arguments are accepted: the generated argv
/// contains only role, stable identities, and private endpoint paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerLaunchSpec {
    pub executable: PathBuf,
    pub registration: WorkerRegistration,
    pub manifest: ManifestBody,
    pub stderr: Option<StderrPolicy>,
    /// Non-secret operator/test environment overrides.  This is kept separate from the argv
    /// contract; capabilities and manifests must never be placed here by callers.
    pub environment: Vec<(OsString, OsString)>,
}

impl WorkerLaunchSpec {
    #[must_use]
    pub fn new(
        executable: impl Into<PathBuf>,
        registration: WorkerRegistration,
        manifest: ManifestBody,
    ) -> Self {
        Self {
            executable: executable.into(),
            registration,
            manifest,
            stderr: None,
            environment: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_environment(
        mut self,
        key: impl Into<OsString>,
        value: impl Into<OsString>,
    ) -> Self {
        self.environment.push((key.into(), value.into()));
        self
    }

    #[must_use]
    pub fn descriptor_policy(&self, default_stderr: StderrPolicy) -> DescriptorPolicy {
        DescriptorPolicy::strict(match self.stderr {
            Some(stderr) => stderr,
            None => default_stderr,
        })
    }
}

/// Observable lifecycle phases.  `Failed` remains observable until the child is reaped and its
/// exact endpoint paths have been dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecyclePhase {
    Starting,
    Ready,
    Suspect,
    Stopping,
    Failed,
}

/// A compact, platform-neutral child status used in lifecycle evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessStatus {
    pub success: bool,
    pub code: Option<i32>,
}

impl From<ExitStatus> for ProcessStatus {
    fn from(status: ExitStatus) -> Self {
        Self {
            success: status.success(),
            code: status.code(),
        }
    }
}

/// Events emitted by [`ProcessSupervisor::poll`].  Events are bounded to one entry per lifecycle
/// transition or accepted control fact; a caller can feed terminal events directly to route
/// cleanup without decoding process internals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LifecycleEvent {
    Spawned {
        worker_id: WorkerId,
        pid: u32,
    },
    ManifestSent {
        worker_id: WorkerId,
    },
    Ready {
        worker_id: WorkerId,
    },
    HeartbeatSuspect {
        worker_id: WorkerId,
    },
    HeartbeatRecovered {
        worker_id: WorkerId,
    },
    ExitReceived {
        worker_id: WorkerId,
        exit: ExitBody,
    },
    ChildReaped {
        worker_id: WorkerId,
        status: ProcessStatus,
    },
    Failed {
        worker_id: WorkerId,
        category: RoutingErrorCategory,
    },
    StopSent {
        worker_id: WorkerId,
        stop_id: StopId,
    },
    /// A graceful stop was accepted by the lifecycle owner. This is emitted separately from
    /// [`Self::StopSent`] so evidence can measure both the request-to-send and send-to-reap
    /// portions without exposing control payloads.
    StopRequested {
        worker_id: WorkerId,
        stop_id: StopId,
    },
    ForcedStop {
        worker_id: WorkerId,
    },
    Stopped {
        worker_id: WorkerId,
        forced: bool,
    },
    RestartScheduled {
        worker_id: WorkerId,
        after: Duration,
    },
    RestartExhausted {
        worker_id: WorkerId,
    },
    Cleaned {
        worker_id: WorkerId,
    },
    /// A validated control body received from a worker.  The payload type is retained so the
    /// routing owner can expose allocation/result facts without decoding gameplay itself.
    Control {
        worker_id: WorkerId,
        body: ControlBody,
    },
    /// A validated match Result. It is emitted before the worker can enter graceful stopping.
    ResultReceived {
        worker_id: WorkerId,
        result: ResultBody,
    },
}

/// Errors from process admission, manifest validation, or bounded child polling.
#[derive(Debug)]
pub enum LifecycleError {
    Io(io::Error),
    Codec(CodecError),
    Routing(RoutingErrorCategory),
    InvalidConfiguration(&'static str),
    InvalidLaunch(&'static str),
    WorkerUnknown(WorkerId),
    WorkerAlreadyExists(WorkerId),
    Process { worker_id: WorkerId, detail: String },
    ShutdownTimeout,
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "worker lifecycle I/O error: {error}"),
            Self::Codec(error) => write!(formatter, "worker lifecycle codec error: {error}"),
            Self::Routing(category) => {
                write!(formatter, "worker lifecycle routing error: {category:?}")
            }
            Self::InvalidConfiguration(detail) => write!(
                formatter,
                "invalid worker lifecycle configuration: {detail}"
            ),
            Self::InvalidLaunch(detail) => write!(formatter, "invalid worker launch: {detail}"),
            Self::WorkerUnknown(worker_id) => write!(formatter, "unknown worker {worker_id}"),
            Self::WorkerAlreadyExists(worker_id) => {
                write!(formatter, "worker {worker_id} already exists")
            }
            Self::Process { worker_id, detail } => {
                write!(formatter, "worker {worker_id} process error: {detail}")
            }
            Self::ShutdownTimeout => {
                formatter.write_str("worker shutdown exceeded its bounded deadline")
            }
        }
    }
}

impl std::error::Error for LifecycleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Codec(_)
            | Self::Routing(_)
            | Self::InvalidConfiguration(_)
            | Self::InvalidLaunch(_)
            | Self::WorkerUnknown(_)
            | Self::WorkerAlreadyExists(_)
            | Self::Process { .. }
            | Self::ShutdownTimeout => None,
        }
    }
}

impl From<io::Error> for LifecycleError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<CodecError> for LifecycleError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

/// Shutdown accounting.  The report distinguishes normal Stop/Exit completion from forced
/// `Child::kill` escalation and never claims a child was reaped before `try_wait` confirmed it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShutdownReport {
    pub graceful: usize,
    pub forced: usize,
    pub reaped: usize,
    pub elapsed: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StopState {
    stop_id: StopId,
    queued: bool,
    sent: bool,
    requested_at: Instant,
    graceful_deadline: Instant,
    forced_deadline: Instant,
    forced: bool,
}

struct ManagedWorker {
    spec: WorkerLaunchSpec,
    child: Child,
    listeners: Option<UnixWorkerListeners>,
    pending_packet: Option<UnixStream>,
    pending_control: Option<UnixStream>,
    channels: Option<UnixWorkerChannels>,
    manifest_frame: Vec<u8>,
    manifest_digest: [u8; 32],
    manifest_sent: bool,
    phase: LifecyclePhase,
    ready_deadline: Instant,
    last_heartbeat: Option<Instant>,
    stop: Option<StopState>,
    sequences: ControlSequenceTracker,
    next_sequence: u64,
    exit: Option<ExitBody>,
    result: Option<ResultBody>,
    status: Option<ProcessStatus>,
    failure: Option<RoutingErrorCategory>,
    kill_requested: bool,
    /// When true, the surrounding runtime owns listeners/channels and this lifecycle object only
    /// owns the child, readiness deadlines, heartbeats, and queued control records.
    external_io: bool,
    pending_external_controls: VecDeque<Vec<u8>>,
}

struct PendingRestart {
    spec: WorkerLaunchSpec,
    due: Instant,
    external_io: bool,
}

/// Plain Rust owner of worker children and their two private IPC streams.
pub struct ProcessSupervisor {
    config: ProcessSupervisorConfig,
    runtime_dir: Option<PrivateRuntimeDir>,
    workers: HashMap<WorkerId, ManagedWorker>,
    pending_restarts: HashMap<WorkerId, PendingRestart>,
    restart_history: HashMap<WorkerId, VecDeque<Instant>>,
    admission_open: bool,
    shutdown_started: Option<Instant>,
    /// Lifecycle actions can happen between owner polls (for example a Result triggering Stop).
    /// Keep their bounded, secret-free observability records until the next poll returns them to
    /// the single owner loop.
    pending_events: Vec<LifecycleEvent>,
}

impl ProcessSupervisor {
    /// Create a supervisor with a fresh owner-only private runtime directory.
    pub fn new(config: ProcessSupervisorConfig) -> Result<Self, LifecycleError> {
        config.validate()?;
        Ok(Self::with_runtime_dir(config, PrivateRuntimeDir::create()?))
    }

    /// Use a runtime directory owned by a surrounding supervisor runtime.  Ownership is moved so
    /// socket cleanup still occurs exactly once when this process supervisor is dropped.
    #[must_use]
    pub fn with_runtime_dir(
        config: ProcessSupervisorConfig,
        runtime_dir: PrivateRuntimeDir,
    ) -> Self {
        Self {
            config,
            runtime_dir: Some(runtime_dir),
            workers: HashMap::new(),
            pending_restarts: HashMap::new(),
            restart_history: HashMap::new(),
            admission_open: true,
            shutdown_started: None,
            pending_events: Vec::new(),
        }
    }

    /// Construct lifecycle supervision for a surrounding Mio owner.  The caller must use
    /// [`Self::spawn_with_listeners`] so this object never creates a second owner for IPC streams.
    #[must_use]
    pub fn without_runtime_dir(config: ProcessSupervisorConfig) -> Self {
        Self {
            config,
            runtime_dir: None,
            workers: HashMap::new(),
            pending_restarts: HashMap::new(),
            restart_history: HashMap::new(),
            admission_open: true,
            shutdown_started: None,
            pending_events: Vec::new(),
        }
    }

    #[must_use]
    pub fn runtime_dir(&self) -> Option<&Path> {
        self.runtime_dir.as_ref().map(PrivateRuntimeDir::path)
    }

    #[must_use]
    pub const fn admission_open(&self) -> bool {
        self.admission_open
    }

    #[must_use]
    pub fn worker_phase(&self, worker_id: WorkerId) -> Option<LifecyclePhase> {
        self.workers.get(&worker_id).map(|worker| worker.phase)
    }

    /// Whether this worker already owns a queued or in-flight Stop request.  The surrounding
    /// runtime uses this to distinguish a pre-shutdown Stop event being drained from the new
    /// Stop sequence allocated by global shutdown.
    #[must_use]
    pub fn worker_is_stopping(&self, worker_id: WorkerId) -> bool {
        self.workers
            .get(&worker_id)
            .is_some_and(|worker| worker.stop.is_some())
    }

    #[must_use]
    pub fn worker_registration(&self, worker_id: WorkerId) -> Option<WorkerRegistration> {
        self.workers
            .get(&worker_id)
            .map(|worker| worker.spec.registration)
    }

    #[must_use]
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    /// Return external workers whose bounded restart backoff has elapsed. The surrounding Mio
    /// owner calls this on each lifecycle tick because an external restart cannot be started by
    /// [`Self::start_due_restarts`] without taking listener ownership away from that owner.
    #[must_use]
    pub fn due_external_restart_ids(&self, now: Instant) -> Vec<WorkerId> {
        let mut worker_ids = self
            .pending_restarts
            .iter()
            .filter_map(|(worker_id, pending)| {
                (pending.external_io && now >= pending.due).then_some(*worker_id)
            })
            .collect::<Vec<_>>();
        worker_ids.sort_by_key(|worker_id| worker_id.get());
        worker_ids
    }

    /// Return exactly the arguments admitted to a child process.  Secret-bearing manifests and
    /// capabilities never enter this vector.
    pub fn argv_for(
        &self,
        spec: &WorkerLaunchSpec,
        listeners: &UnixWorkerListeners,
    ) -> Vec<OsString> {
        let role = match spec.registration.kind {
            WorkerKind::Lobby => "lobby",
            WorkerKind::Match => "match",
        };
        vec![
            "--role".into(),
            role.into(),
            "--logical-server-id".into(),
            self.config.logical_server_id.get().to_string().into(),
            "--supervisor-generation".into(),
            self.config.supervisor_generation.get().to_string().into(),
            "--worker-id".into(),
            spec.registration.worker_id.get().to_string().into(),
            "--process-id".into(),
            spec.registration.process_id.get().to_string().into(),
            "--worker-generation".into(),
            spec.registration.generation.get().to_string().into(),
            "--packet-socket".into(),
            listeners.packet_path().as_os_str().to_owned(),
            "--control-socket".into(),
            listeners.control_path().as_os_str().to_owned(),
        ]
    }

    /// Spawn one worker.  Listener paths are created before `Command::spawn`; any failed launch
    /// drops both listeners and removes only those exact paths.
    pub fn spawn(&mut self, spec: WorkerLaunchSpec) -> Result<Vec<LifecycleEvent>, LifecycleError> {
        self.spawn_at(spec, None, Instant::now())
    }

    /// Spawn a child while transferring already-bound listeners through the lifecycle admission
    /// path.  The caller takes them back immediately with [`Self::take_worker_listeners`] and
    /// becomes their sole Mio owner; this object only supervises the child and control state.
    pub fn spawn_with_listeners(
        &mut self,
        spec: WorkerLaunchSpec,
        listeners: UnixWorkerListeners,
    ) -> Result<Vec<LifecycleEvent>, LifecycleError> {
        self.spawn_at(spec, Some(listeners), Instant::now())
    }

    fn spawn_at(
        &mut self,
        spec: WorkerLaunchSpec,
        supplied_listeners: Option<UnixWorkerListeners>,
        now: Instant,
    ) -> Result<Vec<LifecycleEvent>, LifecycleError> {
        if !self.admission_open {
            return Err(LifecycleError::InvalidLaunch("worker admission is closed"));
        }
        if self.workers.len() >= self.config.max_workers {
            return Err(LifecycleError::Routing(
                RoutingErrorCategory::AllocationCapacity,
            ));
        }
        if self.workers.contains_key(&spec.registration.worker_id)
            || self
                .pending_restarts
                .contains_key(&spec.registration.worker_id)
        {
            return Err(LifecycleError::WorkerAlreadyExists(
                spec.registration.worker_id,
            ));
        }
        let manifest_digest = self.validate_spec(&spec)?;
        let executable = validate_executable(&spec.executable)?;
        let external_io = supplied_listeners.is_some();
        let listeners = if let Some(listeners) = supplied_listeners {
            listeners
        } else {
            let runtime = self
                .runtime_dir
                .as_ref()
                .ok_or(LifecycleError::InvalidLaunch("runtime directory is closed"))?;
            UnixWorkerListeners::bind(runtime, spec.registration.worker_id)?
        };
        let args = self.argv_for(&spec, &listeners);
        let descriptor = spec.descriptor_policy(self.config.stderr);
        let mut command = Command::new(executable);
        command.args(args);
        command.envs(spec.environment.iter().map(|(key, value)| (key, value)));
        if descriptor.stdin_null {
            command.stdin(Stdio::null());
        }
        if descriptor.stdout_null {
            command.stdout(Stdio::null());
        }
        match descriptor.stderr {
            StderrPolicy::Inherit => {
                command.stderr(Stdio::inherit());
            }
            StderrPolicy::Null => {
                command.stderr(Stdio::null());
            }
        }
        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => return Err(LifecycleError::Io(error)),
        };
        let process_id = spec.registration.process_id;
        let worker_id = spec.registration.worker_id;
        let manifest_frame = ControlFrame::from_raw_sequence(
            1,
            process_id,
            worker_id,
            ControlBody::Manifest(spec.manifest.clone()),
        )?
        .encode()?;
        let pid = child.id();
        self.workers.insert(
            worker_id,
            ManagedWorker {
                spec,
                child,
                listeners: Some(listeners),
                pending_packet: None,
                pending_control: None,
                channels: None,
                manifest_frame,
                manifest_digest,
                manifest_sent: false,
                phase: LifecyclePhase::Starting,
                ready_deadline: now + self.config.ready_timeout,
                last_heartbeat: None,
                stop: None,
                sequences: ControlSequenceTracker::default(),
                next_sequence: 2,
                exit: None,
                result: None,
                status: None,
                failure: None,
                kill_requested: false,
                external_io,
                pending_external_controls: VecDeque::new(),
            },
        );
        Ok(vec![LifecycleEvent::Spawned { worker_id, pid }])
    }

    /// Take the exact listener pair after external spawn admission.  Once taken, this lifecycle
    /// object performs no listener/channel reads or writes for the worker.
    pub fn take_worker_listeners(
        &mut self,
        worker_id: WorkerId,
    ) -> Result<UnixWorkerListeners, LifecycleError> {
        let worker = self
            .workers
            .get_mut(&worker_id)
            .ok_or(LifecycleError::WorkerUnknown(worker_id))?;
        if !worker.external_io {
            return Err(LifecycleError::InvalidLaunch(
                "worker does not use externally-owned IPC",
            ));
        }
        worker.listeners.take().ok_or(LifecycleError::InvalidLaunch(
            "worker listeners already transferred",
        ))
    }

    /// Return the manifest frame for the surrounding owner to send after both `IPC` streams
    /// attach.
    #[must_use]
    pub fn worker_manifest(&self, worker_id: WorkerId) -> Option<Vec<u8>> {
        self.workers
            .get(&worker_id)
            .filter(|worker| worker.external_io && !worker.manifest_sent)
            .map(|worker| worker.manifest_frame.clone())
    }

    /// Mark the externally-owned manifest as queued on the control stream.
    pub fn mark_manifest_sent(&mut self, worker_id: WorkerId) -> Result<(), LifecycleError> {
        let worker = self
            .workers
            .get_mut(&worker_id)
            .ok_or(LifecycleError::WorkerUnknown(worker_id))?;
        worker.manifest_sent = true;
        Ok(())
    }

    /// Drain control records queued by lifecycle actions (currently Stop and route `PeerClose`) so
    /// the surrounding Mio owner can enqueue them on its sole worker channel.
    pub fn take_external_control_records(&mut self, worker_id: WorkerId) -> Vec<Vec<u8>> {
        self.workers
            .get_mut(&worker_id)
            .map(|worker| worker.pending_external_controls.drain(..).collect())
            .unwrap_or_default()
    }

    /// Feed one already-decoded, identity-validated worker control frame from an external owner.
    /// Lifecycle transitions and the typed body event remain owned here.
    pub fn observe_control_frame(
        &mut self,
        worker_id: WorkerId,
        frame: &ControlFrame,
        now: Instant,
    ) -> Result<Vec<LifecycleEvent>, LifecycleError> {
        let body = frame.body.clone();
        let mut events = Vec::new();
        let failure = {
            let worker = self
                .workers
                .get_mut(&worker_id)
                .ok_or(LifecycleError::WorkerUnknown(worker_id))?;
            if !worker.external_io {
                return Err(LifecycleError::InvalidLaunch(
                    "worker control is not externally owned",
                ));
            }
            if matches!(
                worker
                    .sequences
                    .observe(frame.clone())
                    .map_err(LifecycleError::Codec)?,
                SequenceDisposition::Duplicate
            ) {
                return Ok(events);
            }
            process_control_frame(worker, frame, now, &mut events).err()
        };
        if let Some(category) = failure {
            if category != RoutingErrorCategory::WorkerReportedFailure {
                return Err(LifecycleError::Routing(category));
            }
            events.push(LifecycleEvent::Control { worker_id, body });
            self.mark_failed(worker_id, category, &mut events);
            return Ok(events);
        }
        events.push(LifecycleEvent::Control { worker_id, body });
        Ok(events)
    }

    /// Take a due restart specification for a surrounding Mio owner.  Internal process
    /// supervision keeps the backoff/capacity decision; the caller recreates listeners and calls
    /// [`Self::spawn_with_listeners`] so stream ownership never changes hands implicitly.
    pub fn take_due_external_restart(
        &mut self,
        worker_id: WorkerId,
        now: Instant,
    ) -> Result<Option<WorkerLaunchSpec>, LifecycleError> {
        let Some(pending) = self.pending_restarts.get(&worker_id) else {
            return Ok(None);
        };
        if !pending.external_io || now < pending.due {
            return Ok(None);
        }
        let pending = self
            .pending_restarts
            .remove(&worker_id)
            .expect("pending restart was checked");
        Ok(Some(pending.spec))
    }

    /// Mark an externally-owned worker failed after its owner rejects malformed packet/control
    /// bytes.  Child cleanup remains on the normal bounded reap path.
    pub fn fail_worker(
        &mut self,
        worker_id: WorkerId,
        category: RoutingErrorCategory,
    ) -> Result<Vec<LifecycleEvent>, LifecycleError> {
        let mut events = Vec::new();
        self.mark_failed(worker_id, category, &mut events);
        Ok(events)
    }

    /// Poll listeners, bounded IPC, heartbeats, deadlines, and child status.  `try_wait` is
    /// intentionally issued on every call (the caller's poll cadence is capped at 100 ms).
    pub fn poll(&mut self) -> Result<Vec<LifecycleEvent>, LifecycleError> {
        self.poll_at(Instant::now())
    }

    /// Deterministic-clock variant used by lifecycle tests and by an owner loop that already has
    /// a monotonic timestamp.
    pub fn poll_at(&mut self, now: Instant) -> Result<Vec<LifecycleEvent>, LifecycleError> {
        let mut events = std::mem::take(&mut self.pending_events);
        self.start_due_restarts(now, &mut events)?;
        let worker_ids: Vec<_> = self.workers.keys().copied().collect();
        for worker_id in worker_ids {
            if self.workers.contains_key(&worker_id) {
                self.poll_worker(worker_id, now, &mut events)?;
            }
        }
        self.apply_shutdown_deadlines(now, &mut events);
        Ok(events)
    }

    /// Poll and perform exact route/capability cleanup for terminal worker observations.  This is
    /// intentionally a separate method so the lifecycle module remains usable without a routing
    /// registry in deterministic tests.
    pub fn poll_with_core(
        &mut self,
        core: &mut SupervisorCore,
    ) -> Result<Vec<LifecycleEvent>, LifecycleError> {
        let events = self.poll()?;
        for event in &events {
            if let LifecycleEvent::Failed { worker_id, .. }
            | LifecycleEvent::Stopped { worker_id, .. }
            | LifecycleEvent::ChildReaped { worker_id, .. } = event
            {
                let _ = core.cleanup_worker(*worker_id);
            }
        }
        Ok(events)
    }

    /// Send one idempotent Stop request.  Repeating the same `StopId` never queues another frame;
    /// presenting a different `StopId` while stopping is rejected as a lifecycle conflict.
    pub fn stop_worker(
        &mut self,
        worker_id: WorkerId,
        stop_id: StopId,
        reason: u16,
    ) -> Result<bool, LifecycleError> {
        self.stop_worker_at(worker_id, stop_id, reason, Instant::now())
    }

    /// Synchronize the next supervisor-to-worker control sequence with an external owner that
    /// also emits typed control frames.  Runtime-owned BRCT records (for example lobby
    /// authentication and `PeerClose`) advance their own sequence cursor, while this lifecycle
    /// object owns `Stop`. `Stop` must be allocated after every already-queued runtime record or a
    /// lobby can reject the valid shutdown frame as a stale sequence.
    pub fn sync_external_next_sequence(
        &mut self,
        worker_id: WorkerId,
        next_sequence: u64,
    ) -> Result<(), LifecycleError> {
        let worker = self
            .workers
            .get_mut(&worker_id)
            .ok_or(LifecycleError::WorkerUnknown(worker_id))?;
        if !worker.external_io {
            return Err(LifecycleError::InvalidLaunch(
                "worker does not use externally-owned IPC",
            ));
        }
        worker.next_sequence = worker.next_sequence.max(next_sequence.max(2));
        Ok(())
    }

    pub fn stop_worker_at(
        &mut self,
        worker_id: WorkerId,
        stop_id: StopId,
        reason: u16,
        now: Instant,
    ) -> Result<bool, LifecycleError> {
        let graceful_stop = self.config.graceful_stop;
        let worker = self
            .workers
            .get_mut(&worker_id)
            .ok_or(LifecycleError::WorkerUnknown(worker_id))?;
        if let Some(previous) = worker.stop {
            if previous.stop_id == stop_id {
                return Ok(false);
            }
            return Err(LifecycleError::Routing(
                RoutingErrorCategory::WorkerProtocolConflict,
            ));
        }
        worker.phase = LifecyclePhase::Stopping;
        worker.stop = Some(StopState {
            stop_id,
            queued: false,
            sent: false,
            requested_at: now,
            graceful_deadline: now + self.config.graceful_stop,
            forced_deadline: now + self.config.graceful_stop + self.config.forced_reap,
            forced: false,
        });
        queue_stop(worker, reason, graceful_stop)?;
        self.pending_events
            .push(LifecycleEvent::StopRequested { worker_id, stop_id });
        Ok(true)
    }

    /// Confirm that the externally-owned control writer has fully flushed its queued Stop frame.
    /// Queue insertion is not a send boundary: the runtime calls this only after the framed writer
    /// reports no pending control bytes.
    pub fn mark_external_stop_sent(&mut self, worker_id: WorkerId) -> Result<bool, LifecycleError> {
        let worker = self
            .workers
            .get_mut(&worker_id)
            .ok_or(LifecycleError::WorkerUnknown(worker_id))?;
        if !worker.external_io {
            return Err(LifecycleError::InvalidLaunch(
                "worker does not use externally-owned IPC",
            ));
        }
        let Some(stop_id) = mark_stop_sent(worker) else {
            return Ok(false);
        };
        // Preserve causal order with StopRequested. External IO is flushed after the lifecycle
        // poll that requested the stop, so both facts are published together on the next poll.
        self.pending_events
            .push(LifecycleEvent::StopSent { worker_id, stop_id });
        Ok(true)
    }

    /// Begin bounded shutdown: admission closes, pending lobby restarts are cancelled, and each
    /// live child receives one Stop.  Call [`Self::poll`] until the returned report is complete.
    pub fn begin_shutdown(&mut self) -> Result<Vec<LifecycleEvent>, LifecycleError> {
        self.begin_shutdown_at(Instant::now())
    }

    pub fn begin_shutdown_at(
        &mut self,
        now: Instant,
    ) -> Result<Vec<LifecycleEvent>, LifecycleError> {
        if self.shutdown_started.is_some() {
            return Ok(Vec::new());
        }
        self.admission_open = false;
        self.pending_restarts.clear();
        self.shutdown_started = Some(now);
        let mut events = Vec::new();
        let ids: Vec<_> = self.workers.keys().copied().collect();
        for worker_id in ids {
            let stop_id = random_stop_id()?;
            let _ = self.stop_worker_at(worker_id, stop_id, 1, now)?;
        }
        events.extend(self.poll_at(now)?);
        Ok(events)
    }

    /// Drive a bounded five-second shutdown loop.  The method performs no blocking operation on a
    /// worker stream; the only sleep is the owner-loop interval (at most 100 ms, normally 10 ms).
    pub fn shutdown(&mut self) -> Result<ShutdownReport, LifecycleError> {
        let started = Instant::now();
        let mut report = ShutdownReport::default();
        let _ = self.begin_shutdown_at(started)?;
        while !self.workers.is_empty() && started.elapsed() < self.config.shutdown_deadline {
            let before = self.workers.len();
            let events = self.poll_at(Instant::now())?;
            for event in events {
                match event {
                    LifecycleEvent::Stopped { forced, .. } => {
                        if forced {
                            report.forced += 1;
                        } else {
                            report.graceful += 1;
                        }
                    }
                    LifecycleEvent::ChildReaped { .. } => report.reaped += 1,
                    _ => {}
                }
            }
            if self.workers.len() == before {
                thread::sleep(self.config.poll_interval.min(Duration::from_millis(100)));
            }
        }
        report.elapsed = started.elapsed();
        if !self.workers.is_empty() {
            return Err(LifecycleError::ShutdownTimeout);
        }
        Ok(report)
    }

    #[allow(clippy::too_many_lines)]
    fn poll_worker(
        &mut self,
        worker_id: WorkerId,
        now: Instant,
        events: &mut Vec<LifecycleEvent>,
    ) -> Result<(), LifecycleError> {
        let mut failure = None;
        let mut child_status = None;
        let graceful_stop = self.config.graceful_stop;
        {
            let worker = self.workers.get_mut(&worker_id).expect("worker exists");
            if worker.failure.is_none() && worker.status.is_none() {
                if worker.external_io {
                    // The surrounding Mio owner reads/writes both streams and feeds decoded
                    // control frames through `observe_control_frame`.
                } else {
                    if let Some(listeners) = worker.listeners.as_mut() {
                        if worker.pending_packet.is_none() {
                            worker.pending_packet = listeners.accept_packet()?;
                        }
                        if worker.pending_control.is_none() {
                            worker.pending_control = listeners.accept_control()?;
                        }
                    }
                    if worker.channels.is_none()
                        && worker.pending_packet.is_some()
                        && worker.pending_control.is_some()
                    {
                        let packet = worker.pending_packet.take().expect("packet checked");
                        let control = worker.pending_control.take().expect("control checked");
                        let mut channels = UnixWorkerChannels::new(packet, control);
                        if let Err(error) = channels.enqueue_control(&worker.manifest_frame) {
                            failure = Some(map_ipc_error(error, IpcChannelKind::Control));
                        } else {
                            worker.manifest_sent = true;
                            worker.channels = Some(channels);
                            if let Err(error) = queue_stop(worker, 1, graceful_stop) {
                                failure = Some(RoutingErrorCategory::IpcControlClosed);
                                let _ = error;
                            }
                            events.push(LifecycleEvent::ManifestSent { worker_id });
                        }
                    }
                    let mut control_records = Vec::new();
                    if failure.is_none()
                        && let Some(channels) = worker.channels.as_mut()
                    {
                        match channels.control_read_ready(16) {
                            Ok(progress) => {
                                if progress.eof
                                    && worker.exit.is_none()
                                    && !worker.stop.is_some_and(|stop| stop.forced)
                                {
                                    failure = Some(RoutingErrorCategory::IpcControlClosed);
                                }
                                control_records = progress.records;
                            }
                            Err(error) => {
                                failure = Some(map_ipc_error(error, IpcChannelKind::Control));
                            }
                        }
                        if failure.is_none()
                            && !worker.stop.is_some_and(|stop| stop.forced)
                            && let Err(error) = channels.flush_control(16)
                        {
                            failure = Some(map_ipc_error(error, IpcChannelKind::Control));
                        }
                        if failure.is_none()
                            && !channels.control_pending()
                            && let Some(stop_id) = mark_stop_sent(worker)
                        {
                            events.push(LifecycleEvent::StopSent { worker_id, stop_id });
                        }
                    }
                    // Process decoded records after releasing the mutable channel borrow.  This keeps
                    // the stream owner and lifecycle state independently borrowable and makes the
                    // deferred-control boundary explicit.
                    for raw in control_records {
                        if failure.is_some() {
                            break;
                        }
                        match ControlFrame::decode_for(
                            &raw,
                            worker.spec.registration.process_id,
                            worker.spec.registration.worker_id,
                        ) {
                            Ok(frame) => match worker.sequences.observe(frame.clone()) {
                                Ok(SequenceDisposition::Duplicate) => {}
                                Ok(SequenceDisposition::Accepted) => {
                                    if let Err(category) =
                                        process_control_frame(worker, &frame, now, events)
                                    {
                                        if category == RoutingErrorCategory::WorkerReportedFailure {
                                            events.push(LifecycleEvent::Control {
                                                worker_id,
                                                body: frame.body.clone(),
                                            });
                                        }
                                        failure = Some(category);
                                    } else {
                                        events.push(LifecycleEvent::Control {
                                            worker_id,
                                            body: frame.body.clone(),
                                        });
                                    }
                                }
                                Err(_) => {
                                    failure = Some(RoutingErrorCategory::IpcMalformed);
                                }
                            },
                            Err(_) => {
                                failure = Some(RoutingErrorCategory::IpcMalformed);
                            }
                        }
                    }
                }
                if failure.is_none()
                    && let Some(status) = worker.child.try_wait()?
                {
                    let compact = ProcessStatus::from(status);
                    worker.status = Some(compact);
                    child_status = Some(compact);
                }
                if failure.is_none() && worker.status.is_none() {
                    match worker.phase {
                        LifecyclePhase::Starting if now >= worker.ready_deadline => {
                            failure = Some(RoutingErrorCategory::WorkerReadyTimeout);
                        }
                        LifecyclePhase::Ready | LifecyclePhase::Suspect
                            if worker.last_heartbeat.is_some_and(|last| {
                                now.duration_since(last) >= self.config.heartbeat_fail_after
                            }) =>
                        {
                            failure = Some(RoutingErrorCategory::HeartbeatTimeout);
                        }
                        _ => {}
                    }
                    if failure.is_none()
                        && worker.phase == LifecyclePhase::Ready
                        && worker.last_heartbeat.is_some_and(|last| {
                            now.duration_since(last) >= self.config.heartbeat_suspect_after
                        })
                    {
                        worker.phase = LifecyclePhase::Suspect;
                        events.push(LifecycleEvent::HeartbeatSuspect { worker_id });
                    }
                }
            }
        }
        if let Some(mut category) = failure {
            // A process that closes control while crashing should be classified from the OS
            // status, not as a mere stream EOF.  The status is sampled before `kill` so a
            // supervisor-induced forced stop remains distinguishable from a worker crash.
            if category == RoutingErrorCategory::IpcControlClosed
                && let Some(worker) = self.workers.get_mut(&worker_id)
                && worker.status.is_none()
                && let Some(status) = worker.child.try_wait()?
            {
                let compact = ProcessStatus::from(status);
                worker.status = Some(compact);
                child_status = Some(compact);
                if !compact.success && worker.exit.is_none() {
                    category = RoutingErrorCategory::WorkerCrash;
                }
            }
            self.mark_failed(worker_id, category, events);
        }
        if child_status.is_none()
            && let Some(worker) = self.workers.get_mut(&worker_id)
            && worker.status.is_none()
            && let Some(status) = worker.child.try_wait()?
        {
            let compact = ProcessStatus::from(status);
            worker.status = Some(compact);
            child_status = Some(compact);
        }
        if child_status.is_some() {
            self.reconcile_reaped(worker_id, now, events)?;
        }
        self.force_due_stop(worker_id, now, events);
        Ok(())
    }

    fn mark_failed(
        &mut self,
        worker_id: WorkerId,
        category: RoutingErrorCategory,
        events: &mut Vec<LifecycleEvent>,
    ) {
        let worker = self.workers.get_mut(&worker_id).expect("worker exists");
        if worker.failure.is_some() || worker.status.is_some() {
            return;
        }
        worker.failure = Some(category);
        worker.phase = LifecyclePhase::Failed;
        worker.listeners.take();
        worker.pending_packet.take();
        worker.pending_control.take();
        worker.channels.take();
        if !worker.kill_requested {
            let _ = worker.child.kill();
            worker.kill_requested = true;
        }
        events.push(LifecycleEvent::Failed {
            worker_id,
            category,
        });
    }

    fn force_due_stop(
        &mut self,
        worker_id: WorkerId,
        now: Instant,
        events: &mut Vec<LifecycleEvent>,
    ) {
        let Some(worker) = self.workers.get_mut(&worker_id) else {
            return;
        };
        let Some(stop) = worker.stop.as_mut() else {
            return;
        };
        if worker.status.is_some() || worker.failure.is_some() {
            return;
        }
        if !stop.forced && now >= stop.graceful_deadline {
            let _ = worker.child.kill();
            worker.kill_requested = true;
            stop.forced = true;
            events.push(LifecycleEvent::ForcedStop { worker_id });
        }
        // This second deadline is an evidence boundary. `poll_worker` continues trying `try_wait`
        // until the overall shutdown deadline; it never forgets an unreaped child.
        let _ = stop.forced_deadline;
    }

    fn reconcile_reaped(
        &mut self,
        worker_id: WorkerId,
        now: Instant,
        events: &mut Vec<LifecycleEvent>,
    ) -> Result<(), LifecycleError> {
        let Some(worker) = self.workers.get(&worker_id) else {
            return Ok(());
        };
        let status = worker.status.expect("reap status checked");
        events.push(LifecycleEvent::ChildReaped { worker_id, status });
        let forced = worker.stop.is_some_and(|stop| stop.forced) || worker.kill_requested;
        let failure = worker.failure.or_else(|| {
            if forced && worker.stop.is_some() {
                None
            } else if !status.success {
                match worker.exit {
                    Some(exit) if exit.exit_category != 0 => {
                        Some(RoutingErrorCategory::WorkerReportedFailure)
                    }
                    Some(_) => Some(RoutingErrorCategory::WorkerExitMismatch),
                    None => Some(RoutingErrorCategory::WorkerCrash),
                }
            } else {
                match worker.exit {
                    None => Some(RoutingErrorCategory::WorkerExitMismatch),
                    Some(exit) if exit.exit_category == 0 => None,
                    Some(_) => Some(RoutingErrorCategory::WorkerReportedFailure),
                }
            }
        });
        let kind = worker.spec.registration.kind;
        let external_io = worker.external_io;
        let stop_was_requested = worker.stop.is_some();
        let spec = worker.spec.clone();
        if let Some(category) = failure {
            if worker.failure.is_none() {
                events.push(LifecycleEvent::Failed {
                    worker_id,
                    category,
                });
            }
            self.workers.remove(&worker_id);
            events.push(LifecycleEvent::Cleaned { worker_id });
            if kind == WorkerKind::Lobby && !stop_was_requested && self.shutdown_started.is_none() {
                self.schedule_restart(worker_id, spec, external_io, now, events)?;
            }
        } else {
            self.workers.remove(&worker_id);
            events.push(LifecycleEvent::Stopped { worker_id, forced });
            events.push(LifecycleEvent::Cleaned { worker_id });
        }
        Ok(())
    }

    fn schedule_restart(
        &mut self,
        worker_id: WorkerId,
        spec: WorkerLaunchSpec,
        external_io: bool,
        now: Instant,
        events: &mut Vec<LifecycleEvent>,
    ) -> Result<(), LifecycleError> {
        let history = self.restart_history.entry(worker_id).or_default();
        while history
            .front()
            .is_some_and(|at| now.duration_since(*at) >= Duration::from_mins(1))
        {
            history.pop_front();
        }
        if history.len() >= 3 {
            events.push(LifecycleEvent::RestartExhausted { worker_id });
            return Ok(());
        }
        history.push_back(now);
        let refreshed = refresh_spec(&self.config, spec)?;
        self.pending_restarts.insert(
            worker_id,
            PendingRestart {
                spec: refreshed,
                due: now + Duration::from_secs(1),
                external_io,
            },
        );
        events.push(LifecycleEvent::RestartScheduled {
            worker_id,
            after: Duration::from_secs(1),
        });
        Ok(())
    }

    fn start_due_restarts(
        &mut self,
        now: Instant,
        events: &mut Vec<LifecycleEvent>,
    ) -> Result<(), LifecycleError> {
        let due: Vec<_> = self
            .pending_restarts
            .iter()
            .filter_map(|(worker_id, pending)| (now >= pending.due).then_some(*worker_id))
            .collect();
        for worker_id in due {
            if self
                .pending_restarts
                .get(&worker_id)
                .is_some_and(|pending| pending.external_io)
            {
                continue;
            }
            let pending = self
                .pending_restarts
                .remove(&worker_id)
                .expect("due restart exists");
            events.extend(self.spawn_at(pending.spec, None, now)?);
        }
        Ok(())
    }

    fn apply_shutdown_deadlines(&mut self, now: Instant, events: &mut Vec<LifecycleEvent>) {
        let Some(started) = self.shutdown_started else {
            return;
        };
        if now.duration_since(started) >= self.config.graceful_stop {
            let ids: Vec<_> = self.workers.keys().copied().collect();
            for worker_id in ids {
                self.force_due_stop(worker_id, now, events);
            }
        }
    }

    fn validate_spec(&self, spec: &WorkerLaunchSpec) -> Result<[u8; 32], LifecycleError> {
        spec.manifest.validate()?;
        let expected_role = match spec.registration.kind {
            WorkerKind::Lobby => WorkerRole::Lobby,
            WorkerKind::Match => WorkerRole::Match,
        };
        if spec.manifest.role != expected_role {
            return Err(LifecycleError::Routing(
                RoutingErrorCategory::ManifestIdentity,
            ));
        }
        let common = decode_manifest_common(&spec.manifest)?;
        if common.logical_server_id != self.config.logical_server_id
            || common.process_id != spec.registration.process_id
            || common.worker_id != spec.registration.worker_id
            || common.generation != spec.registration.generation
            || common.network_protocol != self.config.network_protocol
            || common.content_fingerprint != self.config.content_fingerprint
        {
            return Err(LifecycleError::Routing(
                RoutingErrorCategory::ManifestIncompatible,
            ));
        }
        manifest_digest_from_decoded(&spec.manifest)
    }
}

fn process_control_frame(
    worker: &mut ManagedWorker,
    frame: &ControlFrame,
    now: Instant,
    events: &mut Vec<LifecycleEvent>,
) -> Result<(), RoutingErrorCategory> {
    match &frame.body {
        ControlBody::Ready(ready) => {
            if !worker.manifest_sent
                || worker.phase != LifecyclePhase::Starting
                || ready.generation != worker.spec.registration.generation
                || ready.manifest_digest != worker.manifest_digest
            {
                return Err(RoutingErrorCategory::ManifestIncompatible);
            }
            worker.phase = LifecyclePhase::Ready;
            worker.last_heartbeat = Some(now);
            events.push(LifecycleEvent::Ready {
                worker_id: worker.spec.registration.worker_id,
            });
        }
        ControlBody::Heartbeat(heartbeat) => {
            if heartbeat.generation != worker.spec.registration.generation
                || !matches!(
                    worker.phase,
                    LifecyclePhase::Ready | LifecyclePhase::Suspect | LifecyclePhase::Stopping
                )
            {
                return Err(RoutingErrorCategory::ManifestIncompatible);
            }
            let recovered = worker.phase == LifecyclePhase::Suspect;
            if recovered {
                worker.phase = LifecyclePhase::Ready;
                events.push(LifecycleEvent::HeartbeatRecovered {
                    worker_id: worker.spec.registration.worker_id,
                });
            }
            worker.last_heartbeat = Some(now);
        }
        ControlBody::Exit(exit) => {
            let expected_role = match worker.spec.registration.kind {
                WorkerKind::Lobby => WorkerRole::Lobby,
                WorkerKind::Match => WorkerRole::Match,
            };
            if exit.role != expected_role
                || exit.terminal_peers != 0
                || exit.terminal_queue_bytes != 0
                || exit.result_sent != worker.result.is_some()
                || worker.exit.is_some()
            {
                return Err(RoutingErrorCategory::WorkerExitMismatch);
            }
            worker.exit = Some(*exit);
            events.push(LifecycleEvent::ExitReceived {
                worker_id: worker.spec.registration.worker_id,
                exit: *exit,
            });
        }
        ControlBody::Failure(_) => {
            return Err(RoutingErrorCategory::WorkerReportedFailure);
        }
        ControlBody::Result(result) => {
            let expected = if worker.spec.registration.kind == WorkerKind::Match {
                MatchManifestV1::decode(&worker.spec.manifest.manifest)
                    .ok()
                    .map(|manifest| (manifest.match_id, manifest.allocation_id))
            } else {
                None
            };
            let Some((match_id, allocation_id)) = expected else {
                return Err(RoutingErrorCategory::WorkerProtocolConflict);
            };
            if result.match_id != match_id
                || result.allocation_id != allocation_id
                || worker.result.is_some()
            {
                return Err(RoutingErrorCategory::WorkerProtocolConflict);
            }
            worker.result = Some(result.clone());
            events.push(LifecycleEvent::ResultReceived {
                worker_id: worker.spec.registration.worker_id,
                result: result.clone(),
            });
        }
        ControlBody::Manifest(_)
        | ControlBody::AllocateRequest(_)
        | ControlBody::AllocationGranted(_)
        | ControlBody::AllocationRejected(_)
        | ControlBody::PeerClose(_)
        | ControlBody::Stop(_)
        | ControlBody::LobbyAuthenticated(_)
        | ControlBody::LobbyNetcodeAuthenticated(_) => {
            // Other control bodies belong to the routing/worker protocol.  They are decoded and
            // sequence-checked here, but their domain semantics stay outside process supervision.
        }
    }
    Ok(())
}

fn queue_stop(
    worker: &mut ManagedWorker,
    reason: u16,
    graceful_stop: Duration,
) -> Result<(), LifecycleError> {
    let Some(stop) = worker.stop else {
        return Ok(());
    };
    if stop.queued {
        return Ok(());
    }
    let frame = ControlFrame::from_raw_sequence(
        worker.next_sequence,
        worker.spec.registration.process_id,
        worker.spec.registration.worker_id,
        ControlBody::Stop(StopBody {
            stop_id: stop.stop_id,
            reason,
            graceful_deadline_ms: u32::try_from(graceful_stop.as_millis()).unwrap_or(u32::MAX),
        }),
    )?
    .encode()?;
    if let Some(channels) = worker.channels.as_mut() {
        channels
            .enqueue_control(&frame)
            .map_err(|error| LifecycleError::Process {
                worker_id: worker.spec.registration.worker_id,
                detail: error.to_string(),
            })?;
    } else if worker.external_io {
        worker.pending_external_controls.push_back(frame);
    } else {
        return Err(LifecycleError::Process {
            worker_id: worker.spec.registration.worker_id,
            detail: "worker IPC channels are unavailable".to_string(),
        });
    }
    worker.next_sequence = worker.next_sequence.saturating_add(1);
    if let Some(stop) = worker.stop.as_mut() {
        stop.queued = true;
    }
    Ok(())
}

fn mark_stop_sent(worker: &mut ManagedWorker) -> Option<StopId> {
    let stop = worker.stop.as_mut()?;
    if !stop.queued || stop.sent {
        return None;
    }
    stop.sent = true;
    Some(stop.stop_id)
}

fn decode_manifest_common(manifest: &ManifestBody) -> Result<ManifestCommon, LifecycleError> {
    match manifest.role {
        WorkerRole::Lobby => Ok(crate::LobbyManifest::decode(&manifest.manifest)?.common),
        WorkerRole::Match => Ok(MatchManifestV1::decode(&manifest.manifest)?.common),
    }
}

fn manifest_digest_from_decoded(manifest: &ManifestBody) -> Result<[u8; 32], LifecycleError> {
    match manifest.role {
        WorkerRole::Lobby => Ok(crate::LobbyManifest::decode(&manifest.manifest)?.digest),
        WorkerRole::Match => Ok(MatchManifestV1::decode(&manifest.manifest)?.digest),
    }
}

fn validate_executable(path: &Path) -> Result<PathBuf, LifecycleError> {
    if path.as_os_str().is_empty() {
        return Err(LifecycleError::InvalidLaunch("executable path is empty"));
    }
    let metadata = std::fs::metadata(path).map_err(LifecycleError::Io)?;
    if !metadata.is_file() {
        return Err(LifecycleError::InvalidLaunch(
            "executable is not a regular file",
        ));
    }
    Ok(path.to_path_buf())
}

fn random_stop_id() -> Result<StopId, LifecycleError> {
    let mut bytes = [0_u8; 8];
    getrandom::fill(&mut bytes).map_err(|error| LifecycleError::Process {
        worker_id: WorkerId::new(1).expect("constant ID"),
        detail: format!("OS entropy unavailable: {error}"),
    })?;
    StopId::new(u64::from_be_bytes(bytes).max(1))
        .ok_or(LifecycleError::InvalidLaunch("zero stop ID"))
}

fn refresh_spec(
    config: &ProcessSupervisorConfig,
    mut spec: WorkerLaunchSpec,
) -> Result<WorkerLaunchSpec, LifecycleError> {
    let process_id = random_process_id()?;
    let generation = Generation::new(spec.registration.generation.get().saturating_add(1))
        .ok_or(LifecycleError::InvalidLaunch("worker generation exhausted"))?;
    spec.registration.process_id = process_id;
    spec.registration.generation = generation;
    spec.manifest = match spec.manifest.role {
        WorkerRole::Lobby => {
            let mut manifest = crate::LobbyManifest::decode(&spec.manifest.manifest)?;
            manifest.common.process_id = process_id;
            manifest.common.generation = generation;
            manifest.digest = [0; 32];
            ManifestBody::from_lobby(&manifest)?
        }
        WorkerRole::Match => {
            let mut manifest = MatchManifestV1::decode(&spec.manifest.manifest)?;
            manifest.common.process_id = process_id;
            manifest.common.generation = generation;
            manifest.digest = [0; 32];
            ManifestBody::from_match(&manifest)?
        }
    };
    if spec.manifest.role
        != match spec.registration.kind {
            WorkerKind::Lobby => WorkerRole::Lobby,
            WorkerKind::Match => WorkerRole::Match,
        }
    {
        return Err(LifecycleError::Routing(
            RoutingErrorCategory::ManifestIdentity,
        ));
    }
    if config.logical_server_id != decode_manifest_common(&spec.manifest)?.logical_server_id {
        return Err(LifecycleError::Routing(
            RoutingErrorCategory::ManifestIncompatible,
        ));
    }
    Ok(spec)
}

fn random_process_id() -> Result<ProcessId, LifecycleError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|_| LifecycleError::InvalidLaunch("OS entropy unavailable"))?;
    ProcessId::new(u128::from_be_bytes(bytes).max(1))
        .ok_or(LifecycleError::InvalidLaunch("zero process ID"))
}

#[derive(Clone, Copy)]
enum IpcChannelKind {
    Control,
}

fn map_ipc_error(error: IpcIoError, channel: IpcChannelKind) -> RoutingErrorCategory {
    match (channel, error) {
        (IpcChannelKind::Control, IpcIoError::Eof) => RoutingErrorCategory::IpcControlClosed,
        (IpcChannelKind::Control, IpcIoError::Malformed(_) | IpcIoError::WouldBlock) => {
            RoutingErrorCategory::IpcMalformed
        }
        (IpcChannelKind::Control, IpcIoError::Io(_)) => RoutingErrorCategory::IpcIo,
    }
}

impl Drop for ProcessSupervisor {
    fn drop(&mut self) {
        // Drop is a last-resort safety net.  Explicit `shutdown` is the evidence-bearing path;
        // this best effort still kills children before listener/path ownership is released and
        // never uses unsafe signal primitives or broad filesystem cleanup.
        for worker in self.workers.values_mut() {
            if worker.status.is_none() {
                let _ = worker.child.kill();
                for _ in 0..100 {
                    match worker.child.try_wait() {
                        Ok(Some(_)) | Err(_) => break,
                        Ok(None) => thread::sleep(Duration::from_millis(1)),
                    }
                }
            }
        }
        self.workers.clear();
        self.pending_restarts.clear();
        self.runtime_dir.take();
    }
}
