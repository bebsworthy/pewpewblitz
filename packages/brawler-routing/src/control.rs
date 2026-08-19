//! Versioned framed control records exchanged between the supervisor and a worker.

use std::{collections::BTreeMap, fmt};

use crate::{
    AllocationId, CONTROL_HEADER_BYTES, CONTROL_MAGIC, CONTROL_MAX_BODY_BYTES,
    CONTROL_MAX_RECORD_BYTES, CONTROL_PREFIXED_MAX_BYTES, CONTROL_VERSION_CURRENT, Capability,
    CodecError, Generation, LobbySessionId, MAX_LOBBY_MANIFEST_BYTES, MAX_MANIFEST_BYTES,
    MAX_PARTICIPANTS, MAX_RESULT_BYTES, MatchId, NetcodeClientId, PeerId, ProcessId, RequestId,
    RouteId, Sequence, StopId, WorkerId,
    codec::{Decoder, Encoder, FramedDecoder, frame_record},
    digest::result_digest,
    manifest::{GameMode, LobbyManifest, MatchManifestV1, WorkerRole},
};

/// The nineteen control body kinds in the current BRCT contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ControlType {
    Manifest = 1,
    Ready = 2,
    Heartbeat = 3,
    AllocateRequest = 4,
    AllocationGranted = 5,
    AllocationRejected = 6,
    PeerClose = 7,
    Stop = 8,
    Result = 9,
    Failure = 10,
    Exit = 11,
    /// A lobby worker's authenticated routed peer. The supervisor uses this fact only to
    /// promote the exact source already bound to the route out of the pre-auth budget.
    LobbyAuthenticated = 12,
    /// A lobby worker's Netcode-authenticated routed peer, emitted before Brawler hello/session
    /// admission. This promotes only the exact source already bound to the route.
    LobbyNetcodeAuthenticated = 13,
    CancelActivation = 16,
    ActivationDissolved = 17,
    Activated = 18,
    StartFailed = 19,
}

impl TryFrom<u8> for ControlType {
    type Error = CodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Manifest),
            2 => Ok(Self::Ready),
            3 => Ok(Self::Heartbeat),
            4 => Ok(Self::AllocateRequest),
            5 => Ok(Self::AllocationGranted),
            6 => Ok(Self::AllocationRejected),
            7 => Ok(Self::PeerClose),
            8 => Ok(Self::Stop),
            9 => Ok(Self::Result),
            10 => Ok(Self::Failure),
            11 => Ok(Self::Exit),
            12 => Ok(Self::LobbyAuthenticated),
            13 => Ok(Self::LobbyNetcodeAuthenticated),
            16 => Ok(Self::CancelActivation),
            17 => Ok(Self::ActivationDissolved),
            18 => Ok(Self::Activated),
            19 => Ok(Self::StartFailed),
            other => Err(CodecError::UnsupportedType(other)),
        }
    }
}

/// A worker-role tag plus its exact canonical manifest bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct ManifestBody {
    pub role: WorkerRole,
    pub manifest: Vec<u8>,
}

impl fmt::Debug for ManifestBody {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManifestBody")
            .field("role", &self.role)
            .field("manifest_bytes", &self.manifest.len())
            .finish()
    }
}

impl ManifestBody {
    pub fn new(role: WorkerRole, manifest: Vec<u8>) -> Result<Self, CodecError> {
        let body = Self { role, manifest };
        body.validate()?;
        Ok(body)
    }

    pub fn from_lobby(manifest: &LobbyManifest) -> Result<Self, CodecError> {
        Self::new(WorkerRole::Lobby, manifest.encode()?)
    }

    pub fn from_match(manifest: &MatchManifestV1) -> Result<Self, CodecError> {
        Self::new(WorkerRole::Match, manifest.encode()?)
    }

    pub fn validate(&self) -> Result<(), CodecError> {
        if self.manifest.is_empty() {
            return Err(CodecError::InvalidValue);
        }
        match self.role {
            WorkerRole::Lobby => {
                if self.manifest.len() > MAX_LOBBY_MANIFEST_BYTES {
                    return Err(CodecError::Oversize);
                }
                LobbyManifest::decode(&self.manifest)?;
            }
            WorkerRole::Match => {
                if self.manifest.len() > MAX_MANIFEST_BYTES {
                    return Err(CodecError::Oversize);
                }
                MatchManifestV1::decode(&self.manifest)?;
            }
        }
        Ok(())
    }
}

/// Ready acknowledgement and compatibility values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadyBody {
    pub manifest_digest: [u8; 32],
    pub generation: Generation,
    pub route_version: u8,
    pub packet_version: u8,
    pub control_version: u8,
    pub flags: u8,
}

impl ReadyBody {
    fn validate(self) -> Result<(), CodecError> {
        if self.route_version != crate::ROUTE_VERSION_V1
            || self.packet_version != crate::PACKET_VERSION_V1
            || self.control_version != CONTROL_VERSION_CURRENT
        {
            return Err(CodecError::InvalidValue);
        }
        if self.flags != 0 {
            return Err(CodecError::ReservedNonZero);
        }
        Ok(())
    }
}

/// Fixed-size worker health report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeartbeatBody {
    pub generation: Generation,
    pub uptime_ms: u64,
    pub active_peers: u16,
    pub packet_frames: u16,
    pub packet_bytes: u32,
    pub control_frames: u16,
    pub control_bytes: u32,
    pub fixed_tick_lag_us: u32,
    pub health_flags: u32,
}

/// Correlation shared by the match worker's prepare fact and the supervisor's one-shot commit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActivationBody {
    pub request_id: RequestId,
    pub allocation_id: AllocationId,
    pub match_id: MatchId,
}

/// Participant selection sent from the lobby to the supervisor.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AllocateParticipant {
    pub lobby_session_id: LobbySessionId,
    pub player_id: crate::PlayerId,
    pub netcode_client_id: NetcodeClientId,
    pub team: u8,
    pub source_build_preset: Option<u16>,
    pub recipe_fingerprint: u64,
    pub build_revision: u16,
    pub build_snapshot: crate::MatchBuildSnapshot,
}

impl fmt::Debug for AllocateParticipant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AllocateParticipant")
            .field("identity", &"[REDACTED]")
            .field("team", &self.team)
            .field("source_build", &"[REDACTED]")
            .field("build_revision", &self.build_revision)
            .finish_non_exhaustive()
    }
}

fn encode_allocate_participant(encoder: &mut Encoder, participant: &AllocateParticipant) {
    encoder.put_u128(participant.lobby_session_id.get());
    encoder.put_u64(participant.player_id.get());
    encoder.put_u64(participant.netcode_client_id.get());
    encoder.put_u8(participant.team);
    match participant.source_build_preset {
        None => encoder.put_u8(0),
        Some(preset) => {
            encoder.put_u8(1);
            encoder.put_u16(preset);
        }
    }
    encoder.put_u64(participant.recipe_fingerprint);
    encoder.put_u16(participant.build_revision);
    encoder.put_u8(u8::try_from(participant.build_snapshot.as_bytes().len()).expect("bounded"));
    encoder.put_bytes(participant.build_snapshot.as_bytes());
}

