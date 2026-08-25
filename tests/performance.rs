use avian2d::prelude::{
    AngularVelocity, Collider, CollisionLayers, CustomPositionIntegration, LinearVelocity,
    Position, RigidBody, Rotation,
};
use bevy::{prelude::*, time::TimeUpdateStrategy};
use brawler::{
    builds::SelectingBuild,
    combat::{
        ActiveEffects, AttackId, AttackSource, CombatSourceKind, ComposedProjectileRuntime,
        CurrentHealth, ExternalMotion, FighterDefinitionId, FighterDefinitions, LobbedFlight,
        Projectile, SlowEffect, TeamId, WeaponDefinitions, WeaponPhase, WeaponPresetId,
        WeaponState, WorldPoint, default_fighter_runtime,
    },
    config::{NetworkTransport, ServerNetworkConfig},
    gameplay::GameplayPlugin,
    map::AuthoritativeMapPlugin,
    matchplay::{
        ActiveCombatant, MatchMember, MatchParticipant, MatchPhase, MatchRoot, MatchState,
    },
    movement::{
        AuthoritativeMovementPlugin, AvianNetworkPlugin, DESTRUCTIBLE_MAP_LAYER, FIGHTER_LAYER,
        InputFreshness, PROJECTILE_LAYER, STATIC_MAP_LAYER, fighter_collision_layers,
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
            let mut position = Vec2::new(
                -700.0 + f32::from(column) * 155.0,
                -400.0 + f32::from(row) * 88.0,
            );
            if position.x.abs() < 130.0 && position.y.abs() < 130.0 {
                // Keep benchmark fighters clear of the central destructible block.
                position.x += 260.0;
            }
            let (fighter_id, team, health, weapon) = default_fighter_runtime(
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
                    SelectingBuild,
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
    let weapon_catalog = app
        .world()
        .resource::<brawler::combat::WeaponCatalogResource>()
        .0
        .clone();
    let build_catalog =
        brawler::builds::BuildCatalog::embedded().expect("embedded build catalog is valid");
    let base_recipe = build_catalog
        .preset(brawler::builds::BuildPresetId(1))
        .expect("build preset 1 exists")
        .recipe;
    let recipe = brawler::builds::BrawlerBuildRecipe {
        weapon: brawler::builds::WeaponChoice::Preset(WeaponPresetId(preset_id)),
        ..base_recipe
    };
    let loadout = brawler::builds::resolve_build_recipe(
        &build_catalog,
        &weapon_catalog,
        &fighter,
        recipe,
        None,
    )
    .expect("benchmark loadout resolves");
    let entity = app
        .world_mut()
        .spawn((
            Fighter,
            PlayerId(player_id),
            NetworkEntityId(player_id),
            FighterDefinitionId(fighter.id.0),
            loadout.identity,
            loadout.clone(),
            team,
            CurrentHealth(fighter.maximum_health),
            WeaponState {
                ammo: loadout.primary_weapon.recipe.economy.capacity(),
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
    // pass-through to terminate the projectile. These central Feature Yard lanes pass through
    // projectile-passable water while remaining clear of walls and damageable placements; the
    // resolved perimeter and other map colliders still participate in broad-phase work.
    {
        let world = app.world_mut();
        let mut fighters = world.query::<&mut TeamId>();
        for mut team in fighters.iter_mut(world) {
            team.0 = 0;
        }
    }
    let lanes = [-160.0, -128.0, -96.0, -64.0, 64.0, 96.0, 128.0, 160.0];
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
                    kind: CombatSourceKind::PrimaryWeapon,
                    attack_id: AttackId(index as u64 + 1),
                    player_id: PlayerId(1),
                    owner_network_entity_id: NetworkEntityId(1),
                    team_id: TeamId(0),
                    recipe_fingerprint: recipe.recipe_fingerprint,
                    presentation_profile_id: recipe.presentation_profile_id,
                    legacy_compatibility: false,
                    source_preset_id: Some(WeaponPresetId(1)),
                    origin: WorldPoint::from(position),
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
                FIGHTER_LAYER | STATIC_MAP_LAYER | DESTRUCTIBLE_MAP_LAYER,
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
        preset_id: brawler::map::FEATURE_YARD_HOT_ZONE_PRESET,
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

fn heist_performance_app() -> App {
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
    ));
    app.insert_resource(brawler::server::match_lifecycle_rules_for_profile(
        brawler::config::MatchRulesProfile::ProcessVerification,
    ));
    app.insert_resource(brawler::matchplay::MatchModeSetup {
        mode_definition_id: brawler::map::HEIST_MODE_DEFINITION,
        rules_revision: brawler::matchplay::HEIST_RULES_REVISION,
    });
    app.insert_resource(brawler::matchplay::HeistRules::default());
    app.insert_resource(brawler::map::ServerMapSelection {
        preset_id: brawler::map::FEATURE_YARD_HEIST_PRESET,
    });
    app.insert_resource(ServerNetworkConfig {
        transport: NetworkTransport::Crossbeam,
        ..default()
    });
    app.insert_resource(TimeUpdateStrategy::ManualDuration(SIMULATION_TICK));
    app.add_plugins(brawler::matchplay::HeistModePlugin);
    app.update();
    app
}

/// Exercise the bounded objective ingress at the supported 3v3 capacity. Each measured tick
/// supplies one more request than the 64-hit transaction limit, proving both bounded rejection
/// and fixed-tick cost without completing either safe.
#[test]
fn v10_m02_heist_objective_burst_stays_within_fixed_tick_budget() {
    let mut app = heist_performance_app();
    let match_id = {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&mut MatchState, With<MatchRoot>>();
        let mut state = roots.single_mut(world).expect("one match root");
        state.phase = MatchPhase::Active {
            ends_at_tick: u64::MAX,
        };
        state.match_id
    };
    let mut fighters = Vec::new();
    for index in 0_u8..6 {
        let team = TeamId(index % 2);
        let entity = spawn_m05_fighter(
            &mut app,
            50_000 + u64::from(index),
            1,
            Vec2::new(-500.0 + f32::from(index) * 180.0, 300.0),
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
        fighters.push(entity);
    }
    app.update();
    remove_benchmark_actions(&mut app, &fighters);

    let (source, friendly_safe) = {
        let world = app.world_mut();
        let mut sources = world
            .query_filtered::<(&PlayerId, &NetworkEntityId, &TeamId, &Position), With<Fighter>>();
        let (player, network_id, team, position) = sources
            .iter(world)
            .find(|(_, _, team, _)| **team == TeamId(0))
            .expect("team-zero benchmark source");
        let source = AttackSource {
            kind: CombatSourceKind::PrimaryWeapon,
            attack_id: AttackId(1),
            player_id: *player,
            owner_network_entity_id: *network_id,
            team_id: *team,
            recipe_fingerprint: brawler::combat::WeaponRecipeFingerprint::default(),
            presentation_profile_id: brawler::combat::WeaponPresentationProfileId(3),
            legacy_compatibility: false,
            source_preset_id: None,
            origin: WorldPoint::from(position.0),
            facing: 0.0,
        };
        let mut safes = world.query::<&brawler::map::DamageableTargetIdentity>();
        let target = *safes
            .iter(world)
            .find(|target| {
                matches!(
                    target,
                    brawler::map::DamageableTargetIdentity::HeistSafe {
                        defending_team: TeamId(0),
                        ..
                    }
                )
            })
            .expect("friendly benchmark safe");
        (source, target)
    };

    let mut samples = Vec::with_capacity(120);
    for sample in 0_u64..120 {
        let mut pending = app
            .world_mut()
            .resource_mut::<brawler::matchplay::PendingModeObjectiveDamages>();
        for request in 0_u8..65 {
            pending
                .0
                .push(brawler::matchplay::PendingModeObjectiveDamage {
                    source: AttackSource {
                        attack_id: AttackId(10_000 + sample * 65 + u64::from(request)),
                        ..source
                    },
                    target: friendly_safe,
                    requested_damage: 1,
                    delivery_index: 0,
                    bundle_index: 0,
                    effect_index: request,
                });
        }
        let started = Instant::now();
        app.update();
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    let p95 = samples[(samples.len() * 95) / 100];
    println!("v10-m02-heist-65-objective-hits: p95={p95:?}");
    assert!(p95 < SIMULATION_TICK);
    let telemetry = app.world().resource::<brawler::matchplay::HeistTelemetry>();
    assert_eq!(telemetry.capacity_rejections, 120);
    assert_eq!(telemetry.invalid_rejections, 64 * 120);
    assert_eq!(telemetry.accepted_hits, 0);
}

#[test]
fn map_dynamic_maximum_state_serialization_stays_bounded() {
    let state = brawler::map::MapDynamicState {
        map_instance_id: brawler::map::MapInstanceId(1),
        generation: 1,
        revision: 512,
        terminal_states: (1..=512)
            .map(|placement_id| brawler::map::MapPlacementTransition {
                placement_id: brawler::map::MapPlacementId(placement_id),
                outcome: brawler::map::MapPlacementOutcome::Removed,
            })
            .collect(),
    };
    let encoded = postcard::to_allocvec(&state).expect("maximum map dynamic state serializes");
    assert!(encoded.len() <= 64 * 1024);

    let mut samples = Vec::with_capacity(256);
    for _ in 0..256 {
        let started = Instant::now();
        let bytes = postcard::to_allocvec(&state).expect("repeat serialization succeeds");
        assert_eq!(bytes.len(), encoded.len());
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    let p95 = samples[samples.len() * 95 / 100];
    println!(
        "maximum map dynamic state: placements=512 bytes={} serialization_p95={p95:?}",
        encoded.len()
    );
    assert!(p95 <= std::time::Duration::from_millis(5));
}
