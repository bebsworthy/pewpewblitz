//! Touch-to-intent mapping driven by synthetic `TouchInput` messages through the production
//! sampling chain.

use super::model::{TouchControlKind, TouchLayout};
use super::{TouchControls, apply_touch_controls};
use crate::tests::{resolved_arc_launcher_loadout, resolved_reveal_scan_loadout};
use crate::{
    ClientInputContext, ClientInputSettings, ClientPlayableGate, Controlled, Fighter,
    InputDeviceActivity, PendingLocalActions, Position, resolve_targeted_ultimate_input,
    sample_local_input,
};
use bevy::input::touch::{TouchInput, TouchPhase, Touches, touch_screen_input_system};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use brawler::protocol::FighterInput;

/// `Window::default()` is 1280x720 logical px: short side 720.
const VIEWPORT: Vec2 = Vec2::new(1280.0, 720.0);
const MOVE_START: Vec2 = Vec2::new(300.0, 500.0);
const ATTACK_START: Vec2 = Vec2::new(1100.0, 600.0);

struct TouchApp {
    app: App,
    window: Entity,
}

impl TouchApp {
    fn new(loadout: brawler::builds::ResolvedMatchLoadout) -> Self {
        let charge = loadout.ultimate.charge_policy.maximum;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<TouchInput>()
            .init_resource::<Touches>()
            .insert_resource(ButtonInput::<KeyCode>::default())
            .init_resource::<PendingLocalActions>()
            .insert_resource(ClientPlayableGate(true))
            .init_resource::<ClientInputContext>()
            .init_resource::<InputDeviceActivity>()
            .init_resource::<ClientInputSettings>()
            .init_resource::<TouchControls>()
            .add_systems(PreUpdate, touch_screen_input_system)
            .add_systems(
                Update,
                (
                    sample_local_input,
                    apply_touch_controls,
                    resolve_targeted_ultimate_input,
                )
                    .chain(),
            );
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        app.world_mut().spawn((
            Fighter,
            Controlled,
            Position::default(),
            loadout,
            brawler::builds::AbilityState {
                charge,
                phase: brawler::builds::AbilityPhase::Ready,
            },
        ));
        Self { app, window }
    }

    fn touch(&mut self, id: u64, phase: TouchPhase, position: Vec2) -> &mut Self {
        let window = self.window;
        self.app.world_mut().write_message(TouchInput {
            phase,
            position,
            window,
            force: None,
            id,
        });
        self
    }

    fn frame(&mut self) -> PendingLocalActions {
        self.app.update();
        *self.app.world().resource::<PendingLocalActions>()
    }

    /// The fixed tick consumes a latched action; mirror `write_client_input`'s clear.
    fn consume_latch(&mut self) {
        self.app
            .world_mut()
            .resource_mut::<PendingLocalActions>()
            .latched_buttons = 0;
    }

    fn controls(&self) -> &TouchControls {
        self.app.world().resource::<TouchControls>()
    }
}

fn assert_direction(actual: Option<Vec2>, expected: Vec2) {
    let actual = actual.expect("an aim direction");
    assert!(
        actual.distance(expected) < 0.001,
        "expected {expected:?}, got {actual:?}"
    );
}

#[test]
fn layout_assigns_zones_and_scales_with_settings() {
    let settings = ClientInputSettings::default();
    let layout = TouchLayout::new(VIEWPORT, &settings);
    assert_eq!(layout.classify(MOVE_START), Some(TouchControlKind::Move));
    assert_eq!(
        layout.classify(ATTACK_START),
        Some(TouchControlKind::Attack)
    );
    assert_eq!(
        layout.classify(layout.ultimate_center),
        Some(TouchControlKind::Ultimate)
    );
    // The top band stays with the HUD in both halves.
    assert_eq!(layout.classify(Vec2::new(100.0, 20.0)), None);
    assert_eq!(layout.classify(Vec2::new(1200.0, 20.0)), None);
    // The ultimate button never overlaps the move half.
    assert!(layout.ultimate_center.x - layout.ultimate_radius > VIEWPORT.x * 0.5);

    let sensitive = TouchLayout::new(
        VIEWPORT,
        &ClientInputSettings {
            touch_sensitivity: 1.0,
            ..settings
        },
    );
    assert!(sensitive.travel < layout.travel);
    // A zero dead zone still keeps the tap slop.
    let no_deadzone = TouchLayout::new(
        VIEWPORT,
        &ClientInputSettings {
            touch_deadzone: 0.0,
            ..settings
        },
    );
    assert!(no_deadzone.dead_fraction * no_deadzone.travel >= 9.99);
}

