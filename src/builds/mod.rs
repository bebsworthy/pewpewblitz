//! Bounded authored build primitives (ultimates, passives, charge) and deterministic loadout
//! resolution. Roster characters (`crate::characters`) compose them into player picks.

mod definitions;
mod model;

pub(crate) use definitions::{
    MAX_PASSIVE_DEFINITIONS, MAX_ULTIMATE_DEFINITIONS, validate_fighter_stats,
};
pub(crate) use model::{
    PASSIVE_ADRENAL_DURATION_TICKS_BOUNDS, PASSIVE_ADRENAL_MOVEMENT_BONUS_BASIS_POINTS_BOUNDS,
    PASSIVE_ADRENAL_REARM_TICKS_BOUNDS, PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS,
    PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS,
    PASSIVE_ELEMENTAL_RESISTANCE_BASIS_POINTS_BOUNDS, PASSIVE_FRAME_HEALTH_BASIS_POINTS_BOUNDS,
    PASSIVE_FRAME_SPEED_BASIS_POINTS_BOUNDS, PASSIVE_QUICK_CYCLE_REFILL_BASIS_POINTS_BOUNDS,
    PASSIVE_TENACITY_SLOW_BASIS_POINTS_BOUNDS, ULTIMATE_PARAMETER_BOUNDS,
};

pub use definitions::resolve_reveal_proximity_radius;
pub use definitions::{
    BUILD_CATALOG_SCHEMA_VERSION, BUILD_FINGERPRINT_FORMAT_VERSION, BUILD_POINT_BUDGET,
    BuildCatalog, BuildCatalogResource, BuildContentPlugin, BuildResolutionError,
    MAX_COLD_CAPACITY, MAX_FIGHTER_BODY_RADIUS, MAX_FIGHTER_MOVEMENT_SPEED,
    MAX_RESOLVED_LOADOUT_BYTES, MAX_REVEAL_PROXIMITY_RADIUS, MIN_FIGHTER_BODY_RADIUS,
    MIN_REVEAL_PROXIMITY_RADIUS, PassiveDefinition, UltimateDefinition, WeaponPointCost,
    resolve_loadout,
};
pub use model::{
    AbilityPhase, AbilityState, AcceptedBuildSummary, BuildRecipeFingerprint, BuildRevision,
    DeployableId, ElementalFieldEffect, FighterBody, LoadoutRecipe, MatchLoadoutProjection,
    PASSIVE_SLOT_COUNT, PassiveDefinitionId, PassiveKind, PassiveParameters, PassiveRuntimeState,
    ResolvedFighterStats, ResolvedMatchLoadout, ResolvedPassive, ResolvedPassives,
    ResolvedUltimate, RevealProximityModifier, SelectedBuild, UltimateActivationStyle,
    UltimateChargePolicy, UltimateDefinitionId, UltimateKind, UltimateParameters,
    world_units_from_milliunits,
};

#[cfg(test)]
mod tests;
