//! Bounded authored brawler builds and deterministic loadout resolution.

mod definitions;
mod model;
#[cfg(feature = "server")]
pub(crate) mod server;
#[cfg(feature = "server")]
mod telemetry;

pub use definitions::{
    BUILD_CATALOG_SCHEMA_VERSION, BUILD_FINGERPRINT_FORMAT_VERSION, BUILD_POINT_BUDGET,
    BuildCatalog, BuildCatalogResource, BuildContentPlugin, BuildPresetDefinition,
    BuildResolutionError, MAX_BUILD_CANDIDATE_BYTES, MAX_RESOLVED_LOADOUT_BYTES, PassiveDefinition,
    UltimateDefinition, WeaponPointCost, build_point_total, resolve_build_recipe,
};
pub use model::{
    AbilityPhase, AbilityState, AcceptedBuildSummary, BrawlerBuildRecipe, BuildCandidate,
    BuildPresetId, BuildRecipeFingerprint, BuildRevision, BuildSelection, DeployableId,
    PassiveDefinitionId, PassiveKind, PassiveRuntimeState, PulseMagazine, PulsePower, PulseReach,
    ResolvedFighterStats, ResolvedMatchLoadout, ResolvedPassive, ResolvedUltimate, SelectedBuild,
    SelectingBuild, UltimateDefinitionId, UltimateKind, WeaponChoice,
};
#[cfg(feature = "server")]
pub use telemetry::{BuildSelectionTelemetryRecord, BuildTelemetry};

#[cfg(test)]
mod tests;
