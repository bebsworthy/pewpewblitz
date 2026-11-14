//! Attack and ultimate buttons (both platforms), and the touch stick visuals and menu button
//! (touch only).
//!
//! Geometry comes from [`HudControlLayout`], the same circles the touch sampler hit-tests, and
//! live stick state from the [`TouchControls`] seam. Everything here reads replicated fighter state
//! and local intent; nothing writes gameplay input.

use super::layout::{HudControlLayout, ScreenCircle};
use super::palette;
use crate::settings::binding_edit::BindingValue;
use crate::touch::TouchControls;
use crate::{
    ActiveInputDevice, ClientInputContext, ClientInputSettings, ClientShellSettings,
    PendingLocalActions, ui,
};
use bevy::prelude::*;
use bevy::ui::UiScale;
use bevy::window::PrimaryWindow;
use brawler::builds::{AbilityPhase, AbilityState, ResolvedMatchLoadout};
use brawler::combat::{AuthoritativeTick, Defeated, WeaponState};
use brawler::protocol::Fighter;
use core::f32::consts::TAU;

/// Ring fill steps; the gradient is rebuilt only when the quantized fill changes.
const RING_STEPS: f32 = 100.0;
/// Ultimate ring thickness as a share of the button.
const RING_INSET_PERCENT: f32 = 9.0;
/// Ready pulse: scale amplitude and frequency (Hz). Reduced motion disables it.
const READY_PULSE_SCALE: f32 = 0.06;
const READY_PULSE_HZ: f32 = 1.3;
/// Opacity of controls while the local fighter is defeated.
const DEFEATED_ALPHA: f32 = 0.35;
/// Floating stick knob size as a share of the travel radius.
const KNOB_FRACTION: f32 = 0.42;

/// How the ultimate button presents this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UltimateLook {
    Charging,
    Ready,
    /// A mouse/controller targeted ultimate is waiting for its confirm.
    Targeting,
    /// The ultimate is running; whole seconds left when it has an expiry worth showing.
    Active(Option<u64>),
}

/// Pure ultimate face: radial fill share and look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UltimateFace {
    pub(crate) fill: f32,
    pub(crate) look: UltimateLook,
}

pub(crate) fn ultimate_face(
    ability: AbilityState,
    maximum_charge: u16,
    now: Option<u64>,
    targeting: bool,
) -> UltimateFace {
    let seconds =
        |expires_at_tick: u64| now.map(|now| expires_at_tick.saturating_sub(now).div_ceil(60));
    let look = match ability.phase {
        AbilityPhase::Charging => UltimateLook::Charging,
        AbilityPhase::Ready if targeting => UltimateLook::Targeting,
        AbilityPhase::Ready => UltimateLook::Ready,
        AbilityPhase::Dashing { .. } => UltimateLook::Active(None),
        AbilityPhase::Deployed {
            expires_at_tick, ..
        }
        | AbilityPhase::Cloaked {
            expires_at_tick, ..
        }
        | AbilityPhase::FieldActive {
            expires_at_tick, ..
        }
        | AbilityPhase::ElementalFieldActive {
            expires_at_tick, ..
        } => UltimateLook::Active(seconds(expires_at_tick)),
    };
    let fill = match look {
        UltimateLook::Ready | UltimateLook::Targeting | UltimateLook::Active(_) => 1.0,
        UltimateLook::Charging => f32::from(ability.charge) / f32::from(maximum_charge.max(1)),
    };
    UltimateFace {
        fill: fill.clamp(0.0, 1.0),
        look,
    }
}

/// Reload share while the magazine is empty, `None` while a shot is available.
pub(crate) fn attack_reload(weapon: &WeaponState, now: Option<u64>) -> Option<f32> {
    if weapon.ammo > 0 {
        return None;
    }
    let progress = weapon
        .ammo_recovery
        .zip(now)
        .map_or(0.0, |(recovery, now)| {
            let duration = recovery
                .ready_at_tick
                .saturating_sub(recovery.started_at_tick)
                .max(1);
            let elapsed = now.saturating_sub(recovery.started_at_tick).min(duration);
            #[allow(clippy::cast_precision_loss, reason = "tick spans are small")]
            let share = elapsed as f32 / duration as f32;
            share
        });
    Some(progress)
}

