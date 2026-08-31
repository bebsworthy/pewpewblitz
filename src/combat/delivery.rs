//! Pure geometry shared by server delivery systems and client previews.

#[allow(clippy::wildcard_imports)]
#[cfg(feature = "server")]
use super::*;
use bevy::prelude::Vec2;

#[cfg(feature = "server")]
mod projectiles;
#[cfg(feature = "server")]
pub(super) use projectiles::sweep_composed_projectiles;

#[must_use]
#[cfg(feature = "client")]
pub fn lob_height(progress: f32, visual_arc_height: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    4.0 * visual_arc_height * progress * (1.0 - progress)
}

#[must_use]
#[cfg(feature = "server")]
pub fn sector_contains(
    origin: Vec2,
    facing: f32,
    reach: f32,
    angle_degrees: f32,
    target_center: Vec2,
    target_radius: f32,
) -> bool {
    let delta = target_center - origin;
    let distance = delta.length();
    if !delta.is_finite() || !distance.is_finite() || distance > reach + target_radius {
        return false;
    }
    if distance <= f32::EPSILON {
        return true;
    }
    let half_angle = (angle_degrees.to_radians() / 2.0).clamp(0.0, std::f32::consts::PI);
    let angular_padding = (target_radius / distance).clamp(0.0, 1.0).asin();
    let difference = (delta.y.atan2(delta.x) - facing + std::f32::consts::PI)
        .rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    difference.abs() <= half_angle + angular_padding
}

#[must_use]
pub fn repaired_landing_point(
    launch: Vec2,
    desired: Vec2,
    minimum_distance: f32,
    mut is_clear: impl FnMut(Vec2) -> bool,
) -> Option<Vec2> {
    let ray = desired - launch;
    let distance = ray.length();
    if !distance.is_finite() || distance <= f32::EPSILON {
        return is_clear(launch).then_some(launch);
    }
    let direction = ray / distance;
    let minimum_distance = minimum_distance.clamp(0.0, distance);
    let mut furthest_clear = None;
    let mut blocked = distance;
    let mut sample = distance;
    for _ in 0..128 {
        let point = launch + direction * sample;
        if is_clear(point) {
            furthest_clear = Some(sample);
            break;
        }
        blocked = sample;
        if sample <= minimum_distance {
            break;
        }
        sample = (sample - 5.0).max(minimum_distance);
    }
    let mut clear = furthest_clear?;
    for _ in 0..8 {
        let middle = clear.midpoint(blocked);
        if is_clear(launch + direction * middle) {
            clear = middle;
        } else {
            blocked = middle;
        }
    }
    Some(launch + direction * clear)
}

#[cfg(feature = "server")]
pub(super) fn queue_damageable_target(
    world: &mut crate::map::PendingWorldTargetDamages,
    objectives: &mut crate::matchplay::PendingModeObjectiveDamages,
    request: crate::map::PendingWorldTargetDamage,
) {
    match request.target {
        crate::map::DamageableTargetIdentity::MapObject { .. } => world.0.push(request),
        crate::map::DamageableTargetIdentity::HeistSafe { .. } => {
            objectives
                .0
                .push(crate::matchplay::PendingModeObjectiveDamage {
                    target: request.target,
                    source: request.source,
                    requested_damage: request.requested_damage,
                    delivery_index: request.delivery_index,
                    bundle_index: request.bundle_index,
                    effect_index: request.effect_index,
                });
        }
    }
}

#[cfg(feature = "server")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AreaFighterCandidate {
    pub(super) entity: Entity,
    pub(super) position: Vec2,
    pub(super) team: TeamId,
    pub(super) network_id: NetworkEntityId,
    pub(super) defeated: bool,
    pub(super) disconnected: bool,
    pub(super) line_of_sight_clear: bool,
}

#[cfg(feature = "server")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AreaObjectCandidate {
    pub(super) position: Vec2,
    pub(super) identity: crate::map::DamageableTargetIdentity,
    pub(super) line_of_sight_clear: bool,
}

