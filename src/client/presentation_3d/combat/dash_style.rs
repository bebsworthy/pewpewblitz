//! Retained Dash wake styling shared by production gameplay and `VfxLab`.

use super::super::*;
use bevy::{reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef};

const DASH_TRAIL_SHADER_PATH: &str = "shaders/dash_trail.wgsl";

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(in crate::client) struct DashTrailMaterial {
    #[uniform(0)]
    params: Vec4,
    #[uniform(1)]
    color: Vec4,
}

impl DashTrailMaterial {
    pub(super) const fn new(quality: DashTrailQuality) -> Self {
        Self {
            params: Vec4::new(0.0, 1.0, quality.shader_index(), 0.0),
            color: Vec4::new(0.08, 1.45, 2.6, 1.0),
        }
    }

    pub(super) fn set_time_and_opacity(&mut self, time_seconds: f32, opacity: f32) {
        self.params.x = time_seconds;
        self.params.y = opacity.clamp(0.0, 1.0);
    }
}

impl Material for DashTrailMaterial {
    fn fragment_shader() -> ShaderRef {
        DASH_TRAIL_SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        9.0
    }

    fn enable_shadows() -> bool {
        false
    }
}

pub(in crate::client) struct DashTrailMaterialPlugin;

impl Plugin for DashTrailMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<DashTrailMaterial>::default());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DashTrailQuality {
    Normal,
    Reduced,
}

impl DashTrailQuality {
    const fn shader_index(self) -> f32 {
        match self {
            Self::Normal => 0.0,
            Self::Reduced => 1.0,
        }
    }
}

pub(super) const fn quality_for_variant(
    variant: vfx::VfxPresentationVariant,
) -> Option<DashTrailQuality> {
    match variant {
        vfx::VfxPresentationVariant::Normal => Some(DashTrailQuality::Normal),
        vfx::VfxPresentationVariant::Reduced => Some(DashTrailQuality::Reduced),
        vfx::VfxPresentationVariant::Primitive | vfx::VfxPresentationVariant::ReducedPrimitive => {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dash_quality_preserves_reduced_identity_and_primitive_fallback() {
        use vfx::VfxPresentationVariant::{Normal, Primitive, Reduced, ReducedPrimitive};

        assert_eq!(quality_for_variant(Normal), Some(DashTrailQuality::Normal));
        assert_eq!(
            quality_for_variant(Reduced),
            Some(DashTrailQuality::Reduced)
        );
        assert_eq!(quality_for_variant(Primitive), None);
        assert_eq!(quality_for_variant(ReducedPrimitive), None);
    }

    #[test]
    fn dash_material_clamps_dissipation_opacity() {
        let mut material = DashTrailMaterial::new(DashTrailQuality::Normal);
        material.set_time_and_opacity(2.5, 1.4);
        assert!((material.params.x - 2.5).abs() < f32::EPSILON);
        assert!((material.params.y - 1.0).abs() < f32::EPSILON);

        material.set_time_and_opacity(3.0, -0.2);
        assert!(material.params.y.abs() < f32::EPSILON);
    }
}
