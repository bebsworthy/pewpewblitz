//! Shared attack economy and deterministic delivery identity helpers.

#[allow(clippy::wildcard_imports)]
#[cfg(feature = "server")]
use super::*;
use super::{AttackId, FiringPattern, WeaponEconomy};

#[must_use]
pub fn delivery_count(firing: FiringPattern) -> u8 {
    match firing {
        FiringPattern::Single => 1,
        FiringPattern::Spread { delivery_count, .. } => delivery_count,
    }
}

#[must_use]
pub fn economy_ready(resource: u8, phase_ready: bool) -> bool {
    phase_ready && resource > 0
}

#[must_use]
pub fn refill_deadline(current_tick: u64, economy: WeaponEconomy) -> u64 {
    current_tick.saturating_add(economy.refill_ticks())
}

#[must_use]
pub fn delivery_key(attack_id: AttackId, delivery_index: u8) -> (u64, u8) {
    (attack_id.0, delivery_index)
}

#[cfg(feature = "server")]
pub(super) fn advance_composed_weapon_state(
    state: &mut WeaponState,
    recipe: &WeaponRecipe,
    tick: u64,
) {
    match state.phase {
        WeaponPhase::Cooldown { ready_at_tick } if tick >= ready_at_tick => {
            state.phase = WeaponPhase::Ready;
        }
        WeaponPhase::Reloading { ready_at_tick } if tick >= ready_at_tick => {
            state.ammo = recipe.economy.capacity();
            state.phase = WeaponPhase::Ready;
        }
        _ => {}
    }
}

#[cfg(feature = "server")]
fn delivery_angles(facing: f32, firing: FiringPattern) -> Vec<f32> {
    spread_angles(
        facing,
        match firing {
            FiringPattern::Single => 1,
            FiringPattern::Spread { delivery_count, .. } => delivery_count,
        },
        match firing {
            FiringPattern::Single => 0.0,
            FiringPattern::Spread {
                total_angle_degrees,
                ..
            } => total_angle_degrees,
        },
    )
}

#[cfg(feature = "server")]
fn resolved_lob_landing(
    origin: Vec2,
    facing: f32,
    recipe: &WeaponRecipe,
    arena: &crate::movement::GreyboxArenaDefinition,
    spatial_query: &avian2d::prelude::SpatialQuery,
) -> Option<Vec2> {
    let DeliveryMethod::Lobbed {
        distance,
        landing_clearance_radius,
        ..
    } = recipe.delivery
    else {
        return None;
    };
    let desired = origin + Vec2::from_angle(facing) * distance;
    let bounded = desired.clamp(
        arena.min + Vec2::splat(landing_clearance_radius),
        arena.max - Vec2::splat(landing_clearance_radius),
    );
    let terrain_filter = avian2d::prelude::SpatialQueryFilter::from_mask(
        INDESTRUCTIBLE_TERRAIN_LAYER | DESTRUCTIBLE_TERRAIN_LAYER,
    );
    delivery::repaired_landing_point(origin, bounded, landing_clearance_radius, |candidate| {
        spatial_query
            .shape_intersections(
                &Collider::circle(landing_clearance_radius),
                candidate,
                0.0,
                &terrain_filter,
            )
            .is_empty()
    })
}

