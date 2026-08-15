use crate::combat::{ResolvedWeapon, WeaponPresetId};
use bevy::prelude::Component;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct BuildPresetId(pub u16);

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

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedBuild {
    pub source_build_preset_id: Option<BuildPresetId>,
    pub recipe_fingerprint: BuildRecipeFingerprint,
    pub revision: BuildRevision,
}

/// Presence means the waiting participant must confirm the initial or retained loadout before ready.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SelectingBuild;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ResolvedFighterStats {
    pub maximum_health: u16,
    pub movement_speed: f32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UltimateKind {
    Dash,
    Sentry,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedUltimate {
    pub id: UltimateDefinitionId,
    pub kind: UltimateKind,
    pub point_cost: u8,
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
