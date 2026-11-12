//! The short white hit flash on a damaged presentation root.
//!
//! The flash swaps each descendant `StandardMaterial` for a cached white/emissive variant of the
//! material it currently shows, so a concealed (translucent) fighter flashes translucent, and
//! restores the base when the flash ends. It runs after every other material writer each frame.

use super::super::combat::FighterGroundMarker3d;
use super::*;

/// Distinct source materials that may receive a flash variant. Fighters, deployables, NPCs, and
/// damageable objects use a few dozen; past the ceiling a new material simply does not flash.
const MAX_HIT_FLASH_VARIANTS: usize = 256;

/// An active flash on one presentation root.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(in crate::client::presentation_3d) struct HitFlash {
    pub(super) remaining_seconds: f32,
    pub(super) reduced: bool,
}

impl HitFlash {
    pub(super) fn new(tuning: &FlashTuning, reduced: bool) -> Self {
        let millis = if reduced {
            tuning.reduced_duration_millis
        } else {
            tuning.duration_millis
        };
        Self {
            remaining_seconds: millis / 1_000.0,
            reduced,
        }
    }
}

/// Cached flash variants keyed by source material, and the reverse map back to each base.
#[derive(Resource, Default)]
pub(in crate::client::presentation_3d) struct HitFlashMaterials {
    variants: HashMap<(AssetId<StandardMaterial>, bool), Handle<StandardMaterial>>,
    bases: HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>>,
}

impl HitFlashMaterials {
    /// The material a flash variant was derived from, or the handle itself when it is not one.
    pub(in crate::client::presentation_3d) fn base_of<'a>(
        &'a self,
        handle: &'a Handle<StandardMaterial>,
    ) -> &'a Handle<StandardMaterial> {
        self.bases.get(&handle.id()).unwrap_or(handle)
    }

    pub(super) fn is_flash(&self, handle: &Handle<StandardMaterial>) -> bool {
        self.bases.contains_key(&handle.id())
    }

    #[cfg(test)]
    pub(super) fn variant_count(&self) -> usize {
        self.variants.len()
    }

    fn variant(
        &mut self,
        source: &Handle<StandardMaterial>,
        reduced: bool,
        tuning: &FlashTuning,
        materials: &mut Assets<StandardMaterial>,
    ) -> Option<Handle<StandardMaterial>> {
        if let Some(handle) = self.variants.get(&(source.id(), reduced)) {
            return Some(handle.clone());
        }
        if self.variants.len() >= MAX_HIT_FLASH_VARIANTS {
            return None;
        }
        let material = flash_material(materials.get(source)?, tuning, reduced);
        let handle = materials.add(material);
        self.variants.insert((source.id(), reduced), handle.clone());
        self.bases.insert(handle.id(), source.clone());
        Some(handle)
    }
}

/// White-shifted, emissive copy of `source` that keeps its alpha and blend mode.
pub(super) fn flash_material(
    source: &StandardMaterial,
    tuning: &FlashTuning,
    reduced: bool,
) -> StandardMaterial {
    let (mix, emissive) = if reduced {
        (tuning.reduced_base_color_mix, tuning.reduced_emissive)
    } else {
        (tuning.base_color_mix, tuning.emissive)
    };
    let mut material = source.clone();
    let base = source.base_color.to_srgba();
    let lerp = |channel: f32| channel + (1.0 - channel) * mix;
    material.base_color = Color::srgba(
        lerp(base.red),
        lerp(base.green),
        lerp(base.blue),
        base.alpha,
    );
    let white = LinearRgba::new(emissive, emissive, emissive, 1.0);
    material.emissive = LinearRgba::new(
        source.emissive.red.max(white.red),
        source.emissive.green.max(white.green),
        source.emissive.blue.max(white.blue),
        1.0,
    );
    material
}

/// Count down active flashes and keep every descendant material of a flashing root on its flash
/// variant; restore the base materials on the frame a flash ends.
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "one flash pass owns the flash lifecycle, the descendant material swap, and its bounded variant cache"
)]
pub(in crate::client::presentation_3d) fn apply_hit_flash_materials(
    mut commands: Commands,
    time: Res<Time>,
    catalog: Res<HitFeedbackCatalog>,
    mut flashes: Query<(Entity, &mut HitFlash)>,
    children: Query<&Children>,
    mut meshes: Query<&mut MeshMaterial3d<StandardMaterial>, Without<FighterGroundMarker3d>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut variants: ResMut<HitFlashMaterials>,
) {
    let delta = time.delta_secs();
    for (root, mut flash) in &mut flashes {
        let active = flash.remaining_seconds > 0.0;
        for entity in core::iter::once(root).chain(children.iter_descendants(root)) {
            let Ok(mut material) = meshes.get_mut(entity) else {
                continue;
            };
            if active {
                if variants.is_flash(&material.0) {
                    continue;
                }
                if let Some(handle) =
                    variants.variant(&material.0, flash.reduced, &catalog.flash, &mut materials)
                {
                    material.0 = handle;
                }
            } else if variants.is_flash(&material.0) {
                let base = variants.base_of(&material.0).clone();
                material.0 = base;
            }
        }
        if active {
            flash.remaining_seconds -= delta;
        } else {
            commands.entity(root).remove::<HitFlash>();
        }
    }
}
