//! Authenticated dashboard snapshot and product-action presentation.
#![allow(clippy::wildcard_imports)]

use super::super::*;

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "the dashboard entry renders one bounded authenticated product snapshot"
)]
pub(in crate::client::flow) fn spawn_dashboard(
    mut commands: Commands,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    mut navigation: ResMut<FlowNavigation>,
    mut selection: ResMut<SelectedGameType>,
    profile: Res<crate::client::ClientProfileModel>,
    queue: Res<crate::client::ClientQueueModel>,
    practice: Res<crate::client::ClientPracticeModel>,
    mut purpose: ResMut<SessionPurpose>,
    mut return_focus: ResMut<DashboardReturnFocus>,
    mut notice: ResMut<DashboardNotice>,
    assets: Option<Res<crate::client::ClientAssetHandles>>,
) {
    let Some(membership) = memberships.iter().next() else {
        return;
    };
    *purpose = SessionPurpose::Multiplayer;
    let previous_game_type = selection.game_type_id.clone();
    let selected_index = selection
        .game_type_id
        .as_ref()
        .and_then(|selected| {
            membership
                .game_types
                .iter()
                .position(|game| game.id == *selected)
        })
        .unwrap_or(0);
    let Some(game) = membership.game_types.get(selected_index) else {
        return;
    };
    selection.catalog_revision = Some(membership.catalog_revision);
    selection.game_type_id = Some(game.id.clone());
    selection.configuration_revision = Some(game.configuration_revision);
    if previous_game_type.is_some() && previous_game_type.as_ref() != Some(&game.id) {
        notice.0 = Some(format!(
            "The previous game is unavailable. {} is now selected.",
            game.display_name
        ));
    }
    navigation.selected = return_focus.0.take().unwrap_or(DASHBOARD_PLAY_INDEX);
    let dashboard_notice = notice.0.take();
    let admission_pending = queue.pending().is_some() || practice.pending() || profile.pending();
    let selected_brawler = membership.profile.selected_brawler_id.and_then(|id| {
        membership
            .profile
            .brawlers
            .iter()
            .find(|brawler| brawler.id == id)
    });
    let build_name =
        selected_brawler.map_or("CREATE YOUR FIRST BRAWLER", |brawler| brawler.name.as_str());
    let build_summary = selected_brawler.map_or_else(
        || "Choose a permanent fighter profile and weapon base".to_string(),
        |brawler| brawler_loadout_summary(brawler, &membership.brawler_catalog),
    );
    let game_summary = dashboard_game_summary(game);
    let population = if queue.required_snapshot_is_fresh() {
        queue_population(&queue, game)
    } else {
        "Population updating".to_string()
    };
    let capacity_occupied = queue.snapshot().is_some_and(|snapshot| {
        snapshot.formation_availability == crate::lobby::FormationAvailability::ProductMatchOccupied
    });
    let build_accessible = format!("View brawlers: {build_name}, {build_summary}");
    let game_accessible = format!(
        "Change game type: {}, {game_summary}, {population}",
        game.display_name
    );

    commands
        .spawn((
            FlowRoot,
            DashboardRoot,
            DashboardLayoutRole::Root,
            DashboardLayoutClass::Wide,
            DespawnOnExit(ClientFlow::Dashboard),
            dashboard_root_node(),
            ScrollPosition::default(),
            BackgroundColor(Color::NONE),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            root.spawn((
                DashboardLayoutRole::Header,
                Node {
                    width: percent(100),
                    display: Display::Flex,
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                    padding: UiRect::axes(px(18), px(6)),
                    ..default()
                },
            ))
            .with_children(|header| {
                if let Some(assets) = assets.as_deref() {
                    header.spawn((
                        DashboardLayoutRole::Wordmark,
                        ImageNode::new(assets.wordmark.clone()),
                        Node {
                            width: px(220),
                            height: auto(),
                            ..default()
                        },
                    ));
                } else {
                    header.spawn((
                        DashboardLayoutRole::Wordmark,
                        Text::new("PEWPEW BLITZ"),
                        dashboard_font(assets.as_deref(), 32.0),
                        TextColor(Color::srgb(0.28, 0.92, 1.0)),
                    ));
                }
                header
                    .spawn((
                        DashboardLayoutRole::Identity,
                        AccessibleLabel::new(format!(
                            "Player {}, server {}, online",
                            membership.accepted_display_name, membership.server_name
                        )),
                        Node {
                            min_width: px(240),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::axes(px(14), px(7)),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(11)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.035, 0.12, 0.24, 0.92)),
                        BorderColor::all(Color::srgba(0.15, 0.5, 0.8, 0.45)),
                    ))
                    .with_children(|identity| {
                        identity.spawn((
                            Text::new(&membership.accepted_display_name),
                            dashboard_font(assets.as_deref(), 22.0),
                            TextColor(Color::WHITE),
                        ));
                        identity.spawn((
                            Text::new(format!("SERVER: {}  ONLINE", membership.server_name)),
                            TextFont::from_font_size(12.0),
                            TextColor(Color::srgb(0.2, 0.9, 0.72)),
                        ));
                    });
                header.spawn((
                    DashboardLayoutRole::HeaderSpacer,
                    Node {
                        flex_grow: 1.0,
                        ..default()
                    },
                ));
                spawn_dashboard_button(
                    header,
                    DASHBOARD_SETTINGS_INDEX,
                    FlowUiAction::OpenSettings,
                    DashboardButtonPresentation {
                        label: "SETTINGS",
                        width: px(112),
                        primary: false,
                        disabled: false,
                        assets: assets.as_deref(),
                        icon: assets.as_deref().map(|assets| assets.settings_icon.clone()),
                    },
                );
                spawn_dashboard_button(
                    header,
                    DASHBOARD_MENU_INDEX,
                    FlowUiAction::OpenDashboardMenu,
                    DashboardButtonPresentation {
                        label: "MENU",
                        width: px(92),
                        primary: false,
                        disabled: false,
                        assets: assets.as_deref(),
                        icon: assets.as_deref().map(|assets| assets.menu_icon.clone()),
                    },
                );
            });
            if let Some(message) = dashboard_notice {
                root.spawn((
                    Text::new(message),
                    TextFont::from_font_size(13.0),
                    TextColor(Color::srgb(1.0, 0.86, 0.48)),
                ));
            }
            root.spawn((
                DashboardLayoutRole::Center,
                Node {
                    width: percent(100),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: px(5),
                    ..default()
                },
            ))
            .with_children(|center| {
                let mut preview_button = center.spawn((
                    DashboardPreviewHost,
                    DashboardLayoutRole::Preview,
                    DashboardButtonStyle::Preview,
                    AccessibleLabel::new(build_accessible.clone()),
                    Button,
                    FlowButton {
                        index: DASHBOARD_BUILD_INDEX,
                        action: if selected_brawler.is_some() {
                            FlowUiAction::OpenBrawlerList
                        } else {
                            FlowUiAction::CreateBrawler
                        },
                        error_action: false,
                    },
                    Node {
                        width: percent(54),
                        max_width: px(650),
                        min_height: px(280),
                        max_height: px(470),
                        flex_grow: 1.0,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::End,
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(24)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    BorderColor::all(Color::NONE),
                ));
                if admission_pending {
                    preview_button.insert(InteractionDisabled);
                }
                let mut build_button = center.spawn((
                    Button,
                    DashboardBuildCard,
                    DashboardLayoutRole::Build,
                    DashboardButtonStyle::Build,
                    AccessibleLabel::new(build_accessible.clone()),
                    FlowButton {
                        index: DASHBOARD_BUILD_INDEX,
                        action: if selected_brawler.is_some() {
                            FlowUiAction::OpenBrawlerList
                        } else {
                            FlowUiAction::CreateBrawler
                        },
                        error_action: false,
                    },
                    Node {
                        width: percent(30),
                        max_width: px(365),
                        min_height: px(104),
                        column_gap: px(12),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        padding: UiRect::axes(px(12), px(8)),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(14)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.91, 0.95, 1.0)),
                    BorderColor::all(Color::srgb(0.55, 0.7, 0.9)),
                    BoxShadow::new(
                        Color::srgba(0.0, 0.02, 0.08, 0.65),
                        px(0),
                        px(5),
                        px(0),
                        px(3),
                    ),
                ));
                if admission_pending {
                    build_button.insert(InteractionDisabled);
                }
                build_button.with_children(|card| {
                    spawn_dashboard_icon_well(
                        card,
                        assets.as_deref().map(|assets| assets.build_icon.clone()),
                        52.0,
                        31.0,
                        Color::srgb(0.05, 0.34, 0.82),
                    );
                    card.spawn((Node {
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },))
                        .with_children(|details| {
                            details.spawn((
                                Text::new(build_name.to_uppercase()),
                                DashboardBrawlerNameLabel,
                                dashboard_font(assets.as_deref(), 24.0),
                                TextColor(Color::srgb(0.035, 0.12, 0.32)),
                            ));
                            details.spawn((
                                Text::new(build_summary),
                                DashboardBrawlerSummaryLabel,
                                TextFont::from_font_size(12.0),
                                TextColor(Color::srgb(0.08, 0.18, 0.35)),
                            ));
                            details.spawn((
                                Text::new(if selected_brawler.is_some() {
                                    "VIEW BRAWLERS"
                                } else {
                                    "CREATE BRAWLER"
                                }),
                                dashboard_font(assets.as_deref(), 15.0),
                                TextColor(Color::srgb(0.03, 0.36, 0.82)),
                            ));
                        });
                });
            });
            root.spawn((
                DashboardLayoutRole::ActionRow,
                Node {
                    width: percent(94),
                    max_width: px(1180),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    column_gap: px(12),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Stretch,
                    ..default()
                },
            ))
            .with_children(|actions| {
                let mut mode_button = actions.spawn((
                    Button,
                    DashboardModeCard,
                    DashboardLayoutRole::Mode,
                    DashboardButtonStyle::Mode,
                    AccessibleLabel::new(game_accessible),
                    FlowButton {
                        index: DASHBOARD_GAME_INDEX,
                        action: FlowUiAction::OpenGameTypeSelect,
                        error_action: false,
                    },
                    Node {
                        width: percent(44),
                        min_height: px(104),
                        column_gap: px(14),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        padding: UiRect::axes(px(18), px(10)),
                        border: UiRect::all(px(3)),
                        border_radius: BorderRadius::all(px(16)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.92, 0.95, 1.0)),
                    BorderColor::all(Color::srgb(0.55, 0.7, 0.9)),
                    BoxShadow::new(
                        Color::srgba(0.0, 0.02, 0.08, 0.65),
                        px(0),
                        px(5),
                        px(0),
                        px(3),
                    ),
                ));
                if admission_pending {
                    mode_button.insert(InteractionDisabled);
                }
                mode_button.with_children(|card| {
                    spawn_dashboard_icon_well(
                        card,
                        assets.as_deref().map(|assets| assets.mode_icon.clone()),
                        68.0,
                        42.0,
                        Color::srgb(0.05, 0.34, 0.82),
                    );
                    card.spawn((Node {
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },))
                        .with_children(|details| {
                            details.spawn((
                                Text::new(game.display_name.to_uppercase()),
                                dashboard_font(assets.as_deref(), 28.0),
                                TextColor(Color::srgb(0.035, 0.12, 0.32)),
                            ));
                            details.spawn((
                                Text::new(format!("{game_summary}\n{population}")),
                                DashboardGameSummaryLabel,
                                TextFont::from_font_size(12.0),
                                TextColor(Color::srgb(0.08, 0.18, 0.35)),
                                TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                            ));
                        });
                });
                spawn_dashboard_button(
                    actions,
                    DASHBOARD_PRACTICE_INDEX,
                    FlowUiAction::StartPractice,
                    DashboardButtonPresentation {
                        label: if practice.pending() {
                            "STARTING..."
                        } else {
                            "PRACTICE"
                        },
                        width: percent(21),
                        primary: false,
                        disabled: admission_pending || selected_brawler.is_none(),
                        assets: assets.as_deref(),
                        icon: assets.as_deref().map(|assets| assets.practice_icon.clone()),
                    },
                );
                spawn_dashboard_button(
                    actions,
                    DASHBOARD_PLAY_INDEX,
                    FlowUiAction::JoinQueue,
                    DashboardButtonPresentation {
                        label: if queue.pending().is_some() {
                            "JOINING..."
                        } else if capacity_occupied {
                            "MATCH IN PROGRESS"
                        } else {
                            "PLAY"
                        },
                        width: percent(33),
                        primary: true,
                        disabled: capacity_occupied
                            || admission_pending
                            || selected_brawler.is_none(),
                        assets: assets.as_deref(),
                        icon: assets.as_deref().map(|assets| assets.play_icon.clone()),
                    },
                );
            });
        });
}

