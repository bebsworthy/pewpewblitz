use crate::client::{
    flow::screens::shared::{FlowButton, FlowNavigation},
    ui::scroll::{clamp_scroll_offset, normalized_wheel_delta, offset_keeping_interval_visible},
};
use bevy::{input::mouse::MouseWheel, prelude::*};
#[derive(Component)]
pub(in crate::client::flow) struct ResultsAction;
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Native focus and scroll geometry"
)]
pub(in crate::client::flow) fn scroll_results(
    navigation: Res<FlowNavigation>,
    mut wheel: MessageReader<MouseWheel>,
    mut areas: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            &mut ScrollPosition,
        ),
        With<super::view::ResultsView>,
    >,
    buttons: Query<(&FlowButton, &ComputedNode, &UiGlobalTransform), With<ResultsAction>>,
    mut previous: Local<Option<(Entity, usize)>>,
) {
    let delta = normalized_wheel_delta(wheel.read(), 36.);
    let Some((entity, node, transform, mut scroll)) = areas.iter_mut().next() else {
        *previous = None;
        return;
    };
    scroll.0.y -= delta;
    let key = (entity, navigation.selected);
    if previous.as_ref() != Some(&key)
        && let Some((_, button, bt)) = buttons
            .iter()
            .find(|(b, _, _)| b.index == navigation.selected)
    {
        let (_, _, center) = transform.to_scale_angle_translation();
        let (_, _, bc) = bt.to_scale_angle_translation();
        if !node.is_empty() && !button.is_empty() {
            scroll.0.y = offset_keeping_interval_visible(
                scroll.0.y,
                center.y - node.size().y / 2. + 8. ..center.y + node.size().y / 2. - 8.,
                bc.y - button.size().y / 2. ..bc.y + button.size().y / 2.,
                node.inverse_scale_factor(),
            );
            *previous = Some(key);
        }
    }
    scroll.0.y = clamp_scroll_offset(
        scroll.0.y,
        node.content_size().y,
        node.size().y,
        node.inverse_scale_factor(),
    );
}
