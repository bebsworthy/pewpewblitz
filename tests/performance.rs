use avian2d::prelude::{
    AngularVelocity, Collider, CollisionLayers, CustomPositionIntegration, LinearVelocity,
    Position, RigidBody, Rotation,
};
use bevy::{prelude::*, time::TimeUpdateStrategy};
use brawler::{
    combat::{
        ActiveEffects, AttackId, AttackSource, ComposedProjectileRuntime, CurrentHealth,
        ExternalMotion, FighterDefinitionId, FighterDefinitions, LobbedFlight, Projectile,
        SelectedBuild, SlowEffect, TeamId, WeaponDefinitionId, WeaponDefinitions, WeaponPhase,
        WeaponPresetId, WeaponState, default_fighter_runtime,
    },
    config::{NetworkTransport, ServerNetworkConfig},
    gameplay::GameplayPlugin,
    map::AuthoritativeMapPlugin,
    matchplay::{
        ActiveCombatant, MatchMember, MatchParticipant, MatchPhase, MatchRoot, MatchState,
    },
    movement::{
        AuthoritativeMovementPlugin, AvianNetworkPlugin, DESTRUCTIBLE_TERRAIN_LAYER, FIGHTER_LAYER,
        INDESTRUCTIBLE_TERRAIN_LAYER, InputFreshness, PROJECTILE_LAYER, fighter_collision_layers,
    },
    protocol::{Fighter, FighterInput, NetworkEntityId, PlayerId, ProtocolPlugin},
    server::ServerNetworkPlugin,
    timing::SIMULATION_TICK,
};
use lightyear::prelude::input::native::ActionState;
use lightyear::prelude::server::ServerPlugins;
use std::time::Instant;

fn performance_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::state::app::StatesPlugin,
        ServerPlugins {
            tick_duration: SIMULATION_TICK,
        },
        GameplayPlugin,
        ProtocolPlugin,
        AvianNetworkPlugin,
        AuthoritativeMapPlugin,
        AuthoritativeMovementPlugin,
        ServerNetworkPlugin,
        brawler::matchplay::AuthoritativeMatchPlugin,
        brawler::matchplay::WipeoutModePlugin,
        brawler::terrain::AuthoritativeTerrainPlugin,
    ));
    app.insert_resource(brawler::matchplay::MatchLifecycleRules::default());
    app.insert_resource(brawler::matchplay::WipeoutRules::default());
    app.insert_resource(ServerNetworkConfig {
        transport: NetworkTransport::Crossbeam,
        ..default()
    });
    app.insert_resource(TimeUpdateStrategy::ManualDuration(SIMULATION_TICK));
    app.update();
    app
}

fn spawn_headless_fighters(app: &mut App) -> Vec<Entity> {
    let fighters = app.world().resource::<FighterDefinitions>().clone();
    let weapons = app.world().resource::<WeaponDefinitions>().clone();
    let mut entities = Vec::with_capacity(100);
    for row in 0_u16..10 {
        for column in 0_u16..10 {
            let player_id = u64::from(row) * 10 + u64::from(column) + 1;
            let mut position = Vec2::new(
                -700.0 + f32::from(column) * 155.0,
                -400.0 + f32::from(row) * 88.0,
            );
            if position.x.abs() < 130.0 && position.y.abs() < 130.0 {
                // Keep benchmark fighters clear of the central destructible block.
                position.x += 260.0;
            }
            let (fighter_id, build, team, health, weapon) = default_fighter_runtime(
                TeamId(u8::try_from(player_id % 2).expect("benchmark team fits in u8")),
                &fighters,
                &weapons,
            );
            let entity = app
                .world_mut()
                .spawn((
                    Fighter,
                    PlayerId(player_id),
                    NetworkEntityId(player_id),
                    fighter_id,
                    build,
                    team,
                    health,
                    weapon,
                    Position::from_xy(position.x, position.y),
                    Rotation::IDENTITY,
                    LinearVelocity::default(),
                    AngularVelocity::default(),
                    Collider::circle(24.0),
                    RigidBody::Kinematic,
                ))
                .id();
            app.world_mut().entity_mut(entity).insert((
                CustomPositionIntegration,
                fighter_collision_layers(),
                InputFreshness::default(),
                Transform::from_translation(position.extend(0.0)),
            ));
            entities.push(entity);
        }
    }
    entities
}

fn spawn_m05_fighter(
    app: &mut App,
    player_id: u64,
    preset_id: u16,
    position: Vec2,
    team: TeamId,
    fire: bool,
) -> Entity {
    let position = if position.x.abs() < 130.0 && position.y.abs() < 130.0 {
        // Keep benchmark fighters clear of the central destructible block.
        Vec2::new(position.x + 260.0, position.y)
    } else {
        position
    };
    let fighter = *app
        .world()
        .resource::<FighterDefinitions>()
        .get(brawler::combat::STANDARD_FIGHTER_DEFINITION)
        .expect("standard fighter definition");
    let resolved = app
        .world()
        .resource::<brawler::combat::WeaponCatalogResource>()
        .0
        .resolve_preset(WeaponPresetId(preset_id), &fighter)
        .expect("benchmark preset resolves");
    let source_preset_id = WeaponPresetId(preset_id);
    let entity = app
        .world_mut()
        .spawn((
            Fighter,
            PlayerId(player_id),
            NetworkEntityId(player_id),
            FighterDefinitionId(fighter.id.0),
            SelectedBuild {
                primary_weapon: WeaponDefinitionId(preset_id),
                source_preset_id: Some(source_preset_id),
                recipe_fingerprint: Some(resolved.recipe_fingerprint),
            },
            resolved.clone(),
            team,
            CurrentHealth(fighter.maximum_health),
            WeaponState {
                ammo: resolved.recipe.economy.capacity(),
                phase: WeaponPhase::Ready,
            },
        ))
        .id();
    app.world_mut().entity_mut(entity).insert((
        Position::from_xy(position.x, position.y),
        Rotation::IDENTITY,
        LinearVelocity::default(),
        AngularVelocity::default(),
        Collider::circle(fighter.body_radius),
        RigidBody::Kinematic,
        ActionState(FighterInput::from_axes(
            Vec2::ZERO,
            Some(Vec2::X),
            if fire { FighterInput::PRIMARY_FIRE } else { 0 },
        )),
    ));
    app.world_mut().entity_mut(entity).insert((
        CustomPositionIntegration,
        fighter_collision_layers(),
        InputFreshness {
            last_fresh_tick: Some(0),
        },
        Transform::from_translation(position.extend(0.0)),
    ));
    entity
}

