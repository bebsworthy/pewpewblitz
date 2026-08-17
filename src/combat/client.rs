//! Client-only combat presentation geometry.
#![allow(clippy::wildcard_imports)]

use super::*;
use bevy::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::env;
use std::path::PathBuf;
use std::time::Instant;

pub const MAX_PREVIEW_SEGMENTS: usize = 24;

#[cfg(feature = "client")]
#[allow(clippy::too_many_lines)]
pub(super) fn preview_segments(
    origin: Vec2,
    facing: f32,
    aim_distance: Option<f32>,
    resolved: &ResolvedWeapon,
    map: &crate::map::ResolvedMapSnapshot,
    terrain_chunks: &BTreeMap<crate::terrain::TerrainChunkId, crate::terrain::TerrainBits>,
) -> Vec<(Vec2, f32, Vec2, Color)> {
    let mut segments = Vec::with_capacity(MAX_PREVIEW_SEGMENTS);
    match resolved.recipe.delivery {
        DeliveryMethod::Straight { range, .. } => {
            let angles = match resolved.recipe.firing {
                FiringPattern::Single => vec![facing],
                FiringPattern::Spread {
                    delivery_count,
                    total_angle_degrees,
                } => spread_angles(facing, delivery_count, total_angle_degrees),
            };
            let is_spread = angles.len() > 1;
            let preview_angles = if is_spread {
                vec![angles[0], *angles.last().expect("spread has an angle")]
            } else {
                angles.clone()
            };
            for angle in preview_angles {
                let direction = Vec2::from_angle(angle);
                let line_range = if is_spread { range * 0.78 } else { range };
                segments.push((
                    origin + direction * (line_range * 0.5),
                    angle,
                    Vec2::new(line_range, if is_spread { 2.0 } else { 3.0 }),
                    if is_spread {
                        Color::srgba(1.0, 0.72, 0.2, 0.25)
                    } else {
                        Color::srgba(0.95, 0.85, 0.25, 0.30)
                    },
                ));
            }
            let marker_color = if is_spread {
                Color::srgba(1.0, 0.72, 0.2, 0.35)
            } else {
                Color::srgba(1.0, 0.9, 0.35, 0.45)
            };
            if is_spread {
                let start = angles[0];
                let end = *angles.last().expect("spread has an angle");
                for index in 0..6 {
                    let a0 = start + (end - start) * index as f32 / 6.0;
                    let a1 = start + (end - start) * (index + 1) as f32 / 6.0;
                    segments.push(segment_between(
                        origin + Vec2::from_angle(a0) * range,
                        origin + Vec2::from_angle(a1) * range,
                        2.0,
                        marker_color,
                    ));
                }
            } else {
                segments.push((
                    origin + Vec2::from_angle(facing) * range,
                    0.0,
                    Vec2::splat(10.0),
                    marker_color,
                ));
            }
        }
        DeliveryMethod::Lobbed {
            distance,
            landing_clearance_radius,
            ..
        } => {
            let direction = Vec2::from_angle(facing);
            let desired =
                origin + direction * aim_distance.unwrap_or(distance).clamp(0.0, distance);
            let bounded = desired.clamp(
                map.playable_bounds.min + Vec2::splat(landing_clearance_radius),
                map.playable_bounds.max - Vec2::splat(landing_clearance_radius),
            );
            let repaired_landing = delivery::repaired_landing_point(
                origin,
                bounded,
                landing_clearance_radius,
                |candidate| {
                    map.geometry.iter().all(|geometry| {
                        !circle_overlaps_map_shape(
                            candidate,
                            landing_clearance_radius,
                            geometry.position,
                            geometry.rotation,
                            geometry.shape,
                        )
                    }) && !crate::terrain::grid::circle_overlaps_occupied(
                        candidate,
                        landing_clearance_radius,
                        terrain_chunks,
                    )
                },
            );
            let landing = repaired_landing.unwrap_or(bounded);
            let landing_color = if repaired_landing.is_none() {
                Color::srgba(1.0, 0.16, 0.16, 0.50)
            } else if landing.distance(bounded) > 0.5 {
                Color::srgba(0.95, 0.35, 1.0, 0.45)
            } else if bounded.distance(desired) > 0.5 {
                Color::srgba(1.0, 0.65, 0.2, 0.40)
            } else {
                Color::srgba(0.35, 0.85, 1.0, 0.34)
            };
            segments.push((
                origin + direction * (origin.distance(landing) * 0.5),
                facing,
                Vec2::new(origin.distance(landing), 2.0),
                landing_color,
            ));
            segments.push((landing, 0.0, Vec2::splat(12.0), landing_color));
            let explosion_radius = resolved
                .recipe
                .payload_bundles
                .iter()
                .find_map(|bundle| match bundle.target {
                    TargetSelection::Area { radius, .. } => Some(radius),
                    TargetSelection::Direct => None,
                })
                .unwrap_or(24.0);
            for index in 0..12 {
                let a0 = std::f32::consts::TAU * index as f32 / 12.0;
                let a1 = std::f32::consts::TAU * (index + 1) as f32 / 12.0;
                segments.push(segment_between(
                    landing + Vec2::from_angle(a0) * explosion_radius,
                    landing + Vec2::from_angle(a1) * explosion_radius,
                    3.0,
                    landing_color,
                ));
            }
        }
        DeliveryMethod::MeleeArc {
            reach,
            angle_degrees,
        } => {
            for angle in [
                facing - angle_degrees.to_radians() * 0.5,
                facing + angle_degrees.to_radians() * 0.5,
            ] {
                let direction = Vec2::from_angle(angle);
                segments.push((
                    origin + direction * (reach * 0.5),
                    angle,
                    Vec2::new(reach, 3.0),
                    Color::srgba(1.0, 0.35, 0.35, 0.32),
                ));
            }
            for index in 0..8 {
                let a0 = facing - angle_degrees.to_radians() * 0.5
                    + angle_degrees.to_radians() * index as f32 / 8.0;
                let a1 = facing - angle_degrees.to_radians() * 0.5
                    + angle_degrees.to_radians() * (index + 1) as f32 / 8.0;
                segments.push(segment_between(
                    origin + Vec2::from_angle(a0) * reach,
                    origin + Vec2::from_angle(a1) * reach,
                    4.0,
                    Color::srgba(1.0, 0.25, 0.25, 0.30),
                ));
            }
        }
    }
    segments.truncate(MAX_PREVIEW_SEGMENTS);
    segments
}

