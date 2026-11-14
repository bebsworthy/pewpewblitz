//! Cosmetic straight projectiles for the local player's input-frame shots.
//!
//! When the controlled fighter's shot is presented locally (`client::local_attack`), each
//! straight-family delivery gets a client-only cosmetic built with the same projectile geometry
//! and style as replicated projectiles. It flies from the muzzle at the recipe speed and stops at
//! the first effective projectile-blocking map geometry, the recipe range, or the recipe
//! lifetime; it never tests fighters, deployables, or damage.
//!
//! When the replicated projectile for the same (echoed attack, delivery) appears, its visual
//! starts exactly at the cosmetic's pose and carries a [`LocalProjectileLead`]: it keeps flying at
//! the projectile speed and converges on the authoritative pose no faster than
//! [`LOCAL_PROJECTILE_CORRECTION_SPEED_FRACTION`] of that speed, so the swap has no pop and never
//! reverses. A rejected (or never handed-off) cosmetic shrinks out within
//! [`LOCAL_PROJECTILE_FADE_SECONDS`]. Replicated visuals of the local player's projectiles are
//! hidden while their cosmetic already finished its flight, or while a local shot is unconfirmed.

#[allow(
    clippy::wildcard_imports,
    reason = "the world-renderer submodule consumes the presentation_3d composition surface"
)]
use super::super::*;
use super::common::CombatVfxContext;
use crate::local_attack::{
    LocalAttackKind, LocalAttackLedger, LocalAttackPresented, LocalShotId, LocalShotStatus,
};
use brawler::combat::client::{AimTraceBlockerIndex, straight_map_travel};
use brawler::combat::{CombatCue, DeduplicatedCombatCue, DeliveryMethod, FiringPattern};
use std::collections::VecDeque;

/// Simultaneous cosmetic projectiles (three sixteen-bolt spreads).
pub(crate) const MAX_LOCAL_PROJECTILES: usize = 48;
/// Seconds a rejected or orphaned cosmetic projectile takes to shrink out.
pub(crate) const LOCAL_PROJECTILE_FADE_SECONDS: f32 = 0.15;
/// Authored hand-off tolerance: the converging visual's correction speed, as a fraction of the
/// projectile speed. Below one, the handed-off visual always keeps moving forward.
pub(crate) const LOCAL_PROJECTILE_CORRECTION_SPEED_FRACTION: f32 = 0.5;
/// Seconds a confirmed cosmetic waits for its replicated projectile before fading (the server
/// may resolve a point-blank delivery before it is ever replicated).
const CONFIRMED_HANDOFF_WAIT_SECONDS: f32 = 0.25;
/// Retained hand-off outcomes (bounded FIFO).
const MAX_HANDOFF_RECORDS: usize = 128;
/// Request identity of locally presented projectiles; disjoint from server attack ids.
const LOCAL_PROJECTILE_IDENTITY: u64 = 1 << 63;

/// Client-only cosmetic flight of one delivery of a locally presented shot.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(crate) struct LocalCosmeticProjectile {
    shot: LocalShotId,
    delivery_index: u8,
    muzzle: Vec2,
    direction: Vec2,
    speed: f32,
    travel_limit: f32,
    elapsed: f32,
    confirmed_for: f32,
    fade_remaining: Option<f32>,
}

impl LocalCosmeticProjectile {
    fn travelled(&self) -> f32 {
        (self.speed * self.elapsed).min(self.travel_limit)
    }

    fn position(&self) -> Vec2 {
        self.muzzle + self.direction * self.travelled()
    }

    fn finished(&self) -> bool {
        self.speed * self.elapsed >= self.travel_limit
    }

    fn lead(&self) -> LocalProjectileLead {
        LocalProjectileLead {
            direction: self.direction,
            speed: self.speed,
            stop_point: self.muzzle + self.direction * self.travel_limit,
        }
    }
}

/// How a handed-off replicated visual keeps the cosmetic's motion while it converges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LocalProjectileLead {
    direction: Vec2,
    speed: f32,
    /// The cosmetic's traced map stop; the visual hides there instead of entering cover.
    stop_point: Vec2,
}

/// One presentation step of a handed-off visual.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LedProjectileStep {
    pub position: Vec2,
    pub converged: bool,
    pub reached_stop: bool,
}

