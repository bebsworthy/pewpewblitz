//! Pure touch-gesture state: screen zones, floating sticks, and release classification.

use crate::client::ClientInputSettings;
use bevy::input::touch::Touches;
use bevy::prelude::{Resource, Vec2};

/// Share of the viewport height at the top that stays reserved for the HUD.
const TOP_RESERVED_FRACTION: f32 = 0.12;
/// Stick travel for full deflection at sensitivity 0 and the reduction at sensitivity 1.
const TRAVEL_AT_LOWEST_SENSITIVITY: f32 = 0.26;
const TRAVEL_SENSITIVITY_SPAN: f32 = 0.16;
/// A finger may wobble this many logical pixels and still count as a tap.
const TAP_SLOP_PX: f32 = 10.0;
/// Touch points beyond this are ignored: some Android `GameActivity` builds misreport pointers
/// past eight, and the controls never need more than three.
pub(super) const MAX_CONSIDERED_TOUCHES: usize = 8;

/// The control a touch owns for its whole lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TouchControlKind {
    Move,
    Attack,
    Ultimate,
}

/// Screen layout derived from the logical viewport and the player's touch settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TouchLayout {
    pub(crate) viewport: Vec2,
    /// Finger travel, in logical pixels, for full deflection.
    pub(crate) travel: f32,
    /// Dead zone as a fraction of `travel`.
    pub(crate) dead_fraction: f32,
    pub(crate) ultimate_center: Vec2,
    pub(crate) ultimate_radius: f32,
    top_reserved: f32,
}

impl TouchLayout {
    #[must_use]
    pub(crate) fn new(viewport: Vec2, settings: &ClientInputSettings) -> Self {
        let unit = viewport.min_element().max(1.0);
        let sensitivity = settings.touch_sensitivity.clamp(0.0, 1.0);
        let travel = unit * (TRAVEL_AT_LOWEST_SENSITIVITY - TRAVEL_SENSITIVITY_SPAN * sensitivity);
        let dead_fraction = settings
            .touch_deadzone
            .clamp(0.0, 0.5)
            .max(TAP_SLOP_PX / travel)
            .min(0.9);
        // The ultimate hit circle is exactly the button the HUD draws.
        let controls = crate::client::hud::HudControlLayout::new(viewport);
        Self {
            viewport,
            travel,
            dead_fraction,
            ultimate_center: controls.ultimate.center,
            ultimate_radius: controls.ultimate.radius,
            top_reserved: viewport.y * TOP_RESERVED_FRACTION,
        }
    }

    /// Which control a touch starting at `point` (logical px, y down) belongs to.
    #[must_use]
    pub(crate) fn classify(&self, point: Vec2) -> Option<TouchControlKind> {
        if !point.is_finite() || point.y < self.top_reserved {
            return None;
        }
        if point.x < self.viewport.x * 0.5 {
            Some(TouchControlKind::Move)
        } else if point.distance(self.ultimate_center) <= self.ultimate_radius {
            Some(TouchControlKind::Ultimate)
        } else {
            Some(TouchControlKind::Attack)
        }
    }
}

/// One floating stick: the base is where the finger landed, the knob is where it is now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TouchStick {
    pub(crate) id: u64,
    pub(crate) base: Vec2,
    pub(crate) knob: Vec2,
    /// The finger left the dead zone at least once, so this is a drag rather than a tap.
    pub(crate) aimed: bool,
}

/// What a finished attack or ultimate gesture requests.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum GestureRelease {
    /// Released outside the dead zone: act along this shaped screen deflection.
    Aimed(Vec2),
    /// Never left the dead zone: act with auto-aim.
    Tap,
    /// Dragged back into the dead zone, canceled by the platform, or lost.
    Canceled,
}

impl TouchStick {
    fn new(id: u64, base: Vec2) -> Self {
        Self {
            id,
            base,
            knob: base,
            aimed: false,
        }
    }

    /// Move the knob. A following stick drags its base so the knob stays within travel.
    fn drag(&mut self, point: Vec2, layout: &TouchLayout, follow: bool) {
        if !point.is_finite() {
            return;
        }
        self.knob = point;
        let offset = self.knob - self.base;
        if follow && offset.length() > layout.travel {
            self.base = self.knob - offset.normalize() * layout.travel;
        }
        self.aimed |= self.shaped(layout).is_some();
    }

    /// Raw deflection in screen space (y down), clamped to unit length.
    #[must_use]
    pub(crate) fn deflection(&self, layout: &TouchLayout) -> Vec2 {
        ((self.knob - self.base) / layout.travel).clamp_length_max(1.0)
    }

