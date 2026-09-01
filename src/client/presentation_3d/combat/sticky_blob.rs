//! Fuse-driven retained presentation for armed Sticky Blomb and Big Blob payloads.

use super::super::*;
use bevy::{
    ecs::{hierarchy::ChildSpawnerCommands, system::SystemParam},
    reflect::TypePath,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

const STICKY_BLOB_SHADER_PATH: &str = "shaders/sticky_blob.wgsl";

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(in crate::client) struct StickyBlobMaterial {
    #[uniform(0)]
    params: Vec4,
}

impl StickyBlobMaterial {
    fn new(quality: StickyBlobQuality, seed: f32) -> Self {
        Self {
            params: Vec4::new(0.0, 0.02, quality.shader_index(), seed),
        }
    }
}

impl Material for StickyBlobMaterial {
    fn fragment_shader() -> ShaderRef {
        STICKY_BLOB_SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        6.0
    }

    fn enable_shadows() -> bool {
        false
    }
}

pub(in crate::client) struct StickyBlobMaterialPlugin;

impl Plugin for StickyBlobMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<StickyBlobMaterial>::default());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StickyBlobQuality {
    Normal,
    Reduced,
}

impl StickyBlobQuality {
    const fn shader_index(self) -> f32 {
        match self {
            Self::Normal => 0.0,
            Self::Reduced => 1.0,
        }
    }

    const fn droplet_count(self) -> u8 {
        match self {
            Self::Normal => 4,
            Self::Reduced => 2,
        }
    }
}

#[derive(Component)]
pub(in super::super) struct StickyBlobSurfaceVisual3d {
    owner: Entity,
}

#[derive(Component)]
pub(in super::super) struct StickyBlobDropletVisual3d {
    owner: Entity,
    index: u8,
    blob_scale: f32,
}

#[derive(SystemParam)]
pub(in super::super) struct StickyBlobStyleContext<'w, 's> {
    materials: ResMut<'w, Assets<StickyBlobMaterial>>,
    surfaces: Query<
        'w,
        's,
        (
            &'static StickyBlobSurfaceVisual3d,
            &'static MeshMaterial3d<StickyBlobMaterial>,
        ),
    >,
}

impl StickyBlobStyleContext<'_, '_> {
    pub(super) fn spawn(
        &mut self,
        parent: &mut ChildSpawnerCommands,
        primitives: &Primitive3dAssets,
        materials: &Material3dAssets,
        owner: Entity,
        signature: vfx::CombatVisualSignature,
        blob_scale: f32,
    ) {
        spawn_sticky_blob_style(
            parent,
            primitives,
            materials,
            &mut self.materials,
            owner,
            signature,
            blob_scale,
        );
    }

    pub(super) fn remove_owner(&mut self, owner: Entity) {
        let handles = self
            .surfaces
            .iter()
            .filter(|(surface, _)| surface.owner == owner)
            .map(|(_, handle)| handle.0.id())
            .collect::<Vec<_>>();
        for handle in handles {
            self.materials.remove(handle);
        }
    }
}