/// Advance a handed-off visual: continue at the projectile speed, then pull toward the
/// authoritative pose by at most the authored correction speed.
#[must_use]
pub(crate) fn advance_led_projectile(
    presented: Vec2,
    authoritative: Vec2,
    lead: LocalProjectileLead,
    delta_seconds: f32,
) -> LedProjectileStep {
    let advanced = presented + lead.direction * lead.speed * delta_seconds;
    let maximum_correction =
        lead.speed * LOCAL_PROJECTILE_CORRECTION_SPEED_FRACTION * delta_seconds.max(0.0);
    let error = authoritative - advanced;
    let (position, converged) = if error.length() <= maximum_correction {
        (authoritative, true)
    } else {
        (advanced + error.normalize() * maximum_correction, false)
    };
    LedProjectileStep {
        position,
        converged,
        reached_stop: (position - lead.stop_point).dot(lead.direction) >= 0.0,
    }
}

/// Scale of a fading cosmetic: shrinks linearly to nothing.
fn fade_scale(remaining: Option<f32>) -> f32 {
    remaining.map_or(1.0, |remaining| {
        (remaining / LOCAL_PROJECTILE_FADE_SECONDS).clamp(0.0, 1.0)
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HandoffRecord {
    /// The replicated visual took over the cosmetic's pose.
    Led,
    /// The cosmetic finished its flight before the replicated projectile appeared.
    Completed,
}

/// Bounded outcomes of cosmetic deliveries, keyed by (local shot, delivery index).
#[derive(Resource, Default, Debug)]
pub(crate) struct LocalProjectileHandoffs {
    records: VecDeque<((LocalShotId, u8), HandoffRecord)>,
}

impl LocalProjectileHandoffs {
    fn record(&mut self, key: (LocalShotId, u8), record: HandoffRecord) {
        if let Some(existing) = self
            .records
            .iter_mut()
            .find(|(candidate, _)| *candidate == key)
        {
            existing.1 = record;
            return;
        }
        self.records.push_back((key, record));
        while self.records.len() > MAX_HANDOFF_RECORDS {
            self.records.pop_front();
        }
    }

    fn get(&self, key: (LocalShotId, u8)) -> Option<HandoffRecord> {
        self.records
            .iter()
            .find(|(candidate, _)| *candidate == key)
            .map(|(_, record)| *record)
    }
}

/// Straight-family launch facts of one recipe.
#[derive(Clone, Copy, Debug, PartialEq)]
struct StraightLaunch {
    speed: f32,
    body: brawler::combat::ProjectileBody,
    /// Flight distance from the muzzle allowed by the recipe range and lifetime.
    flight_limit: f32,
    muzzle_offset: f32,
}

#[allow(
    clippy::cast_precision_loss,
    reason = "authored projectile lifetimes are a few hundred ticks, exact in f32"
)]
fn straight_launch(recipe: &brawler::combat::WeaponRecipe) -> Option<StraightLaunch> {
    let (DeliveryMethod::Straight {
        speed,
        radius,
        range,
        lifetime_ticks,
        muzzle_offset,
    }
    | DeliveryMethod::StickyStraight {
        speed,
        radius,
        range,
        lifetime_ticks,
        muzzle_offset,
        ..
    }) = recipe.delivery
    else {
        return None;
    };
    let lifetime_seconds = brawler::timing::SIMULATION_TICK.as_secs_f32() * lifetime_ticks as f32;
    Some(StraightLaunch {
        speed,
        body: brawler::combat::ProjectileBody::circle(radius),
        flight_limit: range.min(speed * lifetime_seconds),
        muzzle_offset,
    })
}

fn delivery_directions(facing: f32, firing: FiringPattern) -> Vec<f32> {
    match firing {
        FiringPattern::Single => vec![facing],
        FiringPattern::Spread {
            delivery_count,
            total_angle_degrees,
        } => brawler::combat::spread_angles(facing, delivery_count, total_angle_degrees),
    }
}

/// Spawn one cosmetic projectile per unblocked straight delivery of each local shot.
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "cosmetic materialization joins the local shot, the replicated recipe and map, and retained render assets"
)]
pub(crate) fn spawn_local_projectiles(
    mut commands: Commands,
    mut local_attacks: MessageReader<LocalAttackPresented>,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    vfx: CombatVfxContext,
    mut blockers: ResMut<AimTraceBlockerIndex>,
    catalog: Option<Res<brawler::map::MapCatalogResource>>,
    maps: Query<
        (
            &brawler::map::ResolvedMapSnapshot,
            &brawler::map::MapDynamicState,
        ),
        With<brawler::map::MapRoot>,
    >,
    loadouts: Query<&brawler::builds::ResolvedMatchLoadout>,
    existing: Query<(), With<LocalCosmeticProjectile>>,
) {
    let mut active = existing.iter().count();
    let mut refreshed = false;
    for local in local_attacks.read() {
        let LocalAttackKind::PrimaryFire(shot) = local.kind else {
            continue;
        };
        let Ok(loadout) = loadouts.get(local.fighter) else {
            continue;
        };
        let recipe = &loadout.primary_weapon.recipe;
        let Some(launch) = straight_launch(recipe) else {
            continue;
        };
        if !refreshed
            && let (Some(catalog), Ok((snapshot, state))) = (catalog.as_deref(), maps.single())
        {
            blockers.refresh(snapshot, state, &catalog.0);
            refreshed = true;
        }
        for (index, angle) in delivery_directions(local.facing, recipe.firing)
            .into_iter()
            .enumerate()
        {
            if active >= MAX_LOCAL_PROJECTILES {
                return;
            }
            let direction = Vec2::from_angle(angle);
            let traced = straight_map_travel(
                &blockers,
                launch.body,
                local.origin,
                direction,
                launch.muzzle_offset + launch.flight_limit,
            );
            if traced < launch.muzzle_offset {
                // The server blocks this delivery at the muzzle; there is no projectile to show.
                continue;
            }
            let cosmetic = LocalCosmeticProjectile {
                shot,
                delivery_index: u8::try_from(index).unwrap_or(u8::MAX),
                muzzle: local.origin + direction * launch.muzzle_offset,
                direction,
                speed: launch.speed,
                travel_limit: traced - launch.muzzle_offset,
                elapsed: 0.0,
                confirmed_for: 0.0,
                fade_remaining: None,
            };
            let signature = vfx.cache().signature_for_local_attack(
                local.owner,
                local.team,
                recipe,
                vfx::VfxCarrier::StraightProjectile,
                LOCAL_PROJECTILE_IDENTITY | shot.0,
                cosmetic.delivery_index,
                vfx::VfxPhase::Travel,
                vfx::VfxGeometry::Projectile {
                    radius: launch.body.shape.bounding_radius(),
                },
                vfx::VfxAuthoritativeTiming {
                    activated_at_tick: Some(local.simulation_tick),
                    observed_at_tick: None,
                    expires_at_tick: None,
                },
                vfx.variant(),
            );
            spawn_cosmetic(
                &mut commands,
                (&primitives, &materials),
                local,
                launch,
                cosmetic,
                signature,
            );
            active += 1;
        }
    }
}

