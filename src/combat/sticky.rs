//! Server-authoritative armed Sticky Blomb attachment, fuse, and detonation lifecycle.

#![allow(clippy::wildcard_imports)]

use super::*;

pub(crate) const MAX_ACTIVE_STICKY_BLOBS: usize = 96;

pub(crate) struct StickyPlanningLedger {
    active_by_owner: HashMap<u64, usize>,
    active_total: usize,
    existing_primaries_by_target: HashMap<u64, Vec<(Entity, StickyBlobState)>>,
    newest_planned_primary_by_target: HashMap<u64, Entity>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StickyArmPlan {
    entity: Entity,
    position: Vec2,
    travelled: f32,
    state: StickyBlobState,
    runtime: StickyBlobRuntime,
    chain_writes: Vec<(Entity, StickyBlobState)>,
}

impl StickyPlanningLedger {
    pub(crate) fn from_active(
        blobs: &Query<(Entity, &StickyBlobState, &StickyBlobRuntime)>,
    ) -> Self {
        Self::from_snapshots(blobs.iter().map(|(entity, state, runtime)| {
            (entity, *state, runtime.source.owner_network_entity_id)
        }))
    }

    fn from_snapshots(
        blobs: impl IntoIterator<Item = (Entity, StickyBlobState, NetworkEntityId)>,
    ) -> Self {
        let mut active_by_owner = HashMap::new();
        let mut active_total = 0_usize;
        let mut existing_primaries_by_target =
            HashMap::<u64, Vec<(Entity, StickyBlobState)>>::new();
        for (entity, state, owner) in blobs {
            *active_by_owner.entry(owner.0).or_default() += 1;
            active_total = active_total.saturating_add(1);
            if matches!(state.kind, StickyBlobKind::Primary)
                && let Some(target) = state.attached_to
            {
                existing_primaries_by_target
                    .entry(target.0)
                    .or_default()
                    .push((entity, state));
            }
        }
        Self {
            active_by_owner,
            active_total,
            existing_primaries_by_target,
            newest_planned_primary_by_target: HashMap::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self::from_snapshots([])
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_plan_arm(
        &mut self,
        entity: Entity,
        runtime: &ComposedProjectileRuntime,
        position: Vec2,
        attached_to: Option<NetworkEntityId>,
        tick: u64,
        travelled: f32,
    ) -> Option<StickyArmPlan> {
        let (fuse_ticks, max_active, explosion_radius) =
            sticky_delivery_parameters(&runtime.recipe)?;
        let owner = runtime.source.owner_network_entity_id;
        if self
            .active_by_owner
            .get(&owner.0)
            .copied()
            .unwrap_or_default()
            >= usize::from(max_active)
            || self.active_total >= MAX_ACTIVE_STICKY_BLOBS
        {
            return None;
        }
        let kind = sticky_kind(runtime.source.kind);
        let mut chain_writes = Vec::new();
        if kind == StickyBlobKind::Primary
            && let Some(target) = attached_to
        {
            chain_writes.extend(
                self.existing_primaries_by_target
                    .get(&target.0)
                    .into_iter()
                    .flatten()
                    .map(|(existing, state)| {
                        (
                            *existing,
                            StickyBlobState {
                                detonates_at_tick: tick,
                                ..*state
                            },
                        )
                    }),
            );
            if let Some(previous) = self
                .newest_planned_primary_by_target
                .get(&target.0)
                .copied()
            {
                chain_writes.push((
                    previous,
                    StickyBlobState {
                        kind: StickyBlobKind::Primary,
                        attached_to: Some(target),
                        armed_at_tick: tick,
                        detonates_at_tick: tick,
                        explosion_radius,
                    },
                ));
            }
            self.newest_planned_primary_by_target
                .insert(target.0, entity);
        }
        *self.active_by_owner.entry(owner.0).or_default() += 1;
        self.active_total = self.active_total.saturating_add(1);
        Some(StickyArmPlan {
            entity,
            position,
            travelled,
            state: StickyBlobState {
                kind,
                attached_to,
                armed_at_tick: tick,
                detonates_at_tick: tick.saturating_add(fuse_ticks),
                explosion_radius,
            },
            runtime: StickyBlobRuntime {
                source: runtime.source,
                delivery_index: runtime.delivery_index,
                recipe: runtime.recipe.clone(),
            },
            chain_writes,
        })
    }
}

pub(crate) fn commit_arm_plan(
    commands: &mut Commands,
    projectile_runtime: &mut ComposedProjectileRuntime,
    plan: StickyArmPlan,
) {
    projectile_runtime.travelled = plan.travelled;
    for (entity, state) in plan.chain_writes {
        commands.entity(entity).insert(state);
    }
    commands
        .entity(plan.entity)
        .remove::<(
            Projectile,
            StraightFlight,
            ProjectileBody,
            ProjectileDeadline,
            Collider,
            CollisionLayers,
            ComposedProjectileRuntime,
        )>()
        .insert((Position(plan.position), plan.state, plan.runtime));
}

const fn sticky_kind(source: CombatSourceKind) -> StickyBlobKind {
    if matches!(source, CombatSourceKind::PrimaryWeapon) {
        StickyBlobKind::Primary
    } else {
        StickyBlobKind::UltimateSecondary
    }
}

#[must_use]
pub(crate) fn sticky_delivery_parameters(recipe: &WeaponRecipe) -> Option<(u64, u8, f32)> {
    let DeliveryMethod::StickyStraight {
        fuse_ticks,
        max_active_per_owner,
        ..
    } = recipe.delivery
    else {
        return None;
    };
    let explosion_radius = recipe.payload_bundles.iter().find_map(|bundle| {
        if let TargetSelection::Area { radius, .. } = bundle.target {
            Some(radius)
        } else {
            None
        }
    })?;
    Some((fuse_ticks, max_active_per_owner, explosion_radius))
}

pub(crate) fn arm_projectile(
    commands: &mut Commands,
    entity: Entity,
    runtime: ComposedProjectileRuntime,
    position: Vec2,
    attached_to: Option<NetworkEntityId>,
    kind: StickyBlobKind,
    tick: u64,
) -> bool {
    let Some((fuse_ticks, _, explosion_radius)) = sticky_delivery_parameters(&runtime.recipe)
    else {
        return false;
    };
    commands
        .entity(entity)
        .remove::<(
            Projectile,
            StraightFlight,
            ProjectileBody,
            ProjectileDeadline,
            Collider,
            CollisionLayers,
            ComposedProjectileRuntime,
        )>()
        .insert((
            Position(position),
            StickyBlobState {
                kind,
                attached_to,
                armed_at_tick: tick,
                detonates_at_tick: tick.saturating_add(fuse_ticks),
                explosion_radius,
            },
            StickyBlobRuntime {
                source: runtime.source,
                delivery_index: runtime.delivery_index,
                recipe: runtime.recipe,
            },
        ));
    true
}

#[allow(
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "the fixed-tick attachment pass owns the complete stable carrier lookup"
)]
pub(crate) fn advance_sticky_attachments(
    mut blobs: Query<
        (Entity, &mut Position, &mut StickyBlobState),
        (
            Without<Fighter>,
            Without<crate::abilities::Sentry>,
            Without<crate::map::DamageableWorldObject>,
            Without<crate::matchplay::HeistSafe>,
        ),
    >,
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
    disconnected: Query<Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
) {
    let disconnected: HashSet<_> = disconnected.iter().collect();
    let carriers: HashMap<_, _> = fighters
        .iter()
        .map(|(_, position, _, network_id, defeated, controlled)| {
            (
                network_id.0,
                (
                    position.0,
                    defeated.is_some()
                        || controlled.is_some_and(|owner| disconnected.contains(&owner.owner)),
                ),
            )
        })
        .collect();
    for (_, mut position, mut state) in &mut blobs {
        if let Some(carrier) = state.attached_to {
            if let Some((carrier_position, unavailable)) = carriers.get(&carrier.0).copied() {
                if unavailable {
                    state.attached_to = None;
                } else {
                    position.0 = carrier_position;
                }
            } else {
                state.attached_to = None;
            }
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "the fixed-tick detonation pass owns the shared area-payload view"
)]
pub(crate) fn detonate_sticky_blobs(
    tick: Res<SimulationTick>,
    mut pending: MessageWriter<PendingPayload>,
    mut deliveries: MessageWriter<PendingDelivery>,
    mut world_pending: ResMut<crate::map::PendingWorldTargetDamages>,
    mut objective_pending: ResMut<crate::matchplay::PendingModeObjectiveDamages>,
    blobs: Query<(Entity, &Position, &StickyBlobState, &StickyBlobRuntime)>,
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
    disconnected: Query<Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
    spatial_query: avian2d::prelude::SpatialQuery,
) {
    let disconnected: HashSet<_> = disconnected.iter().collect();
    let mut ordered: Vec<_> = blobs.iter().collect();
    ordered.sort_by_key(|(_, _, _, runtime)| (runtime.source.attack_id.0, runtime.delivery_index));
    for (entity, position, state, runtime) in ordered {
        if tick.0 < state.detonates_at_tick {
            continue;
        }
        let center = position.0;
        let _ = queue_area_payloads(
            center,
            runtime.source,
            runtime.delivery_index,
            &runtime.recipe,
            &fighters,
            &objects,
            &disconnected,
            &spatial_query,
            &mut pending,
            &mut world_pending,
            &mut objective_pending,
        );
        deliveries.write(PendingDelivery {
            entity: Some(entity),
            source: runtime.source,
            delivery_index: runtime.delivery_index,
            tick: tick.0,
            engagement_distance: 0.0,
            delivery_travel: runtime.source.origin.as_vec2().distance(center),
            kind: PendingDeliveryKind::StickyDetonated {
                position: center.into(),
            },
            world_effects: Vec::new(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sticky_runtime(
        owner_entity: Entity,
        owner: NetworkEntityId,
        attack_id: u64,
    ) -> ComposedProjectileRuntime {
        ComposedProjectileRuntime {
            owner_entity,
            source_entity: owner_entity,
            source: AttackSource {
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
            },
            delivery_index: 0,
            velocity: Vec2::X,
            travelled: 0.0,
            expires_at_tick: 100,
            maximum_range: 1_000.0,
            landing: None,
            recipe: WeaponCatalog::embedded()
                .unwrap()
                .preset(WeaponPresetId(5))
                .unwrap()
                .configuration
                .recipe
                .clone(),
        }
    }

    #[test]
    fn sticky_parameters_use_authored_fuse_ceiling_and_area_radius() {
        let recipe = WeaponCatalog::embedded()
            .unwrap()
            .preset(WeaponPresetId(5))
            .unwrap()
            .configuration
            .recipe
            .clone();
        assert_eq!(sticky_delivery_parameters(&recipe), Some((69, 6, 85.44)));
    }

    #[test]
    fn planning_ledger_owns_existing_and_same_batch_primary_chain_writes() {
        let mut world = World::new();
        let owner_entity = world.spawn_empty().id();
        let existing_entity = world.spawn_empty().id();
        let first_entity = world.spawn_empty().id();
        let second_entity = world.spawn_empty().id();
        let owner = NetworkEntityId(7);
        let carrier = NetworkEntityId(41);
        let existing = StickyBlobState {
            kind: StickyBlobKind::Primary,
            attached_to: Some(carrier),
            armed_at_tick: 10,
            detonates_at_tick: 79,
            explosion_radius: 85.44,
        };
        let mut ledger = StickyPlanningLedger::from_snapshots([(existing_entity, existing, owner)]);
        let first_runtime = sticky_runtime(owner_entity, owner, 51);
        let first = ledger
            .try_plan_arm(
                first_entity,
                &first_runtime,
                Vec2::X,
                Some(carrier),
                20,
                4.0,
            )
            .unwrap();
        assert_eq!(first.chain_writes.len(), 1);
        assert_eq!(first.chain_writes[0].0, existing_entity);
        assert_eq!(
            first.chain_writes[0].1,
            StickyBlobState {
                detonates_at_tick: 20,
                ..existing
            }
        );

        // Carrier chaining is intentionally cross-owner and follows sorted projectile order.
        let second_owner = NetworkEntityId(8);
        let second_runtime = sticky_runtime(owner_entity, second_owner, 52);
        let second = ledger
            .try_plan_arm(
                second_entity,
                &second_runtime,
                Vec2::X * 2.0,
                Some(carrier),
                20,
                8.0,
            )
            .unwrap();
        assert_eq!(second.chain_writes.len(), 2);
        assert_eq!(second.chain_writes[0].0, existing_entity);
        assert_eq!(second.chain_writes[1].0, first_entity);
        assert_eq!(second.chain_writes[1].1.armed_at_tick, 20);
        assert_eq!(second.chain_writes[1].1.detonates_at_tick, 20);
        assert!((second.chain_writes[1].1.explosion_radius - 85.44).abs() < f32::EPSILON);
        assert_eq!(second.state.detonates_at_tick, 89);
    }

    #[test]
    fn planning_ledger_rejects_owner_capacity_without_mutating_chain_state() {
        let mut world = World::new();
        let owner_entity = world.spawn_empty().id();
        let projectile = world.spawn_empty().id();
        let owner = NetworkEntityId(7);
        let snapshots = (0..6)
            .map(|_| {
                (
                    world.spawn_empty().id(),
                    StickyBlobState {
                        kind: StickyBlobKind::Primary,
                        attached_to: None,
                        armed_at_tick: 1,
                        detonates_at_tick: 70,
                        explosion_radius: 85.44,
                    },
                    owner,
                )
            })
            .collect::<Vec<_>>();
        let mut ledger = StickyPlanningLedger::from_snapshots(snapshots);
        let runtime = sticky_runtime(owner_entity, owner, 51);

        assert!(
            ledger
                .try_plan_arm(projectile, &runtime, Vec2::ZERO, None, 20, 4.0)
                .is_none()
        );
    }

    #[test]
    fn attached_blob_follows_a_live_carrier_and_anchors_on_defeat() {
        let mut app = App::new();
        app.add_systems(Update, advance_sticky_attachments);
        let carrier_id = NetworkEntityId(73);
        let carrier = app
            .world_mut()
            .spawn((
                Fighter,
                Position(Vec2::new(25.0, 30.0)),
                TeamId(2),
                carrier_id,
            ))
            .id();
        let blob = app
            .world_mut()
            .spawn((
                Position(Vec2::ZERO),
                StickyBlobState {
                    kind: StickyBlobKind::Primary,
                    attached_to: Some(carrier_id),
                    armed_at_tick: 1,
                    detonates_at_tick: 70,
                    explosion_radius: 85.44,
                },
            ))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<Position>(blob).unwrap().0,
            Vec2::new(25.0, 30.0)
        );
        app.world_mut().entity_mut(carrier).insert(Defeated {
            event_id: CombatEventId(1),
        });
        app.world_mut()
            .entity_mut(carrier)
            .insert(Position(Vec2::new(40.0, 50.0)));
        app.update();

        let state = app.world().get::<StickyBlobState>(blob).unwrap();
        assert_eq!(state.attached_to, None);
        assert_eq!(
            app.world().get::<Position>(blob).unwrap().0,
            Vec2::new(25.0, 30.0)
        );
    }
}