#[test]
fn floating_move_stick_drives_full_speed_movement_and_releases_to_neutral() {
    let mut test = TouchApp::new(resolved_arc_launcher_loadout());
    test.touch(1, TouchPhase::Started, MOVE_START);
    // Inside the dead zone the stick is neutral.
    assert_eq!(test.frame().move_axis, Vec2::ZERO);

    test.touch(1, TouchPhase::Moved, MOVE_START + Vec2::new(60.0, 0.0));
    let pending = test.frame();
    assert_direction(Some(pending.move_axis), Vec2::X);
    assert_eq!(pending.aim_axis, None, "movement never aims");

    // Screen up (y down) is simulation +y; far past travel drags the floating base along.
    test.touch(1, TouchPhase::Moved, MOVE_START + Vec2::new(0.0, -400.0));
    assert_direction(Some(test.frame().move_axis), Vec2::Y);
    let base = test.controls().movement.expect("move stick").base;
    assert!(base.y < MOVE_START.y - 200.0, "base follows the finger");

    test.touch(1, TouchPhase::Ended, MOVE_START + Vec2::new(0.0, -400.0));
    let pending = test.frame();
    assert_eq!(pending.move_axis, Vec2::ZERO);
    assert_eq!(pending.latched_buttons, 0);
    assert!(test.controls().movement.is_none());
}

#[test]
fn attack_drag_shows_aim_and_release_fires_once_along_the_drag() {
    let mut test = TouchApp::new(resolved_arc_launcher_loadout());
    test.touch(7, TouchPhase::Started, ATTACK_START);
    test.frame();
    test.touch(7, TouchPhase::Moved, ATTACK_START + Vec2::new(-200.0, 0.0));
    let pending = test.frame();
    // While dragging the lane shows (aim present) but nothing fires yet.
    assert_direction(pending.aim_axis, Vec2::NEG_X);
    assert!(
        pending
            .aim_distance
            .is_some_and(|distance| (distance - 520.0).abs() < 0.01),
        "full deflection maps the lobbed weapon's full range"
    );
    assert_eq!(pending.latched_buttons & FighterInput::PRIMARY_FIRE, 0);
    assert_eq!(pending.held_buttons & FighterInput::PRIMARY_FIRE, 0);

    test.touch(7, TouchPhase::Ended, ATTACK_START + Vec2::new(-200.0, 0.0));
    let pending = test.frame();
    assert_ne!(pending.latched_buttons & FighterInput::PRIMARY_FIRE, 0);
    assert_direction(pending.aim_axis, Vec2::NEG_X);

    // A render frame without a fixed tick keeps the release aim with the pending shot.
    let pending = test.frame();
    assert_ne!(pending.latched_buttons & FighterInput::PRIMARY_FIRE, 0);
    assert_direction(pending.aim_axis, Vec2::NEG_X);

    // The action stays asserted for the short release hold, one fixed tick at a time.
    for _ in 1..super::model::RELEASE_HOLD_TICKS {
        test.consume_latch();
        let pending = test.frame();
        assert_ne!(pending.latched_buttons & FighterInput::PRIMARY_FIRE, 0);
        assert_direction(pending.aim_axis, Vec2::NEG_X);
    }
    test.consume_latch();
    let pending = test.frame();
    assert_eq!(pending.latched_buttons, 0);
    assert_eq!(pending.aim_axis, None);
}

#[test]
fn quick_tap_fires_with_auto_aim_and_without_a_target_keeps_facing() {
    let mut test = TouchApp::new(resolved_arc_launcher_loadout());
    // Start and end in one frame, with a wobble inside the tap slop.
    test.touch(3, TouchPhase::Started, ATTACK_START);
    test.touch(3, TouchPhase::Ended, ATTACK_START + Vec2::new(4.0, -3.0));
    let pending = test.frame();
    assert_ne!(pending.latched_buttons & FighterInput::PRIMARY_FIRE, 0);
    // No arena camera means no viewport target: the shot keeps the current facing.
    assert_eq!(pending.aim_axis, None);
    assert_eq!(pending.aim_distance, None);
}

