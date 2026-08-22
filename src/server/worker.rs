//! Worker-process control-plane bootstrap.
//!
//! A worker receives only role/identity/socket paths in argv.  Its manifest, compatibility
//! values, and all route-bearing control facts arrive on the supervisor-owned control stream.
//! This module validates that first frame before constructing either the minimum lobby graph or
//! the reused authoritative match graph.

use super::{
    RoutedPeerClose, RoutedWorker, RoutedWorkerFailure, ServerNetworkConfig,
    build_lobby_worker_app, build_match_worker_app,
};
use crate::{
    config::{GameMode, MatchRulesProfile},
    matchplay::{MatchPhase, MatchRoot, MatchState},
    protocol::DEVELOPMENT_PRIVATE_KEY,
};
use bevy::{app::AppExit, prelude::*};
use brawler_routing::{
    ActivationBody, AllocateRequestBody, CONTROL_VERSION_CURRENT, CodecError, ControlBody,
    ControlFrame, ControlSequenceTracker, FramedReader, IpcChannel, IpcIoError,
    LobbyAuthenticatedBody, LobbyManifest, LobbyNetcodeAuthenticatedBody, MatchManifestV1,
    PACKET_VERSION_V1, PeerCloseBody, ProcessId, ROUTE_VERSION_V1, ReadyBody, SequenceDisposition,
    StopBody, UnixWorkerChannels, WorkerId, WorkerRole,
};
use lightyear::prelude::Linked;
use lightyear::prelude::server::Server as LightyearServer;
use lightyear::prelude::server::{NetcodeConfig, NetcodeServer, Start, Started};
use std::{
    collections::VecDeque,
    io,
    os::unix::net::UnixStream,
    path::PathBuf,
    time::{Duration, Instant},
};

const CONTROL_BURST: usize = 16;
const FAILURE_PHASE_BOOTSTRAP: u16 = 1;
const FAILURE_PHASE_RUNTIME: u16 = 2;
const FAILURE_CATEGORY_PROTOCOL: u16 = 1;
const FAILURE_DETAIL_MALFORMED: u32 = 1;
const FAILURE_DETAIL_EOF: u32 = 2;
const LOBBY_CONTROL_INBOX_FRAMES: usize = 8;
const LOBBY_CONTROL_OUTBOX_FRAMES: usize = 4;

/// A supervisor-owned control write half may close immediately after delivering Stop during
/// process shutdown.  Once Stop has been validated, that EOF is an expected half-close and the
/// worker must still flush its Exit on the surviving worker-to-supervisor direction.  An EOF
/// without a validated Stop remains a protocol failure.
const fn control_eof_is_failure(eof: bool, shutdown_requested: bool, stop_seen: bool) -> bool {
    eof && !shutdown_requested && !stop_seen
}

/// Bounded worker-to-lobby control handoff. The worker remains the sole BRCT stream reader.
#[derive(Resource, Default)]
pub struct LobbyControlInbox {
    frames: VecDeque<ControlFrame>,
}

impl LobbyControlInbox {
    fn push(&mut self, frame: ControlFrame) -> bool {
        if self.frames.len() >= LOBBY_CONTROL_INBOX_FRAMES {
            return false;
        }
        self.frames.push_back(frame);
        true
    }

    pub(crate) fn drain(&mut self) -> impl Iterator<Item = ControlFrame> + '_ {
        self.frames.drain(..)
    }
}

/// Bounded lobby-to-worker control handoff. The worker adds the shared sequence and envelope.
#[derive(Resource, Default)]
pub struct LobbyControlOutbox {
    authenticated: VecDeque<LobbyAuthenticatedBody>,
    netcode_authenticated: VecDeque<LobbyNetcodeAuthenticatedBody>,
    requests: VecDeque<AllocateRequestBody>,
    activation_cancels: VecDeque<ActivationBody>,
}

#[derive(Resource, Default)]
pub(crate) struct MatchControlOutbox {
    cancel: Option<ActivationBody>,
    activated: Option<ActivationBody>,
    start_failed: Option<ActivationBody>,
}

impl MatchControlOutbox {
    pub(crate) fn cancel(&mut self, body: ActivationBody) {
        self.cancel.get_or_insert(body);
    }

    pub(crate) fn activated(&mut self, body: ActivationBody) {
        self.activated.get_or_insert(body);
    }

    pub(crate) fn start_failed(&mut self, body: ActivationBody) {
        self.start_failed.get_or_insert(body);
        self.cancel = None;
    }
}

impl LobbyControlOutbox {
    pub(crate) fn push_activation_cancel(&mut self, fact: ActivationBody) -> bool {
        if self.activation_cancels.len() >= LOBBY_CONTROL_OUTBOX_FRAMES {
            return false;
        }
        self.activation_cancels.push_back(fact);
        true
    }
    pub(crate) fn push_authenticated(&mut self, fact: LobbyAuthenticatedBody) -> bool {
        if self.authenticated.len() >= LOBBY_CONTROL_OUTBOX_FRAMES * 8 {
            return false;
        }
        self.authenticated.push_back(fact);
        true
    }

    pub(crate) fn authenticated_front(&self) -> Option<&LobbyAuthenticatedBody> {
        self.authenticated.front()
    }

    pub(crate) fn pop_authenticated(&mut self) {
        let _ = self.authenticated.pop_front();
    }

    pub(crate) fn push_netcode_authenticated(
        &mut self,
        fact: LobbyNetcodeAuthenticatedBody,
    ) -> bool {
        if self.netcode_authenticated.len() >= LOBBY_CONTROL_OUTBOX_FRAMES * 8 {
            return false;
        }
        self.netcode_authenticated.push_back(fact);
        true
    }

    pub(crate) fn netcode_authenticated_front(&self) -> Option<&LobbyNetcodeAuthenticatedBody> {
        self.netcode_authenticated.front()
    }

    pub(crate) fn pop_netcode_authenticated(&mut self) {
        let _ = self.netcode_authenticated.pop_front();
    }

    pub(crate) fn push(&mut self, request: AllocateRequestBody) -> bool {
        if self.requests.len() >= LOBBY_CONTROL_OUTBOX_FRAMES {
            return false;
        }
        self.requests.push_back(request);
        true
    }

    fn front(&self) -> Option<&AllocateRequestBody> {
        self.requests.front()
    }

    fn pop_front(&mut self) {
        let _ = self.requests.pop_front();
    }
}

/// Which explicit brawler-server worker mode is being launched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkerEntrypointRole {
    Lobby,
    Match,
}

impl WorkerEntrypointRole {
    #[must_use]
    pub const fn wire_role(self) -> WorkerRole {
        match self {
            Self::Lobby => WorkerRole::Lobby,
            Self::Match => WorkerRole::Match,
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "lobby" => Some(Self::Lobby),
            "match" => Some(Self::Match),
            _ => None,
        }
    }
}

