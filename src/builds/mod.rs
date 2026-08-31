//! Bounded authored brawler builds and deterministic loadout resolution.

mod definitions;
mod model;

#[cfg(feature = "server")]
pub(crate) use definitions::resolve_direct_diagnostic_loadout;
pub(crate) use definitions::{MAX_PASSIVE_DEFINITIONS, MAX_ULTIMATE_DEFINITIONS};
pub(crate) use model::{
    PASSIVE_ADRENAL_DURATION_TICKS_BOUNDS, PASSIVE_ADRENAL_MOVEMENT_BONUS_BASIS_POINTS_BOUNDS,
    PASSIVE_ADRENAL_REARM_TICKS_BOUNDS, PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS,
    PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS,
    PASSIVE_ELEMENTAL_RESISTANCE_BASIS_POINTS_BOUNDS,
    PASSIVE_QUICK_CYCLE_REFILL_BASIS_POINTS_BOUNDS, PASSIVE_TENACITY_SLOW_BASIS_POINTS_BOUNDS,
};

pub use definitions::resolve_reveal_proximity_radius;
pub use definitions::{
    BUILD_CATALOG_SCHEMA_VERSION, BUILD_FINGERPRINT_FORMAT_VERSION, BUILD_POINT_BUDGET,
    BuildCatalog, BuildCatalogResource, BuildContentPlugin, BuildResolutionError,
    CustomPulseTuning, FighterStatProfiles, MAX_BUILD_CANDIDATE_BYTES, MAX_COLD_CAPACITY,
    MAX_FIGHTER_BODY_RADIUS, MAX_FIGHTER_MOVEMENT_SPEED, MAX_RESOLVED_LOADOUT_BYTES,
    MAX_REVEAL_PROXIMITY_RADIUS, MIN_FIGHTER_BODY_RADIUS, MIN_REVEAL_PROXIMITY_RADIUS,
    PassiveDefinition, PulseMagazineTuning, PulsePowerTuning, PulseReachTuning, UltimateDefinition,
    WeaponPointCost, build_point_total, resolve_build_recipe, resolve_saved_brawler_recipe,
};
pub use model::{
    AbilityPhase, AbilityState, AcceptedBuildSummary, BrawlerBuildRecipe, BuildRecipeFingerprint,
    BuildRevision, DeployableId, ElementalFieldEffect, FighterBody, MatchLoadoutProjection,
    PassiveDefinitionId, PassiveKind, PassiveParameters, PassiveRuntimeState, PulseMagazine,
    PulsePower, PulseReach, ResolvedFighterStats, ResolvedMatchLoadout, ResolvedPassive,
    ResolvedPassives, ResolvedUltimate, RevealProximityModifier, SelectedBuild,
    UltimateActivationStyle, UltimateChargePolicy, UltimateDefinitionId, UltimateKind,
    UltimateParameters, WeaponChoice, world_units_from_milliunits,
};

#[cfg(test)]
mod tests;