fn decode_allocate_participant(
    decoder: &mut Decoder<'_>,
) -> Result<AllocateParticipant, CodecError> {
    let lobby_session_id = LobbySessionId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?;
    let player_id = crate::PlayerId::new(decoder.u64()?).ok_or(CodecError::ZeroId)?;
    let netcode_client_id = NetcodeClientId::new(decoder.u64()?).ok_or(CodecError::ZeroId)?;
    let team = decoder.u8()?;
    let source_build_preset = decoder.optional(Decoder::u16)?;
    let recipe_fingerprint = decoder.u64()?;
    let build_revision = decoder.u16()?;
    let build_length = usize::from(decoder.u8()?);
    Ok(AllocateParticipant {
        lobby_session_id,
        player_id,
        netcode_client_id,
        team,
        source_build_preset,
        recipe_fingerprint,
        build_revision,
        build_snapshot: crate::MatchBuildSnapshot::new(decoder.take(build_length)?)?,
    })
}

/// Idempotent lobby allocation request.
#[derive(Clone, PartialEq, Eq)]
pub struct AllocateRequestBody {
    pub request_id: RequestId,
    pub lobby_session_id: LobbySessionId,
    pub mode: GameMode,
    pub map_preset: u16,
    pub map_revision: u16,
    pub rules_profile: u8,
    pub objective_target: u16,
    pub match_duration_ticks: u64,
    pub countdown_ticks: u64,
    pub respawn_ticks: u64,
    pub team_count: u8,
    pub players_per_team: u8,
    pub participants: Vec<AllocateParticipant>,
}

impl fmt::Debug for AllocateRequestBody {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AllocateRequestBody")
            .field("request_id", &self.request_id)
            .field("mode", &self.mode)
            .field("map_preset", &self.map_preset)
            .field("topology", &(self.team_count, self.players_per_team))
            .field("participant_count", &self.participants.len())
            .finish_non_exhaustive()
    }
}

impl AllocateRequestBody {
    pub fn validate(&self) -> Result<(), CodecError> {
        if self.map_preset == 0
            || self.map_revision == 0
            || self.rules_profile == 0
            || self.objective_target == 0
            || self.match_duration_ticks == 0
            || self.countdown_ticks == 0
            || self.respawn_ticks == 0
            || self.team_count == 0
            || self.players_per_team == 0
            || self.participants.is_empty()
            || self.participants.len() > MAX_PARTICIPANTS
        {
            return Err(CodecError::InvalidValue);
        }
        Ok(())
    }

    pub fn validate_product(&self) -> Result<(), CodecError> {
        self.validate()?;
        let expected = usize::from(self.team_count)
            .checked_mul(usize::from(self.players_per_team))
            .ok_or(CodecError::InvalidValue)?;
        if self.team_count != 2
            || !matches!(self.players_per_team, 1..=3)
            || self.participants.len() != expected
        {
            return Err(CodecError::InvalidValue);
        }
        Ok(())
    }

    /// M01's minimum lobby transaction accepts exactly two participants.  The wire format stays
    /// extensible to eight for later milestones.
    pub fn validate_m01(&self) -> Result<(), CodecError> {
        self.validate()?;
        if self.participants.len() != 2 {
            return Err(CodecError::InvalidValue);
        }
        Ok(())
    }
}

/// One capability-bearing grant.  Its custom Debug implementation is intentionally redacted.
#[derive(Clone, PartialEq, Eq)]
pub struct AllocationGrant {
    pub lobby_session_id: LobbySessionId,
    pub route_id: RouteId,
    pub peer_id: PeerId,
    pub capability: Capability,
    pub activation_expiry_unix_ms: u64,
    pub route_expiry_unix_ms: u64,
}

impl fmt::Debug for AllocationGrant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AllocationGrant")
            .field("lobby_session_id", &self.lobby_session_id)
            .field("route_id", &self.route_id)
            .field("peer_id", &self.peer_id)
            .field("capability", &"[REDACTED]")
            .field("activation_expiry_unix_ms", &self.activation_expiry_unix_ms)
            .field("route_expiry_unix_ms", &self.route_expiry_unix_ms)
            .finish()
    }
}

impl AllocationGrant {
    fn validate(&self) -> Result<(), CodecError> {
        if self.activation_expiry_unix_ms > self.route_expiry_unix_ms {
            return Err(CodecError::InvalidValue);
        }
        Ok(())
    }
}

/// Successful match-worker allocation response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AllocationGrantedBody {
    pub request_id: RequestId,
    pub allocation_id: AllocationId,
    pub match_id: MatchId,
    pub worker_id: WorkerId,
    pub grants: Vec<AllocationGrant>,
}

impl AllocationGrantedBody {
    fn validate(&self) -> Result<(), CodecError> {
        if self.grants.is_empty() || self.grants.len() > MAX_PARTICIPANTS {
            return Err(CodecError::InvalidValue);
        }
        for grant in &self.grants {
            grant.validate()?;
        }
        Ok(())
    }
}

/// Bounded allocation rejection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllocationRejectedBody {
    pub request_id: RequestId,
    pub reason: u16,
    pub retry_after_ms: u32,
}

/// Peer unlink request/notification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerCloseBody {
    pub route_id: RouteId,
    pub peer_id: PeerId,
    pub reason: u16,
}

/// Explicit lobby authentication fact emitted only after the lobby worker has accepted the
/// client's authenticated Netcode identity. The supervisor validates route, peer, worker role,
/// and the source learned from the corresponding public datagram before promoting ingress.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LobbyAuthenticatedBody {
    pub route_id: RouteId,
    pub peer_id: PeerId,
    pub lobby_session_id: LobbySessionId,
    pub netcode_client_id: NetcodeClientId,
}

impl fmt::Debug for LobbyAuthenticatedBody {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LobbyAuthenticatedBody")
            .field("identity", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

/// Explicit post-Netcode authentication fact emitted at the routed Lightyear `Connected`
/// boundary. It intentionally carries no lobby session: Brawler hello compatibility and session
/// admission may still reject the connection, while the cryptographic Netcode identity has
/// already passed the worker's authentication boundary.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LobbyNetcodeAuthenticatedBody {
    pub route_id: RouteId,
    pub peer_id: PeerId,
    pub netcode_client_id: NetcodeClientId,
}

impl fmt::Debug for LobbyNetcodeAuthenticatedBody {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LobbyNetcodeAuthenticatedBody")
            .field("identity", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

/// Idempotent worker stop request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StopBody {
    pub stop_id: StopId,
    pub reason: u16,
    pub graceful_deadline_ms: u32,
}

/// Immutable match result bytes and their domain-separated digest.
#[derive(Clone, PartialEq, Eq)]
pub struct ResultBody {
    pub match_id: MatchId,
    pub allocation_id: AllocationId,
    pub result_digest: [u8; 32],
    pub result: Vec<u8>,
}

/// Version of the bounded canonical match-result bytes carried by [`ResultBody`].
///
/// The supervisor treats the bytes as opaque, but the version is part of the canonical
/// application record so later milestones can extend result semantics without changing the BRCT
/// framing or digest contract.
pub const RESULT_SCHEMA_VERSION_V1: u8 = 1;

impl fmt::Debug for ResultBody {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResultBody")
            .field("match_id", &self.match_id)
            .field("allocation_id", &self.allocation_id)
            .field("result_bytes", &self.result.len())
            .field("result_digest", &"[REDACTED]")
            .finish()
    }
}

impl ResultBody {
    /// Construct a result body and compute its domain-separated digest from the exact canonical
    /// bytes. Callers cannot accidentally advertise a digest for a different byte sequence.
    pub fn new(
        match_id: MatchId,
        allocation_id: AllocationId,
        result: Vec<u8>,
    ) -> Result<Self, CodecError> {
        let body = Self {
            match_id,
            allocation_id,
            result_digest: result_digest(&result),
            result,
        };
        body.validate()?;
        Ok(body)
    }

