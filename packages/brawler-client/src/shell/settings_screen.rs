//! Preference pages own values and saving; shared controls own their presentation.
use super::{
    ClientInputSettings, ClientOverlay, ClientSettingsPath, ClientShellSettings,
    InputSettingsDraft, InputSettingsSelection, MAX_UI_SCALE, MIN_UI_SCALE, NavigationDirty,
    OverlayRoot, PendingActions, ShellAction, ShellButton, ShellControlId, ShellLayer,
    ShellScrollArea, ShellSettingsDraft, despawn_overlays, save_settings,
};
use crate::presentation_3d::render_quality::RenderCapabilities;
use crate::settings::persistence::GraphicsQuality;
use crate::ui::{
    controls::{ControlState, ControlValue, spawn_control},
    slider::{SliderPointer, SliderTouch},
    theme,
};
use bevy::ui::RelativeCursorPosition;
use bevy::{input_focus::InputFocus, prelude::*};

#[derive(Resource, Clone, Default)]
pub(super) struct SettingsPage {
    pub(super) category: Category,
    save_error: String,
    pub(super) rebuild: bool,
    pub(super) defer_save: bool,
    pending_save: bool,
    undo: Option<ClientShellSettings>,
    display_change: Option<(bool, f32)>,
    layout: SettingsLayout,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum SettingsLayout {
    #[default]
    Columns,
    Compact,
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy responsive layout inputs"
)]
pub(super) fn responsive(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
    mut page: ResMut<SettingsPage>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let width = window.width() / scale.as_ref().map_or(1.0, |scale| scale.0);
    let layout = if width < 1200.0 {
        SettingsLayout::Compact
    } else {
        SettingsLayout::Columns
    };
    if page.layout != layout {
        page.layout = layout;
        page.rebuild = true;
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Category {
    #[default]
    Display,
    Audio,
    Accessibility,
    Controls,
}

impl Category {
    fn title(self) -> &'static str {
        match self {
            Self::Display => "DISPLAY",
            Self::Audio => "AUDIO",
            Self::Accessibility => "ACCESSIBILITY",
            Self::Controls => "CONTROLS",
        }
    }
    pub(super) fn control(self) -> (ShellControlId, ShellAction) {
        match self {
            Self::Display => (
                ShellControlId::DisplayCategory,
                ShellAction::DisplayCategory,
            ),
            Self::Audio => (ShellControlId::AudioCategory, ShellAction::AudioCategory),
            Self::Accessibility => (
                ShellControlId::AccessibilityCategory,
                ShellAction::AccessibilityCategory,
            ),
            Self::Controls => (
                ShellControlId::ControlsCategory,
                ShellAction::ControlsCategory,
            ),
        }
    }
}

#[derive(Component)]
pub(super) struct CategoryTab(Category);
#[derive(Component)]
pub(super) struct Preference;
#[derive(Component)]
pub(super) struct Details;
#[derive(Component)]
pub(super) struct SaveError;
#[derive(Component)]
pub(super) struct SaveErrorContainer;
#[derive(Component)]
pub(super) struct RetryPreference;

pub(super) fn spawn(commands: &mut Commands) {
    commands.queue(|world: &mut World| {
        let page = world.resource::<SettingsPage>().clone();
        let category = page.category;
        let input = world.resource::<super::controls::ControlsState>().clone();
        let font = TextFont {
            font: world
                .get_resource::<AssetServer>()
                .map(|assets| assets.load(theme::FONT_PATH))
                .unwrap_or_default()
                .into(),
            font_size: if page.layout == SettingsLayout::Compact {
                18.0
            } else {
                22.0
            }
            .into(),
            ..default()
        };
        let mut commands = world.commands();
        commands
            .spawn((
                OverlayRoot,
                GlobalZIndex(500),
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(24)),
                    row_gap: px(20),
                    ..default()
                },
                theme::shell_background(),
            ))
            .with_children(|root| {
                spawn_header(
                    root,
                    &font,
                    category == Category::Controls && input.child_page(),
                );
                if page.layout == SettingsLayout::Compact {
                    spawn_categories(root, &font, true);
                }
                spawn_columns(root, &font, &page, &input);
                root.spawn((
                    SaveError,
                    Node {
                        display: Display::None,
                        ..default()
                    },
                    Text::new(""),
                    font.clone(),
                    TextColor(theme::PLAY_HIGHLIGHT),
                ));
                root.spawn((
                    SaveErrorContainer,
                    Node {
                        width: px(180),
                        display: Display::None,
                        ..default()
                    },
                ))
                .with_children(|footer| {
                    let entity = control(
                        footer,
                        &font,
                        "Retry save",
                        ShellControlId::RetryPreferences,
                        ShellAction::RetryPreferences,
                        ControlValue::Action,
                    );
                    footer.commands().entity(entity).insert(RetryPreference);
                });
            });
    });
}

