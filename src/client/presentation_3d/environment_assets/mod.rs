//! Client-only environment catalog and runtime asset-preparation facade.

use super::*;

mod catalog;
mod fitting;
mod runtime;

#[cfg(test)]
pub(crate) use catalog::environment_asset_paths;
pub(in crate::client::presentation_3d) use catalog::{
    MapVisualCatalog, MapVisualFitting, MapVisualKind, MapVisualProfile,
};
pub(in crate::client::presentation_3d) use runtime::{
    EnvironmentAssetReadiness, EnvironmentImportedScenes, EnvironmentMaterialTint,
    FittedEnvironmentScene, load_environment_assets, prepare_environment_scenes,
    tint_environment_instance,
};
pub(crate) use runtime::{EnvironmentThemeMaterialCatalog, EnvironmentThemeMaterials};