    fn validate(&self) -> Result<(), CodecError> {
        if self.result.is_empty() || self.result.len() > MAX_RESULT_BYTES {
            return Err(CodecError::InvalidValue);
        }
        if self.result_digest == [0; 32] || self.result_digest != result_digest(&self.result) {
            return Err(CodecError::InvalidValue);
        }
        Ok(())
    }
}

/// Bounded process failure report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FailureBody {
    pub phase: u16,
    pub category: u16,
    pub related_sequence: u64,
    pub detail_code: u32,
}

/// Terminal worker report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExitBody {
    pub role: WorkerRole,
    pub exit_category: u16,
    pub result_sent: bool,
    pub terminal_peers: u16,
    pub terminal_queue_bytes: u32,
}

/// A complete BRCT v1 record without its u32 stream prefix.
#[derive(Clone, PartialEq, Eq)]
pub struct ControlFrame {
    pub sequence: Sequence,
    pub process_id: ProcessId,
    pub worker_id: WorkerId,
    pub body: ControlBody,
}

impl fmt::Debug for ControlFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ControlFrame")
            .field("sequence", &self.sequence)
            .field("process_id", &self.process_id)
            .field("worker_id", &self.worker_id)
            .field("body", &self.body)
            .finish()
    }
}

impl ControlFrame {
    pub fn new(
        sequence: Sequence,
        process_id: ProcessId,
        worker_id: WorkerId,
        body: ControlBody,
    ) -> Result<Self, CodecError> {
        let frame = Self {
            sequence,
            process_id,
            worker_id,
            body: body.with_computed_digests(),
        };
        frame.validate()?;
        Ok(frame)
    }

    pub fn from_raw_sequence(
        sequence: u64,
        process_id: ProcessId,
        worker_id: WorkerId,
        body: ControlBody,
    ) -> Result<Self, CodecError> {
        Self::new(
            Sequence::new(sequence).ok_or(CodecError::ZeroId)?,
            process_id,
            worker_id,
            body,
        )
    }

    fn validate(&self) -> Result<(), CodecError> {
        if self.sequence.get() == 0 || self.process_id.get() == 0 || self.worker_id.get() == 0 {
            return Err(CodecError::ZeroId);
        }
        self.body.validate()
    }

    #[must_use]
    pub fn control_type(&self) -> ControlType {
        self.body.control_type()
    }

    pub fn encode(&self) -> Result<Vec<u8>, CodecError> {
        self.validate()?;
        let body = self.body.encode()?;
        if body.len() > CONTROL_MAX_BODY_BYTES
            || CONTROL_HEADER_BYTES.saturating_add(body.len()) > CONTROL_MAX_RECORD_BYTES
        {
            return Err(CodecError::Oversize);
        }
        let mut encoder = Encoder::with_capacity(CONTROL_HEADER_BYTES + body.len());
        encoder.put_bytes(&CONTROL_MAGIC);
        encoder.put_u8(CONTROL_VERSION_CURRENT);
        encoder.put_u8(self.control_type() as u8);
        encoder.put_u16(0);
        encoder.put_u64(self.sequence.get());
        encoder.put_u128(self.process_id.get());
        encoder.put_u128(self.worker_id.get());
        encoder.put_u32(u32::try_from(body.len()).map_err(|_| CodecError::Oversize)?);
        encoder.put_bytes(&body);
        Ok(encoder.finish())
    }

    pub fn encode_framed(&self) -> Result<Vec<u8>, CodecError> {
        frame_record(&self.encode()?, CONTROL_MAX_RECORD_BYTES)
    }

    /// Decode one exact record and validate its wire identities as nonzero IDs.
    pub fn decode(record: &[u8]) -> Result<Self, CodecError> {
        Self::decode_with_expected(record, None, None)
    }

    /// Decode one record and require the process/worker identity expected by the endpoint.
    pub fn decode_for(
        record: &[u8],
        process_id: ProcessId,
        worker_id: WorkerId,
    ) -> Result<Self, CodecError> {
        Self::decode_with_expected(record, Some(process_id), Some(worker_id))
    }

    fn decode_with_expected(
        record: &[u8],
        expected_process: Option<ProcessId>,
        expected_worker: Option<WorkerId>,
    ) -> Result<Self, CodecError> {
        if record.len() > CONTROL_MAX_RECORD_BYTES {
            return Err(CodecError::Oversize);
        }
        if record.len() < CONTROL_HEADER_BYTES {
            return Err(CodecError::Truncated);
        }
        let mut decoder = Decoder::new(record);
        if decoder.take(4)? != CONTROL_MAGIC {
            return Err(CodecError::InvalidMagic);
        }
        let version = decoder.u8()?;
        let kind_raw = decoder.u8()?;
        let flags = decoder.u16()?;
        let sequence_raw = decoder.u64()?;
        let process_raw = decoder.u128()?;
        let worker_raw = decoder.u128()?;
        let body_length = usize::try_from(decoder.u32()?).map_err(|_| CodecError::Oversize)?;
        if body_length > CONTROL_MAX_BODY_BYTES {
            return Err(CodecError::Oversize);
        }
        if body_length != decoder.remaining() {
            return Err(CodecError::LengthMismatch);
        }
        if version != CONTROL_VERSION_CURRENT {
            return Err(CodecError::UnsupportedVersion(version));
        }
        let kind = ControlType::try_from(kind_raw)?;
        if flags != 0 {
            return Err(CodecError::ReservedNonZero);
        }
        let sequence = Sequence::new(sequence_raw).ok_or(CodecError::ZeroId)?;
        let process_id = ProcessId::new(process_raw).ok_or(CodecError::ZeroId)?;
        let worker_id = WorkerId::new(worker_raw).ok_or(CodecError::ZeroId)?;
        if expected_process.is_some_and(|expected| expected != process_id)
            || expected_worker.is_some_and(|expected| expected != worker_id)
        {
            return Err(CodecError::InvalidValue);
        }
        let body = ControlBody::decode(kind, &mut decoder, body_length)?;
        decoder.finish()?;
        let frame = Self {
            sequence,
            process_id,
            worker_id,
            body,
        };
        frame.validate()?;
        Ok(frame)
    }

    #[must_use]
    pub fn same_content(&self, other: &Self) -> bool {
        self == other
    }
}

/// Typed BRCT body union.
#[derive(Clone, PartialEq, Eq)]
pub enum ControlBody {
    Manifest(ManifestBody),
    Ready(ReadyBody),
    Heartbeat(HeartbeatBody),
    AllocateRequest(AllocateRequestBody),
    AllocationGranted(AllocationGrantedBody),
    AllocationRejected(AllocationRejectedBody),
    PeerClose(PeerCloseBody),
    Stop(StopBody),
    Result(ResultBody),
    Failure(FailureBody),
    Exit(ExitBody),
    LobbyAuthenticated(LobbyAuthenticatedBody),
    LobbyNetcodeAuthenticated(LobbyNetcodeAuthenticatedBody),
    CancelActivation(ActivationBody),
    ActivationDissolved(ActivationBody),
    Activated(ActivationBody),
    StartFailed(ActivationBody),
}