fn spawn_header(root: &mut ChildSpawnerCommands, font: &TextFont, child: bool) {
    root.spawn(Node {
        width: percent(100),
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|header| {
        header
            .spawn(Node {
                width: px(if child { 240.0 } else { 160.0 }),
                ..default()
            })
            .with_children(|back| {
                control(
                    back,
                    font,
                    if child { "BACK TO CONTROLS" } else { "BACK" },
                    ShellControlId::Cancel,
                    ShellAction::Back,
                    ControlValue::Tab,
                );
            });
        header.spawn((
            Text::new("SETTINGS"),
            TextFont {
                font_size: 44.0.into(),
                ..font.clone()
            },
            TextColor(theme::TEXT),
            Node {
                flex_grow: 1.0,
                margin: UiRect::right(px(if child { 240.0 } else { 160.0 })),
                ..default()
            },
            TextLayout::new(Justify::Center, LineBreak::WordBoundary),
        ));
    });
}

fn spawn_categories(parent: &mut ChildSpawnerCommands, font: &TextFont, compact: bool) {
    parent
        .spawn(Node {
            width: if compact { percent(100) } else { percent(21) },
            flex_shrink: 0.0,
            flex_direction: if compact {
                FlexDirection::Row
            } else {
                FlexDirection::Column
            },
            row_gap: px(16),
            column_gap: px(8),
            ..default()
        })
        .with_children(|tabs| {
            for tab in [
                Category::Display,
                Category::Audio,
                Category::Accessibility,
                Category::Controls,
            ] {
                let (id, action) = tab.control();
                let entity = control(tabs, font, tab.title(), id, action, ControlValue::Tab);
                tabs.commands().entity(entity).insert(CategoryTab(tab));
                if compact {
                    tabs.commands()
                        .entity(entity)
                        .entry::<Node>()
                        .and_modify(|mut node| {
                            node.width = px(0);
                            node.flex_grow = 1.0;
                            node.min_width = px(0);
                        });
                }
            }
        });
}

fn spawn_columns(
    root: &mut ChildSpawnerCommands,
    font: &TextFont,
    page: &SettingsPage,
    input: &super::controls::ControlsState,
) {
    let category = page.category;
    root.spawn(Node {
        width: percent(100),
        flex_grow: 1.0,
        min_height: px(0),
        column_gap: px(16),
        ..default()
    })
    .with_children(|columns| {
        if page.layout == SettingsLayout::Columns {
            spawn_categories(columns, font, false);
        }
        spawn_options(columns, font, page, input);
        columns
            .spawn((
                Node {
                    flex_grow: 1.0,
                    flex_basis: px(0),
                    min_width: px(0),
                    padding: UiRect::horizontal(px(20)),
                    border: UiRect::left(px(2)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(16),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                BorderColor::all(theme::INK),
            ))
            .insert((
                ShellScrollArea,
                ScrollPosition::default(),
                RelativeCursorPosition::default(),
            ))
            .with_children(|details| {
                details.spawn((Details, Text::new(""), font.clone(), TextColor(theme::TEXT)));
                if page.display_change.is_some() {
                    control(
                        details,
                        font,
                        "Keep display",
                        ShellControlId::KeepDisplay,
                        ShellAction::KeepDisplay,
                        ControlValue::Action,
                    );
                    control(
                        details,
                        font,
                        "Revert",
                        ShellControlId::RevertDisplay,
                        ShellAction::RevertDisplay,
                        ControlValue::Action,
                    );
                } else if category == Category::Controls {
                    super::controls::spawn_details(details, font, input);
                }
            });
    });
}

fn spawn_options(
    columns: &mut ChildSpawnerCommands,
    font: &TextFont,
    page: &SettingsPage,
    input: &super::controls::ControlsState,
) {
    let category = page.category;
    columns
        .spawn((
            ShellScrollArea,
            ScrollPosition::default(),
            RelativeCursorPosition::default(),
            Node {
                width: percent(if page.layout == SettingsLayout::Compact {
                    57.0
                } else {
                    47.0
                }),
                min_width: px(0),
                padding: UiRect::horizontal(px(20)),
                border: UiRect::left(px(2)),
                overflow: Overflow::scroll_y(),
                flex_direction: FlexDirection::Column,
                row_gap: px(18),
                ..default()
            },
            BorderColor::all(theme::INK),
        ))
        .with_children(|options| {
            options.spawn((
                Text::new(category.title()),
                TextFont {
                    font_size: 30.0.into(),
                    ..font.clone()
                },
                TextColor(theme::TEXT),
            ));
            if category == Category::Controls {
                super::controls::spawn_options(options, font, input);
            } else {
                for id in category_rows(category) {
                    let entity = control(
                        options,
                        font,
                        description(*id).0,
                        *id,
                        action(*id),
                        if *id == ShellControlId::GraphicsQuality {
                            ControlValue::Binding(String::new())
                        } else {
                            ControlValue::Toggle(false)
                        },
                    );
                    // Sliders need their native track children at creation time.
                    options.commands().entity(entity).insert(Preference);
                }
                control(
                    options,
                    font,
                    "Reset this category",
                    ShellControlId::ResetPreferences,
                    ShellAction::ResetPreferences,
                    ControlValue::Action,
                );
                if page.undo.is_some() {
                    control(
                        options,
                        font,
                        "Undo reset",
                        ShellControlId::UndoPreferences,
                        ShellAction::UndoPreferences,
                        ControlValue::Action,
                    );
                }
            }
        });
}

pub(super) fn control(
    parent: &mut ChildSpawnerCommands,
    font: &TextFont,
    label: &str,
    id: ShellControlId,
    action: ShellAction,
    value: ControlValue,
) -> Entity {
    let value = if slider_actions(id).is_some() {
        ControlValue::Slider {
            fraction: 0.0,
            label: String::new(),
        }
    } else {
        value
    };
    let entity = spawn_control(parent, font, label, value);
    if action == ShellAction::Back {
        parent
            .commands()
            .entity(entity)
            .insert(crate::ui::surface::Surface::Back);
    }
    parent.commands().entity(entity).insert(ShellButton {
        id,
        action: Some(action),
        layer: ShellLayer::Settings,
    });
    entity
}

fn category_rows(category: Category) -> &'static [ShellControlId] {
    use ShellControlId::{
        CameraShakeUp, EffectsVolumeUp, GraphicsQuality, MusicVolumeUp, ToggleFocusMute,
        ToggleFullscreen, ToggleReducedEffects, ToggleReducedMotion, ToggleVsync, UiScaleUp,
        VolumeUp,
    };
    match category {
        Category::Display => &[ToggleFullscreen, ToggleVsync, GraphicsQuality],
        Category::Audio => &[VolumeUp, EffectsVolumeUp, MusicVolumeUp, ToggleFocusMute],
        Category::Accessibility => &[
            UiScaleUp,
            ToggleReducedMotion,
            ToggleReducedEffects,
            CameraShakeUp,
        ],
        Category::Controls => &[],
    }
}

fn action(id: ShellControlId) -> ShellAction {
    match id {
        ShellControlId::ToggleFullscreen => ShellAction::ToggleFullscreen,
        ShellControlId::ToggleVsync => ShellAction::ToggleVsync,
        ShellControlId::GraphicsQuality => ShellAction::CycleGraphicsQuality,
        ShellControlId::VolumeUp => ShellAction::VolumeUp,
        ShellControlId::EffectsVolumeUp => ShellAction::EffectsVolumeUp,
        ShellControlId::MusicVolumeUp => ShellAction::MusicVolumeUp,
        ShellControlId::CameraShakeUp => ShellAction::CameraShakeUp,
        ShellControlId::ToggleFocusMute => ShellAction::ToggleFocusMute,
        ShellControlId::UiScaleUp => ShellAction::UiScaleUp,
        ShellControlId::ToggleReducedMotion => ShellAction::ToggleReducedMotion,
        ShellControlId::ToggleReducedEffects => ShellAction::ToggleReducedEffects,
        _ => unreachable!("preference rows have an owned action"),
    }
}

fn description(id: ShellControlId) -> (&'static str, &'static str) {
    match id {
        ShellControlId::ToggleFullscreen => (
            "Fullscreen",
            "Fill the current display with a borderless game window.",
        ),
        ShellControlId::ToggleVsync => (
            "VSync",
            "Synchronize presentation with the display refresh to reduce screen tearing.",
        ),
        ShellControlId::GraphicsQuality => (
            "Graphics quality",
            "Tap to cycle Auto, Low, Medium and High. Auto picks a tier for this device; lower tiers trade anti-aliasing, glow, shadows and particle detail for a steadier frame rate.",
        ),
        ShellControlId::VolumeUp => ("Master volume", "Adjust the volume of all game audio."),
        ShellControlId::EffectsVolumeUp => (
            "Effects volume",
            "Adjust combat, match and interface sounds.",
        ),
        ShellControlId::MusicVolumeUp => ("Music volume", "Adjust menu and match music."),
        ShellControlId::ToggleFocusMute => (
            "Mute when unfocused",
            "Mute game audio while another application has focus.",
        ),
        ShellControlId::UiScaleUp => (
            "UI scale",
            "Change the size of interface text and controls.",
        ),
        ShellControlId::ToggleReducedMotion => (
            "Reduced motion",
            "Reduce menu movement and animated transitions.",
        ),
        ShellControlId::ToggleReducedEffects => (
            "Reduced combat effects",
            "Reduce combat effects while keeping gameplay information visible.",
        ),
        ShellControlId::CameraShakeUp => (
            "Camera shake",
            "Adjust how strongly the camera shakes from shots, hits and explosions. Reduced motion or reduced combat effects turn it off.",
        ),
        _ => ("", ""),
    }
}

pub(super) fn slider_actions(id: ShellControlId) -> Option<(ShellAction, ShellAction)> {
    match id {
        ShellControlId::UiScaleUp => Some((ShellAction::UiScaleDown, ShellAction::UiScaleUp)),
        ShellControlId::VolumeUp => Some((ShellAction::VolumeDown, ShellAction::VolumeUp)),
        ShellControlId::EffectsVolumeUp => {
            Some((ShellAction::EffectsVolumeDown, ShellAction::EffectsVolumeUp))
        }
        ShellControlId::MusicVolumeUp => {
            Some((ShellAction::MusicVolumeDown, ShellAction::MusicVolumeUp))
        }
        ShellControlId::CameraShakeUp => {
            Some((ShellAction::CameraShakeDown, ShellAction::CameraShakeUp))
        }
        ShellControlId::Input(id) => super::controls::slider_actions(id),
        _ => None,
    }
}

/// `auto_tier` is the tier `Auto` currently resolves to, shown beside it.
fn value(
    id: ShellControlId,
    settings: ClientShellSettings,
    auto_tier: Option<&str>,
) -> ControlValue {
    match id {
        ShellControlId::GraphicsQuality => {
            graphics_quality_value(settings.graphics_quality, auto_tier)
        }
        ShellControlId::ToggleFullscreen => ControlValue::Toggle(settings.fullscreen),
        ShellControlId::ToggleVsync => ControlValue::Toggle(settings.vsync),
        ShellControlId::ToggleFocusMute => ControlValue::Toggle(settings.mute_when_unfocused),
        ShellControlId::ToggleReducedMotion => ControlValue::Toggle(settings.reduced_motion),
        ShellControlId::ToggleReducedEffects => {
            ControlValue::Toggle(settings.reduced_combat_effects)
        }
        ShellControlId::UiScaleUp => ControlValue::Slider {
            fraction: (settings.ui_scale - MIN_UI_SCALE) / (MAX_UI_SCALE - MIN_UI_SCALE),
            label: format!("{:.0}%", settings.ui_scale * 100.0),
        },
        ShellControlId::VolumeUp => volume_slider(settings.master_volume),
        ShellControlId::EffectsVolumeUp => volume_slider(settings.effects_volume),
        ShellControlId::MusicVolumeUp => volume_slider(settings.music_volume),
        ShellControlId::CameraShakeUp => volume_slider(settings.camera_shake),
        _ => ControlValue::Action,
    }
}

/// The preference chip, showing the resolved tier beside `Auto`.
fn graphics_quality_value(quality: GraphicsQuality, auto_tier: Option<&str>) -> ControlValue {
    ControlValue::Binding(match (quality, auto_tier) {
        (GraphicsQuality::Auto, Some(tier)) => format!("AUTO ({tier})"),
        _ => quality.label().to_string(),
    })
}

fn volume_slider(volume: u8) -> ControlValue {
    ControlValue::Slider {
        fraction: f32::from(volume) / 100.0,
        label: format!("{volume}%"),
    }
}

/// The nearest 10% step to a pointer position along a percentage (volume or camera shake) slider.
fn volume_from_fraction(fraction: f32) -> u8 {
    (0_u8..=10)
        .min_by(|left, right| {
            (f32::from(*left) - fraction * 10.0)
                .abs()
                .total_cmp(&(f32::from(*right) - fraction * 10.0).abs())
        })
        .unwrap_or(0)
        * 10
}

/// The persisted percentage a 10%-step slider row (volumes, camera shake) owns.
fn volume_field(settings: &mut ClientShellSettings, id: ShellControlId) -> Option<&mut u8> {
    match id {
        ShellControlId::VolumeUp => Some(&mut settings.master_volume),
        ShellControlId::EffectsVolumeUp => Some(&mut settings.effects_volume),
        ShellControlId::MusicVolumeUp => Some(&mut settings.music_volume),
        ShellControlId::CameraShakeUp => Some(&mut settings.camera_shake),
        _ => None,
    }
}

#[allow(clippy::needless_pass_by_value, reason = "Bevy system parameters")]
pub(super) fn choose_category(
    mut pending: ResMut<PendingActions>,
    mut page: ResMut<SettingsPage>,
    mut dirty: ResMut<NavigationDirty>,
    mut selection: ResMut<InputSettingsSelection>,
    mut input: ResMut<super::controls::ControlsState>,
) {
    let mut chosen = None;
    pending.0.retain(|action| {
        let category = match action {
            ShellAction::RetryPreferences => {
                page.pending_save = true;
                return false;
            }
            ShellAction::DisplayCategory => Category::Display,
            ShellAction::AudioCategory => Category::Audio,
            ShellAction::AccessibilityCategory => Category::Accessibility,
            ShellAction::ControlsCategory => Category::Controls,
            _ => return true,
        };
        chosen = Some(category);
        false
    });
    if let Some(category) = chosen.filter(|category| *category != page.category) {
        page.category = category;
        selection.listening = false;
        page.rebuild = true;
        page.undo = None;
        input.cancel_transient();
        dirty.0 = Some(category.control().0);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "Bevy presentation queries; the graphics row also reads detected render capability"
)]
pub(super) fn present(
    page: Res<SettingsPage>,
    focus: Res<InputFocus>,
    draft: Option<Res<ShellSettingsDraft>>,
    input: Res<super::controls::ControlsState>,
    input_draft: Option<Res<InputSettingsDraft>>,
    capabilities: Option<Res<RenderCapabilities>>,
    mut controls: Query<(
        Entity,
        &ShellButton,
        &mut ControlState,
        &mut ControlValue,
        Option<&CategoryTab>,
        Has<Preference>,
    )>,
    mut details: Query<&mut Text, (With<Details>, Without<SaveError>)>,
) {
    let auto_tier = capabilities.map(|capabilities| capabilities.auto_tier().0.label());
    let mut detail = category_rows(page.category).first().copied();
    let mut input_detail = None;
    for (entity, button, mut state, mut current, tab, preference) in &mut controls {
        state.focused = focus.get() == Some(entity);
        state.selected = tab.is_some_and(|tab| tab.0 == page.category);
        state.inactive = !super::controls::allows(button.id, &input);
        if state.focused
            && let ShellControlId::Input(id) = button.id
        {
            input_detail = Some(id);
        }
        if preference {
            if state.focused {
                detail = Some(button.id);
            }
            if let Some(draft) = &draft {
                *current = value(button.id, draft.0, auto_tier);
            }
        }
    }
    for mut text in &mut details {
        if let Some((_, remaining)) = page.display_change {
            text.0 = format!(
                "KEEP THIS DISPLAY?\n\nReverting in {:.0} seconds.",
                remaining.ceil()
            );
            continue;
        }
        if page.category == Category::Controls {
            text.0 = super::controls::detail_text(
                &input,
                input_detail,
                input_draft
                    .as_ref()
                    .map_or_else(ClientInputSettings::default, |draft| draft.0),
            );
            continue;
        }
        text.0 = detail.map_or_else(String::new, |id| {
            let (title, body) = description(id);
            format!("{}\n\n{body}", title.to_uppercase())
        });
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "One persistence boundary commits valid input and shell preferences"
)]
pub(super) fn autosave(
    draft: Option<Res<ShellSettingsDraft>>,
    input_draft: Option<ResMut<InputSettingsDraft>>,
    mut active: ResMut<ClientShellSettings>,
    mut input: ResMut<ClientInputSettings>,
    path: Res<ClientSettingsPath>,
    mut page: ResMut<SettingsPage>,
    mouse: Res<ButtonInput<MouseButton>>,
    touch: Option<Res<SliderTouch>>,
) {
    let mut candidate = draft.as_ref().map_or(*active, |draft| draft.0);
    if let Some((previous, _)) = page.display_change {
        candidate.fullscreen = previous;
    }
    let mut input_draft = input_draft;
    let mut candidate_input = input_draft.as_ref().map_or(*input, |draft| draft.0);
    if let Err(error) = candidate
        .validate()
        .and_then(|()| candidate_input.validate())
    {
        page.save_error = error;
        return;
    }
    if candidate_input != *input {
        candidate_input.revision = input.revision.saturating_add(1);
        *input = candidate_input;
        if let Some(draft) = input_draft.as_deref_mut() {
            draft.0 = candidate_input;
        }
        page.pending_save = true;
    }
    if candidate != *active {
        *active = candidate;
        page.pending_save = true;
    }
    let slider_held =
        mouse.pressed(MouseButton::Left) || touch.as_deref().is_some_and(SliderTouch::held);
    if page.defer_save && slider_held {
        return;
    }
    page.defer_save = false;
    if !page.pending_save {
        return;
    }
    page.pending_save = false;
    page.save_error = save_settings(&path.0, *input, *active)
        .err()
        .map_or_else(String::new, |error| {
            format!("Changes are active but could not be saved: {error}")
        });
}

#[allow(clippy::needless_pass_by_value, reason = "Bevy slider input resources")]
pub(super) fn pointer_sliders(
    pointer: SliderPointer,
    overlay: Res<ClientOverlay>,
    buttons: Query<&ShellButton>,
    draft: Option<ResMut<ShellSettingsDraft>>,
    mut page: ResMut<SettingsPage>,
) {
    if *overlay != ClientOverlay::Settings {
        return;
    }
    let Some(mut draft) = draft else {
        return;
    };
    for adjust in pointer.adjustments() {
        let Ok(button) = buttons.get(adjust.owner) else {
            continue;
        };
        match button.id {
            ShellControlId::UiScaleUp => {
                page.defer_save = adjust.held;
                draft.0.ui_scale =
                    ((MIN_UI_SCALE + adjust.fraction * (MAX_UI_SCALE - MIN_UI_SCALE)) * 10.0)
                        .round()
                        / 10.0;
            }
            id => {
                if let Some(volume) = volume_field(&mut draft.0, id) {
                    page.defer_save = adjust.held;
                    *volume = volume_from_fraction(adjust.fraction);
                }
            }
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy presentation queries explicitly separate mutable node ownership"
)]
pub(super) fn present_retry(
    page: Res<SettingsPage>,
    mut buttons: Query<(&mut Node, &mut ShellButton), With<RetryPreference>>,
    mut errors: Query<
        (&mut Text, &mut Node),
        (
            With<SaveError>,
            Without<RetryPreference>,
            Without<SaveErrorContainer>,
        ),
    >,
    mut containers: Query<
        &mut Node,
        (
            With<SaveErrorContainer>,
            Without<RetryPreference>,
            Without<SaveError>,
        ),
    >,
) {
    let display = if page.save_error.is_empty() {
        Display::None
    } else {
        Display::Flex
    };
    for (mut text, mut node) in &mut errors {
        text.0.clone_from(&page.save_error);
        node.display = display;
    }
    for mut node in &mut containers {
        node.display = display;
    }
    for (mut node, mut button) in &mut buttons {
        let visible = !page.save_error.is_empty();
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        button.action = visible.then_some(ShellAction::RetryPreferences);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Settings root refresh preserves the screen-owned navigation target"
)]
pub(super) fn refresh(
    mut commands: Commands,
    mut page: ResMut<SettingsPage>,
    overlay: Res<ClientOverlay>,
    roots: Query<Entity, With<OverlayRoot>>,
    focus: Res<InputFocus>,
    buttons: Query<&ShellButton>,
    mut dirty: ResMut<NavigationDirty>,
) {
    if *overlay == ClientOverlay::Settings
        && core::mem::take(&mut page.rebuild)
        && !roots.is_empty()
    {
        if dirty.0.is_none() {
            dirty.0 = focus
                .get()
                .and_then(|entity| buttons.get(entity).ok())
                .map(|button| button.id);
        }
        despawn_overlays(&mut commands, &roots);
        spawn(&mut commands);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Settings preference actions and display rollback"
)]
pub(super) fn preference_actions(
    mut pending: ResMut<PendingActions>,
    mut page: ResMut<SettingsPage>,
    draft: Option<ResMut<ShellSettingsDraft>>,
    time: Res<Time<Real>>,
) {
    let Some(mut draft) = draft else {
        return;
    };
    if let Some((previous, remaining)) = &mut page.display_change {
        *remaining -= time.delta_secs();
        if *remaining <= 0.0 {
            draft.0.fullscreen = *previous;
            page.display_change = None;
            page.rebuild = true;
        }
    }
    pending.0.retain(|action| {
        match *action {
            ShellAction::KeepDisplay => {
                page.display_change = None;
                page.rebuild = true;
            }
            ShellAction::RevertDisplay | ShellAction::Back => {
                if let Some((previous, _)) = page.display_change.take() {
                    draft.0.fullscreen = previous;
                    page.rebuild = true;
                }
                if *action == ShellAction::Back {
                    return true;
                }
            }
            ShellAction::ResetPreferences => {
                page.undo = Some(draft.0);
                let old_fullscreen = draft.0.fullscreen;
                reset_category(page.category, &mut draft.0, ClientShellSettings::default());
                if old_fullscreen != draft.0.fullscreen {
                    page.display_change = Some((old_fullscreen, 15.0));
                }
                page.rebuild = true;
            }
            ShellAction::UndoPreferences => {
                if let Some(previous) = page.undo.take() {
                    reset_category(page.category, &mut draft.0, previous);
                }
                page.display_change = None;
                page.rebuild = true;
            }
            ShellAction::ToggleFullscreen => {
                let original = page
                    .display_change
                    .map_or(draft.0.fullscreen, |(previous, _)| previous);
                draft.0.fullscreen = !draft.0.fullscreen;
                page.display_change = (draft.0.fullscreen != original).then_some((original, 15.0));
                page.undo = None;
                page.rebuild = true;
            }
            _ => {
                if !adjust_preference(*action, &mut draft.0) {
                    return true;
                }
                if page.undo.take().is_some() {
                    page.rebuild = true;
                }
            }
        }
        false
    });
}

fn reset_category(
    category: Category,
    settings: &mut ClientShellSettings,
    source: ClientShellSettings,
) {
    match category {
        Category::Display => {
            settings.fullscreen = source.fullscreen;
            settings.vsync = source.vsync;
            settings.graphics_quality = source.graphics_quality;
        }
        Category::Audio => {
            settings.master_volume = source.master_volume;
            settings.effects_volume = source.effects_volume;
            settings.music_volume = source.music_volume;
            settings.mute_when_unfocused = source.mute_when_unfocused;
        }
        Category::Accessibility => {
            settings.ui_scale = source.ui_scale;
            settings.reduced_motion = source.reduced_motion;
            settings.reduced_combat_effects = source.reduced_combat_effects;
            settings.camera_shake = source.camera_shake;
        }
        Category::Controls => {}
    }
}
fn adjust_preference(action: ShellAction, settings: &mut ClientShellSettings) -> bool {
    match action {
        ShellAction::UiScaleDown | ShellAction::UiScaleUp => {
            let step = if action == ShellAction::UiScaleDown {
                -0.1
            } else {
                0.1
            };
            settings.ui_scale = (((settings.ui_scale + step) * 10.0).round() / 10.0)
                .clamp(MIN_UI_SCALE, MAX_UI_SCALE);
        }
        ShellAction::VolumeDown => {
            settings.master_volume = settings.master_volume.saturating_sub(10);
        }
        ShellAction::VolumeUp => {
            settings.master_volume = settings.master_volume.saturating_add(10).min(100);
        }
        ShellAction::EffectsVolumeDown => {
            settings.effects_volume = settings.effects_volume.saturating_sub(10);
        }
        ShellAction::EffectsVolumeUp => {
            settings.effects_volume = settings.effects_volume.saturating_add(10).min(100);
        }
        ShellAction::MusicVolumeDown => {
            settings.music_volume = settings.music_volume.saturating_sub(10);
        }
        ShellAction::MusicVolumeUp => {
            settings.music_volume = settings.music_volume.saturating_add(10).min(100);
        }
        ShellAction::CameraShakeDown => {
            settings.camera_shake = settings.camera_shake.saturating_sub(10);
        }
        ShellAction::CameraShakeUp => {
            settings.camera_shake = settings.camera_shake.saturating_add(10).min(100);
        }
        ShellAction::ToggleFocusMute => {
            settings.mute_when_unfocused = !settings.mute_when_unfocused;
        }
        ShellAction::ToggleVsync => settings.vsync = !settings.vsync,
        ShellAction::CycleGraphicsQuality => {
            settings.graphics_quality = settings.graphics_quality.next();
        }
        ShellAction::ToggleReducedMotion => settings.reduced_motion = !settings.reduced_motion,
        ShellAction::ToggleReducedEffects => {
            settings.reduced_combat_effects = !settings.reduced_combat_effects;
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::controls::SliderTrack;

    #[test]
    fn display_page_cycles_graphics_quality_and_shows_the_auto_tier() {
        assert!(category_rows(Category::Display).contains(&ShellControlId::GraphicsQuality));
        // A tap-to-cycle chip rather than a slider, so touch screens can change it.
        assert!(slider_actions(ShellControlId::GraphicsQuality).is_none());
        let mut settings = ClientShellSettings::default();
        let ControlValue::Binding(label) =
            value(ShellControlId::GraphicsQuality, settings, Some("MEDIUM"))
        else {
            panic!("graphics quality is a value chip");
        };
        assert_eq!(label, "AUTO (MEDIUM)");
        let mut seen = Vec::new();
        for _ in 0..4 {
            assert!(adjust_preference(
                ShellAction::CycleGraphicsQuality,
                &mut settings
            ));
            seen.push(settings.graphics_quality);
        }
        assert_eq!(
            seen,
            [
                GraphicsQuality::Low,
                GraphicsQuality::Medium,
                GraphicsQuality::High,
                GraphicsQuality::Auto
            ]
        );
        let mut changed = ClientShellSettings {
            graphics_quality: GraphicsQuality::High,
            ..ClientShellSettings::default()
        };
        reset_category(
            Category::Display,
            &mut changed,
            ClientShellSettings::default(),
        );
        assert_eq!(changed.graphics_quality, GraphicsQuality::Auto);
    }

    #[test]
    fn audio_page_owns_master_effects_and_music_volume_sliders() {
        assert_eq!(
            category_rows(Category::Audio),
            [
                ShellControlId::VolumeUp,
                ShellControlId::EffectsVolumeUp,
                ShellControlId::MusicVolumeUp,
                ShellControlId::ToggleFocusMute,
            ]
        );
        let mut settings = ClientShellSettings::default();
        assert!(adjust_preference(ShellAction::MusicVolumeUp, &mut settings));
        assert_eq!(settings.music_volume, 80);
        for _ in 0..5 {
            adjust_preference(ShellAction::MusicVolumeUp, &mut settings);
        }
        assert_eq!(settings.music_volume, 100);
        assert!(adjust_preference(
            ShellAction::EffectsVolumeDown,
            &mut settings
        ));
        assert_eq!(settings.effects_volume, 90);
        assert_eq!(settings.master_volume, 100);
        assert_eq!(volume_from_fraction(0.34), 30);

        let mut changed = ClientShellSettings {
            effects_volume: 10,
            music_volume: 0,
            ..ClientShellSettings::default()
        };
        reset_category(
            Category::Audio,
            &mut changed,
            ClientShellSettings::default(),
        );
        assert_eq!(changed, ClientShellSettings::default());
        assert_eq!(
            slider_actions(ShellControlId::MusicVolumeUp),
            Some((ShellAction::MusicVolumeDown, ShellAction::MusicVolumeUp))
        );
    }

    #[test]
    fn accessibility_page_owns_a_camera_shake_slider_independent_of_reduced_motion() {
        assert_eq!(
            category_rows(Category::Accessibility),
            [
                ShellControlId::UiScaleUp,
                ShellControlId::ToggleReducedMotion,
                ShellControlId::ToggleReducedEffects,
                ShellControlId::CameraShakeUp,
            ]
        );
        assert_eq!(
            slider_actions(ShellControlId::CameraShakeUp),
            Some((ShellAction::CameraShakeDown, ShellAction::CameraShakeUp))
        );
        let mut settings = ClientShellSettings::default();
        assert_eq!(settings.camera_shake, 100);
        assert!(adjust_preference(ShellAction::CameraShakeUp, &mut settings));
        assert_eq!(settings.camera_shake, 100);
        for _ in 0..12 {
            adjust_preference(ShellAction::CameraShakeDown, &mut settings);
        }
        assert_eq!(settings.camera_shake, 0);
        assert!(!settings.reduced_motion && !settings.reduced_combat_effects);
        let ControlValue::Slider { label, .. } =
            value(ShellControlId::CameraShakeUp, settings, None)
        else {
            panic!("camera shake is a slider");
        };
        assert_eq!(label, "0%");
        assert_eq!(
            volume_field(&mut settings, ShellControlId::CameraShakeUp).copied(),
            Some(0)
        );
        reset_category(
            Category::Accessibility,
            &mut settings,
            ClientShellSettings::default(),
        );
        assert_eq!(settings, ClientShellSettings::default());
    }

    #[test]
    fn pointer_slider_maps_bevy_track_coordinates_to_the_preference_range() {
        let mut app = App::new();
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Left);
        mouse.release(MouseButton::Left);
        app.insert_resource(mouse)
            .insert_resource(ClientOverlay::Settings)
            .init_resource::<SettingsPage>()
            .init_resource::<SliderTouch>()
            .insert_resource(ShellSettingsDraft(ClientShellSettings::default()))
            .add_systems(Update, pointer_sliders);
        let owner = app
            .world_mut()
            .spawn(ShellButton {
                id: ShellControlId::UiScaleUp,
                action: Some(ShellAction::UiScaleUp),
                layer: ShellLayer::Settings,
            })
            .id();
        app.world_mut().spawn((
            SliderTrack(owner),
            RelativeCursorPosition {
                cursor_over: true,
                normalized: Some(Vec2::new(0.5, 0.0)),
            },
        ));
        app.update();
        assert!(
            (app.world().resource::<ShellSettingsDraft>().0.ui_scale - MAX_UI_SCALE).abs() < 0.001
        );
    }
}
