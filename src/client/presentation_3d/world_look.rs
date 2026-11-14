//! Presentation-only world look: HDR, filmic tonemapping, restrained bloom, and color grading.
//!
//! The arena camera renders HDR, tonemaps with [`WORLD_TONEMAPPING`], and grades with the active
//! map theme's [`ThemeGrading`]. Bloom thresholds the HDR main pass so only deliberately HDR
//! materials (projectile cores, muzzle/explosion flashes, pickups) glow; reduced effects removes
//! bloom but keeps HDR, tonemapping, and grading so colors do not shift. The HUD camera shares the
//! HDR target with no tonemapping, grading, or bloom, so it composites untouched on top. Offscreen
//! previews use the same tonemapper and the preview grading without HDR or bloom.

use super::*;
use bevy::{
    camera::Hdr,
    post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter},
    render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection},
};
use serde::Deserialize;

const WORLD_LOOK_CATALOG: &str = include_str!("../../../assets/catalogs/world_look.ron");
const WORLD_LOOK_SCHEMA_VERSION: u16 = 1;
/// The one filmic tonemapper for the world and its previews (chosen over `AgX` in BRL-0146).
pub(in crate::client) const WORLD_TONEMAPPING: Tonemapping = Tonemapping::TonyMcMapface;
/// Additive bloom stays a restrained accent rather than a screen-wide haze.
const MAX_BLOOM_INTENSITY: f32 = 0.5;
/// Lit surfaces must stay below the threshold, so it may not sink into the lit range.
const MIN_BLOOM_THRESHOLD: f32 = 1.0;
const MAX_BLOOM_THRESHOLD: f32 = 8.0;
const MAX_GRADING_EXPOSURE_EV: f32 = 3.0;
const MAX_GRADING_WHITE_BALANCE: f32 = 0.5;
const MIN_GRADING_SATURATION: f32 = 0.8;
const MAX_GRADING_SATURATION: f32 = 1.4;
const MIN_GRADING_CONTRAST: f32 = 0.8;
const MAX_GRADING_CONTRAST: f32 = 1.3;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct WorldLookCatalogSource {
    schema_version: u16,
    bloom: BloomTuning,
    preview_grading: ThemeGrading,
}

/// Restrained bloom tuning. A threshold is used, so composition is additive.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct BloomTuning {
    pub(super) intensity: f32,
    pub(super) low_frequency_boost: f32,
    /// Max-channel HDR value (before grading) where bloom starts.
    pub(super) threshold: f32,
    pub(super) threshold_softness: f32,
}

impl BloomTuning {
    fn bloom(self) -> Bloom {
        Bloom {
            intensity: self.intensity,
            low_frequency_boost: self.low_frequency_boost,
            prefilter: BloomPrefilter {
                threshold: self.threshold,
                threshold_softness: self.threshold_softness,
            },
            composite_mode: BloomCompositeMode::Additive,
            ..Bloom::NATURAL
        }
    }
}

/// Authored grading for one map theme (or the offscreen previews).
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(in crate::client::presentation_3d) struct ThemeGrading {
    /// Exposure offset in stops, applied after the bloom prefilter and before tonemapping.
    pub(in crate::client::presentation_3d) exposure: f32,
    /// CIE x white-balance shift: positive is warmer.
    pub(in crate::client::presentation_3d) temperature: f32,
    /// CIE y white-balance shift: positive is more magenta.
    pub(in crate::client::presentation_3d) tint: f32,
    pub(in crate::client::presentation_3d) saturation: f32,
    pub(in crate::client::presentation_3d) contrast: f32,
}

impl ThemeGrading {
    pub(in crate::client::presentation_3d) fn validate(&self, owner: &str) -> Result<(), String> {
        check_range(
            owner,
            "exposure",
            self.exposure,
            -MAX_GRADING_EXPOSURE_EV,
            MAX_GRADING_EXPOSURE_EV,
        )?;
        check_range(
            owner,
            "temperature",
            self.temperature,
            -MAX_GRADING_WHITE_BALANCE,
            MAX_GRADING_WHITE_BALANCE,
        )?;
        check_range(
            owner,
            "tint",
            self.tint,
            -MAX_GRADING_WHITE_BALANCE,
            MAX_GRADING_WHITE_BALANCE,
        )?;
        check_range(
            owner,
            "saturation",
            self.saturation,
            MIN_GRADING_SATURATION,
            MAX_GRADING_SATURATION,
        )?;
        check_range(
            owner,
            "contrast",
            self.contrast,
            MIN_GRADING_CONTRAST,
            MAX_GRADING_CONTRAST,
        )
    }