/// Short key/controller hints for the attack and ultimate buttons.
pub(crate) fn control_hints(
    settings: &ClientInputSettings,
    device: ActiveInputDevice,
) -> [String; 2] {
    match device {
        ActiveInputDevice::KeyboardMouse => [
            match settings.mouse_primary {
                MouseButton::Left => "LMB".to_string(),
                MouseButton::Right => "RMB".to_string(),
                MouseButton::Middle => "MMB".to_string(),
                other => BindingValue::Mouse(other).label(),
            },
            BindingValue::Key(settings.keyboard.ultimate).label(),
        ],
        ActiveInputDevice::Gamepad(_) => {
            [settings.gamepad.primary, settings.gamepad.ultimate].map(|button| match button {
                GamepadButton::RightTrigger2 => "RT".to_string(),
                GamepadButton::LeftTrigger2 => "LT".to_string(),
                GamepadButton::RightTrigger => "RB".to_string(),
                GamepadButton::LeftTrigger => "LB".to_string(),
                GamepadButton::South => "A".to_string(),
                GamepadButton::East => "B".to_string(),
                GamepadButton::West => "X".to_string(),
                GamepadButton::North => "Y".to_string(),
                other => BindingValue::Gamepad(other).label(),
            })
        }
    }
}

/// Touch visuals and hidden key hints apply on Android and once a touch reached gameplay.
pub(super) fn touch_mode(controls: &TouchControls) -> bool {
    controls.seen || cfg!(target_os = "android")
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ControlPart {
    Group,
    AttackButton,
    AttackIcon,
    UltimateButton,
    UltimateCore,
    UltimateIcon,
    UltimateTimer,
    Hint(usize),
    HintText(usize),
    MoveHome,
    /// The touch menu button; the touch sampler owns its hit square.
    MenuButton,
    Stick(StickPart),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StickPart {
    MoveBase,
    MoveKnob,
    AttackBase,
    AttackKnob,
    UltimateKnob,
}

const STICK_PARTS: [StickPart; 5] = [
    StickPart::MoveBase,
    StickPart::MoveKnob,
    StickPart::AttackBase,
    StickPart::AttackKnob,
    StickPart::UltimateKnob,
];

/// Last quantized ring fill and color written to a button's conic gradient.
#[derive(Component, Default, PartialEq)]
pub(super) struct RingFill(Option<(u16, [u8; 4])>);

fn circle_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        border: UiRect::all(px(3.0)),
        border_radius: BorderRadius::MAX,
        ..default()
    }
}

fn inset_node(inset_percent: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: percent(inset_percent),
        top: percent(inset_percent),
        width: percent(100.0 - 2.0 * inset_percent),
        height: percent(100.0 - 2.0 * inset_percent),
        border_radius: BorderRadius::MAX,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    }
}

fn hint_chip(commands: &mut Commands, font: &Handle<Font>, index: usize) -> Entity {
    commands
        .spawn((
            ControlPart::Hint(index),
            Node {
                position_type: PositionType::Absolute,
                bottom: px(-12.0),
                left: percent(50.0),
                padding: UiRect::axes(px(7.0), px(1.0)),
                border: UiRect::all(px(2.0)),
                border_radius: BorderRadius::all(px(6.0)),
                ..default()
            },
            UiTransform::from_translation(Val2::percent(-50.0, 0.0)),
            BackgroundColor(ui::theme::PANEL),
            BorderColor::all(ui::theme::INK),
            Visibility::Hidden,
        ))
        .with_child((
            ControlPart::HintText(index),
            Text::new(""),
            ui::theme::hud_text(font, 13.0),
            TextColor(ui::theme::TEXT),
            TextLayout::new(Justify::Center, LineBreak::NoWrap),
        ))
        .id()
}

