//! Presentation quality tiers (BRL-0161).
//!
//! The player's [`GraphicsQuality`] preference resolves to one [`QualityTier`]; `Auto` picks from
//! the capability detected once the render device exists. Each tier's contents are authored in
//! `assets/catalogs/render_quality.ron`: MSAA, HDR, bloom, arena shadow maps, foliage sway, floor
//! decals, and the particle backend. Code owns the ceilings (MSAA at most 4 samples, bloom needs HDR). Everything
//! here is client presentation: the server never sees a tier and gameplay facts never depend on it.

#[allow(
    clippy::wildcard_imports,
    reason = "the world-renderer submodule consumes the presentation_3d composition surface"
)]
use super::*;
use crate::settings::persistence::GraphicsQuality;
use bevy::{
    camera::Hdr,
    render::{
        render_resource::DownlevelFlags,
        renderer::{RenderAdapter, RenderAdapterInfo, RenderDevice},
        settings::Backends,
    },
};
use serde::Deserialize;

const RENDER_QUALITY_CATALOG: &str = include_str!("../../../../assets/catalogs/render_quality.ron");
const RENDER_QUALITY_SCHEMA_VERSION: u16 = 2;
/// Desktop verification override (like `BRAWLER_FORCE_PRIMITIVE_WORLD`): `1` treats the GPU as
/// unable to run compute particles, so Hanabi is never installed and the CPU fallback is used.
pub(crate) const PARTICLE_FALLBACK_ENV: &str = "BRAWLER_FORCE_PARTICLE_FALLBACK";
/// Hanabi binds four groups (sim params, particles, spawner, metadata).
const MIN_PARTICLE_BIND_GROUPS: u32 = 4;
/// The WebGPU default; Hanabi's update and render passes read several storage buffers per stage.
const MIN_PARTICLE_STORAGE_BUFFERS_PER_STAGE: u32 = 8;
/// Downlevel features Hanabi's compute simulation and indirect, storage-fed rendering need.
const PARTICLE_DOWNLEVEL_FLAGS: DownlevelFlags = DownlevelFlags::COMPUTE_SHADERS
    .union(DownlevelFlags::VERTEX_STORAGE)
    .union(DownlevelFlags::INDIRECT_EXECUTION);

/// A resolved presentation tier.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QualityTier {
    Low,
    Medium,
    High,
}

impl QualityTier {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
        }
    }
}

/// MSAA sample counts a tier may author; 4 is the code-owned ceiling.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MsaaSamples {
    Off,
    Sample2,
    Sample4,
}

impl MsaaSamples {
    const fn msaa(self) -> Msaa {
        match self {
            Self::Off => Msaa::Off,
            Self::Sample2 => Msaa::Sample2,
            Self::Sample4 => Msaa::Sample4,
        }
    }
}

/// Where decorative particle layers are simulated.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParticleBackend {
    /// Hanabi compute particles (needs a compute-capable GPU).
    Gpu,
    /// The bounded non-compute mesh-particle fallback.
    Cpu,
}

/// One tier's authored contents.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each flag is one independent authored renderer feature of the tier"
)]
pub(crate) struct QualityProfile {
    pub(crate) msaa: MsaaSamples,
    pub(crate) hdr: bool,
    pub(crate) bloom: bool,
    pub(crate) shadow_maps: bool,
    pub(crate) foliage_sway: bool,
    /// Sparse wall-adjacent floor decals (BRL-0178).
    pub(crate) floor_decals: bool,
    pub(crate) particles: ParticleBackend,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct RenderQualityCatalogSource {
    schema_version: u16,
    high: QualityProfile,
    medium: QualityProfile,
    low: QualityProfile,
}

/// Validated tier contents, embedded from `assets/catalogs/render_quality.ron`.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RenderQualityCatalog {
    high: QualityProfile,
    medium: QualityProfile,
    low: QualityProfile,
}

impl RenderQualityCatalog {
    pub(crate) fn embedded() -> Result<Self, String> {
        Self::from_ron(RENDER_QUALITY_CATALOG)
    }

