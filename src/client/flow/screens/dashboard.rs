//! Authenticated dashboard snapshot and product-action presentation.
use super::scroll::{clamp_scroll_offset, normalized_wheel_delta, offset_keeping_interval_visible};
use super::{
    brawlers::ultimate_name,
    shared::{FlowButton, FlowNavigation, FlowRoot},
};
use crate::client::{
    ClientLobbyMembership, RuntimeLobbyTarget,
    flow::{
        actions::FlowUiAction,
        model::{ClientFlow, ClientOverlay, SelectedGameType, SessionPurpose},
        persistence::ConnectionPersistence,
    },
};
use bevy::{
    input::mouse::MouseWheel,
    prelude::*,
    ui::{InteractionDisabled, ScrollPosition},
};
use lightyear::prelude::client::Client;

pub(in crate::client::flow) const DASHBOARD_PLAY_INDEX: usize = 0;
pub(in crate::client::flow) const DASHBOARD_PRACTICE_INDEX: usize = 1;
pub(in crate::client::flow) const DASHBOARD_GAME_INDEX: usize = 2;
pub(in crate::client::flow) const DASHBOARD_BUILD_INDEX: usize = 3;
pub(in crate::client::flow) const DASHBOARD_MENU_INDEX: usize = 4;
mod layout;
pub(in crate::client::flow) use layout::{apply_dashboard_layout, dashboard_layout_class};
pub(in crate::client::flow) const INK: Color = Color::srgb(0.025, 0.10, 0.145);
pub(in crate::client::flow) const CREAM: Color = Color::srgb(0.98, 0.92, 0.81);
pub(in crate::client::flow) const TEAL: Color = Color::srgb(0.32, 0.64, 0.62);
pub(in crate::client::flow) const ORANGE: Color = Color::srgb(1.0, 0.40, 0.025);

#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::client::flow) struct DashboardReturnFocus(pub(in crate::client::flow) Option<usize>);

#[derive(Resource, Default, Clone, Debug, PartialEq, Eq)]
pub(in crate::client::flow) struct DashboardNotice(pub(in crate::client::flow) Option<String>);

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::client::flow) enum DashboardLayoutClass {
    #[default]
    Wide,
    Compact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::client::flow) enum DashboardNavigationDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Component)]
pub(in crate::client) struct DashboardRoot;

#[derive(Component, Clone, Copy)]
pub(in crate::client::flow) enum DashboardLayoutRole {
    Root,
    Header,
    Wordmark,
    Identity,
    HeaderSpacer,
    Center,
    Preview,
    Build,
    ActionRow,
    Mode,
    UtilityButton { wide_width: f32 },
    UtilityLabel,
    ActionButton,
    Notice,
}

#[derive(Component)]
pub(in crate::client::flow) struct DashboardGameSummaryLabel;

#[derive(Component)]
pub(in crate::client::flow) struct DashboardPlayLabel;

#[derive(Component)]
pub(in crate::client::flow) struct DashboardPracticeLabel;

#[derive(Component)]
pub(in crate::client::flow) struct DashboardBrawlerNameLabel;

#[derive(Component)]
pub(in crate::client::flow) struct DashboardBrawlerSummaryLabel;

#[derive(Component)]
pub(in crate::client::flow) struct DashboardBrawlerActionLabel;

#[derive(Component)]
pub(in crate::client::flow) struct DashboardMenuRoot;

#[derive(Component)]
struct DashboardBuildCard;

#[derive(Component)]
struct DashboardModeCard;

#[derive(Component, Clone, Copy, Debug)]
pub(in crate::client::flow) enum DashboardButtonStyle {
    Preview,
    Header,
    Build,
    Mode,
    Practice,
    Play,
}