#[cfg(feature = "server")]
pub(super) struct AreaBundleCandidates {
    bundle_index: usize,
    fighters: Vec<AreaFighterCandidate>,
    objects: Vec<AreaObjectCandidate>,
}

#[cfg(feature = "server")]
pub(super) struct PlannedAreaPayloads {
    pub(super) payloads: Vec<PendingPayload>,
    pub(super) world_damages: Vec<crate::map::PendingWorldTargetDamage>,
    pub(super) selected_targets: usize,
}

#[cfg(feature = "server")]
pub(super) fn collect_area_bundle_candidates(
    recipe: &WeaponRecipe,
    mut fighters: impl FnMut(f32, bool) -> Vec<AreaFighterCandidate>,
    mut objects: impl FnMut(f32, bool) -> Vec<AreaObjectCandidate>,
) -> Vec<AreaBundleCandidates> {
    recipe
        .payload_bundles
        .iter()
        .enumerate()
        .filter_map(|(bundle_index, bundle)| {
            let TargetSelection::Area {
                radius,
                map_occlusion,
                ..
            } = bundle.target
            else {
                return None;
            };
            Some(AreaBundleCandidates {
                bundle_index,
                fighters: fighters(radius, map_occlusion),
                objects: objects(radius, map_occlusion),
            })
        })
        .collect()
}