    fn from_ron(source: &str) -> Result<Self, String> {
        let source: RenderQualityCatalogSource = ron::from_str(source)
            .map_err(|error| format!("render quality catalog parse failed: {error}"))?;
        if source.schema_version != RENDER_QUALITY_SCHEMA_VERSION {
            return Err(format!(
                "render quality catalog schema {} is not {RENDER_QUALITY_SCHEMA_VERSION}",
                source.schema_version
            ));
        }
        for (name, profile) in [
            ("high", source.high),
            ("medium", source.medium),
            ("low", source.low),
        ] {
            if profile.bloom && !profile.hdr {
                return Err(format!(
                    "render quality tier {name} enables bloom without HDR"
                ));
            }
        }
        Ok(Self {
            high: source.high,
            medium: source.medium,
            low: source.low,
        })
    }

    pub(crate) const fn profile(&self, tier: QualityTier) -> QualityProfile {
        match tier {
            QualityTier::Low => self.low,
            QualityTier::Medium => self.medium,
            QualityTier::High => self.high,
        }
    }
}

/// The GPU facts the compute-particle decision reads, separated from wgpu objects for tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GpuFacts {
    pub(crate) gl_backend: bool,
    pub(crate) downlevel: DownlevelFlags,
    pub(crate) max_bind_groups: u32,
    pub(crate) max_storage_buffers_per_shader_stage: u32,
}

/// Whether Hanabi's compute particles can run on this GPU.
pub(crate) fn gpu_particles_supported(facts: GpuFacts) -> bool {
    !facts.gl_backend
        && facts.downlevel.contains(PARTICLE_DOWNLEVEL_FLAGS)
        && facts.max_bind_groups >= MIN_PARTICLE_BIND_GROUPS
        && facts.max_storage_buffers_per_shader_stage >= MIN_PARTICLE_STORAGE_BUFFERS_PER_STAGE
}

/// Capability detected once after the render device exists.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RenderCapabilities {
    pub(crate) gpu_particles: bool,
    pub(crate) mobile: bool,
}

impl RenderCapabilities {
    /// Reads the main-world render resources `RenderPlugin::finish` inserts. Without a render
    /// device (headless tests) nothing GPU-side is available.
    pub(crate) fn detect(world: &World) -> Self {
        let forced_fallback = env::var(PARTICLE_FALLBACK_ENV).is_ok_and(|value| value == "1");
        let facts = match (
            world.get_resource::<RenderDevice>(),
            world.get_resource::<RenderAdapter>(),
            world.get_resource::<RenderAdapterInfo>(),
        ) {
            (Some(device), Some(adapter), Some(info)) => {
                let limits = device.limits();
                Some(GpuFacts {
                    gl_backend: Backends::from(info.backend) == Backends::GL,
                    downlevel: adapter.get_downlevel_capabilities().flags,
                    max_bind_groups: limits.max_bind_groups,
                    max_storage_buffers_per_shader_stage: limits
                        .max_storage_buffers_per_shader_stage,
                })
            }
            _ => None,
        };
        Self {
            gpu_particles: !forced_fallback && facts.is_some_and(gpu_particles_supported),
            mobile: cfg!(target_os = "android"),
        }
    }

    /// The tier `Auto` resolves to, with the reason that is logged.
    pub(crate) const fn auto_tier(self) -> (QualityTier, &'static str) {
        if !self.gpu_particles {
            (QualityTier::Low, "no compute-capable GPU particles")
        } else if self.mobile {
            (QualityTier::Medium, "mobile GPU")
        } else {
            (QualityTier::High, "desktop GPU")
        }
    }
}

/// Resolves the player's preference against detected capability.
pub(crate) fn resolve_tier(
    preference: GraphicsQuality,
    capabilities: RenderCapabilities,
) -> (QualityTier, &'static str) {
    match preference {
        GraphicsQuality::Auto => capabilities.auto_tier(),
        GraphicsQuality::Low => (QualityTier::Low, "player override"),
        GraphicsQuality::Medium => (QualityTier::Medium, "player override"),
        GraphicsQuality::High => (QualityTier::High, "player override"),
    }
}

/// The tier currently applied to the world presentation.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ActiveRenderQuality {
    pub(crate) preference: GraphicsQuality,
    pub(crate) tier: QualityTier,
    pub(crate) profile: QualityProfile,
    gpu_particles_available: bool,
}

impl ActiveRenderQuality {
    /// GPU particles only when the tier asks for them and the device can run them.
    pub(crate) const fn particle_backend(&self) -> ParticleBackend {
        match self.profile.particles {
            ParticleBackend::Gpu if self.gpu_particles_available => ParticleBackend::Gpu,
            _ => ParticleBackend::Cpu,
        }
    }
}

