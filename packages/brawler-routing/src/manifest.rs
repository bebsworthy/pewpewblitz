//! Canonical v1 lobby and match worker manifests.
//!
//! The manifest bytes are deliberately independent of the control-frame envelope.  A manifest's
//! digest covers the exact canonical bytes before its trailing digest field, including its common
//! header, and never includes a process-local value or an outer length prefix.

use std::fmt;

use sha2::{Digest as _, Sha256};

use crate::{
    CodecError, Generation, LobbySessionId, LogicalServerId, MAX_LOBBY_CATALOG_BYTES,
    MAX_LOBBY_MANIFEST_BYTES, MAX_MANIFEST_BYTES, MAX_PARTICIPANTS, MatchId, PeerId, ProcessId,
    RequestId, WorkerId,
    codec::{Decoder, Encoder},
    digest::manifest_digest,
    limits::{CONTROL_VERSION_CURRENT, PACKET_VERSION_V1, ROUTE_VERSION_V1},
};

/// Worker role carried by manifests and control bodies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WorkerRole {
    Lobby = 1,
    Match = 2,
}

impl TryFrom<u8> for WorkerRole {
    type Error = CodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Lobby),
            2 => Ok(Self::Match),
            _ => Err(CodecError::InvalidValue),
        }
    }
}

/// M01's two supported game modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum GameMode {
    Wipeout = 1,
    HotZone = 2,
}

impl TryFrom<u16> for GameMode {
    type Error = CodecError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Wipeout),
            2 => Ok(Self::HotZone),
            _ => Err(CodecError::InvalidValue),
        }
    }
}

/// Common identity and compatibility fields shared by both manifest forms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ManifestCommon {
    pub manifest_version: u16,
    pub role: WorkerRole,
    pub logical_server_id: LogicalServerId,
    pub process_id: ProcessId,
    pub worker_id: WorkerId,
    pub generation: Generation,
    pub network_protocol: u64,
    pub protocol_registry_fingerprint: u64,
    pub content_fingerprint: u64,
    pub route_version: u8,
    pub packet_version: u8,
    pub control_version: u8,
    pub flags: u8,
}

impl ManifestCommon {
    fn validate(self, expected_role: WorkerRole) -> Result<(), CodecError> {
        let expected_version = match expected_role {
            WorkerRole::Lobby => 1,
            WorkerRole::Match => 2,
        };
        if self.manifest_version != expected_version || self.role != expected_role {
            return Err(CodecError::InvalidValue);
        }
        if self.route_version != ROUTE_VERSION_V1
            || self.packet_version != PACKET_VERSION_V1
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

fn encode_common(encoder: &mut Encoder, common: ManifestCommon) {
    encoder.put_u16(common.manifest_version);
    encoder.put_u8(common.role as u8);
    encoder.put_u128(common.logical_server_id.get());
    encoder.put_u128(common.process_id.get());
    encoder.put_u128(common.worker_id.get());
    encoder.put_u64(common.generation.get());
    encoder.put_u64(common.network_protocol);
    encoder.put_u64(common.protocol_registry_fingerprint);
    encoder.put_u64(common.content_fingerprint);
    encoder.put_u8(common.route_version);
    encoder.put_u8(common.packet_version);
    encoder.put_u8(common.control_version);
    encoder.put_u8(common.flags);
}

fn decode_common(
    decoder: &mut Decoder<'_>,
    expected_role: WorkerRole,
) -> Result<ManifestCommon, CodecError> {
    let common = ManifestCommon {
        manifest_version: decoder.u16()?,
        role: WorkerRole::try_from(decoder.u8()?)?,
        logical_server_id: LogicalServerId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?,
        process_id: ProcessId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?,
        worker_id: WorkerId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?,
        generation: Generation::new(decoder.u64()?).ok_or(CodecError::ZeroId)?,
        network_protocol: decoder.u64()?,
        protocol_registry_fingerprint: decoder.u64()?,
        content_fingerprint: decoder.u64()?,
        route_version: decoder.u8()?,
        packet_version: decoder.u8()?,
        control_version: decoder.u8()?,
        flags: decoder.u8()?,
    };
    common.validate(expected_role)?;
    Ok(common)
}

fn validate_digest(bytes: &[u8], digest: [u8; 32]) -> Result<(), CodecError> {
    if manifest_digest(bytes) != digest {
        return Err(CodecError::InvalidValue);
    }
    Ok(())
}

/// The one current lobby-worker manifest.
#[derive(Clone, PartialEq, Eq)]
pub struct LobbyManifest {
    pub common: ManifestCommon,
    pub default_route_id: crate::RouteId,
    pub max_authenticated_sessions: u16,
    pub outstanding_allocations: u16,
    pub active_matches: u16,
    pub heartbeat_ms: u32,
    /// Opaque operator configuration. Only the lobby worker parses these bytes.
    pub raw_catalog: Vec<u8>,
    pub raw_catalog_fingerprint: [u8; 32],
    pub nonce: u128,
    pub digest: [u8; 32],
}

impl fmt::Debug for LobbyManifest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LobbyManifest")
            .field("common", &self.common)
            .field("default_route_id", &self.default_route_id)
            .field(
                "max_authenticated_sessions",
                &self.max_authenticated_sessions,
            )
            .field("outstanding_allocations", &self.outstanding_allocations)
            .field("active_matches", &self.active_matches)
            .field("heartbeat_ms", &self.heartbeat_ms)
            .field("raw_catalog_bytes", &self.raw_catalog.len())
            .field("raw_catalog_fingerprint", &"[REDACTED]")
            .field("nonce", &"[REDACTED]")
            .field("digest", &"[REDACTED]")
            .finish()
    }
}