/// Materialize one cosmetic with the replicated projectile's geometry and style.
fn spawn_cosmetic(
    commands: &mut Commands,
    (primitives, materials): (&Primitive3dAssets, &Material3dAssets),
    local: &LocalAttackPresented,
    launch: StraightLaunch,
    cosmetic: LocalCosmeticProjectile,
    signature: vfx::CombatVisualSignature,
) {
    let facing = cosmetic.direction.to_angle();
    let flight = brawler::combat::StraightFlight {
        origin: brawler::combat::WorldPoint::from(cosmetic.muzzle),
        facing,
        speed: launch.speed,
        maximum_range: launch.flight_limit,
        launched_at_tick: local.simulation_tick,
    };
    let (root, _) = super::entities::spawn_projectile(
        commands,
        primitives,
        materials,
        None,
        false,
        local.fighter,
        cosmetic.muzzle,
        local.team,
        Some(&flight),
        Some(&launch.body),
        None,
        signature,
    );
    commands.entity(root).insert((
        cosmetic,
        Transform::from_translation(
            ground_position(cosmetic.muzzle) + Vec3::Y * STRAIGHT_PROJECTILE_HEIGHT,
        )
        .with_rotation(ground_rotation(Rotation::radians(facing))),
        Name::new("Local input-frame cosmetic projectile"),
    ));
}