#[test]
fn dragging_back_into_the_dead_zone_cancels_the_shot() {
    let mut test = TouchApp::new(resolved_arc_launcher_loadout());
    test.touch(4, TouchPhase::Started, ATTACK_START);
    test.frame();
    test.touch(4, TouchPhase::Moved, ATTACK_START + Vec2::new(0.0, -120.0));
    assert!(test.frame().aim_axis.is_some());
    test.touch(4, TouchPhase::Moved, ATTACK_START + Vec2::new(2.0, -2.0));
    test.touch(4, TouchPhase::Ended, ATTACK_START + Vec2::new(2.0, -2.0));
    let pending = test.frame();
    assert_eq!(pending.latched_buttons, 0);
    assert_eq!(pending.aim_axis, None);
}

#[test]
fn targeted_ultimate_drag_previews_and_release_activates_with_placement() {
    let loadout = resolved_reveal_scan_loadout();
    let ultimate_id = loadout.ultimate.id;
    let mut test = TouchApp::new(loadout);
    let center = TouchLayout::new(VIEWPORT, &ClientInputSettings::default()).ultimate_center;
    test.touch(9, TouchPhase::Started, center);
    let pending = test.frame();
    assert!(!pending.targeted_ultimate.is_targeting(ultimate_id));

    let half = Vec2::new(0.0, 65.0);
    test.touch(9, TouchPhase::Moved, center + half);
    let pending = test.frame();
    assert!(
        pending.targeted_ultimate.is_targeting(ultimate_id),
        "dragging arms the area preview"
    );
    assert_direction(pending.aim_axis, Vec2::NEG_Y);
    let distance = pending.aim_distance.expect("placement distance");
    assert!(distance > 0.0 && distance < 640.0, "{distance}");
    assert_eq!(pending.latched_buttons, 0);

    test.touch(9, TouchPhase::Ended, center + half);
    let pending = test.frame();
    assert_ne!(pending.latched_buttons & FighterInput::ULTIMATE, 0);
    assert_eq!(pending.latched_buttons & FighterInput::PRIMARY_FIRE, 0);
    assert!(!pending.targeted_ultimate.is_targeting(ultimate_id));
    assert_direction(pending.aim_axis, Vec2::NEG_Y);
    assert!(
        pending
            .aim_distance
            .is_some_and(|value| (value - distance).abs() < 0.01)
    );
}

#[test]
fn ultimate_tap_activates_immediately_with_auto_aim_fallback() {
    let mut test = TouchApp::new(resolved_reveal_scan_loadout());
    let center = TouchLayout::new(VIEWPORT, &ClientInputSettings::default()).ultimate_center;
    test.touch(2, TouchPhase::Started, center);
    test.touch(2, TouchPhase::Ended, center);
    let pending = test.frame();
    assert_ne!(pending.latched_buttons & FighterInput::ULTIMATE, 0);
    // No viewport target: authority places the scan along facing at full range.
    assert_eq!(pending.aim_axis, None);
    assert_eq!(pending.aim_distance, None);

    // An immediate ultimate (Dash) uses the same tap.
    let mut dash = TouchApp::new(resolved_arc_launcher_loadout());
    dash.touch(2, TouchPhase::Started, center);
    dash.touch(2, TouchPhase::Ended, center);
    assert_ne!(dash.frame().latched_buttons & FighterInput::ULTIMATE, 0);
}

#[test]
fn simultaneous_touches_each_own_one_control_and_extra_fingers_are_ignored() {
    let mut test = TouchApp::new(resolved_arc_launcher_loadout());
    test.touch(10, TouchPhase::Started, MOVE_START);
    test.touch(11, TouchPhase::Started, ATTACK_START);
    test.frame();
    test.touch(10, TouchPhase::Moved, MOVE_START + Vec2::new(0.0, 80.0));
    test.touch(11, TouchPhase::Moved, ATTACK_START + Vec2::new(150.0, 0.0));
    // A third finger on the move half cannot steal the stick.
    test.touch(12, TouchPhase::Started, MOVE_START + Vec2::new(-100.0, 0.0));
    let pending = test.frame();
    assert_direction(Some(pending.move_axis), Vec2::NEG_Y);
    assert_direction(pending.aim_axis, Vec2::X);
    assert_eq!(test.controls().movement.map(|stick| stick.id), Some(10));

    // Releasing the extra finger changes nothing; releasing the attack fires while moving.
    test.touch(12, TouchPhase::Ended, MOVE_START + Vec2::new(-100.0, 0.0));
    test.touch(11, TouchPhase::Ended, ATTACK_START + Vec2::new(150.0, 0.0));
    let pending = test.frame();
    assert_ne!(pending.latched_buttons & FighterInput::PRIMARY_FIRE, 0);
    assert_direction(Some(pending.move_axis), Vec2::NEG_Y);
}

