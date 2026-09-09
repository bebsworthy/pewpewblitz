pub(in crate::client::flow) use super::equipment::{
    WeaponEquipmentDraft, keep_weapon_equipment_focus_visible, present_weapon_equipment,
    scroll_weapon_equipment,
};
#[cfg(test)]
pub(in crate::client::flow) use super::equipment::{
    WeaponEquipmentRoot, WeaponEquipmentScrollArea,
};
// Saved-brawler browsing, creation, editing and deletion presentation.
use super::shared::{FlowButton, FlowNavigation};
use super::{
    dashboard::{DashboardLayoutClass, dashboard_layout_class},
    scroll::{clamp_scroll_offset, normalized_wheel_delta, offset_keeping_interval_visible},
};
use crate::client::flow::{
    actions::FlowUiAction,
    model::{ClientFlow, ClientOverlay},
};
use crate::client::ui::theme::{CREAM, INK, TEAL, font};
use bevy::{
    input::mouse::MouseWheel,
    prelude::*,
    ui::{InteractionDisabled, ScrollPosition, UiScale},
    window::PrimaryWindow,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::client::flow) enum CreationPicker {
    Fighter,
    Weapon,
    Ultimate,
}

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub(in crate::client::flow) struct BrawlerCreationDraft {
    pub(in crate::client::flow) name: String,
    pub(in crate::client::flow) name_caret: usize,
    pub(in crate::client::flow) editing_name: bool,
    pub(in crate::client::flow) picker: Option<CreationPicker>,
    pub(in crate::client::flow) fighter_profile_id: crate::profiles::FighterProfileId,
    pub(in crate::client::flow) weapon_base_id: crate::profiles::WeaponBaseId,
    pub(in crate::client::flow) ultimate: crate::builds::UltimateDefinitionId,
    pub(in crate::client::flow) inline_error: Option<String>,
}

impl Default for BrawlerCreationDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            name_caret: 0,
            editing_name: false,
            picker: None,
            fighter_profile_id: crate::profiles::FighterProfileId(1),
            weapon_base_id: crate::profiles::WeaponBaseId(1),
            ultimate: crate::builds::UltimateDefinitionId(1),
            inline_error: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::client::flow) enum AbilityPicker {
    Ultimate,
    Passive(usize),
}

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub(in crate::client::flow) struct BrawlerEditDraft {
    pub(in crate::client::flow) brawler_id: Option<crate::profiles::SavedBrawlerId>,
    pub(in crate::client::flow) name: String,
    pub(in crate::client::flow) fighter_profile_id: crate::profiles::FighterProfileId,
    pub(in crate::client::flow) weapon_base_id: crate::profiles::WeaponBaseId,
    pub(in crate::client::flow) ultimate_id: crate::builds::UltimateDefinitionId,
    pub(in crate::client::flow) passive_ids: [crate::builds::PassiveDefinitionId; 2],
    pub(in crate::client::flow) name_caret: usize,
    pub(in crate::client::flow) editing_name: bool,
    pub(in crate::client::flow) inline_rename: bool,
    pub(in crate::client::flow) picker: Option<AbilityPicker>,
    pub(in crate::client::flow) inline_error: Option<String>,
}

impl Default for BrawlerEditDraft {
    fn default() -> Self {
        Self {
            brawler_id: None,
            name: String::new(),
            fighter_profile_id: crate::profiles::FighterProfileId(1),
            weapon_base_id: crate::profiles::WeaponBaseId(1),
            ultimate_id: crate::builds::UltimateDefinitionId(1),
            passive_ids: [
                crate::builds::PassiveDefinitionId(3),
                crate::builds::PassiveDefinitionId(4),
            ],
            name_caret: 0,
            editing_name: false,
            inline_rename: false,
            picker: None,
            inline_error: None,
        }
    }
}

#[derive(Component)]
pub(in crate::client::flow) struct DeleteBrawlerConfirmationRoot;

#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(in crate::client::flow) struct BrawlerListRoot {
    width: f32,
    height: f32,
    profile_revision: crate::profiles::ProfileRevision,
    pending: bool,
}

#[derive(Component)]
pub(in crate::client::flow) struct BrawlerListScrollArea;

