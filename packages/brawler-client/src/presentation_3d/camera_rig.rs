//! Presentation-only gameplay camera rig: smoothed follow, aim look-ahead, and trauma shake.
//!
//! The rig reads the controlled fighter's presented (predicted) pose, the local aim sample, and
//! combat cues, and only ever writes the arena camera transform plus the [`ArenaCameraShake`]
//! offset baked into it. Aim and targeting project through the unshaken view
//! ([`arena_aim_view`]), so shake never moves aim, targeting, input, or gameplay state. The aim
//! lean grows with the commanded reach and the view's depth in the aim direction (BRL-vptw), the
//! combined lead keeps the fighter inside a safe frame of the visible ground, and the result
//! always passes through the unchanged per-axis framing clamp. At the map's north edge the clamp
//! may yield just enough of a top HUD band to keep the local overhead below the score plate
//! (BRL-fwtg); shake is a bounded transient offset applied after it.

use super::camera::{
    GroundView, clamp_3d_camera_center, ground_view, top_hud_fighter_limit, viewport_aspect,
};
#[allow(
    clippy::wildcard_imports,
    reason = "the world-renderer submodule consumes the presentation_3d composition surface"
)]
use super::*;
use crate::local_attack::{LocalAttackKind, LocalAttackLedger, LocalAttackPresented};
use brawler::builds::ResolvedMatchLoadout;
use brawler::combat::client::DeduplicatedCombatCue;
use brawler::combat::{DeliveryMethod, FiringPattern, WeaponRecipe};
use brawler::map::{AxisAlignedMapRect, MAP_CELL_SIZE_WORLD};
use serde::Deserialize;
use std::collections::VecDeque;

const CAMERA_RIG_CATALOG: &str = include_str!("../../../../assets/catalogs/camera_rig.ron");
const CAMERA_RIG_SCHEMA_VERSION: u16 = 3;
/// Ceiling for the authored look-ahead lean. The lean actually applied is also limited per
/// direction by the fighter safe frame for the current viewport (BRL-vptw).
const MAX_LOOK_AHEAD_CELLS: f32 = 6.0;
/// The fighter stays at least this far inside every screen edge while the rig leads it.
const MIN_FIGHTER_SAFE_MARGIN_CELLS: f32 = 1.0;
const MAX_FIGHTER_SAFE_MARGIN_CELLS: f32 = 3.0;
const MAX_REACH_MARGIN_CELLS: f32 = 2.0;
/// Mouse aim distance is measured through the leaning view, so the lean feeds back into it. Full
/// strength must sit at least this many times farther than the largest lean, which keeps that
/// loop's gain at or below two thirds: the view settles instead of chasing the cursor.
const MIN_FULL_STRENGTH_TO_LEAN_RATIO: f32 = 1.5;
/// A follow spring further behind than this is a teleport, not a move.
const MIN_SNAP_DISTANCE_CELLS: f32 = 3.0;
/// Shake stays a small transient offset: at most a quarter cell beyond the framing clamp.
const MAX_SHAKE_OFFSET_WORLD: f32 = MAP_CELL_SIZE_WORLD * 0.25;
const MIN_SMOOTH_SECONDS: f32 = 0.01;
const MAX_SMOOTH_SECONDS: f32 = 1.0;
/// A long hitch is integrated as one bounded step instead of a large spring jump.
const MAX_RIG_STEP_SECONDS: f32 = 0.1;
const MAX_SEEN_WORLD_OBJECT_CUES: usize = 32;
/// A spread weapon's fire trauma may at most quadruple its delivery family's value.
const MAX_SPREAD_FIRE_SCALE: f32 = 4.0;
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
    /// Lean of every aim, and the whole lean of a weapon whose reach is already on screen.
    pub(super) distance_cells: f32,
    /// Largest lean, used when the weapon's reach runs past the visible ground.
    pub(super) max_distance_cells: f32,
    /// Ground shown past the weapon's reach when the lean is large enough.
    pub(super) reach_margin_cells: f32,
    /// Smallest distance between the fighter and a screen edge while the rig leads it.
    pub(super) fighter_safe_margin_cells: f32,
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
    pub(super) local_fire: LocalFireTrauma,
    pub(super) local_ultimate: f32,
    pub(super) local_damage: LocalDamageTrauma,
    pub(super) local_defeat: f32,
    pub(super) nearby_explosion: f32,
    pub(super) nearby_defeat: f32,
    pub(super) falloff_radius_cells: f32,
}