pub(in crate::client::flow) fn dashboard_game_summary(
    game: &crate::lobby::AdvertisedGameType,
) -> String {
    let maps = crate::map::MapContentCatalog::embedded().ok().map_or_else(
        || "Map pool unavailable".to_string(),
        |catalog| {
            game.map_preset_ids
                .iter()
                .map(|id| {
                    catalog
                        .presets
                        .iter()
                        .find(|preset| preset.id == *id)
                        .map_or_else(
                            || format!("Map {}", id.0),
                            |preset| preset.display_name.clone(),
                        )
                })
                .collect::<Vec<_>>()
                .join(", ")
        },
    );
    let rules = match game.rules_summary {
        crate::lobby::AdvertisedRulesSummary::Wipeout {
            target_score,
            active_limit_ticks,
        } => format!(
            "First to {target_score} - {}s limit",
            active_limit_ticks / 60
        ),
        crate::lobby::AdvertisedRulesSummary::HotZone {
            target_progress_ticks,
            active_limit_ticks,
        } => format!(
            "Hold {}s - {}s limit",
            target_progress_ticks / 60,
            active_limit_ticks / 60
        ),
        crate::lobby::AdvertisedRulesSummary::Heist {
            safe_maximum_health,
            active_limit_ticks,
        } => format!(
            "Destroy the {safe_maximum_health} HP enemy idol - {}s limit",
            active_limit_ticks / 60
        ),
    };
    format!("{rules}\nMap pool: {maps}")
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    reason = "Dashboard presentation reads authenticated lobby and bounded queue resources"
)]
pub(in crate::client::flow) fn update_dashboard_live_facts(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    selection: Res<SelectedGameType>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    queue: Res<crate::client::ClientQueueModel>,
    practice: Res<crate::client::ClientPracticeModel>,
    profile: Res<crate::client::ClientProfileModel>,
    mut texts: Query<(
        &mut Text,
        Has<DashboardGameSummaryLabel>,
        Has<DashboardPlayLabel>,
        Has<DashboardPracticeLabel>,
        Has<DashboardBrawlerNameLabel>,
        Has<DashboardBrawlerSummaryLabel>,
    )>,
    mut action_buttons: Query<(
        Entity,
        &DashboardButtonStyle,
        &mut FlowButton,
        Option<&AccessibleLabel>,
    )>,
) {
    if *flow.get() != ClientFlow::Dashboard {
        return;
    }
    let Some(membership) = memberships.iter().next() else {
        return;
    };
    let Some(game) = selection.game_type_id.as_ref().and_then(|selected| {
        membership
            .game_types
            .iter()
            .find(|game| game.id == *selected)
    }) else {
        return;
    };
    let population = if queue.required_snapshot_is_fresh() {
        queue_population(&queue, game)
    } else {
        "Population updating".to_string()
    };
    let copy = format!("{}\n{population}", dashboard_game_summary(game));
    let selected_brawler = membership.profile.selected_brawler_id.and_then(|id| {
        membership
            .profile
            .brawlers
            .iter()
            .find(|brawler| brawler.id == id)
    });
    let brawler_accessible = selected_brawler.map_or_else(
        || "Create your first brawler".to_string(),
        |brawler| {
            format!(
                "View brawlers: {}, {}",
                brawler.name,
                brawler_loadout_summary(brawler, &membership.brawler_catalog)
            )
        },
    );
    for (mut text, is_summary, _, _, is_brawler_name, is_brawler_summary) in &mut texts {
        if is_summary {
            text.0.clone_from(&copy);
        } else if is_brawler_name {
            text.0 = selected_brawler.map_or_else(
                || "CREATE YOUR FIRST BRAWLER".to_string(),
                |brawler| brawler.name.to_uppercase(),
            );
        } else if is_brawler_summary {
            text.0 = selected_brawler.map_or_else(
                || "Choose a permanent fighter profile and weapon base".to_string(),
                |brawler| brawler_loadout_summary(brawler, &membership.brawler_catalog),
            );
        }
    }
    let capacity_occupied = queue.snapshot().is_some_and(|snapshot| {
        snapshot.formation_availability == crate::lobby::FormationAvailability::ProductMatchOccupied
    });
    let admission_pending = queue.pending().is_some() || practice.pending() || profile.pending();
    let profile_empty = selected_brawler.is_none();
    let play_copy = if queue.pending().is_some() {
        "Joining match; Play unavailable"
    } else if capacity_occupied {
        "Match in progress; Play unavailable"
    } else {
        "Play"
    };
    let practice_copy = if practice.pending() {
        "Starting practice; Practice unavailable"
    } else {
        "Practice"
    };
    let busy_suffix = "; unavailable while admission is pending";
    for (entity, style, mut button, current_label) in &mut action_buttons {
        let disabled = match style {
            DashboardButtonStyle::Preview
            | DashboardButtonStyle::Build
            | DashboardButtonStyle::Mode => admission_pending,
            DashboardButtonStyle::Practice => admission_pending || profile_empty,
            DashboardButtonStyle::Play => capacity_occupied || admission_pending || profile_empty,
            DashboardButtonStyle::Header => false,
        };
        if disabled {
            commands.entity(entity).insert(InteractionDisabled);
        } else {
            commands.entity(entity).remove::<InteractionDisabled>();
        }
        let next_label = match style {
            DashboardButtonStyle::Mode => format!(
                "Change game type: {}, {}, {population}{}",
                game.display_name,
                dashboard_game_summary(game),
                if admission_pending { busy_suffix } else { "" }
            ),
            DashboardButtonStyle::Preview | DashboardButtonStyle::Build => {
                button.action = if profile_empty {
                    FlowUiAction::CreateBrawler
                } else {
                    FlowUiAction::OpenBrawlerList
                };
                format!(
                    "{brawler_accessible}{}",
                    if admission_pending { busy_suffix } else { "" }
                )
            }
            DashboardButtonStyle::Practice => practice_copy.to_string(),
            DashboardButtonStyle::Play => play_copy.to_string(),
            DashboardButtonStyle::Header => continue,
        };
        if current_label.is_none_or(|current| current.0 != next_label) {
            commands
                .entity(entity)
                .insert(AccessibleLabel::new(next_label));
        }
    }
    for (mut text, _, is_play_label, is_practice_label, _, _) in &mut texts {
        if is_play_label {
            text.0 = if queue.pending().is_some() {
                "JOINING...".to_string()
            } else if capacity_occupied {
                "MATCH IN PROGRESS".to_string()
            } else {
                "PLAY".to_string()
            };
        } else if is_practice_label {
            text.0 = if practice.pending() {
                "STARTING...".to_string()
            } else {
                "PRACTICE".to_string()
            };
        }
    }
}

