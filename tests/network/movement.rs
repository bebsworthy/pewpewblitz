//! Network integration scenarios extracted from the shared harness.

use super::*;

#[test]
fn native_input_moves_the_server_owned_fighter_and_replicates_position() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });

    let initial = harness.server_positions();
    assert_eq!(initial.len(), 1);
    harness.set_controlled_input(0, FighterInput::from_axes(Vec2::X, Some(Vec2::X), 0));

    let mut previous = initial[0].1;
    for _ in 0..36 {
        harness.step();
        let current = harness.server_positions()[0].1;
        assert!(
            (current.0 - previous.0).length() <= MovementTuning::default().speed / 60.0 + 0.1,
            "one tick displacement exceeded the authoritative speed limit: {previous:?} -> {current:?}"
        );
        previous = current;
    }

    let final_positions = harness.server_positions();
    assert_eq!(final_positions.len(), 1);
    assert!(final_positions[0].1.0.x > initial[0].1.0.x + 1.0);
    assert!(final_positions[0].1.0.x <= 800.0 - 24.0 + f32::EPSILON);
}

#[test]
fn two_clients_move_simultaneously_and_observe_the_same_authoritative_poses() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
            && harness.client_ids(0).len() == 2
            && harness.client_ids(1).len() == 2
    });

    let initial = harness.server_positions();
    for index in 0..2 {
        let direction = if harness.controlled_player_id(index).0 == 1 {
            Vec2::X
        } else {
            -Vec2::X
        };
        harness.set_controlled_input(
            index,
            FighterInput::from_axes(direction, Some(direction), 0),
        );
    }
    for _ in 0..36 {
        harness.step();
    }

    let server = harness.server_positions();
    assert!(
        server[0].1.0.x > initial[0].1.0.x + 1.0,
        "initial={initial:?} server={server:?}"
    );
    assert!(
        server[1].1.0.x < initial[1].1.0.x - 1.0,
        "initial={initial:?} server={server:?}"
    );
    let poses = harness.server_poses();
    assert!(poses[0].2.as_radians().abs() < 0.01);
    assert!((poses[1].2.as_radians() - std::f32::consts::PI).abs() < 0.02);

    harness.set_controlled_input(0, FighterInput::default());
    harness.set_controlled_input(1, FighterInput::default());
    for _ in 0..8 {
        harness.step();
    }
    harness.sample_client_at_newest_position_history(0);
    harness.sample_client_at_newest_position_history(1);
    let client_zero = harness.client_positions(0);
    let client_one = harness.client_positions(1);
    assert_eq!(client_zero.len(), 2);
    assert_eq!(client_one.len(), 2);
    for (((player_zero, position_zero), (player_one, position_one)), (_, server_position)) in
        client_zero.iter().zip(&client_one).zip(&server)
    {
        assert_eq!(player_zero, player_one);
        assert!((position_zero.0 - position_one.0).length() < 0.5);
        assert!(
            (position_zero.0 - server_position.0).length() < 64.0,
            "client pose did not converge toward the authoritative pose: client={position_zero:?} server={server_position:?}"
        );
    }
    assert_eq!(harness.client_interpolated_fighters(0), 2);
    assert_eq!(harness.client_interpolated_fighters(1), 2);
}

#[test]
fn owner_view_records_authoritative_interpolation_baseline_without_prediction() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });

    let world = harness.clients[0].world_mut();
    let mut controlled = world.query_filtered::<Entity, (With<Fighter>, With<Controlled>)>();
    assert_eq!(controlled.iter(world).count(), 1);
    let mut predicted = world.query_filtered::<Entity, With<Predicted>>();
    assert_eq!(predicted.iter(world).count(), 0);
}