impl LobbyManifest {
    pub const ROLE: WorkerRole = WorkerRole::Lobby;

    /// Return a manifest with its digest populated from canonical fields.
    pub fn with_digest(mut self) -> Result<Self, CodecError> {
        self.digest = self.compute_digest()?;
        Ok(self)
    }

    pub fn compute_digest(&self) -> Result<[u8; 32], CodecError> {
        Ok(manifest_digest(&self.encode_prefix()?))
    }

    fn encode_prefix(&self) -> Result<Vec<u8>, CodecError> {
        self.validate()?;
        let mut encoder = Encoder::with_capacity(256 + self.raw_catalog.len());
        encode_common(&mut encoder, self.common);
        encoder.put_u128(self.default_route_id.get());
        encoder.put_u16(self.max_authenticated_sessions);
        encoder.put_u16(self.outstanding_allocations);
        encoder.put_u16(self.active_matches);
        encoder.put_u32(self.heartbeat_ms);
        encoder.put_u32(u32::try_from(self.raw_catalog.len()).map_err(|_| CodecError::Oversize)?);
        encoder.put_bytes(&self.raw_catalog);
        encoder.put_bytes(&self.raw_catalog_fingerprint);
        encoder.put_u128(self.nonce);
        Ok(encoder.finish())
    }

