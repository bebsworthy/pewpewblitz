//! Combat-owned transaction for neutral environment damage to fighters.

use avian2d::prelude::{CollisionLayers, LayerMask, Position};
use bevy::prelude::*;

use super::{
    ActiveEffects, CombatCue, CombatLogRecord, CombatOutcomeFact, CombatOutcomeKind,
    CombatSourceKind, CombatTargetKind, CurrentHealth, DamageSource, Defeated, DistanceBand,
    NextCombatIds, WorldPoint,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct NeutralEnvironmentDamage {
    pub target: Entity,
    pub generation: crate::map::MapDynamicGeneration,
    pub placement_id: crate::map::MapPlacementId,
    pub damage: u16,
    pub tick: u64,
}

#[allow(
    clippy::too_many_lines,
    reason = "the exclusive combat transaction keeps health, lifecycle, facts, cues, and telemetry atomic"
)]
pub(crate) fn apply_neutral_environment_damage(
    world: &mut World,
    request: NeutralEnvironmentDamage,
) -> bool {
    let Some((&health, &team, &network_id, &position)) = world
        .get::<CurrentHealth>(request.target)
        .zip(world.get::<super::TeamId>(request.target))
        .zip(world.get::<crate::protocol::NetworkEntityId>(request.target))
        .zip(world.get::<Position>(request.target))
        .map(|(((health, team), network_id), position)| (health, team, network_id, position))
    else {
        return false;
    };
    if health.0 == 0 || world.get::<Defeated>(request.target).is_some() {
        return false;
    }
    let protected = world
        .get::<crate::matchplay::SpawnProtection>(request.target)
        .is_some();
    let defeated = !protected && request.damage >= health.0;
    let event_count = if defeated { 2 } else { 1 };
    let (attack_id, event_ids) = {
        let mut ids = world.resource_mut::<NextCombatIds>();
        let Some(attack_id) = ids.allocate_attack() else {
            return false;
        };
        let Some(event_ids) = super::server::reserve_event_ids(&mut ids, event_count) else {
            return false;
        };
        (attack_id, event_ids)
    };
    let source = DamageSource::Environment {
        map_instance_id: request.generation.map_instance_id.0,
        generation: request.generation.generation,
        placement_id: request.placement_id.0,
        initiating_player: None,
        initiating_fighter: None,
    };
    if protected {
        world
            .resource_mut::<super::CombatOutcomeFacts>()
            .0
            .push(CombatOutcomeFact {
                event_id: event_ids[0],
                tick: request.tick,
                attack_id,
                source_kind: CombatSourceKind::Environment,
                source_player: None,
                source_network_id: None,
                source_team: None,
                target_network_id: network_id,
                target_kind: CombatTargetKind::Fighter,
                target_team: team,
                preset_id: None,
                recipe_fingerprint: None,
                position: WorldPoint::from(position.0),
                engagement_distance: 0.0,
                kind: CombatOutcomeKind::ProtectedContact,
            });
        return false;
    }

    let applied = request.damage.min(health.0);
    let health_after = health.0 - applied;
    world
        .entity_mut(request.target)
        .insert(CurrentHealth(health_after));
    world
        .resource_mut::<super::CombatOutcomeFacts>()
        .0
        .push(CombatOutcomeFact {
            event_id: event_ids[0],
            tick: request.tick,
            attack_id,
            source_kind: CombatSourceKind::Environment,
            source_player: None,
            source_network_id: None,
            source_team: None,
            target_network_id: network_id,
            target_kind: CombatTargetKind::Fighter,
            target_team: team,
            preset_id: None,
            recipe_fingerprint: None,
            position: WorldPoint::from(position.0),
            engagement_distance: 0.0,
            kind: CombatOutcomeKind::Damage { amount: applied },
        });
    let cue = CombatCue::Damage {
        event_id: event_ids[0],
        tick: request.tick,
        source,
        target: network_id,
        amount: applied,
        health_after,
        distance_band: DistanceBand::Close,
    };
    world
        .resource_mut::<super::CombatOutbox>()
        .0
        .push(cue.clone());
    let mut telemetry = world.resource_mut::<super::CombatTelemetry>();
    telemetry.applied_damage = telemetry.applied_damage.saturating_add(u64::from(applied));
    telemetry.record_cue(cue);
    telemetry.record(CombatLogRecord::Damage {
        tick: request.tick,
        event_id: event_ids[0],
        source,
        target: network_id,
        requested: request.damage,
        applied,
        health_after,
    });
    if !defeated {
        return true;
    }

    let defeat_event = event_ids[1];
    world
        .entity_mut(request.target)
        .insert((
            Defeated {
                event_id: defeat_event,
            },
            CollisionLayers::new(crate::movement::FIGHTER_LAYER, LayerMask::NONE),
            ActiveEffects::default(),
        ))
        .remove::<super::ExternalMotion>()
        .remove::<super::KnockbackFeedback>()
        .remove::<crate::map::EffectTileOccupancy>();
    world
        .resource_mut::<super::CombatOutcomeFacts>()
        .0
        .push(CombatOutcomeFact {
            event_id: defeat_event,
            tick: request.tick,
            attack_id,
            source_kind: CombatSourceKind::Environment,
            source_player: None,
            source_network_id: None,
            source_team: None,
            target_network_id: network_id,
            target_kind: CombatTargetKind::Fighter,
            target_team: team,
            preset_id: None,
            recipe_fingerprint: None,
            position: WorldPoint::from(position.0),
            engagement_distance: 0.0,
            kind: CombatOutcomeKind::Defeat,
        });
    let defeat_cue = CombatCue::Defeat {
        event_id: defeat_event,
        tick: request.tick,
        source: Some(source),
        target: network_id,
    };
    world
        .resource_mut::<super::CombatOutbox>()
        .0
        .push(defeat_cue.clone());
    let mut telemetry = world.resource_mut::<super::CombatTelemetry>();
    telemetry.defeats = telemetry.defeats.saturating_add(1);
    telemetry.record_cue(defeat_cue);
    telemetry.record(CombatLogRecord::Defeat {
        tick: request.tick,
        event_id: defeat_event,
        source: Some(source),
        target: network_id,
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<NextCombatIds>()
            .init_resource::<super::super::CombatOutcomeFacts>()
            .init_resource::<super::super::CombatOutbox>()
            .init_resource::<super::super::CombatTelemetry>();
        app
    }

    fn spawn_target(app: &mut App, health: u16) -> Entity {
        app.world_mut()
            .spawn((
                CurrentHealth(health),
                super::super::TeamId(1),
                crate::protocol::NetworkEntityId(9),
                Position(Vec2::ZERO),
            ))
            .id()
    }

    fn request(target: Entity) -> NeutralEnvironmentDamage {
        NeutralEnvironmentDamage {
            target,
            generation: crate::map::MapDynamicGeneration {
                map_instance_id: crate::map::MapInstanceId(3),
                generation: 2,
            },
            placement_id: crate::map::MapPlacementId(17),
            damage: 10,
            tick: 30,
        }
    }

    #[test]
    fn neutral_environment_damage_emits_damage_and_defeat_facts() {
        let mut app = app();
        let target = spawn_target(&mut app, 10);
        assert!(apply_neutral_environment_damage(
            app.world_mut(),
            request(target)
        ));
        assert_eq!(
            app.world().get::<CurrentHealth>(target),
            Some(&CurrentHealth(0))
        );
        assert!(app.world().get::<Defeated>(target).is_some());
        let facts = &app.world().resource::<super::super::CombatOutcomeFacts>().0;
        assert_eq!(facts.len(), 2);
        assert!(facts.iter().all(|fact| fact.source_team.is_none()));
        assert!(matches!(
            facts[0].kind,
            CombatOutcomeKind::Damage { amount: 10 }
        ));
        assert!(matches!(facts[1].kind, CombatOutcomeKind::Defeat));
    }

    #[test]
    fn spawn_protection_blocks_neutral_environment_damage() {
        let mut app = app();
        let target = spawn_target(&mut app, 20);
        app.world_mut()
            .entity_mut(target)
            .insert(crate::matchplay::SpawnProtection {
                expires_at_tick: 99,
            });
        assert!(!apply_neutral_environment_damage(
            app.world_mut(),
            request(target)
        ));
        assert_eq!(
            app.world().get::<CurrentHealth>(target),
            Some(&CurrentHealth(20))
        );
        let facts = &app.world().resource::<super::super::CombatOutcomeFacts>().0;
        assert_eq!(facts.len(), 1);
        assert!(matches!(facts[0].kind, CombatOutcomeKind::ProtectedContact));
    }
}