fn remove_benchmark_actions(app: &mut App, entities: &[Entity]) {
    for &entity in entities {
        app.world_mut()
            .entity_mut(entity)
            .remove::<ActionState<FighterInput>>();
    }
}

fn fixed_tick_p95(app: &mut App, label: &str, samples_count: usize) -> std::time::Duration {
    let mut samples = Vec::with_capacity(samples_count);
    for _ in 0..samples_count {
        let start = Instant::now();
        app.update();
        samples.push(start.elapsed());
    }
    samples.sort_unstable();
    let p95 = samples[(samples.len() * 95) / 100];
    println!(
        "{label}: arch={} os={} p95={p95:?}",
        std::env::consts::ARCH,
        std::env::consts::OS,
    );
    assert!(
        p95.as_secs_f64() < SIMULATION_TICK.as_secs_f64(),
        "{label} p95 {p95:?} exceeded {SIMULATION_TICK:?}"
    );
    p95
}

fn count_active_effects(app: &mut App) -> usize {
    let world = app.world_mut();
    let mut query = world.query_filtered::<(&ActiveEffects, &ExternalMotion), With<Fighter>>();
    query
        .iter(world)
        .filter(|(effects, _)| effects.slow.is_some())
        .count()
}

fn fighter_count(app: &mut App) -> usize {
    let world = app.world_mut();
    let mut query = world.query_filtered::<Entity, With<Fighter>>();
    query.iter(world).count()
}

fn projectile_count(app: &mut App) -> usize {
    let world = app.world_mut();
    let mut query = world.query_filtered::<Entity, With<Projectile>>();
    query.iter(world).count()
}

#[test]
fn one_hundred_headless_fighters_stay_within_fixed_tick_budget() {
    let mut app = performance_app();
    spawn_headless_fighters(&mut app);

    let mut samples = Vec::with_capacity(120);
    for _ in 0..120 {
        let start = Instant::now();
        app.update();
        samples.push(start.elapsed());
    }
    samples.sort_unstable();
    let median = samples[samples.len() / 2];
    let p95 = samples[(samples.len() * 95) / 100];
    println!(
        "100-fighter fixed tick benchmark: arch={} os={} median={:?} p95={:?}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        median,
        p95,
    );
    assert!(
        p95.as_secs_f64() < SIMULATION_TICK.as_secs_f64(),
        "p95 fixed tick {p95:?} exceeded {SIMULATION_TICK:?}"
    );
}

#[test]
fn m07_four_participant_match_telemetry_stays_within_fixed_tick_budget() {
    let mut app = performance_app();
    let match_id = {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&mut MatchState, With<MatchRoot>>();
        let mut state = roots.single_mut(world).expect("one match root");
        state.phase = MatchPhase::Active {
            ends_at_tick: u64::MAX,
        };
        state.match_id
    };
    let mut owners = Vec::new();
    for (index, (preset, position, team)) in [
        (1, Vec2::new(-220.0, -100.0), TeamId(0)),
        (2, Vec2::new(-220.0, 100.0), TeamId(0)),
        (3, Vec2::new(220.0, -100.0), TeamId(1)),
        (4, Vec2::new(220.0, 100.0), TeamId(1)),
    ]
    .into_iter()
    .enumerate()
    {
        let entity = spawn_m05_fighter(
            &mut app,
            30_000 + u64::try_from(index).expect("benchmark index fits"),
            preset,
            position,
            team,
            true,
        );
        app.world_mut().entity_mut(entity).insert((
            MatchParticipant {
                match_id,
                ready: true,
                restart_ready: false,
            },
            MatchMember(match_id),
            ActiveCombatant,
        ));
        owners.push(entity);
    }
    app.update();
    remove_benchmark_actions(&mut app, &owners);
    fixed_tick_p95(&mut app, "m07-four-participant-match-telemetry", 120);
}

#[test]
fn m08_four_sentries_target_fire_and_cleanup_within_fixed_tick_budget() {
    let mut app = performance_app();
    let match_id = {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&mut MatchState, With<MatchRoot>>();
        let mut state = roots.single_mut(world).expect("one match root");
        state.phase = MatchPhase::Active {
            ends_at_tick: u64::MAX,
        };
        state.match_id
    };
    let build_catalog = app
        .world()
        .resource::<brawler::builds::BuildCatalogResource>()
        .0
        .clone();
    let weapon_catalog = app
        .world()
        .resource::<brawler::combat::WeaponCatalogResource>()
        .0
        .clone();
    let fighter_definition = *app
        .world()
        .resource::<FighterDefinitions>()
        .get(brawler::combat::STANDARD_FIGHTER_DEFINITION)
        .unwrap();
    let controller = brawler::builds::resolve_build_recipe(
        &build_catalog,
        &weapon_catalog,
        &fighter_definition,
        build_catalog.presets[2].recipe,
        Some(build_catalog.presets[2].id),
    )
    .unwrap();
    let mut owners = Vec::new();
    for (index, (position, facing, team)) in [
        (Vec2::new(-650.0, -420.0), 0.0, TeamId(0)),
        (Vec2::new(-650.0, 420.0), 0.0, TeamId(0)),
        (Vec2::new(650.0, -420.0), std::f32::consts::PI, TeamId(1)),
        (Vec2::new(650.0, 420.0), std::f32::consts::PI, TeamId(1)),
    ]
    .into_iter()
    .enumerate()
    {
        let player_id = 40_000 + u64::try_from(index).unwrap();
        let entity = spawn_m05_fighter(&mut app, player_id, 3, position, team, false);
        app.world_mut().entity_mut(entity).insert((
            controller.identity,
            controller.clone(),
            brawler::builds::AbilityState {
                charge: 1_000,
                phase: brawler::builds::AbilityPhase::Ready,
            },
            brawler::builds::PassiveRuntimeState::default(),
            ActiveEffects::default(),
            MatchParticipant {
                match_id,
                ready: true,
                restart_ready: false,
            },
            MatchMember(match_id),
            ActiveCombatant,
            Rotation::radians(facing),
        ));
        owners.push(entity);
    }
    app.update();
    let tick = app.world().resource::<brawler::timing::SimulationTick>().0;
    for entity in &owners {
        app.world_mut().entity_mut(*entity).insert((
            ActionState(FighterInput::from_axes(
                Vec2::ZERO,
                None,
                FighterInput::ULTIMATE,
            )),
            InputFreshness {
                last_fresh_tick: Some(tick),
            },
        ));
    }
    app.update();
    remove_benchmark_actions(&mut app, &owners);
    let sentry_count = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<brawler::abilities::Sentry>>();
        query.iter(world).count()
    };
    assert_eq!(sentry_count, 4);
    fixed_tick_p95(&mut app, "m08-four-live-sentries", 120);
}