/// Retire a cosmetic as soon as authority reports its delivery's impact.
#[allow(
    clippy::needless_pass_by_value,
    reason = "the ledger is a read-only Bevy system resource"
)]
pub(crate) fn retire_impacted_local_projectiles(
    mut commands: Commands,
    mut cues: MessageReader<DeduplicatedCombatCue>,
    ledger: Option<Res<LocalAttackLedger>>,
    mut handoffs: ResMut<LocalProjectileHandoffs>,
    cosmetics: Query<(Entity, &LocalCosmeticProjectile)>,
) {
    let Some(ledger) = ledger else {
        cues.clear();
        return;
    };
    for DeduplicatedCombatCue(cue) in cues.read() {
        let CombatCue::DeliveryImpact {
            attack_id,
            delivery_index,
            ..
        } = cue
        else {
            continue;
        };
        let Some(shot) = ledger.echo_shot(*attack_id) else {
            continue;
        };
        for (entity, cosmetic) in &cosmetics {
            if cosmetic.shot == shot && cosmetic.delivery_index == *delivery_index {
                handoffs.record((shot, *delivery_index), HandoffRecord::Completed);
                commands.entity(entity).try_despawn();
            }
        }
    }
}

type ReplicatedLocalProjectileQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static brawler::combat::ProjectileSource,
        &'static brawler::combat::AttackDelivery,
        Option<&'static brawler::combat::ReplicatedAttackSource>,
    ),
    (
        With<brawler::combat::Projectile>,
        With<brawler::combat::StraightFlight>,
    ),
>;

type ReplicatedProjectileVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static CombatVisualOwner,
        &'static mut V3ProjectileVisual,
        &'static mut Visibility,
    ),
    Without<LocalCosmeticProjectile>,
>;

