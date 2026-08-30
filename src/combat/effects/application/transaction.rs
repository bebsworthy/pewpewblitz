//! Mutation-free composed-record planning, authoritative commit, and ordered projection.

#[allow(clippy::wildcard_imports)]
use super::*;

#[allow(
    clippy::struct_excessive_bools,
    reason = "these independent target admission and mutation facts make the transaction plan explicit; collapsing them into a state machine would obscure valid combinations"
)]
struct PlannedTargetState {
    network_id: NetworkEntityId,
    team: TeamId,
    health: u16,
    authored_maximum_health: Option<u16>,
    world_effects: ActiveEffects,
    world_motion: Option<ExternalMotion>,
    effects: ActiveEffects,
    motion: Option<ExternalMotion>,
    defeated: bool,
    target_kind: CombatTargetKind,
    match_participant: bool,
    active_combatant: bool,
    spawn_protected: bool,
    healing_blocked: bool,
    tenacity: Option<crate::builds::ResolvedPassive>,
    cold_capacity: u16,
    cold_resistance: u16,
    poison_resistance: u16,
    fire_resistance: u16,
    reset_delay_ticks: Option<u64>,
    health_changed: bool,
    commit_runtime: bool,
    defeat: Option<DefeatMutation>,
}

#[derive(Clone, Copy)]
struct DefeatMutation {
    event_id: CombatEventId,
    reset_at_tick: Option<u64>,
}

#[derive(Default)]
struct TrackerMutationPlan {
    cancelled_attacks: HashSet<AttackId>,
    hostile_contacts: HashSet<AttackId>,
    resolved_delivery_keys: HashSet<(AttackId, u8)>,
}

struct DamageProjection<'a> {
    record: &'a PendingPayload,
    tick: u64,
    source: DamageSource,
    target_network_id: NetworkEntityId,
    target_team: TeamId,
    target_kind: CombatTargetKind,
    preset_id: WeaponPresetId,
    applied_effect: (u16, DamageFalloff, RecipientPolicy),
    plan: DamageApplicationPlan,
    event_id: CombatEventId,
    legacy_event_id: Option<CombatEventId>,
    effects: ActiveEffects,
    motion: Option<ExternalMotion>,
}

struct HealingProjection<'a> {
    record: &'a PendingPayload,
    tick: u64,
    source: DamageSource,
    target_network_id: NetworkEntityId,
    target_team: TeamId,
    target_kind: CombatTargetKind,
    preset_id: WeaponPresetId,
    event_id: CombatEventId,
    effect: PayloadEffectDefinition,
    plan: HealingApplicationPlan,
    effects: ActiveEffects,
    motion: Option<ExternalMotion>,
}

struct DefeatProjection<'a> {
    record: &'a PendingPayload,
    tick: u64,
    source: DamageSource,
    target_network_id: NetworkEntityId,
    target_team: TeamId,
    target_kind: CombatTargetKind,
    preset_id: WeaponPresetId,
    owner_contact: bool,
    event_id: CombatEventId,
    legacy_event_id: Option<CombatEventId>,
}

enum ProjectionOp<'a> {
    ProtectedContact(CombatOutcomeFact),
    HostileDeliveryContact {
        preset_id: WeaponPresetId,
        recipe_fingerprint: WeaponRecipeFingerprint,
    },
    AbilityTelemetry(crate::abilities::AbilityTelemetryRecord),
    WeaponTelemetry(WeaponTelemetryRecord),
    Damage(DamageProjection<'a>),
    Healing(HealingProjection<'a>),
    Defeat(DefeatProjection<'a>),
}

pub(in crate::combat::effects) struct ComposedApplicationPlan<'a> {
    targets: HashMap<Entity, PlannedTargetState>,
    target_order: Vec<Entity>,
    tracker: TrackerMutationPlan,
    projections: Vec<ProjectionOp<'a>>,
    contacted_deliveries: HashSet<(AttackId, u8, u64)>,
    cold_contacts: HashSet<(AttackId, u64)>,
    deferred_cues: Vec<(Entity, CombatCue)>,
}

impl ComposedApplicationPlan<'_> {
    fn new(resolved_delivery_keys: HashSet<(AttackId, u8)>) -> Self {
        Self {
            targets: HashMap::new(),
            target_order: Vec::new(),
            tracker: TrackerMutationPlan {
                resolved_delivery_keys,
                ..default()
            },
            projections: Vec::new(),
            contacted_deliveries: HashSet::new(),
            cold_contacts: HashSet::new(),
            deferred_cues: Vec::new(),
        }
    }
}