pub(in crate::client::flow) fn dashboard_layout_class(
    logical_width: f32,
    logical_height: f32,
    ui_scale: f32,
) -> DashboardLayoutClass {
    let scale = ui_scale.max(0.01);
    let effective_width = logical_width / scale;
    let effective_height = logical_height / scale;
    if effective_width < DASHBOARD_COMPACT_WIDTH || effective_height < DASHBOARD_COMPACT_HEIGHT {
        DashboardLayoutClass::Compact
    } else {
        DashboardLayoutClass::Wide
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "one change-driven pass applies the closed Wide/Compact dashboard node contract"
)]
pub(in crate::client::flow) fn apply_dashboard_layout(
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
    mut roots: Query<(&mut DashboardLayoutClass, &mut ScrollPosition), With<DashboardRoot>>,
    mut nodes: Query<(
        &mut Node,
        &DashboardLayoutRole,
        Option<&DashboardButtonStyle>,
    )>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let next = dashboard_layout_class(
        window.resolution.width(),
        window.resolution.height(),
        scale.as_deref().map_or(1.0, |scale| scale.0),
    );
    let Some((mut current, mut scroll)) = roots.iter_mut().next() else {
        return;
    };
    if *current == next {
        return;
    }
    *current = next;
    if next == DashboardLayoutClass::Wide {
        scroll.0 = Vec2::ZERO;
    }
    for (mut node, role, style) in &mut nodes {
        apply_dashboard_layout_node(&mut node, *role, style.copied(), next);
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the closed role match keeps the complete Wide/Compact layout contract reviewable"
)]
fn apply_dashboard_layout_node(
    node: &mut Node,
    role: DashboardLayoutRole,
    style: Option<DashboardButtonStyle>,
    class: DashboardLayoutClass,
) {
    let compact = class == DashboardLayoutClass::Compact;
    match role {
        DashboardLayoutRole::Root => {
            node.row_gap = px(if compact { 8 } else { 5 });
            node.padding = if compact {
                UiRect::axes(px(8), px(6))
            } else {
                UiRect::axes(px(16), px(8))
            };
            node.overflow = if compact {
                Overflow::scroll_y()
            } else {
                Overflow::clip()
            };
        }
        DashboardLayoutRole::Header => {
            node.column_gap = px(if compact { 6 } else { 10 });
            node.padding = if compact {
                UiRect::axes(px(4), px(4))
            } else {
                UiRect::axes(px(18), px(6))
            };
        }
        DashboardLayoutRole::Wordmark => {
            node.width = px(if compact { 105 } else { 220 });
        }
        DashboardLayoutRole::Identity => {
            node.min_width = if compact { auto() } else { px(240) };
            node.flex_grow = if compact { 1.0 } else { 0.0 };
            node.flex_shrink = if compact { 1.0 } else { 0.0 };
            node.padding = if compact {
                UiRect::axes(px(8), px(5))
            } else {
                UiRect::axes(px(14), px(7))
            };
        }
        DashboardLayoutRole::HeaderSpacer => {
            node.display = if compact {
                Display::None
            } else {
                Display::Flex
            };
        }
        DashboardLayoutRole::Center => {
            node.flex_grow = if compact { 0.0 } else { 1.0 };
            node.justify_content = if compact {
                JustifyContent::FlexStart
            } else {
                JustifyContent::Center
            };
            node.row_gap = px(if compact { 8 } else { 5 });
        }
        DashboardLayoutRole::Preview => {
            node.width = percent(if compact { 90 } else { 54 });
            node.max_width = px(if compact { 520 } else { 650 });
            node.min_height = px(if compact { 180 } else { 280 });
            node.max_height = px(if compact { 220 } else { 470 });
            node.flex_grow = if compact { 0.0 } else { 1.0 };
        }
        DashboardLayoutRole::Build => {
            node.width = percent(if compact { 94 } else { 30 });
            node.max_width = px(if compact { 700 } else { 365 });
            node.min_height = px(if compact { 88 } else { 104 });
        }
        DashboardLayoutRole::ActionRow => {
            node.flex_direction = if compact {
                FlexDirection::Column
            } else {
                FlexDirection::Row
            };
            node.column_gap = px(if compact { 0 } else { 12 });
            node.row_gap = px(if compact { 8 } else { 0 });
        }
        DashboardLayoutRole::Mode => {
            node.width = percent(if compact { 100 } else { 44 });
            node.min_height = px(if compact { 94 } else { 104 });
        }
        DashboardLayoutRole::UtilityButton { wide_width } => {
            node.width = px(if compact { 48.0 } else { wide_width });
            node.min_height = px(42);
            node.padding = UiRect::axes(px(if compact { 6 } else { 12 }), px(7));
        }
        DashboardLayoutRole::UtilityLabel { has_icon } => {
            node.display = if compact && has_icon {
                Display::None
            } else {
                Display::Flex
            };
        }
    }
    match style {
        Some(DashboardButtonStyle::Practice) => {
            node.width = percent(if compact { 100 } else { 21 });
            node.min_height = px(if compact { 80 } else { 104 });
        }
        Some(DashboardButtonStyle::Play) => {
            node.width = percent(if compact { 100 } else { 33 });
            node.min_height = px(if compact { 88 } else { 104 });
        }
        _ => {}
    }
}

