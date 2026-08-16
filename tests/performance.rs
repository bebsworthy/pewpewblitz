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
            let position = Vec2::new(
                -700.0 + f32::from(column) * 155.0,
                -400.0 + f32::from(row) * 88.0,
            );
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