#[derive(Component)]
pub(in crate::client) struct BrawlerCardPreview;

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub(in crate::client::flow) struct BrawlerDetailsRoot {
    viewport: (u32, u32),
    brawler_id: crate::profiles::SavedBrawlerId,
    profile_revision: crate::profiles::ProfileRevision,
    pending: bool,
    pub(in crate::client::flow) contextual_confirmation: bool,
    layout: BrawlerDetailsLayout,
    rename: Option<BrawlerEditDraft>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BrawlerDetailsLayout {
    Compact,
    Wide,
}

#[derive(Component)]
pub(in crate::client::flow) struct BrawlerDetailsScrollArea;

#[derive(Component)]
pub(in crate::client) struct BrawlerDetailsPreviewHost;

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub(in crate::client::flow) struct BrawlerCreationRoot {
    draft: BrawlerCreationDraft,
    layout: BrawlerDetailsLayout,
    viewport: (u32, u32),
    pending: bool,
}

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub(in crate::client::flow) struct BrawlerEditorRoot {
    draft: BrawlerEditDraft,
    viewport: (u32, u32),
    pending: bool,
}

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn open_empty_profile_creation(
    profile: Res<crate::client::ClientProfileModel>,
    mut overlay: ResMut<ClientOverlay>,
    mut draft: ResMut<BrawlerCreationDraft>,
) {
    if matches!(overlay.as_ref(), ClientOverlay::None)
        && profile
            .snapshot()
            .is_some_and(|snapshot| snapshot.brawlers.is_empty())
    {
        let Some(catalog) = profile.catalog() else {
            return;
        };
        let (Some(fighter), Some(weapon), Some(ultimate)) = (
            catalog.fighter_profiles.first(),
            catalog.weapon_bases.first(),
            catalog.ultimates.first(),
        ) else {
            return;
        };
        *draft = BrawlerCreationDraft {
            fighter_profile_id: fighter.id,
            weapon_base_id: weapon.id,
            ultimate: ultimate.id,
            name: "Brawler 1".into(),
            ..default()
        };
        *overlay = ClientOverlay::BrawlerCreation;
    }
}

fn advertised_fighter_name(
    catalog: &crate::profiles::AdvertisedBrawlerCatalog,
    id: crate::profiles::FighterProfileId,
) -> &str {
    catalog
        .fighter(id)
        .map_or("Unknown", |definition| definition.display_name.as_str())
}

fn advertised_weapon_name(
    catalog: &crate::profiles::AdvertisedBrawlerCatalog,
    id: crate::profiles::WeaponBaseId,
) -> &str {
    catalog
        .weapon(id)
        .map_or("Unknown", |definition| definition.display_name.as_str())
}

pub(in crate::client::flow) fn ultimate_name(
    catalog: &crate::profiles::AdvertisedBrawlerCatalog,
    id: crate::builds::UltimateDefinitionId,
) -> &str {
    catalog
        .ultimate(id)
        .map_or("Unknown", |definition| definition.display_name.as_str())
}

fn passive_name(
    catalog: &crate::profiles::AdvertisedBrawlerCatalog,
    id: crate::builds::PassiveDefinitionId,
) -> &str {
    catalog
        .passive(id)
        .map_or("Unknown", |definition| definition.display_name.as_str())
}

pub(in crate::client::flow) fn brawler_loadout_summary(
    brawler: &crate::profiles::SavedBrawler,
    catalog: &crate::profiles::AdvertisedBrawlerCatalog,
) -> String {
    format!(
        "{} · {}\n{} · {} + {}",
        advertised_fighter_name(catalog, brawler.fighter_profile_id),
        advertised_weapon_name(catalog, brawler.weapon_base_id),
        ultimate_name(catalog, brawler.ultimate_id),
        passive_name(catalog, brawler.passive_ids[0]),
        passive_name(catalog, brawler.passive_ids[1]),
    )
}

struct BrawlerBrowserArt<'a> {
    card_height: f32,
    server: Option<&'a AssetServer>,
    assets: Option<&'a crate::client::ClientAssetHandles>,
}

