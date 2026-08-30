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
    pub heist: ModeAllocationPolicy,
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
            ModeAllocationPolicy::new(7, 5, rules_profile),
            ModeAllocationPolicy::new(8, 2, rules_profile),
            ModeAllocationPolicy::new(9, 2, rules_profile),
            1_000,
            SeedPolicy::OsRandom,
        )
    }

    #[must_use]
    pub const fn new(
        wipeout: ModeAllocationPolicy,
        hot_zone: ModeAllocationPolicy,
        heist: ModeAllocationPolicy,
        heartbeat_ms: u32,
        seed_policy: SeedPolicy,
    ) -> Self {
        Self {
            wipeout,
            hot_zone,
            heist,
            heartbeat_ms,
            seed_policy,
        }
    }

    pub fn validate(self) -> Result<(), CodecError> {
        self.wipeout.validate()?;
        self.hot_zone.validate()?;
        self.heist.validate()?;
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
            GameMode::Heist => self.heist,
        }
    }
}

/// Validate one exact 1v1, 2v2, or 3v3 product request without consulting runtime state.
pub fn validate_product_request(request: &AllocateRequestBody) -> Result<(), CodecError> {
    request.validate_product()?;
    let mut sessions = HashSet::with_capacity(request.participants.len());
    let mut players = HashSet::with_capacity(request.participants.len() + request.bots.len());
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
        if participant.team >= request.team_count {
            return Err(CodecError::InvalidValue);
        }
    }
    for bot in &request.bots {
        if !players.insert(bot.player_id) || bot.team >= request.team_count {
            return Err(CodecError::InvalidValue);
        }
    }
    for team in 0..request.team_count {
        let humans = request
            .participants
            .iter()
            .filter(|participant| participant.team == team)
            .count();
        let bots = request.bots.iter().filter(|bot| bot.team == team).count();
        if humans.saturating_add(bots) != usize::from(request.players_per_team) {
            return Err(CodecError::InvalidValue);
        }
    }
    Ok(())
}

/// Preserve the M01 direct-human baseline contract.
pub fn validate_m01_request(request: &AllocateRequestBody) -> Result<(), CodecError> {
    validate_product_request(request)?;
    if !request.bots.is_empty() {
        return Err(CodecError::InvalidValue);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY: AllocationPolicy = AllocationPolicy::new(
        ModeAllocationPolicy::new(11, 12, 1),
        ModeAllocationPolicy::new(21, 22, 2),
        ModeAllocationPolicy::new(31, 32, 3),
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
            POLICY.heist,
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
            display_name: crate::MatchDisplayName::new("Player 2").unwrap(),
            recipe_fingerprint: 4,
            build_revision: 1,
            build_snapshot: crate::MatchBuildSnapshot::new(&[1]).unwrap(),
        };
        let request = AllocateRequestBody {
            request_id: crate::RequestId::new(5).unwrap(),
            lobby_session_id: participant.lobby_session_id,
            mode: GameMode::Wipeout,
            map_preset: 1,
            map_revision: 1,
            rules_profile: 1,
            objective_target: 10,
            match_duration_ticks: 10_800,
            countdown_ticks: 180,
            respawn_ticks: 180,
            spawn_protection_ticks: 90,
            completed_input_lock_ticks: 60,
            wipeout_recent_hostile_damage_credit_ticks: 300,
            heist_critical_health_percent: 25,
            team_count: 2,
            players_per_team: 2,
            participants: vec![participant, participant],
            bots: Vec::new(),
        };
        assert_eq!(
            validate_m01_request(&request),
            Err(CodecError::InvalidValue)
        );
    }

    #[test]
    fn request_validation_accepts_exact_one_v_one() {
        let participant = |identity: u64, team| crate::AllocateParticipant {
            lobby_session_id: crate::LobbySessionId::new(u128::from(identity)).unwrap(),
            player_id: crate::PlayerId::new(identity).unwrap(),
            netcode_client_id: crate::NetcodeClientId::new(identity).unwrap(),
            team,
            display_name: crate::MatchDisplayName::new("Player").unwrap(),
            recipe_fingerprint: identity,
            build_revision: 1,
            build_snapshot: crate::MatchBuildSnapshot::new(&[1]).unwrap(),
        };
        let request = AllocateRequestBody {
            request_id: crate::RequestId::new(1).unwrap(),
            lobby_session_id: crate::LobbySessionId::new(1).unwrap(),
            mode: GameMode::Wipeout,
            map_preset: 1,
            map_revision: 1,
            rules_profile: 1,
            objective_target: 1,
            match_duration_ticks: 10_800,
            countdown_ticks: 180,
            respawn_ticks: 180,
            spawn_protection_ticks: 90,
            completed_input_lock_ticks: 60,
            wipeout_recent_hostile_damage_credit_ticks: 300,
            heist_critical_health_percent: 25,
            team_count: 2,
            players_per_team: 1,
            participants: vec![participant(1, 0), participant(2, 1)],
            bots: Vec::new(),
        };

        assert_eq!(validate_m01_request(&request), Ok(()));
    }

    #[test]
    fn product_validation_accepts_one_human_and_named_bot_roster() {
        let human = crate::AllocateParticipant {
            lobby_session_id: crate::LobbySessionId::new(1).unwrap(),
            player_id: crate::PlayerId::new(1).unwrap(),
            netcode_client_id: crate::NetcodeClientId::new(1).unwrap(),
            team: 0,
            display_name: crate::MatchDisplayName::new("Player").unwrap(),
            recipe_fingerprint: 1,
            build_revision: 1,
            build_snapshot: crate::MatchBuildSnapshot::new(&[1]).unwrap(),
        };
        let bot = |identity, team, name| crate::AllocateBot {
            player_id: crate::PlayerId::new(identity).unwrap(),
            team,
            display_name: crate::MatchDisplayName::new(name).unwrap(),
            recipe_fingerprint: identity,
            build_revision: 1,
            build_snapshot: crate::MatchBuildSnapshot::new(&[1]).unwrap(),
        };
        let request = AllocateRequestBody {
            request_id: crate::RequestId::new(1).unwrap(),
            lobby_session_id: human.lobby_session_id,
            mode: GameMode::HotZone,
            map_preset: 1,
            map_revision: 1,
            rules_profile: 1,
            objective_target: 1_800,
            match_duration_ticks: 10_800,
            countdown_ticks: 180,
            respawn_ticks: 180,
            spawn_protection_ticks: 90,
            completed_input_lock_ticks: 60,
            wipeout_recent_hostile_damage_credit_ticks: 300,
            heist_critical_health_percent: 25,
            team_count: 2,
            players_per_team: 2,
            participants: vec![human],
            bots: vec![bot(2, 0, "Bot 1"), bot(3, 1, "Bot 2"), bot(4, 1, "Bot 3")],
        };

        assert_eq!(validate_product_request(&request), Ok(()));
    }
}
