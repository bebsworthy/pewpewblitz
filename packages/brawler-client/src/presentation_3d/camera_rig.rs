//! Presentation-only gameplay camera rig: smoothed follow, aim look-ahead, and trauma shake.
//!
//! The rig reads the controlled fighter's presented (predicted) pose, the local aim sample, and
//! combat cues, and only ever writes the arena camera transform plus the [`ArenaCameraShake`]
//! offset baked into it. Aim and targeting project through the unshaken view
//! ([`arena_aim_view`]), so shake never moves aim, targeting, input, or gameplay state. Follow
//! and look-ahead always pass through the unchanged per-axis framing clamp; shake is a bounded
//! transient offset applied after it.

use super::camera::clamp_3d_camera_center;
#[allow(
    clippy::wildcard_imports,
    reason = "the world-renderer submodule consumes the presentation_3d composition surface"
)]
use super::*;
use crate::local_attack::{LocalAttackLedger, LocalAttackPresented};
use brawler::combat::client::DeduplicatedCombatCue;
use brawler::map::{AxisAlignedMapRect, MAP_CELL_SIZE_WORLD};
use serde::Deserialize;
use std::collections::VecDeque;

const CAMERA_RIG_CATALOG: &str = include_str!("../../../../assets/catalogs/camera_rig.ron");
const CAMERA_RIG_SCHEMA_VERSION: u16 = 1;
/// Largest distance the rig may lead or trail the fighter before framing. Inside the conservative
/// view's nearest extent (about 4.9 cells below the look target, 7.1 above, 8.8 to each side at
/// the reference aspect), so the fighter always stays visible.
pub(super) const MAX_CAMERA_LEAD_WORLD: f32 = MAP_CELL_SIZE_WORLD * 3.0;
/// Shake stays a small transient offset: at most a quarter cell beyond the framing clamp.
const MAX_SHAKE_OFFSET_WORLD: f32 = MAP_CELL_SIZE_WORLD * 0.25;
const MIN_SMOOTH_SECONDS: f32 = 0.01;
const MAX_SMOOTH_SECONDS: f32 = 1.0;
/// A long hitch is integrated as one bounded step instead of a large spring jump.
const MAX_RIG_STEP_SECONDS: f32 = 0.1;
const MAX_SEEN_WORLD_OBJECT_CUES: usize = 32;
/// Noise phase wraps on a whole multiple of every component period (eighths of a cycle).
const NOISE_PHASE_WRAP: f32 = 1_024.0;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct CameraRigCatalogSource {
    schema_version: u16,
    follow: FollowTuning,
    look_ahead: LookAheadTuning,
    shake: ShakeTuning,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct FollowTuning {
    pub(super) smooth_seconds: f32,
    pub(super) reduced_motion_smooth_seconds: f32,
    pub(super) snap_distance_cells: f32,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct LookAheadTuning {
    pub(super) distance_cells: f32,
    pub(super) smooth_seconds: f32,
    pub(super) full_strength_aim_distance_cells: f32,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct ShakeTuning {
    pub(super) max_offset_world: f32,
    pub(super) frequency_hz: f32,
    pub(super) decay_per_second: f32,
    pub(super) exponent: f32,
    pub(super) triggers: ShakeTriggers,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct ShakeTriggers {
    pub(super) local_fire: f32,
    pub(super) local_damage: f32,
    pub(super) local_defeat: f32,
    pub(super) nearby_explosion: f32,
    pub(super) nearby_defeat: f32,
    pub(super) falloff_radius_cells: f32,
}

/// Validated client-only camera rig tuning, embedded from `assets/catalogs/camera_rig.ron`.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(super) struct CameraRigCatalog {
    pub(super) follow: FollowTuning,
    pub(super) look_ahead: LookAheadTuning,
    pub(super) shake: ShakeTuning,
}

impl CameraRigCatalog {
    pub(super) fn embedded() -> Result<Self, String> {
        Self::from_ron(CAMERA_RIG_CATALOG)
    }

    fn from_ron(source: &str) -> Result<Self, String> {
        let source: CameraRigCatalogSource = ron::from_str(source)
            .map_err(|error| format!("camera rig catalog parse failed: {error}"))?;
        if source.schema_version != CAMERA_RIG_SCHEMA_VERSION {
            return Err(format!(
                "camera rig catalog schema {} is not {CAMERA_RIG_SCHEMA_VERSION}",
                source.schema_version
            ));
        }
        let catalog = Self {
            follow: source.follow,
            look_ahead: source.look_ahead,
            shake: source.shake,
        };
        catalog.validate()?;
        Ok(catalog)
    }

    fn validate(&self) -> Result<(), String> {
        let follow = self.follow;
        let look = self.look_ahead;
        let shake = self.shake;
        let triggers = shake.triggers;
        check_range(
            "follow.smooth_seconds",
            follow.smooth_seconds,
            MIN_SMOOTH_SECONDS,
            MAX_SMOOTH_SECONDS,
        )?;
        check_range(
            "follow.reduced_motion_smooth_seconds",
            follow.reduced_motion_smooth_seconds,
            MIN_SMOOTH_SECONDS,
            follow.smooth_seconds,
        )?;
        check_range(
            "follow.snap_distance_cells",
            follow.snap_distance_cells,
            MAX_CAMERA_LEAD_WORLD / MAP_CELL_SIZE_WORLD,
            64.0,
        )?;
        check_range(
            "look_ahead.distance_cells",
            look.distance_cells,
            0.0,
            MAX_CAMERA_LEAD_WORLD / MAP_CELL_SIZE_WORLD,
        )?;
        check_range(
            "look_ahead.smooth_seconds",
            look.smooth_seconds,
            MIN_SMOOTH_SECONDS,
            MAX_SMOOTH_SECONDS,
        )?;
        check_range(
            "look_ahead.full_strength_aim_distance_cells",
            look.full_strength_aim_distance_cells,
            0.5,
            32.0,
        )?;
        check_range(
            "shake.max_offset_world",
            shake.max_offset_world,
            0.0,
            MAX_SHAKE_OFFSET_WORLD,
        )?;
        check_range("shake.frequency_hz", shake.frequency_hz, 0.5, 60.0)?;
        check_range("shake.decay_per_second", shake.decay_per_second, 0.1, 20.0)?;
        check_range("shake.exponent", shake.exponent, 1.0, 4.0)?;
        for (name, value) in [
            ("shake.triggers.local_fire", triggers.local_fire),
            ("shake.triggers.local_damage", triggers.local_damage),
            ("shake.triggers.local_defeat", triggers.local_defeat),
            ("shake.triggers.nearby_explosion", triggers.nearby_explosion),
            ("shake.triggers.nearby_defeat", triggers.nearby_defeat),
        ] {
            check_range(name, value, 0.0, 1.0)?;
        }
        check_range(
            "shake.triggers.falloff_radius_cells",
            triggers.falloff_radius_cells,
            0.5,
            32.0,
        )
    }
}

fn check_range(name: &str, value: f32, min: f32, max: f32) -> Result<(), String> {
    if value.is_finite() && (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "camera rig catalog {name} = {value} is outside {min}..={max}"
        ))
    }
}

/// Presentation-only shake translation baked into the arena camera transform this frame.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ArenaCameraShake(pub(crate) Vec3);

/// The arena camera as aim and targeting must see it: without presentation-only shake.
pub(crate) type ArenaAimCameras<'w, 's> = Query<
    'w,
    's,
    (
        &'static Camera,
        &'static GlobalTransform,
        Option<&'static ArenaCameraShake>,
    ),
    With<ArenaCamera>,
>;

/// The arena camera and its unshaken world transform. Shake is a pure translation, so removing
/// it restores exactly the view that follow and look-ahead framed.
pub(crate) fn arena_aim_view<'a>(
    cameras: &'a ArenaAimCameras<'_, '_>,
) -> Option<(&'a Camera, GlobalTransform)> {
    let (camera, transform, shake) = cameras.iter().next()?;
    Some((
        camera,
        unshaken_camera_transform(*transform, shake.copied()),
    ))
}

