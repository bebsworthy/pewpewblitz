//! Shared stylized toon material for fighters and the world (BRL-0150).
//!
//! [`ToonMaterial`] extends `StandardMaterial` with a two-band light ramp, a hue-shifted shadow
//! band, and a lit-side rim (`assets/shaders/toon.wgsl`). Code-built world and fighter palettes are
//! created as toon handles through [`ToonShading::material`]; imported glTF instances spawned under
//! a [`ToonScene`] root are converted once, when the scene is ready, by [`prepare_toon_scene`].
//! The rim colour is chosen per mesh by its [`RimRelation`] tag, so shared materials need no
//! per-relationship variants. Tuning lives in `assets/catalogs/toon_shading.ron`; everything here is
//! client presentation outside the gameplay fingerprint.

#[allow(
    clippy::wildcard_imports,
    reason = "the world-renderer submodule consumes the presentation_3d composition surface"
)]
use super::*;
use bevy::{
    ecs::system::SystemParam,
    mesh::MeshTag,
    pbr::{ExtendedMaterial, MaterialExtension},
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};
use serde::Deserialize;
use std::collections::HashMap;

const TOON_SHADING_CATALOG: &str = include_str!("../../../../assets/catalogs/toon_shading.ron");
const TOON_SHADING_SCHEMA_VERSION: u16 = 1;
const TOON_SHADER_PATH: &str = "shaders/toon.wgsl";
const MAX_RAMP_THRESHOLD: f32 = 0.9;
const MAX_RAMP_SOFTNESS: f32 = 0.25;
const MIN_FLAT_LEVEL: f32 = 0.3;
/// The shadow band must stay readable (no crush) and darker than the lit band.
const MIN_SHADOW_TINT: f32 = 0.3;
const MAX_SHADOW_TINT: f32 = 1.0;
const MAX_RIM_STRENGTH: f32 = 0.6;
/// Distinct imported materials; past the ceiling a mesh keeps PBR shading.
const MAX_TOON_SCENE_MATERIALS: usize = 512;

/// The shared toon material: `StandardMaterial` inputs with toon lighting.
pub(crate) type ToonMaterial = ExtendedMaterial<StandardMaterial, ToonExtension>;

/// Catalog-derived toon uniform, identical for every toon material.
#[derive(Asset, AsBindGroup, TypePath, Clone, Copy, Debug, PartialEq)]
pub(crate) struct ToonExtension {
    #[uniform(100)]
    settings: ToonSettings,
}

#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
struct ToonSettings {
    shadow_tint: Vec4,
    /// Threshold, softness, flat level, lit gradient.
    ramp: Vec4,
    /// Rim start and end on `1 − N·V`, then on the screen-sideways normal component.
    rim: Vec4,
    /// Rim colour (rgb) and strength (a) per [`RimRelation`].
    rim_colors: [Vec4; 4],
}

impl MaterialExtension for ToonExtension {
    fn fragment_shader() -> ShaderRef {
        TOON_SHADER_PATH.into()
    }
}

/// Which relationship rim a fighter's toon meshes use; written as the mesh's `MeshTag`. Untagged
/// meshes (tag 0) use the world rim, and the shader skips received shadows only for tagged ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::presentation_3d) enum RimRelation {
    Local = 1,
    Ally = 2,
    Enemy = 3,
}

impl RimRelation {
    pub(in crate::presentation_3d) const fn tag(self) -> MeshTag {
        MeshTag(self as u32)
    }
}

/// Material families the hit flash and concealment variants operate on.
pub(in crate::presentation_3d) trait ShadedMaterial:
    Material + Clone
{
    fn standard(&self) -> &StandardMaterial;
    fn standard_mut(&mut self) -> &mut StandardMaterial;
}

impl ShadedMaterial for StandardMaterial {
    fn standard(&self) -> &StandardMaterial {
        self
    }

    fn standard_mut(&mut self) -> &mut StandardMaterial {
        self
    }
}