#[derive(Component)]
pub(in crate::client) struct DashboardPreviewHost;

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
    let dashboard_notice = notice
        .0
        .take()
        .filter(|message| message != "Profile saved.");
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
        |brawler| dashboard_build_summary(brawler, &membership.brawler_catalog),
    );
    let game_summary = dashboard_team_summary(game);
    let capacity_occupied = queue.snapshot().is_some_and(|snapshot| {
        snapshot.formation_availability == crate::lobby::FormationAvailability::ProductMatchOccupied
    });
    let build_accessible = format!("View brawlers: {build_name}, {build_summary}");
    let game_accessible = format!("Change game type: {}, {game_summary}", game.display_name);

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
                        ImageNode::new(assets.dashboard_logo.clone()),
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
                        TextColor(INK),
                    ));
                }
                header
                    .spawn((
                        DashboardLayoutRole::Identity,
                        AccessibleLabel::new(format!(
                            "Player {}, server {}",
                            membership.accepted_display_name, membership.server_name
                        )),
                        Node {
                            min_width: px(240),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: px(12),
                            padding: UiRect::axes(px(20), px(12)),
                            border: UiRect::ZERO,
                            border_radius: BorderRadius::all(px(11)),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        shell_panel(assets.as_deref(), 5),
                    ))
                    .with_children(|identity| {
                        shell_icon(identity, assets.as_deref(), 3, 26.0, CREAM);
                        identity.spawn((
                            Text::new(&membership.accepted_display_name),
                            TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
                            dashboard_font(assets.as_deref(), 22.0),
                            TextColor(CREAM),
                        ));
                        identity.spawn((
                            Node {
                                width: px(2),
                                height: px(24),
                                flex_shrink: 0.0,
                                ..default()
                            },
                            BackgroundColor(CREAM),
                        ));
                        shell_icon(identity, assets.as_deref(), 4, 28.0, TEAL);
                        identity.spawn((
                            Text::new(membership.server_name.to_uppercase()),
                            TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
                            dashboard_font(assets.as_deref(), 22.0),
                            TextColor(TEAL),
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
                    DASHBOARD_MENU_INDEX,
                    FlowUiAction::OpenDashboardMenu,
                    DashboardButtonPresentation {
                        label: "MENU",
                        width: px(92),
                        primary: false,
                        disabled: false,
                        assets: assets.as_deref(),
                        icon: assets
                            .as_deref()
                            .map(|assets| assets.dashboard_icons[2].clone()),
                    },
                );
            });
            if let Some(message) = dashboard_notice {
                root.spawn((
                    DashboardLayoutRole::Notice,
                    Text::new(message),
                    dashboard_font(assets.as_deref(), 18.0),
                    TextColor(INK),
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
                        border: UiRect::ZERO,
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
                center
                    .spawn((
                        DashboardLayoutRole::Build,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(12),
                            ..default()
                        },
                    ))
                    .with_children(|details| {
                        details.spawn((
                            Text::new(build_name.to_uppercase()),
                            TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
                            DashboardBrawlerNameLabel,
                            dashboard_font(assets.as_deref(), 64.0),
                            TextColor(INK),
                        ));
                        if let Some(assets) = assets.as_deref() {
                            details.spawn((
                                ImageNode::new(assets.dashboard_icons[7].clone())
                                    .with_mode(NodeImageMode::Stretch),
                                Node {
                                    width: percent(94),
                                    height: px(12),
                                    ..default()
                                },
                            ));
                        }
                        details.spawn((
                            Text::new(build_summary),
                            DashboardBrawlerSummaryLabel,
                            dashboard_font(assets.as_deref(), 20.0),
                            TextColor(INK),
                        ));
                        let mut button = details.spawn((
                            Button,
                            DashboardBuildCard,
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
                                width: percent(100),
                                min_height: px(80),
                                column_gap: px(10),
                                margin: UiRect::top(px(12)),
                                padding: UiRect::all(px(12)),
                                border: UiRect::ZERO,
                                border_radius: BorderRadius::all(px(5)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            BackgroundColor(TEAL),
                            BorderColor::all(INK),
                        ));
                        if admission_pending {
                            button.insert(InteractionDisabled);
                        }
                        button.insert(shell_panel(assets.as_deref(), 3));
                        button.with_children(|button| {
                            shell_icon(button, assets.as_deref(), 5, 38.0, CREAM);
                            button.spawn((
                                DashboardBrawlerActionLabel,
                                Text::new(if selected_brawler.is_some() {
                                    "VIEW BRAWLERS"
                                } else {
                                    "CREATE BRAWLER"
                                }),
                                dashboard_font(assets.as_deref(), 24.0),
                                TextColor(CREAM),
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
                        border: UiRect::ZERO,
                        border_radius: BorderRadius::all(px(16)),
                        ..default()
                    },
                    BackgroundColor(CREAM),
                    BorderColor::all(INK),
                ));
                if admission_pending {
                    mode_button.insert(InteractionDisabled);
                }
                mode_button.insert(shell_panel(assets.as_deref(), 0));
                mode_button.with_children(|card| {
                    if let Some(assets) = assets.as_deref() {
                        card.spawn((
                            ImageNode::new(
                                assets.dashboard_badges[dashboard_badge_index(game)].clone(),
                            ),
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(20),
                                top: px(-8),
                                width: px(64),
                                height: px(80),
                                ..default()
                            },
                        ));
                    }
                    if let Some(assets) = assets.as_deref() {
                        card.spawn((
                            ImageNode {
                                color: INK,
                                ..ImageNode::new(assets.dashboard_icons[6].clone())
                            },
                            Node {
                                position_type: PositionType::Absolute,
                                right: px(26),
                                bottom: px(30),
                                width: px(36),
                                height: px(44),
                                ..default()
                            },
                        ));
                    }
                    card.spawn((Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },))
                        .with_children(|details| {
                            details.spawn((
                                Text::new(dashboard_mode_title(game)),
                                TextFont {
                                    font_size: FontSize::Vh(10.0),
                                    ..dashboard_font(assets.as_deref(), 42.0)
                                },
                                TextColor(INK),
                            ));
                            details.spawn((
                                Text::new(game_summary),
                                DashboardGameSummaryLabel,
                                dashboard_font(assets.as_deref(), 32.0),
                                TextColor(INK),
                                TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                            ));
                        });
                });
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
                        icon: assets
                            .as_deref()
                            .map(|assets| assets.dashboard_icons[0].clone()),
                    },
                );
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
                        icon: assets
                            .as_deref()
                            .map(|assets| assets.dashboard_icons[1].clone()),
                    },
                );
            });
        });
}

