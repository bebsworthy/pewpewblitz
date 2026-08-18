//! Bevy-free policy and pure validation for supervisor-side match allocation.

use std::collections::HashSet;

use crate::{AllocateRequestBody, CodecError, GameMode};

/// Per-mode gameplay values supplied by the application/operator. Routing does not guess these
/// values because map and rules ownership belongs to the authoritative match worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeAllocationPolicy {
    pub map_preset: u16,
    pub map_revision: u16,
    pub rules_profile: u8,
}

impl ModeAllocationPolicy {
    #[must_use]
    pub const fn new(map_preset: u16, map_revision: u16, rules_profile: u8) -> Self {
        Self {
            map_preset,
            map_revision,
            rules_profile,
        }
    }

    pub fn validate(self) -> Result<(), CodecError> {
        if self.map_preset == 0 || self.map_revision == 0 || self.rules_profile == 0 {
            return Err(CodecError::InvalidValue);
        }
        Ok(())
    }
}

/// Match seed source. M01 intentionally exposes only OS CSPRNG generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeedPolicy {
    OsRandom,
}

/// Explicit allocation policy. Callers choose validated map/rules values; the approved Brawler
/// M01 constructor below is the only convenience default and remains Bevy-free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllocationPolicy {
    pub wipeout: ModeAllocationPolicy,
    pub hot_zone: ModeAllocationPolicy,
    pub heartbeat_ms: u32,
    pub seed_policy: SeedPolicy,
}

impl AllocationPolicy {
    /// Approved Brawler M01 defaults. Applications may still supply an explicit policy when
    /// validating a different content/rules catalog.
    #[must_use]
    pub const fn brawler_m01() -> Self {
        Self::brawler_m01_with_rules_profile(1)
    }

    /// Approved Brawler M01 map values with an explicitly selected authoritative rules profile.
    /// Profile selection stays in the supervisor/operator boundary; routing never infers it from
    /// ambient environment state.
    #[must_use]
    pub const fn brawler_m01_with_rules_profile(rules_profile: u8) -> Self {
        Self::new(
            ModeAllocationPolicy::new(1, 1, rules_profile),
            ModeAllocationPolicy::new(2, 1, rules_profile),
            1_000,
            SeedPolicy::OsRandom,
        )
    }

    #[must_use]
    pub const fn new(
        wipeout: ModeAllocationPolicy,
        hot_zone: ModeAllocationPolicy,
        heartbeat_ms: u32,
        seed_policy: SeedPolicy,
    ) -> Self {
        Self {
            wipeout,
            hot_zone,
            heartbeat_ms,
            seed_policy,
        }
    }

    pub fn validate(self) -> Result<(), CodecError> {
        self.wipeout.validate()?;
        self.hot_zone.validate()?;
        if self.heartbeat_ms == 0 {
            return Err(CodecError::InvalidValue);
        }
        Ok(())
    }

    #[must_use]
    pub const fn for_mode(self, mode: GameMode) -> ModeAllocationPolicy {
        match mode {
            GameMode::Wipeout => self.wipeout,
            GameMode::HotZone => self.hot_zone,
        }
    }
}

/// Validate the M01 exact-two request without consulting runtime state.
pub fn validate_m01_request(request: &AllocateRequestBody) -> Result<(), CodecError> {
    request.validate_m01()?;
    let mut sessions = HashSet::with_capacity(request.participants.len());
    let mut players = HashSet::with_capacity(request.participants.len());
    let mut clients = HashSet::with_capacity(request.participants.len());
    if !request
        .participants
        .iter()
        .any(|participant| participant.lobby_session_id == request.lobby_session_id)
    {
        return Err(CodecError::InvalidValue);
    }
    for participant in &request.participants {
        if !sessions.insert(participant.lobby_session_id)
            || !players.insert(participant.player_id)
            || !clients.insert(participant.netcode_client_id)
        {
            return Err(CodecError::InvalidValue);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY: AllocationPolicy = AllocationPolicy::new(
        ModeAllocationPolicy::new(11, 12, 1),
        ModeAllocationPolicy::new(21, 22, 2),
        1_000,
        SeedPolicy::OsRandom,
    );

    #[test]
    fn policy_selects_explicit_mode_values() {
        assert_eq!(POLICY.for_mode(GameMode::Wipeout).map_preset, 11);
        assert_eq!(POLICY.for_mode(GameMode::HotZone).rules_profile, 2);
        assert!(POLICY.validate().is_ok());
    }

    #[test]
    fn policy_rejects_zero_gameplay_values() {
        let invalid = AllocationPolicy::new(
            ModeAllocationPolicy::new(0, 12, 1),
            POLICY.hot_zone,
            POLICY.heartbeat_ms,
            POLICY.seed_policy,
        );
        assert_eq!(invalid.validate(), Err(CodecError::InvalidValue));
    }

    #[test]
    fn request_validation_rejects_duplicate_authenticated_identities() {
        let participant = crate::AllocateParticipant {
            lobby_session_id: crate::LobbySessionId::new(1).unwrap(),
            player_id: crate::PlayerId::new(2).unwrap(),
            netcode_client_id: crate::NetcodeClientId::new(3).unwrap(),
            team: 0,
            source_build_preset: Some(1),
            recipe_fingerprint: 4,
            build_revision: 1,
        };
        let request = AllocateRequestBody {
            request_id: crate::RequestId::new(5).unwrap(),
            lobby_session_id: participant.lobby_session_id,
            mode: GameMode::Wipeout,
            participants: vec![participant, participant],
        };
        assert_eq!(
            validate_m01_request(&request),
            Err(CodecError::InvalidValue)
        );
    }
}