/// Local fire trauma by the controlled fighter's primary weapon delivery family (BRL-0173), so
/// heavy weapons shake more than light ones. Player-authored weapons share their family's value.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct LocalFireTrauma {
    pub(super) straight: f32,
    pub(super) sticky_straight: f32,
    pub(super) lobbed: f32,
    pub(super) melee_arc: f32,
    pub(super) cone_spray: f32,
    pub(super) splash: f32,
    /// Multiplier for a spread firing pattern (one blast of several deliveries).
    pub(super) spread_scale: f32,
}

impl LocalFireTrauma {
    /// Trauma of one local shot. An unknown weapon (loadout not yet replicated) uses the
    /// straight-shot value, matching the fire-sound fallback.
    pub(super) fn for_weapon(&self, recipe: Option<&WeaponRecipe>) -> f32 {
        let Some(recipe) = recipe else {
            return self.straight;
        };
        let family = match recipe.delivery {
            DeliveryMethod::Straight { .. } => self.straight,
            DeliveryMethod::StickyStraight { .. } => self.sticky_straight,
            DeliveryMethod::Lobbed { .. } => self.lobbed,
            DeliveryMethod::MeleeArc { .. } => self.melee_arc,
            DeliveryMethod::ConeSpray { .. } => self.cone_spray,
            DeliveryMethod::Splash { .. } => self.splash,
        };
        match recipe.firing {
            FiringPattern::Single => family,
            FiringPattern::Spread { .. } => (family * self.spread_scale).min(1.0),
        }
    }
}

/// Local damage-taken trauma scaled by the hit's share of the fighter's maximum health
/// (BRL-0173): a hit of at least `full_strength_health_fraction` reaches `maximum`.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct LocalDamageTrauma {
    pub(super) minimum: f32,
    pub(super) maximum: f32,
    pub(super) full_strength_health_fraction: f32,
}

impl LocalDamageTrauma {
    /// Trauma of one damage cue. Unknown maximum health uses the minimum.
    pub(super) fn for_hit(&self, amount: u16, maximum_health: Option<u16>) -> f32 {
        let fraction = maximum_health
            .filter(|maximum| *maximum > 0)
            .map_or(0.0, |maximum| f32::from(amount) / f32::from(maximum));
        let strength = (fraction / self.full_strength_health_fraction).clamp(0.0, 1.0);
        self.minimum + (self.maximum - self.minimum) * strength
    }
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
            MIN_SNAP_DISTANCE_CELLS,
            64.0,
        )?;
        validate_look_ahead(look)?;
        check_range(
            "shake.max_offset_world",
            shake.max_offset_world,
            0.0,
            MAX_SHAKE_OFFSET_WORLD,
        )?;
        check_range("shake.frequency_hz", shake.frequency_hz, 0.5, 60.0)?;
        check_range("shake.decay_per_second", shake.decay_per_second, 0.1, 20.0)?;
        check_range("shake.exponent", shake.exponent, 1.0, 4.0)?;
        let fire = triggers.local_fire;
        let damage = triggers.local_damage;
        for (name, value) in [
            ("shake.triggers.local_fire.straight", fire.straight),
            (
                "shake.triggers.local_fire.sticky_straight",
                fire.sticky_straight,
            ),
            ("shake.triggers.local_fire.lobbed", fire.lobbed),
            ("shake.triggers.local_fire.melee_arc", fire.melee_arc),
            ("shake.triggers.local_fire.cone_spray", fire.cone_spray),
            ("shake.triggers.local_fire.splash", fire.splash),
            ("shake.triggers.local_ultimate", triggers.local_ultimate),
            ("shake.triggers.local_damage.minimum", damage.minimum),
            ("shake.triggers.local_defeat", triggers.local_defeat),
            ("shake.triggers.nearby_explosion", triggers.nearby_explosion),
            ("shake.triggers.nearby_defeat", triggers.nearby_defeat),
        ] {
            check_range(name, value, 0.0, 1.0)?;
        }
        check_range(
            "shake.triggers.local_fire.spread_scale",
            fire.spread_scale,
            1.0,
            MAX_SPREAD_FIRE_SCALE,
        )?;
        check_range(
            "shake.triggers.local_damage.maximum",
            damage.maximum,
            damage.minimum,
            1.0,
        )?;
        check_range(
            "shake.triggers.local_damage.full_strength_health_fraction",
            damage.full_strength_health_fraction,
            0.01,
            1.0,
        )?;
        check_range(
            "shake.triggers.falloff_radius_cells",
            triggers.falloff_radius_cells,
            0.5,
            32.0,
        )
    }
}