pub(super) fn spawn_controls(
    commands: &mut Commands,
    font: &Handle<Font>,
    asset_server: Option<&AssetServer>,
    parent: Entity,
) {
    let icon = |name: &str| {
        asset_server
            .map(|server| server.load(ui::icons::path(name)))
            .unwrap_or_default()
    };
    let group = commands
        .spawn((
            ControlPart::Group,
            Name::new("HUD controls"),
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            },
            Visibility::Hidden,
        ))
        .id();
    commands.entity(parent).add_child(group);
    // Touch stick visuals sit under the buttons.
    let touch = spawn_touch_visuals(commands);
    commands.entity(group).add_children(&touch);
    let ultimate = spawn_ultimate_button(commands, font, icon("blast"));
    let attack = spawn_attack_button(commands, font, icon("crosshair"));
    let menu = spawn_menu_button(commands, icon("menu"));
    commands
        .entity(group)
        .add_children(&[ultimate, attack, menu]);
}

fn spawn_touch_visuals(commands: &mut Commands) -> Vec<Entity> {
    let mut touch = vec![
        commands
            .spawn((
                ControlPart::MoveHome,
                circle_node(),
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.05)),
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.28)),
                Visibility::Hidden,
            ))
            .id(),
    ];
    for part in STICK_PARTS {
        let (fill, stroke) = match part {
            StickPart::MoveBase | StickPart::AttackBase => (
                Color::srgba(1.0, 1.0, 1.0, 0.08),
                Color::srgba(1.0, 1.0, 1.0, 0.5),
            ),
            StickPart::MoveKnob | StickPart::AttackKnob => (
                Color::srgba(1.0, 1.0, 1.0, 0.34),
                Color::srgba(1.0, 1.0, 1.0, 0.7),
            ),
            StickPart::UltimateKnob => (palette::ULTIMATE.with_alpha(0.5), palette::ULTIMATE),
        };
        touch.push(
            commands
                .spawn((
                    ControlPart::Stick(part),
                    circle_node(),
                    BackgroundColor(fill),
                    BorderColor::all(stroke),
                    Visibility::Hidden,
                ))
                .id(),
        );
    }
    touch
}

fn spawn_menu_button(commands: &mut Commands, icon: Handle<Image>) -> Entity {
    commands
        .spawn((
            ControlPart::MenuButton,
            Name::new("HUD menu button"),
            Node {
                position_type: PositionType::Absolute,
                border: UiRect::all(px(3.0)),
                border_radius: BorderRadius::all(percent(24.0)),
                ..default()
            },
            BackgroundColor(palette::PLATE),
            BorderColor::all(ui::theme::INK),
            Visibility::Hidden,
        ))
        .with_child((
            ImageNode::new(icon),
            Node {
                position_type: PositionType::Absolute,
                left: percent(18.0),
                top: percent(18.0),
                width: percent(64.0),
                height: percent(64.0),
                ..default()
            },
        ))
        .id()
}

fn spawn_attack_button(
    commands: &mut Commands,
    font: &Handle<Font>,
    crosshair: Handle<Image>,
) -> Entity {
    let hint = hint_chip(commands, font, 0);
    commands
        .spawn((
            ControlPart::AttackButton,
            Name::new("HUD attack button"),
            circle_node(),
            BackgroundColor(palette::PLATE),
            BorderColor::all(ui::theme::INK),
            RingFill::default(),
        ))
        .with_child((
            ControlPart::AttackIcon,
            ImageNode::new(crosshair),
            Node {
                position_type: PositionType::Absolute,
                left: percent(22.0),
                top: percent(22.0),
                width: percent(56.0),
                height: percent(56.0),
                ..default()
            },
        ))
        .add_child(hint)
        .id()
}