fn unshaken_camera_transform(
    transform: GlobalTransform,
    shake: Option<ArenaCameraShake>,
) -> GlobalTransform {
    shake.map_or(transform, |shake| {
        GlobalTransform::from_translation(-shake.0) * transform
    })
}

/// One critically damped spring (the `SmoothDamp` form): frame-rate independent, never
/// overshooting a stationary target.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Spring {
    value: Vec2,
    velocity: Vec2,
}

impl Spring {
    fn at(value: Vec2) -> Self {
        Self {
            value,
            velocity: Vec2::ZERO,
        }
    }

    fn step(&mut self, target: Vec2, smooth_seconds: f32, dt: f32) -> Vec2 {
        if dt <= 0.0 {
            return self.value;
        }
        let omega = 2.0 / smooth_seconds.max(MIN_SMOOTH_SECONDS);
        let x = omega * dt;
        let decay = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
        let change = self.value - target;
        let temp = (self.velocity + omega * change) * dt;
        let mut next = target + (change + temp) * decay;
        self.velocity = (self.velocity - omega * temp) * decay;
        if (target - self.value).dot(next - target) > 0.0 {
            next = target;
            self.velocity = Vec2::ZERO;
        }
        self.value = next;
        next
    }
}

/// Everything the rig reads for one presented frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CameraRigFrameInput {
    /// Presented pose of the controlled fighter, if one exists.
    pub(super) fighter: Option<Vec2>,
    /// Local aim direction plus optional aim distance (mouse/lob), presentation copy only.
    pub(super) aim: Option<(Vec2, Option<f32>)>,
    pub(super) bounds: AxisAlignedMapRect,
    pub(super) viewport: Vec2,
    pub(super) dt: f32,
    pub(super) reduced_motion: bool,
    pub(super) reduced_effects: bool,
}