/// Look-ahead bounds: the lean ceilings, the fighter safe margin, and the mouse-loop ratio.
fn validate_look_ahead(look: LookAheadTuning) -> Result<(), String> {
    check_range(
        "look_ahead.max_distance_cells",
        look.max_distance_cells,
        0.0,
        MAX_LOOK_AHEAD_CELLS,
    )?;
    check_range(
        "look_ahead.distance_cells",
        look.distance_cells,
        0.0,
        look.max_distance_cells,
    )?;
    check_range(
        "look_ahead.reach_margin_cells",
        look.reach_margin_cells,
        0.0,
        MAX_REACH_MARGIN_CELLS,
    )?;
    check_range(
        "look_ahead.fighter_safe_margin_cells",
        look.fighter_safe_margin_cells,
        MIN_FIGHTER_SAFE_MARGIN_CELLS,
        MAX_FIGHTER_SAFE_MARGIN_CELLS,
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
        (look.max_distance_cells * MIN_FULL_STRENGTH_TO_LEAN_RATIO).max(0.5),
        32.0,
    )?;
    Ok(())
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
    /// World reach of what the aim currently commands (primary weapon or targeted ultimate).
    pub(super) reach: Option<f32>,
    pub(super) bounds: AxisAlignedMapRect,
    pub(super) viewport: Vec2,
    pub(super) dt: f32,
    pub(super) reduced_motion: bool,
    pub(super) reduced_effects: bool,
    /// The player's camera shake intensity in [0, 1]; scales the shake amplitude only.
    pub(super) shake_scale: f32,
    /// Effective `UiScale`; sizes the top HUD band the local fighter's overhead stays below.
    pub(super) ui_scale: f32,
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
        let view = ground_view(viewport_aspect(input.viewport));
        let safe_margin = catalog.look_ahead.fighter_safe_margin_cells * MAP_CELL_SIZE_WORLD;
        let look_target = if input.reduced_motion || input.fighter.is_none() {
            Vec2::ZERO
        } else {
            look_ahead_target(&catalog.look_ahead, input.aim, input.reach, view)
        };
        let look_ahead = self
            .look_ahead
            .step(look_target, catalog.look_ahead.smooth_seconds, dt);
        // The fighter sits at `-lead` from the look target; keep it inside the safe frame.
        let lead = -view.clamp(anchor - (follow + look_ahead), safe_margin);
        let framed = anchor + lead;
        let mut center = clamp_3d_camera_center(framed, input.bounds, input.viewport);
        if input.fighter.is_some() {
            let top_limit = top_hud_fighter_limit(input.viewport, input.ui_scale)
                .clamp(view.near_y + safe_margin, view.far_y - safe_margin);
            center.y = top_hud_band_center(center.y, framed.y, anchor.y, top_limit);
        }

        self.trauma = (self.trauma - catalog.shake.decay_per_second * dt).max(0.0);
        if input.reduced_motion || input.reduced_effects {
            self.trauma = 0.0;
        }
        self.noise_phase =
            (self.noise_phase + dt * catalog.shake.frequency_hz).rem_euclid(NOISE_PHASE_WRAP);
        let shake_scale = if input.shake_scale.is_finite() {
            input.shake_scale.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let amplitude =
            catalog.shake.max_offset_world * self.trauma.powf(catalog.shake.exponent) * shake_scale;
        let shake = if amplitude > 0.0 {
            shake_noise(self.noise_phase) * amplitude
        } else {
            Vec2::ZERO
        };
        CameraRigFrame { center, shake }
    }
}

