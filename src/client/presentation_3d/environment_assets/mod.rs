//! Client-only environment catalog and runtime asset-preparation facade.

use super::*;

mod catalog;
mod fitting;
mod runtime;
mod sway;

#[cfg(test)]
pub(crate) use catalog::environment_asset_paths;
pub(in crate::client::presentation_3d) use catalog::{
    EffectTileStatusVisual, MapVisualCatalog, MapVisualFitting, MapVisualKind, MapVisualProfile,
};
pub(in crate::client::presentation_3d) use runtime::{
    EnvironmentAssetReadiness, EnvironmentImportedScenes, FittedEnvironmentScene,
    load_environment_assets, prepare_environment_scenes,
};
pub(crate) use runtime::{EnvironmentThemeMaterialCatalog, EnvironmentThemeMaterials};
pub(in crate::client::presentation_3d) use sway::{FoliageSway, sway_foliage};