    pub(in crate::client::presentation_3d) fn color_grading(self) -> ColorGrading {
        ColorGrading::with_identical_sections(
            ColorGradingGlobal {
                exposure: self.exposure,
                temperature: self.temperature,
                tint: self.tint,
                ..default()
            },
            ColorGradingSection {
                saturation: self.saturation,
                contrast: self.contrast,
                ..default()
            },
        )
    }
}

fn check_range(owner: &str, name: &str, value: f32, min: f32, max: f32) -> Result<(), String> {
    if value.is_finite() && (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(format!("{owner} {name} = {value} is outside {min}..={max}"))
    }
}

/// Validated client-only world look tuning, embedded from `assets/catalogs/world_look.ron`.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(in crate::client) struct WorldLookCatalog {
    pub(super) bloom: BloomTuning,
    preview_grading: ThemeGrading,
}

impl WorldLookCatalog {
    pub(super) fn embedded() -> Result<Self, String> {
        Self::from_ron(WORLD_LOOK_CATALOG)
    }

    fn from_ron(source: &str) -> Result<Self, String> {
        let source: WorldLookCatalogSource = ron::from_str(source)
            .map_err(|error| format!("world look catalog parse failed: {error}"))?;
        if source.schema_version != WORLD_LOOK_SCHEMA_VERSION {
            return Err(format!(
                "world look catalog schema {} is not {WORLD_LOOK_SCHEMA_VERSION}",
                source.schema_version
            ));
        }
        let bloom = source.bloom;
        let owner = "world look bloom";
        check_range(
            owner,
            "intensity",
            bloom.intensity,
            0.0,
            MAX_BLOOM_INTENSITY,
        )?;
        check_range(
            owner,
            "low_frequency_boost",
            bloom.low_frequency_boost,
            0.0,
            1.0,
        )?;
        check_range(
            owner,
            "threshold",
            bloom.threshold,
            MIN_BLOOM_THRESHOLD,
            MAX_BLOOM_THRESHOLD,
        )?;
        check_range(
            owner,
            "threshold_softness",
            bloom.threshold_softness,
            0.0,
            1.0,
        )?;
        source
            .preview_grading
            .validate("world look preview_grading")?;
        Ok(Self {
            bloom,
            preview_grading: source.preview_grading,
        })
    }

    /// Tonemapping and grading for an offscreen preview camera that shows world assets.
    pub(in crate::client) fn preview_camera_look(&self) -> (Tonemapping, ColorGrading) {
        (WORLD_TONEMAPPING, self.preview_grading.color_grading())
    }
}

/// Post-processing components for the arena camera at spawn. The active theme replaces the
/// identity grading when its map is presented; bloom is reconciled by [`sync_arena_bloom`].
pub(super) fn arena_camera_look() -> (Hdr, Tonemapping, ColorGrading) {
    (Hdr, WORLD_TONEMAPPING, ColorGrading::default())
}

/// The HUD camera shares the HDR target but is never tonemapped, graded, or bloomed.
pub(super) fn hud_camera_look() -> (Hdr, Tonemapping, ColorGrading) {
    (Hdr, Tonemapping::None, ColorGrading::default())
}

/// Keeps bloom on the arena camera only while reduced effects is off and the quality tier keeps it.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are passed by value"
)]
pub(super) fn sync_arena_bloom(
    mut commands: Commands,
    catalog: Res<WorldLookCatalog>,
    settings: Option<Res<ClientShellSettings>>,
    quality: Option<Res<render_quality::ActiveRenderQuality>>,
    cameras: Query<(Entity, Has<Bloom>), With<ArenaCamera>>,
) {
    let wanted = !settings.is_some_and(|settings| settings.reduced_combat_effects)
        && render_quality::bloom_enabled(quality.as_deref());
    for (camera, has_bloom) in &cameras {
        if wanted && !has_bloom {
            commands.entity(camera).try_insert(catalog.bloom.bloom());
        } else if !wanted && has_bloom {
            commands.entity(camera).try_remove::<Bloom>();
        }
    }
}

/// Applies a theme's grading to the arena camera when that theme's lights are applied.
pub(super) fn apply_theme_grading(
    grading: ThemeGrading,
    cameras: &mut Query<&mut ColorGrading, With<ArenaCamera>>,
) {
    for mut camera_grading in cameras.iter_mut() {
        *camera_grading = grading.color_grading();
    }
}

pub(in crate::client) fn install(app: &mut App) {
    app.insert_resource(
        WorldLookCatalog::embedded().expect("the embedded world look catalog must be valid"),
    )
    .add_systems(
        Update,
        sync_arena_bloom.in_set(WorldPresentationSet::ReconcileState),
    );
}

#[cfg(test)]
mod tests;