    /// Encode the exact canonical manifest bytes.  A zero digest is treated as an explicit
    /// request to fill the digest, which makes struct literals convenient while preserving
    /// strict verification for nonzero supplied digests.
    pub fn encode(&self) -> Result<Vec<u8>, CodecError> {
        let prefix = self.encode_prefix()?;
        let digest = if self.digest == [0; 32] {
            manifest_digest(&prefix)
        } else {
            validate_digest(&prefix, self.digest)?;
            self.digest
        };
        let mut bytes = prefix;
        bytes.extend_from_slice(&digest);
        if bytes.len() > MAX_LOBBY_MANIFEST_BYTES {
            return Err(CodecError::Oversize);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() > MAX_LOBBY_MANIFEST_BYTES {
            return Err(CodecError::Oversize);
        }
        if bytes.len() < 32 {
            return Err(CodecError::Truncated);
        }
        let split = bytes.len() - 32;
        let mut decoder = Decoder::new(&bytes[..split]);
        let common = decode_common(&mut decoder, Self::ROLE)?;
        let default_route_id = crate::RouteId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?;
        let max_authenticated_sessions = decoder.u16()?;
        let outstanding_allocations = decoder.u16()?;
        let active_matches = decoder.u16()?;
        let heartbeat_ms = decoder.u32()?;
        let catalog_length = usize::try_from(decoder.u32()?).map_err(|_| CodecError::Oversize)?;
        if catalog_length == 0 || catalog_length > MAX_LOBBY_CATALOG_BYTES {
            return Err(CodecError::Oversize);
        }
        let raw_catalog = decoder.take(catalog_length)?.to_vec();
        let value = Self {
            common,
            default_route_id,
            max_authenticated_sessions,
            outstanding_allocations,
            active_matches,
            heartbeat_ms,
            raw_catalog,
            raw_catalog_fingerprint: decoder
                .take(32)?
                .try_into()
                .expect("exact catalog digest width"),
            nonce: decoder.u128()?,
            digest: bytes[split..].try_into().expect("exact digest width"),
        };
        decoder.finish()?;
        value.validate()?;
        validate_digest(&bytes[..split], value.digest)?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), CodecError> {
        self.common.validate(Self::ROLE)?;
        if self.default_route_id.get() == 0
            || self.max_authenticated_sessions == 0
            || self.outstanding_allocations == 0
            || self.active_matches == 0
            || self.heartbeat_ms == 0
            || self.raw_catalog.is_empty()
            || self.raw_catalog.len() > MAX_LOBBY_CATALOG_BYTES
            || raw_catalog_fingerprint(&self.raw_catalog) != self.raw_catalog_fingerprint
        {
            return Err(CodecError::InvalidValue);
        }
        Ok(())
    }
}

#[must_use]
pub fn raw_catalog_fingerprint(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Bounded application-owned build bytes. Routing transports these opaquely.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MatchBuildSnapshot {
    len: u8,
    bytes: [u8; crate::MAX_MATCH_BUILD_SNAPSHOT_BYTES],
}

impl MatchBuildSnapshot {
    pub fn new(bytes: &[u8]) -> Result<Self, CodecError> {
        let len = u8::try_from(bytes.len()).map_err(|_| CodecError::Oversize)?;
        if bytes.is_empty() || bytes.len() > crate::MAX_MATCH_BUILD_SNAPSHOT_BYTES {
            return Err(CodecError::InvalidValue);
        }
        let mut stored = [0; crate::MAX_MATCH_BUILD_SNAPSHOT_BYTES];
        stored[..bytes.len()].copy_from_slice(bytes);
        Ok(Self { len, bytes: stored })
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

impl fmt::Debug for MatchBuildSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MatchBuildSnapshot")
            .field("bytes", &"[REDACTED]")
            .field("len", &self.len)
            .finish()
    }
}

/// A participant entry in a match manifest.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MatchManifestParticipant {
    pub lobby_session_id: LobbySessionId,
    pub player_id: crate::PlayerId,
    pub netcode_client_id: crate::NetcodeClientId,
    pub peer_id: PeerId,
    pub team: u8,
    pub source_build_preset: Option<u16>,
    pub recipe_fingerprint: u64,
    pub revision: u16,
    pub build_snapshot: MatchBuildSnapshot,
}

impl fmt::Debug for MatchManifestParticipant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MatchManifestParticipant")
            .field("identity", &"[REDACTED]")
            .field("team", &self.team)
            .field("build", &"[REDACTED]")
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}

fn encode_match_participant(encoder: &mut Encoder, participant: &MatchManifestParticipant) {
    encoder.put_u128(participant.lobby_session_id.get());
    encoder.put_u64(participant.player_id.get());
    encoder.put_u64(participant.netcode_client_id.get());
    encoder.put_u128(participant.peer_id.get());
    encoder.put_u8(participant.team);
    match participant.source_build_preset {
        None => encoder.put_u8(0),
        Some(preset) => {
            encoder.put_u8(1);
            encoder.put_u16(preset);
        }
    }
    encoder.put_u64(participant.recipe_fingerprint);
    encoder.put_u16(participant.revision);
    encoder.put_u8(u8::try_from(participant.build_snapshot.as_bytes().len()).expect("bounded"));
    encoder.put_bytes(participant.build_snapshot.as_bytes());
}

fn decode_match_participant(
    decoder: &mut Decoder<'_>,
) -> Result<MatchManifestParticipant, CodecError> {
    let lobby_session_id = LobbySessionId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?;
    let player_id = crate::PlayerId::new(decoder.u64()?).ok_or(CodecError::ZeroId)?;
    let netcode_client_id =
        crate::NetcodeClientId::new(decoder.u64()?).ok_or(CodecError::ZeroId)?;
    let peer_id = PeerId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?;
    let team = decoder.u8()?;
    let source_build_preset = decoder.optional(Decoder::u16)?;
    let recipe_fingerprint = decoder.u64()?;
    let revision = decoder.u16()?;
    let build_length = usize::from(decoder.u8()?);
    Ok(MatchManifestParticipant {
        lobby_session_id,
        player_id,
        netcode_client_id,
        peer_id,
        team,
        source_build_preset,
        recipe_fingerprint,
        revision,
        build_snapshot: MatchBuildSnapshot::new(decoder.take(build_length)?)?,
    })
}

