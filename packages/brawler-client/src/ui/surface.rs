//! Shared shell chrome. Owners supply semantic role and state; geometry, fills, and the
//! presentation-only press, idle, and entrance motion stay here.
use super::theme::{ACCENT, INK, PANEL, PANEL_RAISED, PLAY, PLAY_HIGHLIGHT, TEXT};
use bevy::{
    prelude::*,
    reflect::TypePath,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    ui::{InteractionDisabled, UiSystems},
};

const ACTION_SLOPE: f32 = 0.16;
const PANEL_SLOPE: f32 = 0.055;
const ROW_SLOPE: f32 = 0.04;
const BORDER_WIDTH: f32 = 5.0;
const BACK_LABEL_SIZE: f32 = 22.0;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Surface {
    Back,
    Primary,
    Secondary,
    #[default]
    Action,
    Card,
    Panel,
    Row,
    Field,
    Dark,
    Quiet,
    Danger,
}
#[derive(Component, Clone, Copy, Debug, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Independent selection, focus, ownership and availability axes must compose"
)]
pub(crate) struct SurfaceState {
    pub focused: bool,
    pub selected: bool,
    pub inactive: bool,
    pub disabled: bool,
}

/// Optional per-component geometry for player-shell surfaces whose approved
/// silhouette differs from the shared default. Colors and interaction remain
/// owned by `Surface` and `SurfaceState`.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct SurfaceStyle {
    slope: f32,
    border_width: f32,
    cut: f32,
    shadow_x: f32,
    shadow_y: f32,
}

impl SurfaceStyle {
    pub(crate) const MAP_TOOL: Self = Self {
        slope: 0.07,
        border_width: 5.0,
        cut: 9.0,
        shadow_x: 0.0,
        shadow_y: 8.0,
    };
    pub(crate) const MAP_ACTION: Self = Self {
        slope: 0.11,
        border_width: 4.0,
        cut: 7.0,
        shadow_x: 0.0,
        shadow_y: 6.0,
    };
    pub(crate) const MAP_ASSET: Self = Self {
        slope: 0.025,
        border_width: 3.0,
        cut: 7.0,
        shadow_x: 0.0,
        shadow_y: 5.0,
    };
    pub(crate) const MAP_TAB: Self = Self {
        slope: 0.045,
        border_width: 3.0,
        cut: 6.0,
        shadow_x: 0.0,
        shadow_y: 4.0,
    };
}
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct SurfaceMaterial {
    /// Gradient bottom color.
    #[uniform(0)]
    fill: Vec4,
    #[uniform(1)]
    ink: Vec4,
    /// Navigation focus color: frame and pointer, or the review affordance's frame, brackets or
    /// face highlight.
    #[uniform(2)]
    accent: Vec4,
    #[uniform(3)]
    geometry: Vec4,
    #[uniform(4)]
    flags: Vec4,
    #[uniform(5)]
    details: Vec4,
    /// Gradient top color.
    #[uniform(6)]
    fill_top: Vec4,
    /// Persistent selection rim.
    #[uniform(7)]
    selection: Vec4,
    /// x: press amount (lip collapse), y: shine sweep position or negative when idle, z: burst
    /// flash, w: focus pulse.
    #[uniform(8)]
    motion: Vec4,
}
impl UiMaterial for SurfaceMaterial {
    fn fragment_shader() -> ShaderRef {
        "brawler/shaders/shell_surface.wgsl".into()
    }
}

/// Focus/selection affordance under owner review (BRL-fe7t phase 1). The player shell keeps
/// `Current`; automation builds select another with `BRAWLER_SURFACE_AFFORDANCE` (`frame`,
/// `reticle` or `spotlight`) so the UI gallery and native shell captures can compare them.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(
    not(feature = "automation"),
    allow(
        dead_code,
        reason = "review variants are selected only by automation builds"
    )
)]
pub(crate) enum SurfaceAffordance {
    /// Cyan focus frame with pointer; yellow selection rim.
    #[default]
    Current,
    /// White outline with inner glow and a small lift; gold selection trim.
    Frame,
    /// White corner brackets that breathe; gold selection bar along the bottom.
    Reticle,
    /// The focused face lights up under a white bevel; gold selection outline.
    Spotlight,
}

impl SurfaceAffordance {
    #[cfg(feature = "automation")]
    pub(crate) const ALL: [Self; 4] = [Self::Current, Self::Frame, Self::Reticle, Self::Spotlight];