fn snapshot_target(
    combat: &mut CombatTargetState,
    batch: &BatchView<'_>,
    entity: Entity,
) -> Option<PlannedTargetState> {
    let (
        network_id,
        team,
        health,
        effects,
        motion,
        defeated,
        target_disconnected,
        reset_delay_ticks,
        healing_blocked,
    ) = {
        let targets = combat.targets.p1();
        let (
            _,
            network_id,
            team,
            health,
            effects,
            motion,
            defeated,
            controlled,
            test_dummy,
            effect_tile,
        ) = targets.get(entity).ok()?;
        (
            *network_id,
            *team,
            health.0,
            effects.copied().unwrap_or_default(),
            motion.copied(),
            defeated.is_some(),
            controlled.is_some_and(|controlled| batch.disconnected.contains(&controlled.owner)),
            test_dummy.map(|dummy| dummy.reset_delay_ticks),
            effect_tile.is_some_and(crate::map::EffectTileOccupancy::blocks_healing),
        )
    };
    if target_disconnected {
        return None;
    }
    let target_kind = if combat.sentry_targets.contains(entity) {
        CombatTargetKind::Deployable
    } else {
        CombatTargetKind::Fighter
    };
    let (
        authored_maximum_health,
        tenacity,
        cold_capacity,
        cold_resistance,
        poison_resistance,
        fire_resistance,
    ) = {
        let capabilities = combat.passive_access.p0();
        capabilities
            .get(entity)
            .map_or((None, None, 1_000, 0, 0, 0), |(stats, passives)| {
                (
                    Some(stats.maximum_health),
                    passives.find(crate::builds::PassiveKind::Tenacity),
                    stats.cold_capacity,
                    stats.cold_resistance_basis_points,
                    stats.poison_resistance_basis_points,
                    stats.fire_resistance_basis_points,
                )
            })
    };
    Some(PlannedTargetState {
        network_id,
        team,
        health,
        authored_maximum_health,
        world_effects: effects,
        world_motion: motion,
        effects,
        motion,
        defeated,
        target_kind,
        match_participant: combat.match_access.p0().contains(entity),
        active_combatant: combat.match_access.p1().contains(entity),
        spawn_protected: combat.match_access.p2().contains(entity),
        healing_blocked,
        tenacity,
        cold_capacity,
        cold_resistance,
        poison_resistance,
        fire_resistance,
        reset_delay_ticks,
        health_changed: false,
        commit_runtime: false,
        defeat: None,
    })
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the planner explicitly simulates the complete sorted payload transaction without mutating ECS or output resources"
)]
pub(in crate::combat::effects) fn plan_composed_records<'a>(
    tick: u64,
    batch: &BatchView<'a>,
    combat: &mut CombatTargetState,
    reserved_events: &mut impl Iterator<Item = CombatEventId>,
    resolved_delivery_keys: HashSet<(AttackId, u8)>,
    condition_rules: CombatConditionRules,
) -> ComposedApplicationPlan<'a> {
    let mut plan = ComposedApplicationPlan::new(resolved_delivery_keys);
    for record in batch.records {
        let delivery_key = (record.source.attack_id, record.delivery_index);
        plan.tracker.resolved_delivery_keys.insert(delivery_key);
        if !batch
            .connected_owners
            .contains(&record.source.owner_network_entity_id.0)
            && !batch.retained_delivery_keys.contains(&delivery_key)
        {
            plan.tracker
                .cancelled_attacks
                .insert(record.source.attack_id);
            continue;
        }
        if let std::collections::hash_map::Entry::Vacant(entry) = plan.targets.entry(record.target)
        {
            let Some(target) = snapshot_target(combat, batch, record.target) else {
                continue;
            };
            entry.insert(target);
            plan.target_order.push(record.target);
        }
        let target = plan
            .targets
            .get_mut(&record.target)
            .expect("target snapshot was inserted");
        let gate = payload_target_gate(
            combat_source_allows_target(record.source.kind, target.target_kind),
            target.match_participant,
            target.active_combatant,
            target.spawn_protected,
            teams_are_hostile(record.source.team_id, target.team),
        );
        match gate {
            TargetGate::Skip => continue,
            TargetGate::ProtectedContact => {
                if let Some(event_id) = reserved_events.next() {
                    plan.projections
                        .push(ProjectionOp::ProtectedContact(CombatOutcomeFact {
                            event_id,
                            tick,
                            attack_id: record.source.attack_id,
                            source_kind: record.source.kind,
                            source_player: Some(record.source.player_id),
                            source_network_id: Some(record.source.owner_network_entity_id),
                            source_team: Some(record.source.team_id),
                            target_network_id: target.network_id,
                            target_kind: target.target_kind,
                            target_team: target.team,
                            preset_id: record.source.source_preset_id,
                            recipe_fingerprint: Some(record.source.recipe_fingerprint),
                            position: WorldPoint::from(record.position),
                            engagement_distance: record.engagement_distance,
                            kind: CombatOutcomeKind::ProtectedContact,
                        }));
                }
                continue;
            }
            TargetGate::Apply => {}
        }
        let preset_id = record.source.source_preset_id.unwrap_or(WeaponPresetId(0));
        let source = cue_damage_source(record.source);
        let owner_contact = target.network_id == record.source.owner_network_entity_id;
        // These are record-local telemetry/runtime inputs. Defeat clears the accumulated
        // commit shadow but deferred Commands leave this record's observed world state intact.
        let effects_state = target.effects;
        let motion_state = target.motion;
        // Without focused fighter capabilities, the legacy path treats health at the
        // start of each record as that record's healing ceiling.
        let record_maximum_health = target.authored_maximum_health.unwrap_or(target.health);
        if !owner_contact
            && !target.defeated
            && teams_are_hostile(record.source.team_id, target.team)
            && plan.contacted_deliveries.insert((
                record.source.attack_id,
                record.delivery_index,
                target.network_id.0,
            ))
        {
            plan.tracker
                .hostile_contacts
                .insert(record.source.attack_id);
            plan.projections.push(ProjectionOp::HostileDeliveryContact {
                preset_id,
                recipe_fingerprint: record.source.recipe_fingerprint,
            });
        }
        let mut effects = record.bundle.effects.clone();
        effects.sort_by_key(|effect| {
            u8::from(!matches!(effect, PayloadEffectDefinition::Damage { .. }))
        });
        for effect in effects.iter().copied() {
            if !effect_allows_target(effect, target.target_kind) {
                continue;
            }
            let Some(scale) =
                effect_recipient_scale(effect, record.source, target.network_id, target.team)
            else {
                continue;
            };
            if let PayloadEffectDefinition::Damage {
                amount,
                falloff,
                recipients,
            } = effect
            {
                let close_quarters =
                    if matches!(record.source.kind, CombatSourceKind::PrimaryWeapon) {
                        batch
                            .close_quarters_owners
                            .get(&record.source.owner_network_entity_id.0)
                            .copied()
                    } else {
                        None
                    };
                let Some(damage) = plan_damage_application(
                    target.health,
                    target.defeated,
                    amount,
                    falloff,
                    record.delivery_travel,
                    scale,
                    close_quarters.map(|passive| passive.parameters),
                    record.engagement_distance,
                ) else {
                    continue;
                };
                if let Some(passive) = close_quarters
                    && damage.applied != damage.unmodified_applied
                {
                    plan.projections.push(ProjectionOp::AbilityTelemetry(
                        crate::abilities::AbilityTelemetryRecord {
                            tick,
                            owner_network_id: record.source.owner_network_entity_id,
                            kind: crate::abilities::AbilityTelemetryKind::PassiveModified {
                                passive_id: passive.id,
                                amount: damage.applied.abs_diff(damage.unmodified_applied),
                            },
                        },
                    ));
                }
                let event_id = reserved_events
                    .next()
                    .expect("complete payload event reservation matches damage");
                let legacy_event_id = record.source.legacy_compatibility.then(|| {
                    reserved_events
                        .next()
                        .expect("payload event reservation matches legacy damage")
                });
                let defeat_event = damage.defeats.then(|| {
                    reserved_events
                        .next()
                        .expect("payload event reservation matches defeat")
                });
                let legacy_defeat_event = if damage.defeats && record.source.legacy_compatibility {
                    Some(
                        reserved_events
                            .next()
                            .expect("payload event reservation matches legacy defeat"),
                    )
                } else {
                    None
                };
                target.health = damage.health_after;
                target.health_changed = true;
                plan.projections
                    .push(ProjectionOp::Damage(DamageProjection {
                        record,
                        tick,
                        source,
                        target_network_id: target.network_id,
                        target_team: target.team,
                        target_kind: target.target_kind,
                        preset_id,
                        applied_effect: (amount, falloff, recipients),
                        plan: damage,
                        event_id,
                        legacy_event_id,
                        effects: effects_state,
                        motion: motion_state,
                    }));
                if let Some(defeat_event) = defeat_event {
                    target.defeated = true;
                    target.effects = target.world_effects;
                    target.motion = target.world_motion;
                    target.commit_runtime = false;
                    target.defeat = Some(DefeatMutation {
                        event_id: defeat_event,
                        reset_at_tick: target
                            .reset_delay_ticks
                            .map(|delay| tick.saturating_add(delay)),
                    });
                    plan.projections
                        .push(ProjectionOp::Defeat(DefeatProjection {
                            record,
                            tick,
                            source,
                            target_network_id: target.network_id,
                            target_team: target.team,
                            target_kind: target.target_kind,
                            preset_id,
                            owner_contact,
                            event_id: defeat_event,
                            legacy_event_id: legacy_defeat_event,
                        }));
                }
            } else if let PayloadEffectDefinition::Heal { amount, .. } = effect {
                if target.healing_blocked {
                    continue;
                }
                let healing =
                    plan_healing_application(target.health, record_maximum_health, amount, scale);
                target.health = healing.health_after;
                target.health_changed = true;
                let event_id = reserved_events
                    .next()
                    .expect("payload event reservation matches healing");
                plan.projections
                    .push(ProjectionOp::Healing(HealingProjection {
                        record,
                        tick,
                        source,
                        target_network_id: target.network_id,
                        target_team: target.team,
                        target_kind: target.target_kind,
                        preset_id,
                        event_id,
                        effect,
                        plan: healing,
                        effects: effects_state,
                        motion: motion_state,
                    }));
            }
        }
        if target.defeated || target.target_kind == CombatTargetKind::Deployable {
            target.commit_runtime = false;
            continue;
        }
        let runtime = plan_runtime_effects(
            record,
            tick,
            source,
            target.network_id,
            target.team,
            owner_contact,
            CurrentHealth(target.health),
            preset_id,
            effects_state,
            motion_state,
            target.tenacity,
            target.cold_capacity,
            condition_rules.freeze_duration_ticks,
            target.cold_resistance,
            target.poison_resistance,
            target.fire_resistance,
            plan.cold_contacts
                .insert((record.source.attack_id, target.network_id.0)),
            reserved_events,
        );
        plan.projections.extend(
            runtime
                .weapon_projections
                .into_iter()
                .map(ProjectionOp::WeaponTelemetry),
        );
        plan.projections.extend(
            runtime
                .ability_projections
                .into_iter()
                .map(ProjectionOp::AbilityTelemetry),
        );
        plan.deferred_cues.extend(runtime.deferred_cues);
        target.effects = runtime.effects;
        target.motion = runtime.motion;
        target.commit_runtime = true;
    }
    plan
}

