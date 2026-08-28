//! Game-type catalog selection, scrolling, and focus visibility.

use super::scroll::{clamp_scroll_offset, normalized_wheel_delta, offset_keeping_interval_visible};
use super::{
    queue::queue_population,
    shared::{
        FlowButton, FlowNavigation, FlowRoot, flow_root_node, spawn_flow_button,
        spawn_flow_button_disabled, spawn_heading,
    },
};
use crate::client::{
    ClientLobbyMembership, ClientQueueModel,
    flow::{actions::FlowUiAction, model::ClientFlow},
};
use bevy::{input::mouse::MouseWheel, prelude::*, ui::ScrollPosition};
use lightyear::prelude::client::Client;

pub(in crate::client::flow) const GAME_TYPE_CONFIRM_INDEX: usize = 1_000;
pub(in crate::client::flow) const GAME_TYPE_BACK_INDEX: usize = 1_001;

#[derive(Resource, Default, Clone, Debug, PartialEq, Eq)]
pub(in crate::client::flow) struct GameTypeSelectionDraft {
    pub(in crate::client::flow) selected_index: Option<usize>,
    pub(in crate::client::flow) unavailable_previous: bool,
}

#[derive(Component)]
pub(in crate::client::flow) struct GamePopulationLabel(usize);

#[derive(Component)]
pub(in crate::client::flow) struct GameTypeSelectRoot;

