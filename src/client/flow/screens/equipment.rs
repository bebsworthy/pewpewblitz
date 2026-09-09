//! Weapon overview and fullscreen inventory selection.
use super::{
    brawlers::weapon_part_effect_text,
    scroll::{clamp_scroll_offset, normalized_wheel_delta, offset_keeping_interval_visible},
    shared::{FlowButton, FlowNavigation},
};
use crate::client::flow::{
    actions::FlowUiAction,
    model::{ClientFlow, ClientOverlay},
};
use crate::client::ui::theme::{CREAM, INK, font};
use bevy::{
    input::mouse::MouseWheel,
    prelude::*,
    ui::{InteractionDisabled, ScrollPosition},
};
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub(in crate::client::flow) struct WeaponEquipmentDraft {
    pub(in crate::client::flow) brawler_id: Option<crate::profiles::SavedBrawlerId>,
    pub(in crate::client::flow) equipped_part_ids:
        [Option<crate::weapon_parts::WeaponPartInstanceId>;
            crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
    pub(in crate::client::flow) selected_slot: usize,
    pub(in crate::client::flow) picker_open: bool,
    pub(in crate::client::flow) awaiting_save: bool,
    pub(in crate::client::flow) inline_error: Option<String>,
}

impl Default for WeaponEquipmentDraft {
    fn default() -> Self {
        Self {
            brawler_id: None,
            equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
            selected_slot: 0,
            picker_open: false,
            awaiting_save: false,
            inline_error: None,
        }
    }
}

#[derive(Component, Clone, Debug, PartialEq)]
pub(in crate::client::flow) struct WeaponEquipmentRoot {
    draft: WeaponEquipmentDraft,
    width: f32,
    height: f32,
    revision: crate::profiles::ProfileRevision,
    pending: bool,
}
#[derive(Component)]
pub(in crate::client::flow) struct WeaponEquipmentScrollArea;
pub(in crate::client::flow) fn scroll_weapon_equipment(
    mut wheel: MessageReader<MouseWheel>,
    mut areas: Query<(&ComputedNode, &mut ScrollPosition), With<WeaponEquipmentScrollArea>>,
) {
    let delta = normalized_wheel_delta(wheel.read(), 48.0);
    if delta.abs() <= f32::EPSILON {
        return;
    }
    for (node, mut position) in &mut areas {
        position.0.y = clamp_scroll_offset(
            position.0.y - delta,
            node.content_size().y,
            node.size().y,
            node.inverse_scale_factor(),
        );
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "computed UI bounds are available only after Bevy's layout pass"
)]
pub(in crate::client::flow) fn keep_weapon_equipment_focus_visible(
    overlay: Res<ClientOverlay>,
    navigation: Res<FlowNavigation>,
    buttons: Query<(&FlowButton, &ChildOf, &ComputedNode, &UiGlobalTransform)>,
    parents: Query<&ChildOf>,
    mut areas: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            &mut ScrollPosition,
        ),
        With<WeaponEquipmentScrollArea>,
    >,
    mut prior: Local<Option<(Entity, usize, bool)>>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::WeaponEquipment) {
        *prior = None;
        return;
    }
    let Some((area_entity, area_node, area_transform, mut scroll)) = areas.iter_mut().next() else {
        *prior = None;
        return;
    };
    let focus_key = (area_entity, navigation.selected);
    if prior.as_ref().map(|(entity, index, _)| (*entity, *index)) != Some(focus_key) {
        // Let a newly spawned grid settle before caching its geometry.
        *prior = Some((area_entity, navigation.selected, false));
        return;
    }
    if prior.as_ref().is_some_and(|(_, _, applied)| *applied) {
        return;
    }
    let Some((_, _, button_node, button_transform)) =
        buttons.iter().find(|(button, child_of, _, _)| {
            (child_of.parent() == area_entity
                || parents
                    .get(child_of.parent())
                    .is_ok_and(|p| p.parent() == area_entity))
                && button.index == navigation.selected
        })
    else {
        return;
    };
    if area_node.is_empty() || button_node.is_empty() {
        return;
    }
    *prior = Some((area_entity, navigation.selected, true));
    let (_, _, area_center) = area_transform.to_scale_angle_translation();
    let (_, _, button_center) = button_transform.to_scale_angle_translation();
    let visible_top = area_center.y - area_node.size().y * 0.5 + 8.0;
    let visible_bottom = area_center.y + area_node.size().y * 0.5 - 8.0;
    let button_top = button_center.y - button_node.size().y * 0.5;
    let button_bottom = button_center.y + button_node.size().y * 0.5;
    let offset = offset_keeping_interval_visible(
        scroll.0.y,
        visible_top..visible_bottom,
        button_top..button_bottom,
        area_node.inverse_scale_factor(),
    );
    scroll.0.y = clamp_scroll_offset(
        offset,
        area_node.content_size().y,
        area_node.size().y,
        area_node.inverse_scale_factor(),
    );
}

