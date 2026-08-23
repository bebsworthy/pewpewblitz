//! Readiness and match-shell HUD that remains independent from gameplay mutation.
#![allow(clippy::wildcard_imports)]

use super::*;
use crate::combat::TeamId;
use std::collections::BTreeMap;

#[derive(Component)]
struct ReadinessHudText;

#[derive(Component)]
struct MatchHudText;

#[derive(Component)]
struct CountdownHudText;

#[derive(Component)]
struct MatchPhaseOverlayText;

#[derive(Clone, Debug, PartialEq, Eq)]
struct CachedRosterEntry {
    team: TeamId,
    display_name: String,
    weapon_preset: Option<u16>,
    status: CachedRosterStatus,
    connected: bool,
}

/// Small presentation-only dispatch for the one shared top-right objective slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModeScoreView {
    Wipeout {
        scores: [u16; 2],
        target: u16,
    },
    HotZone {
        progress_percent: [u8; 2],
        status: crate::matchplay::HotZoneStatus,
    },
    Syncing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CachedRosterStatus {
    Alive,
    Ready,
    RestartReady,
    Defeated,
    Respawning(u64),
    Protected(u64),
}

#[derive(Resource, Default)]
struct MatchRosterPresentation {
    match_id: Option<crate::matchplay::MatchId>,
    entries: BTreeMap<u64, CachedRosterEntry>,
}

#[derive(Default)]
struct PhasePresentationFacts {
    participant_count: usize,
    ready_count: usize,
    restart_ready_count: usize,
    local_selected: bool,
    local_ready: bool,
    local_restart_ready: bool,
}

pub struct ClientReadinessHudPlugin;

impl Plugin for ClientReadinessHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MatchRosterPresentation>()
            .add_systems(Startup, spawn_readiness_hud)
            .add_systems(Update, update_readiness_hud);
    }
}