fn segment_between(start: Vec2, end: Vec2, width: f32, color: Color) -> (Vec2, f32, Vec2, Color) {
    let delta = end - start;
    (
        start.midpoint(end),
        delta.y.atan2(delta.x),
        Vec2::new(delta.length(), width),
        color,
    )
}

fn circle_overlaps_map_shape(
    center: Vec2,
    radius: f32,
    shape_center: Vec2,
    rotation: f32,
    shape: crate::map::MapShape,
) -> bool {
    match shape {
        crate::map::MapShape::Circle {
            radius: shape_radius,
        } => center.distance_squared(shape_center) < (radius + shape_radius).powi(2),
        crate::map::MapShape::Rectangle { half_extents } => {
            let local = Vec2::from_angle(-rotation).rotate(center - shape_center);
            let closest = local.clamp(-half_extents, half_extents);
            local.distance_squared(closest) < radius * radius
        }
    }
}

#[cfg(feature = "client")]
pub struct ClientCombatPlugin;

/// A combat cue that passed the bounded client deduplication gate.
#[derive(Message, Clone, Debug)]
pub(crate) struct DeduplicatedCombatCue(pub CombatCue);

#[cfg(feature = "client")]
impl Plugin for ClientCombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FighterDefinitions>()
            .init_resource::<WeaponDefinitions>()
            .init_resource::<RecentCombatEvents>()
            .init_resource::<ClientCombatObservation>()
            .init_resource::<ClientCombatEvidenceStatus>()
            .add_message::<DeduplicatedCombatCue>()
            .add_systems(Startup, validate_definitions)
            .add_systems(
                Update,
                (
                    receive_combat_cues,
                    receive_combat_evidence_checkpoints,
                    ensure_projectile_visuals,
                    ensure_sentry_visuals,
                    ensure_dash_trails,
                    sync_projectile_visuals,
                    sync_sentry_visuals,
                    sync_dash_trails,
                    update_weapon_preview,
                    update_health_bars,
                    update_durable_effect_markers,
                    update_combat_hud,
                    update_combat_effects,
                    capture_client_combat_checkpoints,
                    record_headless_combat_observation,
                )
                    .chain(),
            );
    }
}

#[cfg(feature = "client")]
#[derive(Component)]
struct DashTrailVisual {
    target: Entity,
    last_position: Vec2,
}

#[cfg(feature = "client")]
fn ensure_dash_trails(
    mut commands: Commands,
    fighters: Query<(Entity, &Position, &crate::builds::AbilityState), With<Fighter>>,
    trails: Query<&DashTrailVisual>,
) {
    let existing: HashSet<_> = trails.iter().map(|trail| trail.target).collect();
    for (entity, position, ability) in &fighters {
        if matches!(ability.phase, crate::builds::AbilityPhase::Dashing { .. })
            && !existing.contains(&entity)
        {
            commands.spawn((
                DashTrailVisual {
                    target: entity,
                    last_position: position.0,
                },
                Sprite::from_color(Color::srgba(0.25, 0.9, 1.0, 0.55), Vec2::ONE),
                Transform::from_translation(position.0.extend(10.0)),
                Name::new("Dash Trail"),
            ));
        }
    }
}

#[cfg(feature = "client")]
fn sync_dash_trails(
    mut commands: Commands,
    fighters: Query<(&Position, &crate::builds::AbilityState), With<Fighter>>,
    mut trails: Query<(Entity, &mut DashTrailVisual, &mut Transform, &mut Sprite)>,
) {
    for (entity, mut trail, mut transform, mut sprite) in &mut trails {
        let Ok((position, ability)) = fighters.get(trail.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        if !matches!(ability.phase, crate::builds::AbilityPhase::Dashing { .. }) {
            commands.entity(entity).despawn();
            continue;
        }
        let delta = position.0 - trail.last_position;
        if delta.length_squared() > f32::EPSILON {
            transform.translation = trail.last_position.midpoint(position.0).extend(10.0);
            transform.rotation = Quat::from_rotation_z(delta.y.atan2(delta.x));
            sprite.custom_size = Some(Vec2::new(delta.length().max(2.0), 14.0));
            trail.last_position = position.0;
        }
    }
}

#[cfg(feature = "client")]
fn ensure_sentry_visuals(
    mut commands: Commands,
    sentries: Query<
        (
            Entity,
            &crate::abilities::SentryIdentity,
            &Position,
            &Rotation,
            Option<&Transform>,
        ),
        With<crate::abilities::Sentry>,
    >,
) {
    for (entity, identity, position, rotation, transform) in &sentries {
        if transform.is_none() {
            let color = if identity.team_id.0 == 0 {
                Color::srgb(0.2, 0.75, 1.0)
            } else {
                Color::srgb(1.0, 0.35, 0.15)
            };
            commands.entity(entity).insert((
                Transform {
                    translation: position.0.extend(12.0),
                    rotation: Quat::from_rotation_z(rotation.as_radians()),
                    ..default()
                },
                Sprite::from_color(color, Vec2::splat(38.0)),
                Name::new("Sentry"),
            ));
        }
    }
}

#[cfg(feature = "client")]
fn sync_sentry_visuals(
    mut sentries: Query<(&Position, &Rotation, &mut Transform), With<crate::abilities::Sentry>>,
) {
    for (position, rotation, mut transform) in &mut sentries {
        transform.translation = position.0.extend(12.0);
        transform.rotation = Quat::from_rotation_z(rotation.as_radians());
    }
}

/// Coordinates the headless client lifecycle with its process-level combat evidence contract.
#[cfg(feature = "client")]
#[derive(Resource, Debug)]
pub struct ClientCombatEvidenceStatus {
    required: bool,
    pub(super) ready: bool,
}

#[cfg(feature = "client")]
impl ClientCombatEvidenceStatus {
    #[must_use]
    pub fn permits_exit(&self) -> bool {
        !self.required || self.ready
    }
}

#[cfg(feature = "client")]
impl FromWorld for ClientCombatEvidenceStatus {
    fn from_world(_: &mut World) -> Self {
        Self {
            required: env::var_os("BRAWLER_NETWORK_COMBAT_CLIENT_READY_FILE").is_some(),
            ready: false,
        }
    }
}

#[cfg(feature = "client")]
#[derive(Resource, Default, Debug)]
struct RecentCombatEvents {
    ids: VecDeque<CombatEventId>,
}

/// Lets deterministic network tests consume the wire cue stream themselves instead of having
/// the presentation system drain it first.
#[cfg(feature = "client")]
#[derive(Resource, Debug, Default)]
pub struct CaptureCombatCues {
    pub cues: Vec<CombatCue>,
    pub dropped_cues: u64,
}

#[cfg(feature = "client")]
fn remember_combat_event(recent: &mut RecentCombatEvents, event_id: CombatEventId) -> bool {
    if recent.ids.contains(&event_id) {
        return false;
    }
    recent.ids.push_back(event_id);
    if recent.ids.len() > 256 {
        recent.ids.pop_front();
    }
    true
}

#[cfg(feature = "client")]
#[derive(Resource, Debug)]
pub(super) struct ClientCombatObservation {
    pub(super) saw_defeat: bool,
    pub(super) saw_reset: bool,
    pub(super) cue_timestamps: Vec<(ShotId, u128)>,
    pub(super) cue_stream: Vec<CombatCue>,
    pub(super) dropped_cue_timestamps: u64,
    pub(super) dropped_cue_stream: u64,
    pub(super) checkpoints: BTreeMap<String, CombatStateSnapshot>,
    pub(super) checkpoint_matches: BTreeMap<String, Vec<CombatStateSnapshot>>,
    pub(super) expected_checkpoints: Vec<CombatEvidenceCheckpoint>,
    pub(super) snapshot_history: BTreeMap<u64, CombatStateSnapshot>,
    pub(super) checkpoint_timestamps: BTreeMap<String, u128>,
    pub(super) state_mutation_timestamps: Vec<(u64, u128)>,
    pub(super) last_encoded_snapshot: Option<String>,
    pub(super) ready_file: Option<PathBuf>,
    pub(super) started_at: Instant,
    pub(super) wrote_ready: bool,
    pub(super) waiting_reported_at_tick: Option<u32>,
}

#[cfg(feature = "client")]
impl FromWorld for ClientCombatObservation {
    fn from_world(_: &mut World) -> Self {
        let ready_file = env::var_os("BRAWLER_NETWORK_COMBAT_CLIENT_READY_FILE").map(PathBuf::from);
        Self {
            saw_defeat: false,
            saw_reset: false,
            cue_timestamps: Vec::new(),
            cue_stream: Vec::new(),
            dropped_cue_timestamps: 0,
            dropped_cue_stream: 0,
            checkpoints: BTreeMap::new(),
            checkpoint_matches: BTreeMap::new(),
            expected_checkpoints: Vec::new(),
            snapshot_history: BTreeMap::new(),
            checkpoint_timestamps: BTreeMap::new(),
            state_mutation_timestamps: Vec::new(),
            last_encoded_snapshot: None,
            ready_file,
            started_at: Instant::now(),
            wrote_ready: false,
            waiting_reported_at_tick: None,
        }
    }
}

#[cfg(feature = "client")]
#[derive(Component)]
struct CombatEffect {
    timer: Timer,
}

#[cfg(feature = "client")]
#[derive(Component)]
struct CombatHealthBar {
    target: Entity,
    fill: bool,
}

#[cfg(feature = "client")]
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct CombatStatusMarker {
    target: Entity,
    kind: CombatStatusKind,
}

#[cfg(feature = "client")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum CombatStatusKind {
    Slow,
    Knockback,
}