#[derive(Component)]
pub(in crate::client) struct WeaponPreviewHost;

#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn present_weapon_equipment(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    draft: Res<WeaponEquipmentDraft>,
    roots: Query<(Entity, &WeaponEquipmentRoot)>,
    profile: Res<crate::client::ClientProfileModel>,
    assets: Option<Res<crate::client::ClientAssetHandles>>,
    parts: Res<crate::weapon_parts::WeaponPartCatalogResource>,
    server: Option<Res<AssetServer>>,
    windows: Query<&Window>,
    scale: Option<Res<UiScale>>,
) {
    if *flow.get() != ClientFlow::Dashboard || *overlay != ClientOverlay::WeaponEquipment {
        for (e, _) in &roots {
            commands.entity(e).despawn();
        }
        return;
    }
    let (Some(snapshot), Some(catalog), Some(id)) =
        (profile.snapshot(), profile.catalog(), draft.brawler_id)
    else {
        return;
    };
    let Some(saved) = snapshot.brawlers.iter().find(|b| b.id == id) else {
        return;
    };
    let scale = scale.map_or(1.0, |s| s.0);
    let width = windows.iter().next().map_or(1600.0, Window::width) / scale;
    let height = windows.iter().next().map_or(900.0, Window::height) / scale;
    let key = WeaponEquipmentRoot {
        draft: draft.clone(),
        width,
        height,
        revision: snapshot.revision,
        pending: profile.pending(),
    };
    if roots.iter().any(|(_, r)| *r == key) {
        return;
    }
    for (e, _) in &roots {
        commands.entity(e).despawn();
    }
    let view = EquipmentView {
        parts: &parts,
        columns: picker_columns(width),
        card_height: (260.0 * (width / 1600.0).min(height / 900.0)).max(190.0),
        assets: assets.as_deref(),
        server: server.as_deref(),
        factor: (width / 1600.0).min(height / 900.0).clamp(0.4, 1.5),
        pending: profile.pending(),
    };
    commands
        .spawn((
            key,
            DespawnOnExit(ClientFlow::Dashboard),
            GlobalZIndex(520),
            super::game_select::GameSelectionColumns(if draft.picker_open {
                usize::from(view.columns)
            } else {
                4
            }),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(28.0 * view.factor)),
                row_gap: px(22.0 * view.factor),
                ..default()
            },
            BackgroundColor(CREAM),
        ))
        .with_children(|root| {
            if let Some(s) = view.server {
                root.spawn((
                    ImageNode {
                        image_mode: NodeImageMode::Stretch,
                        ..ImageNode::new(s.load("brawler/ui/shell/game-selection-background.png"))
                    },
                    Node {
                        position_type: PositionType::Absolute,
                        width: percent(100),
                        height: percent(100),
                        left: px(0),
                        top: px(0),
                        ..default()
                    },
                    ZIndex(-1),
                ));
            }
            view.header(root, &draft, &saved.name);
            if draft.picker_open {
                view.picker(root, &draft, snapshot);
            } else {
                view.overview(root, &draft, snapshot, catalog, saved);
            }
            if let Some(error) = &draft.inline_error {
                view.label(root, error, 22.0, Color::srgb(0.75, 0.15, 0.02));
            }
            if draft.awaiting_save {
                view.label(root, "Saving…", 22.0, INK);
            }
        });
}
fn picker_columns(width: f32) -> u16 {
    if width >= 1400.0 {
        6
    } else if width >= 1100.0 {
        5
    } else if width >= 850.0 {
        4
    } else {
        3
    }
}

