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

/// One bounded authored build choice. This shape is shared by direct-match selection and
/// lobby queue admission; resolving it remains server-owned at each authority boundary.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildSelection {
    Preset(BuildPresetId),
    Custom(BrawlerBuildRecipe),
}

/// Complete build input submitted to lobby admission.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildCandidate {
    pub build_revision: BuildRevision,
    pub selection: BuildSelection,
}

/// Public, reproducible result of authoritative build resolution.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcceptedBuildSummary {
    pub canonical_recipe: BrawlerBuildRecipe,
    pub identity: SelectedBuild,
    pub total_points: u8,
}

/// Versioned bounded application snapshot transported opaquely through routing manifests.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchBuildSnapshotV1 {
    pub schema_version: u8,
    pub candidate: BuildCandidate,
    pub accepted: AcceptedBuildSummary,
}

impl MatchBuildSnapshotV1 {
    pub const SCHEMA_VERSION: u8 = 1;

    pub fn encode(self) -> Result<brawler_routing::MatchBuildSnapshot, String> {
        let bytes = postcard::to_allocvec(&self)
            .map_err(|error| format!("match build snapshot encode failed: {error}"))?;
        brawler_routing::MatchBuildSnapshot::new(&bytes)
            .map_err(|error| format!("match build snapshot exceeds bound: {error:?}"))
    }

    pub fn decode(snapshot: &brawler_routing::MatchBuildSnapshot) -> Result<Self, String> {
        let value: Self = postcard::from_bytes(snapshot.as_bytes())
            .map_err(|error| format!("match build snapshot decode failed: {error}"))?;
        if value.schema_version != Self::SCHEMA_VERSION {
            return Err("unsupported match build snapshot version".to_string());
        }
        Ok(value)
    }
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