#[test]
fn changed_authoritative_position_reaches_an_existing_client() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });
    let expected = {
        let world = harness.server.world_mut();
        let mut query =
            world.query_filtered::<&mut Position, (With<Fighter>, Without<TestDummy>)>();
        let mut position = query.single_mut(world).expect("one server fighter");
        position.0.x += 100.0;
        *position
    };
    for _ in 0..12 {
        harness.step();
    }
    harness.sample_client_at_newest_position_history(0);
    let actual = harness.client_positions(0)[0].1;
    let history = {
        let world = harness.clients[0].world_mut();
        let mut query = world.query_filtered::<
            (&PlayerId, Option<&ConfirmedHistory<Position>>),
            (With<Fighter>, With<Remote>, Without<TestDummy>),
        >();
        query
            .iter(world)
            .find(|(player, _)| player.0 != 0)
            .and_then(|(_, history)| {
                history.and_then(|history| {
                    history.newest_present().map(|(tick, value)| (tick, *value))
                })
            })
    };
    let timeline = {
        let timeline = harness.clients[0]
            .world()
            .resource::<InterpolationTimeline>();
        (timeline.is_synced(), timeline.now())
    };
    assert!(
        (actual.0 - expected.0).length() < 64.0,
        "changed authoritative pose did not reach the client: actual={actual:?} expected={expected:?} history={history:?} timeline={timeline:?}"
    );
}

#[test]
fn late_join_receives_current_poses_without_duplicating_static_arena() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });
    let static_count = harness.server_static_arena_count();
    let initial = harness.server_positions()[0].1;
    harness.set_controlled_input(0, FighterInput::from_axes(Vec2::X, Some(Vec2::Y), 0));
    for _ in 0..24 {
        harness.step();
    }
    let before_join = harness.server_positions()[0].1;
    assert!((before_join.0 - initial.0).length() > 1.0);
    harness.set_controlled_input(0, FighterInput::default());
    for _ in 0..60 {
        harness.step();
    }

    harness.add_client(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
            && harness.client_ids(1).len() == 2
    });
    for _ in 0..60 {
        harness.step();
    }
    let server = harness.server_positions();
    let late_view = harness.client_positions(1);
    assert_eq!(server.len(), 2);
    assert_eq!(late_view.len(), 2);
    for ((server_player, server_position), (client_player, client_position)) in
        server.iter().zip(&late_view)
    {
        assert_eq!(server_player, client_player);
        assert!(
            (server_position.0 - client_position.0).length() < 1.0,
            "player={server_player:?} server={server_position:?} late={client_position:?}"
        );
    }
    assert_eq!(harness.server_static_arena_count(), static_count);
}