pub(in crate::client::flow) fn scroll_dashboard(
    mut wheel: MessageReader<MouseWheel>,
    mut roots: Query<(&DashboardLayoutClass, &mut ScrollPosition), With<DashboardRoot>>,
) {
    let delta = wheel
        .read()
        .map(|event| match event.unit {
            MouseScrollUnit::Line => event.y * 24.0,
            MouseScrollUnit::Pixel => event.y,
        })
        .sum::<f32>();
    if delta.abs() <= f32::EPSILON {
        return;
    }
    for (class, mut position) in &mut roots {
        if *class == DashboardLayoutClass::Compact {
            position.0.y = (position.0.y - delta).max(0.0);
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "computed UI bounds are available only after Bevy's layout pass"
)]
pub(in crate::client::flow) fn keep_dashboard_focus_visible(
    navigation: Res<FlowNavigation>,
    buttons: Query<(&FlowButton, &ComputedNode, &UiGlobalTransform)>,
    mut roots: Query<
        (
            Entity,
            &DashboardLayoutClass,
            &ComputedNode,
            &UiGlobalTransform,
            &mut ScrollPosition,
        ),
        With<DashboardRoot>,
    >,
    mut prior: Local<Option<(Entity, DashboardLayoutClass, usize)>>,
) {
    let Some((root_entity, class, _, _, _)) = roots.iter_mut().next() else {
        *prior = None;
        return;
    };
    let focus_key = (root_entity, *class, navigation.selected);
    if prior.as_ref() == Some(&focus_key) {
        return;
    }
    *prior = Some(focus_key);
    let mut focused_top = f32::MAX;
    let mut focused_bottom = f32::MIN;
    let mut found = false;
    for (button, node, transform) in &buttons {
        if button.index != navigation.selected || node.is_empty() {
            continue;
        }
        let (_, _, center) = transform.to_scale_angle_translation();
        found = true;
        focused_top = focused_top.min(center.y - node.size().y * 0.5);
        focused_bottom = focused_bottom.max(center.y + node.size().y * 0.5);
    }
    if !found {
        return;
    }
    for (_, class, root_node, root_transform, mut scroll) in &mut roots {
        if *class != DashboardLayoutClass::Compact || root_node.is_empty() {
            continue;
        }
        let (_, _, center) = root_transform.to_scale_angle_translation();
        let half_height = root_node.size().y * 0.5;
        let visible_top = center.y - half_height + 8.0;
        let visible_bottom = center.y + half_height - 8.0;
        if focused_top < visible_top {
            scroll.0.y = (scroll.0.y - (visible_top - focused_top)).max(0.0);
        } else if focused_bottom > visible_bottom {
            scroll.0.y += focused_bottom - visible_bottom;
        }
    }
}

fn dashboard_root_node() -> Node {
    Node {
        width: percent(100),
        height: percent(100),
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::FlexStart,
        row_gap: px(5),
        padding: UiRect::axes(px(16), px(8)),
        ..default()
    }
}

fn dashboard_font(assets: Option<&crate::client::ClientAssetHandles>, size: f32) -> TextFont {
    assets.map_or_else(
        || TextFont::from_font_size(size),
        |assets| TextFont {
            font: assets.dashboard_font.clone().into(),
            font_size: FontSize::Px(size),
            ..default()
        },
    )
}

fn spawn_dashboard_icon_well(
    parent: &mut ChildSpawnerCommands,
    icon: Option<Handle<Image>>,
    well_size: f32,
    icon_size: f32,
    color: Color,
) {
    parent
        .spawn((
            Node {
                width: px(well_size),
                height: px(well_size),
                flex_shrink: 0.0,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(px(well_size * 0.28)),
                ..default()
            },
            BackgroundColor(color),
        ))
        .with_children(|well| {
            if let Some(icon) = icon {
                well.spawn((
                    ImageNode::new(icon),
                    Node {
                        width: px(icon_size),
                        height: px(icon_size),
                        ..default()
                    },
                ));
            }
        });
}

struct DashboardButtonPresentation<'a> {
    label: &'a str,
    width: Val,
    primary: bool,
    disabled: bool,
    assets: Option<&'a crate::client::ClientAssetHandles>,
    icon: Option<Handle<Image>>,
}