/// Non-secret worker identity and endpoint arguments admitted by the process contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerLaunchArguments {
    pub role: WorkerEntrypointRole,
    pub logical_server_id: brawler_routing::LogicalServerId,
    pub supervisor_generation: brawler_routing::Generation,
    pub worker_id: WorkerId,
    pub process_id: ProcessId,
    pub worker_generation: brawler_routing::Generation,
    pub packet_socket: PathBuf,
    pub control_socket: PathBuf,
}

/// Errors intentionally contain no manifest, payload, capability, or credential bytes.
#[derive(Debug)]
pub enum WorkerBootstrapError {
    Io(io::Error),
    Ipc(IpcIoError),
    Codec(CodecError),
    Invalid(&'static str),
    MatchAdmission,
}

impl std::fmt::Display for WorkerBootstrapError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "worker bootstrap I/O failed: {error}"),
            Self::Ipc(error) => write!(formatter, "worker control IPC failed: {error}"),
            Self::Codec(error) => write!(formatter, "worker control frame rejected: {error}"),
            Self::Invalid(detail) => write!(formatter, "worker bootstrap rejected: {detail}"),
            Self::MatchAdmission => formatter.write_str("match worker manifest admission failed"),
        }
    }
}

impl std::error::Error for WorkerBootstrapError {}

impl From<io::Error> for WorkerBootstrapError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<IpcIoError> for WorkerBootstrapError {
    fn from(error: IpcIoError) -> Self {
        Self::Ipc(error)
    }
}

impl From<CodecError> for WorkerBootstrapError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

enum WorkerManifest {
    Lobby(LobbyManifest),
    Match(MatchManifestV1),
}

impl WorkerManifest {
    fn common(&self) -> brawler_routing::ManifestCommon {
        match self {
            Self::Lobby(manifest) => manifest.common,
            Self::Match(manifest) => manifest.common,
        }
    }

    fn digest(&self) -> [u8; 32] {
        match self {
            Self::Lobby(manifest) => manifest.digest,
            Self::Match(manifest) => manifest.digest,
        }
    }

    fn heartbeat(&self) -> Duration {
        let millis = match self {
            Self::Lobby(manifest) => manifest.heartbeat_ms,
            Self::Match(manifest) => manifest.heartbeat_ms,
        };
        Duration::from_millis(u64::from(millis).max(1))
    }
}

/// A validated worker control-plane connection and immutable manifest.
pub struct WorkerBootstrap {
    args: WorkerLaunchArguments,
    channels: UnixWorkerChannels,
    manifest: WorkerManifest,
    manifest_frame: ControlFrame,
}

impl WorkerBootstrap {
    /// Connect the two private Unix streams and accept exactly one validated Manifest frame.
    /// No app or gameplay state is constructed until this method succeeds.
    pub fn connect(args: WorkerLaunchArguments) -> Result<Self, WorkerBootstrapError> {
        let packet = UnixStream::connect(&args.packet_socket)?;
        let mut control = UnixStream::connect(&args.control_socket)?;
        packet.set_nonblocking(false)?;
        control.set_nonblocking(false)?;

        let mut reader = FramedReader::new(IpcChannel::Control);
        let progress = reader.read_ready(&mut control, 1)?;
        let record = progress
            .records
            .into_iter()
            .next()
            .ok_or(WorkerBootstrapError::Invalid("manifest frame missing"))?;
        let frame = ControlFrame::decode_for(&record, args.process_id, args.worker_id)?;
        let ControlBody::Manifest(body) = &frame.body else {
            return Err(WorkerBootstrapError::Invalid(
                "first control frame is not a manifest",
            ));
        };
        if body.role != args.role.wire_role() {
            return Err(WorkerBootstrapError::Invalid("manifest role mismatch"));
        }
        let manifest = match body.role {
            WorkerRole::Lobby => WorkerManifest::Lobby(LobbyManifest::decode(&body.manifest)?),
            WorkerRole::Match => WorkerManifest::Match(MatchManifestV1::decode(&body.manifest)?),
        };
        let common = manifest.common();
        if common.logical_server_id != args.logical_server_id
            || common.process_id != args.process_id
            || common.worker_id != args.worker_id
            || common.generation != args.worker_generation
        {
            return Err(WorkerBootstrapError::Invalid("manifest identity mismatch"));
        }

        packet.set_nonblocking(true)?;
        control.set_nonblocking(true)?;
        let channels = UnixWorkerChannels::from_std(packet, control);
        Ok(Self {
            args,
            channels,
            manifest,
            manifest_frame: frame,
        })
    }

    /// Construct the role-specific app, send Ready, and install the routed Lightyear endpoint.
    pub fn start(self) -> Result<App, WorkerBootstrapError> {
        let Self {
            args,
            channels,
            manifest,
            manifest_frame,
        } = self;
        let result_identity = match &manifest {
            WorkerManifest::Lobby(_) => None,
            WorkerManifest::Match(manifest) => Some((manifest.match_id, manifest.allocation_id)),
        };
        let automatic_transition_driver = matches!(&manifest, WorkerManifest::Lobby(_))
            && std::env::var("BRAWLER_LOBBY_TRANSITION_DRIVER").as_deref() == Ok("1");
        let mut config = config_from_manifest(&manifest)?;
        if automatic_transition_driver {
            config.game_mode = match std::env::var("BRAWLER_LOBBY_TRANSITION_MODE").as_deref() {
                Ok("wipeout") => GameMode::Wipeout,
                Ok("hot-zone") => GameMode::HotZone,
                _ => {
                    return Err(WorkerBootstrapError::Invalid(
                        "invalid automatic transition game mode",
                    ));
                }
            };
        }
        let heartbeat = manifest.heartbeat();
        let digest = manifest.digest();
        let mut app = match manifest {
            WorkerManifest::Lobby(manifest) => build_lobby_worker_app(config, manifest)
                .map_err(|_| WorkerBootstrapError::MatchAdmission)?,
            WorkerManifest::Match(manifest) => build_match_worker_app(config, manifest)
                .map_err(|_| WorkerBootstrapError::MatchAdmission)?,
        };
        if automatic_transition_driver {
            app.add_plugins(super::lobby::LobbyTransitionDriverPlugin);
        }

        let ready = ControlFrame::from_raw_sequence(
            1,
            args.process_id,
            args.worker_id,
            ControlBody::Ready(ReadyBody {
                manifest_digest: digest,
                generation: args.worker_generation,
                route_version: ROUTE_VERSION_V1,
                packet_version: PACKET_VERSION_V1,
                control_version: CONTROL_VERSION_CURRENT,
                flags: 0,
            }),
        )?;
        let app_config = app.world().resource::<ServerNetworkConfig>().clone();
        let endpoint =
            install_routed_worker_endpoint(&mut app, &app_config, args.worker_id, channels)?;
        app.insert_resource(WorkerControlState::new(
            args.process_id,
            args.worker_id,
            args.role,
            args.worker_generation,
            heartbeat,
            manifest_frame,
            result_identity,
        )?);
        if args.role == WorkerEntrypointRole::Match {
            app.init_resource::<MatchControlOutbox>();
        }
        app.add_plugins(WorkerControlPlugin);
        app.world_mut().flush();
        let endpoint_ready = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<(), (
                With<NetcodeServer>,
                With<Started>,
                With<Linked>,
                With<RoutedWorker>,
            )>();
            query.get(world, endpoint).is_ok()
        };
        if !endpoint_ready {
            return Err(WorkerBootstrapError::Invalid(
                "routed endpoint did not reach linked state",
            ));
        }
        let ready_bytes = ready.encode()?;
        {
            let world = app.world_mut();
            let mut query = world.query::<&mut RoutedWorker>();
            let mut worker = query
                .single_mut(world)
                .map_err(|_| WorkerBootstrapError::Invalid("routed endpoint missing"))?;
            worker.channels_mut().enqueue_control(&ready_bytes)?;
            worker.channels_mut().flush_control(CONTROL_BURST)?;
        }
        debug!(?endpoint, role = ?args.role, "routed worker endpoint bootstrapped");
        Ok(app)
    }
}