impl BrawlerBrowserArt<'_> {
    fn label(
        &self,
        parent: &mut ChildSpawnerCommands,
        text: impl Into<String>,
        size: f32,
        color: Color,
    ) {
        parent.spawn((
            Text::new(text),
            font(self.assets, size),
            TextColor(color),
            TextLayout::justify(Justify::Center),
            Node {
                width: percent(100),
                min_width: px(0),
                flex_shrink: 0.0,
                ..default()
            },
        ));
    }

    fn chrome(&self, root: &mut ChildSpawnerCommands, width: f32) {
        if let Some(server) = self.server {
            root.spawn((
                ImageNode {
                    image_mode: NodeImageMode::Stretch,
                    ..ImageNode::new(server.load("brawler/ui/shell/game-selection-background.png"))
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(0),
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                ZIndex(-1),
            ));
        }
        root.spawn(Node {
            width: percent(100),
            height: px(72),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|header| {
            let back = Self::button(
                header,
                1000,
                FlowUiAction::CloseBrawlerList,
                false,
                false,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(4),
                    width: px(130),
                    height: px(54),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(px(12), px(8)),
                    ..default()
                },
            );
            header
                .commands()
                .entity(back)
                .with_children(|b| self.label(b, "‹ BACK", 22.0, INK));
            header
                .spawn(Node {
                    width: px((width - 330.0).max(130.0)),
                    ..default()
                })
                .with_children(|title| {
                    self.label(
                        title,
                        "BRAWLERS",
                        if width < 760.0 { 30.0 } else { 46.0 },
                        INK,
                    );
                });
        });
    }

    fn cards(
        &self,
        grid: &mut ChildSpawnerCommands,
        snapshot: &crate::profiles::ProfileSnapshot,
        catalog: &crate::profiles::AdvertisedBrawlerCatalog,
        pending: bool,
    ) {
        let full = snapshot.brawlers.len() >= usize::from(catalog.limits.maximum_saved_brawlers);
        let create = Self::button(
            grid,
            0,
            FlowUiAction::CreateBrawler,
            false,
            pending || full,
            browser_card_node(self.card_height),
        );
        grid.commands().entity(create).with_children(|card| {
            self.label(card, "+", 96.0, TEAL);
            self.label(card, "CREATE\nBRAWLER", 32.0, INK);
            if full {
                self.label(card, "Brawler limit reached", 16.0, INK);
            }
        });
        for (index, brawler) in snapshot.brawlers.iter().enumerate() {
            spawn_brawler_list_row(
                grid,
                index + 1,
                brawler,
                snapshot.selected_brawler_id == Some(brawler.id),
                catalog,
                self,
                pending,
            );
        }
    }

    fn button(
        parent: &mut ChildSpawnerCommands,
        index: usize,
        action: FlowUiAction,
        selected: bool,
        disabled: bool,
        node: Node,
    ) -> Entity {
        let mut entity = parent.spawn((
            Button,
            FlowButton {
                index,
                action,
                error_action: true,
            },
            crate::client::ui::surface::SurfaceState {
                selected,
                ..default()
            },
            node,
        ));
        if disabled {
            entity.insert(InteractionDisabled);
        }
        entity.id()
    }
}