#[derive(Clone, Copy)]
enum DashboardButtonContentKind {
    Play,
    Practice,
    Utility { has_icon: bool },
    Other,
}

const fn dashboard_button_icon_size(is_play: bool, is_practice: bool) -> f32 {
    if is_play {
        42.0
    } else if is_practice {
        32.0
    } else {
        21.0
    }
}

fn spawn_dashboard_button(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    presentation: DashboardButtonPresentation<'_>,
) {
    let DashboardButtonPresentation {
        label,
        width,
        primary,
        disabled,
        assets,
        icon,
    } = presentation;
    let is_play = matches!(action, FlowUiAction::JoinQueue);
    let is_practice = matches!(action, FlowUiAction::StartPractice);
    let is_utility = matches!(
        action,
        FlowUiAction::OpenSettings | FlowUiAction::OpenDashboardMenu
    );
    let has_icon = icon.is_some();
    let utility_width = matches!(action, FlowUiAction::OpenSettings)
        .then_some(112.0)
        .or_else(|| matches!(action, FlowUiAction::OpenDashboardMenu).then_some(92.0));
    let icon_size = dashboard_button_icon_size(is_play, is_practice);
    let mut button = parent.spawn((
        Button,
        AccessibleLabel::new(label),
        FlowButton {
            index,
            action,
            error_action: false,
        },
        Node {
            width,
            min_height: px(if is_play || is_practice { 104 } else { 42 }),
            column_gap: px(if is_play || is_practice { 12 } else { 7 }),
            padding: UiRect::axes(px(12), px(7)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(px(if primary { 3 } else { 2 })),
            border_radius: BorderRadius::all(px(if primary { 16 } else { 11 })),
            ..default()
        },
        BackgroundColor(if primary {
            Color::srgb(0.95, 0.48, 0.08)
        } else {
            Color::srgb(0.09, 0.14, 0.2)
        }),
        BorderColor::all(Color::NONE),
        BoxShadow::new(
            Color::srgba(0.0, 0.02, 0.08, 0.65),
            px(0),
            px(if is_play { 7 } else { 4 }),
            px(0),
            px(3),
        ),
    ));
    if is_play {
        button.insert(DashboardButtonStyle::Play);
    } else if is_practice {
        button.insert(DashboardButtonStyle::Practice);
    } else {
        button.insert(DashboardButtonStyle::Header);
    }
    if let Some(wide_width) = utility_width {
        button.insert(DashboardLayoutRole::UtilityButton { wide_width });
    }
    if disabled {
        button.insert(InteractionDisabled);
    }
    let content_kind = if is_play {
        DashboardButtonContentKind::Play
    } else if is_practice {
        DashboardButtonContentKind::Practice
    } else if is_utility {
        DashboardButtonContentKind::Utility { has_icon }
    } else {
        DashboardButtonContentKind::Other
    };
    button.with_children(|button| {
        spawn_dashboard_button_contents(button, label, assets, icon, icon_size, content_kind);
    });
}

fn spawn_dashboard_button_contents(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    assets: Option<&crate::client::ClientAssetHandles>,
    icon: Option<Handle<Image>>,
    icon_size: f32,
    kind: DashboardButtonContentKind,
) {
    if let Some(icon) = icon {
        parent.spawn((
            ImageNode::new(icon),
            Node {
                width: px(icon_size),
                height: px(icon_size),
                ..default()
            },
        ));
    }
    let font_size = match kind {
        DashboardButtonContentKind::Play => 38.0,
        DashboardButtonContentKind::Practice => 24.0,
        DashboardButtonContentKind::Utility { .. } | DashboardButtonContentKind::Other => 15.0,
    };
    let color = match kind {
        DashboardButtonContentKind::Play => Color::WHITE,
        DashboardButtonContentKind::Practice => Color::srgb(0.04, 0.2, 0.55),
        DashboardButtonContentKind::Utility { .. } | DashboardButtonContentKind::Other => {
            Color::srgb(0.9, 0.95, 1.0)
        }
    };
    let mut text = parent.spawn((
        Text::new(label),
        dashboard_font(assets, font_size),
        TextColor(color),
    ));
    match kind {
        DashboardButtonContentKind::Play => {
            text.insert(DashboardPlayLabel);
        }
        DashboardButtonContentKind::Practice => {
            text.insert(DashboardPracticeLabel);
        }
        DashboardButtonContentKind::Utility { has_icon } => {
            text.insert(DashboardLayoutRole::UtilityLabel { has_icon });
        }
        DashboardButtonContentKind::Other => {}
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the bounded menu presenter declares its complete connected Dashboard view"
)]
pub(in crate::client::flow) fn present_dashboard_menu(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    roots: Query<Entity, With<DashboardMenuRoot>>,
    mut navigation: ResMut<FlowNavigation>,
    memberships: Query<Option<&RuntimeLobbyTarget>, With<Client>>,
    persistence: Res<ConnectionPersistence>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::DashboardMenu) {
        for entity in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    if roots.iter().next().is_some() || *flow.get() != ClientFlow::Dashboard {
        return;
    }
    navigation.selected = 0;
    let favorite_label = memberships.iter().flatten().next().map(|target| {
        if persistence
            .state
            .favorites
            .iter()
            .any(|favorite| favorite.address == target.logical_address)
        {
            "REMOVE FAVORITE"
        } else {
            "FAVORITE SERVER"
        }
    });
    commands
        .spawn((
            DashboardMenuRoot,
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
            BackgroundColor(Color::srgba(0.0, 0.015, 0.04, 0.78)),
            GlobalZIndex(500),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(82),
                    max_width: px(430),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(10),
                    padding: UiRect::all(px(22)),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(14)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.035, 0.075, 0.14)),
                BorderColor::all(Color::srgb(0.12, 0.42, 0.7)),
            ))
            .with_children(|panel| {
                spawn_heading(panel, "MENU");
                let mut index = 0;
                spawn_flow_error_button(panel, index, FlowUiAction::OpenCredits, "CREDITS");
                index += 1;
                if let Some(favorite_label) = favorite_label {
                    spawn_flow_error_button(
                        panel,
                        index,
                        FlowUiAction::ToggleFavoriteServer,
                        favorite_label,
                    );
                    index += 1;
                }
                spawn_flow_error_button(
                    panel,
                    index,
                    FlowUiAction::RequestChangeServer,
                    "CHANGE SERVER",
                );
                index += 1;
                spawn_flow_error_button(panel, index, FlowUiAction::Quit, "QUIT");
                spawn_flow_error_button(panel, index + 1, FlowUiAction::CloseDashboardMenu, "BACK");
            });
        });
}