/// Install one addressless Netcode server endpoint backed by the worker's private packet stream.
pub fn install_routed_worker_endpoint(
    app: &mut App,
    config: &ServerNetworkConfig,
    worker_id: WorkerId,
    channels: UnixWorkerChannels,
) -> Result<Entity, WorkerBootstrapError> {
    let timeout_secs = i32::try_from(config.client_timeout.as_secs())
        .map_err(|_| WorkerBootstrapError::Invalid("client timeout exceeds Netcode limit"))?;
    let netcode_config = NetcodeConfig::default()
        .with_protocol_id(config.network_protocol_id)
        .with_key(DEVELOPMENT_PRIVATE_KEY)
        .with_client_timeout_secs(timeout_secs);
    let server = app
        .world_mut()
        .spawn((
            NetcodeServer::new(NetcodeConfig {
                server_addr_check: false,
                ..netcode_config
            }),
            LightyearServer::new(config.impairment_profile.receive_conditioner()),
            RoutedWorker::new(worker_id, channels),
            Name::new("Brawler routed worker"),
        ))
        .id();
    app.world_mut().trigger(Start { entity: server });
    // A worker stream is already connected, so this is the routed adapter's equivalent of the
    // UDP transport observing its endpoint start. RoutedWorkerPlugin owns the transition to
    // Linked and keeps the normal Lightyear lifecycle observable.
    app.world_mut()
        .trigger(lightyear::link::LinkStart { entity: server });
    Ok(server)
}

#[derive(Resource)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent one-shot control-frame and AppExit facts must remain separately observable"
)]
pub(crate) struct WorkerControlState {
    process_id: ProcessId,
    worker_id: WorkerId,
    role: WorkerEntrypointRole,
    generation: brawler_routing::Generation,
    heartbeat_interval: Duration,
    next_heartbeat: Instant,
    next_sequence: u64,
    sequences: ControlSequenceTracker,
    shutdown_requested: bool,
    pending_stop: Option<StopBody>,
    exit_sent: bool,
    app_exit_requested: bool,
    result_sent: bool,
    packet_write_closed: bool,
    result_identity: Option<(brawler_routing::MatchId, brawler_routing::AllocationId)>,
}

/// Queue one worker->supervisor peer-close fact. This is kept beside BRCT sequence ownership so
/// every locally removed routed peer consumes exactly one control sequence.
pub(crate) fn queue_peer_close(
    worker: &mut RoutedWorker,
    state: &mut WorkerControlState,
    route_id: brawler_routing::RouteId,
    peer_id: brawler_routing::PeerId,
    reason: u16,
) -> Result<(), IpcIoError> {
    let frame = ControlFrame::from_raw_sequence(
        state.next_sequence,
        state.process_id,
        state.worker_id,
        ControlBody::PeerClose(PeerCloseBody {
            route_id,
            peer_id,
            reason,
        }),
    )
    .and_then(|frame| frame.encode())
    .map_err(IpcIoError::Malformed)?;
    worker.channels_mut().enqueue_control(&frame)?;
    state.next_sequence = state.next_sequence.saturating_add(1);
    Ok(())
}

impl WorkerControlState {
    fn new(
        process_id: ProcessId,
        worker_id: WorkerId,
        role: WorkerEntrypointRole,
        generation: brawler_routing::Generation,
        heartbeat_interval: Duration,
        manifest_frame: ControlFrame,
        result_identity: Option<(brawler_routing::MatchId, brawler_routing::AllocationId)>,
    ) -> Result<Self, WorkerBootstrapError> {
        let mut sequences = ControlSequenceTracker::default();
        sequences.observe(manifest_frame)?;
        Ok(Self {
            process_id,
            worker_id,
            role,
            generation,
            heartbeat_interval,
            next_heartbeat: Instant::now() + heartbeat_interval,
            next_sequence: 2,
            sequences,
            shutdown_requested: false,
            pending_stop: None,
            exit_sent: false,
            app_exit_requested: false,
            result_sent: false,
            packet_write_closed: false,
            result_identity,
        })
    }
}

struct WorkerControlPlugin;

impl Plugin for WorkerControlPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, worker_control_receive).add_systems(
            Last,
            (
                worker_control_emit_result,
                worker_control_finish_shutdown,
                worker_control_flush,
            )
                .chain(),
        );
    }
}

/// Canonical, bounded M01 result bytes. The supervisor stores and fingerprints these bytes but
/// does not interpret gameplay; the leading schema byte makes future result extensions explicit.
fn canonical_match_result(state: &MatchState) -> Option<Vec<u8>> {
    let MatchPhase::Completed {
        completed_at_tick,
        restart_unlocked_at_tick,
        result,
    } = state.phase
    else {
        return None;
    };
    let mut bytes = Vec::with_capacity(32);
    bytes.push(brawler_routing::RESULT_SCHEMA_VERSION_V1);
    bytes.extend_from_slice(&state.match_id.0.to_be_bytes());
    bytes.extend_from_slice(&state.mode_definition_id.0.to_be_bytes());
    bytes.extend_from_slice(&state.rules_revision.to_be_bytes());
    bytes.extend_from_slice(&completed_at_tick.to_be_bytes());
    bytes.extend_from_slice(&restart_unlocked_at_tick.to_be_bytes());
    match result {
        crate::matchplay::MatchResult::TeamVictory { team } => {
            bytes.push(1);
            bytes.push(team.0);
            bytes.push(0);
        }
        crate::matchplay::MatchResult::Draw => {
            bytes.push(2);
            bytes.push(0);
            bytes.push(0);
        }
        crate::matchplay::MatchResult::Forfeit {
            winner,
            departed_team,
        } => {
            bytes.push(3);
            bytes.push(winner.0);
            bytes.push(departed_team.0);
        }
    }
    Some(bytes)
}

