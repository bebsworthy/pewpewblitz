use crate::combat::{DamageOverTimeKind, ResolvedWeapon, WeaponPresetId};
use bevy::prelude::{Bundle, Component};
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

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ResolvedFighterStats {
    pub maximum_health: u16,
    pub movement_speed: f32,
    /// Integer health points restored per second after the attack-idle delay.
    pub health_recovery_rate: u16,
    /// Consecutive authoritative ticks without an accepted player attack before recovery starts.
    pub idle_attack_delay_ticks: u64,
    /// Observer-owned distance at which an enemy's terrain concealment is revealed.
    pub reveal_proximity_radius: f32,
    /// Target-owned Cold buildup required to trigger Freeze.
    pub cold_capacity: u16,
    pub cold_resistance_basis_points: u16,
    pub poison_resistance_basis_points: u16,
    pub fire_resistance_basis_points: u16,
}

/// Immutable authored collision geometry shared by every current fighter profile.
///
/// Brawler currently has one matched one-cell fighter footprint. Keeping that footprint in the
/// build catalog lets weapon validation and runtime collision consume one data-owned value without
/// expanding the replicated loadout wire shape.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct FighterBody {
    pub radius: f32,
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
    DemolitionStrike,
    CryogenicField,
    FireField,
    PoisonField,
    RestorationField,
    BigBlob,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct UltimateChargePolicy {
    pub maximum: u16,
    pub dealt_damage_multiplier: u16,
    pub received_damage_multiplier: u16,
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
            Self::RevealScan
            | Self::ConcealmentField
            | Self::DemolitionStrike
            | Self::CryogenicField
            | Self::FireField
            | Self::PoisonField
            | Self::RestorationField
            | Self::BigBlob => UltimateActivationStyle::Targeted,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UltimateParameters {
    Dash {
        maximum_distance_milliunits: u32,
        duration_ticks: u64,
        damage: u16,
        knockback_speed_milliunits: u32,
        knockback_duration_ticks: u64,
        maximum_targets: u8,
    },
    Sentry {
        placement_offsets_milliunits: [u32; 6],
        body_radius_milliunits: u32,
        acquisition_range_milliunits: u32,
        acquisition_interval_ticks: u64,
        fire_interval_ticks: u64,
        lifetime_ticks: u64,
        maximum_health: u16,
        projectile_speed_milliunits: u32,
        projectile_radius_milliunits: u32,
        projectile_range_milliunits: u32,
        projectile_lifetime_ticks: u64,
        projectile_damage: u16,
    },
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
    DemolitionStrike {
        maximum_range_milliunits: u32,
        radius_milliunits: u32,
    },
    ElementalField {
        maximum_range_milliunits: u32,
        radius_milliunits: u32,
        duration_ticks: u64,
        pulse_interval_ticks: u64,
        effect: ElementalFieldEffect,
    },
    BigBlob {
        maximum_range_milliunits: u32,
        flight_ticks: u64,
        visual_arc_height_milliunits: u32,
        landing_clearance_milliunits: u32,
        child_speed_milliunits: u32,
        child_radius_milliunits: u32,
        child_range_milliunits: u32,
        child_lifetime_ticks: u64,
        child_fuse_ticks: u64,
        child_explosion_radius_milliunits: u32,
        child_damage: u16,
        max_active_per_owner: u8,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementalFieldEffect {
    Cold {
        amount: u16,
    },
    DamageOverTime {
        kind: DamageOverTimeKind,
        damage_per_tick: u16,
        tick_interval: u64,
        duration_ticks: u64,
    },
    Heal {
        amount: u16,
    },
}

/// Convert bounded authored thousandths to world units without a lossy wide-integer cast.
#[must_use]
pub fn world_units_from_milliunits(value: u32) -> Option<f32> {
    let whole = u16::try_from(value / 1_000).ok()?;
    let remainder = u16::try_from(value % 1_000).expect("milliunit remainder fits u16");
    Some(f32::from(whole) + f32::from(remainder) / 1_000.0)
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedUltimate {
    pub id: UltimateDefinitionId,
    pub kind: UltimateKind,
    pub point_cost: u8,
    pub parameters: UltimateParameters,
    pub charge_policy: UltimateChargePolicy,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassiveKind {
    LightweightFrame,
    ReinforcedFrame,
    AdrenalResponse,
    CloseQuarters,
    QuickCycle,
    Tenacity,
    CryogenicInsulation,
    FilteredCirculation,
    HeatShielding,
}

/// Inclusive authored bounds for one numeric passive-parameter family.
///
/// These remain crate-private because they coordinate build validation with development tooling;
/// they are not a runtime capability or a public content API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PassiveParameterBounds<T> {
    pub(crate) minimum: T,
    pub(crate) maximum: T,
}

impl<T> PassiveParameterBounds<T> {
    pub(crate) const fn new(minimum: T, maximum: T) -> Self {
        Self { minimum, maximum }
    }
}

impl<T: PartialOrd> PassiveParameterBounds<T> {
    pub(crate) fn contains(self, value: &T) -> bool {
        value >= &self.minimum && value <= &self.maximum
    }
}

pub(crate) const PASSIVE_ADRENAL_DURATION_TICKS_BOUNDS: PassiveParameterBounds<u64> =
    PassiveParameterBounds::new(1, 3_600);
pub(crate) const PASSIVE_ADRENAL_REARM_TICKS_BOUNDS: PassiveParameterBounds<u64> =
    PassiveParameterBounds::new(1, 36_000);
pub(crate) const PASSIVE_ADRENAL_MOVEMENT_BONUS_BASIS_POINTS_BOUNDS: PassiveParameterBounds<u16> =
    PassiveParameterBounds::new(1, 10_000);
pub(crate) const PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS: PassiveParameterBounds<u32> =
    PassiveParameterBounds::new(1, 4_096_000);
pub(crate) const PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS: PassiveParameterBounds<u16> =
    PassiveParameterBounds::new(1, 30_000);
pub(crate) const PASSIVE_QUICK_CYCLE_REFILL_BASIS_POINTS_BOUNDS: PassiveParameterBounds<u16> =
    PassiveParameterBounds::new(1, 10_000);
pub(crate) const PASSIVE_TENACITY_SLOW_BASIS_POINTS_BOUNDS: PassiveParameterBounds<u16> =
    PassiveParameterBounds::new(1, 10_000);
pub(crate) const PASSIVE_ELEMENTAL_RESISTANCE_BASIS_POINTS_BOUNDS: PassiveParameterBounds<u16> =
    PassiveParameterBounds::new(1, 6_000);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassiveParameters {
    LightweightFrame,
    ReinforcedFrame,
    AdrenalResponse {
        duration_ticks: u64,
        rearm_ticks: u64,
        movement_bonus_basis_points: u16,
    },
    CloseQuarters {
        near_distance_milliunits: u32,
        far_distance_milliunits: u32,
        near_damage_basis_points: u16,
        far_damage_basis_points: u16,
    },
    QuickCycle {
        refill_duration_basis_points: u16,
    },
    Tenacity {
        slow_duration_basis_points: u16,
    },
    CryogenicInsulation {
        resistance_basis_points: u16,
    },
    FilteredCirculation {
        resistance_basis_points: u16,
    },
    HeatShielding {
        resistance_basis_points: u16,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedPassive {
    pub id: PassiveDefinitionId,
    pub kind: PassiveKind,
    pub point_cost: u8,
    pub parameters: PassiveParameters,
}

/// The immutable passive capabilities installed for one resolved loadout generation.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedPassives {
    pub passives: [ResolvedPassive; 2],
}

impl ResolvedPassives {
    #[must_use]
    pub fn find(self, kind: PassiveKind) -> Option<ResolvedPassive> {
        self.passives
            .into_iter()
            .find(|passive| passive.kind == kind)
    }
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

/// Server-local immutable components projected atomically from a resolved loadout generation.
///
/// The replicated aggregate remains the client convergence and diagnostic contract. Authoritative
/// systems consume these focused components so they cannot fall back to unrelated code defaults.
#[derive(Bundle, Clone, Debug)]
pub struct MatchLoadoutProjection {
    pub fighter_stats: ResolvedFighterStats,
    pub fighter_body: FighterBody,
    pub primary_weapon: ResolvedWeapon,
    pub ultimate: ResolvedUltimate,
    pub passives: ResolvedPassives,
}

impl MatchLoadoutProjection {
    #[must_use]
    pub fn new(loadout: &ResolvedMatchLoadout, fighter_body: FighterBody) -> Self {
        Self {
            fighter_stats: loadout.fighter_stats,
            fighter_body,
            primary_weapon: loadout.primary_weapon.clone(),
            ultimate: loadout.ultimate,
            passives: ResolvedPassives {
                passives: loadout.passives,
            },
        }
    }
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
    ElementalFieldActive {
        field_id: crate::combat::ElementalFieldId,
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
