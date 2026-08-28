//! Saved-brawler browsing, creation, editing, equipment, and deletion presentation.
#![allow(clippy::wildcard_imports)]

use super::super::*;
use super::scroll::{clamp_scroll_offset, normalized_wheel_delta, offset_keeping_interval_visible};

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
            inline_error: None,
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

fn fighter_profile_stats(
    catalog: &crate::profiles::AdvertisedBrawlerCatalog,
    id: crate::profiles::FighterProfileId,
) -> Option<crate::builds::ResolvedFighterStats> {
    catalog.fighter(id).map(|definition| definition.stats)
}

fn spawn_brawler_list_row(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    brawler: &crate::profiles::SavedBrawler,
    selected: bool,
    catalog: &crate::profiles::AdvertisedBrawlerCatalog,
    disabled: bool,
) {
    let summary = brawler_loadout_summary(brawler, catalog);
    let status = if selected {
        " · SELECTED FOR PLAY"
    } else {
        ""
    };
    let mut row = parent.spawn((
        Button,
        AccessibleLabel::new(format!("{}{}: {summary}", brawler.name, status)),
        FlowButton {
            index,
            action: FlowUiAction::OpenBrawlerDetails(brawler.id),
            error_action: true,
        },
        Node {
            width: percent(100),
            min_height: px(92),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            justify_content: JustifyContent::Center,
            row_gap: px(4),
            padding: UiRect::axes(px(18), px(11)),
            border: UiRect::all(px(3)),
            border_radius: BorderRadius::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.09, 0.14, 0.2)),
        BorderColor::all(if selected {
            Color::srgb(0.2, 0.9, 0.72)
        } else {
            Color::NONE
        }),
    ));
    if disabled {
        row.insert(InteractionDisabled);
    }
    row.with_children(|row| {
        row.spawn((
            Text::new(format!(
                "{}{}",
                brawler.name.to_uppercase(),
                if selected {
                    "  ✓ SELECTED FOR PLAY"
                } else {
                    ""
                }
            )),
            TextFont::from_font_size(21.0),
            TextColor(if selected {
                Color::srgb(0.2, 0.95, 0.75)
            } else {
                Color::WHITE
            }),
        ));
        row.spawn((
            Text::new(summary),
            TextFont::from_font_size(15.0),
            TextColor(Color::srgb(0.72, 0.84, 0.94)),
        ));
    });
}

