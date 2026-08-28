//! Ordered combat presentation cues and their process-evidence codec.

use super::{
    AttackId, CombatEventId, DistanceBand, ShotId, WeaponDefinitionId, WeaponPresentationProfileId,
    WorldPoint,
};
use crate::protocol::{NetworkEntityId, PlayerId};
use bevy::prelude::Message;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageSource {
    PlayerWeapon {
        player_id: PlayerId,
        fighter_id: NetworkEntityId,
        weapon_definition_id: WeaponDefinitionId,
        shot_id: ShotId,
    },
    Ultimate {
        player_id: PlayerId,
        fighter_id: NetworkEntityId,
        ultimate_id: crate::builds::UltimateDefinitionId,
        attack_id: AttackId,
    },
    Deployable {
        player_id: PlayerId,
        fighter_id: NetworkEntityId,
        ultimate_id: crate::builds::UltimateDefinitionId,
        deployable_id: crate::builds::DeployableId,
        attack_id: AttackId,
    },
    Environment {
        map_instance_id: u64,
        generation: u64,
        placement_id: u32,
        initiating_player: Option<PlayerId>,
        initiating_fighter: Option<NetworkEntityId>,
    },
}

/// Cue variants used by deterministic/process evidence without serializing presentation payloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatCueKind {
    AttackAccepted,
    DeliveryImpact,
    LobLanded,
    MeleeContact,
    DamageApplied,
    EffectApplied,
    FighterDefeated,
    FighterReset,
    SentryFired,
    DeployableRemoved,
    Muzzle,
    Impact,
    Damage,
    Defeat,
    Reset,
    SelfCloakActivated,
    SelfCloakEnded,
    RevealScanActivated,
    DemolitionStrikeActivated,
    ForcedRevealApplied,
}

