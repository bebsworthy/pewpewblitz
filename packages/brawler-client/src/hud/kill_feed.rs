//! Bounded kill feed fed only by the defeat cues this client received.
//!
//! The server sends a defeat cue to an observer only when the victim and any initiating fighter are
//! relevant to that observer (`combat::authority::combat_cue_subjects`), so the feed never names a
//! concealed fighter. Names come from the public participant roster; defeats of deployables and
//! other non-participants are skipped.
//!
//! The same rows carry held-seat notices (BRL-dwqx): when the server publishes that a bot now
//! drives a disconnected player's seat, that the player is back, or that a held seat disappeared
//! (the hold expired), one row names the player. Notices come only from the public
//! `ParticipantControl` fact; a first observation is a baseline and never a notice.

use super::palette;
use crate::ui;
use bevy::prelude::*;
use brawler::combat::client::DeduplicatedCombatCue;
use brawler::combat::{CombatCue, DamageSource, TeamId};
use brawler::matchplay::{MatchId, MatchPhase, ParticipantControl, PublicParticipantState};
use brawler::protocol::PlayerId;
use std::collections::{BTreeMap, VecDeque};

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

/// A held-seat transition the server published for another player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SeatNotice {
    BotPlaying,
    Back,
    Left,
}

impl SeatNotice {
    const fn text(self) -> &'static str {
        match self {
            Self::BotPlaying => "disconnected - bot playing",
            Self::Back => "is back",
            Self::Left => "left",
        }
    }
}

