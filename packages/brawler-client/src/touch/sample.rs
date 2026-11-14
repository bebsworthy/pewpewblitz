//! Converts tracked touch gestures into the shared local action intent.

use super::model::{
    GestureRelease, HeldRelease, RELEASE_HOLD_TICKS, TouchControlKind, TouchControls, TouchLayout,
};
use crate::input::{primary_placement_range, targeted_ultimate_range};
use crate::{
    ClientInputContext, ClientInputSettings, ClientPlayableGate, Controlled, Fighter,
    PendingLocalActions, Position, presentation_3d, targeting,
};
use bevy::input::touch::Touches;
use bevy::prelude::{Camera, Fixed, GlobalTransform, Query, Res, ResMut, Time, Vec2, Window, With};
use bevy::window::PrimaryWindow;
use brawler::protocol::FighterInput;

/// Screen distance used to probe the ground direction of a stick through the camera.
const PROJECTION_PROBE_PX: f32 = 64.0;

type TouchFighterQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        Option<&'static brawler::builds::ResolvedMatchLoadout>,
    ),
    (With<Fighter>, With<Controlled>),
>;

/// Applies touch gestures over the device sample of this render frame.
///
/// Each channel is overridden only while its touch control is engaged, so keyboard, mouse, and
/// controller input stay unchanged when no finger is down.
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(crate) fn apply_touch_controls(
    mut pending: ResMut<PendingLocalActions>,
    mut controls: ResMut<TouchControls>,
    mut context: ResMut<ClientInputContext>,
    playable: Res<ClientPlayableGate>,
    settings: Res<ClientInputSettings>,
    touches: Option<Res<Touches>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: presentation_3d::ArenaAimCameras,
    auto_aim: targeting::ViewportAutoAim,
    fighters: TouchFighterQuery,
    fixed_time: Option<Res<Time<Fixed>>>,
) {
    let fixed_elapsed = fixed_time.map(|time| time.elapsed());
    let viewport = windows
        .iter()
        .next()
        .map(|window| Vec2::new(window.width(), window.height()));
    let (Some(touches), Some(viewport)) = (touches, viewport) else {
        return;
    };
    if !playable.0 || !matches!(*context, ClientInputContext::Gameplay) {
        if controls.armed_ultimate {
            pending.targeted_ultimate.armed_for = None;
        }
        controls.cancel_all();
        controls.armed_ultimate = false;
        controls.held_release = None;
        return;
    }
    let layout = TouchLayout::new(viewport, &settings);
    controls.layout = Some(layout);
    let finished = controls.track(&touches, &layout);

    let fighter = fighters.iter().next();
    let origin = fighter.map(|(position, _)| position.0);
    let loadout = fighter.and_then(|(_, loadout)| loadout);
    let view = presentation_3d::arena_aim_view(&cameras);
    let direction = |deflection: Vec2| ground_direction(view, origin, deflection);

    let targeted = loadout.filter(|loadout| {
        loadout.ultimate.kind.activation_style()
            == brawler::builds::UltimateActivationStyle::Targeted
    });
    apply_engaged_sticks(
        &mut pending,
        &mut controls,
        &layout,
        loadout,
        targeted,
        &direction,
    );

    let mut open_menu = false;
    for (kind, release) in finished {
        let button = match kind {
            TouchControlKind::Move => continue,
            TouchControlKind::Menu => {
                open_menu |= release == GestureRelease::Tap;
                continue;
            }
            TouchControlKind::Attack => FighterInput::PRIMARY_FIRE,
            TouchControlKind::Ultimate => {
                disarm(&mut pending, &mut controls);
                FighterInput::ULTIMATE
            }
        };
        let (aim, distance) = match release {
            GestureRelease::Canceled => continue,
            GestureRelease::Aimed(shaped) => {
                let range = if kind == TouchControlKind::Ultimate {
                    targeted.and_then(targeted_ultimate_range)
                } else {
                    loadout.and_then(primary_placement_range)
                };
                (
                    direction(shaped),
                    range.map(|range| shaped.length() * range),
                )
            }
            GestureRelease::Tap => {
                let mut targeting = pending.targeted_ultimate;
                targeting.armed_for = targeted
                    .filter(|_| kind == TouchControlKind::Ultimate)
                    .map(|loadout| loadout.ultimate.id);
                auto_aim
                    .resolve(&cameras, targeting)
                    .map_or((None, None), |assisted| {
                        (Some(assisted.direction), Some(assisted.distance))
                    })
            }
        };
        let aim_lead = kind == TouchControlKind::Ultimate
            && targeted.is_none()
            && release == GestureRelease::Tap
            && aim.is_some();
        if !aim_lead {
            pending.latched_buttons |= button;
        }
        controls.held_release = Some(HeldRelease {
            button,
            aim,
            distance,
            remaining_ticks: RELEASE_HOLD_TICKS - 1,
            aim_lead_from: fixed_elapsed.filter(|_| aim_lead),
        });
    }

    carry_held_release(&mut pending, &mut controls, fixed_elapsed);
    // The same local pause request as the keyboard and controller pause bindings; the next frame
    // outside Gameplay drops every gesture.
    crate::apply_pause_request(&mut context, &mut pending, open_menu);
}