fn spawn_ultimate_button(
    commands: &mut Commands,
    font: &Handle<Font>,
    badge: Handle<Image>,
) -> Entity {
    let hint = hint_chip(commands, font, 1);
    commands
        .spawn((
            ControlPart::UltimateButton,
            Name::new("HUD ultimate button"),
            circle_node(),
            BackgroundColor(palette::PLATE),
            BorderColor::all(ui::theme::INK),
            RingFill::default(),
            UiTransform::IDENTITY,
        ))
        .with_children(|button| {
            button
                .spawn((
                    ControlPart::UltimateCore,
                    inset_node(RING_INSET_PERCENT),
                    BackgroundColor(ui::theme::PANEL),
                ))
                .with_children(|core| {
                    core.spawn((
                        ControlPart::UltimateIcon,
                        ImageNode::new(badge),
                        Node {
                            width: percent(74.0),
                            height: percent(62.0),
                            ..default()
                        },
                    ));
                    core.spawn((
                        ControlPart::UltimateTimer,
                        Text::new(""),
                        ui::theme::hud_text(font, 26.0),
                        TextColor(ui::theme::TEXT),
                        TextLayout::new(Justify::Center, LineBreak::NoWrap),
                        Node {
                            position_type: PositionType::Absolute,
                            ..default()
                        },
                        Visibility::Hidden,
                    ));
                });
        })
        .add_child(hint)
        .id()
}

/// The controlled fighter's state the buttons present.
type ControlledFighterQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ResolvedMatchLoadout,
        &'static WeaponState,
        Option<&'static AbilityState>,
        Option<&'static AuthoritativeTick>,
        Option<&'static Defeated>,
    ),
    (With<Fighter>, With<lightyear::prelude::Controlled>),
>;

fn set_circle(node: &mut Node, circle: ScreenCircle, scale: f32) {
    let left = px((circle.center.x - circle.radius) / scale);
    let top = px((circle.center.y - circle.radius) / scale);
    let size = px(circle.radius * 2.0 / scale);
    if node.left != left || node.top != top || node.width != size || node.height != size {
        node.left = left;
        node.top = top;
        node.width = size;
        node.height = size;
    }
}

fn set_rect(node: &mut Node, rect: Rect, scale: f32) {
    let left = px(rect.min.x / scale);
    let top = px(rect.min.y / scale);
    let width = px(rect.width() / scale);
    let height = px(rect.height() / scale);
    if node.left != left || node.top != top || node.width != width || node.height != height {
        node.left = left;
        node.top = top;
        node.width = width;
        node.height = height;
    }
}

fn shown(visible: bool) -> Visibility {
    if visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}

