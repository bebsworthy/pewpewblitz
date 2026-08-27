//! Network integration scenarios extracted from the shared harness.

use super::*;

#[test]
fn projectile_filters_allied_fighters_and_consumes_on_map_collision() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
    });

    {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(
            &PlayerId,
            &mut Position,
            &mut Rotation,
            &mut TeamId,
        ), With<Fighter>>();
        let mut player_one_team = None;
        for (player, mut position, mut rotation, mut team) in query.iter_mut(world) {
            if player.0 == 0 {
                continue;
            }
            let right_side = player.0 % 2 == 0;
            position.0 = Vec2::new(if right_side { 140.0 } else { -140.0 }, 0.0);
            *rotation = Rotation::radians(if right_side {
                std::f32::consts::PI
            } else {
                0.0
            });
            if let Some(player_one_team) = player_one_team {
                *team = player_one_team;
            } else {
                player_one_team = Some(*team);
            }
        }
    }
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    for _ in 0..70 {
        harness.step();
    }
    let server_health: Vec<_> = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(
            &PlayerId,
            &CurrentHealth,
            &brawler::builds::ResolvedMatchLoadout,
        ), With<Fighter>>();
        query
            .iter(world)
            .filter(|(player, _, _)| player.0 != 0)
            .map(|(player, health, loadout)| {
                (*player, *health, loadout.fighter_stats.maximum_health)
            })
            .collect()
    };
    assert!(
        server_health
            .iter()
            .all(|(_, health, maximum)| health.0 == *maximum)
    );
    assert_eq!(
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .hostile_fighter_hits,
        0
    );
    harness.set_controlled_input(0, FighterInput::default());
    for _ in 0..70 {
        harness.step();
    }
    assert_eq!(harness.server_projectile_count(), 0);

    {
        let world = harness.server.world_mut();
        let mut query =
            world.query_filtered::<(&PlayerId, &mut Position, &mut Rotation), With<Fighter>>();
        for (player, mut position, mut rotation) in query.iter_mut(world) {
            if player.0 == 1 {
                position.0 = Vec2::new(700.0, 0.0);
                *rotation = Rotation::IDENTITY;
            }
        }
    }
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .records
            .iter()
            .any(|record| matches!(record, CombatLogRecord::Hit { target: None, .. }))
    });
    assert_eq!(harness.server_projectile_count(), 0);
}

#[test]
fn projectile_hits_the_closest_valid_target_and_does_not_pass_through_it() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
    });
    for index in 0..2 {
        harness.install_saved_brawler_recipe(index, 2, 1, 1, [3, 6]);
    }
    let source_player = harness.controlled_player_id(0);
    let target_player = harness
        .server_ids()
        .into_iter()
        .find(|(player, _)| *player != source_player)
        .map(|(player, _)| player)
        .expect("second player");
    let target_network_id = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &NetworkEntityId), With<Fighter>>();
        *query
            .iter(world)
            .find(|(player, _)| **player == target_player)
            .expect("target fighter")
            .1
    };
    {
        let world = harness.server.world_mut();
        let mut fighters =
            world.query_filtered::<(&PlayerId, &mut Position, &mut Rotation), With<Fighter>>();
        for (player, mut position, mut rotation) in fighters.iter_mut(world) {
            if player.0 == 0 {
                position.0 = Vec2::new(300.0, -300.0);
            } else if *player == source_player {
                position.0 = Vec2::new(-300.0, -300.0);
                *rotation = Rotation::IDENTITY;
            } else {
                position.0 = Vec2::new(-100.0, -300.0);
            }
        }
    }
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .hostile_fighter_hits
            > 0
    });

    let (target_health, dummy_health) = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &CurrentHealth), With<Fighter>>();
        let mut target_health = None;
        let mut dummy_health = None;
        for (player, health) in query.iter(world) {
            if *player == target_player {
                target_health = Some(health.0);
            } else if player.0 == 0 {
                dummy_health = Some(health.0);
            }
        }
        (
            target_health.expect("target health"),
            dummy_health.expect("dummy health"),
        )
    };
    assert!(target_health < 100);
    assert_eq!(dummy_health, 100);
    assert!(
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .records
            .iter()
            .any(|record| matches!(
                record,
                CombatLogRecord::Hit {
                    target: Some(target), ..
                } if *target == target_network_id
            ))
    );
}

#[test]
fn projectile_stops_at_thin_cover_before_the_target() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });
    {
        let world = harness.server.world_mut();
        let mut source = world.query_filtered::<(
            &PlayerId,
            &mut Position,
            &mut Rotation,
            Option<&mut brawler::builds::ResolvedMatchLoadout>,
        ), With<Fighter>>();
        for (player, mut position, mut rotation, loadout) in source.iter_mut(world) {
            if player.0 == 0 {
                position.0 = Vec2::new(300.0, -220.0);
            } else {
                position.0 = Vec2::new(-300.0, -220.0);
                *rotation = Rotation::IDENTITY;
                let mut loadout = loadout.expect("controlled fighter loadout");
                let brawler::combat::DeliveryMethod::Straight { radius, range, .. } =
                    &mut loadout.primary_weapon.recipe.delivery
                else {
                    panic!("Pulse Sidearm uses straight delivery");
                };
                // This fixture proves thin-cover sweep ordering rather than canonical balance.
                *radius = 6.0;
                *range = 900.0;
            }
        }
    }
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .records
            .iter()
            .any(|record| matches!(record, CombatLogRecord::Hit { target: None, .. }))
    });
    harness.set_controlled_input(0, FighterInput::default());
    harness.step_until(|harness| harness.server_projectile_count() == 0);
    let dummy_health = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
        query.single(world).expect("dummy health").0
    };
    assert_eq!(dummy_health, 100);
    assert_eq!(harness.server_projectile_count(), 0);
}

#[test]
fn posthumous_projectile_retains_original_source_attribution() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
    });
    {
        let world = harness.server.world_mut();
        let mut query =
            world.query_filtered::<(&PlayerId, &mut Position, &mut Rotation), With<Fighter>>();
        for (player, mut position, mut rotation) in query.iter_mut(world) {
            if player.0 == 0 {
                continue;
            }
            let right_side = player.0 % 2 == 0;
            position.0 = Vec2::new(if right_side { 140.0 } else { -140.0 }, 160.0);
            *rotation = Rotation::radians(if right_side {
                std::f32::consts::PI
            } else {
                0.0
            });
        }
    }
    let source_player = harness.controlled_player_id(0);
    let source_aim = if source_player.0 == 1 {
        Vec2::X
    } else {
        -Vec2::X
    };
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(source_aim), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| harness.server_projectile_count() > 0);
    let owner = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(Entity, &PlayerId), With<Fighter>>();
        query
            .iter(world)
            .find(|(_, player)| **player == source_player)
            .map(|(entity, _)| entity)
            .expect("owner fighter")
    };
    harness.server.world_mut().entity_mut(owner).insert((
        CurrentHealth(0),
        Defeated {
            event_id: CombatEventId(10_000),
        },
    ));

    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .hostile_fighter_hits
            > 0
    });
    assert!(
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .records
            .iter()
            .any(|record| {
                matches!(
                    record,
                    CombatLogRecord::Damage {
                        source: brawler::combat::DamageSource::PlayerWeapon { player_id, .. },
                        ..
                    } if *player_id == source_player
                )
            })
    );
}