#[test]
#[allow(clippy::cast_precision_loss)]
fn one_hundred_headless_fighters_and_two_hundred_projectiles_stay_within_fixed_tick_budget() {
    let mut app = performance_app();
    let owners = spawn_headless_fighters(&mut app);
    // Make every path exercise the nearby fighter broad phase without allowing a friendly
    // pass-through to terminate the projectile. The lanes remain clear of the resolved map's
    // north/south cover while keeping perimeter terrain in the broad-phase workload.
    {
        let world = app.world_mut();
        let mut fighters = world.query::<&mut TeamId>();
        for mut team in fighters.iter_mut(world) {
            team.0 = 0;
        }
    }
    let lanes = [-500.0, -460.0, -420.0, -380.0, 380.0, 420.0, 460.0, 500.0];
    let recipe = app
        .world()
        .resource::<brawler::combat::WeaponCatalogResource>()
        .0
        .resolve_preset(
            WeaponPresetId(1),
            app.world()
                .resource::<FighterDefinitions>()
                .get(brawler::combat::STANDARD_FIGHTER_DEFINITION)
                .expect("standard fighter definition"),
        )
        .expect("benchmark preset resolves");
    for index in 0_usize..200 {
        let lane = lanes[index % lanes.len()];
        let start_column = index / lanes.len();
        let position = Vec2::new(-700.0 + start_column as f32 * 20.0, lane);
        app.world_mut().spawn((
            Projectile,
            ComposedProjectileRuntime {
                owner_entity: owners[index % owners.len()],
                source_entity: owners[index % owners.len()],
                source: AttackSource {
                    kind: brawler::combat::CombatSourceKind::PrimaryWeapon,
                    attack_id: AttackId(index as u64 + 1),
                    player_id: PlayerId(1),
                    owner_network_entity_id: NetworkEntityId(1),
                    team_id: TeamId(0),
                    recipe_fingerprint: recipe.recipe_fingerprint,
                    presentation_profile_id: recipe.presentation_profile_id,
                    legacy_compatibility: false,
                    source_preset_id: Some(WeaponPresetId(1)),
                    origin: brawler::combat::WorldPoint::from(position),
                    facing: 0.0,
                },
                delivery_index: 0,
                velocity: Vec2::X * 900.0,
                travelled: 0.0,
                expires_at_tick: u64::MAX,
                maximum_range: 100_000.0,
                radius: 6.0,
                landing: None,
                recipe: recipe.recipe.clone(),
            },
            Position::from_xy(position.x, position.y),
            Rotation::IDENTITY,
            Collider::circle(6.0),
            CollisionLayers::new(
                PROJECTILE_LAYER,
                FIGHTER_LAYER | INDESTRUCTIBLE_TERRAIN_LAYER | DESTRUCTIBLE_TERRAIN_LAYER,
            ),
        ));
    }

    let mut samples = Vec::with_capacity(60);
    for _ in 0..60 {
        assert_eq!(projectile_count(&mut app), 200);
        let start = Instant::now();
        app.update();
        samples.push(start.elapsed());
    }
    samples.sort_unstable();
    let p95 = samples[(samples.len() * 95) / 100];
    println!(
        "100-fighter/200-projectile fixed tick benchmark: arch={} os={} p95={:?}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        p95,
    );
    assert!(
        p95.as_secs_f64() < SIMULATION_TICK.as_secs_f64(),
        "p95 fixed tick {p95:?} exceeded {SIMULATION_TICK:?}"
    );
}

#[test]
#[allow(clippy::cast_precision_loss)]
fn m05_scatter_burst_worst_case_stays_within_fixed_tick_budget() {
    let mut app = performance_app();
    let mut owners = Vec::with_capacity(32);
    for index in 0..32 {
        let column = index % 8;
        let row = index / 8;
        owners.push(spawn_m05_fighter(
            &mut app,
            10_000 + u64::try_from(index).expect("benchmark index fits"),
            2,
            Vec2::new(-700.0 + column as f32 * 180.0, -360.0 + row as f32 * 80.0),
            TeamId(0),
            true,
        ));
    }
    for index in 0..68 {
        spawn_m05_fighter(
            &mut app,
            20_000 + u64::try_from(index).expect("benchmark index fits"),
            1,
            Vec2::new(
                -760.0 + (index % 17) as f32 * 92.0,
                -480.0 + (index / 17) as f32 * 120.0,
            ),
            TeamId(0),
            false,
        );
    }
    assert!(fighter_count(&mut app) >= 100);
    app.update();
    let telemetry = app.world().resource::<brawler::combat::WeaponTelemetry>();
    assert_eq!(
        telemetry.accepted_attacks.get(&WeaponPresetId(2)),
        Some(&32)
    );
    assert_eq!(
        telemetry.emitted_deliveries.get(&WeaponPresetId(2)),
        Some(&224)
    );
    assert_eq!(
        telemetry
            .source_aggregates
            .values()
            .find(|aggregate| aggregate.accepted_attacks == 32)
            .map(|aggregate| aggregate.emitted_deliveries),
        Some(224)
    );
    remove_benchmark_actions(&mut app, &owners);
    fixed_tick_p95(&mut app, "32-scatter-attacks/224-pellets", 60);
}