fn spawn_readiness_hud(mut commands: Commands) {
    commands.spawn((
        ReadinessHudText,
        Text::new(""),
        TextFont::from_font_size(28.0),
        TextColor(Color::WHITE),
        TextLayout::new(Justify::Center, LineBreak::WordBoundary),
        GlobalZIndex(100),
        Node {
            position_type: PositionType::Absolute,
            left: percent(40.0),
            right: percent(40.0),
            top: px(16.0),
            padding: UiRect::axes(px(10.0), px(5.0)),
            border_radius: BorderRadius::all(px(7.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.04, 0.9)),
    ));
    commands.spawn((
        MatchHudText,
        Text::new("SYNCING OBJECTIVE"),
        TextFont::from_font_size(18.0),
        TextColor(Color::WHITE),
        TextLayout::new(Justify::Center, LineBreak::WordBoundary),
        GlobalZIndex(100),
        Node {
            position_type: PositionType::Absolute,
            right: px(16.0),
            top: px(16.0),
            width: px(270.0),
            padding: UiRect::all(px(10.0)),
            border_radius: BorderRadius::all(px(7.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.04, 0.92)),
    ));
    commands.spawn((
        CountdownHudText,
        Text::new(""),
        TextFont::from_font_size(72.0),
        TextColor(Color::srgb(1.0, 0.85, 0.35)),
        TextLayout::new(Justify::Center, LineBreak::NoWrap),
        GlobalZIndex(225),
        Node {
            position_type: PositionType::Absolute,
            left: percent(30.0),
            right: percent(30.0),
            top: percent(38.0),
            ..default()
        },
        Visibility::Hidden,
    ));
    commands
        .spawn((
            MatchPhaseOverlayText,
            Text::new(""),
            TextFont::from_font_size(34.0),
            TextColor(Color::WHITE),
            TextLayout::new(Justify::Center, LineBreak::WordBoundary),
            GlobalZIndex(220),
            Node {
                position_type: PositionType::Absolute,
                left: percent(25.0),
                right: percent(25.0),
                top: percent(27.0),
                padding: UiRect::all(px(28.0)),
                border_radius: BorderRadius::all(px(12.0)),
                ..default()
            },
            Visibility::Hidden,
        ))
        .insert(BackgroundColor(Color::srgba(0.025, 0.035, 0.055, 0.92)));
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn update_readiness_hud(
    controlled: Query<(&PlayerId, &TeamId), (With<Fighter>, With<Controlled>)>,
    matches: Query<(&MatchState, Option<&crate::matchplay::WipeoutState>), With<MatchRoot>>,
    hot_zones: Query<&crate::matchplay::HotZoneState, With<MatchRoot>>,
    clocks: Query<&crate::matchplay::MatchClock, With<MatchRoot>>,
    participants: Query<
        (
            &PlayerId,
            &TeamId,
            &crate::matchplay::FighterDisplayName,
            &MatchParticipant,
            Option<&crate::builds::SelectedBuild>,
            Option<&crate::builds::ResolvedMatchLoadout>,
            Option<&RespawnState>,
            Option<&SpawnProtection>,
            Option<&crate::combat::Defeated>,
        ),
        With<Fighter>,
    >,
    mut roster_presentation: ResMut<MatchRosterPresentation>,
    mut readiness_text: Query<
        (&mut Text, &mut Visibility),
        (
            With<ReadinessHudText>,
            Without<MatchHudText>,
            Without<CountdownHudText>,
            Without<MatchPhaseOverlayText>,
            Without<ScoreboardOverlay>,
        ),
    >,
    mut match_text: Query<
        (&mut Text, &mut Visibility),
        (
            With<MatchHudText>,
            Without<ReadinessHudText>,
            Without<CountdownHudText>,
            Without<MatchPhaseOverlayText>,
            Without<ScoreboardOverlay>,
        ),
    >,
    mut countdown_text: Query<
        (&mut Text, &mut Visibility),
        (
            With<CountdownHudText>,
            Without<ReadinessHudText>,
            Without<MatchHudText>,
            Without<MatchPhaseOverlayText>,
            Without<ScoreboardOverlay>,
        ),
    >,
    mut phase_overlay: Query<
        (&mut Text, &mut Visibility),
        (
            With<MatchPhaseOverlayText>,
            Without<ReadinessHudText>,
            Without<MatchHudText>,
            Without<CountdownHudText>,
            Without<ScoreboardOverlay>,
        ),
    >,
    mut scoreboard_text: Query<
        &mut Text,
        (
            With<ScoreboardOverlay>,
            Without<ReadinessHudText>,
            Without<MatchHudText>,
            Without<CountdownHudText>,
            Without<MatchPhaseOverlayText>,
        ),
    >,
) {
    let fighter = controlled.iter().next();
    let match_state = matches.iter().next();
    let clock = clocks.iter().next();
    // Deadlines derive only from the phase minus the generation-tagged match clock; any other
    // component arrival order displays a syncing label instead of a stale or local countdown.
    let now = match_deadline_tick(match_state.map(|(state, _)| *state), clock);
    for (mut text, mut visibility) in &mut readiness_text {
        text.0 = match_state.map_or_else(String::new, |(state, _)| match state.phase {
            MatchPhase::Countdown { starts_at_tick } => now.map_or_else(
                || "SYNCING".to_string(),
                |now| {
                    format!(
                        "START {}",
                        starts_at_tick.saturating_sub(now).div_ceil(60).max(1)
                    )
                },
            ),
            MatchPhase::Active { ends_at_tick } => now.map_or_else(
                || "SYNCING".to_string(),
                |now| format_match_time(ends_at_tick.saturating_sub(now)),
            ),
            MatchPhase::Waiting | MatchPhase::Completed { .. } => String::new(),
        });
        *visibility = if text.0.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    for (mut text, mut visibility) in &mut countdown_text {
        if let Some(label) = countdown_label(match_state.map(|(state, _)| *state), now) {
            text.0 = label;
            *visibility = Visibility::Inherited;
        } else {
            text.0.clear();
            *visibility = Visibility::Hidden;
        }
    }
    let local_player = fighter.map(|(player, _)| player.0);
    let phase_facts = sync_roster_and_collect_phase_facts(
        match_state.map(|(state, _)| *state),
        local_player,
        participants.iter(),
        &mut roster_presentation,
    );
    let final_line = final_objective_line(match_state, hot_zones.iter().next());
    for (mut text, mut visibility) in &mut phase_overlay {
        if let Some(label) = phase_overlay_label(
            match_state.map(|(state, _)| *state),
            final_line.as_deref(),
            now,
            phase_facts.participant_count,
            phase_facts.ready_count,
            phase_facts.restart_ready_count,
            phase_facts.local_selected,
            phase_facts.local_ready,
            phase_facts.local_restart_ready,
        ) {
            text.0 = label;
            *visibility = Visibility::Inherited;
        } else {
            text.0.clear();
            *visibility = Visibility::Hidden;
        }
    }
    let score_view = build_mode_score_view(match_state, hot_zones.iter().next(), clock);
    for (mut text, mut visibility) in &mut match_text {
        text.0 = score_view.map_or_else(String::new, mode_score_text);
        *visibility = if match_state.is_some_and(|(state, _)| {
            matches!(
                state.phase,
                MatchPhase::Countdown { .. } | MatchPhase::Active { .. }
            )
        }) && !text.0.is_empty()
        {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    let mut roster_rows = roster_presentation.entries.iter().collect::<Vec<_>>();
    roster_rows.sort_by_key(|(player, entry)| (entry.team.0, **player));
    let roster_text = roster_rows
        .into_iter()
        .map(|(player, entry)| {
            roster_entry_text(entry, now.unwrap_or(0), local_player == Some(*player))
        })
        .collect::<Vec<_>>()
        .join("\n");
    for mut text in &mut scoreboard_text {
        text.0 = format!("SCOREBOARD\n\n{roster_text}");
    }
}

/// The shared deadline clock, presentable only when the clock, match envelope, and concrete
/// mode state carry the same match ID.
fn match_deadline_tick(
    state: Option<MatchState>,
    clock: Option<&crate::matchplay::MatchClock>,
) -> Option<u64> {
    let state = state?;
    let clock = clock?;
    (clock.match_id == state.match_id).then_some(clock.completed_tick)
}

pub(crate) fn build_mode_score_view(
    state: Option<(&MatchState, Option<&crate::matchplay::WipeoutState>)>,
    hot_zone: Option<&crate::matchplay::HotZoneState>,
    clock: Option<&crate::matchplay::MatchClock>,
) -> Option<ModeScoreView> {
    let (state, wipeout) = state?;
    if clock.is_none_or(|clock| clock.match_id != state.match_id) {
        return Some(ModeScoreView::Syncing);
    }
    match state.mode_definition_id {
        crate::map::WIPEOUT_MODE_DEFINITION => Some(wipeout.map_or(
            ModeScoreView::Syncing,
            |wipeout| ModeScoreView::Wipeout {
                scores: wipeout.team_scores,
                target: wipeout.target_score,
            },
        )),
        crate::map::HOT_ZONE_MODE_DEFINITION => Some(
            hot_zone
                .filter(|hot_zone| hot_zone.match_id == state.match_id)
                .map_or(ModeScoreView::Syncing, |hot_zone| {
                    let percent = |progress: u16| {
                        let value = u32::from(progress) * 100
                            / u32::from(hot_zone.target_progress_ticks.max(1));
                        u8::try_from(value.min(100)).unwrap_or(100)
                    };
                    ModeScoreView::HotZone {
                        progress_percent: [
                            percent(hot_zone.progress_ticks[0]),
                            percent(hot_zone.progress_ticks[1]),
                        ],
                        status: hot_zone.status,
                    }
                }),
        ),
        _ => None,
    }
}

pub(crate) fn mode_score_text(view: ModeScoreView) -> String {
    match view {
        ModeScoreView::Wipeout { scores, target } => {
            format!("T1  {}  —  {}  T2\nFIRST TO {target}", scores[0], scores[1])
        }
        ModeScoreView::HotZone {
            progress_percent,
            status,
        } => {
            let status = match status {
                crate::matchplay::HotZoneStatus::Empty => "EMPTY".to_string(),
                crate::matchplay::HotZoneStatus::Contested => "CONTESTED".to_string(),
                crate::matchplay::HotZoneStatus::Controlled { team } => {
                    format!("T{} CONTROL", team.0 + 1)
                }
            };
            format!(
                "T1  {}%  —  {}%  T2\n{status}",
                progress_percent[0], progress_percent[1]
            )
        }
        ModeScoreView::Syncing => "SYNCING OBJECTIVE".to_string(),
    }
}

fn format_match_time(remaining_ticks: u64) -> String {
    let total_seconds = remaining_ticks.div_ceil(60);
    format!("{}:{:02}", total_seconds / 60, total_seconds % 60)
}

fn sync_roster_and_collect_phase_facts<'a>(
    state: Option<MatchState>,
    local_player: Option<u64>,
    participants: impl Iterator<
        Item = (
            &'a PlayerId,
            &'a TeamId,
            &'a crate::matchplay::FighterDisplayName,
            &'a MatchParticipant,
            Option<&'a crate::builds::SelectedBuild>,
            Option<&'a crate::builds::ResolvedMatchLoadout>,
            Option<&'a RespawnState>,
            Option<&'a SpawnProtection>,
            Option<&'a crate::combat::Defeated>,
        ),
    >,
    roster: &mut MatchRosterPresentation,
) -> PhasePresentationFacts {
    let Some(state) = state else {
        roster.match_id = None;
        roster.entries.clear();
        return PhasePresentationFacts::default();
    };
    if roster.match_id != Some(state.match_id) {
        roster.match_id = Some(state.match_id);
        roster.entries.clear();
    }
    for entry in roster.entries.values_mut() {
        entry.connected = false;
    }
    let mut facts = PhasePresentationFacts::default();
    for (player, team, display_name, participant, build, loadout, respawn, protection, defeated) in
        participants
    {
        if participant.match_id != state.match_id {
            continue;
        }
        facts.participant_count += 1;
        facts.ready_count += usize::from(participant.ready && build.is_some());
        facts.restart_ready_count += usize::from(participant.restart_ready);
        if local_player == Some(player.0) {
            facts.local_ready = participant.ready;
            facts.local_restart_ready = participant.restart_ready;
            facts.local_selected = build.is_some();
        }
        roster.entries.insert(
            player.0,
            CachedRosterEntry {
                team: *team,
                display_name: display_name.0.clone(),
                weapon_preset: loadout.and_then(|loadout| {
                    loadout
                        .primary_weapon
                        .source_preset_id
                        .map(|preset| preset.0)
                }),
                status: if let Some(respawn) = respawn {
                    CachedRosterStatus::Respawning(respawn.respawn_at_tick)
                } else if let Some(protection) = protection {
                    CachedRosterStatus::Protected(protection.expires_at_tick)
                } else if defeated.is_some() {
                    CachedRosterStatus::Defeated
                } else if participant.restart_ready {
                    CachedRosterStatus::RestartReady
                } else if participant.ready {
                    CachedRosterStatus::Ready
                } else {
                    CachedRosterStatus::Alive
                },
                connected: true,
            },
        );
    }
    facts
}

/// The mode-specific final objective line for the completed overlay, or a syncing label.
fn final_objective_line(
    state: Option<(&MatchState, Option<&crate::matchplay::WipeoutState>)>,
    hot_zone: Option<&crate::matchplay::HotZoneState>,
) -> Option<String> {
    let (state, wipeout) = state?;
    Some(match state.mode_definition_id {
        crate::map::WIPEOUT_MODE_DEFINITION => wipeout.map_or_else(
            || "FINAL  SYNCING".to_string(),
            |wipeout| {
                format!(
                    "FINAL  {} - {}  |  MARGIN {}",
                    wipeout.team_scores[0],
                    wipeout.team_scores[1],
                    wipeout.team_scores[0].abs_diff(wipeout.team_scores[1])
                )
            },
        ),
        crate::map::HOT_ZONE_MODE_DEFINITION => hot_zone
            .filter(|hot_zone| hot_zone.match_id == state.match_id)
            .map_or_else(
                || "SYNCING OBJECTIVE".to_string(),
                |hot_zone| {
                    let percent = |progress: u16| {
                        u32::from(progress) * 100 / u32::from(hot_zone.target_progress_ticks)
                    };
                    format!(
                        "FINAL  T1 {}%  T2 {}%",
                        percent(hot_zone.progress_ticks[0]),
                        percent(hot_zone.progress_ticks[1]),
                    )
                },
            ),
        _ => "UNKNOWN MODE".to_string(),
    })
}

#[allow(clippy::too_many_arguments)]
fn phase_overlay_label(
    state: Option<MatchState>,
    final_line: Option<&str>,
    now: Option<u64>,
    participant_count: usize,
    ready_count: usize,
    restart_ready_count: usize,
    local_selected: bool,
    local_ready: bool,
    local_restart_ready: bool,
) -> Option<String> {
    let state = state?;
    match state.phase {
        MatchPhase::Waiting => {
            if !local_selected {
                return None;
            }
            let prompt = if local_ready {
                "READY - WAITING FOR OPPONENTS"
            } else {
                "PRESS SPACE / ENTER / A TO READY"
            };
            Some(format!(
                "GET READY\n{prompt}\n\n{ready_count}/{participant_count} fighters ready"
            ))
        }
        MatchPhase::Completed {
            result,
            restart_unlocked_at_tick,
            ..
        } => {
            let title = match result {
                crate::matchplay::MatchResult::TeamVictory { team } => {
                    format!("TEAM {} WINS", team.0 + 1)
                }
                crate::matchplay::MatchResult::Draw => "DRAW".to_string(),
                crate::matchplay::MatchResult::Forfeit {
                    winner,
                    departed_team,
                } => format!(
                    "TEAM {} WINS\nTEAM {} FORFEITED",
                    winner.0 + 1,
                    departed_team.0 + 1
                ),
            };
            let final_line = final_line.unwrap_or("SYNCING");
            let restart = match now {
                None => "SYNCING".to_string(),
                Some(now) if now < restart_unlocked_at_tick => format!(
                    "Restart unlocks in {}",
                    restart_unlocked_at_tick.saturating_sub(now).div_ceil(60)
                ),
                Some(_) if local_restart_ready => {
                    "Ready for restart - waiting for quorum".to_string()
                }
                Some(_) => "Press SPACE / ENTER / A to ready for restart".to_string(),
            };
            Some(format!(
                "{title}\n\n{final_line}\n\n{restart}\n{restart_ready_count}/{participant_count} restart ready"
            ))
        }
        MatchPhase::Countdown { .. } | MatchPhase::Active { .. } => None,
    }
}

fn countdown_label(state: Option<MatchState>, now: Option<u64>) -> Option<String> {
    let MatchPhase::Countdown { starts_at_tick } = state?.phase else {
        return None;
    };
    Some(
        starts_at_tick
            .saturating_sub(now?)
            .div_ceil(60)
            .max(1)
            .to_string(),
    )
}

fn roster_entry_text(entry: &CachedRosterEntry, now: u64, is_local: bool) -> String {
    let status = if entry.connected {
        match entry.status {
            CachedRosterStatus::Alive => "alive".to_string(),
            CachedRosterStatus::Ready => "ready".to_string(),
            CachedRosterStatus::RestartReady => "restart ready".to_string(),
            CachedRosterStatus::Defeated => "defeated".to_string(),
            CachedRosterStatus::Respawning(respawn_at_tick) => format!(
                "respawn {}",
                respawn_at_tick.saturating_sub(now).div_ceil(60)
            ),
            CachedRosterStatus::Protected(expires_at_tick) => format!(
                "protected {}",
                expires_at_tick.saturating_sub(now).div_ceil(60)
            ),
        }
    } else {
        "disconnected".to_string()
    };
    let weapon = entry
        .weapon_preset
        .map_or_else(|| "W?".to_string(), |preset| format!("W{preset}"));
    let local = if is_local { "YOU " } else { "" };
    format!(
        "T{}  {local}{}  {weapon}  {status}",
        entry.team.0 + 1,
        entry.display_name
    )
}

#[cfg(test)]
fn readiness_status(
    join: Option<&ClientJoinPhase>,
    map: &crate::map::ClientMapReadiness,
    assets: &assets::ClientAssetReadiness,
    playable: bool,
) -> String {
    if let crate::map::ClientMapReadiness::Invalid(reason) = map {
        return format!("MAP REJECTED | {reason}");
    }
    if let Some(ClientJoinPhase::Rejected(reason)) = join {
        return format!("JOIN REJECTED | {reason:?}");
    }
    if matches!(join, Some(ClientJoinPhase::Disconnected)) {
        return "DISCONNECTED | reconnect by restarting the client".to_string();
    }
    if playable {
        return match assets {
            assets::ClientAssetReadiness::Degraded(failed) => {
                format!("READY | FALLBACKS ACTIVE: {}", failed.join(", "))
            }
            _ => "READY".to_string(),
        };
    }
    match (join, map, assets) {
        (_, _, assets::ClientAssetReadiness::Loading) => "LOADING CLIENT CONTENT".to_string(),
        (None | Some(ClientJoinPhase::Connecting), _, _) => "CONNECTING".to_string(),
        (Some(ClientJoinPhase::AwaitingOutcome), _, _) => "HANDSHAKING".to_string(),
        (
            Some(ClientJoinPhase::Active { .. }),
            crate::map::ClientMapReadiness::WaitingForSnapshot,
            _,
        ) => "WAITING FOR AUTHORITATIVE MAP".to_string(),
        _ => "PREPARING SANDBOX".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_hud_text_queries_are_disjoint_in_runtime_composition() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<MatchRosterPresentation>()
            .add_systems(Update, update_readiness_hud);
        app.world_mut()
            .spawn((ReadinessHudText, Text::new(""), Visibility::Hidden));
        app.world_mut()
            .spawn((MatchHudText, Text::new(""), Visibility::Hidden));
        app.world_mut()
            .spawn((CountdownHudText, Text::new(""), Visibility::Hidden));
        app.world_mut()
            .spawn((MatchPhaseOverlayText, Text::new(""), Visibility::Hidden));
        app.world_mut()
            .spawn((ScoreboardOverlay, Text::new(""), Visibility::Hidden));

        app.update();
    }

    #[test]
    fn invalid_map_has_priority_over_loading_state() {
        assert_eq!(
            readiness_status(
                None,
                &crate::map::ClientMapReadiness::Invalid("bad schema".to_string()),
                &assets::ClientAssetReadiness::Loading,
                false,
            ),
            "MAP REJECTED | bad schema"
        );
    }

    #[test]
    fn degraded_assets_are_visible_but_do_not_block_play() {
        assert_eq!(
            readiness_status(
                None,
                &crate::map::ClientMapReadiness::Ready,
                &assets::ClientAssetReadiness::Degraded(vec!["audio.fire"]),
                true,
            ),
            "READY | FALLBACKS ACTIVE: audio.fire"
        );
    }

    #[test]
    fn cached_roster_keeps_weapon_and_disconnected_state() {
        let entry = CachedRosterEntry {
            team: TeamId(1),
            display_name: "Player Seven".to_string(),
            weapon_preset: Some(3),
            status: CachedRosterStatus::Ready,
            connected: false,
        };
        assert_eq!(
            roster_entry_text(&entry, 100, false),
            "T2  Player Seven  W3  disconnected"
        );
        assert_eq!(
            roster_entry_text(&entry, 100, true),
            "T2  YOU Player Seven  W3  disconnected"
        );
    }

    #[test]
    fn countdown_label_uses_authoritative_deadline_and_hides_outside_countdown() {
        let mut state = MatchState {
            match_id: crate::matchplay::MatchId(1),
            mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
            phase: MatchPhase::Countdown {
                starts_at_tick: 180,
            },
            rules_revision: 1,
        };
        assert_eq!(countdown_label(Some(state), Some(0)).as_deref(), Some("3"));
        assert_eq!(
            countdown_label(Some(state), Some(120)).as_deref(),
            Some("1")
        );
        assert_eq!(
            countdown_label(Some(state), None),
            None,
            "a missing or mismatched generation hides the countdown instead of guessing"
        );
        state.phase = MatchPhase::Active {
            ends_at_tick: 1_000,
        };
        assert_eq!(countdown_label(Some(state), Some(120)), None);
    }

    #[test]
    fn mode_score_views_are_generation_safe_and_mode_specific() {
        let wipeout_state = crate::matchplay::WipeoutState {
            team_scores: [3, 2],
            target_score: 5,
        };
        let mut state = MatchState {
            match_id: crate::matchplay::MatchId(7),
            mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
            phase: MatchPhase::Active { ends_at_tick: 600 },
            rules_revision: 1,
        };
        let clock = crate::matchplay::MatchClock {
            match_id: state.match_id,
            completed_tick: 10,
        };
        assert_eq!(
            build_mode_score_view(Some((&state, Some(&wipeout_state))), None, Some(&clock)),
            Some(ModeScoreView::Wipeout {
                scores: [3, 2],
                target: 5,
            })
        );
        assert_eq!(
            mode_score_text(ModeScoreView::Wipeout {
                scores: [3, 2],
                target: 5,
            }),
            "T1  3  —  2  T2\nFIRST TO 5"
        );

        state.mode_definition_id = crate::map::HOT_ZONE_MODE_DEFINITION;
        let hot_zone = crate::matchplay::HotZoneState {
            match_id: state.match_id,
            zone_anchor_id: crate::map::ModeAnchorId(1),
            occupants: [1, 0],
            status: crate::matchplay::HotZoneStatus::Controlled { team: TeamId(0) },
            progress_ticks: [30, 15],
            target_progress_ticks: 60,
            next_evaluation_tick: 11,
        };
        assert_eq!(
            build_mode_score_view(Some((&state, None)), Some(&hot_zone), Some(&clock)),
            Some(ModeScoreView::HotZone {
                progress_percent: [50, 25],
                status: crate::matchplay::HotZoneStatus::Controlled { team: TeamId(0) },
            })
        );
        let stale_clock = crate::matchplay::MatchClock {
            match_id: crate::matchplay::MatchId(8),
            ..clock
        };
        assert_eq!(
            build_mode_score_view(Some((&state, None)), Some(&hot_zone), Some(&stale_clock)),
            Some(ModeScoreView::Syncing)
        );
    }

    #[test]
    fn phase_overlay_explains_ready_and_completed_restart_states() {
        let mut state = MatchState {
            match_id: crate::matchplay::MatchId(1),
            mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
            phase: MatchPhase::Waiting,
            rules_revision: 1,
        };
        let wipeout_state = crate::matchplay::WipeoutState {
            team_scores: [3, 1],
            target_score: 10,
        };
        assert_eq!(
            phase_overlay_label(Some(state), None, Some(0), 4, 0, 0, false, false, false),
            None
        );
        let waiting = phase_overlay_label(
            Some(state),
            final_objective_line(Some((&state, Some(&wipeout_state))), None).as_deref(),
            Some(0),
            4,
            2,
            0,
            true,
            false,
            false,
        )
        .unwrap();
        assert!(waiting.contains("PRESS SPACE / ENTER / A TO READY"));
        assert!(waiting.contains("2/4 fighters ready"));

        state.phase = MatchPhase::Completed {
            completed_at_tick: 100,
            restart_unlocked_at_tick: 160,
            result: crate::matchplay::MatchResult::Forfeit {
                winner: TeamId(0),
                departed_team: TeamId(1),
            },
        };
        let locked = phase_overlay_label(
            Some(state),
            final_objective_line(Some((&state, Some(&wipeout_state))), None).as_deref(),
            Some(100),
            4,
            4,
            2,
            true,
            true,
            false,
        )
        .unwrap();
        assert!(locked.contains("TEAM 2 FORFEITED"));
        assert!(locked.contains("FINAL  3 - 1  |  MARGIN 2"));
        assert!(locked.contains("Restart unlocks in 1"));
        assert!(locked.contains("2/4 restart ready"));

        let syncing = phase_overlay_label(
            Some(state),
            final_objective_line(Some((&state, None)), None).as_deref(),
            None,
            4,
            4,
            2,
            true,
            true,
            false,
        )
        .unwrap();
        assert!(syncing.contains("FINAL  SYNCING"));
        assert!(syncing.contains("SYNCING"));
    }
    #[test]
    fn hot_zone_completed_overlay_shows_final_percentages() {
        let state = MatchState {
            match_id: crate::matchplay::MatchId(1),
            mode_definition_id: crate::map::HOT_ZONE_MODE_DEFINITION,
            phase: MatchPhase::Completed {
                completed_at_tick: 100,
                restart_unlocked_at_tick: 160,
                result: crate::matchplay::MatchResult::TeamVictory { team: TeamId(0) },
            },
            rules_revision: 1,
        };
        let hot_zone = crate::matchplay::HotZoneState {
            match_id: crate::matchplay::MatchId(1),
            zone_anchor_id: crate::map::ModeAnchorId(1),
            occupants: [1, 0],
            status: crate::matchplay::HotZoneStatus::Controlled { team: TeamId(0) },
            progress_ticks: [15, 3],
            target_progress_ticks: 30,
            next_evaluation_tick: 100,
        };
        let label = phase_overlay_label(
            Some(state),
            final_objective_line(Some((&state, None)), Some(&hot_zone)).as_deref(),
            Some(100),
            2,
            2,
            0,
            true,
            true,
            true,
        )
        .unwrap();
        assert!(label.contains("TEAM 1 WINS"));
        assert!(label.contains("FINAL  T1 50%  T2 10%"));

        let mismatched = crate::matchplay::HotZoneState {
            match_id: crate::matchplay::MatchId(9),
            ..hot_zone
        };
        let syncing = phase_overlay_label(
            Some(state),
            final_objective_line(Some((&state, None)), Some(&mismatched)).as_deref(),
            Some(100),
            2,
            2,
            0,
            true,
            true,
            true,
        )
        .unwrap();
        assert!(syncing.contains("SYNCING OBJECTIVE"));
    }
}
