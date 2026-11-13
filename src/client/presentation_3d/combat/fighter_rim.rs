//! Relationship-tinted toon rim on fighter bodies (BRL-0150).
//!
//! Every toon mesh under a fighter visual root carries the observer-relative relation of its ground
//! ring as a `MeshTag`; the toon shader picks the rim colour from it. Shared glTF materials stay
//! shared, and a relation change is only a tag write.

use super::super::*;
use super::common::{GroundMarkerRelation, ground_marker_relation};
use bevy::mesh::MeshTag;

const fn rim_relation(relation: GroundMarkerRelation) -> RimRelation {
    match relation {
        GroundMarkerRelation::Local => RimRelation::Local,
        GroundMarkerRelation::Ally => RimRelation::Ally,
        GroundMarkerRelation::Enemy => RimRelation::Enemy,
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are passed by value"
)]
pub(in super::super) fn tag_fighter_rim_relation(
    mut commands: Commands,
    roots: Query<(Entity, &CombatVisualOwner), With<V3FighterVisual>>,
    children: Query<&Children>,
    fighters: Query<(&crate::combat::TeamId, Has<Controlled>), With<Fighter>>,
    meshes: Query<Option<&MeshTag>, With<MeshMaterial3d<ToonMaterial>>>,
) {
    let controlled_team = fighters
        .iter()
        .find_map(|(team, controlled)| controlled.then_some(*team));
    for (root, owner) in &roots {
        let Ok((team, controlled)) = fighters.get(owner.0) else {
            continue;
        };
        let desired =
            rim_relation(ground_marker_relation(*team, controlled, controlled_team)).tag();
        for entity in children.iter_descendants(root) {
            if let Ok(tag) = meshes.get(entity)
                && tag != Some(&desired)
            {
                commands.entity(entity).try_insert(desired.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fighter_app() -> (App, Entity, Entity, Entity) {
        let mut app = App::new();
        app.init_resource::<Assets<ToonMaterial>>()
            .add_systems(Update, tag_fighter_rim_relation);
        let shading = ToonShading::embedded().unwrap();
        let toon = app
            .world_mut()
            .resource_mut::<Assets<ToonMaterial>>()
            .add(shading.material(StandardMaterial::default()));
        let fighter = app
            .world_mut()
            .spawn((Fighter, crate::combat::TeamId(0)))
            .id();
        let root = app
            .world_mut()
            .spawn((
                CombatVisualOwner(fighter),
                V3FighterVisual {
                    last_position: Vec2::ZERO,
                    moving: false,
                    shoot_seconds: 0.0,
                    recoil_seconds: 0.0,
                },
            ))
            .id();
        let body = app
            .world_mut()
            .spawn((MeshMaterial3d(toon), ChildOf(root)))
            .id();
        (app, fighter, root, body)
    }

    #[test]
    fn fighter_toon_meshes_carry_the_observer_relative_rim_relation() {
        let (mut app, fighter, root, body) = fighter_app();
        let ring = app
            .world_mut()
            .spawn((
                MeshMaterial3d::<StandardMaterial>(Handle::default()),
                ChildOf(root),
            ))
            .id();
        let local = app
            .world_mut()
            .spawn((Fighter, crate::combat::TeamId(1), Controlled))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<MeshTag>(body),
            Some(&RimRelation::Enemy.tag())
        );
        assert!(
            app.world().get::<MeshTag>(ring).is_none(),
            "only toon meshes are tagged"
        );

        // The observer's team changes: the same body is now an ally, with no material change.
        app.world_mut()
            .entity_mut(local)
            .insert(crate::combat::TeamId(0));
        app.update();
        assert_eq!(
            app.world().get::<MeshTag>(body),
            Some(&RimRelation::Ally.tag())
        );

        app.world_mut().entity_mut(fighter).insert(Controlled);
        app.world_mut().entity_mut(local).remove::<Controlled>();
        app.update();
        assert_eq!(
            app.world().get::<MeshTag>(body),
            Some(&RimRelation::Local.tag())
        );
    }
}