pub(in crate::combat::effects) fn commit_composed_plan(
    commands: &mut Commands,
    combat: &mut CombatTargetState,
    trackers: &mut ActiveAttackTrackers,
    plan: &ComposedApplicationPlan<'_>,
) {
    {
        let mut targets = combat.targets.p0();
        for entity in &plan.target_order {
            let target = plan
                .targets
                .get(entity)
                .expect("ordered planned target remains present");
            if !target.health_changed {
                continue;
            }
            let (_, _, mut health, ..) = targets
                .get_mut(*entity)
                .expect("planned target remains present until synchronous commit");
            health.0 = target.health;
        }
    }
    for entity in &plan.target_order {
        let target = plan
            .targets
            .get(entity)
            .expect("ordered planned target remains present");
        if let Some(defeat) = target.defeat {
            commands
                .entity(*entity)
                .insert((
                    Defeated {
                        event_id: defeat.event_id,
                    },
                    CollisionLayers::new(
                        if target.target_kind == CombatTargetKind::Deployable {
                            crate::movement::DEPLOYABLE_LAYER
                        } else {
                            FIGHTER_LAYER
                        },
                        avian2d::prelude::LayerMask::NONE,
                    ),
                    ActiveEffects::default(),
                ))
                .remove::<ExternalMotion>()
                .remove::<KnockbackFeedback>();
            if let Some(reset_at_tick) = defeat.reset_at_tick {
                commands
                    .entity(*entity)
                    .insert(TestDummyResetDeadline(reset_at_tick));
            }
            continue;
        }
        if target.commit_runtime {
            commands.entity(*entity).insert(target.effects);
            if let Some(motion) = target.motion {
                commands.entity(*entity).insert((
                    motion,
                    KnockbackFeedback {
                        velocity: WorldPoint::from(motion.velocity),
                        expires_at_tick: motion.expires_at_tick,
                    },
                ));
            }
        }
    }
    for attack_id in &plan.tracker.cancelled_attacks {
        trackers.active.remove(attack_id);
    }
    for attack_id in &plan.tracker.hostile_contacts {
        if let Some(tracker) = trackers.active.get_mut(attack_id) {
            tracker.had_hostile_contact = true;
        }
    }
    for (attack_id, _) in &plan.tracker.resolved_delivery_keys {
        finish_attack_delivery(trackers, *attack_id);
    }
}

