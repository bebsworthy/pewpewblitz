//! Client-only 3D combat presentation composed from focused visual lifecycles.

mod aim_preview;
mod common;
mod concealment_style;
mod cone_spray;
mod dash_style;
mod effects;
mod elemental_style;
mod entities;
mod fighter_feedback;
mod fighter_ui;
mod melee_style;
mod particles;
mod sentry_style;
mod splash_style;
mod status_style;
mod sticky_blob;
mod straight_projectile_style;

use bevy::prelude::{App, IntoScheduleConfigs, Plugin, PostUpdate, SystemSet, Update};

pub(super) use aim_preview::{reconcile_aim_preview_visuals, update_aim_preview};
pub(super) use concealment_style::animate_concealment_style;
pub(super) use cone_spray::{
    ConeSprayMaterial, ConeSprayMaterialPlugin, animate_cone_spray_materials,
    reconcile_cone_spray_visuals, update_cone_spray_visuals,
};
pub(super) use dash_style::DashTrailMaterialPlugin;
pub(super) use effects::animate_attack_acceptance;
pub(in crate::client) use effects::{
    CombatEffect3d, PendingCombatEffect, VfxTransientDiagnostics, animate_combat_effect_layers,
    cleanup_combat_effects, collect_vfx_evidence_snapshot, materialize_combat_effects,
    resolve_vfx_requests,
};
pub(super) use elemental_style::{
    ElementalSupportAreaMaterial, ElementalSupportAreaMaterialPlugin, ElementalTeamAccent,
    FireAreaMaterial, FireAreaMaterialPlugin, animate_elemental_motion_accents,
    animate_elemental_support_area_materials, animate_fire_area_material,
};
pub(super) use entities::{
    reconcile_concealment_field_visuals, reconcile_elemental_field_visuals,
    reconcile_fighter_visuals, reconcile_persistent_splash_visuals, reconcile_projectile_visuals,
    reconcile_sentry_visuals, reconcile_sticky_blob_visuals, update_sticky_blob_fuse_progress,
    write_fighter_visual_poses, write_lobbed_projectile_aid_poses,
    write_lobbed_projectile_body_poses, write_projectile_visual_poses, write_sentry_visual_poses,
    write_sticky_blob_visual_poses,
};
pub(super) use fighter_feedback::{
    ConcealedMaterialVariants, reconcile_dash_trails, reconcile_status_visuals,
    update_fighter_concealment_visuals, write_status_visual_poses,
};
pub(super) use fighter_ui::{
    prepare_cold_pie_assets, project_fighter_overhead_ui, reconcile_fighter_overheads,
    update_fighter_overhead_state,
};
pub(super) use melee_style::MeleeArcMeshCache;
pub(in crate::client) use particles::{
    MAX_ACTIVE_PARTICLE_EMITTERS, PendingVfxParticleLayer, RetainedProjectileSmoke3d,
    VfxParticleAdapterPlugin, VfxParticleAssets, VfxParticleDiagnostics, VfxParticleEmitter3d,
};
pub(super) use sentry_style::animate_sentry_style;
pub(super) use splash_style::{
    SplashAreaMaterial, SplashAreaMaterialPlugin, animate_splash_area_materials,
    animate_splash_motion_accents,
};
pub(super) use status_style::animate_status_style;
pub(super) use sticky_blob::{StickyBlobMaterialPlugin, animate_sticky_blob_style};
pub(super) use straight_projectile_style::{
    StraightProjectileTrailMaterial, StraightProjectileTrailMaterialPlugin, TrailQuality,
    animate_straight_projectile_trail_materials,
};

/// Schedule seam used only by the offline `VfxLab` to run production lifecycle materializers.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::client) struct VfxLabLifecycleSet;

/// Installs the production materializers needed by isolated lifecycle fixtures.
///
/// The Lab supplies replicated-shape fixture facts but never duplicates their visual construction.
pub(in crate::client) struct VfxLabLifecyclePresentationPlugin;

pub(super) fn add_retained_material_animation_systems<S>(app: &mut App, set: S)
where
    S: SystemSet + Clone,
{
    app.add_systems(
        Update,
        (
            animate_fire_area_material,
            animate_concealment_style,
            animate_elemental_support_area_materials,
            animate_cone_spray_materials,
            animate_straight_projectile_trail_materials,
            animate_sticky_blob_style,
            animate_splash_area_materials,
            animate_splash_motion_accents,
            animate_sentry_style,
            animate_status_style,
        )
            .in_set(set),
    );
}

impl Plugin for VfxLabLifecyclePresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            FireAreaMaterialPlugin,
            ElementalSupportAreaMaterialPlugin,
            ConeSprayMaterialPlugin,
            DashTrailMaterialPlugin,
            StraightProjectileTrailMaterialPlugin,
            StickyBlobMaterialPlugin,
            SplashAreaMaterialPlugin,
        ))
        .init_resource::<MeleeArcMeshCache>()
        .init_resource::<crate::combat::client::AimTraceBlockerIndex>()
        .add_systems(
            Update,
            (
                reconcile_fighter_visuals,
                reconcile_projectile_visuals,
                reconcile_sentry_visuals,
                reconcile_concealment_field_visuals,
                reconcile_elemental_field_visuals,
                reconcile_persistent_splash_visuals,
                reconcile_cone_spray_visuals,
                reconcile_status_visuals,
                reconcile_dash_trails,
                reconcile_sticky_blob_visuals,
                update_cone_spray_visuals,
                update_sticky_blob_fuse_progress,
                animate_elemental_motion_accents,
            )
                .chain()
                .in_set(VfxLabLifecycleSet),
        )
        .add_systems(
            PostUpdate,
            (
                write_fighter_visual_poses,
                (
                    write_projectile_visual_poses,
                    write_lobbed_projectile_body_poses,
                    write_lobbed_projectile_aid_poses,
                    write_sentry_visual_poses,
                    write_sticky_blob_visual_poses,
                )
                    .chain(),
            ),
        );
        add_retained_material_animation_systems(app, VfxLabLifecycleSet);
    }
}
#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;

    #[test]
    fn focused_combat_visual_state_queries_are_runtime_disjoint() {
        let mut schedule = Schedule::default();
        schedule.add_systems((
            update_fighter_overhead_state,
            reconcile_status_visuals,
            reconcile_dash_trails,
            update_aim_preview,
        ));
        schedule.initialize(&mut World::new()).unwrap();
    }
}
