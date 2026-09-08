//! Responsive Dashboard composition; compact layout keeps all actions in scroll flow.
use super::{
    DashboardBrawlerNameLabel, DashboardButtonStyle, DashboardLayoutClass, DashboardLayoutRole,
    DashboardPracticeLabel, DashboardRoot,
};
use bevy::prelude::*;
use bevy::{ui::UiScale, window::PrimaryWindow};
const DASHBOARD_COMPACT_WIDTH: f32 = 1_000.0;
const DASHBOARD_COMPACT_HEIGHT: f32 = 640.0;
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
    mut texts: Query<(
        &Text,
        &mut TextFont,
        Has<DashboardBrawlerNameLabel>,
        Has<DashboardPracticeLabel>,
    )>,
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
    let effective_width =
        window.resolution.width() / scale.as_deref().map_or(1.0, |scale| scale.0).max(0.01);
    let effective_height =
        window.resolution.height() / scale.as_deref().map_or(1.0, |scale| scale.0).max(0.01);
    let reference_scale = (effective_width / 1672.0).min(effective_height / 941.0);
    for (text, mut font, name, practice) in &mut texts {
        if name {
            let width = if next == DashboardLayoutClass::Compact {
                effective_width - 32.0
            } else {
                effective_width * 0.61 * 0.36
            };
            let fitted = width
                / (f32::from(
                    u16::try_from(
                        text.0
                            .split_whitespace()
                            .map(|word| word.chars().count())
                            .max()
                            .unwrap_or(1)
                            .max(1),
                    )
                    .unwrap_or(u16::MAX),
                ) * 0.72);
            font.font_size = FontSize::Px(fitted.min(84.0 * reference_scale.max(0.7)).max(26.0));
        } else if practice {
            font.font_size = FontSize::Px((52.0 * reference_scale).clamp(30.0, 54.0));
        }
    }
    let Some((mut current, mut scroll)) = roots.iter_mut().next() else {
        return;
    };
    if !current.is_added() && *current == next {
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
            node.padding = UiRect::all(px(if compact { 16 } else { 0 }));
            node.row_gap = px(18);
            node.overflow = if compact {
                Overflow::scroll_y()
            } else {
                Overflow::clip()
            };
        }
        DashboardLayoutRole::Notice => {
            node.position_type = if compact {
                PositionType::Relative
            } else {
                PositionType::Absolute
            };
            node.left = if compact { auto() } else { percent(35) };
            node.top = if compact { auto() } else { percent(17) };
            node.width = percent(if compact { 100 } else { 60 });
        }
        DashboardLayoutRole::Header => {
            node.flex_wrap = if compact {
                FlexWrap::Wrap
            } else {
                FlexWrap::NoWrap
            };
            node.row_gap = px(10);
            node.position_type = if compact {
                PositionType::Relative
            } else {
                PositionType::Absolute
            };
            node.top = if compact { auto() } else { percent(4) };
            node.left = if compact { auto() } else { percent(2) };
            node.width = percent(if compact { 100 } else { 96 });
            node.align_items = AlignItems::Start;
            node.column_gap = px(12);
            node.padding = UiRect::ZERO;
            node.flex_shrink = 0.0;
        }
        DashboardLayoutRole::Wordmark => {
            node.width = if compact { px(130) } else { percent(29) };
            node.height = auto();
            node.flex_shrink = 0.0;
        }
        DashboardLayoutRole::Identity => {
            node.min_width = px(0);
            node.max_width = percent(if compact { 100 } else { 47 });
            node.min_height = px(52);
            node.padding = UiRect::axes(px(22), px(10));
            node.border_radius = BorderRadius::all(px(5));
        }
        DashboardLayoutRole::HeaderSpacer => {
            node.flex_grow = 1.0;
        }
        DashboardLayoutRole::Center => {
            node.position_type = if compact {
                PositionType::Relative
            } else {
                PositionType::Absolute
            };
            node.top = if compact { auto() } else { percent(27) };
            node.bottom = if compact { auto() } else { percent(8) };
            node.left = if compact { auto() } else { percent(2) };
            node.width = percent(if compact { 100 } else { 61 });
            node.flex_grow = 0.0;
            node.flex_shrink = 0.0;
            node.row_gap = px(16);
        }
        DashboardLayoutRole::Preview => {
            node.position_type = if compact {
                PositionType::Relative
            } else {
                PositionType::Absolute
            };
            node.right = if compact { auto() } else { percent(0) };
            node.top = if compact { auto() } else { percent(0) };
            node.width = percent(if compact { 100 } else { 70 });
            node.height = if compact { px(260) } else { percent(100) };
            node.min_height = px(0);
            node.max_height = auto();
            node.max_width = auto();
            node.flex_grow = 0.0;
            node.flex_shrink = 0.0;
            node.border_radius = BorderRadius::all(px(5));
        }
        DashboardLayoutRole::Build => {
            node.position_type = if compact {
                PositionType::Relative
            } else {
                PositionType::Absolute
            };
            node.left = if compact { auto() } else { percent(0) };
            node.bottom = if compact { auto() } else { percent(0) };
            node.width = percent(if compact { 100 } else { 36 });
            node.max_width = auto();
            node.min_height = px(132);
            node.padding = UiRect::all(px(0));
            node.row_gap = px(16);
            node.border_radius = BorderRadius::all(px(5));
        }
        DashboardLayoutRole::ActionRow => {
            node.position_type = if compact {
                PositionType::Relative
            } else {
                PositionType::Absolute
            };
            node.right = if compact { auto() } else { percent(4) };
            node.top = if compact { auto() } else { percent(26) };
            node.bottom = if compact { auto() } else { percent(8) };
            node.width = percent(if compact { 100 } else { 34 });
            node.max_width = auto();
            node.flex_direction = FlexDirection::Column;
            node.row_gap = px(if compact { 16 } else { 18 });
            node.flex_shrink = 0.0;
        }
        DashboardLayoutRole::Mode => {
            node.width = percent(if compact { 100 } else { 96 });
            node.align_self = AlignSelf::End;
            node.min_height = px(210);
            node.flex_grow = 1.6;
            node.padding = UiRect {
                top: px(64),
                bottom: px(22),
                left: px(20),
                right: px(20),
            };
            node.border_radius = BorderRadius::all(px(5));
        }
        DashboardLayoutRole::UtilityButton { wide_width } => {
            node.width = px(if compact { 132.0 } else { wide_width });
            node.min_height = px(54);
            node.padding = UiRect::axes(px(10), px(7));
        }
        DashboardLayoutRole::ActionButton => {}
        DashboardLayoutRole::UtilityLabel => {
            node.display = Display::Flex;
        }
    }
    match style {
        Some(DashboardButtonStyle::Practice) => {
            node.width = percent(100);
            node.min_height = px(84);
            node.width = percent(if compact { 100 } else { 96 });
            node.flex_grow = 0.0;
            node.flex_shrink = 0.0;
        }
        Some(DashboardButtonStyle::Play) => {
            node.width = percent(100);
            node.min_height = px(135);
            node.flex_grow = 1.0;
            node.flex_shrink = 0.0;
        }
        _ => {}
    }
}