fn spawn_brawler_list_row(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    brawler: &crate::profiles::SavedBrawler,
    selected: bool,
    catalog: &crate::profiles::AdvertisedBrawlerCatalog,
    art: &BrawlerBrowserArt,
    disabled: bool,
) {
    let summary = brawler_loadout_summary(brawler, catalog);
    let id = BrawlerBrowserArt::button(
        parent,
        index,
        FlowUiAction::OpenBrawlerDetails(brawler.id),
        selected,
        disabled,
        browser_card_node(art.card_height),
    );
    parent
        .commands()
        .entity(id)
        .insert(AccessibleLabel::new(format!(
            "{}{}: {summary}",
            brawler.name,
            if selected { ", selected for play" } else { "" }
        )))
        .with_children(|card| {
            card.spawn(Node {
                width: percent(100),
                height: px(art.card_height - 120.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                ..default()
            })
            .with_children(|frame| {
                frame.spawn((
                    BrawlerCardPreview,
                    Node {
                        width: px(0),
                        height: px(0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                ));
            });
            art.label(card, brawler.name.to_uppercase(), 34.0, INK);
            art.label(
                card,
                advertised_weapon_name(catalog, brawler.weapon_base_id),
                20.0,
                INK,
            );
        });
}

fn browser_columns(width: f32) -> usize {
    if width >= 1350.0 {
        5
    } else if width >= 1050.0 {
        4
    } else if width >= 760.0 {
        3
    } else {
        2
    }
}

fn browser_card_node(height: f32) -> Node {
    Node {
        min_width: px(0),
        height: px(height),
        padding: UiRect::axes(px(20), px(18)),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        row_gap: px(4),
        ..default()
    }
}

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn scroll_brawler_list(
    overlay: Res<ClientOverlay>,
    mut events: MessageReader<MouseWheel>,
    mut areas: Query<(&ComputedNode, &mut ScrollPosition), With<BrawlerListScrollArea>>,
) {
    if !matches!(
        overlay.as_ref(),
        ClientOverlay::BrawlerList | ClientOverlay::BrawlerCreation | ClientOverlay::BrawlerEditor
    ) {
        return;
    }
    let delta = normalized_wheel_delta(events.read(), 36.0);
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
    clippy::too_many_arguments,
    reason = "screen-owned snapshot reconciliation and responsive rendering"
)]
pub(in crate::client::flow) fn present_brawler_list(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    profile: Res<crate::client::ClientProfileModel>,
    roots: Query<(Entity, &BrawlerListRoot)>,
    scroll_areas: Query<&ScrollPosition, With<BrawlerListScrollArea>>,
    mut navigation: ResMut<FlowNavigation>,
    server: Option<Res<AssetServer>>,
    assets: Option<Res<crate::client::ClientAssetHandles>>,
    windows: Query<&Window>,
    scale: Option<Res<UiScale>>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::BrawlerList)
        || *flow.get() != ClientFlow::Dashboard
    {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    let (Some(snapshot), Some(catalog)) = (profile.snapshot(), profile.catalog()) else {
        return;
    };
    let scale = scale.map_or(1.0, |s| s.0);
    let width = windows.iter().next().map_or(1600.0, Window::width) / scale;
    let height = windows.iter().next().map_or(900.0, Window::height) / scale;
    let pending = profile.pending();
    let render_key = BrawlerListRoot {
        profile_revision: snapshot.revision,
        pending,
        width,
        height,
    };
    if roots.iter().any(|(_, root)| *root == render_key) {
        return;
    }
    let retained_scroll = scroll_areas.iter().next().cloned().unwrap_or_default();
    if roots.is_empty() {
        navigation.selected = snapshot
            .selected_brawler_id
            .and_then(|selected| snapshot.brawlers.iter().position(|b| b.id == selected))
            .map_or(0, |i| i + 1);
    }
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    let columns = browser_columns(width);
    let column_count = f32::from(u16::try_from(columns).unwrap_or(5));
    let card_height =
        (((width - 64.0 - 16.0 * (column_count - 1.0)) / column_count) * 1.15).clamp(240.0, 350.0);
    let art = BrawlerBrowserArt {
        card_height,
        server: server.as_deref(),
        assets: assets.as_deref(),
    };
    commands
        .spawn((
            render_key,
            super::game_select::GameSelectionColumns(columns),
            DespawnOnExit(ClientFlow::Dashboard),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(24)),
                row_gap: px(20),
                ..default()
            },
            BackgroundColor(CREAM),
            GlobalZIndex(510),
        ))
        .with_children(|root| {
            art.chrome(root, width);
            root.spawn((
                BrawlerListScrollArea,
                retained_scroll,
                Node {
                    width: percent(100),
                    min_height: px(0),
                    flex_grow: 1.0,
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
                        grid_template_columns: RepeatedGridTrack::flex(
                            u16::try_from(columns).unwrap_or(5),
                            1.0,
                        ),
                        grid_auto_rows: vec![GridTrack::px(card_height)],
                        column_gap: px(16),
                        row_gap: px(16),
                        padding: UiRect::all(px(8)),
                        ..default()
                    })
                    .with_children(|grid| {
                        art.cards(grid, snapshot, catalog, pending);
                    });
            });
        });
}

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn keep_brawler_list_focus_visible(
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
        With<BrawlerListScrollArea>,
    >,
    mut prior: Local<Option<(Entity, usize)>>,
) {
    if !matches!(
        overlay.as_ref(),
        ClientOverlay::BrawlerList | ClientOverlay::BrawlerCreation | ClientOverlay::BrawlerEditor
    ) {
        *prior = None;
        return;
    }
    let Some((area_entity, area_node, area_transform, mut scroll)) = areas.iter_mut().next() else {
        *prior = None;
        return;
    };
    let focus_key = (area_entity, navigation.selected);
    if prior.as_ref() == Some(&focus_key) {
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
    *prior = Some(focus_key);
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

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn scroll_brawler_details(
    overlay: Res<ClientOverlay>,
    mut events: MessageReader<MouseWheel>,
    mut areas: Query<(&ComputedNode, &mut ScrollPosition), With<BrawlerDetailsScrollArea>>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::BrawlerDetails(_)) {
        return;
    }
    let delta = normalized_wheel_delta(events.read(), 36.0);
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

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn keep_brawler_details_focus_visible(
    overlay: Res<ClientOverlay>,
    navigation: Res<FlowNavigation>,
    buttons: Query<(Entity, &FlowButton, &ComputedNode, &UiGlobalTransform)>,
    parents: Query<&ChildOf>,
    mut areas: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            &mut ScrollPosition,
        ),
        With<BrawlerDetailsScrollArea>,
    >,
    mut prior: Local<Option<(Entity, usize)>>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::BrawlerDetails(_)) {
        *prior = None;
        return;
    }
    let Some((area_entity, area_node, area_transform, mut scroll)) = areas.iter_mut().next() else {
        *prior = None;
        return;
    };
    let focus_key = (area_entity, navigation.selected);
    if prior.as_ref() == Some(&focus_key) {
        return;
    }
    *prior = Some(focus_key);
    let Some((_, _, button_node, button_transform)) =
        buttons.iter().find(|(entity, button, _, _)| {
            button.index == navigation.selected
                && parents
                    .iter_ancestors(*entity)
                    .any(|ancestor| ancestor == area_entity)
        })
    else {
        return;
    };
    if area_node.is_empty() || button_node.is_empty() {
        return;
    }
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

#[derive(bevy::ecs::system::SystemParam)]
pub(in crate::client::flow) struct BrawlerScreenUi<'w, 's> {
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    scale: Option<Res<'w, UiScale>>,
    navigation: ResMut<'w, FlowNavigation>,
}

fn brawler_screen_layout(screen: &BrawlerScreenUi<'_, '_>) -> BrawlerDetailsLayout {
    screen
        .windows
        .iter()
        .next()
        .map_or(BrawlerDetailsLayout::Wide, |window| {
            if dashboard_layout_class(
                window.resolution.width(),
                window.resolution.height(),
                screen.scale.as_deref().map_or(1.0, |scale| scale.0),
            ) == DashboardLayoutClass::Compact
            {
                BrawlerDetailsLayout::Compact
            } else {
                BrawlerDetailsLayout::Wide
            }
        })
}

mod details;
pub(in crate::client::flow) use details::present_brawler_details;

mod creation;
pub(in crate::client::flow) use creation::present_brawler_creation;

mod abilities;
mod choice_cards;
pub(in crate::client::flow) use abilities::present_brawler_editor;

pub(super) fn weapon_part_effect_text(effect: crate::weapon_parts::WeaponPartEffect) -> String {
    let modifier = |label: &str, flat: f64, percent: i16, unit: &str| {
        let mut values = Vec::new();
        if flat != 0.0 {
            values.push(format!("{flat:+}{unit}"));
        }
        if percent != 0 {
            values.push(format!("{:+}%", f32::from(percent) / 100.0));
        }
        format!("{label} {}", values.join(" / "))
    };
    match effect {
        crate::weapon_parts::WeaponPartEffect::Capacity {
            flat,
            percent_basis_points,
        } => modifier("Capacity", f64::from(flat), percent_basis_points, ""),
        crate::weapon_parts::WeaponPartEffect::Damage {
            flat,
            percent_basis_points,
        } => modifier("Damage", f64::from(flat), percent_basis_points, ""),
        crate::weapon_parts::WeaponPartEffect::FireInterval {
            flat_ticks,
            percent_basis_points,
        } => modifier(
            "Fire interval",
            f64::from(flat_ticks) / crate::timing::SIMULATION_TICK_HZ_F64,
            percent_basis_points,
            "s",
        ),
        crate::weapon_parts::WeaponPartEffect::RefillInterval {
            flat_ticks,
            percent_basis_points,
        } => modifier(
            "Refill interval",
            f64::from(flat_ticks) / crate::timing::SIMULATION_TICK_HZ_F64,
            percent_basis_points,
            "s",
        ),
        crate::weapon_parts::WeaponPartEffect::Reach {
            flat_milliunits,
            percent_basis_points,
        } => modifier(
            "Reach",
            f64::from(flat_milliunits) / 1_000.0,
            percent_basis_points,
            "",
        ),
        crate::weapon_parts::WeaponPartEffect::Slow {
            penalty_basis_points,
            duration_ticks,
        } => format!(
            "Slow {:.0}% for {:.1}s",
            f32::from(penalty_basis_points) / 100.0,
            crate::timing::simulation_seconds_f64(u32::from(duration_ticks))
        ),
        crate::weapon_parts::WeaponPartEffect::Cold { amount } => {
            format!("Cold +{amount}")
        }
        crate::weapon_parts::WeaponPartEffect::DamageOverTime {
            kind,
            damage_per_tick,
            tick_interval,
            duration_ticks,
        } => format!(
            "{kind:?} {damage_per_tick} every {:.1}s for {:.1}s",
            crate::timing::simulation_seconds_f64(u32::from(tick_interval)),
            crate::timing::simulation_seconds_f64(u32::from(duration_ticks))
        ),
        crate::weapon_parts::WeaponPartEffect::Heal { amount } => {
            format!("Allied healing {amount}")
        }
    }
}

pub(super) fn weapon_stat_rows(
    weapon: &crate::combat::ResolvedWeapon,
) -> Vec<(&'static str, String)> {
    let damage = weapon
        .recipe
        .payload_bundles
        .iter()
        .flat_map(|bundle| &bundle.effects)
        .find_map(|effect| match effect {
            crate::combat::PayloadEffectDefinition::Damage { amount, .. } => Some(*amount),
            _ => None,
        })
        .unwrap_or_default();
    let slow = weapon.recipe.payload_bundles.iter().any(|bundle| {
        bundle
            .effects
            .iter()
            .any(|effect| matches!(effect, crate::combat::PayloadEffectDefinition::Slow { .. }))
    });
    let reach = match weapon.recipe.delivery {
        crate::combat::DeliveryMethod::Straight { range, .. }
        | crate::combat::DeliveryMethod::StickyStraight { range, .. } => range,
        crate::combat::DeliveryMethod::Lobbed { distance, .. }
        | crate::combat::DeliveryMethod::Splash { distance, .. } => distance,
        crate::combat::DeliveryMethod::MeleeArc { reach, .. }
        | crate::combat::DeliveryMethod::ConeSpray { reach, .. } => reach,
    };
    let mut rows = vec![
        ("DAMAGE", damage.to_string()),
        ("CAPACITY", weapon.recipe.economy.capacity().to_string()),
        (
            "FIRE INTERVAL",
            format!(
                "{:.2}s",
                crate::timing::simulation_seconds_f64(
                    u32::try_from(weapon.recipe.fire_cooldown_ticks).unwrap_or(u32::MAX)
                )
            ),
        ),
        (
            "REFILL INTERVAL",
            format!(
                "{:.2}s",
                crate::timing::simulation_seconds_f64(
                    u32::try_from(weapon.recipe.economy.refill_ticks()).unwrap_or(u32::MAX)
                )
            ),
        ),
        ("REACH", format!("{reach:.0}")),
    ];
    if slow {
        rows.push(("SLOW", "YES".into()));
    }
    rows
}