impl fmt::Debug for ControlBody {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manifest(value) => formatter.debug_tuple("Manifest").field(value).finish(),
            Self::Ready(value) => formatter.debug_tuple("Ready").field(value).finish(),
            Self::Heartbeat(value) => formatter.debug_tuple("Heartbeat").field(value).finish(),
            Self::AllocateRequest(value) => formatter
                .debug_tuple("AllocateRequest")
                .field(value)
                .finish(),
            Self::AllocationGranted(value) => formatter
                .debug_tuple("AllocationGranted")
                .field(value)
                .finish(),
            Self::AllocationRejected(value) => formatter
                .debug_tuple("AllocationRejected")
                .field(value)
                .finish(),
            Self::PeerClose(value) => formatter.debug_tuple("PeerClose").field(value).finish(),
            Self::Stop(value) => formatter.debug_tuple("Stop").field(value).finish(),
            Self::Result(value) => formatter.debug_tuple("Result").field(value).finish(),
            Self::Failure(value) => formatter.debug_tuple("Failure").field(value).finish(),
            Self::Exit(value) => formatter.debug_tuple("Exit").field(value).finish(),
            Self::LobbyAuthenticated(value) => formatter
                .debug_tuple("LobbyAuthenticated")
                .field(value)
                .finish(),
            Self::LobbyNetcodeAuthenticated(value) => formatter
                .debug_tuple("LobbyNetcodeAuthenticated")
                .field(value)
                .finish(),
            Self::CancelActivation(value) => formatter
                .debug_tuple("CancelActivation")
                .field(value)
                .finish(),
            Self::ActivationDissolved(value) => formatter
                .debug_tuple("ActivationDissolved")
                .field(value)
                .finish(),
            Self::Activated(value) => formatter.debug_tuple("Activated").field(value).finish(),
            Self::StartFailed(value) => formatter.debug_tuple("StartFailed").field(value).finish(),
        }
    }
}

impl ControlBody {
    fn with_computed_digests(self) -> Self {
        match self {
            Self::Result(mut value) => {
                if value.result_digest == [0; 32] {
                    value.result_digest = result_digest(&value.result);
                }
                Self::Result(value)
            }
            other => other,
        }
    }

    #[must_use]
    pub const fn control_type(&self) -> ControlType {
        match self {
            Self::Manifest(_) => ControlType::Manifest,
            Self::Ready(_) => ControlType::Ready,
            Self::Heartbeat(_) => ControlType::Heartbeat,
            Self::AllocateRequest(_) => ControlType::AllocateRequest,
            Self::AllocationGranted(_) => ControlType::AllocationGranted,
            Self::AllocationRejected(_) => ControlType::AllocationRejected,
            Self::PeerClose(_) => ControlType::PeerClose,
            Self::Stop(_) => ControlType::Stop,
            Self::Result(_) => ControlType::Result,
            Self::Failure(_) => ControlType::Failure,
            Self::Exit(_) => ControlType::Exit,
            Self::LobbyAuthenticated(_) => ControlType::LobbyAuthenticated,
            Self::LobbyNetcodeAuthenticated(_) => ControlType::LobbyNetcodeAuthenticated,
            Self::CancelActivation(_) => ControlType::CancelActivation,
            Self::ActivationDissolved(_) => ControlType::ActivationDissolved,
            Self::Activated(_) => ControlType::Activated,
            Self::StartFailed(_) => ControlType::StartFailed,
        }
    }

    fn validate(&self) -> Result<(), CodecError> {
        match self {
            Self::Manifest(value) => value.validate(),
            Self::Ready(value) => value.validate(),
            Self::Heartbeat(value) => {
                if value.generation.get() == 0 {
                    return Err(CodecError::ZeroId);
                }
                Ok(())
            }
            Self::AllocateRequest(value) => value.validate(),
            Self::AllocationGranted(value) => value.validate(),
            Self::AllocationRejected(value) => {
                if value.request_id.get() == 0 {
                    Err(CodecError::ZeroId)
                } else {
                    Ok(())
                }
            }
            Self::PeerClose(value) => {
                if value.route_id.get() == 0 || value.peer_id.get() == 0 {
                    Err(CodecError::ZeroId)
                } else {
                    Ok(())
                }
            }
            Self::Stop(value) => {
                if value.stop_id.get() == 0 {
                    Err(CodecError::ZeroId)
                } else {
                    Ok(())
                }
            }
            Self::Result(value) => {
                if value.match_id.get() == 0 || value.allocation_id.get() == 0 {
                    return Err(CodecError::ZeroId);
                }
                value.validate()
            }
            Self::Failure(_) | Self::Exit(_) => Ok(()),
            Self::LobbyAuthenticated(value) => {
                if value.route_id.get() == 0
                    || value.peer_id.get() == 0
                    || value.lobby_session_id.get() == 0
                    || value.netcode_client_id.get() == 0
                {
                    Err(CodecError::ZeroId)
                } else {
                    Ok(())
                }
            }
            Self::LobbyNetcodeAuthenticated(value) => {
                if value.route_id.get() == 0
                    || value.peer_id.get() == 0
                    || value.netcode_client_id.get() == 0
                {
                    Err(CodecError::ZeroId)
                } else {
                    Ok(())
                }
            }
            Self::CancelActivation(value)
            | Self::ActivationDissolved(value)
            | Self::Activated(value)
            | Self::StartFailed(value) => {
                if value.request_id.get() == 0
                    || value.allocation_id.get() == 0
                    || value.match_id.get() == 0
                {
                    Err(CodecError::ZeroId)
                } else {
                    Ok(())
                }
            }
        }
    }