#[test]
#[allow(clippy::cast_precision_loss)]
fn m05_simultaneous_lob_landings_with_area_candidates_stay_within_fixed_tick_budget() {
    let mut app = performance_app();
    let mut owners = Vec::with_capacity(16);
    for index in 0..16 {
        owners.push(spawn_m05_fighter(
            &mut app,
            11_000 + u64::try_from(index).expect("benchmark index fits"),
            3,
            Vec2::new(-600.0 + index as f32 * 80.0, 380.0),
            TeamId(0),
            true,
        ));
    }
    for index in 0..100 {
        let column = index % 20;
        let row = index / 20;
        spawn_m05_fighter(
            &mut app,
            12_000 + u64::try_from(index).expect("benchmark index fits"),
            1,
            Vec2::new(-180.0 + column as f32 * 18.0, 330.0 + row as f32 * 22.0),
            TeamId(1),
            false,
        );
    }
    app.update();
    remove_benchmark_actions(&mut app, &owners);
    let current_tick = app.world().resource::<brawler::timing::SimulationTick>().0;
    let world = app.world_mut();
    let mut flights = world.query::<&mut LobbedFlight>();
    let mut flight_count = 0;
    for mut flight in flights.iter_mut(world) {
        flight.lands_at_tick = current_tick;
        flight.launched_at_tick = current_tick.saturating_sub(1);
        flight_count += 1;
    }
    assert_eq!(flight_count, 16);
    app.update();
    let telemetry = app.world().resource::<brawler::combat::WeaponTelemetry>();
    assert_eq!(
        telemetry.accepted_attacks.get(&WeaponPresetId(3)),
        Some(&16)
    );
    assert_eq!(
        telemetry.emitted_deliveries.get(&WeaponPresetId(3)),
        Some(&16)
    );
    let landed = app
        .world()
        .resource::<brawler::combat::CombatTelemetry>()
        .cues
        .iter()
        .filter(|cue| matches!(cue, brawler::combat::CombatCue::LobLanded { .. }))
        .count();
    assert_eq!(landed, 16);
    assert!(
        telemetry
            .hostile_delivery_contacts
            .get(&WeaponPresetId(3))
            .copied()
            .unwrap_or(0)
            > 0
    );
    assert!(
        telemetry
            .hostile_damage
            .get(&WeaponPresetId(3))
            .copied()
            .unwrap_or(0)
            > 0
    );
    fixed_tick_p95(&mut app, "16-same-tick-lob-landings/area-candidates", 60);
}

#[test]
#[allow(clippy::cast_precision_loss)]
fn m05_simultaneous_blade_sectors_with_candidates_stay_within_fixed_tick_budget() {
    let mut app = performance_app();
    let mut owners = Vec::with_capacity(32);
    for index in 0..32 {
        let column = index % 8;
        let row = index / 8;
        let position = Vec2::new(-650.0 + column as f32 * 180.0, -420.0 + row as f32 * 280.0);
        owners.push(spawn_m05_fighter(
            &mut app,
            13_000 + u64::try_from(index).expect("benchmark index fits"),
            4,
            position,
            TeamId(0),
            true,
        ));
        spawn_m05_fighter(
            &mut app,
            14_000 + u64::try_from(index).expect("benchmark index fits"),
            1,
            position + Vec2::new(80.0, 0.0),
            TeamId(1),
            false,
        );
    }
    for index in 0..36 {
        spawn_m05_fighter(
            &mut app,
            20_000 + u64::try_from(index).expect("benchmark index fits"),
            1,
            Vec2::new(
                -760.0 + (index % 12) as f32 * 130.0,
                420.0 - (index / 12) as f32 * 120.0,
            ),
            TeamId(0),
            false,
        );
    }
    assert!(fighter_count(&mut app) >= 100);
    app.update();
    remove_benchmark_actions(&mut app, &owners);
    let telemetry = app.world().resource::<brawler::combat::WeaponTelemetry>();
    assert_eq!(
        telemetry.accepted_attacks.get(&WeaponPresetId(4)),
        Some(&32)
    );
    assert!(
        telemetry
            .hostile_delivery_contacts
            .get(&WeaponPresetId(4))
            .copied()
            .unwrap_or(0)
            > 0
    );
    fixed_tick_p95(&mut app, "32-blade-sectors/candidates", 60);
}

#[test]
#[allow(clippy::cast_precision_loss)]
fn m05_one_hundred_active_effect_states_stay_within_fixed_tick_budget() {
    let mut app = performance_app();
    for index in 0..100 {
        let entity = spawn_m05_fighter(
            &mut app,
            15_000 + u64::try_from(index).expect("benchmark index fits"),
            1,
            Vec2::new(
                -700.0 + (index % 10) as f32 * 150.0,
                -400.0 + (index / 10) as f32 * 80.0,
            ),
            TeamId(0),
            false,
        );
        app.world_mut().entity_mut(entity).insert((
            ActiveEffects {
                slow: Some(SlowEffect {
                    source_attack_id: AttackId(u64::try_from(index + 1).expect("fits")),
                    source_network_entity_id: NetworkEntityId(1),
                    movement_multiplier_milli: 700,
                    expires_at_tick: u64::MAX,
                }),
            },
            ExternalMotion {
                velocity: Vec2::new(20.0, 0.0),
                expires_at_tick: u64::MAX,
            },
        ));
    }
    assert_eq!(count_active_effects(&mut app), 100);
    fixed_tick_p95(&mut app, "100-active-slow/external-motion-states", 60);
    assert_eq!(count_active_effects(&mut app), 100);
}