impl ShadedMaterial for ToonMaterial {
    fn standard(&self) -> &StandardMaterial {
        &self.base
    }

    fn standard_mut(&mut self) -> &mut StandardMaterial {
        &mut self.base
    }
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct ToonShadingSource {
    schema_version: u16,
    ramp: RampTuning,
    shadow_tint: (f32, f32, f32),
    rim: RimTuning,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct RampTuning {
    threshold: f32,
    softness: f32,
    flat_level: f32,
    lit_gradient: f32,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct RimTuning {
    start: f32,
    end: f32,
    side_start: f32,
    side_end: f32,
    world: RimColor,
    local: RimColor,
    ally: RimColor,
    enemy: RimColor,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct RimColor {
    color: (f32, f32, f32),
    strength: f32,
}

impl RimColor {
    fn validate(self, name: &str) -> Result<Vec4, String> {
        let owner = format!("toon rim {name}");
        for channel in [self.color.0, self.color.1, self.color.2] {
            check_range(&owner, "color", channel, 0.0, 1.0)?;
        }
        check_range(&owner, "strength", self.strength, 0.0, MAX_RIM_STRENGTH)?;
        Ok(Vec4::new(
            self.color.0,
            self.color.1,
            self.color.2,
            self.strength,
        ))
    }
}

fn check_range(owner: &str, name: &str, value: f32, min: f32, max: f32) -> Result<(), String> {
    if value.is_finite() && (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(format!("{owner} {name} = {value} is outside {min}..={max}"))
    }
}

/// Validated toon tuning, embedded from `assets/catalogs/toon_shading.ron`.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(crate) struct ToonShading {
    extension: ToonExtension,
}

impl ToonShading {
    pub(crate) fn embedded() -> Result<Self, String> {
        Self::from_ron(TOON_SHADING_CATALOG)
    }

    fn from_ron(source: &str) -> Result<Self, String> {
        let source: ToonShadingSource = ron::from_str(source)
            .map_err(|error| format!("toon shading catalog parse failed: {error}"))?;
        if source.schema_version != TOON_SHADING_SCHEMA_VERSION {
            return Err(format!(
                "toon shading catalog schema {} is not {TOON_SHADING_SCHEMA_VERSION}",
                source.schema_version
            ));
        }
        let ramp = source.ramp;
        check_range(
            "toon ramp",
            "threshold",
            ramp.threshold,
            0.0,
            MAX_RAMP_THRESHOLD,
        )?;
        check_range(
            "toon ramp",
            "softness",
            ramp.softness,
            0.0,
            MAX_RAMP_SOFTNESS,
        )?;
        check_range(
            "toon ramp",
            "flat_level",
            ramp.flat_level,
            MIN_FLAT_LEVEL,
            1.0,
        )?;
        check_range("toon ramp", "lit_gradient", ramp.lit_gradient, 0.0, 1.0)?;
        let tint = source.shadow_tint;
        for channel in [tint.0, tint.1, tint.2] {
            check_range(
                "toon",
                "shadow_tint",
                channel,
                MIN_SHADOW_TINT,
                MAX_SHADOW_TINT,
            )?;
        }
        let rim = source.rim;
        for (start_name, start, end_name, end) in [
            ("start", rim.start, "end", rim.end),
            ("side_start", rim.side_start, "side_end", rim.side_end),
        ] {
            check_range("toon rim", start_name, start, 0.0, 1.0)?;
            check_range("toon rim", end_name, end, 0.0, 1.0)?;
            if end <= start {
                return Err(format!(
                    "toon rim {end_name} {end} must exceed {start_name} {start}"
                ));
            }
        }
        let rim_colors = [
            rim.world.validate("world")?,
            rim.local.validate("local")?,
            rim.ally.validate("ally")?,
            rim.enemy.validate("enemy")?,
        ];
        Ok(Self {
            extension: ToonExtension {
                settings: ToonSettings {
                    shadow_tint: Vec4::new(tint.0, tint.1, tint.2, 1.0),
                    ramp: Vec4::new(
                        ramp.threshold,
                        ramp.softness,
                        ramp.flat_level,
                        ramp.lit_gradient,
                    ),
                    rim: Vec4::new(rim.start, rim.end, rim.side_start, rim.side_end),
                    rim_colors,
                },
            },
        })
    }

    /// The rim colour (sRGB as authored) of one fighter relation; the fighter outline's optional
    /// relationship tint reuses it so both relationship cues agree.
    pub(in crate::presentation_3d) fn rim_rgb(&self, relation: RimRelation) -> [f32; 3] {
        self.extension.settings.rim_colors[relation as usize]
            .truncate()
            .to_array()
    }

    /// The toon material for `base`. The single place every toon handle's shading is chosen, so a
    /// later quality tier (BRL-0161) can adjust it here.
    pub(crate) fn material(&self, base: StandardMaterial) -> ToonMaterial {
        ToonMaterial {
            base,
            extension: self.extension,
        }
    }
}

/// Marks a world-asset instance root whose meshes use the toon material.
#[derive(Component, Clone, Copy, Debug, Default)]
pub(in crate::presentation_3d) struct ToonScene;

/// Toon materials converted from imported scene materials, keyed by source material.
#[derive(Resource, Default)]
pub(in crate::presentation_3d) struct ToonSceneMaterials {
    handles: HashMap<AssetId<StandardMaterial>, Handle<ToonMaterial>>,
}

/// Everything the scene conversion reads and writes.
#[derive(SystemParam)]
pub(in crate::presentation_3d) struct ToonSceneConversion<'w, 's> {
    commands: Commands<'w, 's>,
    roots: Query<'w, 's, (), With<ToonScene>>,
    children: Query<'w, 's, &'static Children>,
    meshes: Query<'w, 's, &'static MeshMaterial3d<StandardMaterial>>,
    standard: Res<'w, Assets<StandardMaterial>>,
    toon: ResMut<'w, Assets<ToonMaterial>>,
    shading: Res<'w, ToonShading>,
    converted: ResMut<'w, ToonSceneMaterials>,
}

impl ToonSceneConversion<'_, '_> {
    /// Converts every `StandardMaterial` mesh under a [`ToonScene`] root to the toon material.
    /// Other roots are left untouched.
    fn convert(&mut self, root: Entity) {
        if !self.roots.contains(root) {
            return;
        }
        for entity in self.children.iter_descendants(root) {
            let Ok(source) = self.meshes.get(entity) else {
                continue;
            };
            let key = source.0.id();
            let handle = if let Some(handle) = self.converted.handles.get(&key) {
                handle.clone()
            } else {
                if self.converted.handles.len() >= MAX_TOON_SCENE_MATERIALS {
                    continue;
                }
                let Some(base) = self.standard.get(&source.0).cloned() else {
                    continue;
                };
                let handle = self.toon.add(self.shading.material(base));
                self.converted.handles.insert(key, handle.clone());
                handle
            };
            self.commands
                .entity(entity)
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d(handle));
        }
    }
}

/// Converts a glTF instance's materials once, when its scene is ready.
#[allow(
    clippy::needless_pass_by_value,
    reason = "the Bevy observer owns its triggered world-instance event"
)]
pub(in crate::presentation_3d) fn prepare_toon_scene(
    ready: On<WorldInstanceReady>,
    mut conversion: ToonSceneConversion,
) {
    conversion.convert(ready.entity);
}

pub(crate) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<ToonMaterial>::default())
        .insert_resource(
            ToonShading::embedded().expect("the embedded toon shading catalog must be valid"),
        )
        .init_resource::<ToonSceneMaterials>()
        .add_observer(prepare_toon_scene);
}

#[cfg(test)]
mod tests;