/// Encode the completed gameplay epoch as the result of the supervisor-owned allocation. A
/// Balance Lab reset advances the internal gameplay match ID without replacing that allocation,
/// so the two IDs are intentionally allowed to differ.
fn canonical_allocation_result(
    state: &MatchState,
    _allocation_match_id: brawler_routing::MatchId,
) -> Option<Vec<u8>> {
    canonical_match_result(state)
}

/// Emit exactly one result after the authoritative match root first becomes Completed. This is
/// in Last so the completed state is final for the frame and the result is queued before the
/// worker's control flush; a supervisor Stop can therefore only arrive after Result.
fn worker_control_emit_result(
    mut state: ResMut<WorkerControlState>,
    mut workers: Query<&mut RoutedWorker, With<Linked>>,
    roots: Query<&MatchState, With<MatchRoot>>,
) {
    if state.role != WorkerEntrypointRole::Match || state.result_sent {
        return;
    }
    let Some((match_id, allocation_id)) = state.result_identity else {
        return;
    };
    let Ok(match_state) = roots.single() else {
        return;
    };
    let Some(result) = canonical_allocation_result(match_state, match_id) else {
        return;
    };
    let Ok(body) = brawler_routing::ResultBody::new(match_id, allocation_id, result) else {
        return;
    };
    let Ok(mut worker) = workers.single_mut() else {
        return;
    };
    let Ok(frame) = ControlFrame::from_raw_sequence(
        state.next_sequence,
        state.process_id,
        state.worker_id,
        ControlBody::Result(body),
    )
    .and_then(|frame| frame.encode()) else {
        return;
    };
    if worker.channels_mut().enqueue_control(&frame).is_ok() {
        state.next_sequence = state.next_sequence.saturating_add(1);
        state.result_sent = true;
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn worker_control_receive(
    mut commands: Commands,
    mut app_exit: MessageWriter<AppExit>,
    mut state: ResMut<WorkerControlState>,
    mut lobby_inbox: Option<ResMut<LobbyControlInbox>>,
    mut lobby_outbox: Option<ResMut<LobbyControlOutbox>>,
    mut match_outbox: Option<ResMut<MatchControlOutbox>>,
    mut workers: Query<(Entity, &mut RoutedWorker), With<Linked>>,
) {
    let Ok((worker_entity, mut worker)) = workers.single_mut() else {
        return;
    };
    let progress = match worker.channels_mut().control_read_ready(CONTROL_BURST) {
        Ok(progress) => progress,
        Err(_error) => {
            error!(worker = ?state.worker_id, "brawler routed worker control read failed");
            worker_control_failure(
                &mut worker,
                &mut state,
                &mut app_exit,
                &mut commands,
                worker_entity,
                FAILURE_DETAIL_MALFORMED,
            );
            return;
        }
    };
    let mut stop = None;
    let mut failure = None;
    for record in progress.records {
        let frame = match ControlFrame::decode_for(&record, state.process_id, state.worker_id) {
            Ok(frame) => frame,
            Err(_error) => {
                error!(worker = ?state.worker_id, "brawler routed worker rejected malformed control frame");
                failure = Some(FAILURE_DETAIL_MALFORMED);
                break;
            }
        };
        match state.sequences.observe(frame.clone()) {
            Ok(SequenceDisposition::Duplicate) => continue,
            Ok(SequenceDisposition::Accepted) => {}
            Err(_error) => {
                error!(worker = ?state.worker_id, "brawler routed worker rejected control sequence");
                failure = Some(FAILURE_DETAIL_MALFORMED);
                break;
            }
        }
        match &frame.body {
            ControlBody::Stop(body) => stop = Some(*body),
            ControlBody::PeerClose(PeerCloseBody {
                route_id,
                peer_id,
                reason,
            }) => commands.trigger(RoutedPeerClose {
                worker: worker_entity,
                route_id: *route_id,
                peer_id: *peer_id,
                reason: lightyear::prelude::UnlinkReason::ByPeer(format!(
                    "supervisor close reason {reason}"
                )),
            }),
            ControlBody::AllocationGranted(_)
            | ControlBody::AllocationRejected(_)
            | ControlBody::LobbyCapacity(_) => {
                if let Some(inbox) = lobby_inbox.as_mut()
                    && !inbox.push(frame.clone())
                {
                    error!(worker = ?state.worker_id, "brawler routed worker lobby control inbox overflow");
                    failure = Some(FAILURE_DETAIL_MALFORMED);
                    break;
                }
            }
            ControlBody::ActivationDissolved(_) | ControlBody::Activated(_) => {
                if let Some(inbox) = lobby_inbox.as_mut()
                    && !inbox.push(frame.clone())
                {
                    failure = Some(FAILURE_DETAIL_MALFORMED);
                    break;
                }
            }
            _ => {}
        }
    }
    if let Some(detail_code) = failure {
        worker_control_failure(
            &mut worker,
            &mut state,
            &mut app_exit,
            &mut commands,
            worker_entity,
            detail_code,
        );
        return;
    }
    if let Some(stop_body) = stop
        && !state.shutdown_requested
    {
        state.shutdown_requested = true;
        state.pending_stop = Some(stop_body);
        worker.request_unlink();
        commands.trigger(lightyear::prelude::Unlink {
            entity: worker_entity,
            reason: lightyear::prelude::UnlinkReason::UserRequested(None),
        });
    }
    // Evaluate EOF only after the current batch has been decoded and Stop has been admitted. A
    // same-read Stop+EOF is therefore equivalent to a Stop followed by a later EOF: the
    // supervisor has closed its write half, while the worker still owns the response direction.
    if control_eof_is_failure(progress.eof, state.shutdown_requested, stop.is_some()) {
        error!(
            worker = ?state.worker_id,
            shutdown_requested = state.shutdown_requested,
            stop_seen = stop.is_some(),
            "brawler routed worker received unsolicited control EOF"
        );
        failure = Some(FAILURE_DETAIL_EOF);
    }
    if let Some(detail_code) = failure {
        worker_control_failure(
            &mut worker,
            &mut state,
            &mut app_exit,
            &mut commands,
            worker_entity,
            detail_code,
        );
        return;
    }
    if !state.shutdown_requested {
        let mut outbox_failed = false;
        let mut outbox_blocked = false;
        if let Some(outbox) = match_outbox.as_mut()
            && let Some(body) = outbox.start_failed
        {
            let encoded = ControlFrame::from_raw_sequence(
                state.next_sequence,
                state.process_id,
                state.worker_id,
                ControlBody::StartFailed(body),
            )
            .and_then(|frame| frame.encode());
            match encoded {
                Ok(bytes) => match worker.channels_mut().enqueue_control(&bytes) {
                    Ok(()) => {
                        outbox.start_failed = None;
                        state.next_sequence = state.next_sequence.saturating_add(1);
                    }
                    Err(IpcIoError::WouldBlock) => outbox_blocked = true,
                    Err(_) => outbox_failed = true,
                },
                Err(_) => outbox_failed = true,
            }
        }
        if let Some(outbox) = match_outbox.as_mut()
            && !outbox_blocked
            && let Some(body) = outbox.cancel
        {
            let encoded = ControlFrame::from_raw_sequence(
                state.next_sequence,
                state.process_id,
                state.worker_id,
                ControlBody::CancelActivation(body),
            )
            .and_then(|frame| frame.encode());
            match encoded {
                Ok(bytes) => match worker.channels_mut().enqueue_control(&bytes) {
                    Ok(()) => {
                        outbox.cancel = None;
                        state.next_sequence = state.next_sequence.saturating_add(1);
                    }
                    Err(IpcIoError::WouldBlock) => outbox_blocked = true,
                    Err(_) => outbox_failed = true,
                },
                Err(_) => outbox_failed = true,
            }
        }
        if let Some(outbox) = match_outbox.as_mut()
            && !outbox_blocked
            && let Some(body) = outbox.activated
        {
            let encoded = ControlFrame::from_raw_sequence(
                state.next_sequence,
                state.process_id,
                state.worker_id,
                ControlBody::Activated(body),
            )
            .and_then(|frame| frame.encode());
            match encoded {
                Ok(bytes) => match worker.channels_mut().enqueue_control(&bytes) {
                    Ok(()) => {
                        outbox.activated = None;
                        state.next_sequence = state.next_sequence.saturating_add(1);
                    }
                    Err(IpcIoError::WouldBlock) => outbox_blocked = true,
                    Err(_) => outbox_failed = true,
                },
                Err(_) => outbox_failed = true,
            }
        }
        if let Some(outbox) = lobby_outbox.as_mut() {
            while !outbox_blocked {
                let Some(body) = outbox.activation_cancels.front().copied() else {
                    break;
                };
                let encoded = ControlFrame::from_raw_sequence(
                    state.next_sequence,
                    state.process_id,
                    state.worker_id,
                    ControlBody::CancelActivation(body),
                )
                .and_then(|frame| frame.encode());
                match encoded {
                    Ok(bytes) => match worker.channels_mut().enqueue_control(&bytes) {
                        Ok(()) => {
                            outbox.activation_cancels.pop_front();
                            state.next_sequence = state.next_sequence.saturating_add(1);
                        }
                        Err(IpcIoError::WouldBlock) => outbox_blocked = true,
                        Err(_) => outbox_failed = true,
                    },
                    Err(_) => outbox_failed = true,
                }
            }
            while let Some(fact) = outbox.netcode_authenticated_front().copied() {
                let Ok(frame) = ControlFrame::from_raw_sequence(
                    state.next_sequence,
                    state.process_id,
                    state.worker_id,
                    ControlBody::LobbyNetcodeAuthenticated(fact),
                ) else {
                    outbox_failed = true;
                    break;
                };
                let Ok(bytes) = frame.encode() else {
                    outbox_failed = true;
                    break;
                };
                match worker.channels_mut().enqueue_control(&bytes) {
                    Ok(()) => {
                        outbox.pop_netcode_authenticated();
                        state.next_sequence = state.next_sequence.saturating_add(1);
                    }
                    Err(IpcIoError::WouldBlock | IpcIoError::Malformed(CodecError::Oversize)) => {
                        outbox_blocked = true;
                        break;
                    }
                    Err(_) => {
                        outbox_failed = true;
                        break;
                    }
                }
            }
            while !outbox_blocked {
                let Some(fact) = outbox.authenticated_front().copied() else {
                    break;
                };
                let Ok(frame) = ControlFrame::from_raw_sequence(
                    state.next_sequence,
                    state.process_id,
                    state.worker_id,
                    ControlBody::LobbyAuthenticated(fact),
                ) else {
                    outbox_failed = true;
                    break;
                };
                let Ok(bytes) = frame.encode() else {
                    outbox_failed = true;
                    break;
                };
                match worker.channels_mut().enqueue_control(&bytes) {
                    Ok(()) => {
                        outbox.pop_authenticated();
                        state.next_sequence = state.next_sequence.saturating_add(1);
                    }
                    Err(IpcIoError::WouldBlock | IpcIoError::Malformed(CodecError::Oversize)) => {
                        outbox_blocked = true;
                        break;
                    }
                    Err(_) => {
                        outbox_failed = true;
                        break;
                    }
                }
            }
            if !outbox_blocked {
                while let Some(body) = outbox.front().cloned() {
                    let Ok(frame) = ControlFrame::from_raw_sequence(
                        state.next_sequence,
                        state.process_id,
                        state.worker_id,
                        ControlBody::AllocateRequest(body),
                    ) else {
                        outbox_failed = true;
                        break;
                    };
                    let Ok(bytes) = frame.encode() else {
                        outbox_failed = true;
                        break;
                    };
                    match worker.channels_mut().enqueue_control(&bytes) {
                        Ok(()) => {
                            outbox.pop_front();
                            state.next_sequence = state.next_sequence.saturating_add(1);
                        }
                        Err(
                            IpcIoError::WouldBlock | IpcIoError::Malformed(CodecError::Oversize),
                        ) => {
                            break;
                        }
                        Err(_) => {
                            outbox_failed = true;
                            break;
                        }
                    }
                }
            }
        }
        if outbox_failed {
            error!(worker = ?state.worker_id, "brawler routed worker lobby control outbox failed");
            worker_control_failure(
                &mut worker,
                &mut state,
                &mut app_exit,
                &mut commands,
                worker_entity,
                FAILURE_DETAIL_MALFORMED,
            );
            return;
        }
    }
    let now = Instant::now();
    if now >= state.next_heartbeat && !state.shutdown_requested {
        let heartbeat = ControlFrame::from_raw_sequence(
            state.next_sequence,
            state.process_id,
            state.worker_id,
            ControlBody::Heartbeat(brawler_routing::HeartbeatBody {
                generation: state.generation,
                uptime_ms: 0,
                active_peers: u16::try_from(worker.peer_count()).unwrap_or(u16::MAX),
                packet_frames: 0,
                packet_bytes: 0,
                control_frames: 0,
                control_bytes: 0,
                fixed_tick_lag_us: 0,
                health_flags: 0,
            }),
        );
        let heartbeat = match heartbeat {
            Ok(frame) => frame.encode().map_err(WorkerBootstrapError::from),
            Err(error) => Err(WorkerBootstrapError::from(error)),
        };
        match heartbeat {
            Ok(bytes) => match worker.channels_mut().enqueue_control(&bytes) {
                Ok(()) => {
                    state.next_sequence = state.next_sequence.saturating_add(1);
                    state.next_heartbeat = now + state.heartbeat_interval;
                }
                Err(_error) => {
                    error!(worker = ?state.worker_id, "brawler routed worker heartbeat enqueue failed");
                    worker_control_failure(
                        &mut worker,
                        &mut state,
                        &mut app_exit,
                        &mut commands,
                        worker_entity,
                        FAILURE_DETAIL_MALFORMED,
                    );
                }
            },
            Err(_error) => {
                error!(worker = ?state.worker_id, "brawler routed worker heartbeat encode failed");
                worker_control_failure(
                    &mut worker,
                    &mut state,
                    &mut app_exit,
                    &mut commands,
                    worker_entity,
                    FAILURE_DETAIL_MALFORMED,
                );
            }
        }
    }
}

/// Complete Stop only after the endpoint's normal Unlink fan-out has removed every peer. This
/// makes a successful Exit's terminal counts truthful and leaves the final control flush in the
/// same ordered `Last` chain before `AppExit` is observed.
fn worker_control_finish_shutdown(
    mut state: ResMut<WorkerControlState>,
    mut workers: Query<&mut RoutedWorker>,
) {
    if !state.shutdown_requested || state.exit_sent {
        return;
    }
    let Ok(mut worker) = workers.single_mut() else {
        return;
    };
    if worker.is_failed()
        || worker.peer_count() != 0
        || worker.pending_send_count() != 0
        || worker.channels().packet_pending()
        || worker.channels().control_pending()
    {
        return;
    }
    let Some(stop) = state.pending_stop.take() else {
        return;
    };
    match queue_exit(&mut worker, &mut state, stop) {
        Ok(()) => {}
        Err(_) => {
            state.pending_stop = Some(stop);
        }
    }
}

fn worker_control_flush(
    mut commands: Commands,
    mut app_exit: MessageWriter<AppExit>,
    mut state: ResMut<WorkerControlState>,
    mut workers: Query<(Entity, &mut RoutedWorker)>,
) {
    let Ok((worker_entity, mut worker)) = workers.single_mut() else {
        return;
    };
    if worker.is_failed() {
        return;
    }
    let packet_flush = worker.channels_mut().flush_packet(CONTROL_BURST);
    if let Err(error) = packet_flush
        && !matches!(error, IpcIoError::WouldBlock)
    {
        error!(worker = ?state.worker_id, ?error, "brawler routed worker packet flush failed");
        commands.trigger(RoutedWorkerFailure {
            worker: worker_entity,
            reason: lightyear::prelude::UnlinkReason::TransportError(
                "worker packet stream write failed".to_string(),
            ),
        });
        app_exit.write(AppExit::error());
        return;
    }
    if let Err(error) = worker.flush_pending_sends(CONTROL_BURST)
        && !matches!(error, IpcIoError::WouldBlock)
    {
        error!(worker = ?state.worker_id, ?error, "brawler routed worker pending send drain failed");
        commands.trigger(RoutedWorkerFailure {
            worker: worker_entity,
            reason: lightyear::prelude::UnlinkReason::TransportError(
                "worker pending send drain failed".to_string(),
            ),
        });
        app_exit.write(AppExit::error());
        return;
    }
    if let Err(error) = worker.channels_mut().flush_packet(CONTROL_BURST)
        && !matches!(error, IpcIoError::WouldBlock)
    {
        error!(worker = ?state.worker_id, ?error, "brawler routed worker packet flush failed");
        commands.trigger(RoutedWorkerFailure {
            worker: worker_entity,
            reason: lightyear::prelude::UnlinkReason::TransportError(
                "worker packet stream write failed".to_string(),
            ),
        });
        app_exit.write(AppExit::error());
        return;
    }
    // Result is a cross-stream terminal fact.  Once every transformed payload has left the
    // adapter FIFO and the framed packet writer, half-close only the worker-to-supervisor packet
    // direction.  The supervisor waits for this EOF before revoking the match route, so a Result
    // frame that overtakes packet IPC cannot discard an already-enqueued final gameplay packet.
    if state.result_sent
        && !state.packet_write_closed
        && worker.pending_send_count() == 0
        && !worker.channels().packet_pending()
    {
        if worker.shutdown_packet_write().is_err() {
            error!(worker = ?state.worker_id, "brawler routed worker packet shutdown failed");
            commands.trigger(RoutedWorkerFailure {
                worker: worker_entity,
                reason: lightyear::prelude::UnlinkReason::TransportError(
                    "worker packet stream shutdown failed".to_string(),
                ),
            });
            app_exit.write(AppExit::error());
            return;
        }
        state.packet_write_closed = true;
    }
    if let Err(error) = worker.channels_mut().flush_control(CONTROL_BURST)
        && !matches!(error, IpcIoError::WouldBlock)
    {
        error!(worker = ?state.worker_id, ?error, "brawler routed worker control flush failed");
        commands.trigger(RoutedWorkerFailure {
            worker: worker_entity,
            reason: lightyear::prelude::UnlinkReason::TransportError(
                "worker control stream write failed".to_string(),
            ),
        });
        app_exit.write(AppExit::error());
        return;
    }
    // AppExit is deliberately delayed until the Exit frame and all preceding Result/PeerClose
    // frames have left the nonblocking writer.  Queueing Exit is not delivery: a same-frame
    // process exit would otherwise truncate the final control record under backpressure.
    if state.shutdown_requested
        && state.exit_sent
        && !state.app_exit_requested
        && !worker.channels().control_pending()
        && !worker.channels().packet_pending()
        && worker.pending_send_count() == 0
    {
        state.app_exit_requested = true;
        app_exit.write(AppExit::Success);
    }
}

fn queue_exit(
    worker: &mut RoutedWorker,
    state: &mut WorkerControlState,
    _stop: StopBody,
) -> Result<(), WorkerBootstrapError> {
    if state.exit_sent {
        return Ok(());
    }
    let frame = ControlFrame::from_raw_sequence(
        state.next_sequence,
        state.process_id,
        state.worker_id,
        ControlBody::Exit(brawler_routing::ExitBody {
            role: state.role.wire_role(),
            exit_category: 0,
            result_sent: state.result_sent,
            terminal_peers: u16::try_from(worker.peer_count()).unwrap_or(u16::MAX),
            terminal_queue_bytes: u32::try_from(worker.terminal_queue_bytes()).unwrap_or(u32::MAX),
        }),
    )?;
    worker.channels_mut().enqueue_control(&frame.encode()?)?;
    state.next_sequence = state.next_sequence.saturating_add(1);
    state.exit_sent = true;
    Ok(())
}

fn worker_control_failure(
    worker: &mut RoutedWorker,
    state: &mut WorkerControlState,
    app_exit: &mut MessageWriter<AppExit>,
    commands: &mut Commands,
    worker_entity: Entity,
    detail_code: u32,
) {
    error!(worker = ?state.worker_id, detail_code, "brawler routed worker control protocol failure");
    if !state.exit_sent {
        if let Ok(frame) = ControlFrame::from_raw_sequence(
            state.next_sequence,
            state.process_id,
            state.worker_id,
            ControlBody::Failure(brawler_routing::FailureBody {
                phase: if detail_code == FAILURE_DETAIL_EOF {
                    FAILURE_PHASE_RUNTIME
                } else {
                    FAILURE_PHASE_BOOTSTRAP
                },
                category: FAILURE_CATEGORY_PROTOCOL,
                related_sequence: state.next_sequence,
                detail_code,
            }),
        ) && let Ok(bytes) = frame.encode()
        {
            let _ = worker.channels_mut().enqueue_control(&bytes);
            state.next_sequence = state.next_sequence.saturating_add(1);
        }
        state.exit_sent = true;
    }
    commands.trigger(RoutedWorkerFailure {
        worker: worker_entity,
        reason: lightyear::prelude::UnlinkReason::TransportError(
            "worker control protocol failure".to_string(),
        ),
    });
    app_exit.write(AppExit::error());
}

fn config_from_manifest(
    manifest: &WorkerManifest,
) -> Result<ServerNetworkConfig, WorkerBootstrapError> {
    let mut config = ServerNetworkConfig {
        network_protocol_id: manifest.common().network_protocol,
        ..ServerNetworkConfig::default()
    };
    match manifest {
        WorkerManifest::Lobby(manifest) => {
            let catalog = super::lobby::resolve_operator_catalog(&manifest.raw_catalog)
                .map_err(|_| WorkerBootstrapError::Invalid("invalid lobby game-type catalog"))?;
            config.game_mode = match catalog
                .game_types
                .first()
                .map(|game_type| game_type.mode_definition_id)
            {
                Some(crate::map::WIPEOUT_MODE_DEFINITION) => GameMode::Wipeout,
                Some(crate::map::HOT_ZONE_MODE_DEFINITION) => GameMode::HotZone,
                _ => return Err(WorkerBootstrapError::Invalid("unsupported lobby game mode")),
            };
        }
        WorkerManifest::Match(manifest) => {
            config.game_mode = match manifest.mode {
                brawler_routing::GameMode::Wipeout => GameMode::Wipeout,
                brawler_routing::GameMode::HotZone => GameMode::HotZone,
            };
            config.match_rules_profile = match manifest.rules_profile {
                1 => MatchRulesProfile::Production,
                2 => MatchRulesProfile::ProcessVerification,
                _ => return Err(WorkerBootstrapError::Invalid("unknown match rules profile")),
            };
            config.match_objective_target = Some(manifest.objective_target);
            config.match_duration_ticks = Some(manifest.match_duration_ticks);
            config.match_countdown_ticks = Some(manifest.countdown_ticks);
            config.match_respawn_ticks = Some(manifest.respawn_ticks);
            // The manifest whitelist controls who may join this isolated match. Keep the
            // production endpoint capacity because match composition validates it against the
            // selected rules profile's maximum active-fighter capacity, not only this roster's
            // current participant count.
            config.max_clients = config.max_clients.max(manifest.participants.len());
        }
    }
    Ok(config)
}

/// Parse only the role/identity/path argv admitted by the supervisor process contract.
pub fn parse_worker_arguments(
    arguments: impl IntoIterator<Item = String>,
) -> Result<WorkerLaunchArguments, String> {
    let mut role = None;
    let mut logical_server_id = None;
    let mut supervisor_generation = None;
    let mut worker_id = None;
    let mut process_id = None;
    let mut worker_generation = None;
    let mut packet_socket = None;
    let mut control_socket = None;
    let mut arguments = arguments.into_iter();
    while let Some(flag) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("{flag} requires a value"))?;
        match flag.as_str() {
            "--role" => {
                role = Some(
                    WorkerEntrypointRole::parse(&value)
                        .ok_or_else(|| "invalid worker role".to_string())?,
                );
            }
            "--logical-server-id" => {
                logical_server_id = Some(parse_id(&value, "logical server ID")?);
            }
            "--supervisor-generation" => {
                supervisor_generation = Some(parse_generation(&value, "supervisor generation")?);
            }
            "--worker-id" => worker_id = Some(parse_worker_id(&value)?),
            "--process-id" => process_id = Some(parse_process_id(&value)?),
            "--worker-generation" => {
                worker_generation = Some(parse_generation(&value, "worker generation")?);
            }
            "--packet-socket" => packet_socket = Some(PathBuf::from(value)),
            "--control-socket" => control_socket = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown worker flag: {flag}")),
        }
    }
    Ok(WorkerLaunchArguments {
        role: role.ok_or_else(|| "missing worker role".to_string())?,
        logical_server_id: logical_server_id
            .ok_or_else(|| "missing logical server ID".to_string())?,
        supervisor_generation: supervisor_generation
            .ok_or_else(|| "missing supervisor generation".to_string())?,
        worker_id: worker_id.ok_or_else(|| "missing worker ID".to_string())?,
        process_id: process_id.ok_or_else(|| "missing process ID".to_string())?,
        worker_generation: worker_generation
            .ok_or_else(|| "missing worker generation".to_string())?,
        packet_socket: packet_socket.ok_or_else(|| "missing packet socket".to_string())?,
        control_socket: control_socket.ok_or_else(|| "missing control socket".to_string())?,
    })
}

