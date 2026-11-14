//! Pure on-screen geometry of the touch-first controls, shared by the HUD visuals and the touch
//! hit areas so the drawn buttons and the touch zones cannot drift apart.

use bevy::math::Rect;
use bevy::prelude::Vec2;

/// Button geometry grows with the short side of the window up to this many logical pixels, so
/// phone controls follow the screen while desktop buttons stay indicator-sized.
const CONTROL_UNIT_MAX: f32 = 480.0;
/// Attack button center offset from the bottom-right corner, in control units.
const ATTACK_OFFSET: Vec2 = Vec2::new(0.32, 0.30);
const ATTACK_RADIUS: f32 = 0.15;
/// Ultimate button center relative to the attack center (left of it, slightly lower).
const ULTIMATE_FROM_ATTACK: Vec2 = Vec2::new(-0.34, 0.08);
const ULTIMATE_RADIUS: f32 = 0.11;
/// Resting movement-zone ring center offset from the bottom-left corner, in control units.
const MOVE_HOME_OFFSET: Vec2 = Vec2::new(0.30, 0.30);
/// Touch menu button: a square in the top-left corner, side and corner inset in control units.
const MENU_SIZE: f32 = 0.12;
const MENU_INSET: f32 = 0.025;

/// A circle in logical window pixels (y down).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScreenCircle {
    pub(crate) center: Vec2,
    pub(crate) radius: f32,
}

/// Where the attack and ultimate buttons and the resting movement zone sit for a viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HudControlLayout {
    pub(crate) attack: ScreenCircle,
    pub(crate) ultimate: ScreenCircle,
    pub(crate) move_home: Vec2,
    /// The touch menu button square; touches starting inside it open the match menu.
    pub(crate) menu: Rect,
}

impl HudControlLayout {
    /// Geometry for a logical viewport (window pixels).
    #[must_use]
    pub(crate) fn new(viewport: Vec2) -> Self {
        let unit = viewport.min_element().clamp(1.0, CONTROL_UNIT_MAX);
        let attack_center = viewport - ATTACK_OFFSET * unit;
        Self {
            attack: ScreenCircle {
                center: attack_center,
                radius: ATTACK_RADIUS * unit,
            },
            ultimate: ScreenCircle {
                center: attack_center + ULTIMATE_FROM_ATTACK * unit,
                radius: ULTIMATE_RADIUS * unit,
            },
            move_home: Vec2::new(
                MOVE_HOME_OFFSET.x * unit,
                viewport.y - MOVE_HOME_OFFSET.y * unit,
            ),
            menu: Rect::from_corners(
                Vec2::splat(MENU_INSET * unit),
                Vec2::splat((MENU_INSET + MENU_SIZE) * unit),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 16:9 desktop, the phone after its cutout strip, the full phone, and 19.5:9.
    const VIEWPORTS: [Vec2; 4] = [
        Vec2::new(1280.0, 720.0),
        Vec2::new(868.0, 411.0),
        Vec2::new(914.0, 411.0),
        Vec2::new(1560.0, 720.0),
    ];

    #[test]
    fn buttons_stay_on_screen_in_the_right_half_and_apart() {
        for viewport in VIEWPORTS {
            let layout = HudControlLayout::new(viewport);
            for circle in [layout.attack, layout.ultimate] {
                assert!(
                    circle.center.x - circle.radius > viewport.x * 0.5,
                    "{viewport}"
                );
                assert!(circle.center.x + circle.radius < viewport.x, "{viewport}");
                assert!(circle.center.y + circle.radius < viewport.y, "{viewport}");
                // Below the reserved top band of the touch layout.
                assert!(
                    circle.center.y - circle.radius > viewport.y * 0.12,
                    "{viewport}"
                );
            }
            let gap = layout.attack.center.distance(layout.ultimate.center)
                - layout.attack.radius
                - layout.ultimate.radius;
            assert!(gap > 8.0, "{viewport}: buttons {gap} px apart");
            assert!(layout.move_home.x < viewport.x * 0.5);
            // The menu button stays in the top-left corner, above the movement zone's home.
            assert!(layout.menu.min.min_element() > 0.0, "{viewport}");
            assert!(layout.menu.max.x < viewport.x * 0.5, "{viewport}");
            assert!(layout.menu.max.y < layout.move_home.y, "{viewport}");
        }
    }

    #[test]
    fn phone_buttons_are_large_touch_targets_and_desktop_buttons_are_capped() {
        let phone = HudControlLayout::new(Vec2::new(868.0, 411.0));
        // 48 dp is the platform minimum touch target; logical px are dp on Android.
        assert!(phone.ultimate.radius * 2.0 >= 80.0);
        assert!(phone.attack.radius * 2.0 >= 110.0);
        assert!(phone.menu.width() >= 48.0 && phone.menu.height() >= 48.0);
        let desktop = HudControlLayout::new(Vec2::new(2560.0, 1440.0));
        assert!((desktop.attack.radius - ATTACK_RADIUS * CONTROL_UNIT_MAX).abs() < 1e-3);
    }
}