#[test]
#[allow(clippy::cast_precision_loss)]
fn m05_combined_worst_case_fixed_tick_stays_within_budget() {
    let mut app = performance_app();
    for index in 0..100 {
        let entity = spawn_m05_fighter(
            &mut app,
            16_000 + u64::try_from(index).expect("benchmark index fits"),
            1,
            Vec2::new(
                -700.0 + (index % 10) as f32 * 150.0,
                -400.0 + (index / 10) as f32 * 80.0,
            ),
            // Keep the 100-state population out of the attack target set so the combined
            // benchmark measures effect maintenance rather than defeat/reset churn.
            TeamId(0),
            false,
        );
        app.world_mut().entity_mut(entity).insert((
            ActiveEffects {
                slow: Some(SlowEffect {
                    source_attack_id: AttackId(u64::try_from(index + 100).expect("fits")),
                    source_network_entity_id: NetworkEntityId(1),
                    movement_multiplier_milli: 700,
                    expires_at_tick: u64::MAX,
                }),
            },
            ExternalMotion {
                velocity: Vec2::new(20.0, 0.0),
                expires_at_tick: u64::MAX,
            },
        ));
    }
    let mut actions = Vec::new();
    for index in 0..32 {
        actions.push(spawn_m05_fighter(
            &mut app,
            17_000 + u64::try_from(index).expect("benchmark index fits"),
            2,
            Vec2::new(
                -700.0 + (index % 8) as f32 * 180.0,
                300.0 + (index / 8) as f32 * 35.0,
            ),
            TeamId(0),
            true,
        ));
    }
    for index in 0..16 {
        actions.push(spawn_m05_fighter(
            &mut app,
            18_000 + u64::try_from(index).expect("benchmark index fits"),
            3,
            Vec2::new(-600.0 + index as f32 * 80.0, -400.0),
            TeamId(0),
            true,
        ));
    }
    for index in 0..32 {
        actions.push(spawn_m05_fighter(
            &mut app,
            19_000 + u64::try_from(index).expect("benchmark index fits"),
            4,
            Vec2::new(
                -650.0 + (index % 8) as f32 * 180.0,
                -300.0 + (index / 8) as f32 * 35.0,
            ),
            TeamId(0),
            true,
        ));
    }
    app.update();
    assert!(fighter_count(&mut app) >= 180);
    remove_benchmark_actions(&mut app, &actions);
    let current_tick = app.world().resource::<brawler::timing::SimulationTick>().0;
    let world = app.world_mut();
    let mut flights = world.query::<&mut LobbedFlight>();
    for mut flight in flights.iter_mut(world) {
        flight.lands_at_tick = current_tick;
    }
    app.update();
    let telemetry = app.world().resource::<brawler::combat::WeaponTelemetry>();
    for preset in 2..=4 {
        assert!(
            telemetry
                .accepted_attacks
                .get(&WeaponPresetId(preset))
                .copied()
                .unwrap_or(0)
                > 0,
            "combined fixture did not accept preset {preset}"
        );
    }
    assert_eq!(count_active_effects(&mut app), 100);
    fixed_tick_p95(
        &mut app,
        "combined/100-effects+32-scatter+16-lob+32-blade",
        60,
    );
}

/// Hot Zone composition used by the M09 objective benchmarks: the same server graph with
/// the Hot Zone map/mode/rules installed instead of Wipeout.
fn hot_zone_performance_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::state::app::StatesPlugin,
        ServerPlugins {
            tick_duration: SIMULATION_TICK,
        },
        GameplayPlugin,
        ProtocolPlugin,
        AvianNetworkPlugin,
        AuthoritativeMapPlugin,
        AuthoritativeMovementPlugin,
        ServerNetworkPlugin,
        brawler::matchplay::AuthoritativeMatchPlugin,
        brawler::terrain::AuthoritativeTerrainPlugin,
    ));
    app.insert_resource(brawler::server::match_lifecycle_rules_for_profile(
        brawler::config::MatchRulesProfile::ProcessVerification,
    ));
    app.insert_resource(brawler::matchplay::hot_zone_setup_for_composition());
    // Production target progress keeps a 120-tick controlled window below threshold so
    // the per-tick delta can be measured without completing the match mid-run.
    app.insert_resource(brawler::matchplay::hot_zone_rules_for_profile(
        brawler::config::MatchRulesProfile::Production,
    ));
    app.insert_resource(brawler::map::ServerMapSelection {
        preset_id: brawler::map::HOT_ZONE_MAP_PRESET,
    });
    app.insert_resource(ServerNetworkConfig {
        transport: NetworkTransport::Crossbeam,
        ..default()
    });
    app.insert_resource(TimeUpdateStrategy::ManualDuration(SIMULATION_TICK));
    app.add_plugins(brawler::matchplay::HotZoneModePlugin);
    app.update();
    app
}

fn hot_zone_progress(app: &mut App) -> [u16; 2] {
    let world = app.world_mut();
    let mut roots = world.query_filtered::<&brawler::matchplay::HotZoneState, With<MatchRoot>>();
    roots
        .single(world)
        .expect("one hot zone state")
        .progress_ticks
}

/// Compare fixed-tick cost across the three objective occupancy states at supported
/// participant capacity, proving evaluation correctness under load: exactly one progress
/// unit per controlled tick, none when empty or contested.
#[test]
fn m09_hot_zone_objective_states_stay_within_fixed_tick_budget() {
    let mut app = hot_zone_performance_app();
    let match_id = {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&mut MatchState, With<MatchRoot>>();
        let mut state = roots.single_mut(world).expect("one match root");
        state.phase = MatchPhase::Active {
            ends_at_tick: u64::MAX,
        };
        state.match_id
    };
    let mut owners = Vec::new();
    for (index, (preset, position, team)) in [
        (1, Vec2::new(-600.0, 0.0), TeamId(0)),
        (2, Vec2::new(-600.0, 200.0), TeamId(0)),
        (3, Vec2::new(600.0, 0.0), TeamId(1)),
        (4, Vec2::new(600.0, 200.0), TeamId(1)),
    ]
    .into_iter()
    .enumerate()
    {
        let entity = spawn_m05_fighter(
            &mut app,
            40_000 + u64::try_from(index).expect("benchmark index fits"),
            preset,
            position,
            team,
            false,
        );
        app.world_mut().entity_mut(entity).insert((
            MatchParticipant {
                match_id,
                ready: true,
                restart_ready: false,
            },
            MatchMember(match_id),
            ActiveCombatant,
        ));
        owners.push((entity, position));
    }
    app.update();
    remove_benchmark_actions(
        &mut app,
        &owners.iter().map(|(entity, _)| *entity).collect::<Vec<_>>(),
    );

    // Empty zone: nobody advances.
    fixed_tick_p95(&mut app, "m09-hot-zone-empty", 120);
    assert_eq!(hot_zone_progress(&mut app), [0, 0]);

    // Controlled zone: two team-1 fighters inside advance exactly one unit per tick.
    for (index, (entity, _)) in owners[0..2].iter().enumerate() {
        app.world_mut()
            .entity_mut(*entity)
            .insert(Position::from_xy(0.0, -100.0 - 20.0 * index as f32));
    }
    app.update();
    let before = hot_zone_progress(&mut app);
    fixed_tick_p95(&mut app, "m09-hot-zone-controlled", 120);
    let after = hot_zone_progress(&mut app);
    assert_eq!(
        after[0] - before[0],
        120,
        "one progress unit per controlled tick"
    );
    assert_eq!(after[1], before[1]);

    // Contested zone: both teams present, neither advances.
    for (index, (entity, _)) in owners[2..4].iter().enumerate() {
        app.world_mut()
            .entity_mut(*entity)
            .insert(Position::from_xy(0.0, 100.0 + 20.0 * index as f32));
    }
    app.update();
    let before = hot_zone_progress(&mut app);
    fixed_tick_p95(&mut app, "m09-hot-zone-contested", 120);
    let after = hot_zone_progress(&mut app);
    assert_eq!(after, before, "contested time advances neither team");
}

