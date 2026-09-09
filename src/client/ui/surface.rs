//! Shared shell chrome. Owners supply semantic role and state; geometry stays here.
use super::theme::{CREAM, INK, ORANGE, TEAL};
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
    Quiet,
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
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct SurfaceMaterial {
    #[uniform(0)]
    fill: Vec4,
    #[uniform(1)]
    ink: Vec4,
    #[uniform(2)]
    accent: Vec4,
    #[uniform(3)]
    geometry: Vec4,
    #[uniform(4)]
    flags: Vec4,
}
impl UiMaterial for SurfaceMaterial {
    fn fragment_shader() -> ShaderRef {
        "brawler/shaders/shell_surface.wgsl".into()
    }
}
pub(crate) struct SurfacePlugin;
impl Plugin for SurfacePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<SurfaceMaterial>::default())
            .add_systems(PostUpdate, bind.before(UiSystems::Layout))
            .add_systems(PostUpdate, present.after(UiSystems::Layout));
    }
}
fn material() -> SurfaceMaterial {
    SurfaceMaterial {
        fill: CREAM.to_linear().to_vec4(),
        ink: INK.to_linear().to_vec4(),
        accent: ORANGE.to_linear().to_vec4(),
        geometry: Vec4::ONE,
        flags: Vec4::ZERO,
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
    mut surfaces: Query<
        (Entity, &Surface, &mut Node, Option<&Children>),
        Without<MaterialNode<SurfaceMaterial>>,
    >,
    mut texts: Query<(&Text, &mut TextFont, Option<&mut TextColor>)>,
    old_markers: Query<(), With<super::controls::FocusMarker>>,
) {
    for (entity, role, mut node, children) in &mut surfaces {
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
                if let Ok((_, mut font, color)) = texts.get_mut(child) {
                    font.font = assets.load(super::theme::FONT_PATH).into();
                    if let Some(mut color) = color
                        && color.0 == Color::WHITE
                    {
                        color.0 = INK;
                    }
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
            ));
        if *role == Surface::Back {
            let label = children
                .and_then(|children| children.iter().find_map(|child| texts.get(child).ok()))
                .map_or("BACK".to_owned(), |(text, _, _)| {
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
                    TextColor(INK),
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
/// The single color/interaction contract for every shell surface.
pub(crate) fn fill(role: Surface, state: SurfaceState, interaction: Interaction) -> Color {
    let base = if state.selected {
        TEAL
    } else {
        match role {
            Surface::Back | Surface::Primary => ORANGE,
            Surface::Secondary => TEAL,
            _ => CREAM,
        }
    };
    if state.disabled {
        return base.mix(&CREAM, 0.65);
    }
    if state.inactive {
        return base;
    }
    match interaction {
        Interaction::Pressed => base.mix(&INK, 0.12),
        Interaction::Hovered => base.mix(&Color::WHITE, 0.12),
        Interaction::None => base,
    }
}
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy material presentation"
)]
fn present(
    mut materials: ResMut<Assets<SurfaceMaterial>>,
    surfaces: Query<(
        &Surface,
        Option<&SurfaceState>,
        &ComputedNode,
        &MaterialNode<SurfaceMaterial>,
        Option<&Interaction>,
        Has<InteractionDisabled>,
    )>,
) {
    for (role, state, node, handle, interaction, disabled) in &surfaces {
        let mut state = state.copied().unwrap_or_default();
        state.disabled |= disabled;
        let Some(mut mat) = materials.get_mut(&handle.0) else {
            continue;
        };
        mat.fill = fill(
            *role,
            state,
            interaction.copied().unwrap_or(Interaction::None),
        )
        .to_linear()
        .to_vec4();
        mat.ink = (if state.disabled {
            INK.mix(&CREAM, 0.55)
        } else {
            INK
        })
        .to_linear()
        .to_vec4();
        let size = node.size() * node.inverse_scale_factor();
        mat.geometry = Vec4::new(
            size.x,
            size.y,
            match role {
                Surface::Row | Surface::Field => ROW_SLOPE,
                Surface::Card | Surface::Panel => PANEL_SLOPE,
                _ => ACTION_SLOPE,
            },
            BORDER_WIDTH,
        );
        mat.flags = Vec4::new(
            f32::from(state.focused && !state.disabled && !state.inactive),
            f32::from(*role == Surface::Quiet),
            0.,
            0.,
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_focus_keeps_choice_and_inactive_ignores_hover() {
        let state = SurfaceState {
            selected: true,
            focused: true,
            ..default()
        };
        assert_eq!(fill(Surface::Card, state, Interaction::None), TEAL);
        assert_eq!(
            fill(
                Surface::Card,
                SurfaceState {
                    inactive: true,
                    ..state
                },
                Interaction::Pressed
            ),
            TEAL
        );
        assert_eq!(
            fill(Surface::Back, SurfaceState::default(), Interaction::None),
            ORANGE
        );
    }
}