pub(in crate::client::flow) fn dashboard_game_summary(
    game: &crate::lobby::AdvertisedGameType,
) -> String {
    format!(
        "{}, {}",
        dashboard_mode_title(game),
        dashboard_team_summary(game)
    )
}

fn dashboard_team_summary(game: &crate::lobby::AdvertisedGameType) -> String {
    if game.team_count == 1 {
        format!("{} PLAYERS · CO-OP", game.players_per_team)
    } else {
        std::iter::repeat_n(
            game.players_per_team.to_string(),
            usize::from(game.team_count),
        )
        .collect::<Vec<_>>()
        .join("v")
    }
}

fn dashboard_mode_title(game: &crate::lobby::AdvertisedGameType) -> &'static str {
    match game.rules_summary {
        crate::lobby::AdvertisedRulesSummary::Wipeout { .. } => "WIPEOUT",
        crate::lobby::AdvertisedRulesSummary::HotZone { .. } => "HOT ZONE",
        crate::lobby::AdvertisedRulesSummary::Heist { .. } => "HEIST",
        crate::lobby::AdvertisedRulesSummary::Zombie { .. } => "ZOMBIE",
    }
}

fn dashboard_badge_index(game: &crate::lobby::AdvertisedGameType) -> usize {
    match game.rules_summary {
        crate::lobby::AdvertisedRulesSummary::Wipeout { .. } => 0,
        crate::lobby::AdvertisedRulesSummary::HotZone { .. } => 1,
        crate::lobby::AdvertisedRulesSummary::Heist { .. } => 2,
        crate::lobby::AdvertisedRulesSummary::Zombie { .. } => 3,
    }
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
        &mut TextFont,
        Has<DashboardGameSummaryLabel>,
        Has<DashboardPlayLabel>,
        Has<DashboardPracticeLabel>,
        Has<DashboardBrawlerNameLabel>,
        Has<DashboardBrawlerSummaryLabel>,
        Has<DashboardBrawlerActionLabel>,
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
    let copy = dashboard_team_summary(game);
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
                dashboard_build_summary(brawler, &membership.brawler_catalog)
            )
        },
    );
    for (mut text, _, is_summary, _, _, is_brawler_name, is_brawler_summary, is_brawler_action) in
        &mut texts
    {
        if is_brawler_action {
            text.0 = if selected_brawler.is_some() {
                "VIEW BRAWLERS"
            } else {
                "CREATE BRAWLER"
            }
            .to_string();
        } else if is_summary {
            text.0.clone_from(&copy);
        } else if is_brawler_name {
            text.0 = selected_brawler.map_or_else(
                || "CREATE YOUR FIRST BRAWLER".to_string(),
                |brawler| brawler.name.to_uppercase(),
            );
        } else if is_brawler_summary {
            text.0 = selected_brawler.map_or_else(
                || "Choose a permanent fighter profile and weapon base".to_string(),
                |brawler| dashboard_build_summary(brawler, &membership.brawler_catalog),
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
                "Change game type: {}, {}{}",
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
    for (mut text, mut font, _, is_play_label, is_practice_label, _, _, _) in &mut texts {
        if is_play_label {
            text.0 = if queue.pending().is_some() {
                "JOINING...".to_string()
            } else if capacity_occupied {
                "MATCH IN PROGRESS".to_string()
            } else {
                "PLAY".to_string()
            };
            font.font_size = if text.0 == "PLAY" {
                FontSize::Vh(14.5)
            } else {
                FontSize::Px(22.0)
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

pub(in crate::client::flow) fn scroll_dashboard(
    mut wheel: MessageReader<MouseWheel>,
    mut roots: Query<
        (&DashboardLayoutClass, &ComputedNode, &mut ScrollPosition),
        With<DashboardRoot>,
    >,
) {
    let delta = normalized_wheel_delta(wheel.read(), 24.0);
    if delta.abs() <= f32::EPSILON {
        return;
    }
    for (class, node, mut position) in &mut roots {
        if *class == DashboardLayoutClass::Compact {
            position.0.y = clamp_scroll_offset(
                position.0.y - delta,
                node.content_size().y,
                node.size().y,
                node.inverse_scale_factor(),
            );
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "computed UI bounds are available only after Bevy's layout pass"
)]
pub(in crate::client::flow) fn keep_dashboard_focus_visible(
    navigation: Res<FlowNavigation>,
    overlay: Res<ClientOverlay>,
    buttons: Query<(&FlowButton, &ComputedNode, &UiGlobalTransform), Without<DashboardPreviewHost>>,
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
    if !matches!(overlay.as_ref(), ClientOverlay::None) {
        *prior = None;
        return;
    }
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
        let offset = offset_keeping_interval_visible(
            scroll.0.y,
            visible_top..visible_bottom,
            focused_top..focused_bottom,
            root_node.inverse_scale_factor(),
        );
        scroll.0.y = clamp_scroll_offset(
            offset,
            root_node.content_size().y,
            root_node.size().y,
            root_node.inverse_scale_factor(),
        );
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
    Utility,
    Other,
}

const fn dashboard_button_icon_size(is_play: bool, is_practice: bool) -> f32 {
    if is_play {
        78.0
    } else if is_practice {
        44.0
    } else {
        28.0
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
    let utility_width = is_utility.then_some(148.0);
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
            column_gap: px(if is_play || is_practice { 24 } else { 10 }),
            padding: UiRect::axes(px(12), px(7)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::ZERO,
            border_radius: BorderRadius::all(px(5)),
            ..default()
        },
        BackgroundColor(if primary { ORANGE } else { TEAL }),
        BorderColor::all(Color::NONE),
    ));
    button.insert(shell_panel(
        assets,
        if is_play {
            1
        } else if is_practice {
            2
        } else {
            4
        },
    ));
    if is_play {
        button.insert((
            DashboardButtonStyle::Play,
            DashboardLayoutRole::ActionButton,
        ));
    } else if is_practice {
        button.insert((
            DashboardButtonStyle::Practice,
            DashboardLayoutRole::ActionButton,
        ));
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
        DashboardButtonContentKind::Utility
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
    let icon_node = icon.map(|icon| {
        (
            ImageNode {
                color: INK,
                ..ImageNode::new(icon)
            },
            Node {
                width: px(icon_size),
                height: px(icon_size),
                flex_shrink: 0.0,
                ..default()
            },
        )
    });
    if matches!(kind, DashboardButtonContentKind::Utility)
        && let Some(icon) = icon_node.clone()
    {
        parent.spawn(icon);
    }
    let font_size = match kind {
        DashboardButtonContentKind::Play => 64.0,
        DashboardButtonContentKind::Practice => 40.0,
        DashboardButtonContentKind::Utility | DashboardButtonContentKind::Other => 24.0,
    };
    let color = INK;
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
        DashboardButtonContentKind::Utility => {
            text.insert(DashboardLayoutRole::UtilityLabel);
        }
        DashboardButtonContentKind::Other => {}
    }
    if !matches!(kind, DashboardButtonContentKind::Utility)
        && let Some(icon) = icon_node
    {
        parent.spawn(icon);
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
    assets: Option<Res<crate::client::ClientAssetHandles>>,
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
                BackgroundColor(CREAM),
                BorderColor::all(INK),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("MENU"),
                    dashboard_font(assets.as_deref(), 34.0),
                    TextColor(INK),
                ));
                spawn_menu_button(
                    panel,
                    assets.as_deref(),
                    0,
                    FlowUiAction::OpenSettings,
                    "SETTINGS",
                );
                let mut index = 1;
                spawn_menu_button(
                    panel,
                    assets.as_deref(),
                    index,
                    FlowUiAction::OpenCredits,
                    "CREDITS",
                );
                index += 1;
                if let Some(favorite_label) = favorite_label {
                    spawn_menu_button(
                        panel,
                        assets.as_deref(),
                        index,
                        FlowUiAction::ToggleFavoriteServer,
                        favorite_label,
                    );
                    index += 1;
                }
                spawn_menu_button(
                    panel,
                    assets.as_deref(),
                    index,
                    FlowUiAction::RequestChangeServer,
                    "CHANGE SERVER",
                );
                index += 1;
                spawn_menu_button(panel, assets.as_deref(), index, FlowUiAction::Quit, "QUIT");
                spawn_menu_button(
                    panel,
                    assets.as_deref(),
                    index + 1,
                    FlowUiAction::CloseDashboardMenu,
                    "BACK",
                );
            });
        });
}

fn spawn_menu_button(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    index: usize,
    action: FlowUiAction,
    label: &str,
) {
    parent
        .spawn((
            Button,
            AccessibleLabel::new(label),
            DashboardButtonStyle::Header,
            FlowButton {
                index,
                action,
                error_action: true,
            },
            Node {
                width: percent(100),
                min_height: px(48),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(5)),
                ..default()
            },
            BackgroundColor(ORANGE),
            BorderColor::all(INK),
        ))
        .with_child((
            Text::new(label),
            dashboard_font(assets, 22.0),
            TextColor(INK),
        ));
}

