use avian2d::prelude::{
    AngularVelocity, Collider, CollisionLayers, CustomPositionIntegration, LinearVelocity,
    Position, RigidBody, Rotation,
};
use bevy::{prelude::*, time::TimeUpdateStrategy};
use brawler::{
    combat::{Projectile, ProjectileRuntime, ProjectileSource, ShotId, TeamId, WeaponDefinitionId},
    config::{NetworkTransport, ServerNetworkConfig},
    gameplay::GameplayPlugin,
    movement::{
        AuthoritativeMovementPlugin, AvianNetworkPlugin, DESTRUCTIBLE_TERRAIN_LAYER, FIGHTER_LAYER,
        INDESTRUCTIBLE_TERRAIN_LAYER, InputFreshness, PROJECTILE_LAYER, fighter_collision_layers,
    },
    protocol::{Fighter, NetworkEntityId, PlayerId, ProtocolPlugin},
    server::ServerNetworkPlugin,
    timing::SIMULATION_TICK,
};
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
        AuthoritativeMovementPlugin,
        ServerNetworkPlugin,
    ));
    app.insert_resource(ServerNetworkConfig {
        transport: NetworkTransport::Crossbeam,
        ..default()
    });
    app.insert_resource(TimeUpdateStrategy::ManualDuration(SIMULATION_TICK));
    app.update();
    app
}

fn spawn_headless_fighters(app: &mut App) {
    for row in 0_u16..10 {
        for column in 0_u16..10 {
            let position = Vec2::new(
                -700.0 + f32::from(column) * 155.0,
                -400.0 + f32::from(row) * 88.0,
            );
            app.world_mut().spawn((
                Fighter,
                Position::from_xy(position.x, position.y),
                Rotation::IDENTITY,
                LinearVelocity::default(),
                AngularVelocity::default(),
                Collider::circle(24.0),
                RigidBody::Kinematic,
                CustomPositionIntegration,
                fighter_collision_layers(),
                InputFreshness::default(),
                Transform::from_translation(position.extend(0.0)),
            ));
        }
    }
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
fn one_hundred_headless_fighters_and_two_hundred_projectiles_stay_within_fixed_tick_budget() {
    let mut app = performance_app();
    spawn_headless_fighters(&mut app);
    for index in 0_u16..200 {
        let position = Vec2::new(-10_000.0 + f32::from(index) * 2.0, 10_000.0);
        app.world_mut().spawn((
            Projectile,
            ProjectileSource {
                shot_id: ShotId(u64::from(index) + 1),
                player_id: PlayerId(1),
                owner_network_entity_id: NetworkEntityId(1),
                team_id: TeamId(0),
                weapon_definition_id: WeaponDefinitionId(1),
            },
            ProjectileRuntime {
                owner_entity: Entity::PLACEHOLDER,
                velocity: Vec2::X * 900.0,
                travelled: 0.0,
                expires_at_tick: u64::MAX,
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
