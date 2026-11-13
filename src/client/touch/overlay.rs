//! Placeholder touch-control visuals until the touch-first HUD (BRL-0155) draws them.
//!
//! Static translucent circles only: no animation, so reduced motion needs no special case. The
//! overlay stays hidden until a touch reaches gameplay, so desktop play never shows it.

use super::model::{TouchControls, TouchStick};
use bevy::prelude::*;
use bevy::ui::UiScale;

pub(in crate::client) struct TouchControlsOverlayPlugin;

impl Plugin for TouchControlsOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, present_touch_controls);
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum TouchOverlayPart {
    UltimateButton,
    MoveBase,
    MoveKnob,
    AttackBase,
    AttackKnob,
    UltimateBase,
    UltimateKnob,
}

const PARTS: [TouchOverlayPart; 7] = [
    TouchOverlayPart::UltimateButton,
    TouchOverlayPart::MoveBase,
    TouchOverlayPart::MoveKnob,
    TouchOverlayPart::AttackBase,
    TouchOverlayPart::AttackKnob,
    TouchOverlayPart::UltimateBase,
    TouchOverlayPart::UltimateKnob,
];

const KNOB_FRACTION: f32 = 0.42;

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn present_touch_controls(
    mut commands: Commands,
    controls: Res<TouchControls>,
    scale: Option<Res<UiScale>>,
    mut parts: Query<(&TouchOverlayPart, &mut Node, &mut Visibility)>,
) {
    if parts.is_empty() {
        if controls.seen && controls.layout.is_some() {
            for part in PARTS {
                commands.spawn((
                    part,
                    Node {
                        position_type: PositionType::Absolute,
                        border: UiRect::all(px(2.0)),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(part.fill()),
                    BorderColor::all(part.stroke()),
                    GlobalZIndex(90),
                    Visibility::Hidden,
                    Name::new("Touch control"),
                ));
            }
        }
        return;
    }
    let scale = scale.map_or(1.0, |scale| scale.0).max(0.01);
    for (part, mut node, mut visibility) in &mut parts {
        let circle = controls
            .layout
            .filter(|_| controls.seen)
            .and_then(|layout| part.circle(&controls, layout));
        let Some((center, radius)) = circle else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        visibility.set_if_neq(Visibility::Inherited);
        let left = px((center.x - radius) / scale);
        let top = px((center.y - radius) / scale);
        let size = px(radius * 2.0 / scale);
        if node.left != left || node.top != top || node.width != size {
            node.left = left;
            node.top = top;
            node.width = size;
            node.height = size;
        }
    }
}

impl TouchOverlayPart {
    fn circle(
        self,
        controls: &TouchControls,
        layout: super::model::TouchLayout,
    ) -> Option<(Vec2, f32)> {
        let travel = layout.travel;
        let base = |stick: Option<TouchStick>| stick.map(|stick| (stick.base, travel));
        let knob = |stick: Option<TouchStick>| {
            stick.map(|stick| {
                (
                    stick.base + stick.deflection(&layout) * travel,
                    travel * KNOB_FRACTION,
                )
            })
        };
        match self {
            Self::UltimateButton => controls
                .ultimate
                .is_none()
                .then_some((layout.ultimate_center, layout.ultimate_radius)),
            Self::MoveBase => base(controls.movement),
            Self::MoveKnob => knob(controls.movement),
            Self::AttackBase => base(controls.attack),
            Self::AttackKnob => knob(controls.attack),
            Self::UltimateBase => base(controls.ultimate),
            Self::UltimateKnob => knob(controls.ultimate),
        }
    }

    fn fill(self) -> Color {
        match self {
            Self::UltimateButton => Color::srgba(0.74, 0.42, 1.0, 0.22),
            Self::MoveBase | Self::AttackBase | Self::UltimateBase => {
                Color::srgba(1.0, 1.0, 1.0, 0.08)
            }
            Self::MoveKnob | Self::AttackKnob => Color::srgba(1.0, 1.0, 1.0, 0.32),
            Self::UltimateKnob => Color::srgba(0.86, 0.62, 1.0, 0.45),
        }
    }

    fn stroke(self) -> Color {
        match self {
            Self::UltimateButton | Self::UltimateBase | Self::UltimateKnob => {
                Color::srgba(0.86, 0.62, 1.0, 0.75)
            }
            _ => Color::srgba(1.0, 1.0, 1.0, 0.5),
        }
    }
}
