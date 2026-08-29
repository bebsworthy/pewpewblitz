//! Bounded authored brawler builds and deterministic loadout resolution.

mod definitions;
mod model;

pub(crate) use definitions::{MAX_PASSIVE_DEFINITIONS, MAX_ULTIMATE_DEFINITIONS};

pub use definitions::resolve_reveal_proximity_radius;
pub use definitions::{
    BUILD_CATALOG_SCHEMA_VERSION, BUILD_FINGERPRINT_FORMAT_VERSION, BUILD_POINT_BUDGET,
    BuildCatalog, BuildCatalogResource, BuildContentPlugin, BuildResolutionError,
    CustomPulseTuning, FighterStatProfiles, MAX_BUILD_CANDIDATE_BYTES, MAX_COLD_CAPACITY,
    MAX_FIGHTER_MOVEMENT_SPEED, MAX_RESOLVED_LOADOUT_BYTES, MAX_REVEAL_PROXIMITY_RADIUS,
    MIN_REVEAL_PROXIMITY_RADIUS, PassiveDefinition, PulseMagazineTuning, PulsePowerTuning,
    PulseReachTuning, UltimateDefinition, WeaponPointCost, build_point_total, resolve_build_recipe,
    resolve_saved_brawler_recipe,
};
pub use model::{
    AbilityPhase, AbilityState, AcceptedBuildSummary, BrawlerBuildRecipe, BuildRecipeFingerprint,
    BuildRevision, DeployableId, ElementalFieldEffect, PassiveDefinitionId, PassiveKind,
    PassiveRuntimeState, PulseMagazine, PulsePower, PulseReach, ResolvedFighterStats,
    ResolvedMatchLoadout, ResolvedPassive, ResolvedUltimate, RevealProximityModifier,
    SelectedBuild, UltimateActivationStyle, UltimateDefinitionId, UltimateKind, UltimateParameters,
    WeaponChoice, world_units_from_milliunits,
};

#[cfg(test)]
mod tests;
