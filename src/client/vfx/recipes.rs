//! Bounded reusable mesh layers and recipe-owned runtime policy.

use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::catalog::{
    VfxAnchorPolicy, VfxLifetime, VfxMaterialKey, VfxRendererFamily, VfxScalePolicy,
};

pub(super) const MAX_VFX_RECIPES: usize = 128;
pub(super) const MAX_VFX_LAYERS: usize = 256;
pub(super) const MAX_VFX_PARTICLE_LAYERS: usize = 64;
pub(super) const MAX_VFX_LAYERS_PER_RECIPE: usize = 8;
pub(super) const MAX_VFX_PARTICLE_LAYERS_PER_RECIPE: usize = 2;
pub(super) const MAX_VFX_CONCURRENCY: usize = 96;
pub(super) const MAX_VFX_LIFETIME_MILLIS: u32 = 10_000;
pub(crate) const MAX_VFX_PARTICLES_PER_LAYER: u16 = 256;
const MAX_VFX_ID_BYTES: usize = 64;
const MAX_TIMELINE_SCALE: f32 = 16.0;
const MAX_PARTICLE_WORLD_VALUE: f32 = 512.0;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) enum VfxReadabilityClass {
    CriticalGeometry,
    Identity,
    Decorative,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) enum VfxEasing {
    Linear,
    SmoothStep,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct VfxScaleKeyframes {
    pub(crate) start: f32,
    pub(crate) peak_at_basis_points: u16,
    pub(crate) peak: f32,
    pub(crate) end: f32,
}

impl VfxScaleKeyframes {
    fn validate(self) -> bool {
        [self.start, self.peak, self.end]
            .into_iter()
            .all(|value| value.is_finite() && (0.0..=MAX_TIMELINE_SCALE).contains(&value))
            && self.peak_at_basis_points <= 10_000
    }

    pub(crate) fn sample(self, progress: f32, easing: VfxEasing) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        let peak_at = f32::from(self.peak_at_basis_points) / 10_000.0;
        let (from, to, local) = if progress <= peak_at {
            let local = if peak_at <= f32::EPSILON {
                1.0
            } else {
                progress / peak_at
            };
            (self.start, self.peak, local)
        } else {
            let tail = 1.0 - peak_at;
            let local = if tail <= f32::EPSILON {
                1.0
            } else {
                (progress - peak_at) / tail
            };
            (self.peak, self.end, local)
        };
        let local = match easing {
            VfxEasing::Linear => local,
            VfxEasing::SmoothStep => local * local * (3.0 - 2.0 * local),
        };
        from + (to - from) * local
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct VfxTimeline {
    pub(crate) delay_millis: u16,
    pub(crate) duration: VfxLifetime,
    pub(crate) easing: VfxEasing,
    pub(crate) scale: VfxScaleKeyframes,
}

impl VfxTimeline {
    pub(crate) fn scale_at(self, elapsed: Duration, duration: Duration) -> Option<f32> {
        let delay = Duration::from_millis(u64::from(self.delay_millis));
        let elapsed = elapsed.checked_sub(delay)?;
        let progress = if duration.is_zero() {
            1.0
        } else {
            (elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
        };
        Some(self.scale.sample(progress, self.easing))
    }

    fn validate(self) -> bool {
        let duration_valid = match self.duration {
            VfxLifetime::Millis(millis) => {
                millis > 0
                    && u32::from(self.delay_millis).saturating_add(millis)
                        <= MAX_VFX_LIFETIME_MILLIS
            }
            VfxLifetime::AuthoritativeDeadline => {
                u32::from(self.delay_millis) <= MAX_VFX_LIFETIME_MILLIS
            }
        };
        duration_valid && self.scale.validate()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct VfxMeshLayer {
    pub(crate) id: String,
    pub(crate) renderer: VfxRendererFamily,
    pub(crate) material: VfxMaterialKey,
    pub(crate) scale: VfxScalePolicy,
    pub(crate) anchor: VfxAnchorPolicy,
    pub(crate) timeline: VfxTimeline,
    pub(crate) readability: VfxReadabilityClass,
    pub(crate) casts_shadows: bool,
    pub(crate) receives_shadows: bool,
}

impl VfxMeshLayer {
    pub(super) fn validate(&self) -> Result<(), String> {
        if !valid_vfx_catalog_id(&self.id) {
            return Err(format!("invalid VFX mesh-layer id: {}", self.id));
        }
        if !(self.scale.has_valid_bounds()
            && self.anchor.has_valid_bounds()
            && self.timeline.validate()
            && matches!(
                (self.renderer, self.anchor),
                (
                    VfxRendererFamily::Sphere,
                    VfxAnchorPolicy::FixedWorldHeight(_)
                        | VfxAnchorPolicy::AuthoritativeRadiusHeight(_)
                ) | (
                    VfxRendererFamily::GroundRing
                        | VfxRendererFamily::GroundThinRing
                        | VfxRendererFamily::GroundDisc
                        | VfxRendererFamily::GroundSpark,
                    VfxAnchorPolicy::GroundOffset(_)
                )
            ))
        {
            return Err(format!(
                "VFX mesh layer {} has unsafe runtime values",
                self.id
            ));
        }
        Ok(())
    }

    pub(super) const fn requires_authoritative_radius(&self) -> bool {
        self.scale.requires_authoritative_radius() || self.anchor.requires_authoritative_radius()
    }

    pub(super) const fn requires_authoritative_deadline(&self) -> bool {
        matches!(self.timeline.duration, VfxLifetime::AuthoritativeDeadline)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) enum VfxParticleProfile {
    ColdFragments,
    FireEmbers,
    PoisonBubbles,
    RestorationPetals,
    ExplosionSparks,
    ExplosionSmoke,
    ProjectileSmoke,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) enum VfxParticleSpawn {
    Burst(u16),
    RatePerSecond(u16),
}

/// Bounded authored input for a decorative GPU-particle layer.
///
/// Particle layers deliberately cannot declare a readability class. They are always decorative;
/// mesh layers remain responsible for geometry, identity, timing, and fallbacks.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct VfxParticleLayer {
    pub(crate) id: String,
    pub(crate) profile: VfxParticleProfile,
    pub(crate) capacity: u16,
    pub(crate) spawn: VfxParticleSpawn,
    pub(crate) lifetime_millis: u16,
    pub(crate) spawn_radius: f32,
    pub(crate) speed_min: f32,
    pub(crate) speed_max: f32,
    pub(crate) size_start: f32,
    pub(crate) size_end: f32,
    pub(crate) anchor_height: f32,
}

impl VfxParticleLayer {
    pub(super) fn validate(&self) -> Result<(), String> {
        let finite_world_value =
            |value: f32| value.is_finite() && (0.0..=MAX_PARTICLE_WORLD_VALUE).contains(&value);
        if !valid_vfx_catalog_id(&self.id)
            || self.capacity == 0
            || self.capacity > MAX_VFX_PARTICLES_PER_LAYER
            || self.spawn.amount() == 0
            || self.spawn.amount() > self.capacity
            || self.lifetime_millis == 0
            || u32::from(self.lifetime_millis) > MAX_VFX_LIFETIME_MILLIS
            || !finite_world_value(self.spawn_radius)
            || !finite_world_value(self.speed_min)
            || !finite_world_value(self.speed_max)
            || self.speed_min > self.speed_max
            || !finite_world_value(self.size_start)
            || !finite_world_value(self.size_end)
            || self.size_start <= 0.0
            || !finite_world_value(self.anchor_height)
        {
            return Err(format!(
                "VFX particle layer {} has unsafe runtime values",
                self.id
            ));
        }
        Ok(())
    }

    pub(crate) const fn lifetime(&self) -> Duration {
        Duration::from_millis(self.lifetime_millis as u64)
    }
}

impl VfxParticleSpawn {
    pub(crate) const fn amount(self) -> u16 {
        match self {
            Self::Burst(amount) | Self::RatePerSecond(amount) => amount,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct VfxRecipe {
    pub(crate) id: String,
    pub(crate) layers: Vec<String>,
    #[serde(default)]
    pub(crate) particle_layers: Vec<String>,
    pub(crate) concurrency_cap: usize,
    pub(crate) fallback_recipe: String,
    pub(crate) reduced_recipe: Option<String>,
}

impl VfxRecipe {
    pub(super) fn validate(&self) -> Result<(), String> {
        if !valid_vfx_catalog_id(&self.id)
            || !valid_vfx_catalog_id(&self.fallback_recipe)
            || self.layers.is_empty()
            || self.layers.len() > MAX_VFX_LAYERS_PER_RECIPE
            || self.particle_layers.len() > MAX_VFX_PARTICLE_LAYERS_PER_RECIPE
            || self.layers.len().saturating_add(self.particle_layers.len())
                > MAX_VFX_LAYERS_PER_RECIPE
            || !(1..=MAX_VFX_CONCURRENCY).contains(&self.concurrency_cap)
            || self.layers.iter().any(|layer| !valid_vfx_catalog_id(layer))
            || self
                .particle_layers
                .iter()
                .any(|layer| !valid_vfx_catalog_id(layer))
            || self
                .reduced_recipe
                .as_deref()
                .is_some_and(|recipe| !valid_vfx_catalog_id(recipe))
        {
            return Err(format!("VFX recipe {} has unsafe runtime values", self.id));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub(crate) struct ResolvedVfxRecipe {
    pub(crate) id: String,
    pub(crate) layers: Vec<VfxMeshLayer>,
    pub(crate) particle_layers: Vec<VfxParticleLayer>,
    pub(crate) concurrency_cap: usize,
    pub(crate) fallback_recipe: String,
    pub(crate) reduced_recipe: Option<String>,
}

pub(super) fn valid_vfx_catalog_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_VFX_ID_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
}

/// Counter-based deterministic sample for authored decorative variation.
///
/// Each property receives its own salt, so adding one sampled property cannot perturb another.
pub(crate) fn vfx_counter_sample(
    presentation_identity: u64,
    recipe_id: &str,
    layer_id: &str,
    occurrence: u8,
    property_salt: u32,
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in presentation_identity
        .to_le_bytes()
        .into_iter()
        .chain(recipe_id.bytes())
        .chain([0xff])
        .chain(layer_id.bytes())
        .chain([occurrence])
        .chain(property_salt.to_le_bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_keyframes_are_deterministic_and_bounded() {
        let frames = VfxScaleKeyframes {
            start: 0.5,
            peak_at_basis_points: 5_000,
            peak: 1.5,
            end: 1.0,
        };
        assert!(frames.validate());
        assert!((frames.sample(0.0, VfxEasing::Linear) - 0.5).abs() < f32::EPSILON);
        assert!((frames.sample(0.5, VfxEasing::Linear) - 1.5).abs() < f32::EPSILON);
        assert!((frames.sample(1.0, VfxEasing::Linear) - 1.0).abs() < f32::EPSILON);
        assert_eq!(
            frames.sample(0.25, VfxEasing::SmoothStep).to_bits(),
            frames.sample(0.25, VfxEasing::SmoothStep).to_bits()
        );
    }

    #[test]
    fn timeline_rejects_unbounded_total_duration() {
        let timeline = VfxTimeline {
            delay_millis: 1,
            duration: VfxLifetime::Millis(MAX_VFX_LIFETIME_MILLIS),
            easing: VfxEasing::Linear,
            scale: VfxScaleKeyframes {
                start: 1.0,
                peak_at_basis_points: 5_000,
                peak: 1.0,
                end: 1.0,
            },
        };
        assert!(!timeline.validate());
    }

    #[test]
    fn counter_samples_are_stable_and_property_local() {
        let first = vfx_counter_sample(7, "impact", "flash", 0, 11);
        assert_eq!(first, vfx_counter_sample(7, "impact", "flash", 0, 11));
        assert_ne!(first, vfx_counter_sample(8, "impact", "flash", 0, 11));
        assert_ne!(first, vfx_counter_sample(7, "impact", "flash", 0, 12));
    }

    #[test]
    fn particle_layers_are_decorative_and_strictly_bounded() {
        let mut layer = VfxParticleLayer {
            id: "cold_fragments".to_string(),
            profile: VfxParticleProfile::ColdFragments,
            capacity: 64,
            spawn: VfxParticleSpawn::Burst(24),
            lifetime_millis: 600,
            spawn_radius: 4.0,
            speed_min: 12.0,
            speed_max: 36.0,
            size_start: 3.0,
            size_end: 0.0,
            anchor_height: 5.0,
        };
        assert!(layer.validate().is_ok());
        layer.spawn = VfxParticleSpawn::Burst(65);
        assert!(layer.validate().is_err());
        layer.spawn = VfxParticleSpawn::RatePerSecond(24);
        assert!(layer.validate().is_ok());
        layer.capacity = MAX_VFX_PARTICLES_PER_LAYER + 1;
        assert!(layer.validate().is_err());
        layer.capacity = 64;
        layer.speed_min = 40.0;
        layer.speed_max = 20.0;
        assert!(layer.validate().is_err());
    }
}