/// A match worker's canonical v1 manifest.
#[derive(Clone, PartialEq, Eq)]
pub struct MatchManifestV1 {
    pub common: ManifestCommon,
    pub request_id: RequestId,
    pub match_id: MatchId,
    pub allocation_id: crate::AllocationId,
    pub mode: GameMode,
    pub map_preset: u16,
    pub map_revision: u16,
    pub rules_profile: u8,
    pub objective_target: u16,
    pub match_duration_ticks: u64,
    pub countdown_ticks: u64,
    pub respawn_ticks: u64,
    pub reserved: u8,
    pub seed: u64,
    pub participants: Vec<MatchManifestParticipant>,
    pub heartbeat_ms: u32,
    pub nonce: u128,
    pub digest: [u8; 32],
}

impl fmt::Debug for MatchManifestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MatchManifestV1")
            .field("common", &self.common)
            .field("request_id", &self.request_id)
            .field("match_id", &self.match_id)
            .field("allocation_id", &self.allocation_id)
            .field("mode", &self.mode)
            .field("map_preset", &self.map_preset)
            .field("map_revision", &self.map_revision)
            .field("rules_profile", &self.rules_profile)
            .field("objective_target", &self.objective_target)
            .field("match_duration_ticks", &self.match_duration_ticks)
            .field("countdown_ticks", &self.countdown_ticks)
            .field("respawn_ticks", &self.respawn_ticks)
            .field("reserved", &self.reserved)
            .field("seed", &self.seed)
            .field("participant_count", &self.participants.len())
            .field("heartbeat_ms", &self.heartbeat_ms)
            .field("nonce", &"[REDACTED]")
            .field("digest", &"[REDACTED]")
            .finish()
    }
}

impl MatchManifestV1 {
    pub const ROLE: WorkerRole = WorkerRole::Match;

    pub fn with_digest(mut self) -> Result<Self, CodecError> {
        self.digest = self.compute_digest()?;
        Ok(self)
    }

    pub fn compute_digest(&self) -> Result<[u8; 32], CodecError> {
        Ok(manifest_digest(&self.encode_prefix()?))
    }

    fn encode_prefix(&self) -> Result<Vec<u8>, CodecError> {
        self.validate()?;
        let mut encoder = Encoder::with_capacity(4_096);
        encode_common(&mut encoder, self.common);
        encoder.put_u64(self.request_id.get());
        encoder.put_u128(self.match_id.get());
        encoder.put_u128(self.allocation_id.get());
        encoder.put_u16(self.mode as u16);
        encoder.put_u16(self.map_preset);
        encoder.put_u16(self.map_revision);
        encoder.put_u8(self.rules_profile);
        encoder.put_u16(self.objective_target);
        encoder.put_u64(self.match_duration_ticks);
        encoder.put_u64(self.countdown_ticks);
        encoder.put_u64(self.respawn_ticks);
        encoder.put_u8(self.reserved);
        encoder.put_u64(self.seed);
        encoder.put_u8(u8::try_from(self.participants.len()).map_err(|_| CodecError::Oversize)?);
        for participant in &self.participants {
            encode_match_participant(&mut encoder, participant);
        }
        encoder.put_u32(self.heartbeat_ms);
        encoder.put_u128(self.nonce);
        Ok(encoder.finish())
    }

