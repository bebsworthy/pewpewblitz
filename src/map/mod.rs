//! Typed map content, authoritative runtime state, and client reconstruction boundaries.

#[cfg(feature = "client")]
mod client;
mod definitions;
mod model;
#[cfg(feature = "server")]
mod server;
#[cfg(all(test, feature = "server"))]
mod tests;

#[cfg(feature = "client")]
pub use client::{
    ClientMapReadiness, MapPresentationMember, MapPresentationPlugin, MapPresentationSet,
    PresentedMap, ZoneObjectiveBoundary, ZoneObjectiveFill, perimeter_visual_shapes,
};
pub use definitions::{
    EngineMapLimits, HOT_ZONE_LAYOUT_SCHEMA_VERSION, HOT_ZONE_MAP_PRESET, HOT_ZONE_MODE_DEFINITION,
    HOT_ZONE_OBJECTIVE_ANCHOR_DEFINITION, HOT_ZONE_OBJECTIVE_PRESENTATION_PROFILE,
    MapCatalogResource, MapContentCatalog, MapContentPlugin, MapLayoutRequirements, MapPreset,
    MapRecipePolicy, PRACTICE_DUMMY_ANCHOR_DEFINITION, RequiredAnchorShape,
    SANDBOX_LAYOUT_SCHEMA_VERSION, WIPEOUT_LAYOUT_SCHEMA_VERSION, WIPEOUT_MODE_DEFINITION,
    objective_presentation_profile, resolve_map_recipe,
};
pub use model::*;
#[cfg(feature = "server")]
pub use server::{
    AuthoritativeMapPlugin, BUILT_IN_MAP_PRESET, MapStartupSet, NextMapInstanceId,
    ServerMapSelection, install_resolved_map, perimeter_wall_shapes, teardown_authoritative_map,
};