/// What one feed row says.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FeedLine {
    Defeat {
        killer: Option<(String, FeedRelation)>,
        victim: (String, FeedRelation),
    },
    Seat {
        name: (String, FeedRelation),
        notice: SeatNotice,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KillFeedEntry {
    pub(crate) line: FeedLine,
    age: f32,
}

impl KillFeedEntry {
    fn new(line: FeedLine) -> Self {
        Self { line, age: 0.0 }
    }
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
    Some(KillFeedEntry::new(FeedLine::Defeat {
        killer,
        victim: (victim.display_name.clone(), relation(victim, observer)),
    }))
}

/// The last published control of each participant in the current match, so a transition can be
/// told from a first observation.
#[derive(Resource, Default, Debug)]
pub(crate) struct SeatControlTransitions {
    match_id: Option<MatchId>,
    seen: BTreeMap<u64, SeenSeat>,
}

#[derive(Debug)]
struct SeenSeat {
    control: ParticipantControl,
    name: (String, FeedRelation),
}

impl SeatControlTransitions {
    /// Records this frame's public roster and returns the notices its changes warrant. The first
    /// observation of a participant only records a baseline; the observer never gets a notice
    /// about itself; a held seat that disappears is "left" only while the match is Active.
    pub(crate) fn observe(
        &mut self,
        match_id: Option<MatchId>,
        active: bool,
        participants: &[&PublicParticipantState],
        observer: Option<FeedObserver>,
    ) -> Vec<KillFeedEntry> {
        if self.match_id != match_id {
            self.match_id = match_id;
            self.seen.clear();
        }
        let mut notices = Vec::new();
        let is_observer =
            |player: u64| observer.is_some_and(|observer| observer.player.0 == player);
        for participant in participants {
            let player = participant.player_id.0;
            let name = (
                participant.display_name.clone(),
                relation(participant, observer),
            );
            let previous = self.seen.insert(
                player,
                SeenSeat {
                    control: participant.control,
                    name: name.clone(),
                },
            );
            let notice = match (previous.map(|seen| seen.control), participant.control) {
                (Some(ParticipantControl::Player), ParticipantControl::HeldByBot { .. }) => {
                    Some(SeatNotice::BotPlaying)
                }
                (Some(ParticipantControl::HeldByBot { .. }), ParticipantControl::Player) => {
                    Some(SeatNotice::Back)
                }
                _ => None,
            };
            if let Some(notice) = notice.filter(|_| !is_observer(player)) {
                notices.push(KillFeedEntry::new(FeedLine::Seat { name, notice }));
            }
        }
        let present: Vec<u64> = participants
            .iter()
            .map(|participant| participant.player_id.0)
            .collect();
        let gone: Vec<u64> = self
            .seen
            .keys()
            .copied()
            .filter(|player| !present.contains(player))
            .collect();
        for player in gone {
            let Some(seen) = self.seen.remove(&player) else {
                continue;
            };
            if active
                && !is_observer(player)
                && matches!(seen.control, ParticipantControl::HeldByBot { .. })
            {
                notices.push(KillFeedEntry::new(FeedLine::Seat {
                    name: seen.name,
                    notice: SeatNotice::Left,
                }));
            }
        }
        notices
    }
}

/// Adds received participant defeats to the feed and ages it out.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn collect_kill_feed(
    mut cues: MessageReader<DeduplicatedCombatCue>,
    matches: Query<&brawler::matchplay::MatchState, With<brawler::matchplay::MatchRoot>>,
    participants: Query<&PublicParticipantState>,
    controlled: Query<
        (&PlayerId, &TeamId),
        (
            With<brawler::protocol::Fighter>,
            With<lightyear::prelude::Controlled>,
        ),
    >,
    time: Res<Time>,
    mut feed: ResMut<KillFeed>,
    mut seats: ResMut<SeatControlTransitions>,
) {
    let match_state = matches.iter().next();
    let match_id = match_state.map(|state| state.match_id);
    if feed.match_id != match_id {
        feed.match_id = match_id;
        feed.entries.clear();
    }
    if !feed.entries.is_empty() {
        feed.advance(time.delta_secs());
    }
    let Some(match_id) = match_id else {
        seats.observe(None, false, &[], None);
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
    let active = match_state.is_some_and(|state| matches!(state.phase, MatchPhase::Active { .. }));
    for notice in seats.observe(Some(match_id), active, &current, observer) {
        feed.push(notice);
    }
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
    Icon(usize),
    Victim(usize),
    Notice(usize),
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
                    FeedPart::Icon(index),
                    ImageNode::new(skull.clone()),
                    Node {
                        width: px(20.0),
                        height: px(20.0),
                        ..default()
                    },
                ));
                row.spawn(name(FeedPart::Victim(index), font));
                // Seat notices are wider than a name; only the name slot is width-bounded.
                row.spawn((
                    FeedPart::Notice(index),
                    Text::new(""),
                    ui::theme::hud_text(font, 15.0),
                    TextColor(ui::theme::TEXT),
                    TextLayout::new(Justify::Left, LineBreak::NoWrap),
                    Visibility::Hidden,
                ));
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
        let (FeedPart::Killer(index)
        | FeedPart::Icon(index)
        | FeedPart::Victim(index)
        | FeedPart::Notice(index)
        | FeedPart::Row(index)) = *part;
        let Some(entry) = feed.entries.get(index) else {
            if matches!(part, FeedPart::Row(_)) {
                visibility.set_if_neq(Visibility::Hidden);
            }
            continue;
        };
        let (label, label_color) = match (*part, &entry.line) {
            (FeedPart::Row(_), _) => {
                visibility.set_if_neq(Visibility::Inherited);
                continue;
            }
            (FeedPart::Icon(_), line) => {
                visibility.set_if_neq(shown(matches!(line, FeedLine::Defeat { .. })));
                continue;
            }
            (FeedPart::Killer(_), FeedLine::Defeat { killer, .. }) => (
                killer.as_ref().map(|(name, _)| name.as_str()),
                killer.as_ref().map(|(_, relation)| relation.color()),
            ),
            (
                FeedPart::Victim(_),
                FeedLine::Defeat { victim, .. } | FeedLine::Seat { name: victim, .. },
            ) => (Some(victim.0.as_str()), Some(victim.1.color())),
            (FeedPart::Notice(_), FeedLine::Seat { notice, .. }) => {
                (Some(notice.text()), Some(ui::theme::TEXT))
            }
            (FeedPart::Killer(_) | FeedPart::Notice(_), _) => (None, None),
        };
        visibility.set_if_neq(shown(label.is_some()));
        if let (Some(label), Some(mut text)) = (label, text)
            && text.0 != label
        {
            label.clone_into(&mut text.0);
        }
        if let (Some(label_color), Some(mut color)) = (label_color, color) {
            color.set_if_neq(TextColor(label_color));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brawler::combat::{AttackId, CombatEventId, ShotId, WeaponDefinitionId, WorldPoint};
    use brawler::matchplay::{MatchParticipant, PublicParticipantStatus};
    use brawler::protocol::NetworkEntityId;

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
            control: ParticipantControl::Player,
        }
    }

    fn defeat_names(
        entry: &KillFeedEntry,
    ) -> (&Option<(String, FeedRelation)>, &(String, FeedRelation)) {
        match &entry.line {
            FeedLine::Defeat { killer, victim } => (killer, victim),
            FeedLine::Seat { .. } => panic!("expected a defeat row"),
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
        let (killer, victim) = defeat_names(&entry);
        assert_eq!(killer, &Some(("Ana".to_string(), FeedRelation::Local)));
        assert_eq!(victim, &("Cy".to_string(), FeedRelation::Enemy));
        let entry = feed_entry(&defeat(Some(weapon(3, 13)), 12), &roster, observer).unwrap();
        let (killer, victim) = defeat_names(&entry);
        assert_eq!(killer, &Some(("Cy".to_string(), FeedRelation::Enemy)));
        assert_eq!(victim, &("Bo".to_string(), FeedRelation::Ally));

        // Hostile NPCs, the environment, and self-defeats.
        let zombie = Some(DamageSource::HostileNpc {
            npc_id: NetworkEntityId(99),
            definition_id: 1,
        });
        let entry = feed_entry(&defeat(zombie, 11), &roster, observer).unwrap();
        assert_eq!(
            defeat_names(&entry).0,
            &Some(("ZOMBIE".to_string(), FeedRelation::Enemy))
        );
        let entry = feed_entry(&defeat(None, 11), &roster, observer).unwrap();
        assert_eq!(defeat_names(&entry).0, &None);
        let entry = feed_entry(&defeat(Some(weapon(1, 11)), 11), &roster, observer).unwrap();
        assert_eq!(defeat_names(&entry).0, &None);
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
            feed.push(KillFeedEntry::new(FeedLine::Defeat {
                killer: None,
                victim: (format!("P{index}"), FeedRelation::Enemy),
            }));
            feed.advance(0.5);
        }
        assert_eq!(feed.entries.len(), KILL_FEED_ROWS);
        assert_eq!(defeat_names(&feed.entries[0]).1.0, "P5");
        feed.advance(KILL_FEED_SECONDS);
        assert!(feed.entries.is_empty());
    }

    #[test]
    fn feed_shows_only_cues_the_client_received_and_clears_on_a_new_match() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<DeduplicatedCombatCue>()
            .init_resource::<KillFeed>()
            .init_resource::<SeatControlTransitions>()
            .add_systems(Update, collect_kill_feed);
        let root = app
            .world_mut()
            .spawn((
                brawler::matchplay::MatchRoot,
                brawler::matchplay::MatchState {
                    match_id: MatchId(1),
                    mode_definition_id: brawler::map::WIPEOUT_MODE_DEFINITION,
                    phase: MatchPhase::Active { ends_at_tick: 600 },
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
        assert_eq!(defeat_names(&feed.entries[0]).1.0, "Cy");

        app.world_mut()
            .get_mut::<brawler::matchplay::MatchState>(root)
            .unwrap()
            .match_id = MatchId(2);
        app.update();
        assert!(app.world().resource::<KillFeed>().entries.is_empty());
    }

    fn held(player: u64, fighter: u64, team: u8, name: &str) -> PublicParticipantState {
        PublicParticipantState {
            control: ParticipantControl::HeldByBot {
                expires_at_tick: 3_600,
            },
            ..participant(player, fighter, team, name)
        }
    }

    fn seat_notices(entries: &[KillFeedEntry]) -> Vec<(&str, FeedRelation, SeatNotice)> {
        entries
            .iter()
            .map(|entry| match &entry.line {
                FeedLine::Seat { name, notice } => (name.0.as_str(), name.1, *notice),
                FeedLine::Defeat { .. } => panic!("expected a seat notice"),
            })
            .collect()
    }

    /// BRL-dwqx: exactly the three transitions produce notices; a first observation, the
    /// observer's own seat, a departure outside Active, and a new match only set a baseline.
    #[test]
    fn seat_control_transitions_emit_only_the_three_notices() {
        let observer = Some(FeedObserver {
            player: PlayerId(1),
            team: TeamId(0),
        });
        let mut seats = SeatControlTransitions::default();
        let observe = |seats: &mut SeatControlTransitions,
                       match_id: u128,
                       active: bool,
                       roster: &[&PublicParticipantState]| {
            seats.observe(Some(MatchId(match_id)), active, roster, observer)
        };
        let (ana, bo, cy) = (
            participant(1, 11, 0, "Ana"),
            participant(2, 12, 0, "Bo"),
            participant(3, 13, 1, "Cy"),
        );
        let (ana_held, bo_held, cy_held) = (
            held(1, 11, 0, "Ana"),
            held(2, 12, 0, "Bo"),
            held(3, 13, 1, "Cy"),
        );

        // A first observation is a baseline, even of a seat that is already held (a rejoining
        // client's first snapshot, or a late join).
        assert!(observe(&mut seats, 1, true, &[&ana, &bo, &cy_held]).is_empty());
        assert!(observe(&mut seats, 1, true, &[&ana, &bo, &cy_held]).is_empty());

        // Player -> HeldByBot and HeldByBot -> Player, in the observer-relative color.
        let notices = observe(&mut seats, 1, true, &[&ana, &bo_held, &cy]);
        assert_eq!(
            seat_notices(&notices),
            [
                ("Bo", FeedRelation::Ally, SeatNotice::BotPlaying),
                ("Cy", FeedRelation::Enemy, SeatNotice::Back),
            ]
        );

        // The observer never gets a notice about its own seat.
        assert!(observe(&mut seats, 1, true, &[&ana_held, &bo_held, &cy]).is_empty());
        assert!(observe(&mut seats, 1, true, &[&ana, &bo_held, &cy]).is_empty());

        // A held seat that disappears while Active is "left"; a Player seat leaving is silent.
        let notices = observe(&mut seats, 1, true, &[&ana]);
        assert_eq!(
            seat_notices(&notices),
            [("Bo", FeedRelation::Ally, SeatNotice::Left)]
        );

        // Outside Active, a held seat's removal is not "left".
        assert!(observe(&mut seats, 1, true, &[&ana, &bo_held]).is_empty());
        assert!(observe(&mut seats, 1, false, &[&ana]).is_empty());

        // A new match resets the baseline: a held seat seen first in match 2 is silent, and
        // nothing from match 1 is reported as left.
        assert!(observe(&mut seats, 1, true, &[&ana, &bo_held]).is_empty());
        assert!(observe(&mut seats, 2, true, &[&ana, &cy_held]).is_empty());
        assert!(observe(&mut seats, 2, true, &[&ana, &cy_held]).is_empty());
        assert_eq!(SeatNotice::BotPlaying.text(), "disconnected - bot playing");
    }

    #[test]
    fn seat_notices_reach_the_feed_from_the_public_roster() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<DeduplicatedCombatCue>()
            .init_resource::<KillFeed>()
            .init_resource::<SeatControlTransitions>()
            .add_systems(Update, collect_kill_feed);
        app.world_mut().spawn((
            brawler::matchplay::MatchRoot,
            brawler::matchplay::MatchState {
                match_id: MatchId(1),
                mode_definition_id: brawler::map::WIPEOUT_MODE_DEFINITION,
                phase: MatchPhase::Active { ends_at_tick: 600 },
                rules_revision: 1,
            },
        ));
        let cy = app.world_mut().spawn(participant(3, 13, 1, "Cy")).id();
        app.update();
        assert!(app.world().resource::<KillFeed>().entries.is_empty());
        app.world_mut()
            .get_mut::<PublicParticipantState>(cy)
            .unwrap()
            .control = ParticipantControl::HeldByBot {
            expires_at_tick: 3_600,
        };
        app.update();
        let feed = app.world().resource::<KillFeed>();
        let entries: Vec<_> = feed.entries.iter().cloned().collect();
        assert_eq!(
            seat_notices(&entries),
            [("Cy", FeedRelation::Enemy, SeatNotice::BotPlaying)]
        );
    }
}