    // Keeping every BRCT body encoding in this single exhaustive match makes the wire layout
    // reviewable beside `decode`; splitting four lines into a helper would obscure that symmetry.
    #[allow(clippy::too_many_lines)]
    fn encode(&self) -> Result<Vec<u8>, CodecError> {
        self.validate()?;
        let mut encoder = Encoder::new();
        match self {
            Self::Manifest(value) => {
                encoder.put_u8(value.role as u8);
                encoder.put_u32(
                    u32::try_from(value.manifest.len()).map_err(|_| CodecError::Oversize)?,
                );
                encoder.put_bytes(&value.manifest);
            }
            Self::Ready(value) => {
                encoder.put_bytes(&value.manifest_digest);
                encoder.put_u64(value.generation.get());
                encoder.put_u8(value.route_version);
                encoder.put_u8(value.packet_version);
                encoder.put_u8(value.control_version);
                encoder.put_u8(value.flags);
            }
            Self::Heartbeat(value) => {
                encoder.put_u64(value.generation.get());
                encoder.put_u64(value.uptime_ms);
                encoder.put_u16(value.active_peers);
                encoder.put_u16(value.packet_frames);
                encoder.put_u32(value.packet_bytes);
                encoder.put_u16(value.control_frames);
                encoder.put_u32(value.control_bytes);
                encoder.put_u32(value.fixed_tick_lag_us);
                encoder.put_u32(value.health_flags);
            }
            Self::AllocateRequest(value) => {
                encoder.put_u64(value.request_id.get());
                encoder.put_u128(value.lobby_session_id.get());
                encoder.put_u16(value.mode as u16);
                encoder.put_u16(value.map_preset);
                encoder.put_u16(value.map_revision);
                encoder.put_u8(value.rules_profile);
                encoder.put_u16(value.objective_target);
                encoder.put_u64(value.match_duration_ticks);
                encoder.put_u64(value.countdown_ticks);
                encoder.put_u64(value.respawn_ticks);
                encoder.put_u8(value.team_count);
                encoder.put_u8(value.players_per_team);
                encoder.put_u8(
                    u8::try_from(value.participants.len()).map_err(|_| CodecError::Oversize)?,
                );
                for participant in &value.participants {
                    encode_allocate_participant(&mut encoder, participant);
                }
            }
            Self::AllocationGranted(value) => {
                encoder.put_u64(value.request_id.get());
                encoder.put_u128(value.allocation_id.get());
                encoder.put_u128(value.match_id.get());
                encoder.put_u128(value.worker_id.get());
                encoder.put_u8(u8::try_from(value.grants.len()).map_err(|_| CodecError::Oversize)?);
                for grant in &value.grants {
                    encoder.put_u128(grant.lobby_session_id.get());
                    encoder.put_u128(grant.route_id.get());
                    encoder.put_u128(grant.peer_id.get());
                    encoder.put_bytes(grant.capability.expose_bytes());
                    encoder.put_u64(grant.activation_expiry_unix_ms);
                    encoder.put_u64(grant.route_expiry_unix_ms);
                }
            }
            Self::AllocationRejected(value) => {
                encoder.put_u64(value.request_id.get());
                encoder.put_u16(value.reason);
                encoder.put_u32(value.retry_after_ms);
            }
            Self::PeerClose(value) => {
                encoder.put_u128(value.route_id.get());
                encoder.put_u128(value.peer_id.get());
                encoder.put_u16(value.reason);
            }
            Self::Stop(value) => {
                encoder.put_u64(value.stop_id.get());
                encoder.put_u16(value.reason);
                encoder.put_u32(value.graceful_deadline_ms);
            }
            Self::Result(value) => {
                encoder.put_u128(value.match_id.get());
                encoder.put_u128(value.allocation_id.get());
                encoder.put_bytes(&value.result_digest);
                encoder
                    .put_u32(u32::try_from(value.result.len()).map_err(|_| CodecError::Oversize)?);
                encoder.put_bytes(&value.result);
            }
            Self::Failure(value) => {
                encoder.put_u16(value.phase);
                encoder.put_u16(value.category);
                encoder.put_u64(value.related_sequence);
                encoder.put_u32(value.detail_code);
            }
            Self::Exit(value) => {
                encoder.put_u8(value.role as u8);
                encoder.put_u16(value.exit_category);
                encoder.put_u8(u8::from(value.result_sent));
                encoder.put_u16(value.terminal_peers);
                encoder.put_u32(value.terminal_queue_bytes);
            }
            Self::LobbyAuthenticated(value) => {
                encoder.put_u128(value.route_id.get());
                encoder.put_u128(value.peer_id.get());
                encoder.put_u128(value.lobby_session_id.get());
                encoder.put_u64(value.netcode_client_id.get());
            }
            Self::LobbyNetcodeAuthenticated(value) => {
                encoder.put_u128(value.route_id.get());
                encoder.put_u128(value.peer_id.get());
                encoder.put_u64(value.netcode_client_id.get());
            }
            Self::CancelActivation(value)
            | Self::ActivationDissolved(value)
            | Self::Activated(value)
            | Self::StartFailed(value) => {
                encoder.put_u64(value.request_id.get());
                encoder.put_u128(value.allocation_id.get());
                encoder.put_u128(value.match_id.get());
            }
        }
        Ok(encoder.finish())
    }

    #[allow(clippy::too_many_lines)]
    fn decode(
        kind: ControlType,
        decoder: &mut Decoder<'_>,
        body_length: usize,
    ) -> Result<Self, CodecError> {
        let expected_length = match kind {
            ControlType::Manifest
            | ControlType::AllocateRequest
            | ControlType::AllocationGranted
            | ControlType::Result => None,
            ControlType::Ready => Some(44),
            ControlType::Heartbeat => Some(38),
            ControlType::AllocationRejected | ControlType::Stop => Some(14),
            ControlType::PeerClose => Some(34),
            ControlType::Failure => Some(16),
            ControlType::Exit => Some(10),
            ControlType::LobbyAuthenticated => Some(56),
            ControlType::LobbyNetcodeAuthenticated
            | ControlType::CancelActivation
            | ControlType::ActivationDissolved
            | ControlType::Activated
            | ControlType::StartFailed => Some(40),
        };
        if expected_length.is_some_and(|length| length != body_length) {
            return Err(CodecError::LengthMismatch);
        }
        let body = match kind {
            ControlType::Manifest => {
                let role = WorkerRole::try_from(decoder.u8()?)?;
                let length = usize::try_from(decoder.u32()?).map_err(|_| CodecError::Oversize)?;
                let maximum = match role {
                    WorkerRole::Lobby => MAX_LOBBY_MANIFEST_BYTES,
                    WorkerRole::Match => MAX_MANIFEST_BYTES,
                };
                if length == 0 || length > maximum || 5 + length != body_length {
                    return Err(CodecError::LengthMismatch);
                }
                Self::Manifest(ManifestBody::new(role, decoder.take(length)?.to_vec())?)
            }
            ControlType::Ready => Self::Ready(ReadyBody {
                manifest_digest: decoder.take(32)?.try_into().expect("exact digest width"),
                generation: Generation::new(decoder.u64()?).ok_or(CodecError::ZeroId)?,
                route_version: decoder.u8()?,
                packet_version: decoder.u8()?,
                control_version: decoder.u8()?,
                flags: decoder.u8()?,
            }),
            ControlType::Heartbeat => Self::Heartbeat(HeartbeatBody {
                generation: Generation::new(decoder.u64()?).ok_or(CodecError::ZeroId)?,
                uptime_ms: decoder.u64()?,
                active_peers: decoder.u16()?,
                packet_frames: decoder.u16()?,
                packet_bytes: decoder.u32()?,
                control_frames: decoder.u16()?,
                control_bytes: decoder.u32()?,
                fixed_tick_lag_us: decoder.u32()?,
                health_flags: decoder.u32()?,
            }),
            ControlType::AllocateRequest => {
                let request_id = RequestId::new(decoder.u64()?).ok_or(CodecError::ZeroId)?;
                let lobby_session_id =
                    LobbySessionId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?;
                let mode = GameMode::try_from(decoder.u16()?)?;
                let map_preset = decoder.u16()?;
                let map_revision = decoder.u16()?;
                let rules_profile = decoder.u8()?;
                let objective_target = decoder.u16()?;
                let match_duration_ticks = decoder.u64()?;
                let countdown_ticks = decoder.u64()?;
                let respawn_ticks = decoder.u64()?;
                let team_count = decoder.u8()?;
                let players_per_team = decoder.u8()?;
                let count = usize::from(decoder.u8()?);
                if count == 0 || count > MAX_PARTICIPANTS {
                    return Err(CodecError::InvalidValue);
                }
                let mut participants = Vec::with_capacity(count);
                for _ in 0..count {
                    participants.push(decode_allocate_participant(decoder)?);
                }
                Self::AllocateRequest(AllocateRequestBody {
                    request_id,
                    lobby_session_id,
                    mode,
                    map_preset,
                    map_revision,
                    rules_profile,
                    objective_target,
                    match_duration_ticks,
                    countdown_ticks,
                    respawn_ticks,
                    team_count,
                    players_per_team,
                    participants,
                })
            }
            ControlType::AllocationGranted => {
                let request_id = RequestId::new(decoder.u64()?).ok_or(CodecError::ZeroId)?;
                let allocation_id = AllocationId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?;
                let match_id = MatchId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?;
                let worker_id = WorkerId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?;
                let count = usize::from(decoder.u8()?);
                if count == 0 || count > MAX_PARTICIPANTS {
                    return Err(CodecError::InvalidValue);
                }
                let mut grants = Vec::with_capacity(count);
                for _ in 0..count {
                    let lobby_session_id =
                        LobbySessionId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?;
                    let route_id = RouteId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?;
                    let peer_id = PeerId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?;
                    let capability = Capability::from_bytes(
                        decoder
                            .take(32)?
                            .try_into()
                            .expect("exact capability width"),
                    )
                    .ok_or(CodecError::InvalidValue)?;
                    grants.push(AllocationGrant {
                        lobby_session_id,
                        route_id,
                        peer_id,
                        capability,
                        activation_expiry_unix_ms: decoder.u64()?,
                        route_expiry_unix_ms: decoder.u64()?,
                    });
                }
                Self::AllocationGranted(AllocationGrantedBody {
                    request_id,
                    allocation_id,
                    match_id,
                    worker_id,
                    grants,
                })
            }
            ControlType::AllocationRejected => Self::AllocationRejected(AllocationRejectedBody {
                request_id: RequestId::new(decoder.u64()?).ok_or(CodecError::ZeroId)?,
                reason: decoder.u16()?,
                retry_after_ms: decoder.u32()?,
            }),
            ControlType::PeerClose => Self::PeerClose(PeerCloseBody {
                route_id: RouteId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?,
                peer_id: PeerId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?,
                reason: decoder.u16()?,
            }),
            ControlType::Stop => Self::Stop(StopBody {
                stop_id: StopId::new(decoder.u64()?).ok_or(CodecError::ZeroId)?,
                reason: decoder.u16()?,
                graceful_deadline_ms: decoder.u32()?,
            }),
            ControlType::Result => {
                let match_id = MatchId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?;
                let allocation_id = AllocationId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?;
                let digest: [u8; 32] = decoder.take(32)?.try_into().expect("exact digest width");
                let length = usize::try_from(decoder.u32()?).map_err(|_| CodecError::Oversize)?;
                if length == 0 || length > MAX_RESULT_BYTES || 68 + length != body_length {
                    return Err(CodecError::LengthMismatch);
                }
                Self::Result(ResultBody {
                    match_id,
                    allocation_id,
                    result_digest: digest,
                    result: decoder.take(length)?.to_vec(),
                })
            }
            ControlType::Failure => Self::Failure(FailureBody {
                phase: decoder.u16()?,
                category: decoder.u16()?,
                related_sequence: decoder.u64()?,
                detail_code: decoder.u32()?,
            }),
            ControlType::Exit => Self::Exit(ExitBody {
                role: WorkerRole::try_from(decoder.u8()?)?,
                exit_category: decoder.u16()?,
                result_sent: decoder.boolean()?,
                terminal_peers: decoder.u16()?,
                terminal_queue_bytes: decoder.u32()?,
            }),
            ControlType::LobbyAuthenticated => Self::LobbyAuthenticated(LobbyAuthenticatedBody {
                route_id: RouteId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?,
                peer_id: PeerId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?,
                lobby_session_id: LobbySessionId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?,
                netcode_client_id: NetcodeClientId::new(decoder.u64()?)
                    .ok_or(CodecError::ZeroId)?,
            }),
            ControlType::LobbyNetcodeAuthenticated => {
                Self::LobbyNetcodeAuthenticated(LobbyNetcodeAuthenticatedBody {
                    route_id: RouteId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?,
                    peer_id: PeerId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?,
                    netcode_client_id: NetcodeClientId::new(decoder.u64()?)
                        .ok_or(CodecError::ZeroId)?,
                })
            }
            ControlType::CancelActivation
            | ControlType::ActivationDissolved
            | ControlType::Activated
            | ControlType::StartFailed => {
                let value = ActivationBody {
                    request_id: RequestId::new(decoder.u64()?).ok_or(CodecError::ZeroId)?,
                    allocation_id: AllocationId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?,
                    match_id: MatchId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?,
                };
                match kind {
                    ControlType::CancelActivation => Self::CancelActivation(value),
                    ControlType::ActivationDissolved => Self::ActivationDissolved(value),
                    ControlType::Activated => Self::Activated(value),
                    ControlType::StartFailed => Self::StartFailed(value),
                    _ => unreachable!("activation kinds were matched above"),
                }
            }
        };
        body.validate()?;
        Ok(body)
    }
}

