//! Combat-owned transactions for environment damage to fighters and deployables.

use avian2d::prelude::{CollisionLayers, LayerMask, Position};
use bevy::prelude::*;
use std::collections::BTreeSet;

use super::{
    ActiveEffects, AttackId, AttackSource, CombatCue, CombatLogRecord, CombatOutcomeFact,
    CombatOutcomeKind, CombatSourceKind, CombatTargetKind, CurrentHealth, DamageSource, Defeated,
    DistanceBand, NextCombatIds, WorldPoint,
};

const MAX_ENVIRONMENT_DAMAGE_TARGETS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum EnvironmentAttack {
    Neutral,
    Initiated(AttackSource),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EnvironmentProtection {
    RespectSpawnProtection,
    IgnoreSpawnProtection,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EnvironmentDamageBatch<'a> {
    pub targets: &'a [Entity],
    pub generation: crate::map::MapDynamicGeneration,
    pub placement_id: crate::map::MapPlacementId,
    pub damage: u16,
    pub tick: u64,
    pub origin: Option<Vec2>,
    pub attack: EnvironmentAttack,
    pub protection: EnvironmentProtection,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct EnvironmentDamageResult {
    pub applied_targets: usize,
    pub protected_targets: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EnvironmentDamageError {
    TooManyTargets,
    DuplicateTarget,
    IdentityExhausted,
}

#[derive(Clone, Copy)]
struct DamageTargetSnapshot {
    entity: Entity,
    position: Vec2,
    health: CurrentHealth,
    team: super::TeamId,
    network_id: crate::protocol::NetworkEntityId,
    kind: CombatTargetKind,
    protected: bool,
}

fn collect_damage_targets(
    world: &World,
    batch: &EnvironmentDamageBatch<'_>,
) -> Result<Vec<DamageTargetSnapshot>, EnvironmentDamageError> {
    if batch.targets.len() > MAX_ENVIRONMENT_DAMAGE_TARGETS {
        return Err(EnvironmentDamageError::TooManyTargets);
    }
    let mut seen = BTreeSet::new();
    let mut snapshots = Vec::with_capacity(batch.targets.len());
    for &entity in batch.targets {
        if !seen.insert(entity) {
            return Err(EnvironmentDamageError::DuplicateTarget);
        }
        let Some((&health, &team, &network_id, &position)) = world
            .get::<CurrentHealth>(entity)
            .zip(world.get::<super::TeamId>(entity))
            .zip(world.get::<crate::protocol::NetworkEntityId>(entity))
            .zip(world.get::<Position>(entity))
            .map(|(((health, team), network_id), position)| (health, team, network_id, position))
        else {
            continue;
        };
        if health.0 == 0 || world.get::<Defeated>(entity).is_some() {
            continue;
        }
        let kind = if world.get::<crate::abilities::Sentry>(entity).is_some() {
            CombatTargetKind::Deployable
        } else if world.get::<crate::protocol::Fighter>(entity).is_some() {
            CombatTargetKind::Fighter
        } else {
            continue;
        };
        let protected = matches!(
            batch.protection,
            EnvironmentProtection::RespectSpawnProtection
        ) && matches!(kind, CombatTargetKind::Fighter)
            && world
                .get::<crate::matchplay::SpawnProtection>(entity)
                .is_some();
        snapshots.push(DamageTargetSnapshot {
            entity,
            position: position.0,
            health,
            team,
            network_id,
            kind,
            protected,
        });
    }
    Ok(snapshots)
}

fn initiated_source_is_current(world: &mut World, source: AttackSource) -> bool {
    let active_match = world
        .query::<&crate::matchplay::MatchState>()
        .iter(world)
        .find_map(|state| {
            matches!(state.phase, crate::matchplay::MatchPhase::Active { .. })
                .then_some(state.match_id)
        });
    active_match.is_some_and(|match_id| {
        world
            .query_filtered::<(
                &crate::protocol::PlayerId,
                &crate::protocol::NetworkEntityId,
                &super::TeamId,
                &crate::matchplay::MatchMember,
            ), (
                With<crate::protocol::Fighter>,
                With<crate::matchplay::ActiveCombatant>,
            )>()
            .iter(world)
            .any(|(player, network_id, team, member)| {
                *player == source.player_id
                    && *network_id == source.owner_network_entity_id
                    && *team == source.team_id
                    && member.0 == match_id
            })
    })
}

fn reserve_batch_identity(
    world: &mut World,
    attack: EnvironmentAttack,
    event_count: usize,
) -> Result<(AttackId, Vec<super::CombatEventId>), EnvironmentDamageError> {
    let mut ids = world.resource_mut::<NextCombatIds>();
    match attack {
        EnvironmentAttack::Neutral => {
            super::server::reserve_attack_and_events(&mut ids, event_count)
        }
        EnvironmentAttack::Initiated(source) => {
            super::server::reserve_event_ids(&mut ids, event_count)
                .map(|events| (source.attack_id, events))
        }
    }
    .ok_or(EnvironmentDamageError::IdentityExhausted)
}

#[allow(
    clippy::too_many_lines,
    reason = "the exclusive combat transaction keeps health, lifecycle, facts, cues, and telemetry atomic"
)]
pub(crate) fn apply_environment_damage_batch(
    world: &mut World,
    batch: EnvironmentDamageBatch<'_>,
) -> Result<EnvironmentDamageResult, EnvironmentDamageError> {
    let snapshots = collect_damage_targets(world, &batch)?;
    let event_count = snapshots.iter().try_fold(0_usize, |count, target| {
        let lethal = !target.protected && batch.damage >= target.health.0;
        count.checked_add(usize::from(lethal) + 1)
    });
    let Some(event_count) = event_count else {
        return Err(EnvironmentDamageError::TooManyTargets);
    };
    if event_count == 0 {
        return Ok(EnvironmentDamageResult::default());
    }
    let (attack_id, event_ids) = reserve_batch_identity(world, batch.attack, event_count)?;
    let initiated_source = match batch.attack {
        EnvironmentAttack::Neutral => None,
        EnvironmentAttack::Initiated(source) => Some(source),
    };
    let lineage_is_current =
        initiated_source.is_some_and(|source| initiated_source_is_current(world, source));
    let generation = batch.generation;
    let damage_source = DamageSource::Environment {
        map_instance_id: generation.map_instance_id.0,
        generation: generation.generation,
        placement_id: batch.placement_id.0,
        initiating_player: initiated_source
            .filter(|_| lineage_is_current)
            .map(|source| source.player_id),
        initiating_fighter: initiated_source
            .filter(|_| lineage_is_current)
            .map(|source| source.owner_network_entity_id),
    };
    let mut result = EnvironmentDamageResult::default();
    let mut next_event = 0;

    for target in snapshots {
        let event_id = event_ids[next_event];
        next_event += 1;
        let engagement_distance = batch
            .origin
            .map_or(0.0, |origin| origin.distance(target.position));
        let hostile_credit = initiated_source.is_some_and(|source| {
            lineage_is_current
                && source.team_id != target.team
                && source.owner_network_entity_id != target.network_id
                && source.team_id.0 <= 1
        });
        let source_team = initiated_source
            .filter(|_| hostile_credit)
            .map(|source| source.team_id);
        let source_player = initiated_source
            .filter(|_| lineage_is_current)
            .map(|source| source.player_id);
        let source_network_id = initiated_source
            .filter(|_| lineage_is_current)
            .map(|source| source.owner_network_entity_id);

        if target.protected {
            result.protected_targets += 1;
            world
                .resource_mut::<super::CombatOutcomeFacts>()
                .0
                .push(CombatOutcomeFact {
                    event_id,
                    tick: batch.tick,
                    attack_id,
                    source_kind: CombatSourceKind::Environment,
                    source_player,
                    source_network_id,
                    source_team: None,
                    target_network_id: target.network_id,
                    target_kind: target.kind,
                    target_team: target.team,
                    preset_id: None,
                    recipe_fingerprint: None,
                    position: WorldPoint::from(target.position),
                    engagement_distance,
                    kind: CombatOutcomeKind::ProtectedContact,
                });
            continue;
        }

        let applied = batch.damage.min(target.health.0);
        let health_after = target.health.0 - applied;
        let defeated = health_after == 0;
        result.applied_targets += 1;
        world
            .entity_mut(target.entity)
            .insert(CurrentHealth(health_after));
        world
            .resource_mut::<super::CombatOutcomeFacts>()
            .0
            .push(CombatOutcomeFact {
                event_id,
                tick: batch.tick,
                attack_id,
                source_kind: CombatSourceKind::Environment,
                source_player,
                source_network_id,
                source_team,
                target_network_id: target.network_id,
                target_kind: target.kind,
                target_team: target.team,
                preset_id: None,
                recipe_fingerprint: None,
                position: WorldPoint::from(target.position),
                engagement_distance,
                kind: CombatOutcomeKind::Damage { amount: applied },
            });
        let cue = CombatCue::Damage {
            event_id,
            tick: batch.tick,
            source: damage_source,
            target: target.network_id,
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
            tick: batch.tick,
            event_id,
            source: damage_source,
            target: target.network_id,
            requested: batch.damage,
            applied,
            health_after,
        });
        if !defeated {
            continue;
        }

        let defeat_event = event_ids[next_event];
        next_event += 1;
        let collision_layer = match target.kind {
            CombatTargetKind::Fighter => crate::movement::FIGHTER_LAYER,
            CombatTargetKind::Deployable => crate::movement::DEPLOYABLE_LAYER,
        };
        let mut entity = world.entity_mut(target.entity);
        entity
            .insert((
                Defeated {
                    event_id: defeat_event,
                },
                CollisionLayers::new(collision_layer, LayerMask::NONE),
                ActiveEffects::default(),
            ))
            .remove::<super::ExternalMotion>()
            .remove::<super::KnockbackFeedback>();
        if matches!(target.kind, CombatTargetKind::Fighter) {
            entity.remove::<crate::map::EffectTileOccupancy>();
        }
        world
            .resource_mut::<super::CombatOutcomeFacts>()
            .0
            .push(CombatOutcomeFact {
                event_id: defeat_event,
                tick: batch.tick,
                attack_id,
                source_kind: CombatSourceKind::Environment,
                source_player,
                source_network_id,
                source_team,
                target_network_id: target.network_id,
                target_kind: target.kind,
                target_team: target.team,
                preset_id: None,
                recipe_fingerprint: None,
                position: WorldPoint::from(target.position),
                engagement_distance,
                kind: match target.kind {
                    CombatTargetKind::Fighter => CombatOutcomeKind::Defeat,
                    CombatTargetKind::Deployable => CombatOutcomeKind::DeployableDestroyed,
                },
            });
        let defeat_cue = CombatCue::Defeat {
            event_id: defeat_event,
            tick: batch.tick,
            source: Some(damage_source),
            target: target.network_id,
        };
        world
            .resource_mut::<super::CombatOutbox>()
            .0
            .push(defeat_cue.clone());
        let mut telemetry = world.resource_mut::<super::CombatTelemetry>();
        telemetry.defeats = telemetry.defeats.saturating_add(1);
        telemetry.record_cue(defeat_cue);
        telemetry.record(CombatLogRecord::Defeat {
            tick: batch.tick,
            event_id: defeat_event,
            source: Some(damage_source),
            target: target.network_id,
        });
    }
    debug_assert_eq!(next_event, event_ids.len());
    Ok(result)
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

    fn spawn_fighter(app: &mut App, network_id: u64, team: u8, health: u16) -> Entity {
        app.world_mut()
            .spawn((
                crate::protocol::Fighter,
                CurrentHealth(health),
                super::super::TeamId(team),
                crate::protocol::NetworkEntityId(network_id),
                Position(Vec2::new(
                    f32::from(u16::try_from(network_id).unwrap()),
                    0.0,
                )),
            ))
            .id()
    }

    fn neutral_batch(targets: &[Entity]) -> EnvironmentDamageBatch<'_> {
        EnvironmentDamageBatch {
            targets,
            generation: crate::map::MapDynamicGeneration {
                map_instance_id: crate::map::MapInstanceId(3),
                generation: 2,
            },
            placement_id: crate::map::MapPlacementId(17),
            damage: 10,
            tick: 30,
            origin: None,
            attack: EnvironmentAttack::Neutral,
            protection: EnvironmentProtection::RespectSpawnProtection,
        }
    }

    #[test]
    fn neutral_environment_damage_emits_damage_and_defeat_facts() {
        let mut app = app();
        let target = spawn_fighter(&mut app, 9, 1, 10);
        let result =
            apply_environment_damage_batch(app.world_mut(), neutral_batch(&[target])).unwrap();
        assert_eq!(result.applied_targets, 1);
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
        let target = spawn_fighter(&mut app, 9, 1, 20);
        app.world_mut()
            .entity_mut(target)
            .insert(crate::matchplay::SpawnProtection {
                expires_at_tick: 99,
            });
        let result =
            apply_environment_damage_batch(app.world_mut(), neutral_batch(&[target])).unwrap();
        assert_eq!(result.protected_targets, 1);
        assert_eq!(
            app.world().get::<CurrentHealth>(target),
            Some(&CurrentHealth(20))
        );
        let facts = &app.world().resource::<super::super::CombatOutcomeFacts>().0;
        assert_eq!(facts.len(), 1);
        assert!(matches!(facts[0].kind, CombatOutcomeKind::ProtectedContact));
    }

    #[test]
    fn ignored_protection_and_stale_lineage_preserve_environment_attribution() {
        let mut app = app();
        let target = spawn_fighter(&mut app, 9, 1, 20);
        app.world_mut()
            .entity_mut(target)
            .insert(crate::matchplay::SpawnProtection {
                expires_at_tick: 99,
            });
        let source = AttackSource {
            kind: CombatSourceKind::PrimaryWeapon,
            attack_id: AttackId(44),
            player_id: crate::protocol::PlayerId(7),
            owner_network_entity_id: crate::protocol::NetworkEntityId(70),
            team_id: super::super::TeamId(0),
            recipe_fingerprint: super::super::WeaponRecipeFingerprint(3),
            legacy_compatibility: false,
            source_preset_id: None,
            origin: WorldPoint::from(Vec2::ZERO),
            facing: 0.0,
        };
        let result = apply_environment_damage_batch(
            app.world_mut(),
            EnvironmentDamageBatch {
                targets: &[target],
                attack: EnvironmentAttack::Initiated(source),
                protection: EnvironmentProtection::IgnoreSpawnProtection,
                origin: Some(Vec2::ZERO),
                ..neutral_batch(&[])
            },
        )
        .unwrap();

        assert_eq!(result.applied_targets, 1);
        assert_eq!(
            app.world().get::<CurrentHealth>(target),
            Some(&CurrentHealth(10))
        );
        let fact = &app.world().resource::<super::super::CombatOutcomeFacts>().0[0];
        assert_eq!(fact.attack_id, AttackId(44));
        assert_eq!(fact.source_player, None);
        assert_eq!(fact.source_network_id, None);
        assert_eq!(fact.source_team, None);
    }

    #[test]
    fn batch_projection_preserves_planner_order_and_rejects_duplicates() {
        let mut app = app();
        let first = spawn_fighter(&mut app, 9, 1, 20);
        let second = spawn_fighter(&mut app, 10, 1, 20);
        let ordered = [second, first];
        apply_environment_damage_batch(app.world_mut(), neutral_batch(&ordered)).unwrap();
        let facts = &app.world().resource::<super::super::CombatOutcomeFacts>().0;
        assert_eq!(
            facts
                .iter()
                .map(|fact| fact.target_network_id.0)
                .collect::<Vec<_>>(),
            vec![10, 9]
        );

        let duplicate = [first, first];
        assert_eq!(
            apply_environment_damage_batch(app.world_mut(), neutral_batch(&duplicate)),
            Err(EnvironmentDamageError::DuplicateTarget)
        );
        assert_eq!(
            app.world().get::<CurrentHealth>(first),
            Some(&CurrentHealth(10))
        );
    }

    #[test]
    fn initiated_batch_credits_current_hostile_lineage_and_destroys_deployables() {
        let mut app = app();
        let source = app
            .world_mut()
            .spawn((
                crate::protocol::Fighter,
                crate::protocol::PlayerId(7),
                crate::protocol::NetworkEntityId(70),
                super::super::TeamId(0),
                crate::matchplay::MatchMember(crate::matchplay::MatchId(5)),
                crate::matchplay::ActiveCombatant,
            ))
            .id();
        let _ = source;
        app.world_mut().spawn(crate::matchplay::MatchState {
            match_id: crate::matchplay::MatchId(5),
            mode_definition_id: crate::map::ModeDefinitionId(1),
            phase: crate::matchplay::MatchPhase::Active { ends_at_tick: 99 },
            rules_revision: 1,
        });
        let target = app
            .world_mut()
            .spawn((
                crate::abilities::Sentry,
                CurrentHealth(10),
                super::super::TeamId(1),
                crate::protocol::NetworkEntityId(88),
                Position(Vec2::new(4.0, 0.0)),
            ))
            .id();
        let source = AttackSource {
            kind: CombatSourceKind::PrimaryWeapon,
            attack_id: AttackId(44),
            player_id: crate::protocol::PlayerId(7),
            owner_network_entity_id: crate::protocol::NetworkEntityId(70),
            team_id: super::super::TeamId(0),
            recipe_fingerprint: super::super::WeaponRecipeFingerprint(3),
            legacy_compatibility: false,
            source_preset_id: None,
            origin: WorldPoint::from(Vec2::ZERO),
            facing: 0.0,
        };
        let result = apply_environment_damage_batch(
            app.world_mut(),
            EnvironmentDamageBatch {
                targets: &[target],
                attack: EnvironmentAttack::Initiated(source),
                protection: EnvironmentProtection::IgnoreSpawnProtection,
                origin: Some(Vec2::ZERO),
                ..neutral_batch(&[])
            },
        )
        .unwrap();
        assert_eq!(result.applied_targets, 1);
        let facts = &app.world().resource::<super::super::CombatOutcomeFacts>().0;
        assert_eq!(facts[0].attack_id, AttackId(44));
        assert_eq!(facts[0].source_team, Some(super::super::TeamId(0)));
        assert_eq!(facts[0].target_kind, CombatTargetKind::Deployable);
        assert!(matches!(
            facts[1].kind,
            CombatOutcomeKind::DeployableDestroyed
        ));
    }

    #[test]
    fn identity_exhaustion_leaves_the_whole_batch_unchanged() {
        let mut app = app();
        let first = spawn_fighter(&mut app, 9, 1, 10);
        let second = spawn_fighter(&mut app, 10, 1, 10);
        app.world_mut()
            .resource_mut::<NextCombatIds>()
            .next_event_id = u64::MAX - 1;
        let before = *app.world().resource::<NextCombatIds>();
        assert_eq!(
            apply_environment_damage_batch(app.world_mut(), neutral_batch(&[first, second])),
            Err(EnvironmentDamageError::IdentityExhausted)
        );
        assert_eq!(*app.world().resource::<NextCombatIds>(), before);
        assert_eq!(
            app.world().get::<CurrentHealth>(first),
            Some(&CurrentHealth(10))
        );
        assert_eq!(
            app.world().get::<CurrentHealth>(second),
            Some(&CurrentHealth(10))
        );
        assert!(
            app.world()
                .resource::<super::super::CombatOutcomeFacts>()
                .0
                .is_empty()
        );
    }
}