// Artwork contains only chrome; all text and actions remain live Bevy UI.
fn shell_panel(assets: Option<&crate::client::ClientAssetHandles>, index: usize) -> ImageNode {
    ImageNode {
        image: assets.map_or_else(
            || ImageNode::default().image,
            |assets| assets.dashboard_panels[index].clone(),
        ),
        image_mode: NodeImageMode::Stretch,
        visual_box: VisualBox::BorderBox,
        ..default()
    }
}

fn shell_icon(
    parent: &mut ChildSpawnerCommands,
    assets: Option<&crate::client::ClientAssetHandles>,
    index: usize,
    size: f32,
    color: Color,
) {
    if let Some(assets) = assets {
        parent.spawn((
            ImageNode {
                color,
                ..ImageNode::new(assets.dashboard_icons[index].clone())
            },
            Node {
                width: px(size),
                height: px(size),
                flex_shrink: 0.0,
                ..default()
            },
        ));
    }
}

fn dashboard_build_summary(
    brawler: &crate::profiles::SavedBrawler,
    catalog: &crate::profiles::AdvertisedBrawlerCatalog,
) -> String {
    let weapon = catalog
        .weapon(brawler.weapon_base_id)
        .map_or("Weapon", |weapon| weapon.display_name.as_str());
    format!(
        "{} · {}\n{} passives · {} part slots",
        weapon,
        ultimate_name(catalog, brawler.ultimate_id),
        brawler.passive_ids.len(),
        catalog.limits.weapon_part_slot_count
    )
}