/// Stream-level sequence disposition. Exact duplicates are ignored; same-sequence different
/// content is a protocol failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SequenceDisposition {
    Accepted,
    Duplicate,
}

/// Bounded duplicate detector for one control direction.
#[derive(Clone, Debug)]
pub struct ControlSequenceTracker {
    maximum_entries: usize,
    seen: BTreeMap<Sequence, ControlFrame>,
    highest: Option<Sequence>,
}

impl Default for ControlSequenceTracker {
    fn default() -> Self {
        Self::new(64)
    }
}

impl ControlSequenceTracker {
    #[must_use]
    pub fn new(maximum_entries: usize) -> Self {
        Self {
            maximum_entries: maximum_entries.max(1),
            seen: BTreeMap::new(),
            highest: None,
        }
    }

    pub fn observe(&mut self, frame: ControlFrame) -> Result<SequenceDisposition, CodecError> {
        if let Some(previous) = self.seen.get(&frame.sequence) {
            if previous.same_content(&frame) {
                return Ok(SequenceDisposition::Duplicate);
            }
            return Err(CodecError::InvalidValue);
        }
        if self
            .highest
            .is_some_and(|highest| frame.sequence.get() < highest.get())
        {
            return Err(CodecError::InvalidValue);
        }
        self.highest = Some(frame.sequence);
        self.seen.insert(frame.sequence, frame);
        while self.seen.len() > self.maximum_entries {
            let Some(first) = self.seen.keys().next().copied() else {
                break;
            };
            self.seen.remove(&first);
        }
        Ok(SequenceDisposition::Accepted)
    }
}

/// Framed BRCT stream decoder with process/worker identity and sequence validation.
#[derive(Clone, Debug)]
pub struct ControlStreamDecoder {
    framed: FramedDecoder,
    process_id: ProcessId,
    worker_id: WorkerId,
    sequences: ControlSequenceTracker,
}

impl ControlStreamDecoder {
    #[must_use]
    pub fn new(process_id: ProcessId, worker_id: WorkerId) -> Self {
        Self {
            framed: FramedDecoder::new(CONTROL_MAX_RECORD_BYTES),
            process_id,
            worker_id,
            sequences: ControlSequenceTracker::default(),
        }
    }

    pub fn push(&mut self, incoming: &[u8]) -> Result<Vec<ControlFrame>, CodecError> {
        let records = self.framed.push(incoming)?;
        let mut frames = Vec::with_capacity(records.len());
        for record in records {
            let frame = ControlFrame::decode_for(&record, self.process_id, self.worker_id)?;
            if self.sequences.observe(frame.clone())? == SequenceDisposition::Accepted {
                frames.push(frame);
            }
        }
        Ok(frames)
    }

    #[must_use]
    pub fn buffered_bytes(&self) -> usize {
        self.framed.buffered_bytes()
    }

    #[must_use]
    pub const fn maximum_framed_bytes() -> usize {
        CONTROL_PREFIXED_MAX_BYTES
    }
}