/// Writes movement and live aim for the sticks that are currently held.
fn apply_engaged_sticks(
    pending: &mut PendingLocalActions,
    controls: &mut TouchControls,
    layout: &TouchLayout,
    loadout: Option<&brawler::builds::ResolvedMatchLoadout>,
    targeted: Option<&brawler::builds::ResolvedMatchLoadout>,
    direction: &impl Fn(Vec2) -> Option<Vec2>,
) {
    if let Some(stick) = controls.movement {
        // Arcade convention: any deflection past the dead zone moves at full speed.
        pending.move_axis = stick
            .shaped(layout)
            .and_then(direction)
            .unwrap_or(Vec2::ZERO);
    }
    if let Some(shaped) = controls.attack.and_then(|stick| stick.shaped(layout)) {
        pending.aim_axis = direction(shaped);
        pending.aim_distance = loadout
            .and_then(primary_placement_range)
            .map(|range| shaped.length() * range);
    }
    match controls.ultimate.map(|stick| stick.shaped(layout)) {
        Some(Some(shaped)) => {
            if let Some(loadout) = targeted {
                pending.targeted_ultimate.armed_for = Some(loadout.ultimate.id);
                controls.armed_ultimate = true;
            }
            pending.aim_axis = direction(shaped);
            pending.aim_distance = targeted
                .and_then(targeted_ultimate_range)
                .map(|range| shaped.length() * range);
        }
        Some(None) => disarm(pending, controls),
        None => {}
    }
}

pub(super) fn carry_held_release(
    pending: &mut PendingLocalActions,
    controls: &mut TouchControls,
    fixed_elapsed: Option<core::time::Duration>,
) {
    // The release action and its aim ride along for a few fixed ticks, so neither a render frame
    // without a fixed tick nor one late input packet can separate the shot from its direction.
    let Some(held) = controls.held_release.as_mut() else {
        return;
    };
    if let Some(released_at) = held.aim_lead_from {
        // The aim has been sent for at least one fixed tick once the fixed clock moves on.
        if fixed_elapsed.is_none_or(|now| now > released_at) {
            held.aim_lead_from = None;
            pending.latched_buttons |= held.button;
        }
    } else if pending.latched_buttons & held.button == 0 {
        if held.remaining_ticks == 0 {
            controls.held_release = None;
            return;
        }
        held.remaining_ticks -= 1;
        pending.latched_buttons |= held.button;
    }
    if held.aim.is_some() {
        pending.aim_axis = held.aim;
    }
    pending.aim_distance = held.distance;
}

fn disarm(pending: &mut PendingLocalActions, controls: &mut TouchControls) {
    if controls.armed_ultimate {
        pending.targeted_ultimate.armed_for = None;
        controls.armed_ultimate = false;
    }
}

/// Maps a screen-space stick deflection (y down) onto a normalized ground direction.
///
/// With an arena view the fighter's screen point is offset along the finger direction and cast
/// back to the ground, so the lane follows the finger on screen. Without one, screen up is
/// simulation +y because the arena camera has no yaw.
fn ground_direction(
    view: Option<(&Camera, GlobalTransform)>,
    origin: Option<Vec2>,
    deflection: Vec2,
) -> Option<Vec2> {
    let screen = deflection.try_normalize()?;
    let projected = view.zip(origin).and_then(|((camera, transform), origin)| {
        let fighter_screen = camera
            .world_to_viewport(
                &transform,
                presentation_3d::coordinates::ground_position(origin),
            )
            .ok()?;
        let target = presentation_3d::cursor_ground_point(
            camera,
            &transform,
            fighter_screen + screen * PROJECTION_PROBE_PX,
        )?;
        (target - origin).try_normalize()
    });
    projected.or_else(|| Vec2::new(screen.x, -screen.y).try_normalize())
}
