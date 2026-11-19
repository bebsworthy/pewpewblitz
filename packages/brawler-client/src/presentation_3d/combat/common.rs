use super::super::{CombatVisualOwner, Material3dAssets};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use std::collections::HashSet;

pub(super) const GROUND_EFFECT_HEIGHT: f32 = 2.5;

/// Writes a shared animated material's visual time only when it changed. A mutable borrow marks
/// the material modified, and every modified material is re-prepared (new uniform buffer and bind
/// group) on the render thread; outside a match the authoritative tick does not advance, so these
/// materials stay untouched there (BRL-fs5k).
pub(super) fn advance_material_time<M: Asset>(
    material: Option<bevy::asset::AssetMut<'_, M>>,
    time: f32,
    time_of: fn(&mut M) -> &mut f32,
) {
    let Some(mut material) = material else {
        return;
    };
    if time_of(material.bypass_change_detection()).to_bits() != time.to_bits() {
        *time_of(&mut material) = time;
    }
}

#[derive(SystemParam)]
pub(crate) struct CombatVfxContext<'w> {
    cache: Res<'w, crate::vfx::VfxSignatureCache>,
    settings: Option<Res<'w, crate::settings::persistence::ClientShellSettings>>,
    fallback: Option<Res<'w, super::super::ImportedWorldFallbackPolicy>>,
    quality: Option<Res<'w, super::super::render_quality::ActiveRenderQuality>>,
}

impl CombatVfxContext<'_> {
    pub(super) fn cache(&self) -> &crate::vfx::VfxSignatureCache {
        &self.cache
    }

    /// The active tier's particle backend; without a tier, GPU particles when installed.
    pub(super) fn particle_backend(&self) -> super::super::render_quality::ParticleBackend {
        self.quality.as_deref().map_or(
            super::super::render_quality::ParticleBackend::Gpu,
            super::super::render_quality::ActiveRenderQuality::particle_backend,
        )
    }

    pub(super) fn variant(&self) -> crate::vfx::VfxPresentationVariant {
        crate::vfx::vfx_presentation_variant(
            self.settings
                .as_deref()
                .is_some_and(|settings| settings.reduced_combat_effects),
            self.fallback.as_deref().is_some_and(|fallback| {
                matches!(
                    fallback,
                    super::super::ImportedWorldFallbackPolicy::ForcePrimitive
                )
            }),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GroundMarkerRelation {
    Local,
    Ally,
    Enemy,
}

pub(super) fn ground_marker_relation(
    team: brawler::combat::TeamId,
    controlled: bool,
    controlled_team: Option<brawler::combat::TeamId>,
) -> GroundMarkerRelation {
    if controlled {
        GroundMarkerRelation::Local
    } else if controlled_team == Some(team) {
        GroundMarkerRelation::Ally
    } else {
        GroundMarkerRelation::Enemy
    }
}

pub(super) fn ground_marker_material(
    relation: GroundMarkerRelation,
    materials: &Material3dAssets,
) -> Handle<StandardMaterial> {
    match relation {
        GroundMarkerRelation::Local => materials.marker_local.clone(),
        GroundMarkerRelation::Ally => materials.marker_ally.clone(),
        GroundMarkerRelation::Enemy => materials.marker_enemy.clone(),
    }
}

pub(super) fn unique_roots<T: Component>(
    commands: &mut Commands,
    roots: &Query<(Entity, &CombatVisualOwner), With<T>>,
) -> HashSet<Entity> {
    let mut result = HashSet::new();
    let mut ordered = roots.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|(entity, _)| entity.index());
    for (root, owner) in ordered {
        if !result.insert(owner.0) {
            commands.entity(root).despawn();
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ground_marker_relations_are_observer_relative() {
        let local_team = Some(brawler::combat::TeamId(1));

        assert_eq!(
            ground_marker_relation(brawler::combat::TeamId(1), true, local_team),
            GroundMarkerRelation::Local
        );
        assert_eq!(
            ground_marker_relation(brawler::combat::TeamId(1), false, local_team),
            GroundMarkerRelation::Ally
        );
        assert_eq!(
            ground_marker_relation(brawler::combat::TeamId(0), false, local_team),
            GroundMarkerRelation::Enemy
        );
    }
}