/// Compatibility aliases for code that spells body names without the `Body` suffix.
pub type Manifest = ManifestBody;
pub type Ready = ReadyBody;
pub type Heartbeat = HeartbeatBody;
pub type AllocateRequest = AllocateRequestBody;
pub type AllocationGranted = AllocationGrantedBody;
pub type AllocationRejected = AllocationRejectedBody;
pub type PeerClose = PeerCloseBody;
pub type LobbyAuthenticated = LobbyAuthenticatedBody;
pub type Stop = StopBody;
pub type Failure = FailureBody;
pub type Exit = ExitBody;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CONTROL_HEADER_BYTES, CONTROL_MAX_RECORD_BYTES, CONTROL_PREFIXED_MAX_BYTES, LobbyManifest,
        ManifestCommon, WorkerRole,
    };

    fn id128<T: TryFrom<u128>>(value: u128) -> T
    where
        T::Error: fmt::Debug,
    {
        T::try_from(value).unwrap()
    }

    fn id64<T: TryFrom<u64>>(value: u64) -> T
    where
        T::Error: fmt::Debug,
    {
        T::try_from(value).unwrap()
    }

    fn lobby_manifest() -> LobbyManifest {
        LobbyManifest {
            common: ManifestCommon {
                manifest_version: 1,
                role: WorkerRole::Lobby,
                logical_server_id: id128(1),
                process_id: id128(2),
                worker_id: id128(3),
                generation: id64(4),
                network_protocol: 5,
                protocol_registry_fingerprint: 6,
                content_fingerprint: 7,
                route_version: 1,
                packet_version: 1,
                control_version: CONTROL_VERSION_CURRENT,
                flags: 0,
            },
            default_route_id: id128(8),
            max_authenticated_sessions: 32,
            outstanding_allocations: 2,
            active_matches: 4,
            heartbeat_ms: 1_000,
            raw_catalog: b"catalog".to_vec(),
            raw_catalog_fingerprint: crate::raw_catalog_fingerprint(b"catalog"),
            nonce: 9,
            digest: [0; 32],
        }
    }

    fn participant(index: u128) -> AllocateParticipant {
        let small = u16::try_from(index).unwrap();
        let wide = u64::try_from(index).unwrap();
        AllocateParticipant {
            lobby_session_id: id128(index + 10),
            player_id: id64(wide + 20),
            netcode_client_id: id64(wide + 25),
            team: u8::try_from(index % 2).unwrap(),
            source_build_preset: Some(small),
            recipe_fingerprint: wide + 30,
            build_revision: small,
            build_snapshot: crate::MatchBuildSnapshot::new(&[1, 2, 3]).unwrap(),
        }
    }

    fn frame(body: ControlBody) -> ControlFrame {
        ControlFrame::from_raw_sequence(1, id128(2), id128(3), body).unwrap()
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn fixed_body_lengths_match_the_contract() {
        assert_eq!(
            frame(ControlBody::Ready(ReadyBody {
                manifest_digest: [1; 32],
                generation: id64(1),
                route_version: 1,
                packet_version: 1,
                control_version: CONTROL_VERSION_CURRENT,
                flags: 0,
            }))
            .encode()
            .unwrap()
            .len(),
            CONTROL_HEADER_BYTES + 44
        );
        assert_eq!(
            frame(ControlBody::Heartbeat(HeartbeatBody {
                generation: id64(1),
                uptime_ms: 2,
                active_peers: 3,
                packet_frames: 4,
                packet_bytes: 5,
                control_frames: 6,
                control_bytes: 7,
                fixed_tick_lag_us: 8,
                health_flags: 9,
            }))
            .encode()
            .unwrap()
            .len(),
            CONTROL_HEADER_BYTES + 38
        );
        assert_eq!(
            frame(ControlBody::AllocationRejected(AllocationRejectedBody {
                request_id: id64(1),
                reason: 2,
                retry_after_ms: 3,
            }))
            .encode()
            .unwrap()
            .len(),
            CONTROL_HEADER_BYTES + 14
        );
        assert_eq!(
            frame(ControlBody::PeerClose(PeerCloseBody {
                route_id: id128(1),
                peer_id: id128(2),
                reason: 3,
            }))
            .encode()
            .unwrap()
            .len(),
            CONTROL_HEADER_BYTES + 34
        );
        assert_eq!(
            frame(ControlBody::LobbyAuthenticated(LobbyAuthenticatedBody {
                route_id: id128(1),
                peer_id: id128(2),
                lobby_session_id: id128(3),
                netcode_client_id: id64(4),
            }))
            .encode()
            .unwrap()
            .len(),
            CONTROL_HEADER_BYTES + 56
        );
        assert_eq!(
            frame(ControlBody::LobbyNetcodeAuthenticated(
                LobbyNetcodeAuthenticatedBody {
                    route_id: id128(1),
                    peer_id: id128(2),
                    netcode_client_id: id64(4),
                }
            ))
            .encode()
            .unwrap()
            .len(),
            CONTROL_HEADER_BYTES + 40
        );
        assert_eq!(
            frame(ControlBody::Stop(StopBody {
                stop_id: id64(1),
                reason: 2,
                graceful_deadline_ms: 3,
            }))
            .encode()
            .unwrap()
            .len(),
            CONTROL_HEADER_BYTES + 14
        );
        assert_eq!(
            frame(ControlBody::Failure(FailureBody {
                phase: 1,
                category: 2,
                related_sequence: 3,
                detail_code: 4,
            }))
            .encode()
            .unwrap()
            .len(),
            CONTROL_HEADER_BYTES + 16
        );
        assert_eq!(
            frame(ControlBody::Exit(ExitBody {
                role: WorkerRole::Match,
                exit_category: 1,
                result_sent: true,
                terminal_peers: 2,
                terminal_queue_bytes: 3,
            }))
            .encode()
            .unwrap()
            .len(),
            CONTROL_HEADER_BYTES + 10
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn every_typed_body_round_trips_and_maxima_are_bounded() {
        let manifest = ManifestBody::from_lobby(&lobby_manifest()).unwrap();
        let capability = Capability::from_bytes([0x5a; 32]).unwrap();
        let grant = AllocationGrant {
            lobby_session_id: id128(1),
            route_id: id128(2),
            peer_id: id128(3),
            capability,
            activation_expiry_unix_ms: 4,
            route_expiry_unix_ms: 5,
        };
        let bodies = vec![
            ControlBody::Manifest(manifest),
            ControlBody::Ready(ReadyBody {
                manifest_digest: [1; 32],
                generation: id64(1),
                route_version: 1,
                packet_version: 1,
                control_version: CONTROL_VERSION_CURRENT,
                flags: 0,
            }),
            ControlBody::Heartbeat(HeartbeatBody {
                generation: id64(1),
                uptime_ms: 2,
                active_peers: 3,
                packet_frames: 4,
                packet_bytes: 5,
                control_frames: 6,
                control_bytes: 7,
                fixed_tick_lag_us: 8,
                health_flags: 9,
            }),
            ControlBody::AllocateRequest(AllocateRequestBody {
                request_id: id64(1),
                lobby_session_id: id128(2),
                mode: GameMode::Wipeout,
                map_preset: 1,
                map_revision: 1,
                rules_profile: 1,
                objective_target: 10,
                match_duration_ticks: 10_800,
                countdown_ticks: 180,
                respawn_ticks: 180,
                team_count: 2,
                players_per_team: 1,
                participants: vec![participant(1), participant(2)],
            }),
            ControlBody::AllocationGranted(AllocationGrantedBody {
                request_id: id64(1),
                allocation_id: id128(2),
                match_id: id128(3),
                worker_id: id128(4),
                grants: vec![grant],
            }),
            ControlBody::AllocationRejected(AllocationRejectedBody {
                request_id: id64(1),
                reason: 2,
                retry_after_ms: 3,
            }),
            ControlBody::PeerClose(PeerCloseBody {
                route_id: id128(1),
                peer_id: id128(2),
                reason: 3,
            }),
            ControlBody::LobbyAuthenticated(LobbyAuthenticatedBody {
                route_id: id128(1),
                peer_id: id128(2),
                lobby_session_id: id128(3),
                netcode_client_id: id64(4),
            }),
            ControlBody::LobbyNetcodeAuthenticated(LobbyNetcodeAuthenticatedBody {
                route_id: id128(1),
                peer_id: id128(2),
                netcode_client_id: id64(4),
            }),
            ControlBody::CancelActivation(ActivationBody {
                request_id: id64(1),
                allocation_id: id128(2),
                match_id: id128(3),
            }),
            ControlBody::ActivationDissolved(ActivationBody {
                request_id: id64(1),
                allocation_id: id128(2),
                match_id: id128(3),
            }),
            ControlBody::Activated(ActivationBody {
                request_id: id64(1),
                allocation_id: id128(2),
                match_id: id128(3),
            }),
            ControlBody::StartFailed(ActivationBody {
                request_id: id64(1),
                allocation_id: id128(2),
                match_id: id128(3),
            }),
            ControlBody::Stop(StopBody {
                stop_id: id64(1),
                reason: 2,
                graceful_deadline_ms: 3,
            }),
            ControlBody::Result(ResultBody {
                match_id: id128(1),
                allocation_id: id128(2),
                result_digest: [0; 32],
                result: vec![7, 8, 9],
            }),
            ControlBody::Failure(FailureBody {
                phase: 1,
                category: 2,
                related_sequence: 3,
                detail_code: 4,
            }),
            ControlBody::Exit(ExitBody {
                role: WorkerRole::Match,
                exit_category: 1,
                result_sent: true,
                terminal_peers: 2,
                terminal_queue_bytes: 3,
            }),
        ];
        for body in bodies {
            let encoded = frame(body.clone()).encode().unwrap();
            let decoded = ControlFrame::decode(&encoded).unwrap();
            assert_eq!(decoded.body, body.with_computed_digests());
        }

        let request = AllocateRequestBody {
            request_id: id64(1),
            lobby_session_id: id128(2),
            mode: GameMode::HotZone,
            map_preset: 1,
            map_revision: 1,
            rules_profile: 1,
            objective_target: 10,
            match_duration_ticks: 10_800,
            countdown_ticks: 180,
            respawn_ticks: 180,
            team_count: 2,
            players_per_team: 4,
            participants: (1..=8).map(participant).collect(),
        };
        assert_eq!(
            frame(ControlBody::AllocateRequest(request))
                .encode()
                .unwrap()
                .len(),
            52 + 460
        );
        let granted = AllocationGrantedBody {
            request_id: id64(1),
            allocation_id: id128(2),
            match_id: id128(3),
            worker_id: id128(4),
            grants: (1..=8)
                .map(|index| AllocationGrant {
                    lobby_session_id: id128(index),
                    route_id: id128(index + 10),
                    peer_id: id128(index + 20),
                    capability: Capability::from_bytes([u8::try_from(index).unwrap(); 32]).unwrap(),
                    activation_expiry_unix_ms: 1,
                    route_expiry_unix_ms: 2,
                })
                .collect(),
        };
        assert_eq!(
            frame(ControlBody::AllocationGranted(granted))
                .encode()
                .unwrap()
                .len(),
            52 + 825
        );
        assert_eq!(CONTROL_PREFIXED_MAX_BYTES, CONTROL_MAX_RECORD_BYTES + 4);
    }

    #[test]
    fn result_decode_rejects_zero_and_mismatched_digests() {
        let frame = frame(ControlBody::Result(ResultBody {
            match_id: id128(1),
            allocation_id: id128(2),
            result_digest: [0; 32],
            result: vec![7, 8, 9],
        }));
        let encoded = frame.encode().unwrap();
        let digest_start = CONTROL_HEADER_BYTES + 32;

        let mut zero_digest = encoded.clone();
        zero_digest[digest_start..digest_start + 32].fill(0);
        assert_eq!(
            ControlFrame::decode(&zero_digest),
            Err(CodecError::InvalidValue)
        );

        let mut mismatched_digest = encoded;
        mismatched_digest[digest_start..digest_start + 32].fill(0xa5);
        assert_eq!(
            ControlFrame::decode(&mismatched_digest),
            Err(CodecError::InvalidValue)
        );
    }

    #[test]
    fn result_constructor_is_versioned_bounded_and_digest_exact_bytes() {
        let result =
            ResultBody::new(id128(1), id128(2), vec![RESULT_SCHEMA_VERSION_V1, 7, 8]).unwrap();
        assert_eq!(result.result[0], RESULT_SCHEMA_VERSION_V1);
        assert_eq!(result.result_digest, result_digest(&result.result));
        assert!(ResultBody::new(id128(1), id128(2), Vec::new()).is_err());
        assert!(ResultBody::new(id128(1), id128(2), vec![0; MAX_RESULT_BYTES + 1]).is_err());
    }

    #[test]
    fn allocation_debug_redacts_player_and_build_selection_details() {
        let request = AllocateRequestBody {
            request_id: id64(1),
            lobby_session_id: id128(2),
            mode: GameMode::Wipeout,
            map_preset: 1,
            map_revision: 1,
            rules_profile: 1,
            objective_target: 10,
            match_duration_ticks: 10_800,
            countdown_ticks: 180,
            respawn_ticks: 180,
            team_count: 1,
            players_per_team: 1,
            participants: vec![participant(3)],
        };
        let debug = format!("{request:?} {:?}", request.participants[0]);
        assert!(debug.contains("participant_count: 1"));
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains("13"));
        assert!(!debug.contains("23"));
        assert!(!debug.contains("33"));
    }

    #[test]
    fn stream_decoder_validates_identity_and_ignores_only_identical_duplicates() {
        let process_id = id128(2);
        let worker_id = id128(3);
        let first = frame(ControlBody::Stop(StopBody {
            stop_id: id64(4),
            reason: 5,
            graceful_deadline_ms: 6,
        }));
        let mut stream = ControlStreamDecoder::new(process_id, worker_id);
        let encoded = first.encode_framed().unwrap();
        assert_eq!(stream.push(&encoded).unwrap(), vec![first.clone()]);
        assert!(stream.push(&encoded).unwrap().is_empty());

        let mut conflicting = first.clone();
        conflicting.body = ControlBody::Stop(StopBody {
            stop_id: id64(99),
            reason: 5,
            graceful_deadline_ms: 6,
        });
        assert_eq!(
            stream.push(&conflicting.encode_framed().unwrap()),
            Err(CodecError::InvalidValue)
        );

        let wrong_process = ControlFrame::from_raw_sequence(
            2,
            id128(99),
            worker_id,
            ControlBody::Stop(StopBody {
                stop_id: id64(4),
                reason: 5,
                graceful_deadline_ms: 6,
            }),
        )
        .unwrap();
        assert_eq!(
            stream.push(&wrong_process.encode_framed().unwrap()),
            Err(CodecError::InvalidValue)
        );
    }

    #[test]
    fn capability_and_payload_debug_are_redacted() {
        let capability = Capability::from_bytes([0x5a; 32]).unwrap();
        let body = ControlBody::AllocationGranted(AllocationGrantedBody {
            request_id: id64(1),
            allocation_id: id128(2),
            match_id: id128(3),
            worker_id: id128(4),
            grants: vec![AllocationGrant {
                lobby_session_id: id128(5),
                route_id: id128(6),
                peer_id: id128(7),
                capability,
                activation_expiry_unix_ms: 8,
                route_expiry_unix_ms: 9,
            }],
        });
        let debug = format!("{body:?}");
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains("5a"));
    }
}