    #[cfg(feature = "automation")]
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Frame => "frame",
            Self::Reticle => "reticle",
            Self::Spotlight => "spotlight",
        }
    }

    #[cfg(feature = "automation")]
    fn from_env() -> Self {
        let value = std::env::var("BRAWLER_SURFACE_AFFORDANCE").unwrap_or_default();
        Self::ALL
            .into_iter()
            .find(|variant| variant.name() == value)
            .unwrap_or_default()
    }

    /// Shader selector (`flags.w`).
    fn index(self) -> f32 {
        match self {
            Self::Current => 0.0,
            Self::Frame => 1.0,
            Self::Reticle => 2.0,
            Self::Spotlight => 3.0,
        }
    }

    /// Whether a focused surface lifts above its neighbours.
    fn lifts(self) -> bool {
        self == Self::Frame
    }

    /// The focus color the shader receives for `role`.
    fn focus_color(self, role: Surface) -> Color {
        match self {
            Self::Frame | Self::Reticle => TEXT,
            Self::Spotlight if role == Surface::Primary => PLAY_HIGHLIGHT,
            Self::Current | Self::Spotlight => ACCENT,
        }
    }
}

/// Gallery review: present this surface as if reduced motion were on, beside surfaces that are
/// not, so one capture can compare both.
#[derive(Component, Clone, Copy, Debug, Default)]
#[cfg_attr(
    not(feature = "automation"),
    allow(dead_code, reason = "constructed only by the automation UI gallery")
)]
pub(crate) struct ReducedMotionPreview;

/// Opt-in idle motion for the screen's single primary call to action (Dashboard Play).
#[derive(Component, Clone, Copy, Debug, Default)]
pub(crate) struct PrimaryIdleMotion;

/// Presentation-only motion policy. The client syncs `reduced` from reduced motion/effects and
/// `motifs` from the render quality tier (BRL-0161: Low drops the background motif layer).
#[derive(Resource, Clone, Copy, Debug)]
pub(crate) struct SurfaceMotionPolicy {
    pub reduced: bool,
    pub motifs: bool,
}

impl Default for SurfaceMotionPolicy {
    fn default() -> Self {
        Self {
            reduced: false,
            motifs: true,
        }
    }
}

/// One short celebratory burst on the element whose state just changed (BRL-51zd): a scale pop,
/// one shine sweep and a brief face flash on surfaces, and the scale pop alone on other nodes.
/// The owner inserts it once; it is inert after it finishes and never under reduced motion.
#[derive(Component, Clone, Copy, Debug, Default)]
pub(crate) struct SurfaceBurst {
    elapsed: f32,
}

/// One frame of a running burst.
#[derive(Clone, Copy, Debug, PartialEq)]
struct BurstFrame {
    scale: f32,
    shine: Option<f32>,
    flash: f32,
}

impl SurfaceBurst {
    /// Gallery reference: the burst at its scale peak when re-asserted every frame.
    #[cfg(feature = "automation")]
    pub(crate) const AT_PEAK: Self = Self {
        elapsed: BURST_SECONDS * BURST_RISE,
    };

    /// Advances the burst; `None` once it has finished or when motion is reduced.
    fn advance(&mut self, dt: f32, reduced: bool) -> Option<BurstFrame> {
        if reduced {
            self.elapsed = BURST_SECONDS;
        }
        if self.elapsed >= BURST_SECONDS {
            return None;
        }
        let t = self.elapsed / BURST_SECONDS;
        self.elapsed += dt;
        Some(burst_frame(t))
    }
}

/// Burst shape at normalized time `t` in `0..1`: a quick rise to the pop, then an ease-out-back
/// settle; the shine crosses the face once and the flash decays.
fn burst_frame(t: f32) -> BurstFrame {
    let t = t.clamp(0.0, 1.0);
    let scale = if t < BURST_RISE {
        1.0 + BURST_POP * (1.0 - (1.0 - t / BURST_RISE).powi(2))
    } else {
        1.0 + BURST_POP * (1.0 - ease_out_back((t - BURST_RISE) / (1.0 - BURST_RISE)))
    };
    let sweep = t / BURST_SHINE_FRACTION;
    BurstFrame {
        scale,
        shine: (sweep < 1.0).then_some(sweep),
        flash: BURST_FLASH * (1.0 - t).powi(2),
    }
}

/// When the active screen or overlay last changed. Surfaces created shortly afterwards play the
/// entrance; ordinary rebuilds of an unchanged screen do not.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub(crate) struct SurfaceEntrance {
    opened_at: f32,
    open: bool,
}

