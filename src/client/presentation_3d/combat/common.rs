use super::super::{CombatVisualOwner, Material3dAssets};
use bevy::prelude::*;
use std::collections::HashSet;

pub(super) const GROUND_EFFECT_HEIGHT: f32 = 2.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GroundMarkerRelation {
    Local,
    Ally,
    Enemy,
}

pub(super) fn ground_marker_relation(
    team: crate::combat::TeamId,
    controlled: bool,
    controlled_team: Option<crate::combat::TeamId>,
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
        let local_team = Some(crate::combat::TeamId(1));

        assert_eq!(
            ground_marker_relation(crate::combat::TeamId(1), true, local_team),
            GroundMarkerRelation::Local
        );
        assert_eq!(
            ground_marker_relation(crate::combat::TeamId(1), false, local_team),
            GroundMarkerRelation::Ally
        );
        assert_eq!(
            ground_marker_relation(crate::combat::TeamId(0), false, local_team),
            GroundMarkerRelation::Enemy
        );
    }
}