/// The framed camera center and the transient shake applied on top of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CameraRigFrame {
    pub(super) center: Vec2,
    pub(super) shake: Vec2,
}

/// Client-local camera rig state. Never replicated and never read by gameplay.
#[derive(Resource, Clone, Debug, Default)]
pub(super) struct CameraRig {
    follow: Option<Spring>,
    look_ahead: Spring,
    bounds: Option<AxisAlignedMapRect>,
    trauma: f32,
    noise_phase: f32,
    seen_world_object_cues: VecDeque<brawler::combat::CombatEventId>,
}

impl CameraRig {
    #[cfg(test)]
    pub(super) fn trauma(&self) -> f32 {
        self.trauma
    }

    pub(super) fn add_trauma(&mut self, amount: f32) {
        if amount.is_finite() && amount > 0.0 {
            self.trauma = (self.trauma + amount).min(1.0);
        }
    }

    /// Forget follow history so the next gameplay frame snaps (map editor, map changes).
    pub(super) fn reset(&mut self) {
        self.follow = None;
        self.look_ahead = Spring::default();
        self.bounds = None;
        self.trauma = 0.0;
    }

    pub(super) fn step(
        &mut self,
        catalog: &CameraRigCatalog,
        input: CameraRigFrameInput,
    ) -> CameraRigFrame {
        let dt = if input.dt.is_finite() {
            input.dt.clamp(0.0, MAX_RIG_STEP_SECONDS)
        } else {
            0.0
        };
        let anchor = input.fighter.unwrap_or_else(|| input.bounds.center());
        let snap_distance = catalog.follow.snap_distance_cells * MAP_CELL_SIZE_WORLD;
        let follow = match self.follow.as_mut() {
            Some(follow)
                if self.bounds == Some(input.bounds)
                    && follow.value.distance(anchor) <= snap_distance =>
            {
                let smooth = if input.reduced_motion {
                    catalog.follow.reduced_motion_smooth_seconds
                } else {
                    catalog.follow.smooth_seconds
                };
                follow.step(anchor, smooth, dt)
            }
            _ => {
                self.follow = Some(Spring::at(anchor));
                self.look_ahead = Spring::default();
                self.bounds = Some(input.bounds);
                anchor
            }
        };
        let look_target = if input.reduced_motion || input.fighter.is_none() {
            Vec2::ZERO
        } else {
            look_ahead_target(&catalog.look_ahead, input.aim)
        };
        let look_ahead = self
            .look_ahead
            .step(look_target, catalog.look_ahead.smooth_seconds, dt);
        let lead = (follow + look_ahead - anchor).clamp_length_max(MAX_CAMERA_LEAD_WORLD);
        let center = clamp_3d_camera_center(anchor + lead, input.bounds, input.viewport);

        self.trauma = (self.trauma - catalog.shake.decay_per_second * dt).max(0.0);
        if input.reduced_motion || input.reduced_effects {
            self.trauma = 0.0;
        }
        self.noise_phase =
            (self.noise_phase + dt * catalog.shake.frequency_hz).rem_euclid(NOISE_PHASE_WRAP);
        let amplitude = catalog.shake.max_offset_world * self.trauma.powf(catalog.shake.exponent);
        let shake = if amplitude > 0.0 {
            shake_noise(self.noise_phase) * amplitude
        } else {
            Vec2::ZERO
        };
        CameraRigFrame { center, shake }
    }
}

