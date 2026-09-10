//! Objective-only results share the approved shell without fabricated player statistics.
use super::ResultsPainter;
use crate::{client::ui::theme::TEAL, matchplay::ObjectiveTotals};
use crate::{
    client::ui::{
        surface::{Surface, SurfaceState},
        theme::{CREAM, INK},
    },
    combat::TeamId,
};
use bevy::prelude::*;

fn values(totals: ObjectiveTotals, team: usize) -> (&'static str, String, f32) {
    let (label, current, maximum) = match totals {
        ObjectiveTotals::HotZone { progress, target } => {
            ("CAPTURE PROGRESS", progress[team], target)
        }
        ObjectiveTotals::Heist { health, maximum } => ("IDOL HEALTH", health[team], maximum[team]),
    };
    let percent = f32::from(current) * 100. / f32::from(maximum.max(1));
    let detail = match totals {
        ObjectiveTotals::HotZone { .. } => format!("{percent:.1}%"),
        ObjectiveTotals::Heist { .. } => format!("{current} / {maximum}"),
    };
    (label, detail, percent.clamp(0., 100.))
}

impl ResultsPainter<'_> {
    pub(super) fn objective(&self, p: &mut ChildSpawnerCommands, totals: ObjectiveTotals) {
        p.spawn(Node {
            width: percent(100),
            max_width: px(1200),
            flex_shrink: 0.,
            flex_direction: if self.stacked {
                FlexDirection::Column
            } else {
                FlexDirection::Row
            },
            column_gap: px(28),
            row_gap: px(22),
            margin: UiRect::vertical(px(16)),
            ..default()
        })
        .with_children(|teams| {
            let first = self.context.local_team.unwrap_or(TeamId(0));
            for team in [first, TeamId(1 - first.0.min(1))] {
                let ours = self.context.local_team == Some(team);
                let (label, detail, amount) = values(totals, usize::from(team.0));
                teams
                    .spawn((
                        Surface::Panel,
                        SurfaceState {
                            selected: ours,
                            ..default()
                        },
                        Node {
                            width: if self.stacked { percent(100) } else { px(0) },
                            min_width: px(0),
                            flex_grow: 1.,
                            flex_shrink: 0.,
                            padding: UiRect::all(px(if self.compact { 26. } else { 40. })),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: px(20),
                            ..default()
                        },
                    ))
                    .with_children(|card| {
                        self.text(
                            card,
                            if ours {
                                "YOUR TEAM"
                            } else if self.context.local_team.is_some() {
                                "OPPONENTS"
                            } else if team.0 == 0 {
                                "TEAM 1"
                            } else {
                                "TEAM 2"
                            },
                            if self.compact { 26. } else { 36. },
                        );
                        self.text(card, label, if self.compact { 19. } else { 24. });
                        self.text(card, &detail, if self.compact { 42. } else { 64. });
                        card.spawn((
                            BackgroundColor(CREAM),
                            Node {
                                width: percent(100),
                                height: px(22),
                                border: UiRect::all(px(3)),
                                ..default()
                            },
                            BorderColor::all(INK),
                        ))
                        .with_children(|bar| {
                            bar.spawn((
                                BackgroundColor(if ours { INK } else { TEAL }),
                                Node {
                                    width: percent(amount),
                                    height: percent(100),
                                    ..default()
                                },
                            ));
                        });
                    });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn objective_values_keep_exact_heist_health_and_capture_progress() {
        assert_eq!(
            values(
                ObjectiveTotals::Heist {
                    health: [0, 731],
                    maximum: [2000, 2000]
                },
                1
            )
            .1,
            "731 / 2000"
        );
        assert_eq!(
            values(
                ObjectiveTotals::HotZone {
                    progress: [1800, 927],
                    target: 1800
                },
                1
            )
            .1,
            "51.5%"
        );
        assert_eq!(
            values(
                ObjectiveTotals::HotZone {
                    progress: [1800, 927],
                    target: 1800
                },
                0
            )
            .1,
            "100.0%"
        );
    }
}