fn spawn_brawler_screen_button(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    label: &str,
    width: Val,
    disabled: bool,
    color: Color,
) {
    let mut button = parent.spawn((
        Button,
        FlowButton {
            index,
            action,
            error_action: true,
        },
        Node {
            width,
            min_height: px(52),
            padding: UiRect::axes(px(16), px(10)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(9)),
            ..default()
        },
        BackgroundColor(color),
        BorderColor::all(Color::srgb(0.38, 0.78, 1.0)),
    ));
    if disabled {
        button.insert(InteractionDisabled);
    }
    button.with_child((
        Text::new(label),
        TextFont::from_font_size(18.0),
        TextColor(Color::WHITE),
    ));
}

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn scroll_brawler_list(
    overlay: Res<ClientOverlay>,
    mut events: MessageReader<MouseWheel>,
    mut areas: Query<(&ComputedNode, &mut ScrollPosition), With<BrawlerListScrollArea>>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::BrawlerList) {
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
    clippy::too_many_lines,
    reason = "one cohesive bounded list builder keeps touch layout, navigation indices, and snapshot reconciliation adjacent"
)]
pub(in crate::client::flow) fn present_brawler_list(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    profile: Res<crate::client::ClientProfileModel>,
    roots: Query<(Entity, &BrawlerListRoot)>,
    scroll_areas: Query<&ScrollPosition, With<BrawlerListScrollArea>>,
    mut navigation: ResMut<FlowNavigation>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::BrawlerList)
        || *flow.get() != ClientFlow::Dashboard
    {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    let Some(snapshot) = profile.snapshot() else {
        return;
    };
    let Some(catalog) = profile.catalog() else {
        return;
    };
    let pending = profile.pending();
    let render_key = BrawlerListRoot {
        profile_revision: snapshot.revision,
        pending,
    };
    if roots.iter().any(|(_, root)| *root == render_key) {
        return;
    }
    let retained_scroll = scroll_areas.iter().next().cloned().unwrap_or_default();
    let first_render = roots.is_empty();
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    if first_render {
        navigation.selected = snapshot
            .selected_brawler_id
            .and_then(|selected| {
                snapshot
                    .brawlers
                    .iter()
                    .position(|brawler| brawler.id == selected)
            })
            .unwrap_or(0);
    }
    commands
        .spawn((
            render_key,
            DespawnOnExit(ClientFlow::Dashboard),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                ..default()
            },
            BackgroundColor(Color::srgb(0.018, 0.11, 0.24)),
            GlobalZIndex(510),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(100),
                    min_height: px(72),
                    align_items: AlignItems::Center,
                    column_gap: px(14),
                    padding: UiRect::axes(px(18), px(10)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.04, 0.085)),
            ))
            .with_children(|header| {
                spawn_brawler_screen_button(
                    header,
                    snapshot.brawlers.len() + 1,
                    FlowUiAction::CloseBrawlerList,
                    "‹ DASHBOARD",
                    px(170),
                    false,
                    Color::srgb(0.06, 0.22, 0.4),
                );
                header.spawn((
                    Text::new("BRAWLERS"),
                    TextFont::from_font_size(34.0),
                    TextColor(Color::WHITE),
                ));
                header.spawn((Node {
                    flex_grow: 1.0,
                    ..default()
                },));
                header.spawn((
                    Text::new(format!(
                        "{} / {} SAVED",
                        snapshot.brawlers.len(),
                        catalog.limits.maximum_saved_brawlers
                    )),
                    TextFont::from_font_size(16.0),
                    TextColor(Color::srgb(0.42, 0.88, 1.0)),
                ));
            });
            root.spawn((
                Node {
                    width: percent(100),
                    padding: UiRect::axes(px(24), px(14)),
                    align_items: AlignItems::Center,
                    column_gap: px(16),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.025, 0.2, 0.42)),
            ))
            .with_children(|intro| {
                intro.spawn((
                    Text::new("CHOOSE YOUR BRAWLER"),
                    TextFont::from_font_size(22.0),
                    TextColor(Color::WHITE),
                ));
                intro.spawn((
                    Text::new("Tap any saved brawler to inspect its build."),
                    TextFont::from_font_size(15.0),
                    TextColor(Color::srgb(0.72, 0.88, 0.98)),
                ));
            });
            root.spawn((Node {
                width: percent(100),
                min_height: px(0),
                flex_grow: 1.0,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(22), px(12)),
                ..default()
            },))
                .with_children(|body| {
                    body.spawn((
                        BrawlerListScrollArea,
                        retained_scroll,
                        Node {
                            width: percent(100),
                            max_width: px(1040),
                            min_height: px(0),
                            flex_grow: 1.0,
                            flex_shrink: 1.0,
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Stretch,
                            row_gap: px(11),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                    ))
                    .with_children(|list| {
                        if snapshot.brawlers.is_empty() {
                            list.spawn((
                                Text::new("NO BRAWLERS YET\nCreate one to enter Play or Practice."),
                                TextFont::from_font_size(21.0),
                                TextColor(Color::srgb(0.78, 0.86, 0.94)),
                            ));
                        }
                        for (index, brawler) in snapshot.brawlers.iter().enumerate() {
                            spawn_brawler_list_row(
                                list,
                                index,
                                brawler,
                                snapshot.selected_brawler_id == Some(brawler.id),
                                catalog,
                                pending,
                            );
                        }
                    });
                });
            root.spawn((
                Node {
                    width: percent(100),
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(px(22), px(12)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.055, 0.11)),
            ))
            .with_children(|footer| {
                footer
                    .spawn((Node {
                        width: percent(100),
                        max_width: px(1040),
                        ..default()
                    },))
                    .with_children(|actions| {
                        let create_index = snapshot.brawlers.len();
                        spawn_brawler_screen_button(
                            actions,
                            create_index,
                            FlowUiAction::CreateBrawler,
                            "+ CREATE BRAWLER",
                            percent(100),
                            pending
                                || snapshot.brawlers.len()
                                    >= usize::from(catalog.limits.maximum_saved_brawlers),
                            Color::srgb(0.03, 0.5, 0.78),
                        );
                    });
            });
        });
}

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn keep_brawler_list_focus_visible(
    overlay: Res<ClientOverlay>,
    navigation: Res<FlowNavigation>,
    buttons: Query<(&FlowButton, &ChildOf, &ComputedNode, &UiGlobalTransform)>,
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
    if !matches!(overlay.as_ref(), ClientOverlay::BrawlerList) {
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
        buttons.iter().find(|(button, child_of, _, _)| {
            child_of.parent() == area_entity && button.index == navigation.selected
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

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one cohesive bounded detail builder keeps resolved stats, touch actions, and navigation indices adjacent"
)]
pub(in crate::client::flow) fn present_brawler_details(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    profile: Res<crate::client::ClientProfileModel>,
    mut screen: BrawlerScreenUi,
    roots: Query<(Entity, &BrawlerDetailsRoot)>,
) {
    let (brawler_id, contextual_confirmation) = match overlay.as_ref() {
        ClientOverlay::BrawlerDetails(brawler_id) => (brawler_id, false),
        ClientOverlay::DeleteBrawlerConfirmation(brawler_id) => (brawler_id, true),
        _ => {
            for (entity, _) in &roots {
                commands.entity(entity).despawn();
            }
            return;
        }
    };
    if *flow.get() != ClientFlow::Dashboard {
        return;
    }
    let Some(snapshot) = profile.snapshot() else {
        return;
    };
    let Some(catalog) = profile.catalog() else {
        return;
    };
    let Some(brawler) = snapshot
        .brawlers
        .iter()
        .find(|brawler| brawler.id == *brawler_id)
    else {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    };
    let layout = brawler_screen_layout(&screen);
    let render_key = BrawlerDetailsRoot {
        brawler_id: *brawler_id,
        profile_revision: snapshot.revision,
        pending: profile.pending(),
        contextual_confirmation,
        layout,
    };
    if roots.iter().any(|(_, root)| *root == render_key) {
        return;
    }
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    screen.navigation.selected = 0;
    let compact = layout == BrawlerDetailsLayout::Compact;
    let selected = snapshot.selected_brawler_id == Some(brawler.id);
    let stats = fighter_profile_stats(catalog, brawler.fighter_profile_id);
    let fighter_copy = stats.map_or_else(
        || "Fighter statistics unavailable".to_string(),
        |stats| {
            format!(
                "Health {} · Speed {:.0} · Reveal distance {:.0}",
                stats.maximum_health, stats.movement_speed, stats.reveal_proximity_radius
            )
        },
    );
    let resolved_weapon = snapshot
        .weapon_modifiers(brawler)
        .ok()
        .and_then(|modifiers| {
            let fighters = crate::combat::FighterDefinitions::default();
            let weapon = catalog.weapon(brawler.weapon_base_id)?;
            crate::weapon_parts::resolve_advertised_weapon_parts(
                &weapon.configuration,
                &catalog.weapon_policy,
                &fighters.entries[0],
                crate::combat::WeaponPresetId(brawler.weapon_base_id.0),
                modifiers,
            )
            .ok()
        });
    let weapon_copy = resolved_weapon.as_ref().map_or_else(
        || "Weapon statistics unavailable".to_string(),
        weapon_preview_text,
    );
    let equipped = brawler.equipped_part_ids.iter().flatten().count();
    let pending = profile.pending();
    let controls_disabled = pending || contextual_confirmation;
    commands
        .spawn((
            render_key,
            DespawnOnExit(ClientFlow::Dashboard),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                ..default()
            },
            BackgroundColor(Color::srgb(0.018, 0.13, 0.29)),
            GlobalZIndex(510),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(100),
                    min_height: px(72),
                    align_items: AlignItems::Center,
                    column_gap: px(16),
                    padding: UiRect::axes(px(18), px(10)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.04, 0.085)),
            ))
            .with_children(|header| {
                spawn_brawler_screen_button(
                    header,
                    4,
                    FlowUiAction::BackToBrawlerList,
                    "‹ BRAWLERS",
                    px(160),
                    contextual_confirmation,
                    Color::srgb(0.06, 0.22, 0.4),
                );
                header.spawn((
                    Text::new(brawler.name.to_uppercase()),
                    TextFont::from_font_size(if compact { 27.0 } else { 34.0 }),
                    TextColor(Color::WHITE),
                ));
                header.spawn((
                    Node {
                        flex_grow: 1.0,
                        ..default()
                    },
                ));
                if selected {
                    header.spawn((
                        Text::new("✓ SELECTED FOR PLAY"),
                        TextFont::from_font_size(16.0),
                        TextColor(Color::srgb(0.2, 0.95, 0.75)),
                    ));
                }
            });
            root.spawn((
                BrawlerDetailsScrollArea,
                Node {
                    width: percent(100),
                    min_height: px(0),
                    flex_grow: 1.0,
                    flex_direction: if compact {
                        FlexDirection::Column
                    } else {
                        FlexDirection::Row
                    },
                    align_items: AlignItems::Stretch,
                    justify_content: JustifyContent::Center,
                    row_gap: px(16),
                    column_gap: px(20),
                    padding: UiRect::all(if compact { px(14) } else { px(24) }),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollPosition::default(),
            ))
            .with_children(|body| {
                body.spawn((
                    Node {
                        width: if compact { percent(100) } else { percent(27) },
                        max_width: if compact { Val::Auto } else { px(360) },
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Stretch,
                        row_gap: px(12),
                        padding: UiRect::all(px(18)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.03, 0.22, 0.46)),
                ))
                .with_children(|identity| {
                    identity.spawn((
                        Text::new("FIGHTER"),
                        TextFont::from_font_size(15.0),
                        TextColor(Color::srgb(0.38, 0.88, 1.0)),
                    ));
                    identity.spawn((
                        Text::new(advertised_fighter_name(catalog, brawler.fighter_profile_id)),
                        TextFont::from_font_size(28.0),
                        TextColor(Color::WHITE),
                    ));
                    identity.spawn((
                        Text::new("PERMANENT BASE"),
                        TextFont::from_font_size(13.0),
                        TextColor(Color::srgb(1.0, 0.78, 0.25)),
                    ));
                    identity.spawn((
                        Text::new(fighter_copy),
                        TextFont::from_font_size(17.0),
                        TextColor(Color::srgb(0.82, 0.91, 0.98)),
                    ));
                });
                body.spawn((
                    BrawlerDetailsPreviewHost,
                    AccessibleLabel::new(format!("3D preview of {}", brawler.name)),
                    Node {
                        width: if compact { percent(100) } else { percent(42) },
                        max_width: if compact { Val::Auto } else { px(680) },
                        height: if compact { px(360) } else { percent(100) },
                        min_height: px(340),
                        flex_grow: if compact { 0.0 } else { 1.0 },
                        flex_shrink: 0.0,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::End,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.015, 0.3, 0.58)),
                ));
                body.spawn((
                    Node {
                        width: if compact { percent(100) } else { percent(31) },
                        max_width: if compact { Val::Auto } else { px(420) },
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Stretch,
                        row_gap: px(10),
                        padding: UiRect::all(px(18)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.025, 0.075, 0.15)),
                ))
                .with_children(|loadout| {
                    loadout.spawn((
                        Text::new("LOADOUT"),
                        TextFont::from_font_size(24.0),
                        TextColor(Color::WHITE),
                    ));
                    loadout.spawn((
                        Text::new(format!(
                            "WEAPON\n{} · PERMANENT\n{}\n\nULTIMATE\n{}\n\nPASSIVES\n{}\n{}\n\nWEAPON PARTS\n{equipped}/{} EQUIPPED",
                            advertised_weapon_name(catalog, brawler.weapon_base_id),
                            weapon_copy,
                            ultimate_name(catalog, brawler.ultimate_id),
                            passive_name(catalog, brawler.passive_ids[0]),
                            passive_name(catalog, brawler.passive_ids[1]),
                            crate::weapon_parts::WEAPON_PART_SLOT_COUNT,
                        )),
                        TextFont::from_font_size(15.0),
                        TextColor(Color::srgb(0.78, 0.88, 0.96)),
                    ));
                    spawn_brawler_screen_button(
                        loadout,
                        0,
                        FlowUiAction::SelectBrawler(brawler.id),
                        if selected {
                            "SELECTED FOR PLAY"
                        } else if profile.selection_pending(brawler.id) {
                            "SELECTING..."
                        } else {
                            "SELECT FOR PLAY"
                        },
                        percent(100),
                        controls_disabled,
                        Color::srgb(0.04, 0.62, 0.38),
                    );
                    spawn_brawler_screen_button(
                        loadout,
                        1,
                        FlowUiAction::OpenBrawlerEditor(brawler.id),
                        "CUSTOMIZE ABILITIES",
                        percent(100),
                        controls_disabled,
                        Color::srgb(0.04, 0.42, 0.72),
                    );
                    spawn_brawler_screen_button(
                        loadout,
                        2,
                        FlowUiAction::OpenWeaponEquipment(brawler.id),
                        "CUSTOMIZE WEAPON",
                        percent(100),
                        controls_disabled,
                        Color::srgb(0.04, 0.42, 0.72),
                    );
                    spawn_brawler_screen_button(
                        loadout,
                        3,
                        FlowUiAction::DeleteBrawler(brawler.id),
                        "DELETE BRAWLER",
                        percent(100),
                        controls_disabled,
                        Color::srgb(0.48, 0.08, 0.12),
                    );
                });
            });
        });
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    reason = "one bounded creation-screen builder owns its touch controls and inline mutation state"
)]
pub(in crate::client::flow) fn present_brawler_creation(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    draft: Res<BrawlerCreationDraft>,
    profile: Res<crate::client::ClientProfileModel>,
    mut screen: BrawlerScreenUi,
    roots: Query<(Entity, &BrawlerCreationRoot)>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::BrawlerCreation)
        || *flow.get() != ClientFlow::Dashboard
    {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    let Some(catalog) = profile.catalog() else {
        return;
    };
    let layout = brawler_screen_layout(&screen);
    let render_key = BrawlerCreationRoot {
        draft: draft.clone(),
        layout,
    };
    if roots.iter().any(|(_, root)| *root == render_key) {
        return;
    }
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    screen.navigation.selected = 0;
    let compact = layout == BrawlerDetailsLayout::Compact;
    commands
        .spawn((
            render_key,
            DespawnOnExit(ClientFlow::Dashboard),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                ..default()
            },
            BackgroundColor(Color::srgb(0.018, 0.11, 0.24)),
            GlobalZIndex(510),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(100),
                    min_height: px(72),
                    align_items: AlignItems::Center,
                    column_gap: px(16),
                    padding: UiRect::axes(px(18), px(10)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.04, 0.085)),
            ))
            .with_children(|header| {
                spawn_brawler_screen_button(
                    header,
                    4,
                    FlowUiAction::CancelCreateBrawler,
                    "‹ BRAWLERS",
                    px(170),
                    false,
                    Color::srgb(0.06, 0.22, 0.4),
                );
                header.spawn((
                    Text::new("CREATE BRAWLER"),
                    TextFont::from_font_size(if compact { 27.0 } else { 34.0 }),
                    TextColor(Color::WHITE),
                ));
            });
            root.spawn((Node {
                width: percent(100),
                min_height: px(0),
                flex_grow: 1.0,
                flex_direction: if compact {
                    FlexDirection::Column
                } else {
                    FlexDirection::Row
                },
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::Center,
                row_gap: px(16),
                column_gap: px(18),
                padding: UiRect::all(if compact { px(14) } else { px(28) }),
                overflow: Overflow::scroll_y(),
                ..default()
            },))
            .with_children(|body| {
                body.spawn((
                    Node {
                        width: if compact { percent(100) } else { percent(31) },
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Stretch,
                        row_gap: px(12),
                        padding: UiRect::all(px(20)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.03, 0.22, 0.46)),
                ))
                .with_children(|card| {
                    card.spawn((
                        Text::new("FIGHTER PROFILE"),
                        TextFont::from_font_size(18.0),
                        TextColor(Color::srgb(0.38, 0.88, 1.0)),
                    ));
                    card.spawn((
                        Text::new("Defines health, speed, and reveal distance. Permanent after creation."),
                        TextFont::from_font_size(14.0),
                        TextColor(Color::srgb(0.76, 0.86, 0.95)),
                    ));
                    spawn_brawler_screen_button(
                        card,
                        0,
                        FlowUiAction::CycleCreationProfile,
                        advertised_fighter_name(catalog, draft.fighter_profile_id),
                        percent(100),
                        false,
                        Color::srgb(0.04, 0.42, 0.72),
                    );
                });
                body.spawn((
                    Node {
                        width: if compact { percent(100) } else { percent(31) },
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Stretch,
                        row_gap: px(12),
                        padding: UiRect::all(px(20)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.025, 0.16, 0.34)),
                ))
                .with_children(|card| {
                    card.spawn((
                        Text::new("WEAPON BASE"),
                        TextFont::from_font_size(18.0),
                        TextColor(Color::srgb(1.0, 0.78, 0.25)),
                    ));
                    card.spawn((
                        Text::new("Defines the permanent weapon family. Parts remain customizable."),
                        TextFont::from_font_size(14.0),
                        TextColor(Color::srgb(0.76, 0.86, 0.95)),
                    ));
                    spawn_brawler_screen_button(
                        card,
                        1,
                        FlowUiAction::CycleCreationWeapon,
                        advertised_weapon_name(catalog, draft.weapon_base_id),
                        percent(100),
                        false,
                        Color::srgb(0.12, 0.35, 0.62),
                    );
                });
                body.spawn((
                    Node {
                        width: if compact { percent(100) } else { percent(31) },
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Stretch,
                        row_gap: px(12),
                        padding: UiRect::all(px(20)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.08, 0.12, 0.28)),
                ))
                .with_children(|card| {
                    card.spawn((
                        Text::new("STARTING ULTIMATE"),
                        TextFont::from_font_size(18.0),
                        TextColor(Color::srgb(0.82, 0.62, 1.0)),
                    ));
                    card.spawn((
                        Text::new("The ultimate and both passives can be changed later."),
                        TextFont::from_font_size(14.0),
                        TextColor(Color::srgb(0.76, 0.86, 0.95)),
                    ));
                    spawn_brawler_screen_button(
                        card,
                        2,
                        FlowUiAction::CycleCreationUltimate,
                        ultimate_name(catalog, draft.ultimate),
                        percent(100),
                        false,
                        Color::srgb(0.34, 0.18, 0.62),
                    );
                });
            });
            root.spawn((
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: px(16),
                    padding: UiRect::axes(px(22), px(12)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.055, 0.11)),
            ))
            .with_children(|footer| {
                if let Some(error) = &draft.inline_error {
                    footer.spawn((
                        Text::new(error),
                        TextFont::from_font_size(14.0),
                        TextColor(Color::srgb(1.0, 0.5, 0.45)),
                    ));
                }
                spawn_brawler_screen_button(
                    footer,
                    3,
                    FlowUiAction::ConfirmCreateBrawler,
                    "CREATE BRAWLER",
                    if compact { percent(100) } else { px(420) },
                    profile.pending(),
                    Color::srgb(0.03, 0.58, 0.4),
                );
            });
        });
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    reason = "one cohesive Bevy overlay builder keeps editor layout and navigation indices adjacent"
)]
pub(in crate::client::flow) fn present_brawler_editor(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    draft: Res<BrawlerEditDraft>,
    profile: Res<crate::client::ClientProfileModel>,
    mut screen: BrawlerScreenUi,
    roots: Query<(Entity, &BrawlerEditorRoot)>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::BrawlerEditor)
        || *flow.get() != ClientFlow::Dashboard
    {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    let Some(catalog) = profile.catalog() else {
        return;
    };
    let layout = brawler_screen_layout(&screen);
    let render_key = BrawlerEditorRoot {
        draft: draft.clone(),
        layout,
    };
    if roots.iter().any(|(_, root)| *root == render_key) {
        return;
    }
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    screen.navigation.selected = 0;
    let compact = layout == BrawlerDetailsLayout::Compact;
    let name = if draft.editing_name {
        let caret = draft.name_caret.min(draft.name.len());
        format!("{}|{}", &draft.name[..caret], &draft.name[caret..])
    } else {
        draft.name.clone()
    };
    commands
        .spawn((
            render_key,
            DespawnOnExit(ClientFlow::Dashboard),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                ..default()
            },
            BackgroundColor(Color::srgb(0.018, 0.11, 0.24)),
            GlobalZIndex(510),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(100),
                    min_height: px(72),
                    align_items: AlignItems::Center,
                    column_gap: px(16),
                    padding: UiRect::axes(px(18), px(10)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.04, 0.085)),
            ))
            .with_children(|header| {
                spawn_brawler_screen_button(
                    header,
                    5,
                    FlowUiAction::CancelBrawlerEdit,
                    "‹ BRAWLER",
                    px(170),
                    false,
                    Color::srgb(0.06, 0.22, 0.4),
                );
                header.spawn((
                    Text::new("CUSTOMIZE ABILITIES"),
                    TextFont::from_font_size(if compact { 27.0 } else { 34.0 }),
                    TextColor(Color::WHITE),
                ));
            });
            root.spawn((Node {
                width: percent(100),
                min_height: px(0),
                flex_grow: 1.0,
                flex_direction: if compact {
                    FlexDirection::Column
                } else {
                    FlexDirection::Row
                },
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::Center,
                row_gap: px(16),
                column_gap: px(20),
                padding: UiRect::all(if compact { px(14) } else { px(28) }),
                overflow: Overflow::scroll_y(),
                ..default()
            },))
            .with_children(|body| {
                body.spawn((
                    Node {
                        width: if compact { percent(100) } else { percent(32) },
                        max_width: if compact { Val::Auto } else { px(420) },
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Stretch,
                        row_gap: px(12),
                        padding: UiRect::all(px(22)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.03, 0.22, 0.46)),
                ))
                .with_children(|identity| {
                    identity.spawn((
                        Text::new("IDENTITY"),
                        TextFont::from_font_size(18.0),
                        TextColor(Color::srgb(0.38, 0.88, 1.0)),
                    ));
                    identity.spawn((
                        Text::new(format!(
                            "{}\n{}\n\nFighter profile and weapon base are permanent.",
                            advertised_fighter_name(catalog, draft.fighter_profile_id),
                            advertised_weapon_name(catalog, draft.weapon_base_id)
                        )),
                        TextFont::from_font_size(16.0),
                        TextColor(Color::srgb(0.8, 0.9, 0.98)),
                    ));
                    spawn_brawler_screen_button(
                        identity,
                        0,
                        FlowUiAction::BeginBrawlerNameEdit,
                        &format!("NAME · {name}"),
                        percent(100),
                        false,
                        Color::srgb(0.06, 0.35, 0.62),
                    );
                });
                body.spawn((
                    Node {
                        width: if compact { percent(100) } else { percent(68) },
                        max_width: if compact { Val::Auto } else { px(760) },
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Stretch,
                        row_gap: px(14),
                        padding: UiRect::all(px(22)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.025, 0.075, 0.15)),
                ))
                .with_children(|abilities| {
                    abilities.spawn((
                        Text::new("ACTIVE LOADOUT"),
                        TextFont::from_font_size(18.0),
                        TextColor(Color::srgb(0.82, 0.62, 1.0)),
                    ));
                    abilities.spawn((
                        Text::new("Tap a slot to cycle through the choices advertised by this server."),
                        TextFont::from_font_size(14.0),
                        TextColor(Color::srgb(0.72, 0.84, 0.94)),
                    ));
                    spawn_brawler_screen_button(
                        abilities,
                        1,
                        FlowUiAction::CycleBrawlerUltimate,
                        &format!("ULTIMATE\n{}", ultimate_name(catalog, draft.ultimate_id)),
                        percent(100),
                        false,
                        Color::srgb(0.34, 0.18, 0.62),
                    );
                    spawn_brawler_screen_button(
                        abilities,
                        2,
                        FlowUiAction::CycleBrawlerPassiveOne,
                        &format!("PASSIVE 1\n{}", passive_name(catalog, draft.passive_ids[0])),
                        percent(100),
                        false,
                        Color::srgb(0.08, 0.34, 0.5),
                    );
                    spawn_brawler_screen_button(
                        abilities,
                        3,
                        FlowUiAction::CycleBrawlerPassiveTwo,
                        &format!("PASSIVE 2\n{}", passive_name(catalog, draft.passive_ids[1])),
                        percent(100),
                        false,
                        Color::srgb(0.08, 0.34, 0.5),
                    );
                });
            });
            root.spawn((
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: px(16),
                    padding: UiRect::axes(px(22), px(12)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.055, 0.11)),
            ))
            .with_children(|footer| {
                if let Some(error) = &draft.inline_error {
                    footer.spawn((
                        Text::new(error),
                        TextFont::from_font_size(14.0),
                        TextColor(Color::srgb(1.0, 0.5, 0.45)),
                    ));
                }
                spawn_brawler_screen_button(
                    footer,
                    4,
                    FlowUiAction::ConfirmBrawlerEdit,
                    "SAVE CHANGES",
                    if compact { percent(100) } else { px(420) },
                    profile.pending(),
                    Color::srgb(0.03, 0.58, 0.4),
                );
            });
        });
}