impl SurfaceEntrance {
    pub(crate) fn begin(&mut self, now: f32) {
        self.opened_at = now;
    }
}

const PRESS_IN_SECONDS: f32 = 0.06;
const RELEASE_SECONDS: f32 = 0.12;
const ENTRANCE_SECONDS: f32 = 0.2;
const ENTRANCE_WINDOW_SECONDS: f32 = 0.25;
const ENTRANCE_SCALE: f32 = 0.92;
const PRESS_SCALE: f32 = 0.95;
const PRESS_DROP: f32 = 3.0;
const BREATH_AMPLITUDE: f32 = 0.02;
const BREATH_PERIOD_SECONDS: f32 = 1.6;
const SHINE_PERIOD_SECONDS: f32 = 4.0;
const SHINE_SWEEP_SECONDS: f32 = 0.7;
/// Reward/confirmation burst: about 420 ms, a 7% pop reached at 25%, one shine over the first
/// 60%, and a face flash that starts at 30% toward white (never above 1.0, so nothing blooms).
const BURST_SECONDS: f32 = 0.42;
const BURST_RISE: f32 = 0.25;
const BURST_POP: f32 = 0.07;
const BURST_SHINE_FRACTION: f32 = 0.6;
const BURST_FLASH: f32 = 0.3;
const DEFAULT_DETAILS: Vec4 = Vec4::new(8.0, 0.0, 6.0, 0.0);
/// Review affordances: focus lift scale, its ease time, and the focus pulse period.
const FOCUS_LIFT: f32 = 0.035;
const FOCUS_LIFT_SECONDS: f32 = 0.09;
const FOCUS_PULSE_SECONDS: f32 = 1.2;

/// Per-surface presentation motion. Never read by input, navigation, or networking.
#[derive(Component, Clone, Copy, Debug, Default)]
pub(crate) struct SurfaceMotion {
    press: f32,
    release: Option<(f32, f32)>,
    entrance: Option<f32>,
    /// Lip collapse fed to the shader, 0 at rest and 1 fully pressed.
    lip: f32,
    shine: Option<f32>,
    /// Burst face flash fed to the shader, 0 at rest.
    flash: f32,
    /// Focus lift, 0 at rest and 1 fully lifted.
    lift: f32,
}

fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

impl SurfaceMotion {
    /// Eases the focus lift toward `target` and returns its scale factor; immediate and flat
    /// under reduced motion.
    fn ease_lift(&mut self, dt: f32, target: bool, reduced: bool) -> f32 {
        let goal = f32::from(u8::from(target && !reduced));
        let step = dt / FOCUS_LIFT_SECONDS;
        self.lift = if reduced {
            goal
        } else if self.lift < goal {
            (self.lift + step).min(goal)
        } else {
            (self.lift - step).max(goal)
        };
        1.0 + FOCUS_LIFT * ease_out_back(self.lift)
    }

    fn entering() -> Self {
        Self {
            entrance: Some(0.0),
            ..default()
        }
    }

    /// Advances one frame and returns the `(scale, downward offset)` to present.
    fn step(
        &mut self,
        dt: f32,
        now: f32,
        held: bool,
        idle: bool,
        reduced: bool,
        burst: Option<BurstFrame>,
    ) -> (f32, f32) {
        if reduced {
            *self = Self {
                lip: f32::from(u8::from(held)),
                ..default()
            };
            return (1.0, 0.0);
        }
        let mut press_visual = 0.0;
        if held {
            self.release = None;
            self.press = (self.press + dt / PRESS_IN_SECONDS).min(1.0);
            press_visual = self.press;
        } else if self.press > 0.0 && self.release.is_none() {
            self.release = Some((0.0, self.press));
            self.press = 0.0;
        }
        if let Some((elapsed, from)) = self.release {
            let elapsed = elapsed + dt;
            press_visual = from * (1.0 - ease_out_back(elapsed / RELEASE_SECONDS));
            self.release = (elapsed < RELEASE_SECONDS).then_some((elapsed, from));
        }
        let mut scale = 1.0 - (1.0 - PRESS_SCALE) * press_visual;
        if let Some(elapsed) = self.entrance {
            let elapsed = elapsed + dt;
            scale *=
                ENTRANCE_SCALE + (1.0 - ENTRANCE_SCALE) * ease_out_back(elapsed / ENTRANCE_SECONDS);
            self.entrance = (elapsed < ENTRANCE_SECONDS).then_some(elapsed);
        }
        self.shine = None;
        if idle && !held && self.release.is_none() {
            let phase = now * core::f32::consts::TAU / BREATH_PERIOD_SECONDS;
            scale *= 1.0 + BREATH_AMPLITUDE * phase.sin();
            let sweep = now.rem_euclid(SHINE_PERIOD_SECONDS) / SHINE_SWEEP_SECONDS;
            self.shine = (sweep < 1.0).then_some(sweep);
        }
        self.flash = 0.0;
        if let Some(burst) = burst {
            scale *= burst.scale;
            self.shine = burst.shine.or(self.shine);
            self.flash = burst.flash;
        }
        self.lip = press_visual.clamp(0.0, 1.0);
        (scale, PRESS_DROP * press_visual)
    }
}

