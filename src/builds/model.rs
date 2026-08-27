use crate::combat::{ResolvedWeapon, WeaponPresetId};
use bevy::prelude::Component;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct BuildRevision(pub u16);

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Ord, PartialOrd,
)]
pub struct BuildRecipeFingerprint(pub u64);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct UltimateDefinitionId(pub u16);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct PassiveDefinitionId(pub u16);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct DeployableId(pub u64);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PulsePower {
    Light,
    Balanced,
    Heavy,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PulseReach {
    Compact,
    Standard,
    Long,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PulseMagazine {
    Quick,
    Standard,
    Expanded,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WeaponChoice {
    Preset(WeaponPresetId),
    CustomPulse {
        power: PulsePower,
        reach: PulseReach,
        magazine: PulseMagazine,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BrawlerBuildRecipe {
    pub weapon: WeaponChoice,
    pub ultimate: UltimateDefinitionId,
    pub passives: [PassiveDefinitionId; 2],
}

/// Public, reproducible result of authoritative build resolution.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcceptedBuildSummary {
    pub canonical_recipe: BrawlerBuildRecipe,
    pub identity: SelectedBuild,
    pub total_points: u8,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedBuild {
    pub recipe_fingerprint: BuildRecipeFingerprint,
    pub revision: BuildRevision,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ResolvedFighterStats {
    pub maximum_health: u16,
    pub movement_speed: f32,
    /// Integer health points restored per second after the attack-idle delay.
    pub health_recovery_rate: u16,
    /// Consecutive authoritative ticks without an accepted player attack before recovery starts.
    pub idle_attack_delay_ticks: u64,
    /// Observer-owned distance at which an enemy's terrain concealment is revealed.
    pub reveal_proximity_radius: f32,
}

/// Canonical bonus/malus input applied while resolving reveal proximity.
///
/// Flat values use thousandths of one world unit and percentage values use basis points so
/// content and persistence never depend on platform-specific floating-point inputs.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RevealProximityModifier {
    pub flat_milliunits: i32,
    pub percent_basis_points: i16,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UltimateKind {
    Dash,
    Sentry,
    SelfCloak,
    RevealScan,
    ConcealmentField,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UltimateActivationStyle {
    Immediate,
    Targeted,
}

impl UltimateKind {
    #[must_use]
    pub const fn activation_style(self) -> UltimateActivationStyle {
        match self {
            Self::Dash | Self::Sentry | Self::SelfCloak => UltimateActivationStyle::Immediate,
            Self::RevealScan | Self::ConcealmentField => UltimateActivationStyle::Targeted,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UltimateParameters {
    Dash,
    Sentry,
    SelfCloak {
        duration_ticks: u64,
    },
    RevealScan {
        maximum_range_milliunits: u32,
        radius_milliunits: u32,
        reveal_ticks: u64,
    },
    ConcealmentField {
        maximum_range_milliunits: u32,
        radius_milliunits: u32,
        duration_ticks: u64,
    },
}

/// Convert bounded authored thousandths to world units without a lossy wide-integer cast.
#[must_use]
pub fn world_units_from_milliunits(value: u32) -> Option<f32> {
    let whole = u16::try_from(value / 1_000).ok()?;
    let remainder = u16::try_from(value % 1_000).expect("milliunit remainder fits u16");
    Some(f32::from(whole) + f32::from(remainder) / 1_000.0)
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedUltimate {
    pub id: UltimateDefinitionId,
    pub kind: UltimateKind,
    pub point_cost: u8,
    pub parameters: UltimateParameters,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassiveKind {
    LightweightFrame,
    ReinforcedFrame,
    AdrenalResponse,
    CloseQuarters,
    QuickCycle,
    Tenacity,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedPassive {
    pub id: PassiveDefinitionId,
    pub kind: PassiveKind,
    pub point_cost: u8,
}

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ResolvedMatchLoadout {
    pub identity: SelectedBuild,
    pub total_points: u8,
    pub fighter_stats: ResolvedFighterStats,
    pub primary_weapon: ResolvedWeapon,
    pub ultimate: ResolvedUltimate,
    pub passives: [ResolvedPassive; 2],
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AbilityPhase {
    #[default]
    Charging,
    Ready,
    Dashing {
        ends_at_tick: u64,
    },
    Deployed {
        deployable_id: DeployableId,
        expires_at_tick: u64,
    },
    Cloaked {
        generation: u64,
        activated_at_tick: u64,
        expires_at_tick: u64,
    },
    FieldActive {
        field_id: crate::concealment::ConcealmentFieldId,
        expires_at_tick: u64,
    },
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AbilityState {
    pub charge: u16,
    pub phase: AbilityPhase,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PassiveRuntimeState {
    pub adrenaline_until_tick: Option<u64>,
    pub adrenaline_rearm_at_tick: Option<u64>,
    pub quick_cycle_primed: bool,
}