impl CombatCueKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AttackAccepted => "attack_accepted",
            Self::DeliveryImpact => "delivery_impact",
            Self::LobLanded => "lob_landed",
            Self::MeleeContact => "melee_contact",
            Self::DamageApplied => "damage_applied",
            Self::EffectApplied => "effect_applied",
            Self::FighterDefeated => "fighter_defeated",
            Self::FighterReset => "fighter_reset",
            Self::SentryFired => "sentry_fired",
            Self::DeployableRemoved => "deployable_removed",
            Self::Muzzle => "muzzle",
            Self::Impact => "impact",
            Self::Damage => "damage",
            Self::Defeat => "defeat",
            Self::Reset => "reset",
            Self::SelfCloakActivated => "self_cloak_activated",
            Self::SelfCloakEnded => "self_cloak_ended",
            Self::RevealScanActivated => "reveal_scan_activated",
            Self::DemolitionStrikeActivated => "demolition_strike_activated",
            Self::ForcedRevealApplied => "forced_reveal_applied",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "attack_accepted" => Some(Self::AttackAccepted),
            "delivery_impact" => Some(Self::DeliveryImpact),
            "lob_landed" => Some(Self::LobLanded),
            "melee_contact" => Some(Self::MeleeContact),
            "damage_applied" => Some(Self::DamageApplied),
            "effect_applied" => Some(Self::EffectApplied),
            "fighter_defeated" => Some(Self::FighterDefeated),
            "fighter_reset" => Some(Self::FighterReset),
            "sentry_fired" => Some(Self::SentryFired),
            "deployable_removed" => Some(Self::DeployableRemoved),
            "muzzle" => Some(Self::Muzzle),
            "impact" => Some(Self::Impact),
            "damage" => Some(Self::Damage),
            "defeat" => Some(Self::Defeat),
            "reset" => Some(Self::Reset),
            "self_cloak_activated" => Some(Self::SelfCloakActivated),
            "self_cloak_ended" => Some(Self::SelfCloakEnded),
            "reveal_scan_activated" => Some(Self::RevealScanActivated),
            "demolition_strike_activated" => Some(Self::DemolitionStrikeActivated),
            "forced_reveal_applied" => Some(Self::ForcedRevealApplied),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SelfCloakEndReason {
    Expired,
    Attack,
    Damage,
    Defeated,
    Lifecycle,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum CombatEffectCue {
    Knockback {
        velocity: WorldPoint,
        expires_at_tick: u64,
    },
    Slow {
        movement_multiplier_milli: u16,
        expires_at_tick: u64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombatCueKey {
    pub kind: CombatCueKind,
    pub event_id: CombatEventId,
}

/// Ordered presentation facts. Durable values remain replicated components.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum CombatCue {
    AttackAccepted {
        event_id: CombatEventId,
        tick: u64,
        attack_id: AttackId,
        source: NetworkEntityId,
        position: WorldPoint,
        weapon_definition_id: WeaponDefinitionId,
        presentation_profile_id: WeaponPresentationProfileId,
    },
    DeliveryImpact {
        event_id: CombatEventId,
        tick: u64,
        attack_id: AttackId,
        delivery_index: u8,
        source: NetworkEntityId,
        weapon_definition_id: WeaponDefinitionId,
        presentation_profile_id: WeaponPresentationProfileId,
        target: Option<NetworkEntityId>,
        position: WorldPoint,
        normal: WorldPoint,
        distance_band: DistanceBand,
    },
    LobLanded {
        event_id: CombatEventId,
        tick: u64,
        attack_id: AttackId,
        delivery_index: u8,
        source: NetworkEntityId,
        weapon_definition_id: WeaponDefinitionId,
        presentation_profile_id: WeaponPresentationProfileId,
        position: WorldPoint,
    },
    MeleeContact {
        event_id: CombatEventId,
        tick: u64,
        attack_id: AttackId,
        delivery_index: u8,
        source: NetworkEntityId,
        weapon_definition_id: WeaponDefinitionId,
        presentation_profile_id: WeaponPresentationProfileId,
        target: NetworkEntityId,
        position: WorldPoint,
    },
    DamageApplied {
        event_id: CombatEventId,
        tick: u64,
        attack_id: AttackId,
        source: DamageSource,
        target: NetworkEntityId,
        position: WorldPoint,
        amount: u16,
        health_after: u16,
        distance_band: DistanceBand,
        presentation_profile_id: WeaponPresentationProfileId,
    },
    EffectApplied {
        event_id: CombatEventId,
        tick: u64,
        attack_id: AttackId,
        source: DamageSource,
        target: NetworkEntityId,
        position: WorldPoint,
        effect: CombatEffectCue,
        presentation_profile_id: WeaponPresentationProfileId,
    },
    FighterDefeated {
        event_id: CombatEventId,
        tick: u64,
        attack_id: AttackId,
        source: Option<DamageSource>,
        target: NetworkEntityId,
        position: WorldPoint,
        presentation_profile_id: Option<WeaponPresentationProfileId>,
    },
    FighterReset {
        event_id: CombatEventId,
        tick: u64,
        target: NetworkEntityId,
        position: WorldPoint,
    },
    SentryFired {
        event_id: CombatEventId,
        tick: u64,
        owner: NetworkEntityId,
        deployable_id: crate::builds::DeployableId,
        target: Option<NetworkEntityId>,
        position: WorldPoint,
        presentation_profile_id: WeaponPresentationProfileId,
    },
    DeployableRemoved {
        event_id: CombatEventId,
        tick: u64,
        owner: NetworkEntityId,
        deployable_id: crate::builds::DeployableId,
        position: WorldPoint,
        reason: crate::abilities::SentryCleanupReason,
    },
    Muzzle {
        event_id: CombatEventId,
        tick: u64,
        source: NetworkEntityId,
        shot_id: ShotId,
        weapon_definition_id: WeaponDefinitionId,
        position: WorldPoint,
    },
    Impact {
        event_id: CombatEventId,
        tick: u64,
        source: NetworkEntityId,
        shot_id: ShotId,
        weapon_definition_id: WeaponDefinitionId,
        target: Option<NetworkEntityId>,
        position: WorldPoint,
        normal: WorldPoint,
        distance_band: DistanceBand,
    },
    Damage {
        event_id: CombatEventId,
        tick: u64,
        source: DamageSource,
        target: NetworkEntityId,
        amount: u16,
        health_after: u16,
        distance_band: DistanceBand,
    },
    Defeat {
        event_id: CombatEventId,
        tick: u64,
        source: Option<DamageSource>,
        target: NetworkEntityId,
    },
    Reset {
        event_id: CombatEventId,
        tick: u64,
        target: NetworkEntityId,
        position: WorldPoint,
    },
    SelfCloakActivated {
        event_id: CombatEventId,
        tick: u64,
        source: NetworkEntityId,
        generation: u64,
        expires_at_tick: u64,
    },
    SelfCloakEnded {
        event_id: CombatEventId,
        tick: u64,
        source: NetworkEntityId,
        generation: u64,
        reason: SelfCloakEndReason,
    },
    RevealScanActivated {
        event_id: CombatEventId,
        tick: u64,
        revealing_team: crate::combat::TeamId,
        center: WorldPoint,
        radius_milliunits: u32,
        expires_at_tick: u64,
    },
    DemolitionStrikeActivated {
        event_id: CombatEventId,
        tick: u64,
        source: NetworkEntityId,
        center: WorldPoint,
        radius_milliunits: u32,
    },
    ForcedRevealApplied {
        event_id: CombatEventId,
        tick: u64,
        target: NetworkEntityId,
        revealing_team: crate::combat::TeamId,
        source_generation: u64,
        expires_at_tick: u64,
    },
}

#[must_use]
pub fn combat_cue_key(cue: &CombatCue) -> CombatCueKey {
    let (kind, event_id) = match cue {
        CombatCue::AttackAccepted { event_id, .. } => (CombatCueKind::AttackAccepted, *event_id),
        CombatCue::DeliveryImpact { event_id, .. } => (CombatCueKind::DeliveryImpact, *event_id),
        CombatCue::LobLanded { event_id, .. } => (CombatCueKind::LobLanded, *event_id),
        CombatCue::MeleeContact { event_id, .. } => (CombatCueKind::MeleeContact, *event_id),
        CombatCue::DamageApplied { event_id, .. } => (CombatCueKind::DamageApplied, *event_id),
        CombatCue::EffectApplied { event_id, .. } => (CombatCueKind::EffectApplied, *event_id),
        CombatCue::FighterDefeated { event_id, .. } => (CombatCueKind::FighterDefeated, *event_id),
        CombatCue::FighterReset { event_id, .. } => (CombatCueKind::FighterReset, *event_id),
        CombatCue::SentryFired { event_id, .. } => (CombatCueKind::SentryFired, *event_id),
        CombatCue::DeployableRemoved { event_id, .. } => {
            (CombatCueKind::DeployableRemoved, *event_id)
        }
        CombatCue::Muzzle { event_id, .. } => (CombatCueKind::Muzzle, *event_id),
        CombatCue::Impact { event_id, .. } => (CombatCueKind::Impact, *event_id),
        CombatCue::Damage { event_id, .. } => (CombatCueKind::Damage, *event_id),
        CombatCue::Defeat { event_id, .. } => (CombatCueKind::Defeat, *event_id),
        CombatCue::Reset { event_id, .. } => (CombatCueKind::Reset, *event_id),
        CombatCue::SelfCloakActivated { event_id, .. } => {
            (CombatCueKind::SelfCloakActivated, *event_id)
        }
        CombatCue::SelfCloakEnded { event_id, .. } => (CombatCueKind::SelfCloakEnded, *event_id),
        CombatCue::RevealScanActivated { event_id, .. } => {
            (CombatCueKind::RevealScanActivated, *event_id)
        }
        CombatCue::DemolitionStrikeActivated { event_id, .. } => {
            (CombatCueKind::DemolitionStrikeActivated, *event_id)
        }
        CombatCue::ForcedRevealApplied { event_id, .. } => {
            (CombatCueKind::ForcedRevealApplied, *event_id)
        }
    };
    CombatCueKey { kind, event_id }
}

/// Encode a cue payload for the line-oriented process evidence file.
#[must_use]
pub fn encode_combat_cue(cue: &CombatCue) -> String {
    let bytes = postcard::to_allocvec(cue).expect("combat cue serialization should be infallible");
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use core::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

/// Decode a cue payload from the process evidence file.
#[must_use]
pub fn decode_combat_cue(encoded: &str) -> Option<CombatCue> {
    if !encoded.len().is_multiple_of(2) {
        return None;
    }
    let bytes = encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_value(pair[0])?;
            let low = hex_value(pair[1])?;
            Some((high << 4) | low)
        })
        .collect::<Option<Vec<_>>>()?;
    postcard::from_bytes(&bytes).ok()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}