/// One near-maximum destructible recipe: four engine-maximal reservations filling the
/// playable grid above a clear spawn strip, optionally shifted off the chunk lattice.
fn maximum_terrain_resolved_map(off_grid: bool) -> brawler::map::ResolvedMap {
    let catalog = brawler::map::MapContentCatalog::embedded().expect("embedded map catalog");
    let mut recipe = catalog.presets[0].recipe.clone();
    recipe.recipe_id = brawler::map::MapRecipeId(70);
    recipe.revision = 1;
    // The engine-maximum playable size. Aligned, the span intersects 16x12 global chunks;
    // the same size at an arbitrary offset intersects 17x13 = 221, the chunk ceiling.
    let origin = if off_grid {
        Vec2::new(-100.0, -100.0)
    } else {
        Vec2::ZERO
    };
    recipe.playable_bounds = brawler::map::AxisAlignedMapRect {
        min: origin,
        max: origin + Vec2::new(4096.0, 3072.0),
    };
    recipe.camera_bounds = recipe.playable_bounds;
    recipe.geometry.clear();
    recipe.visuals.clear();
    recipe.entities.clear();
    // Near-complete destructible coverage built from four engine-legal rectangles
    // (extent <= 2048 each): a left column reaching the bottom edge keeps chunk row -1
    // populated on the off-grid map, a right column starting one chunk-row up leaves the
    // bottom-right corner clear for the capacity profile's spawn points and fighters.
    // Every tile edge falls between cell centers, so no cell is selected twice.
    let tiles: [(f32, f32, f32, f32); 4] = if off_grid {
        [
            (924.0, 928.0, 1020.0, 1024.0),
            (2968.0, 968.0, 1024.0, 984.0),
            (924.0, 2460.0, 1020.0, 508.0),
            (2968.0, 2460.0, 1024.0, 508.0),
        ]
    } else {
        [
            (1024.0, 1024.0, 1024.0, 1024.0),
            (3072.0, 1064.0, 1024.0, 984.0),
            (1024.0, 2560.0, 1024.0, 512.0),
            (3072.0, 2560.0, 1024.0, 512.0),
        ]
    };
    // The clear corner notch: y in [-96, -16) off-grid, [0, 80) aligned.
    let spawn_y = if off_grid { -56.0 } else { 40.0 };
    recipe.regions = tiles
        .iter()
        .enumerate()
        .map(
            |(index, (center_x, center_y, half_x, half_y))| brawler::map::MapRegionPlacement {
                placement_id: brawler::map::MapPlacementId(900 + u32::try_from(index).unwrap()),
                region_id: brawler::map::RegionId(1),
                profile_id: brawler::map::RegionProfileId(1),
                presentation_profile_id: brawler::map::MapPresentationProfileId(3),
                position: Vec2::new(*center_x, *center_y),
                rotation: 0.0,
                shape: brawler::map::MapShape::Rectangle {
                    half_extents: Vec2::new(*half_x, *half_y),
                },
            },
        )
        .collect();
    let bounds = recipe.playable_bounds;
    recipe.spawn_areas = vec![
        brawler::map::TeamSpawnArea {
            placement_id: brawler::map::MapPlacementId(910),
            team_slot: 0,
            bounds: brawler::map::AxisAlignedMapRect {
                min: Vec2::new(bounds.max.x - 1848.0, spawn_y - 36.0),
                max: Vec2::new(bounds.max.x - 1440.0, spawn_y + 36.0),
            },
        },
        brawler::map::TeamSpawnArea {
            placement_id: brawler::map::MapPlacementId(911),
            team_slot: 1,
            bounds: brawler::map::AxisAlignedMapRect {
                min: Vec2::new(bounds.max.x - 596.0, spawn_y - 36.0),
                max: Vec2::new(bounds.max.x - 188.0, spawn_y + 36.0),
            },
        },
    ];
    recipe.spawn_points.clear();
    for team_slot in 0..=1_u8 {
        let base_x = if team_slot == 0 {
            bounds.max.x - 1796.0
        } else {
            bounds.max.x - 544.0
        };
        for offset in [0.0_f32, 128.0, 256.0] {
            let index = recipe.spawn_points.len();
            let x = base_x + offset;
            recipe.spawn_points.push(brawler::map::TeamSpawnPoint {
                placement_id: brawler::map::MapPlacementId(920 + u32::try_from(index).unwrap()),
                spawn_point_id: brawler::map::SpawnPointId(200 + u16::try_from(index).unwrap()),
                team_slot,
                position: Vec2::new(x, spawn_y),
                // Both teams spawn from the right-hand notch and face the map
                // interior; the validator requires a positive facing-to-center dot.
                facing: std::f32::consts::PI,
            });
        }
    }
    recipe.mode_anchors.clear();
    brawler::map::resolve_map_recipe(
        &recipe,
        None,
        brawler::map::MapInstanceId(41),
        &catalog,
        &brawler::map::MapLayoutRequirements::wipeout(),
        brawler::map::EngineMapLimits {
            max_destructible_reservations: 6,
            ..brawler::map::EngineMapLimits::default()
        },
    )
    .expect("maximum terrain recipe resolves")
}