    pub fn encode(&self) -> Result<Vec<u8>, CodecError> {
        let prefix = self.encode_prefix()?;
        let digest = if self.digest == [0; 32] {
            manifest_digest(&prefix)
        } else {
            validate_digest(&prefix, self.digest)?;
            self.digest
        };
        let mut bytes = prefix;
        bytes.extend_from_slice(&digest);
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(CodecError::Oversize);
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(CodecError::Oversize);
        }
        if bytes.len() < 32 {
            return Err(CodecError::Truncated);
        }
        let split = bytes.len() - 32;
        let mut decoder = Decoder::new(&bytes[..split]);
        let common = decode_common(&mut decoder, Self::ROLE)?;
        let request_id = RequestId::new(decoder.u64()?).ok_or(CodecError::ZeroId)?;
        let match_id = MatchId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?;
        let allocation_id = crate::AllocationId::new(decoder.u128()?).ok_or(CodecError::ZeroId)?;
        let mode = GameMode::try_from(decoder.u16()?)?;
        let map_preset = decoder.u16()?;
        let map_revision = decoder.u16()?;
        let rules_profile = decoder.u8()?;
        let objective_target = decoder.u16()?;
        let match_duration_ticks = decoder.u64()?;
        let countdown_ticks = decoder.u64()?;
        let respawn_ticks = decoder.u64()?;
        let reserved = decoder.u8()?;
        let seed = decoder.u64()?;
        let count = usize::from(decoder.u8()?);
        if count == 0 || count > MAX_PARTICIPANTS {
            return Err(CodecError::InvalidValue);
        }
        let mut participants = Vec::with_capacity(count);
        for _ in 0..count {
            participants.push(decode_match_participant(&mut decoder)?);
        }
        let value = Self {
            common,
            request_id,
            match_id,
            allocation_id,
            mode,
            map_preset,
            map_revision,
            rules_profile,
            objective_target,
            match_duration_ticks,
            countdown_ticks,
            respawn_ticks,
            reserved,
            seed,
            participants,
            heartbeat_ms: decoder.u32()?,
            nonce: decoder.u128()?,
            digest: bytes[split..].try_into().expect("exact digest width"),
        };
        decoder.finish()?;
        value.validate()?;
        validate_digest(&bytes[..split], value.digest)?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), CodecError> {
        self.common.validate(Self::ROLE)?;
        if self.match_id.get() == 0
            || self.allocation_id.get() == 0
            || self.reserved != 0
            || self.objective_target == 0
            || self.match_duration_ticks == 0
            || self.countdown_ticks == 0
            || self.respawn_ticks == 0
            || self.participants.is_empty()
            || self.participants.len() > MAX_PARTICIPANTS
            || self.heartbeat_ms == 0
        {
            return Err(CodecError::InvalidValue);
        }
        Ok(())
    }
}