    /// Post-dead-zone deflection remapped to the full range, or `None` inside the dead zone.
    #[must_use]
    pub(crate) fn shaped(&self, layout: &TouchLayout) -> Option<Vec2> {
        let shaped =
            crate::movement::radial_deadzone(self.deflection(layout), layout.dead_fraction);
        (shaped.length_squared() > f32::EPSILON).then_some(shaped)
    }

    fn release(&self, layout: &TouchLayout) -> GestureRelease {
        match self.shaped(layout) {
            Some(shaped) => GestureRelease::Aimed(shaped),
            None if self.aimed => GestureRelease::Canceled,
            None => GestureRelease::Tap,
        }
    }
}

/// Fixed ticks a release action stays asserted: about one quick mouse click, so a single late
/// input packet over Wi-Fi cannot drop the whole shot. Authority fires once per Ready weapon
/// and activates an ultimate on its rising edge, so the short hold never repeats an action.
pub(super) const RELEASE_HOLD_TICKS: u8 = 4;

/// An action latched on release whose aim rides along for every fixed tick that sends it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct HeldRelease {
    pub(super) button: u8,
    pub(super) aim: Option<Vec2>,
    pub(super) distance: Option<f32>,
    /// Fixed ticks still to send after the one currently latched.
    pub(super) remaining_ticks: u8,
    /// Fixed-clock time of the release while the button waits one tick behind its aim. Immediate
    /// ultimates read the facing authority committed on the previous tick, so an auto-aimed tap
    /// sends its direction first.
    pub(super) aim_lead_from: Option<core::time::Duration>,
}

/// Live touch-control state; also the read-only seam for touch-control presentation.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub(crate) struct TouchControls {
    /// Layout of the current viewport while gameplay touch controls are active.
    pub(crate) layout: Option<TouchLayout>,
    pub(crate) movement: Option<TouchStick>,
    pub(crate) attack: Option<TouchStick>,
    pub(crate) ultimate: Option<TouchStick>,
    /// A touch reached gameplay controls in this session, so touch visuals are relevant.
    pub(crate) seen: bool,
    pub(super) held_release: Option<HeldRelease>,
    /// Touch armed the targeted-ultimate preview and owns disarming it.
    pub(super) armed_ultimate: bool,
}

impl TouchControls {
    fn slot(&mut self, kind: TouchControlKind) -> &mut Option<TouchStick> {
        match kind {
            TouchControlKind::Move => &mut self.movement,
            TouchControlKind::Attack => &mut self.attack,
            TouchControlKind::Ultimate => &mut self.ultimate,
        }
    }

    /// Drop every gesture without acting on it.
    pub(super) fn cancel_all(&mut self) {
        self.layout = None;
        self.movement = None;
        self.attack = None;
        self.ultimate = None;
    }

    /// Advance ownership from this frame's `Touches`, returning finished gestures.
    ///
    /// New touches claim an empty control by their start position; each control owns one finger
    /// and extra fingers are ignored. An owned finger that vanished without an Ended or Canceled
    /// event is treated as canceled, so a lost platform event cannot leave a stick stuck.
    pub(super) fn track(
        &mut self,
        touches: &Touches,
        layout: &TouchLayout,
    ) -> Vec<(TouchControlKind, GestureRelease)> {
        let mut started: Vec<_> = touches.iter_just_pressed().collect();
        started.sort_by_key(|touch| touch.id());
        let crowded = touches.iter().count() > MAX_CONSIDERED_TOUCHES;
        for touch in started {
            if crowded {
                break;
            }
            self.seen = true;
            let owned = [&self.movement, &self.attack, &self.ultimate]
                .into_iter()
                .flatten()
                .any(|stick| stick.id == touch.id());
            if owned {
                continue;
            }
            if let Some(kind) = layout.classify(touch.start_position()) {
                let slot = self.slot(kind);
                if slot.is_none() {
                    *slot = Some(TouchStick::new(touch.id(), touch.start_position()));
                }
            }
        }

        let mut finished = Vec::new();
        for kind in [
            TouchControlKind::Move,
            TouchControlKind::Attack,
            TouchControlKind::Ultimate,
        ] {
            let follow = kind == TouchControlKind::Move;
            let slot = self.slot(kind);
            let Some(stick) = slot.as_mut() else {
                continue;
            };
            if let Some(touch) = touches.get_pressed(stick.id) {
                stick.drag(touch.position(), layout, follow);
                continue;
            }
            let release = if let Some(touch) = touches.get_released(stick.id) {
                stick.drag(touch.position(), layout, follow);
                stick.release(layout)
            } else {
                GestureRelease::Canceled
            };
            *slot = None;
            finished.push((kind, release));
        }
        finished
    }
}