#[test]
fn hostile_input_and_client_pose_attempts_are_rejected_and_counted() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
            && harness.client_ids(0).len() == 2
    });
    harness.set_controlled_input(0, FighterInput::default());
    harness.set_controlled_input(1, FighterInput::default());
    for _ in 0..60 {
        harness.step();
    }

    let own_player = harness.controlled_player_id(0);
    let target_player = [PlayerId(1), PlayerId(2)]
        .into_iter()
        .find(|player| *player != own_player)
        .expect("two-client harness should have another player");
    let target_client_entity = harness.remote_entity_for_player(0, target_player);
    let initial_target = harness
        .server_positions()
        .into_iter()
        .find(|(player, _)| *player == target_player)
        .expect("target fighter should exist")
        .1;
    let spoof_target = harness.remote_entity_for_player(0, target_player);
    let spoof_tick = harness.server_tick().saturating_add(1);
    harness.send_forged_input(
        0,
        lightyear::input::input_message::InputTarget::Entity(spoof_target),
        spoof_tick,
        FighterInput::from_axes(Vec2::X, None, 0),
    );
    {
        let client = harness.clients[0].world_mut();
        client
            .entity_mut(target_client_entity)
            .insert(Position::from_xy(9_999.0, 9_999.0));
    }
    for _ in 0..4 {
        harness.step();
    }
    let target_after = harness
        .server_positions()
        .into_iter()
        .find(|(player, _)| *player == target_player)
        .expect("target fighter should remain")
        .1;
    assert!(
        (target_after.0 - initial_target.0).length() < 0.01,
        "spoofed target moved: initial={initial_target:?} after={target_after:?}"
    );
    let diagnostics = harness
        .server
        .world_mut()
        .query::<&InputValidationState>()
        .iter(harness.server.world())
        .copied()
        .collect::<Vec<_>>();
    assert!(
        diagnostics
            .iter()
            .any(|state| state.ownership_rejections > 0),
        "spoofed target should increment ownership diagnostics: {diagnostics:?}"
    );

    let own_target = harness.controlled_entity(0);
    let valid_tick = harness.server_tick().saturating_add(1);
    harness.send_forged_input(
        0,
        lightyear::input::input_message::InputTarget::Entity(own_target),
        valid_tick,
        FighterInput::from_axes(Vec2::X, None, 0),
    );
    harness.step();
    harness.send_forged_input(
        0,
        lightyear::input::input_message::InputTarget::Entity(own_target),
        valid_tick,
        FighterInput::default(),
    );
    let future_tick = harness.server_tick().saturating_add(100);
    harness.send_forged_input(
        0,
        lightyear::input::input_message::InputTarget::Entity(own_target),
        future_tick,
        FighterInput::default(),
    );
    let mut malformed = FighterInput::default();
    malformed.gameplay_buttons = 0x80;
    let malformed_tick = harness.server_tick().saturating_add(1);
    harness.send_forged_input(
        0,
        lightyear::input::input_message::InputTarget::Entity(own_target),
        malformed_tick,
        malformed,
    );
    harness.step();
    for link in &harness.server_links {
        harness
            .server
            .world_mut()
            .get_mut::<InputValidationState>(*link)
            .expect("validation state")
            .tokens = 0.0;
    }
    harness
        .server
        .world_mut()
        .resource_mut::<InputTuning>()
        .input_rate = 0.0;
    let rate_limited_tick = harness.server_tick().saturating_add(1);
    harness.send_forged_input(
        0,
        lightyear::input::input_message::InputTarget::Entity(own_target),
        rate_limited_tick,
        FighterInput::default(),
    );
    for _ in 0..4 {
        harness.step();
    }
    let diagnostics = harness
        .server
        .world_mut()
        .query::<&InputValidationState>()
        .iter(harness.server.world())
        .copied()
        .collect::<Vec<_>>();
    assert!(
        diagnostics
            .iter()
            .any(|state| state.stale_or_reordered_rejections > 0)
    );
    assert!(
        diagnostics
            .iter()
            .any(|state| state.old_or_future_rejections > 0)
    );
    assert!(
        diagnostics
            .iter()
            .any(|state| state.malformed_rejections > 0),
        "expected malformed diagnostic, got {diagnostics:?}"
    );
    assert!(diagnostics.iter().any(|state| state.rate_rejections > 0));
}

#[test]
fn client_owned_component_writes_cannot_mutate_authoritative_build_weapon_or_pose() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
            && harness.selection_is_complete(0)
    });
    let player_id = harness.controlled_player_id(0);
    let (server_build, server_fingerprint, server_ammo, server_position) = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(
            &PlayerId,
            &SelectedBuild,
            &ResolvedWeapon,
            &WeaponState,
            &Position,
        ), With<Fighter>>();
        let (_, build, resolved, weapon, position) = query
            .iter(world)
            .find(|(player, _, _, _, _)| **player == player_id)
            .expect("server fighter");
        (*build, resolved.recipe_fingerprint, *weapon, *position)
    };
    let client_entity = harness.controlled_entity(0);
    {
        let world = harness.clients[0].world_mut();
        let mut forged_build = server_build;
        forged_build.source_preset_id = Some(WeaponPresetId(4));
        forged_build.recipe_fingerprint = Some(WeaponRecipeFingerprint(0xdead_beef));
        let mut forged_resolved = world
            .get::<ResolvedWeapon>(client_entity)
            .expect("client resolved weapon")
            .clone();
        forged_resolved.recipe_fingerprint = WeaponRecipeFingerprint(0xdead_beef);
        world.entity_mut(client_entity).insert((
            forged_build,
            forged_resolved,
            WeaponState {
                ammo: 0,
                phase: WeaponPhase::Reloading { ready_at_tick: 1 },
            },
            Position::from_xy(9_000.0, 9_000.0),
        ));
    }
    for _ in 0..12 {
        harness.step();
    }
    let (actual_build, actual_fingerprint, actual_ammo, actual_position) = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(
            &PlayerId,
            &SelectedBuild,
            &ResolvedWeapon,
            &WeaponState,
            &Position,
        ), With<Fighter>>();
        let (_, build, resolved, weapon, position) = query
            .iter(world)
            .find(|(player, _, _, _, _)| **player == player_id)
            .expect("server fighter");
        (*build, resolved.recipe_fingerprint, *weapon, *position)
    };
    assert_eq!(actual_build, server_build);
    assert_eq!(actual_fingerprint, server_fingerprint);
    assert_eq!(actual_ammo, server_ammo);
    assert!((actual_position.0 - server_position.0).length() < 0.01);
}

