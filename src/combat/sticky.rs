//! Server-authoritative armed Sticky Blomb attachment, fuse, and detonation lifecycle.

#![allow(clippy::wildcard_imports)]

use super::*;

pub(crate) const MAX_ACTIVE_STICKY_BLOBS: usize = 96;

pub(crate) struct StickySweepState {
    active_by_owner: HashMap<u64, usize>,
    active_total: usize,
    newly_attached_primaries: HashMap<u64, Entity>,
}

impl StickySweepState {
    pub(crate) fn from_active(blobs: &Query<(&mut StickyBlobState, &StickyBlobRuntime)>) -> Self {
        let mut active_by_owner = HashMap::new();
        let mut active_total = 0_usize;
        for (_, runtime) in blobs {
            *active_by_owner
                .entry(runtime.source.owner_network_entity_id.0)
                .or_default() += 1;
            active_total = active_total.saturating_add(1);
        }
        Self {
            active_by_owner,
            active_total,
            newly_attached_primaries: HashMap::new(),
        }
    }

    pub(crate) fn try_arm_expired(
        &mut self,
        commands: &mut Commands,
        entity: Entity,
        runtime: &ComposedProjectileRuntime,
        position: Vec2,
        tick: u64,
    ) -> bool {
        if !self.has_capacity(runtime) {
            return false;
        }
        let kind = sticky_kind(runtime.source.kind);
        if !arm_projectile(
            commands,
            entity,
            runtime.clone(),
            position,
            None,
            kind,
            tick,
        ) {
            return false;
        }
        self.record_arm(runtime.source.owner_network_entity_id);
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_arm_impact(
        &mut self,
        commands: &mut Commands,
        entity: Entity,
        runtime: &ComposedProjectileRuntime,
        armed_position: Vec2,
        attached_to: Option<NetworkEntityId>,
        tick: u64,
        blobs: &mut Query<(&mut StickyBlobState, &StickyBlobRuntime)>,
    ) -> bool {
        if !self.has_capacity(runtime) {
            return false;
        }
        let kind = sticky_kind(runtime.source.kind);
        if kind == StickyBlobKind::Primary
            && let Some(target) = attached_to
        {
            for (mut existing, _) in blobs.iter_mut() {
                if primary_impact_triggers_existing(kind, *existing, target) {
                    existing.detonates_at_tick = tick;
                }
            }
            if let Some(previous) = self.newly_attached_primaries.get(&target.0).copied() {
                let (_, _, explosion_radius) = sticky_delivery_parameters(&runtime.recipe)
                    .expect("capacity check requires a validated sticky recipe");
                commands.entity(previous).insert(StickyBlobState {
                    kind: StickyBlobKind::Primary,
                    attached_to: Some(target),
                    armed_at_tick: tick,
                    detonates_at_tick: tick,
                    explosion_radius,
                });
            }
        }
        if !arm_projectile(
            commands,
            entity,
            runtime.clone(),
            armed_position,
            attached_to,
            kind,
            tick,
        ) {
            return false;
        }
        if kind == StickyBlobKind::Primary
            && let Some(target) = attached_to
        {
            self.newly_attached_primaries.insert(target.0, entity);
        }
        self.record_arm(runtime.source.owner_network_entity_id);
        true
    }

    fn has_capacity(&self, runtime: &ComposedProjectileRuntime) -> bool {
        let Some((_, max_active, _)) = sticky_delivery_parameters(&runtime.recipe) else {
            return false;
        };
        self.active_by_owner
            .get(&runtime.source.owner_network_entity_id.0)
            .copied()
            .unwrap_or_default()
            < usize::from(max_active)
            && self.active_total < MAX_ACTIVE_STICKY_BLOBS
    }

    fn record_arm(&mut self, owner: NetworkEntityId) {
        *self.active_by_owner.entry(owner.0).or_default() += 1;
        self.active_total = self.active_total.saturating_add(1);
    }
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

#[must_use]
pub(crate) const fn primary_impact_triggers_existing(
    incoming: StickyBlobKind,
    existing: StickyBlobState,
    target: NetworkEntityId,
) -> bool {
    matches!(incoming, StickyBlobKind::Primary)
        && matches!(existing.kind, StickyBlobKind::Primary)
        && matches!(existing.attached_to, Some(attached) if attached.0 == target.0)
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
    fn only_primary_hits_chain_an_existing_primary_on_the_same_carrier() {
        let carrier = NetworkEntityId(41);
        let existing = StickyBlobState {
            kind: StickyBlobKind::Primary,
            attached_to: Some(carrier),
            armed_at_tick: 10,
            detonates_at_tick: 79,
            explosion_radius: 85.44,
        };
        assert!(primary_impact_triggers_existing(
            StickyBlobKind::Primary,
            existing,
            carrier
        ));
        assert!(!primary_impact_triggers_existing(
            StickyBlobKind::UltimateSecondary,
            existing,
            carrier
        ));
        assert!(!primary_impact_triggers_existing(
            StickyBlobKind::Primary,
            existing,
            NetworkEntityId(42)
        ));
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