/// Marks the screen-space HUD camera that shares the arena camera's render target; MSAA and HDR
/// must match between the two.
#[derive(Component)]
pub(super) struct WorldHudCamera;

/// Resolves the active tier from the preference and logs every change of selection.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are passed by value"
)]
fn select_render_quality(
    mut commands: Commands,
    settings: Option<Res<ClientShellSettings>>,
    capabilities: Option<Res<RenderCapabilities>>,
    catalog: Res<RenderQualityCatalog>,
    active: Option<Res<ActiveRenderQuality>>,
) {
    let preference = settings.map_or(GraphicsQuality::Auto, |settings| settings.graphics_quality);
    let capabilities = capabilities.map_or(
        RenderCapabilities {
            gpu_particles: false,
            mobile: cfg!(target_os = "android"),
        },
        |capabilities| *capabilities,
    );
    let (tier, reason) = resolve_tier(preference, capabilities);
    let next = ActiveRenderQuality {
        preference,
        tier,
        profile: catalog.profile(tier),
        gpu_particles_available: capabilities.gpu_particles,
    };
    if active.as_deref() == Some(&next) {
        return;
    }
    info!(
        tier = tier.label(),
        preference = preference.label(),
        reason,
        gpu_particles = capabilities.gpu_particles,
        particles = ?next.particle_backend(),
        msaa = ?next.profile.msaa,
        hdr = next.profile.hdr,
        bloom = next.profile.bloom,
        shadow_maps = next.profile.shadow_maps,
        floor_decals = next.profile.floor_decals,
        "render quality tier selected"
    );
    commands.insert_resource(next);
}

/// Keeps MSAA and HDR on the arena and HUD cameras equal to the active tier.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy system parameters are passed by value; one query covers both shared-target cameras"
)]
fn apply_camera_quality(
    mut commands: Commands,
    quality: Res<ActiveRenderQuality>,
    mut cameras: Query<
        (Entity, &mut Msaa, Has<Hdr>),
        Or<(With<ArenaCamera>, With<WorldHudCamera>)>,
    >,
) {
    let msaa = quality.profile.msaa.msaa();
    for (camera, mut current, has_hdr) in &mut cameras {
        current.set_if_neq(msaa);
        if quality.profile.hdr && !has_hdr {
            commands.entity(camera).try_insert(Hdr);
        } else if !quality.profile.hdr && has_hdr {
            commands.entity(camera).try_remove::<Hdr>();
        }
    }
}

/// Keeps the arena light's shadow maps equal to the active tier.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are passed by value"
)]
fn apply_light_quality(
    quality: Res<ActiveRenderQuality>,
    mut lights: Query<&mut DirectionalLight, With<V3WorldMember>>,
) {
    for mut light in &mut lights {
        if light.shadow_maps_enabled != quality.profile.shadow_maps {
            light.shadow_maps_enabled = quality.profile.shadow_maps;
        }
    }
}

/// Whether the active tier keeps foliage sway; absent quality keeps the authored motion.
pub(super) fn foliage_sway_enabled(quality: Option<&ActiveRenderQuality>) -> bool {
    quality.is_none_or(|quality| quality.profile.foliage_sway)
}

/// Whether the active tier keeps floor decals; absent quality keeps them.
pub(super) fn floor_decals_enabled(quality: Option<&ActiveRenderQuality>) -> bool {
    quality.is_none_or(|quality| quality.profile.floor_decals)
}

/// Whether the active tier keeps bloom; absent quality keeps the authored look.
pub(super) fn bloom_enabled(quality: Option<&ActiveRenderQuality>) -> bool {
    quality.is_none_or(|quality| quality.profile.hdr && quality.profile.bloom)
}

/// Detects capability once the render device exists and resolves the tier every frame.
pub(crate) struct RenderQualityPlugin;

impl Plugin for RenderQualityPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(
            RenderQualityCatalog::embedded()
                .expect("the embedded render quality catalog must be valid"),
        )
        .add_systems(
            Update,
            (
                select_render_quality,
                (apply_camera_quality, apply_light_quality)
                    .run_if(resource_exists::<ActiveRenderQuality>),
            )
                .chain()
                .in_set(WorldPresentationSet::PrepareAssets),
        );
    }

    fn finish(&self, app: &mut App) {
        let capabilities = RenderCapabilities::detect(app.world());
        app.insert_resource(capabilities);
    }
}

#[cfg(test)]
mod tests;