#[test]
fn canceled_and_lost_touches_never_fire() {
    let mut test = TouchApp::new(resolved_arc_launcher_loadout());
    test.touch(5, TouchPhase::Started, ATTACK_START);
    test.frame();
    test.touch(5, TouchPhase::Moved, ATTACK_START + Vec2::new(-150.0, 0.0));
    test.frame();
    test.touch(
        5,
        TouchPhase::Canceled,
        ATTACK_START + Vec2::new(-150.0, 0.0),
    );
    let pending = test.frame();
    assert_eq!(pending.latched_buttons, 0);
    assert!(test.controls().attack.is_none());

    // A finger whose Ended event never arrives (dropped pointer) is canceled, not stuck.
    test.touch(6, TouchPhase::Started, MOVE_START);
    test.touch(8, TouchPhase::Started, ATTACK_START);
    test.frame();
    test.touch(6, TouchPhase::Moved, MOVE_START + Vec2::new(90.0, 0.0));
    test.touch(8, TouchPhase::Moved, ATTACK_START + Vec2::new(-150.0, 0.0));
    test.frame();
    test.app.world_mut().resource_mut::<Touches>().reset_all();
    let pending = test.frame();
    assert_eq!(pending.latched_buttons, 0);
    assert_eq!(pending.move_axis, Vec2::ZERO);
    assert!(test.controls().movement.is_none() && test.controls().attack.is_none());
}

#[test]
fn leaving_gameplay_drops_gestures_without_firing() {
    let mut test = TouchApp::new(resolved_reveal_scan_loadout());
    let center = TouchLayout::new(VIEWPORT, &ClientInputSettings::default()).ultimate_center;
    test.touch(1, TouchPhase::Started, ATTACK_START);
    test.touch(2, TouchPhase::Started, center);
    test.frame();
    test.touch(1, TouchPhase::Moved, ATTACK_START + Vec2::new(-150.0, 0.0));
    test.touch(2, TouchPhase::Moved, center + Vec2::new(0.0, 80.0));
    test.frame();
    *test.app.world_mut().resource_mut::<ClientInputContext>() = ClientInputContext::Menu;
    test.touch(1, TouchPhase::Ended, ATTACK_START + Vec2::new(-150.0, 0.0));
    test.touch(2, TouchPhase::Ended, center + Vec2::new(0.0, 80.0));
    let pending = test.frame();
    assert_eq!(pending.latched_buttons, 0);
    assert_eq!(pending.targeted_ultimate.armed_for, None);
    assert!(test.controls().layout.is_none());

    // Back in gameplay, the fingers that ended during the menu own nothing.
    *test.app.world_mut().resource_mut::<ClientInputContext>() = ClientInputContext::Gameplay;
    let pending = test.frame();
    assert_eq!(pending.latched_buttons, 0);
    assert!(test.controls().attack.is_none() && test.controls().ultimate.is_none());
}

#[test]
fn auto_aimed_immediate_ultimate_sends_its_aim_one_fixed_tick_before_the_button() {
    let mut pending = PendingLocalActions::default();
    let released_at = core::time::Duration::from_millis(500);
    let aim = Some(Vec2::Y);
    let mut controls = TouchControls {
        held_release: Some(super::model::HeldRelease {
            button: FighterInput::ULTIMATE,
            aim,
            distance: Some(120.0),
            remaining_ticks: super::model::RELEASE_HOLD_TICKS - 1,
            aim_lead_from: Some(released_at),
        }),
        ..TouchControls::default()
    };
    // Same fixed tick as the release: only the aim goes out.
    super::sample::carry_held_release(&mut pending, &mut controls, Some(released_at));
    assert_eq!(pending.latched_buttons, 0);
    assert_eq!(pending.aim_axis, aim);
    // The fixed clock moved on, so the facing is committed and the ultimate follows with it.
    let next = released_at + brawler::timing::SIMULATION_TICK;
    super::sample::carry_held_release(&mut pending, &mut controls, Some(next));
    assert_ne!(pending.latched_buttons & FighterInput::ULTIMATE, 0);
    assert_eq!(pending.aim_axis, aim);
}