#[cfg(feature = "client")]
#[derive(Component)]
pub struct CombatHudText;

#[cfg(feature = "client")]
#[derive(Component)]
pub struct BuildSelectionText;

#[cfg(feature = "client")]
fn combat_cue_profile_id(cue: &CombatCue) -> u16 {
    match cue {
        CombatCue::AttackAccepted {
            presentation_profile_id,
            ..
        }
        | CombatCue::DeliveryImpact {
            presentation_profile_id,
            ..
        }
        | CombatCue::LobLanded {
            presentation_profile_id,
            ..
        }
        | CombatCue::MeleeContact {
            presentation_profile_id,
            ..
        }
        | CombatCue::DamageApplied {
            presentation_profile_id,
            ..
        }
        | CombatCue::EffectApplied {
            presentation_profile_id,
            ..
        }
        | CombatCue::SentryFired {
            presentation_profile_id,
            ..
        } => presentation_profile_id.0,
        CombatCue::FighterDefeated {
            presentation_profile_id,
            ..
        } => presentation_profile_id.map_or(1, |profile| profile.0),
        _ => 1,
    }
}

#[cfg(feature = "client")]
fn combat_profile_color(profile_id: u16, fallback: Color) -> Color {
    match profile_id {
        2 => Color::srgb(1.0, 0.45, 0.12),
        3 => Color::srgb(0.25, 0.7, 1.0),
        4 => Color::srgb(0.85, 0.25, 1.0),
        _ => fallback,
    }
}

#[cfg(feature = "client")]
fn combat_profile_size(profile_id: u16, fallback: Vec2) -> Vec2 {
    match profile_id {
        2 => fallback * 0.8,
        3 => fallback * 1.25,
        4 => fallback * 1.1,
        _ => fallback,
    }
}