fn terrain_scale(app: &mut App) -> (usize, u32) {
    let world = app.world_mut();
    let mut chunks = world.query::<&brawler::terrain::TerrainChunkState>();
    let count = chunks.iter(world).count();
    let cells: u32 = chunks.iter(world).map(|state| state.current.count()).sum();
    (count, cells)
}

#[test]
fn m10_aligned_and_off_grid_maximum_terrain_stay_within_fixed_tick_budget() {
    for off_grid in [false, true] {
        let mut app = performance_app();
        let resolved = maximum_terrain_resolved_map(off_grid);
        brawler::map::install_resolved_map(app.world_mut(), resolved)
            .expect("maximum map installs");
        app.update();
        let (chunks, cells) = terrain_scale(&mut app);
        let expected_chunks = if off_grid {
            brawler::terrain::MAX_TERRAIN_CHUNKS
        } else {
            192
        };
        assert_eq!(
            chunks, expected_chunks,
            "maximum map allocates every ceiling chunk (off_grid={off_grid})"
        );
        assert!(
            cells >= 150_000,
            "near-ceiling occupied cells: {cells} (off_grid={off_grid})"
        );
        assert!(
            cells as usize <= brawler::terrain::MAX_TERRAIN_CELLS,
            "occupied cells stay under the engine ceiling: {cells} (off_grid={off_grid})"
        );
        let telemetry = app
            .world()
            .resource::<brawler::terrain::telemetry::TerrainTelemetry>();
        assert_eq!(telemetry.aggregates.defensive_repairs, 0);
        fixed_tick_p95(
            &mut app,
            if off_grid {
                "m10-off-grid-max-terrain"
            } else {
                "m10-aligned-max-terrain"
            },
            60,
        );
    }
}

#[test]
fn m10_24_fighters_and_24_simultaneous_seam_brushes_stay_within_fixed_tick_budget() {
    let mut app = performance_app();
    // Admit the full 24-brush ceiling in one tick for the worst-placement burst.
    app.world_mut()
        .remove_resource::<brawler::matchplay::ResolvedMatchCapacity>();
    app.world_mut()
        .insert_resource(brawler::terrain::authority::TerrainAdmissionCapacity(24));
    let resolved = maximum_terrain_resolved_map(true);
    brawler::map::install_resolved_map(app.world_mut(), resolved).expect("maximum map installs");
    // The maximum map's clear corner notch is the only initially clear ground: place
    // the 24-fighter capacity profile there, clear of every destructible cell.
    let fighters = &brawler::combat::FighterDefinitions::default().clone();
    let weapons = app
        .world()
        .resource::<brawler::combat::WeaponDefinitions>()
        .clone();
    let mut active = Vec::with_capacity(24);
    for index in 0..24_u16 {
        let (fighter_id, build, team, health, weapon) = brawler::combat::default_fighter_runtime(
            TeamId(u8::try_from(index % 2).unwrap()),
            fighters,
            &weapons,
        );
        let position = Vec2::new(2000.0 + f32::from(index) * 80.0, -56.0);
        let entity = app
            .world_mut()
            .spawn((
                Fighter,
                PlayerId(u64::from(index) + 1),
                NetworkEntityId(u64::from(index) + 1),
                fighter_id,
                build,
                team,
                health,
                weapon,
                Position::from_xy(position.x, position.y),
                Rotation::IDENTITY,
                LinearVelocity::default(),
                AngularVelocity::default(),
                Collider::circle(24.0),
                RigidBody::Kinematic,
            ))
            .id();
        app.world_mut().entity_mut(entity).insert((
            ActiveCombatant,
            CustomPositionIntegration,
            fighter_collision_layers(),
            InputFreshness::default(),
            Transform::from_translation(position.extend(0.0)),
        ));
        active.push(entity);
    }
    let _ = &active;
    app.update();
    let (chunks, _) = terrain_scale(&mut app);
    assert_eq!(
        chunks,
        brawler::terrain::MAX_TERRAIN_CHUNKS,
        "the seam-brush workload runs on the full 221-chunk ceiling map"
    );
    // Worst placements: 24 brushes centered across chunk seams of the maximum map,
    // spread far enough apart that every one erases fresh cells in the same tick.
    let mut attack = 1_u64;
    let resolved = maximum_terrain_resolved_map(true);
    brawler::map::install_resolved_map(app.world_mut(), resolved).expect("maximum map installs");
    app.update();
    for row in 0..4_u16 {
        for column in 0..6_u16 {
            #[allow(clippy::cast_precision_loss)]
            let position = Vec2::new(
                260.0 + f32::from(column) * 512.0,
                420.0 + f32::from(row) * 512.0,
            );
            app.world_mut()
                .resource_mut::<brawler::combat::CombatWorldEffectFacts>()
                .0
                .push(brawler::combat::CombatWorldEffectFact {
                    tick: 0,
                    source: AttackSource {
                        kind: brawler::combat::CombatSourceKind::PrimaryWeapon,
                        attack_id: AttackId(attack),
                        player_id: PlayerId(1),
                        owner_network_entity_id: NetworkEntityId(1),
                        team_id: TeamId(0),
                        recipe_fingerprint: Default::default(),
                        presentation_profile_id: brawler::combat::WeaponPresentationProfileId(3),
                        legacy_compatibility: false,
                        source_preset_id: None,
                        origin: brawler::combat::WorldPoint { x: 0.0, y: 0.0 },
                        facing: 0.0,
                    },
                    delivery_index: 0,
                    effect_index: 0,
                    position: brawler::combat::WorldPoint {
                        x: position.x,
                        y: position.y,
                    },
                    effect: brawler::combat::WorldEffectDefinition::DestroyTerrain { radius: 48.0 },
                });
            attack += 1;
        }
    }
    fixed_tick_p95(&mut app, "m10-24-seam-brushes-one-tick", 12);
    let telemetry = app
        .world()
        .resource::<brawler::terrain::telemetry::TerrainTelemetry>();
    assert_eq!(telemetry.aggregates.applied_brushes, 24);
    assert_eq!(telemetry.aggregates.defensive_repairs, 0);
    assert!(
        telemetry.aggregates.max_brushes_in_one_tick <= 24,
        "the engine ceiling holds under the worst burst"
    );
}