#[test]
fn authoritative_fighters_stop_at_walls_slide_tangentially_and_overlap() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
    });

    let wall_client = (0..2)
        .find(|&index| harness.controlled_player_id(index).0 == 1)
        .expect("player one client");
    harness.set_controlled_input(wall_client, FighterInput::from_axes(Vec2::X, None, 0));
    for _ in 0..300 {
        harness.step();
    }
    let wall_pose = harness.server_poses()[0];
    assert!(
        (774.5..=776.0).contains(&wall_pose.1.0.x),
        "wall_pose={wall_pose:?}"
    );

    harness.set_controlled_input(
        wall_client,
        FighterInput::from_axes(Vec2::new(1.0, 1.0), None, 0),
    );
    let before_slide = harness.server_poses()[0].1.0;
    for _ in 0..60 {
        harness.step();
    }
    let after_slide = harness.server_poses()[0].1.0;
    assert!((774.5..=776.0).contains(&after_slide.x));
    assert!(after_slide.y > before_slide.y + 100.0);
    for _ in 0..240 {
        harness.step();
    }
    let corner_pose = harness.server_poses()[0].1.0;
    assert!((774.5..=776.0).contains(&corner_pose.x));
    assert!((474.5..=476.0).contains(&corner_pose.y));

    let mut overlap = Harness::new(2);
    overlap.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
    });
    let second_entity = {
        let world = overlap.server.world_mut();
        let mut query = world.query_filtered::<(Entity, &PlayerId), With<Fighter>>();
        query
            .iter(world)
            .find(|(_, player)| player.0 == 2)
            .map(|(entity, _)| entity)
            .expect("second fighter should exist")
    };
    overlap
        .server
        .world_mut()
        .entity_mut(second_entity)
        .insert(Position::from_xy(620.0, -300.0));
    for index in 0..2 {
        let direction = if overlap.controlled_player_id(index).0 == 1 {
            Vec2::X
        } else {
            -Vec2::X
        };
        overlap.set_controlled_input(index, FighterInput::from_axes(direction, None, 0));
    }
    for _ in 0..140 {
        overlap.step();
    }
    let overlap_poses = overlap.server_poses();
    assert!(
        (overlap_poses[0].1.0 - overlap_poses[1].1.0).length() < 48.0,
        "overlap_poses={overlap_poses:?}"
    );
}

#[test]
fn configured_arena_and_movement_resources_drive_spawn_and_collider_tuning() {
    let mut harness = Harness::new(1);
    {
        let mut arena = harness
            .server
            .world_mut()
            .resource_mut::<GreyboxArenaDefinition>();
        arena.spawn_x = [-700.0, 700.0];
    }
    harness
        .server
        .world_mut()
        .resource_mut::<MovementTuning>()
        .spawn_facing = 1.25;
    harness.step_until(|harness| harness.client_is_active(0) && harness.server_ids().len() == 1);
    let pose = harness.server_poses()[0];
    assert!((pose.1.0.x + 700.0).abs() < 0.5);
    assert!((pose.2.as_radians() - 1.25).abs() < 0.01);
}

#[test]
fn authoritative_move_and_slide_depenetrates_a_spawned_inside_cover_fighter() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| harness.client_is_active(0) && harness.server_ids().len() == 1);
    let fighter = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<Entity, (With<Fighter>, Without<TestDummy>)>();
        query
            .iter(world)
            .next()
            .expect("server should have one fighter")
    };
    harness
        .server
        .world_mut()
        .entity_mut(fighter)
        .insert(Position::from_xy(0.0, -220.0));
    harness.set_controlled_input(0, FighterInput::default());
    for _ in 0..4 {
        harness.step();
    }
    let pose = harness.server_poses()[0].1.0;
    assert!(pose.x.abs() >= 114.0 || (pose.y + 220.0).abs() >= 84.0);
}