struct EquipmentView<'a> {
    parts: &'a crate::weapon_parts::WeaponPartCatalogResource,
    card_height: f32,
    columns: u16,
    assets: Option<&'a crate::client::ClientAssetHandles>,
    server: Option<&'a AssetServer>,
    factor: f32,
    pending: bool,
}
impl EquipmentView<'_> {
    fn part_image(
        &self,
        parent: &mut ChildSpawnerCommands,
        part: &crate::weapon_parts::WeaponPartInstance,
        height: f32,
    ) {
        let family = self
            .parts
            .0
            .definition(part.definition_id)
            .map_or("chamber", |d| d.presentation_type.as_str());
        if let Some(server) = self.server {
            parent.spawn((
                ImageNode::new(server.load(format!(
                    "brawler/ui/shell/part-{}.png",
                    family.to_lowercase()
                ))),
                Node {
                    height: px(height * self.factor),
                    aspect_ratio: Some(1.2),
                    flex_shrink: 0.0,
                    ..default()
                },
            ));
        }
    }
    fn label(&self, p: &mut ChildSpawnerCommands, text: &str, size: f32, color: Color) {
        p.spawn((
            Text::new(text),
            font(self.assets, size * self.factor),
            TextColor(color),
            TextLayout::new(Justify::Center, LineBreak::WordBoundary),
        ));
    }
    fn header(&self, p: &mut ChildSpawnerCommands, draft: &WeaponEquipmentDraft, name: &str) {
        p.spawn(Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: px(18.0 * self.factor),
            ..default()
        })
        .with_children(|h| {
            self.button(
                h,
                1000,
                FlowUiAction::CancelWeaponEquipment,
                false,
                Node {
                    width: percent(23),
                    height: px(66.0 * self.factor),
                    ..default()
                },
                |b| {
                    self.label(
                        b,
                        if draft.picker_open {
                            "‹ BACK TO WEAPON"
                        } else {
                            "‹ BACK TO BRAWLER"
                        },
                        24.0,
                        INK,
                    );
                },
            );
            self.label(
                h,
                if draft.picker_open {
                    "CHOOSE PART"
                } else {
                    "CUSTOMIZE WEAPON"
                },
                46.0,
                INK,
            );
            self.label(
                h,
                &if draft.picker_open {
                    format!("SLOT {} / {}", draft.selected_slot + 1, name)
                } else {
                    name.to_uppercase()
                },
                24.0,
                Color::srgb(0.08, 0.44, 0.44),
            );
        });
    }
    fn button(
        &self,
        p: &mut ChildSpawnerCommands,
        index: usize,
        action: FlowUiAction,
        selected: bool,
        mut node: Node,
        content: impl FnOnce(&mut ChildSpawnerCommands),
    ) {
        node.flex_direction = FlexDirection::Column;
        node.align_items = AlignItems::Center;
        node.justify_content = JustifyContent::Center;
        node.padding = UiRect::axes(px(24.0 * self.factor), px(22.0 * self.factor));
        node.row_gap = px(8.0 * self.factor);
        let mut e = p.spawn((
            crate::client::ui::surface::SurfaceState {
                selected,
                ..default()
            },
            Button,
            FlowButton {
                index,
                action,
                error_action: true,
            },
            node,
        ));
        if self.pending {
            e.insert(InteractionDisabled);
        }
        e.with_children(content);
    }
    fn picker(
        &self,
        p: &mut ChildSpawnerCommands,
        draft: &WeaponEquipmentDraft,
        snapshot: &crate::profiles::ProfileSnapshot,
    ) {
        p.spawn((
            WeaponEquipmentScrollArea,
            ScrollPosition::default(),
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                ..default()
            },
        ))
        .with_children(|viewport| {
            viewport
                .spawn(Node {
                    width: percent(100),
                    flex_shrink: 0.0,
                    display: Display::Grid,
                    grid_template_columns: RepeatedGridTrack::flex(self.columns, 1.0),
                    grid_auto_rows: vec![GridTrack::minmax(
                        MinTrackSizingFunction::Px(self.card_height),
                        MaxTrackSizingFunction::MaxContent,
                    )],
                    column_gap: px(12.0 * self.factor),
                    row_gap: px(12.0 * self.factor),
                    padding: UiRect::all(px(8.0 * self.factor)),
                    ..default()
                })
                .with_children(|grid| {
                    self.button(
                        grid,
                        0,
                        FlowUiAction::UnequipWeaponPart,
                        draft.equipped_part_ids[draft.selected_slot].is_none(),
                        Node::default(),
                        |b| {
                            self.label(b, "−", 75.0, Color::srgb(0.08, 0.44, 0.44));
                            self.label(b, "NOTHING", 24.0, INK);
                        },
                    );
                    for (i, part) in snapshot.inventory.iter().enumerate() {
                        self.button(
                            grid,
                            i + 1,
                            FlowUiAction::EquipWeaponPart(part.id),
                            draft.equipped_part_ids[draft.selected_slot] == Some(part.id),
                            Node::default(),
                            |b| {
                                self.part_image(b, part, 90.0);
                                self.label(b, &part.display_name.to_uppercase(), 24.0, INK);
                                for effect in &part.effects {
                                    self.label(b, &weapon_part_effect_text(*effect), 20.0, INK);
                                }
                                if let Some(owner) = snapshot.brawlers.iter().find(|b| {
                                    Some(b.id) != draft.brawler_id
                                        && b.equipped_part_ids.contains(&Some(part.id))
                                }) {
                                    self.label(b, &format!("Used by {}", owner.name), 18.0, INK);
                                } else if let Some(slot) = draft
                                    .equipped_part_ids
                                    .iter()
                                    .position(|id| *id == Some(part.id))
                                    .filter(|slot| *slot != draft.selected_slot)
                                {
                                    self.label(b, &format!("Slot {}", slot + 1), 18.0, INK);
                                }
                            },
                        );
                    }
                });
        });
    }
    fn stats(
        &self,
        body: &mut ChildSpawnerCommands,
        snapshot: &crate::profiles::ProfileSnapshot,
        catalog: &crate::profiles::AdvertisedBrawlerCatalog,
        saved: &crate::profiles::SavedBrawler,
    ) {
        body.spawn(Node {
            width: percent(30),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceEvenly,
            ..default()
        })
        .with_children(|stats| {
            if let Some(weapon) = snapshot.weapon_modifiers(saved).ok().and_then(|m| {
                catalog.weapon(saved.weapon_base_id).and_then(|w| {
                    crate::weapon_parts::resolve_advertised_weapon_parts(
                        &w.configuration,
                        &catalog.weapon_policy,
                        catalog.fighter_body,
                        crate::combat::WeaponPresetId(saved.weapon_base_id.0),
                        m,
                    )
                    .ok()
                })
            }) {
                for (label, value) in super::brawlers::weapon_stat_rows(&weapon) {
                    stats
                        .spawn(Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            column_gap: px(12.0 * self.factor),
                            padding: UiRect::right(px(18.0 * self.factor)),
                            ..default()
                        })
                        .with_children(|row| {
                            self.label(row, label, 27.0, INK);
                            self.label(row, &value, 34.0, Color::srgb(0.08, 0.44, 0.44));
                        });
                }
            }
        });
    }
    fn overview(
        &self,
        p: &mut ChildSpawnerCommands,
        draft: &WeaponEquipmentDraft,
        snapshot: &crate::profiles::ProfileSnapshot,
        catalog: &crate::profiles::AdvertisedBrawlerCatalog,
        saved: &crate::profiles::SavedBrawler,
    ) {
        p.spawn(Node {
            width: percent(100),
            flex_grow: 1.0,
            min_height: px(0),
            column_gap: px(20.0 * self.factor),
            ..default()
        })
        .with_children(|body| {
            body.spawn(Node {
                width: percent(22),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|title| {
                self.label(
                    title,
                    &catalog
                        .weapon(saved.weapon_base_id)
                        .map_or("WEAPON", |w| w.display_name.as_str())
                        .to_uppercase(),
                    65.0,
                    INK,
                );
            });
            body.spawn((
                WeaponPreviewHost,
                Node {
                    width: percent(48),
                    height: percent(100),
                    ..default()
                },
            ));
            self.stats(body, snapshot, catalog, saved);
        });
        p.spawn(Node {
            width: percent(88),
            height: px(245.0 * self.factor),
            column_gap: px(18.0 * self.factor),
            ..default()
        })
        .with_children(|slots| {
            for slot in 0..4 {
                let part = draft.equipped_part_ids[slot]
                    .and_then(|id| snapshot.inventory.iter().find(|p| p.id == id));
                self.button(
                    slots,
                    slot,
                    FlowUiAction::SelectEquipmentSlot(slot),
                    false,
                    Node {
                        width: percent(24),
                        height: percent(100),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    |b| {
                        self.label(b, &format!("{}", slot + 1), 23.0, INK);
                        if let Some(part) = part {
                            self.part_image(b, part, 90.0);
                        } else {
                            self.label(b, "+", 62.0, Color::srgb(0.08, 0.44, 0.44));
                        }
                        self.label(
                            b,
                            part.map_or("EMPTY", |p| p.display_name.as_str()),
                            24.0,
                            INK,
                        );
                    },
                );
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wheel_scrolls_both_directions_and_clamps_to_viewport_content() {
        use bevy::input::{mouse::MouseScrollUnit, touch::TouchPhase};
        let mut app = App::new();
        app.add_message::<MouseWheel>()
            .add_systems(Update, scroll_weapon_equipment);
        let area = app
            .world_mut()
            .spawn((
                WeaponEquipmentScrollArea,
                ScrollPosition::default(),
                ComputedNode {
                    size: Vec2::splat(200.0),
                    content_size: Vec2::new(200.0, 1200.0),
                    inverse_scale_factor: 1.0,
                    ..default()
                },
            ))
            .id();
        for (delta, expected) in [(-3.0, 144.0), (3.0, 0.0), (-999.0, 1000.0)] {
            app.world_mut().write_message(MouseWheel {
                unit: MouseScrollUnit::Line,
                x: 0.0,
                y: delta,
                window: Entity::PLACEHOLDER,
                phase: TouchPhase::Moved,
            });
            app.update();
            assert!(
                (app.world().get::<ScrollPosition>(area).unwrap().0.y - expected).abs()
                    < f32::EPSILON
            );
        }
    }
    #[test]
    fn picker_density_tracks_available_width() {
        assert_eq!(picker_columns(1600.0), 6);
        assert_eq!(picker_columns(1280.0), 5);
        assert_eq!(picker_columns(1000.0), 4);
        assert_eq!(picker_columns(720.0), 3);
    }
    #[test]
    fn picker_focus_waits_for_layout_then_reveals_lower_row_without_locking_mouse_scroll() {
        let mut app = App::new();
        app.insert_resource(ClientOverlay::WeaponEquipment)
            .insert_resource(FlowNavigation { selected: 6 })
            .add_systems(Update, keep_weapon_equipment_focus_visible);
        let area = app
            .world_mut()
            .spawn((
                WeaponEquipmentScrollArea,
                ScrollPosition::default(),
                ComputedNode::default(),
                UiGlobalTransform::default(),
            ))
            .id();
        let grid = app.world_mut().spawn(ChildOf(area)).id();
        let card = app
            .world_mut()
            .spawn((
                FlowButton {
                    index: 6,
                    action: FlowUiAction::UnequipWeaponPart,
                    error_action: true,
                },
                ChildOf(grid),
                ComputedNode::default(),
                UiGlobalTransform::default(),
            ))
            .id();
        app.update();
        app.update();
        app.world_mut().entity_mut(area).insert((
            ComputedNode {
                size: Vec2::splat(100.0),
                content_size: Vec2::new(100.0, 300.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            UiGlobalTransform::from(bevy::math::Affine2::from_translation(Vec2::splat(50.0))),
        ));
        app.world_mut().entity_mut(card).insert((
            ComputedNode {
                size: Vec2::splat(80.0),
                inverse_scale_factor: 1.0,
                ..default()
            },
            UiGlobalTransform::from(bevy::math::Affine2::from_translation(Vec2::new(
                50.0, 200.0,
            ))),
        ));
        app.update();
        assert!(app.world().get::<ScrollPosition>(area).unwrap().0.y > 100.0);
        app.world_mut().get_mut::<ScrollPosition>(area).unwrap().0.y = 0.0;
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(area).unwrap().0.y, 0.0);
    }
}