pub(crate) struct SurfacePlugin;
impl Plugin for SurfacePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            UiMaterialPlugin::<SurfaceMaterial>::default(),
            super::motifs::ShellMotifPlugin,
        ))
        .init_resource::<SurfaceMotionPolicy>()
        .init_resource::<SurfaceEntrance>()
        .init_resource::<SurfaceAffordance>()
        .add_systems(
            PostUpdate,
            (
                refresh_entrance_window,
                bind,
                animate,
                animate_node_bursts,
                stroke_display_text,
            )
                .chain()
                .before(UiSystems::Layout),
        )
        .add_systems(PostUpdate, present.after(UiSystems::Layout));
        #[cfg(feature = "automation")]
        app.insert_resource(SurfaceAffordance::from_env());
    }
}
fn material() -> SurfaceMaterial {
    SurfaceMaterial {
        fill: PANEL.to_linear().to_vec4(),
        ink: INK.to_linear().to_vec4(),
        accent: ACCENT.to_linear().to_vec4(),
        geometry: Vec4::ONE,
        flags: Vec4::ZERO,
        details: DEFAULT_DETAILS,
        fill_top: PANEL.to_linear().to_vec4(),
        selection: PLAY_HIGHLIGHT.to_linear().to_vec4(),
        motion: Vec4::new(0.0, -1.0, 0.0, 0.0),
    }
}
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy component installation"
)]
fn bind(
    mut commands: Commands,
    mut materials: ResMut<Assets<SurfaceMaterial>>,
    assets: Res<AssetServer>,
    entrance: Res<SurfaceEntrance>,
    mut surfaces: Query<
        (
            Entity,
            &Surface,
            &mut Node,
            Option<&Children>,
            Has<UiTransform>,
        ),
        Without<MaterialNode<SurfaceMaterial>>,
    >,
    mut texts: Query<(&Text, &mut TextFont)>,
    old_markers: Query<(), With<super::controls::FocusMarker>>,
) {
    let entering = entrance.open;
    for (entity, role, mut node, children, has_transform) in &mut surfaces {
        node.border = UiRect::ZERO;
        node.border_radius = BorderRadius::ZERO;
        if matches!(role, Surface::Row | Surface::Field) {
            node.padding = UiRect {
                left: px(32),
                right: px(24),
                top: px(16),
                bottom: px(22),
            };
            node.align_items = AlignItems::Center;
        }
        if let Some(children) = children {
            for child in children.iter() {
                if old_markers.contains(child) {
                    commands.entity(child).despawn();
                    continue;
                }
                if let Ok((_, mut font)) = texts.get_mut(child) {
                    font.font = assets.load(super::theme::FONT_PATH).into();
                }
            }
        }
        let mut target = commands.entity(entity);
        target
            .remove::<(ImageNode, BackgroundColor, BorderColor, BoxShadow, Outline)>()
            .insert((
                MaterialNode(materials.add(material())),
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
                if entering {
                    SurfaceMotion::entering()
                } else {
                    SurfaceMotion::default()
                },
            ));
        // Owners that already transform a surface keep that transform; motion only adds to
        // surfaces it owns.
        if !has_transform {
            target.insert(UiTransform::IDENTITY);
        }
        if *role == Surface::Back {
            let label = children
                .and_then(|children| children.iter().find_map(|child| texts.get(child).ok()))
                .map_or("BACK".to_owned(), |(text, _)| {
                    text.0.trim_start_matches(['‹', '←', '<', ' ']).to_owned()
                });
            if let Some(children) = children {
                for child in children.iter() {
                    commands.entity(child).despawn();
                }
            }
            node.padding = UiRect::axes(px(24), px(8));
            // Screen sizing must not squeeze the shared navigation label out of its frame.
            let label_units = label.chars().fold(2., |width, _| width + 1.);
            node.min_width = px(label_units * BACK_LABEL_SIZE * 0.7 + 48.);
            node.flex_direction = FlexDirection::Row;
            node.align_items = AlignItems::Center;
            node.justify_content = JustifyContent::Center;
            commands.entity(entity).with_children(|parent| {
                parent.spawn((
                    Text::new(format!("‹ {label}")),
                    TextFont {
                        font: assets.load(super::theme::FONT_PATH).into(),
                        font_size: BACK_LABEL_SIZE.into(),
                        ..default()
                    },
                    TextColor(TEXT),
                    Node {
                        width: percent(100),
                        min_width: px(0),
                        ..default()
                    },
                    TextLayout::justify(Justify::Center),
                ));
            });
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn refresh_entrance_window(
    time: Res<Time>,
    policy: Res<SurfaceMotionPolicy>,
    mut entrance: ResMut<SurfaceEntrance>,
) {
    let open =
        !policy.reduced && time.elapsed_secs() - entrance.opened_at <= ENTRANCE_WINDOW_SECONDS;
    if entrance.open != open {
        entrance.open = open;
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy presentation-only motion"
)]
fn animate(
    time: Res<Time>,
    policy: Res<SurfaceMotionPolicy>,
    affordance: Option<Res<SurfaceAffordance>>,
    mut surfaces: Query<(
        Option<&SurfaceState>,
        Option<&Interaction>,
        (Has<InteractionDisabled>, Has<ReducedMotionPreview>),
        Has<PrimaryIdleMotion>,
        &mut SurfaceMotion,
        &mut UiTransform,
        Option<&mut SurfaceBurst>,
    )>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let lifts = affordance.is_some_and(|affordance| affordance.lifts());
    for (state, interaction, (disabled, preview), idle, mut motion, mut transform, burst) in
        &mut surfaces
    {
        let state = state.copied().unwrap_or_default();
        let reduced = policy.reduced || preview;
        let live = !disabled && !state.disabled && !state.inactive;
        let held = live && interaction == Some(&Interaction::Pressed);
        let burst = burst.and_then(|mut burst| burst.advance(dt, reduced));
        let (mut scale, drop) = motion.step(dt, now, held, idle && live, reduced, burst);
        if lifts {
            scale *= motion.ease_lift(dt, live && state.focused, reduced);
        }
        transform.set_if_neq(UiTransform {
            translation: Val2::new(px(0), px(drop)),
            scale: Vec2::splat(scale),
            ..*transform
        });
    }
}

/// Bursts on nodes without surface motion (for example a results header) only pop their scale.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy presentation-only motion"
)]
fn animate_node_bursts(
    time: Res<Time>,
    policy: Res<SurfaceMotionPolicy>,
    mut nodes: Query<(&mut SurfaceBurst, &mut UiTransform), Without<SurfaceMotion>>,
) {
    let dt = time.delta_secs();
    for (mut burst, mut transform) in &mut nodes {
        let Some(frame) = burst.advance(dt, policy.reduced).or_else(|| {
            // Settle exactly at rest on the frame after the burst ends.
            (transform.scale != Vec2::ONE).then_some(BurstFrame {
                scale: 1.0,
                shine: None,
                flash: 0.0,
            })
        }) else {
            continue;
        };
        transform.set_if_neq(UiTransform {
            scale: Vec2::splat(frame.scale),
            ..*transform
        });
    }
}

/// White display type gets the shared ink stroke wherever it appears in the shell.
#[allow(clippy::type_complexity, reason = "Bevy text presentation query")]
fn stroke_display_text(
    mut commands: Commands,
    texts: Query<(Entity, &TextColor, &TextFont), (Changed<TextColor>, Without<TextShadow>)>,
) {
    for (entity, color, font) in &texts {
        if color.0 == TEXT {
            let size = match font.font_size {
                FontSize::Px(size) => size,
                _ => 24.0,
            };
            commands
                .entity(entity)
                .insert(super::theme::type_stroke(size));
        }
    }
}

/// The single color/interaction contract for every shell surface: the gradient bottom color.
pub(crate) fn fill(role: Surface, state: SurfaceState, interaction: Interaction) -> Color {
    let base = if state.selected && role != Surface::Primary {
        PANEL_RAISED
    } else {
        match role {
            Surface::Primary => PLAY,
            Surface::Back | Surface::Secondary => PANEL_RAISED,
            Surface::Danger => DANGER,
            Surface::Dark => INK.mix(&PANEL, 0.35),
            _ => PANEL,
        }
    };
    if state.disabled {
        return base.mix(&DISABLED, 0.85);
    }
    if state.inactive {
        return base;
    }
    match interaction {
        Interaction::Pressed => base.mix(&INK, 0.15),
        Interaction::Hovered => base.mix(&TEXT, 0.12),
        Interaction::None => base,
    }
}

const DANGER: Color = Color::srgb(0.86, 0.2, 0.2);
const DISABLED: Color = Color::srgb(0.36, 0.4, 0.52);

/// The gradient top color: a lighter band of the same fill; the primary action warms toward
/// `play-highlight`.
fn fill_top(role: Surface, state: SurfaceState, bottom: Color) -> Color {
    if role == Surface::Primary && !state.disabled {
        bottom.mix(&PLAY_HIGHLIGHT, 0.75)
    } else {
        bottom.mix(&TEXT, 0.16)
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy material presentation"
)]
fn present(
    time: Res<Time>,
    policy: Res<SurfaceMotionPolicy>,
    affordance: Res<SurfaceAffordance>,
    mut materials: ResMut<Assets<SurfaceMaterial>>,
    surfaces: Query<(
        &Surface,
        Option<&SurfaceState>,
        &ComputedNode,
        &MaterialNode<SurfaceMaterial>,
        Option<&Interaction>,
        Option<&SurfaceStyle>,
        Option<&SurfaceMotion>,
        Has<InteractionDisabled>,
        Has<ReducedMotionPreview>,
    )>,
) {
    let phase = time.elapsed_secs() * core::f32::consts::TAU / FOCUS_PULSE_SECONDS;
    for (role, state, node, handle, interaction, style, motion, disabled, preview) in &surfaces {
        let mut state = state.copied().unwrap_or_default();
        state.disabled |= disabled;
        let Some(mut mat) = materials.get_mut(&handle.0) else {
            continue;
        };
        let bottom = fill(
            *role,
            state,
            interaction.copied().unwrap_or(Interaction::None),
        );
        mat.fill = bottom.to_linear().to_vec4();
        mat.fill_top = fill_top(*role, state, bottom).to_linear().to_vec4();
        mat.ink = (if state.disabled {
            INK.mix(&DISABLED, 0.5)
        } else {
            INK
        })
        .to_linear()
        .to_vec4();
        let size = node.size() * node.inverse_scale_factor();
        let default_slope = match role {
            Surface::Row | Surface::Field => ROW_SLOPE,
            Surface::Card | Surface::Panel => PANEL_SLOPE,
            _ => ACTION_SLOPE,
        };
        mat.geometry = Vec4::new(
            size.x,
            size.y,
            style.map_or(default_slope, |style| style.slope),
            style.map_or(BORDER_WIDTH, |style| style.border_width),
        );
        mat.accent = affordance.focus_color(*role).to_linear().to_vec4();
        mat.selection = PLAY_HIGHLIGHT.to_linear().to_vec4();
        mat.flags = Vec4::new(
            f32::from(state.focused && !state.disabled && !state.inactive),
            f32::from(*role == Surface::Quiet),
            f32::from(state.selected && !state.disabled),
            affordance.index(),
        );
        mat.details = style.map_or(DEFAULT_DETAILS, |style| {
            Vec4::new(style.cut, style.shadow_x, style.shadow_y, 0.0)
        });
        let motion = motion.copied().unwrap_or_default();
        let pulse = if policy.reduced || preview {
            1.0
        } else {
            0.5 + 0.5 * phase.sin()
        };
        mat.motion = Vec4::new(
            motion.lip,
            motion.shine.unwrap_or(-1.0),
            motion.flash,
            pulse,
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use core::time::Duration;

    #[test]
    fn selected_focus_keeps_choice_and_inactive_ignores_hover() {
        let state = SurfaceState {
            selected: true,
            focused: true,
            ..default()
        };
        assert_eq!(fill(Surface::Card, state, Interaction::None), PANEL_RAISED);
        assert_eq!(
            fill(
                Surface::Card,
                SurfaceState {
                    inactive: true,
                    ..state
                },
                Interaction::Pressed
            ),
            PANEL_RAISED
        );
        assert_eq!(
            fill(Surface::Primary, state, Interaction::None),
            PLAY,
            "the single primary action keeps its play fill when selected"
        );
        assert_eq!(
            fill(Surface::Back, SurfaceState::default(), Interaction::None),
            PANEL_RAISED
        );
        assert_eq!(
            fill(Surface::Action, SurfaceState::default(), Interaction::None),
            PANEL
        );
        assert_eq!(
            fill(Surface::Danger, SurfaceState::default(), Interaction::None),
            DANGER
        );
    }

    #[test]
    fn gradient_top_is_lighter_and_primary_warms_to_play_highlight() {
        let state = SurfaceState::default();
        let panel_top = fill_top(Surface::Action, state, PANEL).to_srgba();
        assert!(panel_top.blue > PANEL.to_srgba().blue);
        let play_top = fill_top(Surface::Primary, state, PLAY).to_srgba();
        assert!(play_top.green > PLAY.to_srgba().green);
    }

    fn motion_app(reduced: bool) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                10,
            )))
            .insert_resource(SurfaceMotionPolicy {
                reduced,
                ..default()
            })
            .add_systems(Update, animate);
        let entity = app
            .world_mut()
            .spawn((
                Surface::Primary,
                Interaction::None,
                SurfaceMotion::default(),
                UiTransform::IDENTITY,
            ))
            .id();
        app.update();
        (app, entity)
    }

    fn transform(app: &App, entity: Entity) -> UiTransform {
        *app.world().get::<UiTransform>(entity).unwrap()
    }

    #[test]
    fn press_squashes_and_drops_then_release_overshoots_back_to_rest() {
        let (mut app, entity) = motion_app(false);
        *app.world_mut().get_mut::<Interaction>(entity).unwrap() = Interaction::Pressed;
        for _ in 0..10 {
            app.update();
        }
        let pressed = transform(&app, entity);
        assert!((pressed.scale.x - PRESS_SCALE).abs() < 1e-4);
        assert_eq!(pressed.translation, Val2::new(px(0), px(PRESS_DROP)));
        assert!(app.world().get::<SurfaceMotion>(entity).unwrap().lip > 0.99);

        *app.world_mut().get_mut::<Interaction>(entity).unwrap() = Interaction::Hovered;
        let mut overshoot = false;
        for _ in 0..20 {
            app.update();
            overshoot |= transform(&app, entity).scale.x > 1.0;
        }
        assert!(overshoot, "release overshoots past rest (ease-out-back)");
        assert_eq!(transform(&app, entity).scale, Vec2::ONE);
        assert!(app.world().get::<SurfaceMotion>(entity).unwrap().lip.abs() < 1e-6);
    }

    #[test]
    fn reduced_motion_keeps_press_as_an_immediate_lip_change_without_scale() {
        let (mut app, entity) = motion_app(true);
        *app.world_mut().get_mut::<Interaction>(entity).unwrap() = Interaction::Pressed;
        app.update();
        assert_eq!(transform(&app, entity), UiTransform::IDENTITY);
        assert!((app.world().get::<SurfaceMotion>(entity).unwrap().lip - 1.0).abs() < 1e-6);
    }

    #[test]
    fn idle_breathing_and_shine_run_only_without_reduced_motion() {
        for reduced in [false, true] {
            let (mut app, entity) = motion_app(reduced);
            app.world_mut().entity_mut(entity).insert(PrimaryIdleMotion);
            let mut scales = Vec::new();
            let mut shone = false;
            // 4.5 s of fixed 10 ms frames covers several breaths and one shine sweep.
            for _ in 0..450 {
                app.update();
                scales.push(transform(&app, entity).scale.x);
                shone |= app
                    .world()
                    .get::<SurfaceMotion>(entity)
                    .unwrap()
                    .shine
                    .is_some();
            }
            let (min, max) = scales
                .iter()
                .fold((f32::MAX, f32::MIN), |(lo, hi), s| (lo.min(*s), hi.max(*s)));
            if reduced {
                assert_eq!((min, max), (1.0, 1.0));
                assert!(!shone);
            } else {
                assert!(max > 1.015 && min < 0.985, "breathing ±2%: {min}..{max}");
                assert!(max <= 1.0 + BREATH_AMPLITUDE + 1e-4);
                assert!(shone);
            }
        }
    }

    #[test]
    fn disabled_or_inactive_primary_does_not_breathe() {
        let (mut app, entity) = motion_app(false);
        app.world_mut().entity_mut(entity).insert((
            PrimaryIdleMotion,
            SurfaceState {
                inactive: true,
                ..default()
            },
        ));
        for _ in 0..100 {
            app.update();
            assert_eq!(transform(&app, entity).scale, Vec2::ONE);
        }
    }

    #[test]
    fn burst_pops_shines_flashes_then_settles_at_rest() {
        let (mut app, entity) = motion_app(false);
        app.world_mut()
            .entity_mut(entity)
            .insert(SurfaceBurst::default());
        let (mut peak, mut shone, mut flashed) = (1.0_f32, false, false);
        for _ in 0..60 {
            app.update();
            peak = peak.max(transform(&app, entity).scale.x);
            let motion = *app.world().get::<SurfaceMotion>(entity).unwrap();
            shone |= motion.shine.is_some();
            flashed |= motion.flash > 0.1;
            assert!(
                motion.flash <= BURST_FLASH + 1e-6,
                "flash stays below white"
            );
        }
        assert!(peak > 1.05 && peak <= 1.0 + BURST_POP + 1e-4, "pop {peak}");
        assert!(shone && flashed);
        assert_eq!(transform(&app, entity).scale, Vec2::ONE);
        let motion = *app.world().get::<SurfaceMotion>(entity).unwrap();
        assert!(motion.shine.is_none() && motion.flash.abs() < f32::EPSILON);
    }

    #[test]
    fn reduced_motion_suppresses_bursts_on_surfaces_and_plain_nodes() {
        let (mut app, entity) = motion_app(true);
        app.add_systems(Update, animate_node_bursts);
        let node = app
            .world_mut()
            .spawn((SurfaceBurst::default(), UiTransform::IDENTITY))
            .id();
        app.world_mut()
            .entity_mut(entity)
            .insert(SurfaceBurst::default());
        for _ in 0..60 {
            app.update();
            assert_eq!(transform(&app, entity), UiTransform::IDENTITY);
            assert_eq!(transform(&app, node), UiTransform::IDENTITY);
            let motion = *app.world().get::<SurfaceMotion>(entity).unwrap();
            assert!(motion.shine.is_none() && motion.flash.abs() < f32::EPSILON);
        }
    }

    #[test]
    fn plain_node_burst_pops_and_returns_to_identity() {
        let (mut app, _) = motion_app(false);
        app.add_systems(Update, animate_node_bursts);
        let node = app
            .world_mut()
            .spawn((SurfaceBurst::default(), UiTransform::IDENTITY))
            .id();
        let mut peak = 1.0_f32;
        for _ in 0..60 {
            app.update();
            peak = peak.max(transform(&app, node).scale.x);
        }
        assert!(peak > 1.05, "pop {peak}");
        assert_eq!(transform(&app, node), UiTransform::IDENTITY);
    }

    #[test]
    fn burst_shape_starts_and_ends_at_rest() {
        assert!((burst_frame(0.0).scale - 1.0).abs() < 1e-6);
        assert!((burst_frame(BURST_RISE).scale - (1.0 + BURST_POP)).abs() < 1e-6);
        assert!((burst_frame(1.0).scale - 1.0).abs() < 1e-6);
        assert!(burst_frame(1.0).shine.is_none());
    }

    #[test]
    fn frame_affordance_lifts_focus_except_under_reduced_motion_or_preview() {
        let lifted = |reduced: bool, preview: bool, affordance: SurfaceAffordance| {
            let (mut app, entity) = motion_app(reduced);
            app.insert_resource(affordance);
            let mut surface = app.world_mut().entity_mut(entity);
            surface.insert(SurfaceState {
                focused: true,
                ..default()
            });
            if preview {
                surface.insert(ReducedMotionPreview);
            }
            for _ in 0..30 {
                app.update();
            }
            transform(&app, entity).scale.x
        };
        assert!((lifted(false, false, SurfaceAffordance::Frame) - (1.0 + FOCUS_LIFT)).abs() < 1e-4);
        assert!((lifted(true, false, SurfaceAffordance::Frame) - 1.0).abs() < f32::EPSILON);
        assert!((lifted(false, true, SurfaceAffordance::Frame) - 1.0).abs() < f32::EPSILON);
        assert!((lifted(false, false, SurfaceAffordance::Current) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn entrance_scales_in_from_092_and_settles() {
        let mut motion = SurfaceMotion::entering();
        let (first, _) = motion.step(0.0, 0.0, false, false, false, None);
        assert!((first - ENTRANCE_SCALE).abs() < 1e-4);
        let mut last = first;
        for _ in 0..25 {
            (last, _) = motion.step(0.01, 0.0, false, false, false, None);
        }
        assert!((last - 1.0).abs() < 1e-6);
        assert!(motion.entrance.is_none());
    }
}
