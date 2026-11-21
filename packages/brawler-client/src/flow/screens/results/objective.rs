//! Objective-only results share the approved shell without fabricated player statistics.
use super::ResultsPainter;
use crate::ui::{
    surface::{Surface, SurfaceState},
    theme::{INK, PANEL, PANEL_RAISED, PLAY},
};
use bevy::prelude::*;
use brawler::combat::TeamId;
use brawler::matchplay::ObjectiveTotals;

fn values(totals: ObjectiveTotals, team: usize) -> (&'static str, String, f32) {
    let (label, current, maximum) = match totals {
        ObjectiveTotals::TurfHold { progress, target } => {
            ("CAPTURE PROGRESS", progress[team], target)
        }
        ObjectiveTotals::VaultBreak { health, maximum } => {
            ("IDOL HEALTH", health[team], maximum[team])
        }
        ObjectiveTotals::ShardRush { shards, target } => {
            ("SHARDS HELD", u16::from(shards[team]), u16::from(target))
        }
        ObjectiveTotals::LastStand { .. } => ("", 0, 0),
    };
    let percent = f32::from(current) * 100. / f32::from(maximum.max(1));
    let detail = match totals {
        ObjectiveTotals::TurfHold { .. } => format!("{percent:.1}%"),
        ObjectiveTotals::VaultBreak { .. }
        | ObjectiveTotals::ShardRush { .. }
        | ObjectiveTotals::LastStand { .. } => {
            format!("{current} / {maximum}")
        }
    };
    (label, detail, percent.clamp(0., 100.))
}

/// Last Stand standings in finishing order: `(place, team, label)`, shared places kept.
fn standings(placements: &[u8], fighters: u8, local: Option<TeamId>) -> Vec<(u8, TeamId, String)> {
    let mut rows: Vec<_> = placements
        .iter()
        .take(usize::from(fighters))
        .enumerate()
        .filter(|(_, place)| **place != 0)
        .filter_map(|(team, place)| {
            let team = TeamId(u8::try_from(team).ok()?);
            let label = if local == Some(team) {
                "YOU".to_owned()
            } else {
                format!("FIGHTER {}", team.0 + 1)
            };
            Some((*place, team, label))
        })
        .collect();
    rows.sort_by_key(|(place, team, _)| (*place, team.0));
    rows
}

impl ResultsPainter<'_> {
    fn last_stand(&self, p: &mut ChildSpawnerCommands, placements: &[u8], fighters: u8) {
        let local = self.context.local_team;
        p.spawn((
            super::ResultsLayoutRole::TeamCard,
            Surface::Panel,
            SurfaceState {
                current: true,
                ..default()
            },
            Node {
                width: percent(100),
                max_width: px(720),
                flex_shrink: 0.,
                padding: UiRect::all(px(if self.layout.compact { 20. } else { 32. })),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(if self.layout.short { 4. } else { 10. }),
                margin: UiRect::vertical(px(if self.layout.short { 0. } else { 16. })),
                ..default()
            },
        ))
        .with_children(|card| {
            if let Some(place) = local
                .and_then(|team| placements.get(usize::from(team.0)).copied())
                .filter(|place| *place != 0)
            {
                self.text(
                    card,
                    "YOUR PLACE",
                    if self.layout.compact { 19. } else { 24. },
                );
                self.text(
                    card,
                    &format!("#{place} OF {fighters}"),
                    if self.layout.compact { 42. } else { 56. },
                );
            }
            for (place, _, label) in standings(placements, fighters, local) {
                self.text(
                    card,
                    &format!("#{place}  {label}"),
                    if self.layout.compact { 18. } else { 22. },
                );
            }
        });
    }

    pub(super) fn objective(&self, p: &mut ChildSpawnerCommands, totals: ObjectiveTotals) {
        if let ObjectiveTotals::LastStand {
            placements,
            fighters,
        } = totals
        {
            self.last_stand(p, &placements, fighters);
        } else {
            self.team_totals(p, totals);
        }
    }

    /// Two-team objective cards: the local team first, each with its final objective bar.
    fn team_totals(&self, p: &mut ChildSpawnerCommands, totals: ObjectiveTotals) {
        p.spawn(Node {
            width: percent(100),
            max_width: px(1200),
            flex_shrink: 0.,
            flex_direction: if self.layout.stacked {
                FlexDirection::Column
            } else {
                FlexDirection::Row
            },
            column_gap: px(28),
            row_gap: px(22),
            margin: UiRect::vertical(px(if self.layout.short { 0. } else { 16. })),
            ..default()
        })
        .with_children(|teams| {
            let first = self.context.local_team.unwrap_or(TeamId(0));
            for team in [first, TeamId(1 - first.0.min(1))] {
                let ours = self.context.local_team == Some(team);
                let (label, detail, amount) = values(totals, usize::from(team.0));
                teams
                    .spawn((
                        super::ResultsLayoutRole::TeamCard,
                        Surface::Panel,
                        SurfaceState {
                            current: ours,
                            ..default()
                        },
                        Node {
                            width: if self.layout.stacked {
                                percent(100)
                            } else {
                                px(0)
                            },
                            min_width: px(0),
                            flex_grow: 1.,
                            flex_shrink: 0.,
                            padding: if self.layout.short {
                                UiRect::axes(px(40), px(18))
                            } else {
                                UiRect::all(px(if self.layout.compact { 26. } else { 40. }))
                            },
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: px(if self.layout.short { 8. } else { 20. }),
                            ..default()
                        },
                    ))
                    .insert_if(crate::ui::surface::SurfaceBurst::default(), || {
                        ours && self.celebrate
                    })
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
                            if self.layout.compact { 26. } else { 36. },
                        );
                        self.text(card, label, if self.layout.compact { 19. } else { 24. });
                        self.text(
                            card,
                            &detail,
                            if self.layout.compact {
                                42.
                            } else if self.layout.short {
                                52.
                            } else {
                                64.
                            },
                        );
                        card.spawn((
                            BackgroundColor(INK.mix(&PANEL, 0.4)),
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
                                BackgroundColor(if ours { PLAY } else { PANEL_RAISED }),
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
    fn last_stand_standings_order_places_and_name_the_local_fighter() {
        let rows = standings(&[3, 1, 0, 3, 5, 6], 6, Some(TeamId(3)));
        assert_eq!(
            rows.iter()
                .map(|(place, _, label)| format!("#{place} {label}"))
                .collect::<Vec<_>>(),
            [
                "#1 FIGHTER 2",
                "#3 FIGHTER 1",
                "#3 YOU",
                "#5 FIGHTER 5",
                "#6 FIGHTER 6"
            ]
        );
    }
    #[test]
    fn objective_values_keep_exact_vault_break_health_and_capture_progress() {
        assert_eq!(
            values(
                ObjectiveTotals::VaultBreak {
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
                ObjectiveTotals::TurfHold {
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
                ObjectiveTotals::TurfHold {
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