#[allow(
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "the bounded game-select root renders the complete catalog card contract"
)]
pub(in crate::client::flow) fn spawn_game_type_select(
    mut commands: Commands,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    mut navigation: ResMut<FlowNavigation>,
    draft: Res<GameTypeSelectionDraft>,
    queue: Res<ClientQueueModel>,
) {
    let Some(membership) = memberships.iter().next() else {
        return;
    };
    let selected_index = draft.selected_index.unwrap_or(0);
    navigation.selected = selected_index;
    let map_catalog = crate::map::MapContentCatalog::embedded().ok();
    commands
        .spawn((
            FlowRoot,
            GameTypeSelectRoot,
            DespawnOnExit(ClientFlow::GameTypeSelect),
            flow_root_node(),
            ScrollPosition::default(),
            BackgroundColor(Color::srgb(0.025, 0.04, 0.07)),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            spawn_heading(root, "SELECT GAME TYPE");
            root.spawn(Text::new(format!(
                "{} · {}",
                membership.server_name, membership.accepted_display_name
            )));
            if draft.unavailable_previous {
                root.spawn((
                    Text::new("Your previous game is no longer available. Choose a replacement."),
                    TextColor(Color::srgb(1.0, 0.72, 0.28)),
                ));
            }
            for (index, game_type) in membership.game_types.iter().enumerate() {
                let mode_name =
                    if game_type.mode_definition_id == crate::map::WIPEOUT_MODE_DEFINITION {
                        "Wipeout"
                    } else if game_type.mode_definition_id == crate::map::HOT_ZONE_MODE_DEFINITION {
                        "Hot Zone"
                    } else if game_type.mode_definition_id == crate::map::HEIST_MODE_DEFINITION {
                        "Heist"
                    } else {
                        "Unknown mode"
                    };
                let map_names = game_type
                    .map_preset_ids
                    .iter()
                    .map(|id| {
                        map_catalog
                            .as_ref()
                            .and_then(|catalog| {
                                catalog.presets.iter().find(|preset| preset.id == *id)
                            })
                            .map_or_else(
                                || format!("Map {}", id.0),
                                |preset| preset.display_name.clone(),
                            )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let rules = match game_type.rules_summary {
                    crate::lobby::AdvertisedRulesSummary::Wipeout {
                        target_score,
                        active_limit_ticks,
                    } => format!(
                        "first to {target_score}; {}s limit",
                        active_limit_ticks / 60
                    ),
                    crate::lobby::AdvertisedRulesSummary::HotZone {
                        target_progress_ticks,
                        active_limit_ticks,
                    } => format!(
                        "hold {}s; {}s limit",
                        target_progress_ticks / 60,
                        active_limit_ticks / 60
                    ),
                    crate::lobby::AdvertisedRulesSummary::Heist {
                        safe_maximum_health,
                        active_limit_ticks,
                    } => format!(
                        "{safe_maximum_health} HP idol; {}s limit",
                        active_limit_ticks / 60
                    ),
                };
                spawn_flow_button(
                    root,
                    index,
                    FlowUiAction::SelectGameTypeDraft(index),
                    &format!(
                        "{} | {mode_name} | {}v{} | {map_names} | {rules}",
                        game_type.display_name,
                        game_type.players_per_team,
                        game_type.players_per_team,
                    ),
                );
                root.spawn((
                    GamePopulationLabel(index),
                    Text::new(queue_population(&queue, game_type)),
                    TextFont::from_font_size(15.0),
                    TextColor(Color::srgb(0.68, 0.78, 0.86)),
                ));
            }
            spawn_flow_button_disabled(
                root,
                GAME_TYPE_CONFIRM_INDEX,
                FlowUiAction::ConfirmGameType,
                "CONFIRM",
                draft.selected_index.is_none(),
            );
            spawn_flow_button(
                root,
                GAME_TYPE_BACK_INDEX,
                FlowUiAction::CancelGameType,
                "BACK",
            );
        });
}

#[allow(clippy::needless_pass_by_value)]
pub(in crate::client::flow) fn update_game_population(
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    queue: Res<ClientQueueModel>,
    mut labels: Query<(&GamePopulationLabel, &mut Text)>,
) {
    let Some(membership) = memberships.iter().next() else {
        return;
    };
    for (label, mut text) in &mut labels {
        if let Some(game) = membership.game_types.get(label.0) {
            text.0 = queue_population(&queue, game);
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::client::flow) fn scroll_game_type_select(
    flow: Res<State<ClientFlow>>,
    mut wheel: MessageReader<MouseWheel>,
    mut roots: Query<(&ComputedNode, &mut ScrollPosition), With<GameTypeSelectRoot>>,
) {
    if *flow.get() != ClientFlow::GameTypeSelect {
        return;
    }
    let delta = normalized_wheel_delta(wheel.read(), 24.0);
    if delta.abs() <= f32::EPSILON {
        return;
    }
    for (node, mut position) in &mut roots {
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
pub(in crate::client::flow) fn keep_game_type_focus_visible(
    flow: Res<State<ClientFlow>>,
    navigation: Res<FlowNavigation>,
    buttons: Query<(&FlowButton, &ComputedNode, &UiGlobalTransform)>,
    mut roots: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            &mut ScrollPosition,
        ),
        With<GameTypeSelectRoot>,
    >,
    mut prior: Local<Option<(Entity, usize)>>,
) {
    if *flow.get() != ClientFlow::GameTypeSelect {
        *prior = None;
        return;
    }
    let Some((root_entity, _, _, _)) = roots.iter_mut().next() else {
        *prior = None;
        return;
    };
    let focus_key = (root_entity, navigation.selected);
    if prior.as_ref() == Some(&focus_key) {
        return;
    }
    *prior = Some(focus_key);
    let Some((_, focused_node, focused_transform)) = buttons
        .iter()
        .find(|(button, _, _)| button.index == navigation.selected)
    else {
        return;
    };
    if focused_node.is_empty() {
        return;
    }
    let (_, _, focused_center) = focused_transform.to_scale_angle_translation();
    let focused_half_height = focused_node.size().y * 0.5;
    for (_, root_node, root_transform, mut scroll) in &mut roots {
        if root_node.is_empty() {
            continue;
        }
        let (_, _, root_center) = root_transform.to_scale_angle_translation();
        let root_half_height = root_node.size().y * 0.5;
        let visible_top = root_center.y - root_half_height + 8.0;
        let visible_bottom = root_center.y + root_half_height - 8.0;
        let focused_top = focused_center.y - focused_half_height;
        let focused_bottom = focused_center.y + focused_half_height;
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