/// Deterministic area-target policy shared by immediate and projectile-delivered payloads.
#[cfg(feature = "server")]
pub(super) fn plan_area_payloads(
    landing: Vec2,
    source: AttackSource,
    delivery_index: u8,
    recipe: &WeaponRecipe,
    candidates: Vec<AreaBundleCandidates>,
) -> PlannedAreaPayloads {
    let delivery_travel = lob_launch_point(source, recipe).distance(landing);
    let mut payloads = Vec::new();
    let mut world_damages = Vec::new();
    let mut selected_targets = 0;
    for mut candidates in candidates {
        let bundle = &recipe.payload_bundles[candidates.bundle_index];
        let TargetSelection::Area { max_targets, .. } = bundle.target else {
            continue;
        };
        candidates
            .fighters
            .sort_by_key(|fighter| fighter.network_id.0);
        let mut collected = 0_u8;
        for fighter in candidates.fighters {
            if collected >= max_targets {
                break;
            }
            if fighter.defeated
                || fighter.disconnected
                || !fighter.line_of_sight_clear
                || !payload_can_affect_target(bundle, source, fighter.team, fighter.network_id)
            {
                continue;
            }
            payloads.push(PendingPayload {
                source,
                delivery_index,
                bundle_index: u8::try_from(candidates.bundle_index).unwrap_or(u8::MAX),
                target: fighter.entity,
                target_network_id: fighter.network_id,
                position: landing,
                engagement_distance: source.origin.as_vec2().distance(fighter.position),
                delivery_travel,
                contact_fraction: 1.0,
                bundle: bundle.clone(),
            });
            collected = collected.saturating_add(1);
            selected_targets += 1;
        }
        candidates
            .objects
            .sort_by_key(|object| object.identity.stable_order_key());
        for object in candidates.objects {
            if collected >= max_targets {
                break;
            }
            if !object.line_of_sight_clear {
                continue;
            }
            for (effect_index, effect) in bundle.effects.iter().enumerate() {
                let PayloadEffectDefinition::Damage {
                    amount, falloff, ..
                } = *effect
                else {
                    continue;
                };
                world_damages.push(crate::map::PendingWorldTargetDamage {
                    target: object.identity,
                    source,
                    attack_id: source.attack_id,
                    requested_damage: effects::requested_damage(
                        amount,
                        falloff,
                        delivery_travel,
                        1.0,
                        None,
                        source.origin.as_vec2().distance(object.position),
                    ),
                    delivery_index,
                    bundle_index: u8::try_from(candidates.bundle_index).unwrap_or(u8::MAX),
                    effect_index: u8::try_from(effect_index).unwrap_or(u8::MAX),
                });
            }
            // Objects consume the shared target budget even when a bundle has no Damage effect.
            collected = collected.saturating_add(1);
            selected_targets += 1;
        }
    }
    PlannedAreaPayloads {
        payloads,
        world_damages,
        selected_targets,
    }
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime; the query declares this system's complete world view inline at its schedule boundary"
)]
#[allow(
    clippy::too_many_lines,
    reason = "the authoritative melee system keeps combatant and world-object sector, occlusion, and shared payload planning together"
)]
pub(super) fn resolve_melee_attacks(
    mut attacks: MessageReader<MeleeAttack>,
    mut pending: MessageWriter<PendingPayload>,
    mut world_pending: ResMut<crate::map::PendingWorldTargetDamages>,
    mut objective_pending: ResMut<crate::matchplay::PendingModeObjectiveDamages>,
    mut deliveries: MessageWriter<PendingDelivery>,
    mut trackers: ResMut<ActiveAttackTrackers>,
    disconnected: Query<Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
    fighters: Query<
        (
            Entity,
            &Position,
            &TeamId,
            &NetworkEntityId,
            Option<&Defeated>,
            Option<&lightyear::prelude::ControlledBy>,
        ),
        Or<(With<Fighter>, With<crate::abilities::Sentry>)>,
    >,
    objects: Query<
        (
            Entity,
            &Position,
            &crate::map::DamageableTargetIdentity,
            &CurrentHealth,
            &crate::map::DamageableLifeState,
        ),
        Or<(
            With<crate::map::DamageableWorldObject>,
            With<crate::matchplay::HeistSafe>,
        )>,
    >,
    spatial_query: avian2d::prelude::SpatialQuery,
    builds: Res<crate::builds::BuildCatalogResource>,
) {
    let disconnected: HashSet<_> = disconnected.iter().collect();
    for attack in attacks.read() {
        let owner_connected = fighters.iter().any(|(_, _, _, network_id, _, controlled)| {
            *network_id == attack.source.owner_network_entity_id
                && controlled.is_none_or(|controlled| !disconnected.contains(&controlled.owner))
        });
        if !owner_connected {
            finish_attack_delivery(&mut trackers, attack.source.attack_id);
            continue;
        }
        let Some((reach, angle)) = (match attack.recipe.delivery {
            DeliveryMethod::MeleeArc {
                reach,
                angle_degrees,
            } => Some((reach, angle_degrees)),
            _ => None,
        }) else {
            continue;
        };
        let mut queued_payloads = false;
        let fighter_filter = avian2d::prelude::SpatialQueryFilter::from_mask(
            FIGHTER_LAYER | crate::movement::DEPLOYABLE_LAYER,
        );
        let mut candidates: Vec<_> = spatial_query
            .shape_intersections(
                &Collider::circle(reach),
                attack.origin,
                0.0,
                &fighter_filter,
            )
            .into_iter()
            .filter_map(|entity| fighters.get(entity).ok())
            .collect();
        candidates.sort_by_key(|(_, _, _, network_id, _, _)| network_id.0);
        for (target, position, team, network_id, defeated, controlled) in candidates {
            if defeated.is_some()
                || controlled.is_some_and(|controlled| disconnected.contains(&controlled.owner))
                || !payload_target_visible(attack.source, *team, *network_id)
                || !sector_contains(
                    attack.origin,
                    attack.facing,
                    reach,
                    angle,
                    position.0,
                    builds.0.fighter_body.radius,
                )
                || !area_line_of_sight_clear(attack.origin, position.0, &spatial_query)
            {
                continue;
            }
            let valid_bundles: Vec<_> = attack
                .recipe
                .payload_bundles
                .iter()
                .enumerate()
                .filter(|(_, bundle)| {
                    matches!(bundle.target, TargetSelection::Direct)
                        && payload_can_affect_target(bundle, attack.source, *team, *network_id)
                })
                .collect();
            if valid_bundles.is_empty() {
                continue;
            }
            deliveries.write(PendingDelivery {
                entity: None,
                source: attack.source,
                delivery_index: 0,
                tick: attack.tick,
                engagement_distance: attack.origin.distance(position.0),
                delivery_travel: 0.0,
                kind: PendingDeliveryKind::MeleeContact {
                    target: *network_id,
                    position: WorldPoint::from(position.0),
                },
                world_effects: attack.recipe.world_effects.clone(),
            });
            for (bundle_index, bundle) in valid_bundles {
                pending.write(PendingPayload {
                    source: attack.source,
                    delivery_index: 0,
                    bundle_index: u8::try_from(bundle_index).unwrap_or(u8::MAX),
                    target,
                    target_network_id: *network_id,
                    position: position.0,
                    engagement_distance: attack.origin.distance(position.0),
                    delivery_travel: 0.0,
                    contact_fraction: 1.0,
                    bundle: bundle.clone(),
                });
                queued_payloads = true;
            }
        }
        let mut object_candidates: Vec<_> = objects
            .iter()
            .filter(|(_, position, _, health, life)| {
                crate::map::object_is_live(**health, **life)
                    && sector_contains(attack.origin, attack.facing, reach, angle, position.0, 16.0)
            })
            .collect();
        object_candidates.sort_by_key(|(_, _, identity, ..)| identity.stable_order_key());
        for (entity, position, identity, _, _) in object_candidates {
            if !area_line_of_sight_clear_excluding(
                attack.origin,
                position.0,
                entity,
                &spatial_query,
            ) {
                continue;
            }
            for (bundle_index, bundle) in attack
                .recipe
                .payload_bundles
                .iter()
                .enumerate()
                .filter(|(_, bundle)| matches!(bundle.target, TargetSelection::Direct))
            {
                for (effect_index, effect) in bundle.effects.iter().enumerate() {
                    let PayloadEffectDefinition::Damage {
                        amount, falloff, ..
                    } = *effect
                    else {
                        continue;
                    };
                    queue_damageable_target(
                        &mut world_pending,
                        &mut objective_pending,
                        crate::map::PendingWorldTargetDamage {
                            target: *identity,
                            source: attack.source,
                            attack_id: attack.source.attack_id,
                            requested_damage: effects::requested_damage(
                                amount,
                                falloff,
                                0.0,
                                1.0,
                                None,
                                attack.origin.distance(position.0),
                            ),
                            delivery_index: 0,
                            bundle_index: u8::try_from(bundle_index).unwrap_or(u8::MAX),
                            effect_index: u8::try_from(effect_index).unwrap_or(u8::MAX),
                        },
                    );
                }
            }
        }
        if !queued_payloads {
            finish_attack_delivery(&mut trackers, attack.source.attack_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "server")]
    fn sticky_sweep_app(tick: u64) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(avian2d::prelude::PhysicsPlugins::default())
            .insert_resource(SimulationTick(tick))
            .init_resource::<NextCombatIds>()
            .init_resource::<ActiveAttackTrackers>()
            .init_resource::<WeaponTelemetry>()
            .init_resource::<crate::map::PendingWorldTargetDamages>()
            .init_resource::<crate::matchplay::PendingModeObjectiveDamages>()
            .add_message::<PendingPayload>()
            .add_message::<PendingDelivery>()
            .add_systems(
                FixedPostUpdate,
                (
                    sweep_composed_projectiles
                        .after(avian2d::prelude::PhysicsSystems::StepSimulation),
                    ApplyDeferred.after(sweep_composed_projectiles),
                ),
            );
        app.finish();
        app.cleanup();
        app.update();
        app
    }

    #[cfg(feature = "server")]
    fn sticky_recipe() -> WeaponRecipe {
        WeaponCatalog::embedded()
            .unwrap()
            .preset(WeaponPresetId(5))
            .unwrap()
            .configuration
            .recipe
            .clone()
    }

    #[cfg(feature = "server")]
    fn sticky_attack_source(attack_id: u64, owner: NetworkEntityId) -> AttackSource {
        AttackSource {
            kind: CombatSourceKind::PrimaryWeapon,
            attack_id: AttackId(attack_id),
            player_id: PlayerId(owner.0),
            owner_network_entity_id: owner,
            team_id: TeamId(0),
            recipe_fingerprint: WeaponRecipeFingerprint(11),
            legacy_compatibility: false,
            source_preset_id: Some(WeaponPresetId(5)),
            origin: WorldPoint::from(Vec2::ZERO),
            facing: 0.0,
        }
    }

    #[cfg(feature = "server")]
    fn spawn_sticky_owner(app: &mut App, owner: NetworkEntityId) -> Entity {
        app.world_mut()
            .spawn((Fighter, Position(Vec2::ZERO), TeamId(0), owner))
            .id()
    }

    #[cfg(feature = "server")]
    fn spawn_sticky_projectile(
        app: &mut App,
        owner_entity: Entity,
        source: AttackSource,
        position: Vec2,
        expires_at_tick: u64,
    ) -> Entity {
        let recipe = sticky_recipe();
        app.world_mut()
            .spawn((
                Projectile,
                Position(position),
                ProjectileBody::circle(2.0),
                Collider::circle(2.0),
                CollisionLayers::new(
                    PROJECTILE_LAYER,
                    FIGHTER_LAYER | STATIC_MAP_LAYER | DESTRUCTIBLE_MAP_LAYER,
                ),
                ComposedProjectileRuntime {
                    owner_entity,
                    source_entity: owner_entity,
                    source,
                    delivery_index: 0,
                    velocity: Vec2::new(600.0, 0.0),
                    travelled: 0.0,
                    expires_at_tick,
                    maximum_range: 1_000.0,
                    landing: None,
                    recipe,
                },
            ))
            .id()
    }

    #[cfg(feature = "server")]
    fn run_sticky_sweep(app: &mut App) {
        app.world_mut().run_schedule(FixedPostUpdate);
    }

    #[cfg(feature = "server")]
    fn test_attack_source() -> AttackSource {
        AttackSource {
            kind: CombatSourceKind::PrimaryWeapon,
            attack_id: AttackId(41),
            player_id: PlayerId(3),
            owner_network_entity_id: NetworkEntityId(7),
            team_id: TeamId(0),
            recipe_fingerprint: WeaponRecipeFingerprint(11),
            legacy_compatibility: false,
            source_preset_id: Some(WeaponPresetId(1)),
            origin: WorldPoint::from(Vec2::ZERO),
            facing: 0.0,
        }
    }

    #[cfg(feature = "server")]
    fn target_damage(
        target: crate::map::DamageableTargetIdentity,
    ) -> crate::map::PendingWorldTargetDamage {
        crate::map::PendingWorldTargetDamage {
            target,
            source: test_attack_source(),
            attack_id: AttackId(41),
            requested_damage: 17,
            delivery_index: 2,
            bundle_index: 3,
            effect_index: 4,
        }
    }

    #[test]
    #[cfg(feature = "server")]
    fn projectile_world_hits_route_to_exactly_one_authority_owner() {
        use crate::map::{
            DamageableTargetIdentity, MapDynamicGeneration, MapInstanceId, MapPlacementId,
            ModeAnchorId,
        };
        use crate::matchplay::MatchId;

        let map_target = DamageableTargetIdentity::MapObject {
            generation: MapDynamicGeneration {
                map_instance_id: MapInstanceId(9),
                generation: 2,
            },
            placement_id: MapPlacementId(12),
        };
        let safe_target = DamageableTargetIdentity::HeistSafe {
            match_id: MatchId(13),
            anchor_id: ModeAnchorId(4),
            defending_team: TeamId(1),
        };
        let mut world = crate::map::PendingWorldTargetDamages::default();
        let mut objectives = crate::matchplay::PendingModeObjectiveDamages::default();

        queue_damageable_target(&mut world, &mut objectives, target_damage(map_target));
        queue_damageable_target(&mut world, &mut objectives, target_damage(safe_target));

        assert_eq!(world.0.len(), 1);
        assert_eq!(world.0[0].target, map_target);
        assert_eq!(objectives.0.len(), 1);
        assert_eq!(objectives.0[0].target, safe_target);
        assert_eq!(objectives.0[0].requested_damage, 17);
        assert_eq!(objectives.0[0].delivery_index, 2);
        assert_eq!(objectives.0[0].bundle_index, 3);
        assert_eq!(objectives.0[0].effect_index, 4);
    }

    #[test]
    #[cfg(feature = "server")]
    fn shared_area_plan_stably_orders_candidates_and_shares_one_target_budget() {
        use crate::map::{
            DamageableTargetIdentity, MapDynamicGeneration, MapInstanceId, MapPlacementId,
        };

        let mut world = World::new();
        let fighter = world.spawn_empty().id();
        let mut recipe = WeaponCatalog::embedded()
            .unwrap()
            .preset(WeaponPresetId(5))
            .unwrap()
            .configuration
            .recipe
            .clone();
        let bundle_index = recipe
            .payload_bundles
            .iter()
            .position(|bundle| matches!(bundle.target, TargetSelection::Area { .. }))
            .unwrap();
        recipe.payload_bundles[bundle_index].target = TargetSelection::Area {
            radius: 100.0,
            map_occlusion: false,
            max_targets: 2,
        };
        let generation = MapDynamicGeneration {
            map_instance_id: MapInstanceId(1),
            generation: 1,
        };
        let later_object = DamageableTargetIdentity::MapObject {
            generation,
            placement_id: MapPlacementId(2),
        };
        let earlier_object = DamageableTargetIdentity::MapObject {
            generation,
            placement_id: MapPlacementId(1),
        };
        let candidates = vec![AreaBundleCandidates {
            bundle_index,
            fighters: vec![AreaFighterCandidate {
                entity: fighter,
                position: Vec2::X,
                team: TeamId(1),
                network_id: NetworkEntityId(9),
                defeated: false,
                disconnected: false,
                line_of_sight_clear: true,
            }],
            // Deliberately reverse stable object order. The fighter consumes the first slot.
            objects: vec![
                AreaObjectCandidate {
                    position: Vec2::X * 3.0,
                    identity: later_object,
                    line_of_sight_clear: true,
                },
                AreaObjectCandidate {
                    position: Vec2::X * 2.0,
                    identity: earlier_object,
                    line_of_sight_clear: true,
                },
            ],
        }];

        let plan = plan_area_payloads(Vec2::ZERO, test_attack_source(), 2, &recipe, candidates);

        assert_eq!(plan.selected_targets, 2);
        assert_eq!(plan.payloads.len(), 1);
        assert_eq!(plan.payloads[0].target_network_id, NetworkEntityId(9));
        assert_eq!(plan.world_damages.len(), 1);
        assert_eq!(plan.world_damages[0].target, earlier_object);
    }

    #[test]
    #[cfg(feature = "server")]
    fn production_sweep_arms_an_expired_sticky_at_its_last_position() {
        let mut app = sticky_sweep_app(20);
        let owner_id = NetworkEntityId(70);
        let owner = spawn_sticky_owner(&mut app, owner_id);
        let projectile = spawn_sticky_projectile(
            &mut app,
            owner,
            sticky_attack_source(51, owner_id),
            Vec2::new(17.0, 9.0),
            20,
        );

        run_sticky_sweep(&mut app);

        assert!(app.world().get::<Projectile>(projectile).is_none());
        assert_eq!(
            app.world().get::<Position>(projectile).unwrap().0,
            Vec2::new(17.0, 9.0)
        );
        let state = app.world().get::<StickyBlobState>(projectile).unwrap();
        assert_eq!(state.kind, StickyBlobKind::Primary);
        assert_eq!(state.attached_to, None);
        assert_eq!(state.armed_at_tick, 20);
        assert_eq!(state.detonates_at_tick, 89);
    }

    #[test]
    #[cfg(feature = "server")]
    fn production_sweep_enforces_per_owner_and_global_sticky_caps() {
        let mut app = sticky_sweep_app(20);
        let owner_id = NetworkEntityId(70);
        let owner = spawn_sticky_owner(&mut app, owner_id);
        let recipe = sticky_recipe();
        for attack_id in 1..=6 {
            app.world_mut().spawn((
                StickyBlobState {
                    kind: StickyBlobKind::Primary,
                    attached_to: None,
                    armed_at_tick: 1,
                    detonates_at_tick: 100,
                    explosion_radius: 85.44,
                },
                StickyBlobRuntime {
                    source: sticky_attack_source(attack_id, owner_id),
                    delivery_index: 0,
                    recipe: recipe.clone(),
                },
            ));
        }
        let owner_capped = spawn_sticky_projectile(
            &mut app,
            owner,
            sticky_attack_source(80, owner_id),
            Vec2::ZERO,
            20,
        );

        run_sticky_sweep(&mut app);

        assert!(app.world().get_entity(owner_capped).is_err());

        let mut app = sticky_sweep_app(20);
        let owner_id = NetworkEntityId(70);
        let owner = spawn_sticky_owner(&mut app, owner_id);
        for index in 0..sticky::MAX_ACTIVE_STICKY_BLOBS {
            let existing_owner = NetworkEntityId(1_000 + index as u64);
            app.world_mut().spawn((
                StickyBlobState {
                    kind: StickyBlobKind::Primary,
                    attached_to: None,
                    armed_at_tick: 1,
                    detonates_at_tick: 100,
                    explosion_radius: 85.44,
                },
                StickyBlobRuntime {
                    source: sticky_attack_source(1_000 + index as u64, existing_owner),
                    delivery_index: 0,
                    recipe: recipe.clone(),
                },
            ));
        }
        let globally_capped = spawn_sticky_projectile(
            &mut app,
            owner,
            sticky_attack_source(90, owner_id),
            Vec2::ZERO,
            20,
        );

        run_sticky_sweep(&mut app);

        assert!(app.world().get_entity(globally_capped).is_err());
    }

    #[test]
    #[cfg(feature = "server")]
    fn production_sweep_attaches_and_chains_primary_stickies_on_one_carrier() {
        let mut app = sticky_sweep_app(20);
        let owner_id = NetworkEntityId(70);
        let owner = spawn_sticky_owner(&mut app, owner_id);
        let target_id = NetworkEntityId(71);
        app.world_mut().spawn((
            Fighter,
            Position(Vec2::new(8.0, 0.0)),
            TeamId(1),
            target_id,
            RigidBody::Static,
            Collider::circle(2.0),
            CollisionLayers::new(FIGHTER_LAYER, PROJECTILE_LAYER),
        ));
        let existing = app
            .world_mut()
            .spawn((
                Position(Vec2::new(8.0, 0.0)),
                StickyBlobState {
                    kind: StickyBlobKind::Primary,
                    attached_to: Some(target_id),
                    armed_at_tick: 1,
                    detonates_at_tick: 100,
                    explosion_radius: 85.44,
                },
                StickyBlobRuntime {
                    source: sticky_attack_source(50, owner_id),
                    delivery_index: 0,
                    recipe: sticky_recipe(),
                },
            ))
            .id();
        let incoming = spawn_sticky_projectile(
            &mut app,
            owner,
            sticky_attack_source(51, owner_id),
            Vec2::ZERO,
            100,
        );

        run_sticky_sweep(&mut app);

        assert_eq!(
            app.world()
                .get::<StickyBlobState>(existing)
                .unwrap()
                .detonates_at_tick,
            20
        );
        let incoming_state = app.world().get::<StickyBlobState>(incoming).unwrap();
        assert_eq!(incoming_state.attached_to, Some(target_id));
        assert_eq!(incoming_state.detonates_at_tick, 89);
    }

    #[test]
    #[cfg(feature = "client")]
    fn arc_height_peaks_at_half_progress() {
        assert!((lob_height(0.5, 140.0) - 140.0).abs() < 0.001);
    }
    #[test]
    #[cfg(feature = "server")]
    fn sector_includes_tangent_target_radius() {
        assert!(sector_contains(
            Vec2::ZERO,
            0.0,
            100.0,
            60.0,
            Vec2::new(90.0, 20.0),
            20.0
        ));
    }
    #[test]
    fn landing_repair_returns_furthest_clear_point() {
        let point =
            repaired_landing_point(Vec2::ZERO, Vec2::X * 20.0, 0.0, |p| p.x < 12.0).unwrap();
        assert!(point.x < 12.1 && point.x > 6.0);
    }
}
