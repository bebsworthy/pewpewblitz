use avian2d::prelude::{
    AngularVelocity, Collider, CustomPositionIntegration, LinearVelocity, Position, RigidBody,
    Rotation,
};
use bevy::{prelude::*, time::TimeUpdateStrategy};
use brawler::{
    config::{NetworkTransport, ServerNetworkConfig},
    gameplay::GameplayPlugin,
    movement::{
        AuthoritativeMovementPlugin, AvianNetworkPlugin, InputFreshness, fighter_collision_layers,
    },
    protocol::{Fighter, ProtocolPlugin},
    server::ServerNetworkPlugin,
    timing::SIMULATION_TICK,
};
use lightyear::prelude::server::ServerPlugins;
use std::time::Instant;

#[test]
fn one_hundred_headless_fighters_stay_within_fixed_tick_budget() {
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

    for row in 0..10 {
        for column in 0..10 {
            let position = Vec2::new(-700.0 + column as f32 * 155.0, -400.0 + row as f32 * 88.0);
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
        "p95 fixed tick {:?} exceeded {:?}",
        p95,
        SIMULATION_TICK
    );
}
