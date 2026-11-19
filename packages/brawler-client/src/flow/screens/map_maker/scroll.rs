//! Map Maker's vertical scroll areas: the library tiles, the asset list and the check list.
//! Views rebuild on structural edits, so each area's offset is remembered across rebuilds; the
//! mouse wheel scrolls the area under the pointer (touch drag-scrolling is the shell's generic
//! system), and keyboard/controller focus or a cycled asset is kept in view.

use super::MapMakerRuntime;
use crate::flow::screens::{
    scroll::{clamp_scroll_offset, normalized_wheel_delta, offset_keeping_interval_visible},
    shared::{FlowButton, FlowNavigation},
};
use bevy::{
    input::mouse::MouseWheel,
    prelude::*,
    ui::{RelativeCursorPosition, ScrollPosition},
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::flow) enum ScrollArea {
    Library,
    Assets,
    Check,
}

/// A remembered Map Maker scroll area.
#[derive(Component, Clone, Copy, Debug)]
#[require(RelativeCursorPosition)]
pub(in crate::flow) struct MapMakerScroll(pub(super) ScrollArea);

#[derive(Resource, Default, Debug)]
pub(in crate::flow) struct MapMakerScrollMemory(BTreeMap<ScrollArea, f32>);

impl MapMakerScrollMemory {
    /// The scroll position a rebuilt area starts at.
    pub(super) fn position(&self, area: ScrollArea) -> ScrollPosition {
        ScrollPosition(Vec2::new(0.0, self.0.get(&area).copied().unwrap_or(0.0)))
    }
}

/// Wheel lines scroll this many UI pixels.
const WHEEL_LINE: f32 = 48.0;

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::flow) fn scroll_map_maker(
    mut wheel: MessageReader<MouseWheel>,
    mut memory: ResMut<MapMakerScrollMemory>,
    mut areas: Query<(
        &MapMakerScroll,
        &ComputedNode,
        &RelativeCursorPosition,
        &mut ScrollPosition,
    )>,
) {
    let delta = normalized_wheel_delta(wheel.read(), WHEEL_LINE);
    for (area, computed, cursor, mut scroll) in &mut areas {
        if delta != 0.0 && cursor.cursor_over() {
            let next = clamp_scroll_offset(
                scroll.0.y - delta,
                computed.content_size().y,
                computed.size().y,
                computed.inverse_scale_factor(),
            );
            if (next - scroll.0.y).abs() > f32::EPSILON {
                scroll.0.y = next;
            }
        }
        if memory.0.get(&area.0) != Some(&scroll.0.y) {
            memory.0.insert(area.0, scroll.0.y);
        }
    }
}

/// Keeps the focused library control and the selected asset row inside their scroll area when
/// focus or the selection moves (not on every frame, so wheel and touch scrolling stay free).
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy presentation system parameters"
)]
pub(in crate::flow) fn reveal_map_maker_focus(
    navigation: Res<FlowNavigation>,
    runtime: Res<MapMakerRuntime>,
    mut last: Local<(Option<(usize, usize)>, bool)>,
    buttons: Query<(&FlowButton, &ComputedNode, &UiGlobalTransform, &ChildOf)>,
    parents: Query<&ChildOf>,
    mut areas: Query<
        (&ComputedNode, &UiGlobalTransform, &mut ScrollPosition),
        With<MapMakerScroll>,
    >,
) {
    // A move stays pending until its control is laid out (a rebuild lays out a frame later).
    let key = (navigation.focused, runtime.palette_index);
    if last.0 != Some(key) {
        *last = (Some(key), true);
    }
    if !last.1 {
        return;
    }
    for (button, computed, transform, parent) in &buttons {
        let revealed = match button.action {
            crate::flow::actions::FlowUiAction::MapMakerSelectAsset(index) => {
                index == runtime.palette_index
            }
            _ => button.index == navigation.focused,
        };
        if !revealed || computed.is_empty() {
            continue;
        }
        last.1 = false;
        let area = std::iter::successors(Some(parent.parent()), |entity| {
            parents.get(*entity).ok().map(ChildOf::parent)
        })
        .find(|entity| areas.contains(*entity));
        let Some(area) = area else {
            continue;
        };
        let Ok((area_node, area_transform, mut scroll)) = areas.get_mut(area) else {
            continue;
        };
        let half = area_node.size().y * 0.5;
        let viewport = (area_transform.translation.y - half)..(area_transform.translation.y + half);
        let control_half = computed.size().y * 0.5;
        let control =
            (transform.translation.y - control_half)..(transform.translation.y + control_half);
        let next = offset_keeping_interval_visible(
            scroll.0.y,
            viewport,
            control,
            area_node.inverse_scale_factor(),
        );
        if (next - scroll.0.y).abs() > 0.5 {
            scroll.0.y = next;
        }
    }
}
