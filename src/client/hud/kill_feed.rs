//! Bounded kill feed fed only by the defeat cues this client received.
//!
//! The server sends a defeat cue to an observer only when the victim and any initiating fighter are
//! relevant to that observer (`combat::authority::combat_cue_subjects`), so the feed never names a
//! concealed fighter. Names come from the public participant roster; defeats of deployables and
//! other non-participants are skipped.

use super::palette;
use crate::client::ui;
use crate::combat::client::DeduplicatedCombatCue;
use crate::combat::{CombatCue, DamageSource, TeamId};
use crate::matchplay::{MatchId, PublicParticipantState};
use crate::protocol::PlayerId;
use bevy::prelude::*;
use std::collections::VecDeque;

/// Rows shown at once; older entries are dropped.
pub(super) const KILL_FEED_ROWS: usize = 4;
/// Seconds an entry stays on screen.
const KILL_FEED_SECONDS: f32 = 4.0;
const NAME_MAX_WIDTH: f32 = 118.0;
/// Killer label for defeats by hostile NPCs (Zombie mode).
const HOSTILE_NPC_NAME: &str = "ZOMBIE";

/// How a named fighter relates to the observer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FeedRelation {
    Local,
    Ally,
    Enemy,
}

impl FeedRelation {
    fn color(self) -> Color {
        match self {
            Self::Local => palette::LOCAL,
            Self::Ally => palette::ALLY,
            Self::Enemy => palette::ENEMY,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KillFeedEntry {
    pub(crate) killer: Option<(String, FeedRelation)>,
    pub(crate) victim: (String, FeedRelation),
    age: f32,
}

/// Newest-first bounded feed for the current match.
#[derive(Resource, Clone, Debug, Default)]
pub(crate) struct KillFeed {
    match_id: Option<MatchId>,
    pub(crate) entries: VecDeque<KillFeedEntry>,
}

impl KillFeed {
    fn push(&mut self, entry: KillFeedEntry) {
        self.entries.push_front(entry);
        self.entries.truncate(KILL_FEED_ROWS);
    }

    fn advance(&mut self, seconds: f32) {
        for entry in &mut self.entries {
            entry.age += seconds;
        }
        self.entries.retain(|entry| entry.age < KILL_FEED_SECONDS);
    }
}

/// The observer: its player and team, when it controls a fighter.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FeedObserver {
    pub(crate) player: PlayerId,
    pub(crate) team: TeamId,
}

fn relation(participant: &PublicParticipantState, observer: Option<FeedObserver>) -> FeedRelation {
    match observer {
        Some(observer) if observer.player == participant.player_id => FeedRelation::Local,
        Some(observer) if observer.team == participant.team_id => FeedRelation::Ally,
        None if participant.team_id == TeamId(0) => FeedRelation::Ally,
        _ => FeedRelation::Enemy,
    }
}

/// A feed entry for one received cue, or `None` when the cue is not a participant defeat.
pub(crate) fn feed_entry(
    cue: &CombatCue,
    participants: &[&PublicParticipantState],
    observer: Option<FeedObserver>,
) -> Option<KillFeedEntry> {
    let CombatCue::FighterDefeated { source, target, .. } = cue else {
        return None;
    };
    let named = |fighter| {
        participants
            .iter()
            .copied()
            .find(|participant| participant.fighter_network_id == fighter)
    };
    let victim = named(*target)?;
    let killer = match source {
        Some(DamageSource::HostileNpc { .. }) => {
            Some((HOSTILE_NPC_NAME.to_string(), FeedRelation::Enemy))
        }
        Some(source) => source
            .initiating_fighter()
            .filter(|fighter| fighter != target)
            .and_then(named)
            .map(|killer| (killer.display_name.clone(), relation(killer, observer))),
        None => None,
    };
    Some(KillFeedEntry {
        killer,
        victim: (victim.display_name.clone(), relation(victim, observer)),
        age: 0.0,
    })
}

/// Adds received participant defeats to the feed and ages it out.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn collect_kill_feed(
    mut cues: MessageReader<DeduplicatedCombatCue>,
    matches: Query<&crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>,
    participants: Query<&PublicParticipantState>,
    controlled: Query<
        (&PlayerId, &TeamId),
        (
            With<crate::protocol::Fighter>,
            With<lightyear::prelude::Controlled>,
        ),
    >,
    time: Res<Time>,
    mut feed: ResMut<KillFeed>,
) {
    let match_id = matches.iter().next().map(|state| state.match_id);
    if feed.match_id != match_id {
        feed.match_id = match_id;
        feed.entries.clear();
    }
    if !feed.entries.is_empty() {
        feed.advance(time.delta_secs());
    }
    let Some(match_id) = match_id else {
        cues.clear();
        return;
    };
    let observer = controlled.iter().next().map(|(player, team)| FeedObserver {
        player: *player,
        team: *team,
    });
    let current: Vec<_> = participants
        .iter()
        .filter(|participant| participant.participant.match_id == match_id)
        .collect();
    for DeduplicatedCombatCue(cue) in cues.read() {
        if let Some(entry) = feed_entry(cue, &current, observer) {
            feed.push(entry);
        }
    }
}

/// Layout-tested kill feed root (top right).
#[derive(Component)]
pub(super) struct KillFeedRoot;

#[derive(Component, Clone, Copy)]
pub(super) enum FeedPart {
    Row(usize),
    Killer(usize),
    Icon,
    Victim(usize),
}

pub(super) fn spawn_kill_feed(
    commands: &mut Commands,
    font: &Handle<Font>,
    skull: &Handle<Image>,
    parent: Entity,
) {
    let name = |part, font: &Handle<Font>| {
        (
            part,
            Text::new(""),
            ui::theme::hud_text(font, 15.0),
            TextColor(ui::theme::TEXT),
            TextLayout::new(Justify::Center, LineBreak::NoWrap),
            Node {
                max_width: px(NAME_MAX_WIDTH),
                overflow: Overflow::clip(),
                ..default()
            },
        )
    };
    let root = commands
        .spawn((
            KillFeedRoot,
            Name::new("HUD kill feed"),
            Node {
                position_type: PositionType::Absolute,
                top: px(10.0),
                right: px(16.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: px(4.0),
                ..default()
            },
        ))
        .id();
    commands.entity(parent).add_child(root);
    for index in 0..KILL_FEED_ROWS {
        let row = commands
            .spawn((
                FeedPart::Row(index),
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(6.0),
                    padding: UiRect::axes(px(8.0), px(3.0)),
                    border_radius: BorderRadius::all(px(8.0)),
                    ..default()
                },
                BackgroundColor(palette::PLATE),
                Visibility::Hidden,
            ))
            .with_children(|row| {
                row.spawn(name(FeedPart::Killer(index), font));
                row.spawn((
                    FeedPart::Icon,
                    ImageNode::new(skull.clone()),
                    Node {
                        width: px(20.0),
                        height: px(20.0),
                        ..default()
                    },
                ));
                row.spawn(name(FeedPart::Victim(index), font));
            })
            .id();
        commands.entity(root).add_child(row);
    }
}

/// Writes feed rows; unchanged rows are not touched.
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn present_kill_feed(
    feed: Res<KillFeed>,
    mut parts: Query<(
        &FeedPart,
        Option<&mut Text>,
        Option<&mut TextColor>,
        &mut Visibility,
    )>,
) {
    let shown = |visible: bool| {
        if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        }
    };
    for (part, text, color, mut visibility) in &mut parts {
        match *part {
            FeedPart::Row(index) => {
                visibility.set_if_neq(shown(index < feed.entries.len()));
            }
            FeedPart::Icon => {}
            FeedPart::Killer(index) | FeedPart::Victim(index) => {
                let Some(entry) = feed.entries.get(index) else {
                    continue;
                };
                let name = match *part {
                    FeedPart::Killer(_) => entry.killer.as_ref(),
                    _ => Some(&entry.victim),
                };
                visibility.set_if_neq(shown(name.is_some()));
                if let Some((label, relation)) = name {
                    if let Some(mut text) = text
                        && text.0 != *label
                    {
                        label.clone_into(&mut text.0);
                    }
                    if let Some(mut color) = color {
                        color.set_if_neq(TextColor(relation.color()));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{AttackId, CombatEventId, ShotId, WeaponDefinitionId, WorldPoint};
    use crate::matchplay::{MatchParticipant, PublicParticipantStatus};
    use crate::protocol::NetworkEntityId;

    fn participant(player: u64, fighter: u64, team: u8, name: &str) -> PublicParticipantState {
        PublicParticipantState {
            player_id: PlayerId(player),
            fighter_network_id: NetworkEntityId(fighter),
            team_id: TeamId(team),
            display_name: name.to_string(),
            participant: MatchParticipant {
                match_id: MatchId(1),
                ready: true,
                restart_ready: false,
            },
            selected: true,
            weapon_preset_id: None,
            status: PublicParticipantStatus::Alive,
        }
    }

    fn defeat(source: Option<DamageSource>, target: u64) -> CombatCue {
        CombatCue::FighterDefeated {
            event_id: CombatEventId(1),
            tick: 1,
            attack_id: AttackId(1),
            source,
            target: NetworkEntityId(target),
            position: WorldPoint::from(Vec2::ZERO),
        }
    }

    fn weapon(player: u64, fighter: u64) -> DamageSource {
        DamageSource::PlayerWeapon {
            player_id: PlayerId(player),
            fighter_id: NetworkEntityId(fighter),
            weapon_definition_id: WeaponDefinitionId(1),
            shot_id: ShotId(1),
        }
    }

    #[test]
    fn entries_name_participants_with_observer_relative_colors() {
        let roster = [
            &participant(1, 11, 0, "Ana"),
            &participant(2, 12, 0, "Bo"),
            &participant(3, 13, 1, "Cy"),
        ];
        let observer = Some(FeedObserver {
            player: PlayerId(1),
            team: TeamId(0),
        });
        let entry = feed_entry(&defeat(Some(weapon(1, 11)), 13), &roster, observer).unwrap();
        assert_eq!(entry.killer, Some(("Ana".to_string(), FeedRelation::Local)));
        assert_eq!(entry.victim, ("Cy".to_string(), FeedRelation::Enemy));
        let entry = feed_entry(&defeat(Some(weapon(3, 13)), 12), &roster, observer).unwrap();
        assert_eq!(entry.killer, Some(("Cy".to_string(), FeedRelation::Enemy)));
        assert_eq!(entry.victim, ("Bo".to_string(), FeedRelation::Ally));

        // Hostile NPCs, the environment, and self-defeats.
        let zombie = Some(DamageSource::HostileNpc {
            npc_id: NetworkEntityId(99),
            definition_id: 1,
        });
        let entry = feed_entry(&defeat(zombie, 11), &roster, observer).unwrap();
        assert_eq!(
            entry.killer,
            Some(("ZOMBIE".to_string(), FeedRelation::Enemy))
        );
        assert_eq!(
            feed_entry(&defeat(None, 11), &roster, observer)
                .unwrap()
                .killer,
            None
        );
        assert_eq!(
            feed_entry(&defeat(Some(weapon(1, 11)), 11), &roster, observer)
                .unwrap()
                .killer,
            None
        );
    }

    #[test]
    fn deployables_and_other_cues_are_not_feed_entries() {
        let roster = [&participant(1, 11, 0, "Ana")];
        // A sentry (not a participant fighter) destroyed by Ana.
        assert!(feed_entry(&defeat(Some(weapon(1, 11)), 500), &roster, None).is_none());
        let legacy = CombatCue::Defeat {
            event_id: CombatEventId(2),
            tick: 1,
            source: None,
            target: NetworkEntityId(11),
        };
        assert!(feed_entry(&legacy, &roster, None).is_none());
    }

    #[test]
    fn feed_is_bounded_newest_first_and_expires() {
        let mut feed = KillFeed::default();
        for index in 0..6 {
            feed.push(KillFeedEntry {
                killer: None,
                victim: (format!("P{index}"), FeedRelation::Enemy),
                age: 0.0,
            });
            feed.advance(0.5);
        }
        assert_eq!(feed.entries.len(), KILL_FEED_ROWS);
        assert_eq!(feed.entries[0].victim.0, "P5");
        feed.advance(KILL_FEED_SECONDS);
        assert!(feed.entries.is_empty());
    }

    #[test]
    fn feed_shows_only_cues_the_client_received_and_clears_on_a_new_match() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<DeduplicatedCombatCue>()
            .init_resource::<KillFeed>()
            .add_systems(Update, collect_kill_feed);
        let root = app
            .world_mut()
            .spawn((
                crate::matchplay::MatchRoot,
                crate::matchplay::MatchState {
                    match_id: MatchId(1),
                    mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
                    phase: crate::matchplay::MatchPhase::Active { ends_at_tick: 600 },
                    rules_revision: 1,
                },
            ))
            .id();
        app.world_mut().spawn(participant(1, 11, 0, "Ana"));
        app.world_mut().spawn(participant(3, 13, 1, "Cy"));
        app.update();
        assert!(app.world().resource::<KillFeed>().entries.is_empty());
        // Only a delivered cue produces an entry; a withheld (concealed) defeat never arrives.
        app.world_mut()
            .write_message(DeduplicatedCombatCue(defeat(Some(weapon(1, 11)), 13)));
        app.update();
        let feed = app.world().resource::<KillFeed>();
        assert_eq!(feed.entries.len(), 1);
        assert_eq!(feed.entries[0].victim.0, "Cy");

        app.world_mut()
            .get_mut::<crate::matchplay::MatchState>(root)
            .unwrap()
            .match_id = MatchId(2);
        app.update();
        assert!(app.world().resource::<KillFeed>().entries.is_empty());
    }
}