fn worst_placement_fact(attack: u64, position: Vec2) -> brawler::combat::CombatWorldEffectFact {
    brawler::combat::CombatWorldEffectFact {
        tick: 1,
        source: brawler::combat::AttackSource {
            kind: brawler::combat::CombatSourceKind::PrimaryWeapon,
            attack_id: brawler::combat::AttackId(attack),
            player_id: brawler::protocol::PlayerId(1),
            owner_network_entity_id: brawler::protocol::NetworkEntityId(1),
            team_id: brawler::combat::TeamId(0),
            recipe_fingerprint: Default::default(),
            presentation_profile_id: brawler::combat::WeaponPresentationProfileId(3),
            legacy_compatibility: false,
            source_preset_id: None,
            origin: brawler::combat::WorldPoint { x: 0.0, y: 0.0 },
            facing: 0.0,
        },
        delivery_index: 0,
        effect_index: 0,
        position: brawler::combat::WorldPoint {
            x: position.x,
            y: position.y,
        },
        effect: brawler::combat::WorldEffectDefinition::DestroyTerrain { radius: 48.0 },
    }
}

#[test]
fn m10_varied_team_capacities_derive_admission_and_admit_without_deferral() {
    for (team_count, per_team, expected_admission) in [
        (2_u8, 2_u8, 4_usize),
        (3, 2, 6),
        (4, 3, 12),
        (2, 12, 24),
        (8, 3, 24),
        (24, 1, 24),
    ] {
        let mut app = performance_app();
        let resolved = maximum_terrain_resolved_map(true);
        brawler::map::install_resolved_map(app.world_mut(), resolved)
            .expect("maximum map installs");
        let rules = brawler::matchplay::MatchLifecycleRules {
            team_count,
            minimum_participants_per_team: 1,
            maximum_participants_per_team: per_team,
            ..brawler::matchplay::MatchLifecycleRules::default()
        };
        let capacity = brawler::matchplay::ResolvedMatchCapacity::from_rules(&rules)
            .expect("varied-team capacity resolves");
        assert_eq!(
            u32::from(capacity.maximum_active_fighters),
            u32::from(team_count) * u32::from(per_team),
            "{team_count} teams x {per_team}"
        );
        app.world_mut().insert_resource(capacity);
        app.update();
        assert_eq!(
            app.world()
                .resource::<brawler::terrain::authority::TerrainAdmissionCapacity>()
                .0,
            expected_admission,
            "{team_count} teams x {per_team} derives its admission ceiling"
        );
        // Exactly the admitted ceiling of spread worst-placement brushes applies whole
        // in one fixed tick: no deferral, no rejection, no defensive repair.
        for attack in 0..expected_admission {
            let position = Vec2::new(
                300.0 + (attack % 8) as f32 * 220.0,
                600.0 + (attack / 8) as f32 * 220.0,
            );
            app.world_mut()
                .resource_mut::<brawler::combat::CombatWorldEffectFacts>()
                .0
                .push(worst_placement_fact(
                    u64::try_from(attack).unwrap() + 1,
                    position,
                ));
        }
        app.update();
        let world = app.world();
        let telemetry = world.resource::<brawler::terrain::telemetry::TerrainTelemetry>();
        assert_eq!(
            telemetry.aggregates.applied_brushes as usize, expected_admission,
            "{team_count} teams x {per_team} admits its whole ceiling"
        );
        assert!(
            world
                .resource::<brawler::terrain::PendingTerrainBrushes>()
                .queue
                .is_empty(),
            "{team_count} teams x {per_team} defers nothing"
        );
        assert_eq!(telemetry.aggregates.defensive_repairs, 0);
    }
}

#[test]
fn m10_recovery_serialization_and_client_image_painting_stay_within_budget() {
    let resolved = maximum_terrain_resolved_map(true);
    let layout = brawler::map::resolve_initial_terrain(
        resolved.snapshot.playable_bounds,
        &resolved.snapshot.geometry,
        &resolved.snapshot.regions,
        &resolved.snapshot.spawn_points,
        &resolved.snapshot.mode_anchors,
        brawler::map::EngineMapLimits::default(),
    )
    .expect("layout resolves");
    let generation = brawler::terrain::TerrainGeneration {
        map_instance_id: brawler::map::MapInstanceId(41),
        match_id: brawler::matchplay::MatchId(1),
        terrain_fingerprint: layout.terrain_fingerprint,
    };
    assert_eq!(layout.chunks.len(), brawler::terrain::MAX_TERRAIN_CHUNKS);

    let start = Instant::now();
    let mut sizes = Vec::new();
    for _ in 0..20 {
        let snapshot = brawler::terrain::grid::recovery_snapshot(&layout.chunks, generation, 0);
        sizes.push(
            brawler::terrain::grid::recovery_snapshot_bytes(&snapshot)
                .expect("snapshot serializes"),
        );
    }
    let serialization = start.elapsed() / 20;
    assert!(
        sizes
            .iter()
            .all(|bytes| *bytes <= brawler::terrain::MAX_TERRAIN_RECOVERY_BYTES),
        "every snapshot stays under the wire ceiling"
    );

    let full = layout
        .chunks
        .values()
        .next()
        .copied()
        .expect("allocated chunk bits");
    let start = Instant::now();
    for _ in 0..20 {
        for chunk_bits in layout.chunks.values() {
            let _ = brawler::terrain::paint_chunk_pixels(
                chunk_bits,
                Some(&full),
                Some(&full),
                Some(&full),
                Some(&full),
            );
        }
    }
    let painting = start.elapsed() / 20 / layout.chunks.len().max(1) as u32;
    assert!(
        painting.as_micros() < 500,
        "one chunk image paints in {painting:?}"
    );
    println!(
        "m10 recovery serialize p50={serialization:?} bytes={} chunks={} image-per-chunk={painting:?}",
        sizes[0],
        layout.chunks.len()
    );
}
