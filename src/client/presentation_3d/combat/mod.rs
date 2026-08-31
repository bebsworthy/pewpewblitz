//! Client-only 3D combat presentation composed from focused visual lifecycles.

mod aim_preview;
mod common;
mod cone_spray;
mod effects;
mod elemental_style;
mod entities;
mod fighter_feedback;
mod fighter_ui;

use bevy::prelude::{App, IntoScheduleConfigs, Plugin, PostUpdate, SystemSet, Update};

pub(super) use aim_preview::{reconcile_aim_preview_visuals, update_aim_preview};
pub(super) use cone_spray::{reconcile_cone_spray_visuals, update_cone_spray_visuals};
pub(super) use effects::animate_attack_acceptance;
pub(in crate::client) use effects::{
    CombatEffect3d, PendingCombatEffect, animate_combat_effect_layers, cleanup_combat_effects,
    collect_vfx_evidence_snapshot, materialize_combat_effects, resolve_vfx_requests,
};
pub(super) use elemental_style::animate_elemental_motion_accents;
pub(super) use entities::{
    reconcile_concealment_field_visuals, reconcile_elemental_field_visuals,
    reconcile_fighter_visuals, reconcile_persistent_splash_visuals, reconcile_projectile_visuals,
    reconcile_sentry_visuals, reconcile_sticky_blob_visuals, update_sticky_blob_fuse_progress,
    write_fighter_visual_poses, write_projectile_visual_poses, write_sentry_visual_poses,
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

/// Schedule seam used only by the offline `VfxLab` to run production lifecycle materializers.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::client) struct VfxLabLifecycleSet;

/// Installs the production materializers needed by isolated lifecycle fixtures.
///
/// The Lab supplies replicated-shape fixture facts but never duplicates their visual construction.
pub(in crate::client) struct VfxLabLifecyclePresentationPlugin;

impl Plugin for VfxLabLifecyclePresentationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::combat::client::AimTraceBlockerIndex>()
            .add_systems(
                Update,
                (
                    reconcile_fighter_visuals,
                    reconcile_projectile_visuals,
                    reconcile_elemental_field_visuals,
                    reconcile_cone_spray_visuals,
                    reconcile_status_visuals,
                    update_cone_spray_visuals,
                    animate_elemental_motion_accents,
                )
                    .chain()
                    .in_set(VfxLabLifecycleSet),
            )
            .add_systems(
                PostUpdate,
                (write_fighter_visual_poses, write_projectile_visual_poses),
            );
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