#[cfg(feature = "server")]
fn blocked_straight_deliveries(
    origin: Vec2,
    facing: f32,
    recipe: &WeaponRecipe,
    spatial_query: &avian2d::prelude::SpatialQuery,
) -> Vec<(u8, Vec2, Vec2)> {
    let DeliveryMethod::Straight {
        radius,
        muzzle_offset,
        ..
    } = recipe.delivery
    else {
        return Vec::new();
    };
    delivery_angles(facing, recipe.firing)
        .into_iter()
        .enumerate()
        .filter_map(|(index, angle)| {
            let muzzle = muzzle_position(origin, angle, muzzle_offset);
            terrain_muzzle_contact(origin, muzzle, radius, spatial_query)
                .map(|(point, normal)| (u8::try_from(index).unwrap_or(u8::MAX), point, normal))
        })
        .collect()
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
fn emit_attack_deliveries(
    commands: &mut Commands,
    tick: u64,
    owner_entity: Entity,
    origin: Vec2,
    facing: f32,
    resolved: &ResolvedWeapon,
    source: AttackSource,
    source_component: ProjectileSource,
    weapon_id: WeaponDefinitionId,
    blocked_deliveries: &[(u8, Vec2, Vec2)],
    reserved_events: &[CombatEventId],
    blocked_event_cursor: &mut usize,
    legacy_telemetry: &mut CombatTelemetry,
    outbox: &mut CombatOutbox,
    melee: &mut MessageWriter<MeleeAttack>,
    lob_landing: Option<Vec2>,
) -> u64 {
    let recipe = &resolved.recipe;
    let attack_id = source.attack_id;
    let network_id = &source.owner_network_entity_id;
    let legacy_compatibility = source.legacy_compatibility;
    let entity = owner_entity;
    let mut emitted_deliveries = 0_u64;
    match recipe.delivery {
        DeliveryMethod::Straight {
            speed,
            radius,
            range,
            lifetime_ticks,
            muzzle_offset,
        } => {
            let angles = delivery_angles(facing, recipe.firing);
            for (delivery_index, angle) in angles.into_iter().enumerate() {
                let delivery_index = u8::try_from(delivery_index).unwrap_or(u8::MAX);
                let muzzle = muzzle_position(origin, angle, muzzle_offset);
                if let Some((point, normal)) = blocked_deliveries
                    .iter()
                    .find(|(blocked_index, _, _)| *blocked_index == delivery_index)
                    .map(|(_, point, normal)| (*point, *normal))
                {
                    let impact_event_id = reserved_events[*blocked_event_cursor];
                    *blocked_event_cursor += 1;
                    let impact_cue = CombatCue::DeliveryImpact {
                        event_id: impact_event_id,
                        tick,
                        attack_id,
                        delivery_index,
                        source: *network_id,
                        weapon_definition_id: weapon_id,
                        presentation_profile_id: resolved.presentation_profile_id,
                        target: None,
                        position: WorldPoint::from(point),
                        normal: WorldPoint::from(normal),
                        distance_band: distance_band(origin.distance(point)),
                    };
                    legacy_telemetry.record_cue(impact_cue.clone());
                    outbox.0.push(impact_cue);
                    if legacy_compatibility {
                        let legacy_event = reserved_events[*blocked_event_cursor];
                        *blocked_event_cursor += 1;
                        let legacy_cue = CombatCue::Impact {
                            event_id: legacy_event,
                            tick,
                            source: *network_id,
                            shot_id: ShotId(attack_id.0),
                            weapon_definition_id: weapon_id,
                            target: None,
                            position: WorldPoint::from(point),
                            normal: WorldPoint::from(normal),
                            distance_band: distance_band(origin.distance(point)),
                        };
                        legacy_telemetry.record_cue(legacy_cue.clone());
                        legacy_telemetry.record(CombatLogRecord::Hit {
                            tick,
                            event_id: legacy_event,
                            shot_id: ShotId(attack_id.0),
                            source: *network_id,
                            target: None,
                            weapon: weapon_id,
                            position: WorldPoint::from(point),
                            distance: origin.distance(point),
                            band: distance_band(origin.distance(point)),
                        });
                        outbox.0.push(legacy_cue);
                    }
                    emitted_deliveries = emitted_deliveries.saturating_add(1);
                    continue;
                }
                commands.spawn((
                    Projectile,
                    source_component,
                    ReplicatedAttackSource { attack: source },
                    AttackDelivery {
                        attack_id,
                        delivery_index,
                    },
                    ProjectileDeadline {
                        expires_at_tick: tick.saturating_add(lifetime_ticks),
                    },
                    StraightFlight {
                        origin: WorldPoint::from(muzzle),
                        facing: angle,
                        speed,
                        maximum_range: range,
                        launched_at_tick: tick,
                    },
                    ComposedProjectileRuntime {
                        owner_entity: entity,
                        source,
                        delivery_index,
                        velocity: Vec2::from_angle(angle) * speed,
                        travelled: 0.0,
                        expires_at_tick: tick.saturating_add(lifetime_ticks),
                        maximum_range: range,
                        radius,
                        landing: None,
                        recipe: recipe.clone(),
                    },
                    Position::from_xy(muzzle.x, muzzle.y),
                    Rotation::radians(angle),
                    Collider::circle(radius),
                    CollisionLayers::new(
                        PROJECTILE_LAYER,
                        FIGHTER_LAYER | INDESTRUCTIBLE_TERRAIN_LAYER | DESTRUCTIBLE_TERRAIN_LAYER,
                    ),
                    Replicate::to_clients(NetworkTarget::All),
                    InterpolationTarget::to_clients(NetworkTarget::All),
                ));
                emitted_deliveries = emitted_deliveries.saturating_add(1);
            }
        }
        DeliveryMethod::Lobbed {
            distance,
            flight_ticks,
            visual_arc_height,
            landing_clearance_radius: _,
            muzzle_offset,
        } => {
            let landing = lob_landing.expect("validated lob landing must exist");
            let launch = muzzle_position(origin, facing, muzzle_offset);
            commands.spawn((
                Projectile,
                source_component,
                ReplicatedAttackSource { attack: source },
                AttackDelivery {
                    attack_id,
                    delivery_index: 0,
                },
                ProjectileDeadline {
                    expires_at_tick: tick.saturating_add(flight_ticks),
                },
                LobbedFlight {
                    launch: WorldPoint::from(launch),
                    landing: WorldPoint::from(landing),
                    launched_at_tick: tick,
                    lands_at_tick: tick.saturating_add(flight_ticks),
                    visual_arc_height,
                },
                ComposedProjectileRuntime {
                    owner_entity: entity,
                    source,
                    delivery_index: 0,
                    velocity: Vec2::ZERO,
                    travelled: 0.0,
                    expires_at_tick: tick.saturating_add(flight_ticks),
                    maximum_range: distance,
                    radius: 0.0,
                    landing: Some(landing),
                    recipe: recipe.clone(),
                },
                Position::from_xy(launch.x, launch.y),
                Rotation::radians(facing),
                Replicate::to_clients(NetworkTarget::All),
                InterpolationTarget::to_clients(NetworkTarget::All),
            ));
            emitted_deliveries = 1;
        }
        DeliveryMethod::MeleeArc { .. } => {
            melee.write(MeleeAttack {
                source,
                origin,
                facing,
                tick,
                recipe: recipe.clone(),
            });
            emitted_deliveries = 1;
        }
    }
    emitted_deliveries
}

#[cfg(feature = "server")]
struct AcceptedAttackRecord<'a> {
    tick: u64,
    source: AttackSource,
    weapon_definition_id: WeaponDefinitionId,
    event_id: CombatEventId,
    legacy_muzzle_event: Option<CombatEventId>,
    origin: Vec2,
    facing: f32,
    ammo_after: u8,
    blocked_delivery_count: usize,
    emitted_deliveries: u64,
    recipe: &'a WeaponRecipe,
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_lines)]
fn record_accepted_attack(
    record: AcceptedAttackRecord<'_>,
    evidence_enabled: bool,
    trackers: &mut ActiveAttackTrackers,
    telemetry: &mut WeaponTelemetry,
    legacy_telemetry: &mut CombatTelemetry,
    outbox: &mut CombatOutbox,
) {
    let AcceptedAttackRecord {
        tick,
        source,
        weapon_definition_id,
        event_id,
        legacy_muzzle_event,
        origin,
        facing,
        ammo_after,
        blocked_delivery_count,
        emitted_deliveries,
        recipe,
    } = record;
    let attack_id = source.attack_id;
    let preset_id = source
        .source_preset_id
        .unwrap_or(WeaponPresetId(weapon_definition_id.0));
    telemetry.record_emitted_deliveries(preset_id, source.recipe_fingerprint, emitted_deliveries);
    if emitted_deliveries > 0 {
        if trackers.active.len() < server::MAX_ACTIVE_ATTACK_TRACKERS {
            trackers.active.insert(
                attack_id,
                ActiveAttackTracker {
                    source,
                    expected_deliveries: u8::try_from(emitted_deliveries).unwrap_or(u8::MAX),
                    resolved_deliveries: 0,
                    had_hostile_contact: false,
                },
            );
        } else {
            telemetry.tracker_drops = telemetry.tracker_drops.saturating_add(1);
        }
    }
    for _ in 0..blocked_delivery_count {
        finish_attack_delivery(trackers, attack_id);
    }
    telemetry.record_accepted_attack(preset_id, source.recipe_fingerprint);
    legacy_telemetry.accepted_shots = legacy_telemetry.accepted_shots.saturating_add(1);
    let accepted_cue = CombatCue::AttackAccepted {
        event_id,
        tick,
        attack_id,
        source: source.owner_network_entity_id,
        weapon_definition_id,
        presentation_profile_id: source.presentation_profile_id,
    };
    legacy_telemetry.record_cue(accepted_cue.clone());
    outbox.0.push(accepted_cue);
    if evidence_enabled {
        if legacy_telemetry.accepted_shot_timestamps.len() < MAX_COMBAT_EVIDENCE_EVENTS {
            legacy_telemetry
                .accepted_shot_timestamps
                .push((ShotId(attack_id.0), unix_epoch_micros()));
        } else {
            legacy_telemetry.dropped_accepted_shot_timestamps = legacy_telemetry
                .dropped_accepted_shot_timestamps
                .saturating_add(1);
        }
    }
    if let Some(muzzle_event) = legacy_muzzle_event {
        let muzzle = muzzle_position(
            origin,
            facing,
            match recipe.delivery {
                DeliveryMethod::Straight { muzzle_offset, .. }
                | DeliveryMethod::Lobbed { muzzle_offset, .. } => muzzle_offset,
                DeliveryMethod::MeleeArc { .. } => 0.0,
            },
        );
        legacy_telemetry.record(CombatLogRecord::Shot {
            event_id: muzzle_event,
            tick,
            shot_id: ShotId(attack_id.0),
            source: source.owner_network_entity_id,
            weapon: weapon_definition_id,
            muzzle_position: WorldPoint::from(muzzle),
            ammo_after,
        });
        let muzzle_cue = CombatCue::Muzzle {
            event_id: muzzle_event,
            tick,
            source: source.owner_network_entity_id,
            shot_id: ShotId(attack_id.0),
            weapon_definition_id,
            position: WorldPoint::from(muzzle),
        };
        legacy_telemetry.record_cue(muzzle_cue.clone());
        outbox.0.push(muzzle_cue);
    }
    telemetry.record(WeaponTelemetryRecord {
        tick,
        event_id,
        attack_id,
        preset_id,
        recipe_fingerprint: source.recipe_fingerprint,
        delivery_index: None,
        source: source.owner_network_entity_id,
        target: None,
        position: WorldPoint::from(origin),
        requested_value: 0,
        applied_value: 0,
        engagement_distance: 0.0,
        delivery_travel: 0.0,
        hostile_contact: false,
        effect: None,
        resulting_health: None,
        resulting_effects: None,
        resulting_motion: None,
        outcome: WeaponTelemetryOutcome::AttackAccepted,
    });
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub(super) fn authoritative_composed_fire(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    arena: Res<crate::movement::GreyboxArenaDefinition>,
    spatial_query: avian2d::prelude::SpatialQuery,
    disconnected: Query<Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
    mut ids: ResMut<NextCombatIds>,
    mut telemetry: ResMut<WeaponTelemetry>,
    mut legacy_telemetry: ResMut<CombatTelemetry>,
    evidence: Res<CombatEvidenceMode>,
    mut trackers: ResMut<ActiveAttackTrackers>,
    mut outbox: ResMut<CombatOutbox>,
    mut melee: MessageWriter<MeleeAttack>,
    query: Query<
        (
            Entity,
            &Position,
            &Rotation,
            &SelectedBuild,
            &ResolvedWeapon,
            &TeamId,
            &PlayerId,
            &NetworkEntityId,
            Option<&lightyear::prelude::ControlledBy>,
            &crate::movement::InputFreshness,
            &mut WeaponState,
            Option<&ActionState<FighterInput>>,
            Option<&Defeated>,
            Option<&AwaitingPostSelectionInput>,
        ),
        With<Fighter>,
    >,
) {
    let disconnected: HashSet<_> = disconnected.iter().collect();
    for (
        entity,
        position,
        rotation,
        build,
        resolved,
        team,
        player_id,
        network_id,
        controlled_by,
        freshness,
        mut state,
        action,
        defeated,
        activation_barrier,
    ) in query
    {
        if controlled_by.is_some_and(|controlled| disconnected.contains(&controlled.owner)) {
            continue;
        }
        if defeated.is_some() || activation_barrier.is_some() {
            continue;
        }
        let recipe = &resolved.recipe;
        advance_composed_weapon_state(&mut state, recipe, tick.0);
        let input = action.map_or(FighterInput::default(), |value| value.0);
        let held = !input_should_neutralize(tick.0, freshness.last_fresh_tick, 12)
            && input.is_valid()
            && input.gameplay_buttons & FighterInput::PRIMARY_FIRE != 0;
        if !held || !matches!(state.phase, WeaponPhase::Ready) {
            if held && state.ammo == 0 && matches!(state.phase, WeaponPhase::Ready) {
                state.phase = WeaponPhase::Reloading {
                    ready_at_tick: tick.0.saturating_add(recipe.economy.refill_ticks()),
                };
            }
            continue;
        }
        if state.ammo == 0 {
            state.phase = WeaponPhase::Reloading {
                ready_at_tick: tick.0.saturating_add(recipe.economy.refill_ticks()),
            };
            continue;
        }
        let origin = position.0;
        let facing = rotation.as_radians();
        let lob_landing = resolved_lob_landing(origin, facing, recipe, &arena, &spatial_query);
        if matches!(recipe.delivery, DeliveryMethod::Lobbed { .. }) && lob_landing.is_none() {
            continue;
        }
        let legacy_compatibility = legacy_compatibility_recipe(recipe);
        let blocked_deliveries =
            blocked_straight_deliveries(origin, facing, recipe, &spatial_query);
        let per_blocked_delivery_events = if legacy_compatibility { 2 } else { 1 };
        let event_count = 1
            + usize::from(legacy_compatibility)
            + blocked_deliveries.len() * per_blocked_delivery_events;
        let Some((attack_id, reserved_events)) =
            server::reserve_attack_and_events(&mut ids, event_count)
        else {
            continue;
        };
        let event_id = reserved_events[0];
        let legacy_muzzle_event = if legacy_compatibility {
            Some(reserved_events[1])
        } else {
            None
        };
        let mut blocked_event_cursor = 1 + usize::from(legacy_compatibility);
        state.ammo = state.ammo.saturating_sub(1);
        state.phase = if state.ammo == 0 {
            WeaponPhase::Reloading {
                ready_at_tick: tick.0.saturating_add(recipe.economy.refill_ticks()),
            }
        } else {
            WeaponPhase::Cooldown {
                ready_at_tick: tick.0.saturating_add(recipe.fire_cooldown_ticks),
            }
        };
        let preset_id = resolved.source_preset_id;
        let source = AttackSource {
            attack_id,
            player_id: *player_id,
            owner_network_entity_id: *network_id,
            team_id: *team,
            recipe_fingerprint: resolved.recipe_fingerprint,
            presentation_profile_id: resolved.presentation_profile_id,
            legacy_compatibility,
            source_preset_id: preset_id,
            origin: WorldPoint::from(origin),
            facing,
        };
        let weapon_id = build.primary_weapon;
        let source_component = ProjectileSource {
            shot_id: ShotId(attack_id.0),
            player_id: *player_id,
            owner_network_entity_id: *network_id,
            team_id: *team,
            weapon_definition_id: weapon_id,
        };
        let emitted_deliveries = emit_attack_deliveries(
            &mut commands,
            tick.0,
            entity,
            origin,
            facing,
            resolved,
            source,
            source_component,
            weapon_id,
            &blocked_deliveries,
            &reserved_events,
            &mut blocked_event_cursor,
            &mut legacy_telemetry,
            &mut outbox,
            &mut melee,
            lob_landing,
        );
        record_accepted_attack(
            AcceptedAttackRecord {
                tick: tick.0,
                source,
                weapon_definition_id: weapon_id,
                event_id,
                legacy_muzzle_event,
                origin,
                facing,
                ammo_after: state.ammo,
                blocked_delivery_count: blocked_deliveries.len(),
                emitted_deliveries,
                recipe,
            },
            evidence.enabled,
            &mut trackers,
            &mut telemetry,
            &mut legacy_telemetry,
            &mut outbox,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delivery_indices_are_stable_within_one_attack() {
        assert_eq!(delivery_key(AttackId(7), 3), (7, 3));
        assert_eq!(delivery_count(FiringPattern::Single), 1);
    }
}