fn parse_id(value: &str, label: &str) -> Result<brawler_routing::LogicalServerId, String> {
    value
        .parse::<u128>()
        .ok()
        .and_then(brawler_routing::LogicalServerId::new)
        .ok_or_else(|| format!("invalid {label}"))
}

fn parse_generation(value: &str, label: &str) -> Result<brawler_routing::Generation, String> {
    value
        .parse::<u64>()
        .ok()
        .and_then(brawler_routing::Generation::new)
        .ok_or_else(|| format!("invalid {label}"))
}

fn parse_worker_id(value: &str) -> Result<WorkerId, String> {
    value
        .parse::<u128>()
        .ok()
        .and_then(WorkerId::new)
        .ok_or_else(|| "invalid worker ID".to_string())
}

fn parse_process_id(value: &str) -> Result<ProcessId, String> {
    value
        .parse::<u128>()
        .ok()
        .and_then(ProcessId::new)
        .ok_or_else(|| "invalid process ID".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allocation_body(request_id: u64) -> AllocateRequestBody {
        AllocateRequestBody {
            request_id: brawler_routing::RequestId::new(request_id).unwrap(),
            lobby_session_id: brawler_routing::LobbySessionId::new(1).unwrap(),
            mode: brawler_routing::GameMode::Wipeout,
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
            bots: Vec::new(),
        }
    }

    fn netcode_authenticated_body(value: u128) -> LobbyNetcodeAuthenticatedBody {
        LobbyNetcodeAuthenticatedBody {
            route_id: brawler_routing::RouteId::new(value).unwrap(),
            peer_id: brawler_routing::PeerId::new(value + 1).unwrap(),
            netcode_client_id: brawler_routing::NetcodeClientId::new(u64::try_from(value).unwrap())
                .unwrap(),
        }
    }

    #[test]
    fn lobby_outbox_is_bounded_and_ordered() {
        let mut outbox = LobbyControlOutbox::default();
        for request_id in 1..=LOBBY_CONTROL_OUTBOX_FRAMES {
            assert!(outbox.push(allocation_body(request_id as u64)));
        }
        assert!(!outbox.push(allocation_body(99)));
        assert_eq!(outbox.front().unwrap().request_id.get(), 1);
        outbox.pop_front();
        assert_eq!(outbox.front().unwrap().request_id.get(), 2);
    }

    #[test]
    fn netcode_authentication_outbox_is_bounded_and_ordered() {
        let mut outbox = LobbyControlOutbox::default();
        for value in 1..=(LOBBY_CONTROL_OUTBOX_FRAMES * 8) {
            assert!(outbox.push_netcode_authenticated(netcode_authenticated_body(value as u128,)));
        }
        assert!(!outbox.push_netcode_authenticated(netcode_authenticated_body(99)));
        assert_eq!(
            outbox.netcode_authenticated_front().unwrap().route_id.get(),
            1
        );
        outbox.pop_netcode_authenticated();
        assert_eq!(
            outbox.netcode_authenticated_front().unwrap().route_id.get(),
            2
        );
    }

    #[test]
    fn lobby_inbox_is_bounded_and_drains() {
        let process_id = ProcessId::new(1).unwrap();
        let worker_id = WorkerId::new(2).unwrap();
        let mut inbox = LobbyControlInbox::default();
        for sequence in 1..=LOBBY_CONTROL_INBOX_FRAMES {
            let frame = ControlFrame::from_raw_sequence(
                sequence as u64,
                process_id,
                worker_id,
                ControlBody::AllocationRejected(brawler_routing::AllocationRejectedBody {
                    request_id: brawler_routing::RequestId::new(sequence as u64).unwrap(),
                    reason: 1,
                    retry_after_ms: 0,
                }),
            )
            .unwrap();
            assert!(inbox.push(frame));
        }
        let extra = ControlFrame::from_raw_sequence(
            99,
            process_id,
            worker_id,
            ControlBody::AllocationRejected(brawler_routing::AllocationRejectedBody {
                request_id: brawler_routing::RequestId::new(99).unwrap(),
                reason: 1,
                retry_after_ms: 0,
            }),
        )
        .unwrap();
        assert!(!inbox.push(extra));
        assert_eq!(inbox.drain().count(), LOBBY_CONTROL_INBOX_FRAMES);
    }

    #[test]
    fn worker_argv_parser_accepts_only_non_secret_identity_and_paths() {
        let args = parse_worker_arguments(
            [
                "--role",
                "match",
                "--logical-server-id",
                "1",
                "--supervisor-generation",
                "2",
                "--worker-id",
                "3",
                "--process-id",
                "4",
                "--worker-generation",
                "5",
                "--packet-socket",
                "/tmp/p.sock",
                "--control-socket",
                "/tmp/c.sock",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .unwrap();
        assert_eq!(args.role, WorkerEntrypointRole::Match);
        assert_eq!(args.worker_id.get(), 3);
        assert_eq!(args.packet_socket, PathBuf::from("/tmp/p.sock"));
    }

    #[test]
    fn worker_argv_parser_rejects_secret_like_unknown_flags() {
        let result = parse_worker_arguments(["--capability", "secret"].map(str::to_string));
        assert!(result.is_err());
    }

    #[test]
    fn control_eof_is_accepted_with_stop_in_the_same_read() {
        assert!(!control_eof_is_failure(true, false, true));
    }

    #[test]
    fn control_eof_is_accepted_after_a_validated_stop() {
        assert!(!control_eof_is_failure(true, true, false));
    }

    #[test]
    fn unsolicited_control_eof_remains_a_protocol_failure() {
        assert!(control_eof_is_failure(true, false, false));
        assert!(!control_eof_is_failure(false, false, false));
    }

    #[test]
    fn canonical_completed_result_is_versioned_and_stable() {
        let state = MatchState {
            match_id: crate::matchplay::MatchId(9),
            mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
            phase: MatchPhase::Completed {
                completed_at_tick: 12,
                restart_unlocked_at_tick: 72,
                result: crate::matchplay::MatchResult::Forfeit {
                    winner: crate::combat::TeamId(0),
                    departed_team: crate::combat::TeamId(1),
                },
            },
            rules_revision: 1,
        };
        let first = canonical_match_result(&state).expect("completed state emits a result");
        let second = canonical_match_result(&state).expect("same state remains canonical");
        assert_eq!(first, second);
        assert_eq!(first[0], brawler_routing::RESULT_SCHEMA_VERSION_V1);
        assert!(first.len() <= brawler_routing::MAX_RESULT_BYTES);
        assert!(
            canonical_match_result(&MatchState {
                phase: MatchPhase::Waiting,
                ..state
            })
            .is_none()
        );

        let allocation_match_id = brawler_routing::MatchId::new(4).unwrap();
        assert_ne!(state.match_id.0, allocation_match_id.get());
        assert_eq!(
            canonical_allocation_result(&state, allocation_match_id),
            Some(first)
        );
    }
}
