use super::super::InputSettingsDraft;
use super::model::ControlsState;
use crate::client::ui::theme;
use bevy::prelude::*;

#[derive(Component)]
pub(in crate::client::shell) struct StickPoint {
    index: usize,
    processed: bool,
}
#[derive(Component)]
pub(in crate::client::shell) struct Deadzone(usize);
#[derive(Component)]
pub(in crate::client::shell) struct StickStatus(usize);

pub(super) fn spawn(parent: &mut ChildSpawnerCommands, font: &TextFont) {
    parent.spawn((
        Text::new("Orange: input · Teal: output"),
        TextFont {
            font_size: 17.0.into(),
            ..font.clone()
        },
        TextColor(theme::INK),
    ));
    parent
        .spawn(Node {
            width: percent(100),
            flex_wrap: FlexWrap::Wrap,
            column_gap: px(18),
            row_gap: px(12),
            ..default()
        })
        .with_children(|plots| {
            for (index, label) in ["Movement", "Aim"].into_iter().enumerate() {
                plots
                    .spawn(Node {
                        width: px(150),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(8),
                        flex_shrink: 0.0,
                        ..default()
                    })
                    .with_children(|column| {
                        column.spawn((Text::new(label), font.clone(), TextColor(theme::INK)));
                        column
                            .spawn((
                                Node {
                                    width: px(150),
                                    height: px(150),
                                    border: UiRect::all(px(2)),
                                    border_radius: BorderRadius::all(percent(50)),
                                    ..default()
                                },
                                BorderColor::all(theme::INK),
                            ))
                            .with_children(|plot| {
                                for horizontal in [true, false] {
                                    plot.spawn((
                                        Node {
                                            position_type: PositionType::Absolute,
                                            left: if horizontal { percent(0) } else { percent(50) },
                                            top: if horizontal { percent(50) } else { percent(0) },
                                            width: if horizontal { percent(100) } else { px(1) },
                                            height: if horizontal { px(1) } else { percent(100) },
                                            ..default()
                                        },
                                        BackgroundColor(theme::INK.with_alpha(0.25)),
                                    ));
                                }
                                plot.spawn((
                                    Deadzone(index),
                                    Node {
                                        position_type: PositionType::Absolute,
                                        border_radius: BorderRadius::all(percent(50)),
                                        ..default()
                                    },
                                    BackgroundColor(theme::TEAL.with_alpha(0.3)),
                                ));
                                for processed in [false, true] {
                                    plot.spawn((
                                        StickPoint { index, processed },
                                        Node {
                                            position_type: PositionType::Absolute,
                                            width: px(9),
                                            height: px(9),
                                            margin: UiRect::all(px(-4.5)),
                                            border_radius: BorderRadius::all(percent(50)),
                                            ..default()
                                        },
                                        BackgroundColor(if processed {
                                            theme::TEAL
                                        } else {
                                            theme::ORANGE
                                        }),
                                    ));
                                }
                            });
                        column.spawn((
                            StickStatus(index),
                            Text::new(""),
                            TextFont {
                                font_size: 17.0.into(),
                                ..font.clone()
                            },
                            TextColor(theme::INK),
                        ));
                    });
            }
        });
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Live stick plot presentation"
)]
pub(in crate::client::shell) fn present_plots(
    state: Res<ControlsState>,
    draft: Option<Res<InputSettingsDraft>>,
    mut points: Query<(&StickPoint, &mut Node), Without<Deadzone>>,
    mut zones: Query<(&Deadzone, &mut Node), Without<StickPoint>>,
    mut labels: Query<(&StickStatus, &mut Text)>,
) {
    let Some(draft) = draft else {
        return;
    };
    let mut settings = draft.0;
    if let Some(recommendation) = state.recommendation {
        recommendation.apply(&mut settings);
    }
    let processed = [
        settings.shape_move(state.raw[0]),
        settings
            .shape_targeting_axis(state.raw[1])
            .unwrap_or(Vec2::ZERO),
    ];
    for (point, mut node) in &mut points {
        let value = if point.processed {
            processed[point.index]
        } else {
            state.raw[point.index]
        };
        let value = if value.is_finite() {
            value.clamp_length_max(1.0)
        } else {
            Vec2::ZERO
        };
        node.left = percent(50.0 + value.x * 50.0);
        node.top = percent(50.0 - value.y * 50.0);
    }
    for (zone, mut node) in &mut zones {
        let radius = if zone.0 == 0 {
            settings.move_deadzone
        } else {
            settings.aim_deadzone
        };
        node.width = percent(radius * 100.0);
        node.height = percent(radius * 100.0);
        node.left = percent(50.0 - radius * 50.0);
        node.top = percent(50.0 - radius * 50.0);
    }
    for (status, mut text) in &mut labels {
        text.0 = if state.device.is_none() {
            "No controller".into()
        } else {
            format!(
                "{}\n{:.2} → {:.2}",
                if processed[status.0] == Vec2::ZERO {
                    "Neutral"
                } else {
                    "Active"
                },
                state.raw[status.0].length(),
                processed[status.0].length()
            )
        };
    }
}
