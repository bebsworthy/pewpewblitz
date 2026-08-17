//! Server-only combat identity reservation and lifecycle limits.

#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) const MAX_ACTIVE_ATTACK_TRACKERS: usize = 512;

pub(super) fn reserve_event_ids(
    ids: &mut NextCombatIds,
    count: usize,
) -> Option<Vec<CombatEventId>> {
    let count = u64::try_from(count).ok()?;
    let first = ids.next_event_id;
    let next = first.checked_add(count)?;
    ids.next_event_id = next;
    Some(
        (0..count)
            .map(|offset| CombatEventId(first + offset))
            .collect(),
    )
}

pub(super) fn reserve_attack_and_events(
    ids: &mut NextCombatIds,
    event_count: usize,
) -> Option<(AttackId, Vec<CombatEventId>)> {
    let previous_attack_id = ids.next_attack_id;
    let attack_id = ids.allocate_attack()?;
    let Some(events) = reserve_event_ids(ids, event_count) else {
        ids.next_attack_id = previous_attack_id;
        return None;
    };
    Some((attack_id, events))
}

#[cfg(feature = "server")]
pub struct ServerCombatPlugin;

#[cfg(feature = "server")]
impl Plugin for ServerCombatPlugin {
    fn build(&self, app: &mut App) {
        if env::var("BRAWLER_NETWORK_ASSERT_COMBAT").as_deref() == Ok("1") {
            app.insert_resource(TestDummyFixture {
                position: Vec2::new(0.0, -320.0),
                facing: 0.0,
            });
        } else if env::var("BRAWLER_NETWORK_TERRAIN_TEST_DUMMY").as_deref() == Ok("1") {
            // Terrain-profile target: just south of the central destructible block, so
            // aimed Arc Launcher lobs land within the radius-48 brush reach of its face
            // from any spawn lane. The combat-assert fixture above keeps its own position.
            app.insert_resource(TestDummyFixture {
                position: Vec2::new(0.0, -120.0),
                facing: 0.0,
            });
        }
        app.init_resource::<FighterDefinitions>()
            .init_resource::<WeaponDefinitions>()
            .init_resource::<MovementTuning>()
            .init_resource::<NextCombatIds>()
            .init_resource::<CombatTelemetry>()
            .init_resource::<WeaponTelemetry>()
            .init_resource::<ActiveAttackTrackers>()
            .init_resource::<CombatOutbox>()
            .init_resource::<CombatOutcomeFacts>()
            .init_resource::<CombatWorldEffectFacts>()
            .init_resource::<CombatEvidenceSnapshots>()
            .init_resource::<CombatSummaryLogged>()
            .insert_resource(CombatEvidenceMode {
                enabled: env::var("BRAWLER_NETWORK_ASSERT_COMBAT").as_deref() == Ok("1"),
            })
            .add_message::<MeleeAttack>()
            .add_message::<PendingPayload>()
            .add_message::<PendingDelivery>()
            .add_systems(
                Startup,
                (
                    validate_definitions,
                    spawn_test_dummy.run_if(resource_exists::<TestDummyFixture>),
                )
                    .chain()
                    .after(crate::map::MapStartupSet::Instantiate),
            )
            .add_systems(
                FixedUpdate,
                (
                    reset_due_fighters
                        .run_if(resource_exists::<TestDummyFixture>)
                        .in_set(GameplaySet::Lifecycle),
                    expire_runtime_effects.in_set(GameplaySet::Lifecycle),
                    authoritative_composed_fire.in_set(GameplaySet::Fire),
                    ApplyDeferred.after(GameplaySet::Fire),
                ),
            )
            .add_systems(
                FixedPostUpdate,
                (
                    sweep_composed_projectiles
                        .after(avian2d::prelude::PhysicsSystems::StepSimulation)
                        .in_set(CombatSet::ProjectileSweep),
                    resolve_melee_attacks.in_set(CombatSet::Damage),
                    resolve_composed_payloads
                        .after(resolve_melee_attacks)
                        .in_set(CombatSet::Damage),
                    flush_completed_attack_telemetry.in_set(CombatSet::TelemetryAndCues),
                    send_combat_cues.in_set(CombatSet::TelemetryAndCues),
                    publish_authoritative_tick
                        .in_set(CombatSet::Finalize)
                        .before(crate::gameplay::advance_simulation_tick),
                    capture_server_combat_checkpoints
                        .in_set(CombatSet::Finalize)
                        .after(publish_authoritative_tick),
                    send_combat_evidence_checkpoints
                        .in_set(CombatSet::Finalize)
                        .after(capture_server_combat_checkpoints),
                ),
            )
            .add_systems(
                PreUpdate,
                cleanup_disconnected_projectiles
                    .after(lightyear::transport::plugin::TransportSystems::Receive),
            )
            .add_systems(Last, emit_combat_summary);
        let definition = *app
            .world()
            .resource::<FighterDefinitions>()
            .get(STANDARD_FIGHTER_DEFINITION)
            .expect("standard fighter definition exists");
        let mut tuning = app.world_mut().resource_mut::<MovementTuning>();
        tuning.speed = definition.movement_speed;
        tuning.radius = definition.body_radius;
        tuning.spawn_facing = definition.spawn_facing;
    }
}