#[test]
fn menu_button_tap_opens_the_match_menu_without_feeding_a_stick() {
    let mut test = TouchApp::new(resolved_arc_launcher_loadout());
    let menu = TouchLayout::new(VIEWPORT, &ClientInputSettings::default()).menu;
    assert_eq!(
        TouchLayout::new(VIEWPORT, &ClientInputSettings::default()).classify(menu.center()),
        Some(TouchControlKind::Menu)
    );
    // A movement finger is already down when the other thumb taps the menu button.
    test.touch(1, TouchPhase::Started, MOVE_START);
    test.frame();
    test.touch(1, TouchPhase::Moved, MOVE_START + Vec2::new(60.0, 0.0));
    test.touch(2, TouchPhase::Started, menu.center());
    let pending = test.frame();
    assert_direction(Some(pending.move_axis), Vec2::X);
    // A wobble that stays on the button neither moves nor aims.
    test.touch(2, TouchPhase::Moved, menu.center() + Vec2::new(12.0, 12.0));
    let pending = test.frame();
    assert_direction(Some(pending.move_axis), Vec2::X);
    assert_eq!(pending.aim_axis, None);
    assert_eq!(test.controls().movement.map(|stick| stick.id), Some(1));
    assert!(test.controls().attack.is_none());
    assert_eq!(
        *test.app.world().resource::<ClientInputContext>(),
        ClientInputContext::Gameplay,
        "pressing only arms the button"
    );

    test.touch(2, TouchPhase::Ended, menu.center() + Vec2::new(12.0, 12.0));
    let pending = test.frame();
    assert_eq!(pending.latched_buttons, 0);
    assert_eq!(
        *test.app.world().resource::<ClientInputContext>(),
        ClientInputContext::Menu
    );
    test.frame();
    assert!(
        test.controls().movement.is_none(),
        "the menu drops gestures"
    );

    // Closing the menu hands the screen back: the old finger owns nothing, a new one moves.
    *test.app.world_mut().resource_mut::<ClientInputContext>() = ClientInputContext::Gameplay;
    test.touch(1, TouchPhase::Ended, MOVE_START + Vec2::new(60.0, 0.0));
    assert_eq!(test.frame().move_axis, Vec2::ZERO);
    test.touch(3, TouchPhase::Started, MOVE_START);
    test.frame();
    test.touch(3, TouchPhase::Moved, MOVE_START + Vec2::new(0.0, -60.0));
    assert_direction(Some(test.frame().move_axis), Vec2::Y);
}

#[test]
fn menu_button_released_off_the_button_does_nothing() {
    let mut test = TouchApp::new(resolved_arc_launcher_loadout());
    let menu = TouchLayout::new(VIEWPORT, &ClientInputSettings::default()).menu;
    test.touch(1, TouchPhase::Started, menu.center());
    test.frame();
    // Sliding off into the movement half still never becomes a stick.
    test.touch(1, TouchPhase::Moved, MOVE_START);
    assert_eq!(test.frame().move_axis, Vec2::ZERO);
    test.touch(1, TouchPhase::Ended, MOVE_START);
    let pending = test.frame();
    assert_eq!(pending.move_axis, Vec2::ZERO);
    assert_eq!(
        *test.app.world().resource::<ClientInputContext>(),
        ClientInputContext::Gameplay
    );
    assert!(test.controls().menu.is_none());
}

#[test]
fn menu_button_tap_within_one_frame_opens_the_match_menu() {
    let mut test = TouchApp::new(resolved_arc_launcher_loadout());
    let menu = TouchLayout::new(VIEWPORT, &ClientInputSettings::default()).menu;
    // `adb shell input tap`: down and up land before the same frame.
    test.touch(1, TouchPhase::Started, menu.center());
    test.touch(1, TouchPhase::Ended, menu.center());
    let pending = test.frame();
    assert_eq!(pending.latched_buttons, 0);
    assert_eq!(
        *test.app.world().resource::<ClientInputContext>(),
        ClientInputContext::Menu
    );
    for _ in 0..3 {
        test.frame();
        assert_eq!(
            *test.app.world().resource::<ClientInputContext>(),
            ClientInputContext::Menu,
            "the same tap never closes the menu again"
        );
    }
    assert!(test.controls().menu.is_none());
}