#[cfg(feature = "client")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn receive_combat_cues(
    mut commands: Commands,
    mut recent: ResMut<RecentCombatEvents>,
    mut observation: ResMut<ClientCombatObservation>,
    mut capture: Option<ResMut<CaptureCombatCues>>,
    mut presented_cues: MessageWriter<DeduplicatedCombatCue>,
    mut receivers: Query<
        Option<&mut lightyear::prelude::MessageReceiver<CombatCue>>,
        With<lightyear::prelude::client::Client>,
    >,
    local_fighter: Query<&PlayerId, (With<Fighter>, With<lightyear::prelude::Controlled>)>,
) {
    let local_player = local_fighter.iter().next().copied();
    for receiver in &mut receivers {
        let Some(mut receiver) = receiver else {
            continue;
        };
        let cues: Vec<_> = receiver.receive().collect();
        for cue in cues {
            match &cue {
                CombatCue::Defeat { .. } | CombatCue::FighterDefeated { .. } => {
                    observation.saw_defeat = true;
                }
                CombatCue::Reset { .. } | CombatCue::FighterReset { .. } => {
                    observation.saw_reset = true;
                }
                _ => {}
            }
            let event_id = match &cue {
                CombatCue::AttackAccepted { event_id, .. }
                | CombatCue::DeliveryImpact { event_id, .. }
                | CombatCue::LobLanded { event_id, .. }
                | CombatCue::MeleeContact { event_id, .. }
                | CombatCue::DamageApplied { event_id, .. }
                | CombatCue::EffectApplied { event_id, .. }
                | CombatCue::FighterDefeated { event_id, .. }
                | CombatCue::FighterReset { event_id, .. }
                | CombatCue::SentryFired { event_id, .. }
                | CombatCue::DeployableRemoved { event_id, .. }
                | CombatCue::Muzzle { event_id, .. }
                | CombatCue::Impact { event_id, .. }
                | CombatCue::Damage { event_id, .. }
                | CombatCue::Defeat { event_id, .. }
                | CombatCue::Reset { event_id, .. } => *event_id,
            };
            if !remember_combat_event(&mut recent, event_id) {
                continue;
            }
            let profile_id = combat_cue_profile_id(&cue);
            if let Some(capture) = capture.as_mut() {
                if capture.cues.len() < MAX_COMBAT_EVIDENCE_EVENTS {
                    capture.cues.push(cue.clone());
                } else {
                    capture.dropped_cues = capture.dropped_cues.saturating_add(1);
                }
            }
            if observation.ready_file.is_some() {
                if observation.cue_stream.len() < MAX_COMBAT_EVIDENCE_EVENTS {
                    observation.cue_stream.push(cue.clone());
                } else {
                    observation.dropped_cue_stream =
                        observation.dropped_cue_stream.saturating_add(1);
                }
                let timestamp = match &cue {
                    CombatCue::Muzzle { shot_id, .. } => Some(*shot_id),
                    CombatCue::AttackAccepted { attack_id, .. } => Some(ShotId(attack_id.0)),
                    _ => None,
                };
                if let Some(shot_id) = timestamp {
                    if observation.cue_timestamps.len() < MAX_COMBAT_EVIDENCE_EVENTS {
                        observation
                            .cue_timestamps
                            .push((shot_id, unix_epoch_micros()));
                    } else {
                        observation.dropped_cue_timestamps =
                            observation.dropped_cue_timestamps.saturating_add(1);
                    }
                }
            }
            if matches!(
                &cue,
                CombatCue::Muzzle { .. }
                    | CombatCue::Impact { .. }
                    | CombatCue::Damage { .. }
                    | CombatCue::Defeat { .. }
                    | CombatCue::Reset { .. }
            ) {
                continue;
            }
            presented_cues.write(DeduplicatedCombatCue(cue.clone()));
            let local_hit = match &cue {
                CombatCue::Damage {
                    source: DamageSource::PlayerWeapon { player_id, .. },
                    ..
                }
                | CombatCue::DamageApplied {
                    source: DamageSource::PlayerWeapon { player_id, .. },
                    ..
                }
                | CombatCue::DamageApplied {
                    source:
                        DamageSource::Ultimate { player_id, .. }
                        | DamageSource::Deployable { player_id, .. },
                    ..
                } => local_player == Some(*player_id),
                _ => false,
            };
            let (position, color, size) = match cue {
                CombatCue::AttackAccepted { position, .. } => (
                    position.as_vec2(),
                    combat_profile_color(profile_id, Color::srgb(1.0, 0.8, 0.2)),
                    combat_profile_size(profile_id, Vec2::splat(16.0)),
                ),
                CombatCue::DeliveryImpact { position, .. }
                | CombatCue::LobLanded { position, .. }
                | CombatCue::MeleeContact { position, .. }
                | CombatCue::Impact { position, .. } => (
                    position.as_vec2(),
                    combat_profile_color(profile_id, Color::srgb(1.0, 0.35, 0.1)),
                    combat_profile_size(profile_id, Vec2::splat(28.0)),
                ),
                CombatCue::DamageApplied { position, .. } => (
                    position.as_vec2(),
                    combat_profile_color(
                        profile_id,
                        if local_hit {
                            Color::srgb(1.0, 0.9, 0.2)
                        } else {
                            Color::srgb(1.0, 0.1, 0.1)
                        },
                    ),
                    combat_profile_size(profile_id, Vec2::splat(18.0)),
                ),
                CombatCue::EffectApplied { position, .. } => (
                    position.as_vec2(),
                    combat_profile_color(profile_id, Color::srgb(0.3, 0.8, 1.0)),
                    combat_profile_size(profile_id, Vec2::splat(24.0)),
                ),
                CombatCue::FighterDefeated { position, .. } => (
                    position.as_vec2(),
                    combat_profile_color(profile_id, Color::srgb(0.9, 0.05, 0.05)),
                    combat_profile_size(profile_id, Vec2::splat(64.0)),
                ),
                CombatCue::FighterReset { position, .. } | CombatCue::Reset { position, .. } => (
                    position.as_vec2(),
                    Color::srgb(0.2, 1.0, 0.4),
                    Vec2::splat(42.0),
                ),
                CombatCue::Muzzle { position, .. } => (
                    position.as_vec2(),
                    Color::srgb(1.0, 0.8, 0.2),
                    Vec2::splat(22.0),
                ),
                CombatCue::SentryFired { position, .. } => (
                    position.as_vec2(),
                    Color::srgb(0.25, 0.8, 1.0),
                    Vec2::new(20.0, 10.0),
                ),
                CombatCue::DeployableRemoved {
                    position, reason, ..
                } => (
                    position.as_vec2(),
                    if matches!(reason, crate::abilities::SentryCleanupReason::Destroyed) {
                        Color::srgb(1.0, 0.25, 0.1)
                    } else {
                        Color::srgb(0.35, 0.75, 1.0)
                    },
                    Vec2::splat(46.0),
                ),
                CombatCue::Damage { .. } | CombatCue::Defeat { .. } => {
                    continue;
                }
            };
            commands.spawn((
                CombatEffect {
                    timer: Timer::from_seconds(0.18, TimerMode::Once),
                },
                Sprite::from_color(color, size),
                Transform::from_translation(position.extend(30.0)),
            ));
        }
    }
}

#[cfg(feature = "client")]
fn ensure_projectile_visuals(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &Position,
            &Rotation,
            Option<&Transform>,
            Option<&mut Sprite>,
            &ProjectileSource,
            &ReplicatedAttackSource,
            Option<&LobbedFlight>,
        ),
        With<Projectile>,
    >,
) {
    for (entity, position, rotation, transform, sprite, source, replicated_attack, lobbed) in
        &mut query
    {
        if transform.is_none() {
            commands.entity(entity).insert(Transform {
                translation: position.0.extend(20.0),
                rotation: Quat::from_rotation_z(rotation.as_radians()),
                ..default()
            });
        }
        let color = projectile_color(source.player_id);
        let profile_id = replicated_attack.attack.presentation_profile_id.0;
        let size = match profile_id {
            2 => Vec2::new(9.0, 5.0),
            3 => Vec2::new(16.0, 16.0),
            4 => Vec2::new(24.0, 6.0),
            _ => Vec2::new(20.0, 8.0),
        };
        if let Some(mut sprite) = sprite {
            sprite.color = color;
            sprite.custom_size = Some(size);
        } else {
            commands.entity(entity).insert((
                Sprite::from_color(color, size),
                Name::new(if lobbed.is_some() {
                    "Arc projectile"
                } else {
                    "Weapon delivery"
                }),
            ));
        }
    }
}