/// Compatibility alias used by callers that do not need the v1 suffix.
pub type MatchManifest = MatchManifestV1;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AllocationId, NetcodeClientId, PlayerId};

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

    fn participant(index: u64) -> MatchManifestParticipant {
        MatchManifestParticipant {
            lobby_session_id: id128(u128::from(index) + 10),
            player_id: PlayerId::new(index + 20).unwrap(),
            netcode_client_id: NetcodeClientId::new(index + 25).unwrap(),
            peer_id: id128(u128::from(index) + 30),
            team: u8::try_from(index % 2).unwrap(),
            source_build_preset: Some(u16::try_from(index + 40).unwrap()),
            recipe_fingerprint: index + 50,
            revision: u16::try_from(index + 60).unwrap(),
            build_snapshot: MatchBuildSnapshot::new(&[1, 2, 3]).unwrap(),
        }
    }

    fn match_manifest(participant_count: usize) -> MatchManifestV1 {
        MatchManifestV1 {
            common: ManifestCommon {
                manifest_version: 2,
                role: WorkerRole::Match,
                logical_server_id: id128(1),
                process_id: id128(2),
                worker_id: id128(3),
                generation: id64(4),
                network_protocol: 5,
                protocol_registry_fingerprint: 6,
                content_fingerprint: 7,
                route_version: ROUTE_VERSION_V1,
                packet_version: PACKET_VERSION_V1,
                control_version: CONTROL_VERSION_CURRENT,
                flags: 0,
            },
            request_id: RequestId::new(15).unwrap(),
            match_id: id128(8),
            allocation_id: AllocationId::new(9).unwrap(),
            mode: GameMode::HotZone,
            map_preset: 10,
            map_revision: 11,
            rules_profile: 12,
            objective_target: 10,
            match_duration_ticks: 10_800,
            countdown_ticks: 180,
            respawn_ticks: 180,
            reserved: 0,
            seed: 13,
            participants: (1..=u64::try_from(participant_count).unwrap())
                .map(participant)
                .collect(),
            heartbeat_ms: 1_000,
            nonce: 14,
            digest: [0; 32],
        }
    }

    // Common fields through seed, before the participant count byte.
    const PARTICIPANT_COUNT_OFFSET: usize =
        87 + 8 + 16 + 16 + 2 + 2 + 2 + 1 + 2 + 8 + 8 + 8 + 1 + 8;
    const ALLOCATION_ID_OFFSET: usize = 87 + 8 + 16;

    #[test]
    fn match_manifest_exact_round_trip_and_digest_are_canonical() {
        let manifest = match_manifest(2);
        let encoded = manifest.encode().unwrap();
        assert_eq!(
            MatchManifestV1::decode(&encoded).unwrap(),
            manifest.clone().with_digest().unwrap()
        );
        assert_eq!(encoded.len(), 354);
        assert_eq!(
            &encoded[encoded.len() - 32..],
            &manifest_digest(&encoded[..encoded.len() - 32])
        );
        assert_eq!(
            manifest.compute_digest().unwrap(),
            manifest_digest(&encoded[..encoded.len() - 32])
        );
    }

    #[test]
    fn match_manifest_rejects_every_truncation_and_trailing_byte() {
        let encoded = match_manifest(1).encode().unwrap();
        for length in 0..encoded.len() {
            assert!(
                MatchManifestV1::decode(&encoded[..length]).is_err(),
                "truncation length {length}"
            );
        }
        let mut trailing = encoded;
        trailing.push(0xa5);
        assert_eq!(
            MatchManifestV1::decode(&trailing),
            Err(CodecError::TrailingData)
        );
    }

    #[test]
    fn match_manifest_fixture_encodes_netcode_id_between_player_and_peer() {
        let encoded = match_manifest(1).encode().unwrap();
        let participant_start = PARTICIPANT_COUNT_OFFSET + 1;
        assert_eq!(
            &encoded[participant_start..participant_start + 16],
            &11_u128.to_be_bytes()
        );
        assert_eq!(
            &encoded[participant_start + 16..participant_start + 24],
            &21_u64.to_be_bytes()
        );
        assert_eq!(
            &encoded[participant_start + 24..participant_start + 32],
            &26_u64.to_be_bytes()
        );
        assert_eq!(
            &encoded[participant_start + 32..participant_start + 48],
            &31_u128.to_be_bytes()
        );
    }

    #[test]
    fn match_manifest_rejects_zero_netcode_id_before_digest_validation() {
        let mut encoded = match_manifest(1).encode().unwrap();
        let netcode_id_start = PARTICIPANT_COUNT_OFFSET + 1 + 16 + 8;
        encoded[netcode_id_start..netcode_id_start + 8].fill(0);
        assert_eq!(MatchManifestV1::decode(&encoded), Err(CodecError::ZeroId));
    }

    #[test]
    fn match_manifest_codec_remains_neutral_to_duplicate_netcode_ids() {
        let mut manifest = match_manifest(1);
        manifest.participants.push(participant(1));
        let encoded = manifest.encode().unwrap();
        assert_eq!(
            MatchManifestV1::decode(&encoded).unwrap(),
            manifest.with_digest().unwrap()
        );
    }

    #[test]
    fn match_manifest_enforces_participant_count_and_byte_bounds() {
        let encoded = match_manifest(MAX_PARTICIPANTS).encode().unwrap();
        assert_eq!(
            encoded[PARTICIPANT_COUNT_OFFSET],
            u8::try_from(MAX_PARTICIPANTS).unwrap()
        );
        assert_eq!(
            MatchManifestV1::decode(&encoded)
                .unwrap()
                .participants
                .len(),
            MAX_PARTICIPANTS
        );

        let mut zero_count = encoded.clone();
        zero_count[PARTICIPANT_COUNT_OFFSET] = 0;
        assert_eq!(
            MatchManifestV1::decode(&zero_count),
            Err(CodecError::InvalidValue)
        );

        let mut too_many = encoded;
        too_many[PARTICIPANT_COUNT_OFFSET] = u8::try_from(MAX_PARTICIPANTS + 1).unwrap();
        assert_eq!(
            MatchManifestV1::decode(&too_many),
            Err(CodecError::InvalidValue)
        );
        assert_eq!(
            MatchManifestV1::decode(&vec![0; MAX_MANIFEST_BYTES + 1]),
            Err(CodecError::Oversize)
        );
        assert_eq!(
            match_manifest(MAX_PARTICIPANTS + 1).validate(),
            Err(CodecError::InvalidValue)
        );
    }

    #[test]
    fn match_manifest_checks_allocation_identity_before_participant_bounds() {
        let mut malformed = match_manifest(1).encode().unwrap();
        malformed[ALLOCATION_ID_OFFSET..ALLOCATION_ID_OFFSET + 16].fill(0);
        malformed[PARTICIPANT_COUNT_OFFSET] = u8::MAX;
        assert_eq!(MatchManifestV1::decode(&malformed), Err(CodecError::ZeroId));
    }

    #[test]
    fn match_manifest_participant_debug_redacts_player_identity_and_build() {
        let debug = format!("{:?}", participant(3));
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains("13"));
        assert!(!debug.contains("23"));
        assert!(!debug.contains("33"));
        assert!(!debug.contains("43"));
    }
}
