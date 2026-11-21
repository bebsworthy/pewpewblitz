//! Client presentation for replicated Shard Rush state: loose shards, the source pad, and the
//! crystals a carrier holds. Everything here reads replicated facts and owns only visuals.

use super::{Material3dAssets, Position, Primitive3dAssets, ground_position};
use bevy::prelude::*;

/// Most crystals stacked over one carrier; larger counts read from the HUD totals.
const MAX_CARRIER_CRYSTALS: u8 = 8;
const SHARD_HOVER: f32 = 16.0;
const CARRIER_CROWN_HEIGHT: f32 = 74.0;

#[derive(Component)]
pub(super) struct ShardVisual {
    owner: Entity,
}

#[derive(Component)]
pub(super) struct ShardSourceVisual {
    map: Entity,
}

#[derive(Component)]
pub(super) struct CarrierCrownVisual {
    owner: Entity,
    shown: u8,
}

fn crystal(primitives: &Primitive3dAssets, materials: &Material3dAssets, size: f32) -> impl Bundle {
    (
        Mesh3d(primitives.unit_cuboid.clone()),
        MeshMaterial3d(materials.shard_crystal.clone()),
        Transform::from_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4))
            .with_scale(Vec3::new(size * 0.6, size * 0.6, size * 0.6)),
    )
}

fn source_position(snapshot: &brawler::map::ResolvedMapSnapshot) -> Option<Vec2> {
    snapshot.mode_anchors.iter().find_map(|anchor| {
        let brawler::map::MapModeAnchorKind::ShardRushSource { center_half_cell } = anchor.kind
        else {
            return None;
        };
        snapshot.dimensions.half_cell_world(center_half_cell)
    })
}

/// Loose shards: a spinning crystal over a ground glow, one per replicated shard.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy system parameters own the bounded shard visual lifecycle"
)]
pub(super) fn reconcile_shard_visuals(
    mut commands: Commands,
    time: Res<Time>,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    shards: Query<(Entity, &Position), With<brawler::matchplay::Shard>>,
    mut visuals: Query<(Entity, &ShardVisual, &mut Transform)>,
) {
    let seconds = time.elapsed_secs();
    for (visual_entity, visual, mut transform) in &mut visuals {
        let Ok((_, position)) = shards.get(visual.owner) else {
            commands.entity(visual_entity).despawn();
            continue;
        };
        let bob = (seconds * 3.0 + position.0.x * 0.01).sin() * 3.0;
        transform.translation = ground_position(position.0) + Vec3::Y * (SHARD_HOVER + bob);
        transform.rotation = Quat::from_rotation_y(seconds * 1.6);
    }
    for (owner, position) in &shards {
        if visuals.iter().any(|(_, visual, _)| visual.owner == owner) {
            continue;
        }
        commands
            .spawn((
                ShardVisual { owner },
                Transform::from_translation(ground_position(position.0) + Vec3::Y * SHARD_HOVER),
                Visibility::default(),
                Name::new("shard visual"),
            ))
            .with_children(|parent| {
                parent.spawn(crystal(&primitives, &materials, 18.0));
                parent.spawn((
                    Mesh3d(primitives.area_disc.clone()),
                    MeshMaterial3d(materials.pickup_glow.clone()),
                    Transform::from_xyz(0.0, -SHARD_HOVER + 1.0, 0.0).with_scale(Vec3::splat(16.0)),
                    Name::new("shard ground glow"),
                ));
            });
    }
}

/// The source pad on any map whose snapshot carries a Shard Rush source anchor.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters own the source pad lifecycle"
)]
pub(super) fn reconcile_shard_source_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    maps: Query<(Entity, &brawler::map::ResolvedMapSnapshot), With<brawler::map::MapRoot>>,
    sources: Query<(Entity, &ShardSourceVisual)>,
) {
    for (visual_entity, visual) in &sources {
        if maps.get(visual.map).is_err() {
            commands.entity(visual_entity).despawn();
        }
    }
    for (map, snapshot) in &maps {
        if sources.iter().any(|(_, visual)| visual.map == map) {
            continue;
        }
        let Some(source) = source_position(snapshot) else {
            continue;
        };
        commands
            .spawn((
                ShardSourceVisual { map },
                Transform::from_translation(ground_position(source) + Vec3::Y * 0.6),
                Visibility::default(),
                Name::new("shard source pad"),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(primitives.area_ring.clone()),
                    MeshMaterial3d(materials.shard_crystal.clone()),
                    Transform::from_scale(Vec3::splat(44.0)),
                    Name::new("shard source ring"),
                ));
                parent.spawn((
                    Mesh3d(primitives.area_disc.clone()),
                    MeshMaterial3d(materials.pickup_glow.clone()),
                    Transform::from_scale(Vec3::splat(38.0)),
                    Name::new("shard source glow"),
                ));
            });
    }
}

/// A ring of small crystals over every replicated carrier, one per carried shard up to a cap.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters own the carrier crown lifecycle"
)]
pub(super) fn reconcile_carrier_crowns(
    mut commands: Commands,
    time: Res<Time>,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    carriers: Query<(Entity, &Position, &brawler::matchplay::ShardCarrier)>,
    mut crowns: Query<(Entity, &CarrierCrownVisual, &mut Transform)>,
) {
    let seconds = time.elapsed_secs();
    for (crown_entity, crown, mut transform) in &mut crowns {
        let current = carriers
            .get(crown.owner)
            .ok()
            .filter(|(_, _, carrier)| carrier.0.min(MAX_CARRIER_CRYSTALS) == crown.shown);
        let Some((_, position, _)) = current else {
            commands.entity(crown_entity).despawn();
            continue;
        };
        transform.translation = ground_position(position.0) + Vec3::Y * CARRIER_CROWN_HEIGHT;
        transform.rotation = Quat::from_rotation_y(seconds * 2.0);
    }
    for (owner, position, carrier) in &carriers {
        let shown = carrier.0.min(MAX_CARRIER_CRYSTALS);
        if shown == 0
            || crowns
                .iter()
                .any(|(_, crown, _)| crown.owner == owner && crown.shown == shown)
        {
            continue;
        }
        commands
            .spawn((
                CarrierCrownVisual { owner, shown },
                Transform::from_translation(
                    ground_position(position.0) + Vec3::Y * CARRIER_CROWN_HEIGHT,
                ),
                Visibility::default(),
                Name::new("shard carrier crown"),
            ))
            .with_children(|parent| {
                for index in 0..shown {
                    let angle = f32::from(index) / f32::from(shown) * std::f32::consts::TAU;
                    let radius = if shown == 1 { 0.0 } else { 14.0 };
                    let offset = Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius);
                    parent.spawn(crystal(&primitives, &materials, 9.0)).insert(
                        Transform::from_translation(offset)
                            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4))
                            .with_scale(Vec3::splat(5.4)),
                    );
                }
            });
    }
}