#[cfg(feature = "client")]
fn sync_projectile_visuals(
    tick: Query<&AuthoritativeTick>,
    mut query: Query<
        (&Position, &Rotation, &mut Transform, Option<&LobbedFlight>),
        With<Projectile>,
    >,
) {
    let current_tick = tick.iter().next().map_or(0, |tick| tick.0);
    for (position, rotation, mut transform, lobbed) in &mut query {
        transform.translation.x = position.0.x;
        transform.translation.y = position.0.y;
        if let Some(lobbed) = lobbed {
            let progress = (current_tick.saturating_sub(lobbed.launched_at_tick) as f32)
                / (lobbed
                    .lands_at_tick
                    .saturating_sub(lobbed.launched_at_tick)
                    .max(1) as f32);
            transform.translation.z =
                20.0 + delivery::lob_height(progress, lobbed.visual_arc_height);
            transform.rotation = Quat::IDENTITY;
        } else {
            transform.translation.z = 20.0;
            transform.rotation = Quat::from_rotation_z(rotation.as_radians());
        }
    }
}

#[cfg(feature = "client")]
#[derive(Component)]
struct WeaponPreviewVisual {
    slot: u8,
}

#[cfg(feature = "client")]
fn update_weapon_preview(
    mut commands: Commands,
    maps: Query<&crate::map::ResolvedMapSnapshot, With<crate::map::MapRoot>>,
    pending: Res<crate::client::PendingLocalActions>,
    convergence: Option<Res<crate::terrain::ClientTerrainConvergence>>,
    fighters: Query<
        (&Position, &Rotation, Option<&ResolvedWeapon>),
        (With<Fighter>, With<lightyear::prelude::Controlled>),
    >,
    mut visuals: Query<(
        &WeaponPreviewVisual,
        &mut Transform,
        &mut Sprite,
        &mut Visibility,
    )>,
) {
    let Some(map) = maps.iter().max_by_key(|map| map.identity.instance_id) else {
        for (_, _, _, mut visibility) in &mut visuals {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let Some((position, rotation, resolved)) = fighters.iter().next() else {
        for (_, _, _, mut visibility) in &mut visuals {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let Some(resolved) = resolved else {
        for (_, _, _, mut visibility) in &mut visuals {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let origin = position.0;
    let facing = rotation.as_radians();
    // The preview repairs against the committed destructible occupancy exactly like the
    // server's collider clearance, so the marker never promises a landing the
    // authoritative resolution will pull back to a face.
    let no_terrain = BTreeMap::new();
    let terrain_chunks = convergence
        .as_deref()
        .map_or(&no_terrain, |convergence| convergence.chunks());
    let segments = preview_segments(
        origin,
        facing,
        pending.aim_distance,
        resolved,
        map,
        terrain_chunks,
    );
    for (visual, mut transform, mut sprite, mut visibility) in &mut visuals {
        let Some((center, angle, size, color)) = segments.get(usize::from(visual.slot)) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Inherited;
        transform.translation = center.extend(11.0);
        transform.rotation = Quat::from_rotation_z(*angle);
        sprite.color = *color;
        sprite.custom_size = Some(*size);
    }
    let existing_slots: HashSet<_> = visuals
        .iter()
        .map(|(visual, _, _, _)| visual.slot)
        .collect();
    for slot in 0..MAX_PREVIEW_SEGMENTS as u8 {
        if !existing_slots.contains(&slot) {
            commands.spawn((
                WeaponPreviewVisual { slot },
                Sprite::from_color(Color::srgba(1.0, 1.0, 1.0, 0.0), Vec2::splat(1.0)),
                Transform::default(),
                Visibility::Hidden,
            ));
        }
    }
}

#[cfg(feature = "client")]
fn update_health_bars(
    mut commands: Commands,
    fighters: Query<
        (
            Entity,
            &Position,
            &CurrentHealth,
            &FighterDefinitionId,
            Option<&Defeated>,
            Option<&crate::builds::ResolvedMatchLoadout>,
        ),
        With<Fighter>,
    >,
    definitions: Res<FighterDefinitions>,
    mut bars: Query<(Entity, &CombatHealthBar, &mut Transform, &mut Sprite)>,
) {
    let fighter_data: HashMap<_, _> = fighters
        .iter()
        .map(
            |(entity, position, health, definition_id, defeated, loadout)| {
                let maximum = loadout.map_or_else(
                    || {
                        definitions
                            .get(*definition_id)
                            .map_or(0, |definition| definition.maximum_health)
                    },
                    |loadout| loadout.fighter_stats.maximum_health,
                );
                (entity, (position.0, health.0, maximum, defeated.is_some()))
            },
        )
        .collect();
    let existing: HashSet<_> = bars
        .iter()
        .map(|(_, bar, _, _)| (bar.target, bar.fill))
        .collect();
    for entity in fighter_data.keys().copied() {
        if !existing.contains(&(entity, false)) {
            commands.spawn((
                CombatHealthBar {
                    target: entity,
                    fill: false,
                },
                Sprite::from_color(Color::srgb(0.04, 0.05, 0.07), Vec2::new(56.0, 7.0)),
                Transform::from_xyz(0.0, 0.0, 35.0),
            ));
        }
        if !existing.contains(&(entity, true)) {
            commands.spawn((
                CombatHealthBar {
                    target: entity,
                    fill: true,
                },
                Sprite::from_color(Color::srgb(0.2, 0.95, 0.35), Vec2::new(52.0, 5.0)),
                Transform::from_xyz(0.0, 0.0, 36.0),
            ));
        }
    }
    for (bar_entity, bar, mut transform, mut sprite) in &mut bars {
        let Some((position, health, maximum, defeated)) = fighter_data.get(&bar.target) else {
            commands.entity(bar_entity).despawn();
            continue;
        };
        let ratio = f32::from(*health) / f32::from((*maximum).max(1));
        transform.translation.x = position.x;
        transform.translation.y = position.y + 34.0;
        if bar.fill {
            transform.translation.x -= 26.0 * (1.0 - ratio);
            transform.scale.x = ratio;
            sprite.color = if *defeated {
                Color::srgb(0.75, 0.08, 0.08)
            } else {
                Color::srgb(0.2, 0.95, 0.35)
            };
        } else {
            transform.scale.x = 1.0;
        }
    }
}

#[cfg(feature = "client")]
#[allow(clippy::too_many_lines)]
fn update_combat_hud(
    mut text: Query<&mut Text, With<CombatHudText>>,
    fighter: Query<
        (
            &PlayerId,
            &CurrentHealth,
            &WeaponState,
            Option<&AuthoritativeTick>,
            Option<&SelectedBuild>,
            Option<&ResolvedWeapon>,
            Option<&ActiveEffects>,
            Option<&Defeated>,
            Option<&crate::builds::ResolvedMatchLoadout>,
            Option<&crate::builds::AbilityState>,
            Option<&crate::builds::PassiveRuntimeState>,
        ),
        (With<Fighter>, With<lightyear::prelude::Controlled>),
    >,
    weapons: Res<WeaponDefinitions>,
    catalog: Option<Res<WeaponCatalogResource>>,
    build_catalog: Option<Res<crate::builds::BuildCatalogResource>>,
    sentries: Query<
        (
            &crate::abilities::SentryIdentity,
            &CurrentHealth,
            &crate::abilities::SentryDeadline,
        ),
        With<crate::abilities::Sentry>,
    >,
) {
    let Some((
        player_id,
        health,
        state,
        authoritative_tick,
        build,
        resolved,
        active_effects,
        defeated,
        loadout,
        ability,
        passive_state,
    )) = fighter.iter().next()
    else {
        return;
    };
    let weapon_id = build.map_or(PULSE_SIDEARM_DEFINITION, |build| build.primary_weapon);
    let capacity = resolved.map_or_else(
        || {
            weapons
                .get(weapon_id)
                .map_or(0, |weapon| weapon.magazine_capacity)
        },
        |resolved| resolved.recipe.economy.capacity(),
    );
    let weapon_name = resolved
        .and_then(|resolved| resolved.source_preset_id)
        .and_then(|id| catalog.as_ref().and_then(|catalog| catalog.0.preset(id)))
        .map_or_else(
            || {
                if weapon_id == PULSE_SIDEARM_DEFINITION {
                    "Pulse"
                } else {
                    "Weapon"
                }
            },
            |preset| preset.display_name.as_str(),
        );
    let phase = match state.phase {
        WeaponPhase::Ready => "READY".to_string(),
        WeaponPhase::Cooldown { ready_at_tick } | WeaponPhase::Reloading { ready_at_tick }
            if authoritative_tick.is_some() =>
        {
            let label = if matches!(state.phase, WeaponPhase::Cooldown { .. }) {
                "COOLDOWN"
            } else {
                "RELOADING"
            };
            format!(
                "{label} {}t",
                ready_at_tick.saturating_sub(authoritative_tick.expect("checked above").0)
            )
        }
        WeaponPhase::Cooldown { .. } | WeaponPhase::Reloading { .. } => "SYNCING".to_string(),
    };
    let phase = defeated.map_or(phase, |_| "DEFEATED".to_string());
    let maximum_health = loadout.map_or(100, |loadout| loadout.fighter_stats.maximum_health);
    let build_name = loadout
        .and_then(|loadout| loadout.identity.source_build_preset_id)
        .and_then(|id| {
            build_catalog
                .as_ref()
                .and_then(|catalog| catalog.0.preset(id))
        })
        .map_or("Custom", |preset| preset.display_name.as_str());
    let ultimate = ability.map_or_else(
        || "ULT --".to_string(),
        |ability| {
            let phase = match ability.phase {
                crate::builds::AbilityPhase::Charging => "charging",
                crate::builds::AbilityPhase::Ready => "READY",
                crate::builds::AbilityPhase::Dashing { .. } => "DASHING",
                crate::builds::AbilityPhase::Deployed { .. } => "DEPLOYED",
            };
            format!("ULT {:>3}% {phase}", ability.charge / 10)
        },
    );
    let passive = passive_state.map_or_else(String::new, |state| {
        let adrenaline = state.adrenaline_until_tick.map_or_else(
            || "ready".to_string(),
            |deadline| {
                format!(
                    "{}t",
                    authoritative_tick.map_or(0, |tick| deadline.saturating_sub(tick.0))
                )
            },
        );
        let quick_cycle = if state.quick_cycle_primed {
            "primed"
        } else {
            "idle"
        };
        format!("  ADR {adrenaline} QC {quick_cycle}")
    });
    let sentry = sentries
        .iter()
        .find(|(identity, _, _)| identity.owner_player_id == *player_id)
        .map_or_else(String::new, |(_, health, deadline)| {
            format!(
                "  SENTRY {}hp {}t",
                health.0,
                authoritative_tick
                    .map_or(0, |tick| deadline.expires_at_tick.saturating_sub(tick.0))
            )
        });
    let slow = active_effects
        .and_then(|effects| effects.slow)
        .zip(authoritative_tick)
        .filter(|(slow, tick)| slow.expires_at_tick > tick.0)
        .map_or_else(String::new, |(slow, tick)| {
            format!("  SLOW {}t", slow.expires_at_tick.saturating_sub(tick.0))
        });
    for mut value in &mut text {
        **value = format!(
            "Player {}   {}   Health {:>3}/{:>3}   {} {}/{}   {}{}\n{}{}{}",
            player_id.0,
            build_name,
            health.0,
            maximum_health,
            weapon_name,
            state.ammo,
            capacity,
            phase,
            slow,
            ultimate,
            passive,
            sentry
        );
    }
}

#[cfg(feature = "client")]
fn update_durable_effect_markers(
    mut commands: Commands,
    fighters: Query<
        (
            Entity,
            &Position,
            Option<&AuthoritativeTick>,
            Option<&ActiveEffects>,
            Option<&KnockbackFeedback>,
            Option<&Defeated>,
        ),
        With<Fighter>,
    >,
    mut markers: Query<(Entity, &CombatStatusMarker, &mut Transform, &mut Sprite)>,
) {
    let desired: HashMap<_, _> = fighters
        .iter()
        .flat_map(
            |(entity, position, authoritative_tick, active_effects, knockback, defeated)| {
                if defeated.is_some() {
                    return Vec::new();
                }
                let mut markers = Vec::with_capacity(2);
                if active_effects.is_some_and(|effects| {
                    effects.slow.is_some_and(|slow| {
                        authoritative_tick.is_none_or(|tick| tick.0 < slow.expires_at_tick)
                    })
                }) {
                    markers.push((
                        CombatStatusMarker {
                            target: entity,
                            kind: CombatStatusKind::Slow,
                        },
                        (position.0, Color::srgba(0.25, 0.75, 1.0, 0.85)),
                    ));
                }
                if knockback.is_some() {
                    markers.push((
                        CombatStatusMarker {
                            target: entity,
                            kind: CombatStatusKind::Knockback,
                        },
                        (position.0, Color::srgba(1.0, 0.55, 0.18, 0.85)),
                    ));
                }
                markers
            },
        )
        .collect();
    let mut existing = HashSet::new();
    for (marker_entity, marker, mut transform, mut sprite) in &mut markers {
        if let Some((position, color)) = desired.get(marker) {
            existing.insert(*marker);
            transform.translation = position.extend(39.0);
            sprite.color = *color;
        } else {
            commands.entity(marker_entity).despawn();
        }
    }
    for (marker, (position, color)) in desired {
        if !existing.contains(&marker) {
            commands.spawn((
                marker,
                Sprite::from_color(color, Vec2::splat(13.0)),
                Transform::from_translation(position.extend(39.0)),
            ));
        }
    }
}

#[cfg(feature = "client")]
fn update_combat_effects(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut effects: Query<(Entity, &mut CombatEffect)>,
) {
    for (entity, mut effect) in &mut effects {
        effect.timer.tick(time.delta());
        if effect.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(all(test, feature = "client"))]
mod tests {
    use super::*;
    use crate::combat::{FighterDefinitions, WeaponCatalog, WeaponPresetId};
    use crate::map::{
        MapContentCatalog, MapInstanceId, MapLayoutRequirements, MapPresetId as ArenaPresetId,
    };
    use crate::timing::SimulationTick;
    use core::time::Duration;

    fn preview_for(id: u16) -> Vec<(Vec2, f32, Vec2, Color)> {
        let catalog = WeaponCatalog::embedded().unwrap();
        let fighter = FighterDefinitions::default().entries[0];
        let resolved = catalog
            .resolve_preset(WeaponPresetId(id), &fighter)
            .unwrap();
        let map_catalog = MapContentCatalog::embedded().unwrap();
        let map = map_catalog
            .resolve_preset(
                ArenaPresetId(1),
                MapInstanceId(1),
                &MapLayoutRequirements::wipeout(),
            )
            .unwrap();
        preview_segments(
            Vec2::ZERO,
            0.0,
            None,
            &resolved,
            &map.snapshot,
            &BTreeMap::new(),
        )
    }

    #[test]
    fn preview_geometry_is_bounded_and_finite_for_all_presets() {
        for id in 1..=4 {
            let segments = preview_for(id);
            assert!(segments.len() <= MAX_PREVIEW_SEGMENTS);
            assert!(segments.iter().all(|(center, angle, size, _)| {
                center.is_finite()
                    && angle.is_finite()
                    && size.is_finite()
                    && size.x > 0.0
                    && size.y > 0.0
            }));
        }
        assert_eq!(preview_for(1).len(), 2);
        assert_eq!(preview_for(2).len(), 8);
        assert_eq!(preview_for(3).len(), 14);
        assert_eq!(preview_for(4).len(), 10);
    }

    #[test]
    fn launcher_preview_uses_the_requested_focal_distance() {
        let catalog = WeaponCatalog::embedded().unwrap();
        let fighter = FighterDefinitions::default().entries[0];
        let resolved = catalog.resolve_preset(WeaponPresetId(3), &fighter).unwrap();
        let map_catalog = MapContentCatalog::embedded().unwrap();
        let map = map_catalog
            .resolve_preset(
                ArenaPresetId(1),
                MapInstanceId(1),
                &MapLayoutRequirements::wipeout(),
            )
            .unwrap();
        let segments = preview_segments(
            Vec2::ZERO,
            0.0,
            Some(180.0),
            &resolved,
            &map.snapshot,
            &BTreeMap::new(),
        );

        assert!((segments[0].2.x - 180.0).abs() < 0.001);
    }

    #[test]
    fn launcher_preview_repairs_landings_against_committed_terrain() {
        let catalog = WeaponCatalog::embedded().unwrap();
        let fighter = FighterDefinitions::default().entries[0];
        let resolved = catalog.resolve_preset(WeaponPresetId(3), &fighter).unwrap();
        let map_catalog = MapContentCatalog::embedded().unwrap();
        let map = map_catalog
            .resolve_preset(
                ArenaPresetId(1),
                MapInstanceId(1),
                &MapLayoutRequirements::wipeout(),
            )
            .unwrap();
        // Occupied destructible cells covering world x [288, 328) around the aim axis:
        // the marker must repair exactly like the server's collider clearance instead of
        // promising a landing inside terrain.
        let mut chunks: BTreeMap<crate::terrain::TerrainChunkId, crate::terrain::TerrainBits> =
            BTreeMap::new();
        for cell_y in -3..3 {
            for cell_x in 36..41 {
                let Some((chunk, (local_x, local_y))) =
                    crate::terrain::grid::cell_to_chunk_and_local((cell_x, cell_y))
                else {
                    continue;
                };
                chunks.entry(chunk).or_default().set(local_x, local_y);
            }
        }
        let empty = preview_segments(
            Vec2::ZERO,
            0.0,
            Some(300.0),
            &resolved,
            &map.snapshot,
            &BTreeMap::new(),
        );
        assert!((empty[1].0.x - 300.0).abs() <= 0.5);
        assert_eq!(empty[1].3, Color::srgba(0.35, 0.85, 1.0, 0.34));

        let repaired = preview_segments(
            Vec2::ZERO,
            0.0,
            Some(300.0),
            &resolved,
            &map.snapshot,
            &chunks,
        );
        assert!(
            repaired[1].0.x < 286.0 && repaired[1].0.x > 260.0,
            "the marker pulls back out of the occupied cells: {}",
            repaired[1].0.x
        );
        assert_eq!(repaired[1].3, Color::srgba(0.95, 0.35, 1.0, 0.45));
    }

    #[test]
    fn combat_cue_event_ids_are_deduplicated_with_a_bounded_history() {
        let mut recent = RecentCombatEvents::default();
        assert!(remember_combat_event(&mut recent, CombatEventId(1)));
        assert!(!remember_combat_event(&mut recent, CombatEventId(1)));
        for event_id in 2..=257 {
            assert!(remember_combat_event(&mut recent, CombatEventId(event_id)));
        }
        assert_eq!(recent.ids.len(), 256);
        assert!(!recent.ids.contains(&CombatEventId(1)));
        assert!(remember_combat_event(&mut recent, CombatEventId(1)));
    }

    #[test]
    fn headless_exit_waits_for_required_combat_evidence() {
        let mut status = ClientCombatEvidenceStatus {
            required: true,
            ready: false,
        };
        assert!(!status.permits_exit());
        status.ready = true;
        assert!(status.permits_exit());
        assert!(
            ClientCombatEvidenceStatus {
                required: false,
                ready: false,
            }
            .permits_exit()
        );
    }

    #[test]
    fn combat_effects_expire_after_the_bounded_presentation_lifetime() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_millis(100),
            ))
            .add_systems(Update, update_combat_effects);
        let effect = app
            .world_mut()
            .spawn(CombatEffect {
                timer: Timer::from_seconds(0.18, TimerMode::Once),
            })
            .id();

        app.update();
        assert!(app.world().get_entity(effect).is_ok());
        app.update();
        app.update();

        assert!(app.world().get_entity(effect).is_err());
    }

    #[test]
    fn combat_hud_reports_replicated_reload_and_defeat_state() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<WeaponDefinitions>()
            .insert_resource(SimulationTick(999))
            .add_systems(Update, update_combat_hud);
        let hud = app
            .world_mut()
            .spawn((CombatHudText, Text::new("placeholder")))
            .id();
        app.world_mut().spawn((
            Fighter,
            lightyear::prelude::Controlled,
            PlayerId(1),
            CurrentHealth(42),
            AuthoritativeTick(10),
            WeaponState {
                ammo: 0,
                phase: WeaponPhase::Reloading { ready_at_tick: 25 },
            },
        ));

        app.update();
        assert_eq!(
            app.world().get::<Text>(hud).expect("combat HUD").0,
            "Player 1   Custom   Health  42/100   Pulse 0/6   RELOADING 15t\nULT --"
        );

        app.world_mut().entity_mut(hud).insert(Text::new("stale"));
        let fighter = app
            .world_mut()
            .query_filtered::<Entity, With<Fighter>>()
            .single(app.world())
            .expect("controlled fighter");
        app.world_mut().entity_mut(fighter).insert(Defeated {
            event_id: CombatEventId(1),
        });
        app.update();
        assert_eq!(
            app.world().get::<Text>(hud).expect("combat HUD").0,
            "Player 1   Custom   Health  42/100   Pulse 0/6   DEFEATED\nULT --"
        );
    }

    #[test]
    fn fighter_and_projectile_palettes_distinguish_replicated_sources() {
        assert_ne!(fighter_color(PlayerId(1)), fighter_color(PlayerId(2)));
        assert_ne!(projectile_color(PlayerId(1)), projectile_color(PlayerId(2)));
        assert_ne!(fighter_color(PlayerId(1)), Color::srgb(0.95, 0.25, 0.1));
    }

    #[test]
    fn projectile_presentation_keeps_authoritative_position_and_facing() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_systems(Update, sync_projectile_visuals);
        let projectile = app
            .world_mut()
            .spawn((
                Projectile,
                Position::from_xy(120.0, -40.0),
                Rotation::radians(std::f32::consts::FRAC_PI_2),
                Transform::default(),
            ))
            .id();

        app.update();

        let transform = app
            .world()
            .get::<Transform>(projectile)
            .expect("projectile transform");
        assert_eq!(transform.translation.truncate(), Vec2::new(120.0, -40.0));
        assert!(
            (transform.rotation.to_euler(EulerRot::ZYX).0 - std::f32::consts::FRAC_PI_2).abs()
                < 0.001
        );
    }

    #[test]
    fn replicated_delivery_visuals_wait_for_an_authoritative_pose() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_systems(
            Update,
            (ensure_projectile_visuals, ensure_sentry_visuals).chain(),
        );
        let projectile = app.world_mut().spawn(Projectile).id();
        let sentry = app
            .world_mut()
            .spawn((
                crate::abilities::Sentry,
                crate::abilities::SentryIdentity {
                    deployable_id: crate::builds::DeployableId(1),
                    owner_player_id: PlayerId(1),
                    owner_network_id: NetworkEntityId(1),
                    team_id: TeamId(0),
                    ultimate_id: crate::builds::UltimateDefinitionId(2),
                    match_id: crate::matchplay::MatchId(1),
                },
            ))
            .id();

        app.update();
        assert!(app.world().get::<Transform>(projectile).is_none());
        assert!(app.world().get::<Transform>(sentry).is_none());

        app.world_mut()
            .entity_mut(projectile)
            .insert((Position::from_xy(120.0, -40.0), Rotation::radians(0.5)));
        app.world_mut()
            .entity_mut(sentry)
            .insert((Position::from_xy(-90.0, 75.0), Rotation::radians(-0.25)));
        app.update();

        assert!(
            app.world().get::<Transform>(projectile).is_none(),
            "a pose without both replicated source identities must remain hidden"
        );
        app.world_mut().entity_mut(projectile).insert((
            ProjectileSource {
                shot_id: ShotId(9),
                player_id: PlayerId(1),
                owner_network_entity_id: NetworkEntityId(1),
                team_id: TeamId(0),
                weapon_definition_id: WeaponDefinitionId(1),
            },
            ReplicatedAttackSource {
                attack: AttackSource {
                    kind: CombatSourceKind::PrimaryWeapon,
                    attack_id: AttackId(9),
                    player_id: PlayerId(1),
                    owner_network_entity_id: NetworkEntityId(1),
                    team_id: TeamId(0),
                    recipe_fingerprint: WeaponRecipeFingerprint(1),
                    presentation_profile_id: WeaponPresentationProfileId(1),
                    legacy_compatibility: false,
                    source_preset_id: None,
                    origin: WorldPoint { x: 120.0, y: -40.0 },
                    facing: 0.5,
                },
            },
        ));
        app.update();

        assert_eq!(
            app.world()
                .get::<Transform>(projectile)
                .unwrap()
                .translation,
            Vec3::new(120.0, -40.0, 20.0)
        );
        assert_eq!(
            app.world().get::<Transform>(sentry).unwrap().translation,
            Vec3::new(-90.0, 75.0, 12.0)
        );
    }
}