fn sticky_blob_quality(signature: vfx::CombatVisualSignature) -> Option<StickyBlobQuality> {
    if signature.carrier != vfx::VfxCarrier::StickyPayload {
        return None;
    }
    match signature.variant {
        vfx::VfxPresentationVariant::Normal => Some(StickyBlobQuality::Normal),
        vfx::VfxPresentationVariant::Reduced => Some(StickyBlobQuality::Reduced),
        vfx::VfxPresentationVariant::Primitive | vfx::VfxPresentationVariant::ReducedPrimitive => {
            None
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the retained helper receives one complete immutable sticky-payload visual profile"
)]
pub(super) fn spawn_sticky_blob_style(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    sticky_materials: &mut Assets<StickyBlobMaterial>,
    owner: Entity,
    signature: vfx::CombatVisualSignature,
    blob_scale: f32,
) {
    let Some(quality) = sticky_blob_quality(signature) else {
        return;
    };
    let identity_seed = signature.stable_seed();
    let seed = f32::from(u16::try_from(identity_seed & u64::from(u16::MAX)).unwrap_or_default())
        / f32::from(u16::MAX);
    let shell = sticky_materials.add(StickyBlobMaterial::new(quality, seed));
    parent.spawn((
        StickyBlobSurfaceVisual3d { owner },
        Mesh3d(primitives.effect_sphere.clone()),
        MeshMaterial3d(shell),
        NotShadowCaster,
        NotShadowReceiver,
        Transform::from_xyz(0.0, blob_scale, 0.0).with_scale(Vec3::splat(blob_scale * 1.08)),
        Name::new("Sticky payload animated viscous shell"),
    ));

    for index in 0..quality.droplet_count() {
        parent.spawn((
            StickyBlobDropletVisual3d {
                owner,
                index,
                blob_scale,
            },
            Mesh3d(primitives.effect_sphere.clone()),
            MeshMaterial3d(materials.sticky_blob.clone()),
            NotShadowCaster,
            NotShadowReceiver,
            Transform::default(),
            Name::new("Sticky payload orbiting viscous droplet"),
        ));
    }
}

#[allow(clippy::needless_pass_by_value)]
pub(in super::super) fn animate_sticky_blob_style(
    ticks: Query<&AuthoritativeTick>,
    blobs: Query<&crate::combat::StickyBlobState>,
    surfaces: Query<(
        &StickyBlobSurfaceVisual3d,
        &MeshMaterial3d<StickyBlobMaterial>,
    )>,
    mut droplets: Query<(&StickyBlobDropletVisual3d, &mut Transform)>,
    materials: Option<ResMut<Assets<StickyBlobMaterial>>>,
) {
    let Some(mut materials) = materials else {
        return;
    };
    let tick = ticks.iter().map(|tick| tick.0).max().unwrap_or_default();
    let cycle_ticks = crate::timing::SIMULATION_TICK_HZ * 900;
    let visual_tick =
        u16::try_from(tick % cycle_ticks).expect("the fifteen-minute sticky visual cycle fits u16");
    let tick_hz = u16::try_from(crate::timing::SIMULATION_TICK_HZ)
        .expect("the simulation tick rate fits u16");
    let time = f32::from(visual_tick) / f32::from(tick_hz);

    for (surface, handle) in &surfaces {
        let Ok(state) = blobs.get(surface.owner) else {
            continue;
        };
        if let Some(mut material) = materials.get_mut(handle) {
            material.params.x = time;
            material.params.y = super::entities::sticky_fuse_progress(*state, tick);
        }
    }

    for (droplet, mut transform) in &mut droplets {
        let Ok(state) = blobs.get(droplet.owner) else {
            continue;
        };
        let progress = super::entities::sticky_fuse_progress(*state, tick);
        let index = f32::from(droplet.index);
        let phase = time.mul_add(1.9 + progress * 2.4 + index * 0.17, index * 1.57);
        let orbit = droplet.blob_scale * (0.72 + index * 0.08);
        let pulse = (phase * 1.7).sin().mul_add(0.12, 0.33) * (0.8 + progress * 0.35);
        transform.translation = Vec3::new(
            phase.cos() * orbit,
            droplet.blob_scale * (0.72 + (phase * 1.3).sin() * 0.28),
            phase.sin() * orbit,
        );
        transform.scale = Vec3::splat(droplet.blob_scale * pulse);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_selection_preserves_the_primitive_fallback() {
        let mut signature = vfx::CombatVisualSignature::neutral_point(1, Vec2::ZERO);
        signature.carrier = vfx::VfxCarrier::StickyPayload;
        assert_eq!(
            sticky_blob_quality(signature),
            Some(StickyBlobQuality::Normal)
        );
        signature.variant = vfx::VfxPresentationVariant::Reduced;
        assert_eq!(
            sticky_blob_quality(signature),
            Some(StickyBlobQuality::Reduced)
        );
        signature.variant = vfx::VfxPresentationVariant::Primitive;
        assert_eq!(sticky_blob_quality(signature), None);
    }

    #[test]
    fn production_retained_material_schedule_advances_fuse_driven_surface() {
        let mut app = App::new();
        app.init_resource::<Assets<StickyBlobMaterial>>()
            .init_resource::<Assets<super::super::elemental_style::FireAreaMaterial>>()
            .init_resource::<Assets<super::super::elemental_style::ElementalSupportAreaMaterial>>()
            .init_resource::<Assets<super::super::cone_spray::ConeSprayMaterial>>()
            .init_resource::<
                Assets<super::super::straight_projectile_style::StraightProjectileTrailMaterial>,
            >();
        configure_world_presentation_schedule(&mut app);
        super::super::add_retained_material_animation_systems(
            &mut app,
            WorldPresentationSet::Animate,
        );
        let owner = app
            .world_mut()
            .spawn(crate::combat::StickyBlobState {
                kind: crate::combat::StickyBlobKind::Primary,
                attached_to: None,
                armed_at_tick: 0,
                detonates_at_tick: crate::timing::SIMULATION_TICK_HZ * 2,
                explosion_radius: 72.0,
            })
            .id();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<StickyBlobMaterial>>()
            .add(StickyBlobMaterial::new(StickyBlobQuality::Normal, 0.5));
        app.world_mut().spawn((
            StickyBlobSurfaceVisual3d { owner },
            MeshMaterial3d(handle.clone()),
        ));
        app.world_mut()
            .spawn(AuthoritativeTick(crate::timing::SIMULATION_TICK_HZ));

        app.update();

        let material = app
            .world()
            .resource::<Assets<StickyBlobMaterial>>()
            .get(&handle)
            .expect("production animation retains the live sticky material");
        assert!((material.params.x - 1.0).abs() < f32::EPSILON);
        assert!((material.params.y - 0.5).abs() < f32::EPSILON);
    }
}