/// Top HUD safe band (BRL-fwtg): where the map clamp holds the camera south of the rig's framing
/// (the map's north edge), the camera may still pass that edge by just enough to keep the local
/// fighter at most `top_limit` ahead of the look target, so its overhead clears the top-center
/// score plate. It never goes further north than the unclamped framing, so open-field framing
/// and the look-ahead lean are unchanged.
pub(super) fn top_hud_band_center(clamped: f32, framed: f32, fighter: f32, top_limit: f32) -> f32 {
    clamped.max((fighter - top_limit).min(framed))
}

/// The aim lean: at least `distance_cells`, more when the reach (plus margin) would otherwise run
/// past the visible ground in the aim direction, never past `max_distance_cells` or the fighter
/// safe frame, then scaled by the aim-distance strength.
pub(super) fn look_ahead_target(
    tuning: &LookAheadTuning,
    aim: Option<(Vec2, Option<f32>)>,
    reach: Option<f32>,
    view: GroundView,
) -> Vec2 {
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
    let minimum = tuning.distance_cells * MAP_CELL_SIZE_WORLD;
    let maximum = tuning.max_distance_cells * MAP_CELL_SIZE_WORLD;
    let needed = reach
        .filter(|reach| reach.is_finite())
        .map_or(0.0, |reach| {
            reach + tuning.reach_margin_cells * MAP_CELL_SIZE_WORLD
                - view.ray_extent(direction, 0.0)
        });
    // Leaning along `direction` moves the fighter along `-direction` from the look target.
    let safe_margin = tuning.fighter_safe_margin_cells * MAP_CELL_SIZE_WORLD;
    let lean = needed
        .clamp(minimum, maximum)
        .min(view.ray_extent(-direction, safe_margin));
    direction * lean * strength
}

/// Farthest ground a weapon's delivery threatens from the fighter.
pub(super) fn weapon_reach(recipe: &WeaponRecipe) -> f32 {
    match recipe.delivery {
        DeliveryMethod::Straight {
            range,
            muzzle_offset,
            ..
        }
        | DeliveryMethod::StickyStraight {
            range,
            muzzle_offset,
            ..
        } => range + muzzle_offset,
        DeliveryMethod::Lobbed { distance, .. } | DeliveryMethod::Splash { distance, .. } => {
            distance
        }
        DeliveryMethod::MeleeArc { reach, .. } | DeliveryMethod::ConeSpray { reach, .. } => reach,
    }
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
    controlled: Query<
        (&NetworkEntityId, &Position, Option<&ResolvedMatchLoadout>),
        (With<Fighter>, With<Controlled>),
    >,
) {
    let triggers = catalog.shake.triggers;
    let radius = triggers.falloff_radius_cells * MAP_CELL_SIZE_WORLD;
    let controlled = controlled.iter().next();
    let local = controlled.map(|(id, position, _)| (*id, position.0));
    let loadout = controlled.and_then(|(_, _, loadout)| loadout);
    let local_fire = triggers
        .local_fire
        .for_weapon(loadout.map(|loadout| &loadout.primary_weapon.recipe));
    let maximum_health = loadout.map(|loadout| loadout.fighter_stats.maximum_health);
    let nearby = |amount: f32, position: Vec2| {
        local.map_or(0.0, |(_, origin)| {
            falloff(amount, origin.distance(position), radius)
        })
    };
    let mut trauma = 0.0;
    for local_attack in local_attacks.read() {
        trauma += match local_attack.kind {
            LocalAttackKind::PrimaryFire(_) => local_fire,
            LocalAttackKind::Ultimate { .. } => triggers.local_ultimate,
        };
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
                local_fire
            }
            brawler::combat::CombatCue::DamageApplied { target, amount, .. }
                if is_local(target) =>
            {
                triggers.local_damage.for_hit(*amount, maximum_health)
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