/// Places the buttons and touch sticks, and shows the group only with a controlled fighter.
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn place_controls(
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
    controls: Res<TouchControls>,
    context: Res<ClientInputContext>,
    fighters: ControlledFighterQuery,
    mut parts: Query<(&ControlPart, &mut Node, &mut Visibility)>,
) {
    let Some(viewport) = windows
        .iter()
        .next()
        .map(|window| Vec2::new(window.width(), window.height()))
    else {
        return;
    };
    let scale = scale.map_or(1.0, |scale| scale.0).max(0.01);
    let layout = HudControlLayout::new(viewport);
    let active = fighters.iter().next().is_some() && !matches!(*context, ClientInputContext::Shell);
    let touch = touch_mode(&controls);
    let travel = controls.layout.map(|layout| layout.travel);
    let stick_circle = |part: StickPart| {
        let travel = travel?;
        let touch_layout = controls.layout?;
        let base = |stick: Option<crate::touch::TouchStick>| {
            stick.map(|stick| ScreenCircle {
                center: stick.base,
                radius: travel,
            })
        };
        let knob = |stick: Option<crate::touch::TouchStick>| {
            stick.map(|stick| ScreenCircle {
                center: stick.base + stick.deflection(&touch_layout) * travel,
                radius: travel * KNOB_FRACTION,
            })
        };
        match part {
            StickPart::MoveBase => base(controls.movement),
            StickPart::MoveKnob => knob(controls.movement),
            StickPart::AttackBase => base(controls.attack),
            StickPart::AttackKnob => knob(controls.attack),
            StickPart::UltimateKnob => knob(controls.ultimate),
        }
    };
    for (part, mut node, mut visibility) in &mut parts {
        let circle = match *part {
            ControlPart::Group => {
                visibility.set_if_neq(shown(active));
                continue;
            }
            ControlPart::MenuButton => {
                visibility.set_if_neq(shown(touch));
                set_rect(&mut node, layout.menu, scale);
                continue;
            }
            ControlPart::AttackButton => Some(layout.attack),
            ControlPart::UltimateButton => Some(layout.ultimate),
            ControlPart::MoveHome => {
                (touch && controls.movement.is_none()).then_some(ScreenCircle {
                    center: layout.move_home,
                    radius: travel.unwrap_or(layout.ultimate.radius),
                })
            }
            ControlPart::Stick(stick) => stick_circle(stick).filter(|_| touch),
            _ => continue,
        };
        match circle {
            Some(circle) => {
                visibility.set_if_neq(Visibility::Inherited);
                set_circle(&mut node, circle, scale);
            }
            None => {
                visibility.set_if_neq(Visibility::Hidden);
            }
        }
    }
}

fn ring_gradient(fill: f32, color: Color, track: Color) -> BackgroundGradient {
    let angle = fill.clamp(0.0, 1.0) * TAU;
    BackgroundGradient::from(ConicGradient::new(
        UiPosition::CENTER,
        vec![
            AngularColorStop::new(color, 0.0),
            AngularColorStop::new(color, angle),
            AngularColorStop::new(track, angle),
            AngularColorStop::new(track, TAU),
        ],
    ))
}

fn write_ring(
    commands: &mut Commands,
    entity: Entity,
    ring: &mut RingFill,
    fill: Option<f32>,
    color: Color,
) {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the fill is clamped to 0..=1 before quantizing"
    )]
    let key = fill.map(|fill| {
        (
            (fill.clamp(0.0, 1.0) * RING_STEPS).round() as u16,
            color.to_srgba().to_u8_array(),
        )
    });
    if ring.0 == key {
        return;
    }
    ring.0 = key;
    match fill {
        Some(fill) => {
            commands
                .entity(entity)
                .insert(ring_gradient(fill, color, palette::PLATE));
        }
        None => {
            commands.entity(entity).remove::<BackgroundGradient>();
        }
    }
}

/// What the buttons present for the controlled fighter this frame.
struct ButtonFacts {
    reload: Option<f32>,
    face: Option<UltimateFace>,
    defeated: bool,
    kind: brawler::builds::UltimateKind,
}

fn button_facts(
    fighters: &ControlledFighterQuery,
    pending: &PendingLocalActions,
) -> Option<ButtonFacts> {
    let (loadout, weapon, ability, tick, defeated) = fighters.iter().next()?;
    let now = tick.map(|tick| tick.0);
    Some(ButtonFacts {
        reload: attack_reload(weapon, now),
        face: ability.map(|ability| {
            ultimate_face(
                *ability,
                loadout.ultimate.charge_policy.maximum,
                now,
                pending.targeted_ultimate.is_targeting(loadout.ultimate.id),
            )
        }),
        defeated: defeated.is_some(),
        kind: loadout.ultimate.kind,
    })
}