pub(in crate::client::flow) fn scroll_weapon_equipment(
    mut wheel: MessageReader<MouseWheel>,
    mut areas: Query<(&ComputedNode, &mut ScrollPosition), With<WeaponEquipmentScrollArea>>,
) {
    let delta = normalized_wheel_delta(wheel.read(), 24.0);
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
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "one cohesive Bevy overlay builder renders the four slots, bounded inventory, and live preview"
)]
pub(in crate::client::flow) fn present_weapon_equipment(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    draft: Res<WeaponEquipmentDraft>,
    roots: Query<(Entity, &WeaponEquipmentRoot)>,
    scroll_areas: Query<&ScrollPosition, With<WeaponEquipmentScrollArea>>,
    profile: Res<crate::client::ClientProfileModel>,
    parts: Res<crate::weapon_parts::WeaponPartCatalogResource>,
    mut screen: BrawlerScreenUi,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::WeaponEquipment)
        || *flow.get() != ClientFlow::Dashboard
    {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    let layout = brawler_screen_layout(&screen);
    let render_key = WeaponEquipmentRoot {
        draft: draft.clone(),
        layout,
    };
    let existing = roots.iter().next();
    if existing.is_some_and(|(_, root)| *root == render_key) {
        return;
    }
    let retained_scroll = scroll_areas.iter().next().cloned().unwrap_or_default();
    let first_render = existing.is_none();
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    if first_render {
        screen.navigation.selected = draft.selected_slot;
    }
    let Some(snapshot) = profile.snapshot() else {
        return;
    };
    let Some(catalog) = profile.catalog() else {
        return;
    };
    let Some(brawler_id) = draft.brawler_id else {
        return;
    };
    let Some(saved) = snapshot.brawlers.iter().find(|item| item.id == brawler_id) else {
        return;
    };
    let mut candidate = snapshot.clone();
    if let Some(candidate_brawler) = candidate
        .brawlers
        .iter_mut()
        .find(|item| item.id == brawler_id)
    {
        candidate_brawler.equipped_part_ids = draft.equipped_part_ids;
    } else {
        return;
    }
    let Some(candidate_brawler) = candidate.brawlers.iter().find(|item| item.id == brawler_id)
    else {
        return;
    };
    let resolved_preview =
        candidate
            .weapon_modifiers(candidate_brawler)
            .ok()
            .and_then(|modifiers| {
                let fighters = crate::combat::FighterDefinitions::default();
                let weapon = catalog.weapon(saved.weapon_base_id)?;
                crate::weapon_parts::resolve_advertised_weapon_parts(
                    &weapon.configuration,
                    &catalog.weapon_policy,
                    &fighters.entries[0],
                    crate::combat::WeaponPresetId(saved.weapon_base_id.0),
                    modifiers,
                )
                .ok()
            });
    let preview_valid = resolved_preview.is_some();
    let preview = resolved_preview.map_or_else(
        || "INVALID PART COMBINATION".into(),
        |weapon| weapon_preview_text(&weapon),
    );
    let compact = layout == BrawlerDetailsLayout::Compact;
    let save_index = 5 + snapshot.inventory.len();

    commands
        .spawn((
            render_key,
            DespawnOnExit(ClientFlow::Dashboard),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                ..default()
            },
            BackgroundColor(Color::srgb(0.018, 0.11, 0.24)),
            GlobalZIndex(520),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(100),
                    min_height: px(72),
                    align_items: AlignItems::Center,
                    column_gap: px(16),
                    padding: UiRect::axes(px(18), px(10)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.04, 0.085)),
            ))
            .with_children(|header| {
                spawn_brawler_screen_button(
                    header,
                    save_index + 1,
                    FlowUiAction::CancelWeaponEquipment,
                    "‹ BRAWLER",
                    px(170),
                    false,
                    Color::srgb(0.06, 0.22, 0.4),
                );
                header.spawn((
                    Text::new("CUSTOMIZE WEAPON"),
                    TextFont::from_font_size(if compact { 27.0 } else { 34.0 }),
                    TextColor(Color::WHITE),
                ));
            });
            root.spawn((
                Node {
                    width: percent(100),
                    padding: UiRect::axes(px(24), px(12)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.025, 0.2, 0.42)),
            ))
            .with_children(|summary| {
                summary.spawn((
                    Text::new(format!(
                        "{}  ·  {}",
                        advertised_weapon_name(catalog, saved.weapon_base_id),
                        preview
                    )),
                    TextFont::from_font_size(16.0),
                    TextColor(Color::srgb(0.72, 0.9, 1.0)),
                ));
            });
            root.spawn((Node {
                width: percent(100),
                min_height: px(0),
                flex_grow: 1.0,
                flex_direction: if compact {
                    FlexDirection::Column
                } else {
                    FlexDirection::Row
                },
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::Center,
                row_gap: px(14),
                column_gap: px(18),
                padding: UiRect::all(if compact { px(12) } else { px(22) }),
                ..default()
            },))
                .with_children(|body| {
                    body.spawn((
                        Node {
                            width: if compact { percent(100) } else { percent(36) },
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Stretch,
                            row_gap: px(9),
                            padding: UiRect::all(px(18)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.03, 0.22, 0.46)),
                    ))
                    .with_children(|slots| {
                        slots.spawn((
                            Text::new("EQUIPPED SLOTS"),
                            TextFont::from_font_size(18.0),
                            TextColor(Color::srgb(0.38, 0.88, 1.0)),
                        ));
                        for slot in 0..crate::weapon_parts::WEAPON_PART_SLOT_COUNT {
                            let label = draft.equipped_part_ids[slot]
                                .and_then(|id| snapshot.inventory.iter().find(|part| part.id == id))
                                .map_or_else(
                                    || format!("SLOT {} · EMPTY", slot + 1),
                                    |part| format!("SLOT {} · {}", slot + 1, part.display_name),
                                );
                            spawn_brawler_screen_button(
                                slots,
                                slot,
                                FlowUiAction::SelectEquipmentSlot(slot),
                                &if slot == draft.selected_slot {
                                    format!("> {label}")
                                } else {
                                    label
                                },
                                percent(100),
                                false,
                                if slot == draft.selected_slot {
                                    Color::srgb(0.04, 0.5, 0.66)
                                } else {
                                    Color::srgb(0.06, 0.27, 0.48)
                                },
                            );
                        }
                        spawn_brawler_screen_button(
                            slots,
                            4,
                            FlowUiAction::UnequipWeaponPart,
                            "UNEQUIP SELECTED SLOT",
                            percent(100),
                            false,
                            Color::srgb(0.35, 0.18, 0.24),
                        );
                    });
                    body.spawn((
                        WeaponEquipmentScrollArea,
                        retained_scroll,
                        Node {
                            width: if compact { percent(100) } else { percent(64) },
                            min_height: px(0),
                            flex_grow: 1.0,
                            flex_shrink: 1.0,
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Stretch,
                            row_gap: px(9),
                            padding: UiRect::all(px(18)),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.025, 0.075, 0.15)),
                    ))
                    .with_children(|scroll| {
                        scroll.spawn((
                            Text::new("OWNED PARTS"),
                            TextFont::from_font_size(18.0),
                            TextColor(Color::srgb(1.0, 0.82, 0.38)),
                        ));
                        for (index, part) in snapshot.inventory.iter().enumerate() {
                            let presentation = parts
                                .0
                                .definition(part.definition_id)
                                .map_or("Part", |definition| definition.presentation_type.as_str());
                            let equipped_elsewhere = snapshot.brawlers.iter().find(|brawler| {
                                brawler.id != brawler_id
                                    && brawler.equipped_part_ids.contains(&Some(part.id))
                            });
                            let availability = equipped_elsewhere
                                .map(|brawler| format!(" · EQUIPPED BY {}", brawler.name))
                                .unwrap_or_default();
                            let effects = part
                                .effects
                                .iter()
                                .map(|effect| weapon_part_effect_text(*effect))
                                .collect::<Vec<_>>()
                                .join(" · ");
                            spawn_brawler_screen_button(
                                scroll,
                                5 + index,
                                FlowUiAction::EquipWeaponPart(part.id),
                                &format!(
                                    "{} [{}] — {}{}",
                                    part.display_name, presentation, effects, availability
                                ),
                                percent(100),
                                false,
                                Color::srgb(0.09, 0.18, 0.28),
                            );
                        }
                        if let Some(error) = &draft.inline_error {
                            scroll.spawn((
                                Text::new(error),
                                TextFont::from_font_size(14.0),
                                TextColor(Color::srgb(1.0, 0.5, 0.45)),
                            ));
                        }
                    });
                });
            root.spawn((
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: px(16),
                    padding: UiRect::axes(px(22), px(12)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.055, 0.11)),
            ))
            .with_children(|footer| {
                spawn_brawler_screen_button(
                    footer,
                    save_index,
                    FlowUiAction::ConfirmWeaponEquipment,
                    "SAVE EQUIPMENT",
                    if compact { percent(100) } else { px(420) },
                    !preview_valid || profile.pending(),
                    Color::srgb(0.03, 0.58, 0.4),
                );
            });
        });
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "computed UI bounds are available only after Bevy's layout pass"
)]
pub(in crate::client::flow) fn keep_weapon_equipment_focus_visible(
    overlay: Res<ClientOverlay>,
    navigation: Res<FlowNavigation>,
    buttons: Query<(&FlowButton, &ChildOf, &ComputedNode, &UiGlobalTransform)>,
    mut areas: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            &mut ScrollPosition,
        ),
        With<WeaponEquipmentScrollArea>,
    >,
    mut prior: Local<Option<(Entity, usize)>>,
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
    if prior.as_ref() == Some(&focus_key) {
        return;
    }
    *prior = Some(focus_key);
    let Some((_, _, button_node, button_transform)) =
        buttons.iter().find(|(button, child_of, _, _)| {
            child_of.parent() == area_entity && button.index == navigation.selected
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

fn weapon_part_effect_text(effect: crate::weapon_parts::WeaponPartEffect) -> String {
    let percent = |value: i16| format!("{:+}%", f32::from(value) / 100.0);
    match effect {
        crate::weapon_parts::WeaponPartEffect::Capacity {
            flat,
            percent_basis_points,
        } => format!("capacity {flat:+} {}", percent(percent_basis_points)),
        crate::weapon_parts::WeaponPartEffect::Damage {
            flat,
            percent_basis_points,
        } => format!("damage {flat:+} {}", percent(percent_basis_points)),
        crate::weapon_parts::WeaponPartEffect::FireInterval {
            flat_ticks,
            percent_basis_points,
        } => format!(
            "fire interval {flat_ticks:+}t {}",
            percent(percent_basis_points)
        ),
        crate::weapon_parts::WeaponPartEffect::RefillInterval {
            flat_ticks,
            percent_basis_points,
        } => format!("refill {flat_ticks:+}t {}", percent(percent_basis_points)),
        crate::weapon_parts::WeaponPartEffect::Reach {
            flat_milliunits,
            percent_basis_points,
        } => format!(
            "reach {:+.1} {}",
            f64::from(flat_milliunits) / 1_000.0,
            percent(percent_basis_points)
        ),
        crate::weapon_parts::WeaponPartEffect::Slow {
            penalty_basis_points,
            duration_ticks,
        } => format!(
            "Slow {:.0}%/{duration_ticks}t",
            f32::from(penalty_basis_points) / 100.0
        ),
    }
}

fn weapon_preview_text(weapon: &crate::combat::ResolvedWeapon) -> String {
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
        crate::combat::DeliveryMethod::Straight { range, .. } => range,
        crate::combat::DeliveryMethod::Lobbed { distance, .. } => distance,
        crate::combat::DeliveryMethod::MeleeArc { reach, .. } => reach,
    };
    format!(
        "Capacity {} · Damage {} · Fire {}t · Refill {}t · Reach {:.0}{}",
        weapon.recipe.economy.capacity(),
        damage,
        weapon.recipe.fire_cooldown_ticks,
        weapon.recipe.economy.refill_ticks(),
        reach,
        if slow { " · Slow" } else { "" }
    )
}

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn present_delete_brawler_confirmation(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    roots: Query<Entity, With<DeleteBrawlerConfirmationRoot>>,
    profile: Res<crate::client::ClientProfileModel>,
    mut navigation: ResMut<FlowNavigation>,
) {
    let ClientOverlay::DeleteBrawlerConfirmation(brawler_id) = overlay.as_ref() else {
        for entity in &roots {
            commands.entity(entity).despawn();
        }
        return;
    };
    if !roots.is_empty() || *flow.get() != ClientFlow::Dashboard {
        return;
    }
    let name = profile
        .snapshot()
        .and_then(|snapshot| {
            snapshot
                .brawlers
                .iter()
                .find(|brawler| brawler.id == *brawler_id)
        })
        .map_or("this brawler", |brawler| brawler.name.as_str());
    navigation.selected = 0;
    commands
        .spawn((
            DeleteBrawlerConfirmationRoot,
            DespawnOnExit(ClientFlow::Dashboard),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.78)),
            GlobalZIndex(610),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(84),
                    max_width: px(520),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(12),
                    padding: UiRect::all(px(24)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.055, 0.08, 0.12)),
            ))
            .with_children(|panel| {
                spawn_heading(panel, "DELETE BRAWLER?");
                panel.spawn((
                    Text::new(format!("Delete {name}? This cannot be undone.")),
                    TextColor(Color::srgb(0.82, 0.88, 0.94)),
                ));
                spawn_flow_error_button(
                    panel,
                    0,
                    FlowUiAction::CancelDeleteBrawler,
                    "KEEP BRAWLER",
                );
                spawn_flow_error_button(panel, 1, FlowUiAction::ConfirmDeleteBrawler, "DELETE");
            });
        });
}