/// Fly, fade, and hand off cosmetic projectiles, and gate the local player's replicated
/// projectile visuals, before the replicated projectile poses are written.
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "the hand-off is one atomic presentation step over cosmetic and replicated visuals"
)]
pub(crate) fn advance_local_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    ledger: Option<Res<LocalAttackLedger>>,
    mut handoffs: ResMut<LocalProjectileHandoffs>,
    mut cosmetics: Query<(Entity, &mut LocalCosmeticProjectile, &mut Transform)>,
    controlled: Query<&NetworkEntityId, (With<Fighter>, With<Controlled>)>,
    replicated: ReplicatedLocalProjectileQuery,
    mut visuals: ReplicatedProjectileVisualQuery,
) {
    let delta = time.delta_secs();
    let mut flying = Vec::new();
    for (entity, mut cosmetic, mut transform) in &mut cosmetics {
        cosmetic.elapsed += delta;
        let status = ledger
            .as_deref()
            .and_then(|ledger| ledger.status(cosmetic.shot));
        if matches!(status, Some(LocalShotStatus::Confirmed(_))) {
            cosmetic.confirmed_for += delta;
        }
        let orphaned = matches!(status, None | Some(LocalShotStatus::Rejected))
            || cosmetic.confirmed_for > CONFIRMED_HANDOFF_WAIT_SECONDS;
        if orphaned && cosmetic.fade_remaining.is_none() {
            cosmetic.fade_remaining = Some(LOCAL_PROJECTILE_FADE_SECONDS);
        }
        if let Some(remaining) = cosmetic.fade_remaining.as_mut() {
            *remaining -= delta;
            if *remaining <= 0.0 {
                commands.entity(entity).try_despawn();
                continue;
            }
        } else if cosmetic.finished() {
            handoffs.record(
                (cosmetic.shot, cosmetic.delivery_index),
                HandoffRecord::Completed,
            );
            commands.entity(entity).try_despawn();
            continue;
        }
        transform.translation =
            ground_position(cosmetic.position()) + Vec3::Y * STRAIGHT_PROJECTILE_HEIGHT;
        transform.scale = Vec3::splat(fade_scale(cosmetic.fade_remaining));
        if cosmetic.fade_remaining.is_none() {
            flying.push((entity, *cosmetic));
        }
    }

    let Some(ledger) = ledger else {
        return;
    };
    let Ok(owner) = controlled.single() else {
        return;
    };
    let awaiting_confirmation = ledger.unconfirmed() > 0;
    for (visual_owner, mut visual, mut visibility) in &mut visuals {
        let Ok((position, source, delivery, attack_source)) = replicated.get(visual_owner.0) else {
            continue;
        };
        // Only the local player's primary-weapon projectiles have local presentations; its
        // sentry and ultimate projectiles keep their ordinary replicated visuals.
        if source.owner_network_entity_id != *owner
            || attack_source.is_some_and(|attack_source| {
                attack_source.attack.kind != brawler::combat::CombatSourceKind::PrimaryWeapon
            })
        {
            continue;
        }
        let Some(shot) = ledger.echo_shot(delivery.attack_id) else {
            // Possibly the echo of a shot whose cue has not been paired yet: wait unseen.
            visibility.set_if_neq(if awaiting_confirmation && !visual.local_handoff_applied {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            });
            continue;
        };
        let key = (shot, delivery.delivery_index);
        if visual.local_handoff_applied {
            continue;
        }
        if let Some(&(cosmetic_entity, cosmetic)) = flying.iter().find(|(_, cosmetic)| {
            cosmetic.shot == shot && cosmetic.delivery_index == delivery.delivery_index
        }) {
            // The pose writer advances the visual by this frame's flight next, landing exactly
            // on the cosmetic's pose for this frame: the swap shows no discontinuity.
            visual.planar_position =
                cosmetic.position() - cosmetic.direction * cosmetic.speed * delta;
            visual.local_lead = Some(cosmetic.lead());
            visual.local_handoff_applied = true;
            visibility.set_if_neq(Visibility::Inherited);
            handoffs.record(key, HandoffRecord::Led);
            commands.entity(cosmetic_entity).try_despawn();
            continue;
        }
        match handoffs.get(key) {
            Some(HandoffRecord::Completed) => {
                visibility.set_if_neq(Visibility::Hidden);
                visual.local_handoff_applied = true;
            }
            Some(HandoffRecord::Led) => {
                // A replaced visual root of an already handed-off projectile: continue from the
                // authoritative pose instead of re-firing from the muzzle.
                visual.planar_position = position.0;
                visual.local_handoff_applied = true;
                visibility.set_if_neq(Visibility::Inherited);
            }
            None => {
                visibility.set_if_neq(Visibility::Inherited);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lead() -> LocalProjectileLead {
        LocalProjectileLead {
            direction: Vec2::X,
            speed: 600.0,
            stop_point: Vec2::new(1_000.0, 0.0),
        }
    }

    #[test]
    fn handed_off_visual_never_moves_backward_and_converges_on_authority() {
        let delta = 1.0 / 60.0;
        // The cosmetic leads authority by 120 units (about 200 ms of flight).
        let mut presented = Vec2::new(220.0, 0.0);
        let mut authoritative = Vec2::new(100.0, 0.0);
        let mut previous = presented;
        let mut frames = 0;
        loop {
            authoritative += Vec2::X * 600.0 * delta;
            let step = advance_led_projectile(presented, authoritative, lead(), delta);
            let moved = step.position - previous;
            assert!(moved.x > 0.0, "always forward");
            let correction = (step.position - (previous + Vec2::X * 600.0 * delta)).length();
            assert!(
                correction <= 600.0 * LOCAL_PROJECTILE_CORRECTION_SPEED_FRACTION * delta + 1e-3,
                "correction within the authored tolerance"
            );
            previous = step.position;
            presented = step.position;
            frames += 1;
            if step.converged {
                assert_eq!(step.position, authoritative);
                break;
            }
            assert!(frames < 60, "converges within a second");
        }
    }

    #[test]
    fn handed_off_visual_hides_at_the_cosmetic_map_stop() {
        let step =
            advance_led_projectile(Vec2::new(995.0, 0.0), Vec2::new(900.0, 0.0), lead(), 0.02);
        assert!(step.reached_stop);
        let step =
            advance_led_projectile(Vec2::new(500.0, 0.0), Vec2::new(400.0, 0.0), lead(), 0.02);
        assert!(!step.reached_stop);
    }

    #[test]
    fn cosmetic_flight_is_bounded_by_its_traced_limit_and_fade_is_bounded() {
        let cosmetic = LocalCosmeticProjectile {
            shot: LocalShotId(1),
            delivery_index: 0,
            muzzle: Vec2::new(10.0, 0.0),
            direction: Vec2::X,
            speed: 600.0,
            travel_limit: 90.0,
            elapsed: 1.0,
            confirmed_for: 0.0,
            fade_remaining: None,
        };
        assert!(cosmetic.finished());
        assert_eq!(cosmetic.position(), Vec2::new(100.0, 0.0));
        assert_eq!(cosmetic.lead().stop_point, Vec2::new(100.0, 0.0));
        assert!((fade_scale(Some(LOCAL_PROJECTILE_FADE_SECONDS)) - 1.0).abs() < 1e-6);
        assert!(fade_scale(Some(0.0)).abs() < 1e-6);
        assert!((fade_scale(None) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn handoff_records_are_bounded_and_updatable() {
        let mut handoffs = LocalProjectileHandoffs::default();
        for shot in 0..(MAX_HANDOFF_RECORDS as u64 * 2) {
            handoffs.record((LocalShotId(shot), 0), HandoffRecord::Led);
        }
        assert_eq!(handoffs.records.len(), MAX_HANDOFF_RECORDS);
        let key = (LocalShotId(MAX_HANDOFF_RECORDS as u64 * 2 - 1), 0);
        handoffs.record(key, HandoffRecord::Completed);
        assert_eq!(handoffs.get(key), Some(HandoffRecord::Completed));
        assert_eq!(handoffs.records.len(), MAX_HANDOFF_RECORDS);
    }

    use crate::local_attack::LocalAttackLedger;
    use brawler::combat::{AttackDelivery, AttackId, ProjectileSource, ShotId, StraightFlight};

    const DT: f32 = 1.0 / 60.0;
    const OWNER: NetworkEntityId = NetworkEntityId(12);

    fn lifecycle_app(ledger: LocalAttackLedger) -> App {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(ledger)
            .init_resource::<LocalProjectileHandoffs>()
            .add_systems(
                Update,
                (
                    advance_local_projectiles,
                    super::super::entities::write_projectile_visual_poses,
                )
                    .chain(),
            );
        app.world_mut().spawn((Fighter, Controlled, OWNER));
        app
    }

    fn frame(app: &mut App) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(DT));
        app.update();
    }

    fn cosmetic(shot: LocalShotId) -> (LocalCosmeticProjectile, Transform) {
        (
            LocalCosmeticProjectile {
                shot,
                delivery_index: 0,
                muzzle: Vec2::new(30.0, 0.0),
                direction: Vec2::X,
                speed: 600.0,
                travel_limit: 500.0,
                elapsed: 0.0,
                confirmed_for: 0.0,
                fade_remaining: None,
            },
            Transform::default(),
        )
    }

    #[test]
    fn rejected_cosmetic_shrinks_out_and_leaves_no_ghost_after_the_bounded_fade() {
        let mut ledger = LocalAttackLedger::default();
        let shot = ledger.record(lightyear::prelude::Tick(1), 100);
        ledger.expire(None, lightyear::prelude::Tick(500));
        let mut app = lifecycle_app(ledger);
        let entity = app.world_mut().spawn(cosmetic(shot)).id();
        frame(&mut app);
        let scale = app
            .world()
            .get::<Transform>(entity)
            .expect("fading")
            .scale
            .x;
        assert!(scale < 1.0 && scale > 0.0, "shrinking: {scale}");
        let mut frames = 1_u8;
        while app.world().get_entity(entity).is_ok() {
            frame(&mut app);
            frames += 1;
            assert!(frames < 60, "the fade must end");
        }
        assert!(
            f32::from(frames) * DT <= LOCAL_PROJECTILE_FADE_SECONDS + DT,
            "despawned within the bounded fade ({frames} frames)"
        );
    }

    #[test]
    fn replicated_projectile_takes_over_the_cosmetic_pose_without_a_pop() {
        let mut ledger = LocalAttackLedger::default();
        let shot = ledger.record(lightyear::prelude::Tick(1), 100);
        ledger.observe_own_attack(AttackId(11), 100);
        let mut app = lifecycle_app(ledger);
        let cosmetic_entity = app.world_mut().spawn(cosmetic(shot)).id();
        // The cosmetic flies ten frames before the delayed replicated projectile appears.
        for _ in 0..10 {
            frame(&mut app);
        }
        let before = {
            let cosmetic = app
                .world()
                .get::<LocalCosmeticProjectile>(cosmetic_entity)
                .expect("still flying");
            cosmetic.position()
        };
        let projectile = app
            .world_mut()
            .spawn((
                brawler::combat::Projectile,
                StraightFlight {
                    origin: brawler::combat::WorldPoint::from(Vec2::new(30.0, 0.0)),
                    facing: 0.0,
                    speed: 600.0,
                    maximum_range: 500.0,
                    launched_at_tick: 100,
                },
                Position(Vec2::new(40.0, 0.0)),
                Rotation::default(),
                ProjectileSource {
                    shot_id: ShotId(11),
                    player_id: PlayerId(1),
                    owner_network_entity_id: OWNER,
                    team_id: brawler::combat::TeamId(1),
                    weapon_definition_id: brawler::combat::WeaponDefinitionId(1),
                },
                AttackDelivery {
                    attack_id: AttackId(11),
                    delivery_index: 0,
                },
            ))
            .id();
        let visual = app
            .world_mut()
            .spawn((
                CombatVisualOwner(projectile),
                V3ProjectileVisual {
                    planar_position: Vec2::new(30.0, 0.0),
                    local_lead: None,
                    local_handoff_applied: false,
                },
                Transform::default(),
                Visibility::default(),
            ))
            .id();
        frame(&mut app);
        assert!(
            app.world().get_entity(cosmetic_entity).is_err(),
            "the cosmetic retires at the swap"
        );
        let presented = app
            .world()
            .get::<V3ProjectileVisual>(visual)
            .expect("visual")
            .planar_position;
        let expected = before + Vec2::X * 600.0 * DT;
        assert!(
            presented.distance(expected)
                <= 600.0 * LOCAL_PROJECTILE_CORRECTION_SPEED_FRACTION * DT + 1e-3,
            "hand-off continuity: presented {presented}, cosmetic continuation {expected}"
        );
        assert_eq!(
            app.world().get::<Visibility>(visual),
            Some(&Visibility::Inherited)
        );
    }

    #[test]
    fn replicated_visual_of_a_finished_cosmetic_stays_hidden() {
        let mut ledger = LocalAttackLedger::default();
        let shot = ledger.record(lightyear::prelude::Tick(1), 100);
        let mut app = lifecycle_app(ledger);
        let (mut stopped, transform) = cosmetic(shot);
        stopped.travel_limit = 5.0;
        let cosmetic_entity = app.world_mut().spawn((stopped, transform)).id();
        frame(&mut app);
        assert!(
            app.world().get_entity(cosmetic_entity).is_err(),
            "stopped at its map stop"
        );
        app.world_mut()
            .resource_mut::<LocalAttackLedger>()
            .observe_own_attack(AttackId(11), 100);
        let projectile = app
            .world_mut()
            .spawn((
                brawler::combat::Projectile,
                StraightFlight {
                    origin: brawler::combat::WorldPoint::from(Vec2::new(30.0, 0.0)),
                    facing: 0.0,
                    speed: 600.0,
                    maximum_range: 500.0,
                    launched_at_tick: 100,
                },
                Position(Vec2::new(32.0, 0.0)),
                Rotation::default(),
                ProjectileSource {
                    shot_id: ShotId(11),
                    player_id: PlayerId(1),
                    owner_network_entity_id: OWNER,
                    team_id: brawler::combat::TeamId(1),
                    weapon_definition_id: brawler::combat::WeaponDefinitionId(1),
                },
                AttackDelivery {
                    attack_id: AttackId(11),
                    delivery_index: 0,
                },
            ))
            .id();
        let visual = app
            .world_mut()
            .spawn((
                CombatVisualOwner(projectile),
                V3ProjectileVisual {
                    planar_position: Vec2::new(30.0, 0.0),
                    local_lead: None,
                    local_handoff_applied: false,
                },
                Transform::default(),
                Visibility::default(),
            ))
            .id();
        frame(&mut app);
        assert_eq!(
            app.world().get::<Visibility>(visual),
            Some(&Visibility::Hidden),
            "no second flight from the muzzle"
        );
    }
}