/// Button frames: reload ring, ultimate charge ring, Ready rim and pulse, drag highlight.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn present_button_rings(
    mut commands: Commands,
    fighters: ControlledFighterQuery,
    pending: Res<PendingLocalActions>,
    controls: Res<TouchControls>,
    shell: Option<Res<ClientShellSettings>>,
    time: Res<Time>,
    mut buttons: Query<(
        Entity,
        &ControlPart,
        &mut RingFill,
        &mut BorderColor,
        Option<&mut UiTransform>,
    )>,
) {
    let Some(facts) = button_facts(&fighters, &pending) else {
        return;
    };
    let reduced_motion = shell.is_some_and(|shell| shell.reduced_motion);
    for (entity, part, mut ring, mut border, transform) in &mut buttons {
        match part {
            ControlPart::AttackButton => {
                let color = ui::theme::ACCENT.with_alpha(0.55);
                write_ring(&mut commands, entity, &mut ring, facts.reload, color);
                border.set_if_neq(BorderColor::all(if controls.attack.is_some() {
                    ui::theme::ACCENT
                } else {
                    ui::theme::INK
                }));
            }
            ControlPart::UltimateButton => {
                let (fill, color) = facts.face.map_or((0.0, palette::ULTIMATE), |face| {
                    let color = match face.look {
                        UltimateLook::Charging => palette::ULTIMATE,
                        UltimateLook::Ready => ui::theme::PLAY_HIGHLIGHT,
                        UltimateLook::Targeting | UltimateLook::Active(_) => ui::theme::ACCENT,
                    };
                    (face.fill, color)
                });
                write_ring(&mut commands, entity, &mut ring, Some(fill), color);
                let ready = facts
                    .face
                    .is_some_and(|face| face.look == UltimateLook::Ready);
                border.set_if_neq(BorderColor::all(if ready || controls.ultimate.is_some() {
                    ui::theme::PLAY_HIGHLIGHT
                } else {
                    ui::theme::INK
                }));
                if let Some(mut transform) = transform {
                    let pulse = if ready && !reduced_motion && !facts.defeated {
                        1.0 + READY_PULSE_SCALE
                            * (time.elapsed_secs() * READY_PULSE_HZ * TAU).sin().abs()
                    } else {
                        1.0
                    };
                    let next = UiTransform::from_scale(Vec2::splat(pulse));
                    if *transform != next {
                        *transform = next;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Button content: the family badge, icon dimming, and the active-ultimate seconds.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn present_button_content(
    fighters: ControlledFighterQuery,
    pending: Res<PendingLocalActions>,
    asset_server: Option<Res<AssetServer>>,
    mut icons: Query<(&ControlPart, &mut ImageNode)>,
    mut timers: Query<(&ControlPart, &mut Text, &mut Visibility)>,
    mut ultimate_icon: Local<Option<brawler::builds::UltimateKind>>,
) {
    let Some(facts) = button_facts(&fighters, &pending) else {
        return;
    };
    let swap_icon = *ultimate_icon != Some(facts.kind);
    *ultimate_icon = Some(facts.kind);
    let alpha = if facts.defeated { DEFEATED_ALPHA } else { 1.0 };
    for (part, mut image) in &mut icons {
        let icon_alpha = match part {
            ControlPart::AttackIcon if facts.reload.is_some() => alpha.min(0.55),
            ControlPart::AttackIcon | ControlPart::UltimateIcon => alpha,
            _ => continue,
        };
        if *part == ControlPart::UltimateIcon
            && swap_icon
            && let Some(server) = asset_server.as_deref()
        {
            image.image = server.load(ui::icons::path(ui::icons::ultimate_icon(facts.kind)));
        }
        let color = Color::WHITE.with_alpha(icon_alpha);
        if image.color != color {
            image.color = color;
        }
    }
    let seconds = facts.face.and_then(|face| match face.look {
        UltimateLook::Active(seconds) => seconds,
        _ => None,
    });
    for (part, mut text, mut visibility) in &mut timers {
        if *part != ControlPart::UltimateTimer {
            continue;
        }
        let label = seconds.map_or_else(String::new, |seconds| seconds.to_string());
        if text.0 != label {
            text.0 = label;
        }
        visibility.set_if_neq(shown(seconds.is_some()));
    }
}

/// Key/controller hints on desktop; hidden for touch play.
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn present_hints(
    settings: Res<ClientInputSettings>,
    pending: Res<PendingLocalActions>,
    controls: Res<TouchControls>,
    mut parts: Query<(&ControlPart, Option<&mut Text>, Option<&mut Visibility>)>,
) {
    let visible = !touch_mode(&controls);
    let hints = control_hints(&settings, pending.active_device);
    for (part, text, visibility) in &mut parts {
        match *part {
            ControlPart::Hint(_) => {
                if let Some(mut visibility) = visibility {
                    visibility.set_if_neq(shown(visible));
                }
            }
            ControlPart::HintText(index) => {
                if let (Some(mut text), Some(label)) = (text, hints.get(index))
                    && text.0 != *label
                {
                    text.0.clone_from(label);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brawler::combat::{AmmoRecovery, WeaponPhase};

    #[test]
    fn ultimate_face_fills_with_charge_and_distinguishes_states() {
        let charging = AbilityState {
            charge: 250,
            phase: AbilityPhase::Charging,
        };
        let face = ultimate_face(charging, 1000, Some(0), false);
        assert!((face.fill - 0.25).abs() < 1e-6);
        assert_eq!(face.look, UltimateLook::Charging);
        let ready = AbilityState {
            charge: 1000,
            phase: AbilityPhase::Ready,
        };
        assert_eq!(
            ultimate_face(ready, 1000, None, false).look,
            UltimateLook::Ready
        );
        assert_eq!(
            ultimate_face(ready, 1000, None, true).look,
            UltimateLook::Targeting
        );
        let cloaked = AbilityState {
            charge: 0,
            phase: AbilityPhase::Cloaked {
                generation: 1,
                activated_at_tick: 0,
                expires_at_tick: 300,
            },
        };
        let face = ultimate_face(cloaked, 1000, Some(121), false);
        assert_eq!(face.look, UltimateLook::Active(Some(3)));
        assert!((face.fill - 1.0).abs() < f32::EPSILON);
        // Overfull or zero-maximum charge stays bounded.
        assert!(
            (ultimate_face(
                AbilityState {
                    charge: 5,
                    phase: AbilityPhase::Charging
                },
                0,
                None,
                false
            )
            .fill
                - 1.0)
                .abs()
                < f32::EPSILON
        );
    }

    #[test]
    fn attack_reload_shows_only_while_empty() {
        let mut weapon = WeaponState {
            ammo: 2,
            phase: WeaponPhase::Ready,
            ammo_recovery: Some(AmmoRecovery {
                started_at_tick: 100,
                ready_at_tick: 160,
            }),
        };
        assert_eq!(attack_reload(&weapon, Some(130)), None);
        weapon.ammo = 0;
        assert!((attack_reload(&weapon, Some(130)).unwrap() - 0.5).abs() < 1e-6);
        assert!((attack_reload(&weapon, Some(500)).unwrap() - 1.0).abs() < 1e-6);
        assert_eq!(attack_reload(&weapon, None), Some(0.0));
    }

    #[test]
    fn hints_follow_bindings_and_the_active_device() {
        let mut settings = ClientInputSettings::default();
        assert_eq!(
            control_hints(&settings, ActiveInputDevice::KeyboardMouse),
            ["LMB".to_string(), "E".to_string()]
        );
        settings.keyboard.ultimate = KeyCode::KeyR;
        settings.mouse_primary = MouseButton::Right;
        assert_eq!(
            control_hints(&settings, ActiveInputDevice::KeyboardMouse),
            ["RMB".to_string(), "R".to_string()]
        );
        assert_eq!(
            control_hints(&settings, ActiveInputDevice::Gamepad(Entity::PLACEHOLDER)),
            ["RT".to_string(), "RB".to_string()]
        );
        for label in control_hints(&settings, ActiveInputDevice::KeyboardMouse) {
            assert!(label.chars().all(ui::theme::display_font_has_glyph));
        }
    }
}