pub(in crate::combat::effects) fn project_composed_plan(
    plan: ComposedApplicationPlan<'_>,
    gameplay_telemetry: &mut AbilityWeaponTelemetry,
    transaction: &mut CombatTransactionState,
) {
    for projection in plan.projections {
        match projection {
            ProjectionOp::ProtectedContact(fact) => transaction.outcome_facts.0.push(fact),
            ProjectionOp::HostileDeliveryContact {
                preset_id,
                recipe_fingerprint,
            } => gameplay_telemetry
                .weapon
                .record_hostile_delivery_contact(preset_id, recipe_fingerprint),
            ProjectionOp::AbilityTelemetry(record) => {
                gameplay_telemetry.ability.record(record);
            }
            ProjectionOp::WeaponTelemetry(record) => {
                gameplay_telemetry.weapon.record(record);
            }
            ProjectionOp::Damage(damage) => project_committed_damage(
                damage.record,
                damage.tick,
                damage.source,
                damage.target_network_id,
                damage.target_team,
                damage.target_kind,
                damage.preset_id,
                damage.applied_effect,
                damage.plan.requested,
                damage.plan.applied,
                damage.plan.health_after,
                damage.event_id,
                damage.legacy_event_id,
                damage.effects,
                damage.motion,
                gameplay_telemetry,
                transaction,
            ),
            ProjectionOp::Healing(healing) => project_committed_healing(
                healing.record,
                healing.tick,
                healing.source,
                healing.target_network_id,
                healing.target_team,
                healing.target_kind,
                healing.preset_id,
                healing.event_id,
                healing.effect,
                healing.plan,
                healing.effects,
                healing.motion,
                gameplay_telemetry,
                transaction,
            ),
            ProjectionOp::Defeat(defeat) => project_committed_defeat(
                defeat.record,
                defeat.tick,
                defeat.source,
                defeat.target_network_id,
                defeat.target_team,
                defeat.target_kind,
                defeat.preset_id,
                defeat.owner_contact,
                defeat.event_id,
                defeat.legacy_event_id,
                gameplay_telemetry,
                transaction,
            ),
        }
    }
    for (entity, cue) in plan.deferred_cues {
        if plan
            .targets
            .get(&entity)
            .is_none_or(|target| !target.defeated)
        {
            transaction.legacy_telemetry.record_cue(cue.clone());
            transaction.outbox.0.push(cue);
        }
    }
}