fn look_ahead_target(tuning: &LookAheadTuning, aim: Option<(Vec2, Option<f32>)>) -> Vec2 {
    let Some((direction, distance)) = aim else {
        return Vec2::ZERO;
    };
    let Some(direction) = direction.try_normalize() else {
        return Vec2::ZERO;
    };
    let full = tuning.full_strength_aim_distance_cells * MAP_CELL_SIZE_WORLD;
    let strength = distance.map_or(1.0, |distance| {
        if distance.is_finite() {
            (distance / full).clamp(0.0, 1.0)
        } else {
            0.0
        }
    });
    direction * tuning.distance_cells * MAP_CELL_SIZE_WORLD * strength
}

/// Smooth deterministic noise in roughly [-1, 1] per axis. The rational frequency ratios share a
/// period of eight phase cycles, so wrapping the phase at a multiple of eight stays continuous.
fn shake_noise(phase: f32) -> Vec2 {
    let wave = |ratio: f32, offset: f32| (core::f32::consts::TAU * (phase * ratio + offset)).sin();
    Vec2::new(
        0.5 * wave(1.0, 0.0) + 0.3 * wave(1.375, 0.31) + 0.2 * wave(2.125, 0.67),
        0.5 * wave(1.125, 0.53) + 0.3 * wave(1.625, 0.12) + 0.2 * wave(2.375, 0.84),
    )
}

fn falloff(amount: f32, distance: f32, radius: f32) -> f32 {
    amount * (1.0 - distance / radius).clamp(0.0, 1.0)
}

/// Feed camera trauma from local fire and nearby impactful combat or world-object cues.
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "the system reads every presentation cue stream that feeds one bounded shake channel"
)]
pub(super) fn accumulate_camera_trauma(
    catalog: Res<CameraRigCatalog>,
    mut rig: ResMut<CameraRig>,
    mut local_attacks: MessageReader<LocalAttackPresented>,
    mut cues: MessageReader<DeduplicatedCombatCue>,
    ledger: Option<Res<LocalAttackLedger>>,
    world_objects: Option<Res<brawler::map::ReceivedWorldObjectCues>>,
    controlled: Query<(&NetworkEntityId, &Position), (With<Fighter>, With<Controlled>)>,
) {
    let triggers = catalog.shake.triggers;
    let radius = triggers.falloff_radius_cells * MAP_CELL_SIZE_WORLD;
    let local = controlled
        .iter()
        .next()
        .map(|(id, position)| (*id, position.0));
    let nearby = |amount: f32, position: Vec2| {
        local.map_or(0.0, |(_, origin)| {
            falloff(amount, origin.distance(position), radius)
        })
    };
    let mut trauma = 0.0;
    for _ in local_attacks.read() {
        trauma += triggers.local_fire;
    }
    for DeduplicatedCombatCue(cue) in cues.read() {
        let is_local = |id: &NetworkEntityId| local.is_some_and(|(local, _)| local == *id);
        trauma += match cue {
            brawler::combat::CombatCue::AttackAccepted {
                source, attack_id, ..
            } if is_local(source)
                && !ledger
                    .as_deref()
                    .is_some_and(|ledger| ledger.is_echo(*attack_id)) =>
            {
                triggers.local_fire
            }
            brawler::combat::CombatCue::DamageApplied { target, .. } if is_local(target) => {
                triggers.local_damage
            }
            brawler::combat::CombatCue::FighterDefeated { target, .. } if is_local(target) => {
                triggers.local_defeat
            }
            brawler::combat::CombatCue::FighterDefeated { position, .. } => {
                nearby(triggers.nearby_defeat, position.as_vec2())
            }
            brawler::combat::CombatCue::LobLanded { position, .. } => {
                nearby(triggers.nearby_explosion, position.as_vec2())
            }
            _ => 0.0,
        };
    }
    // The world-object VFX producer drains this inbox after this system each frame; remembering
    // seen ids keeps the channel exact if a cue survives into another frame.
    if let Some(world_objects) = world_objects {
        for cue in &world_objects.0 {
            let brawler::map::WorldObjectCue::Exploded { position, .. } = *cue else {
                continue;
            };
            if rig.seen_world_object_cues.contains(&cue.event_id()) {
                continue;
            }
            if rig.seen_world_object_cues.len() >= MAX_SEEN_WORLD_OBJECT_CUES {
                rig.seen_world_object_cues.pop_front();
            }
            rig.seen_world_object_cues.push_back(cue.event_id());
            trauma += nearby(triggers.nearby_explosion, position.as_vec2());
        }
    }
    rig.add_trauma(trauma);
}

#[cfg(test)]
mod tests;
