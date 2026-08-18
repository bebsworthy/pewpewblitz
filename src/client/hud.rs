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
    weapon_preset: Option<u16>,
    status: CachedRosterStatus,
    connected: bool,
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
        Text::new("LOADING CLIENT CONTENT"),
        TextFont::from_font_size(18.0),
        TextColor(Color::srgb(0.72, 0.88, 1.0)),
        TextLayout::new(Justify::Center, LineBreak::WordBoundary),
        GlobalZIndex(100),
        Node {
            position_type: PositionType::Absolute,
            left: percent(30.0),
            right: percent(30.0),
            top: px(16.0),
            ..default()
        },
    ));
    commands.spawn((
        MatchHudText,
        Text::new("WIPEOUT | waiting for fighter"),
        TextFont::from_font_size(16.0),
        TextColor(Color::srgb(0.78, 0.82, 0.88)),
        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
        GlobalZIndex(100),
        Node {
            position_type: PositionType::Absolute,
            right: px(16.0),
            top: px(80.0),
            width: px(380.0),
            ..default()
        },
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
    joins: Query<&ClientJoinStatus>,
    map: Res<crate::map::ClientMapReadiness>,
    terrain: Res<crate::terrain::ClientTerrainReadiness>,
    assets: Res<assets::ClientAssetReadiness>,
    playable: Res<ClientPlayableGate>,
    controlled: Query<(&PlayerId, &TeamId), (With<Fighter>, With<Controlled>)>,
    matches: Query<(&MatchState, Option<&crate::matchplay::WipeoutState>), With<MatchRoot>>,
    hot_zones: Query<&crate::matchplay::HotZoneState, With<MatchRoot>>,
    clocks: Query<&crate::matchplay::MatchClock, With<MatchRoot>>,
    participants: Query<
        (
            &PlayerId,
            &TeamId,
            &MatchParticipant,
            Option<&crate::builds::SelectedBuild>,
            Option<&crate::builds::ResolvedMatchLoadout>,
            Option<&RespawnState>,
            Option<&SpawnProtection>,
            Option<&crate::combat::Defeated>,
        ),
        With<Fighter>,
    >,
    pending: Res<PendingLocalActions>,
    mut roster_presentation: ResMut<MatchRosterPresentation>,
    mut readiness_text: Query<
        &mut Text,
        (
            With<ReadinessHudText>,
            Without<MatchHudText>,
            Without<CountdownHudText>,
            Without<MatchPhaseOverlayText>,
        ),
    >,
    mut match_text: Query<
        &mut Text,
        (
            With<MatchHudText>,
            Without<ReadinessHudText>,
            Without<CountdownHudText>,
            Without<MatchPhaseOverlayText>,
        ),
    >,
    mut countdown_text: Query<
        (&mut Text, &mut Visibility),
        (
            With<CountdownHudText>,
            Without<ReadinessHudText>,
            Without<MatchHudText>,
            Without<MatchPhaseOverlayText>,
        ),
    >,
    mut phase_overlay: Query<
        (&mut Text, &mut Visibility),
        (
            With<MatchPhaseOverlayText>,
            Without<ReadinessHudText>,
            Without<MatchHudText>,
            Without<CountdownHudText>,
        ),
    >,
) {
    let join = joins.iter().next().map(|status| &status.phase);
    let status = readiness_status(join, &map, &terrain, &assets, playable.0);
    for mut text in &mut readiness_text {
        text.0 = if status == "READY" {
            String::new()
        } else {
            status.clone()
        };
    }
    let fighter = controlled.iter().next();
    let match_state = matches.iter().next();
    let clock = clocks.iter().next();
    // Deadlines derive only from the phase minus the generation-tagged match clock; any other
    // component arrival order displays a syncing label instead of a stale or local countdown.
    let now = match_deadline_tick(match_state.map(|(state, _)| *state), clock);
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
    for mut text in &mut match_text {
        **text = match_state.map_or_else(
            || "waiting for match state".to_string(),
            |(state, wipeout)| {
                let phase = match state.phase {
                    MatchPhase::Waiting => "WAITING".to_string(),
                    MatchPhase::Countdown { starts_at_tick } => match now {
                        Some(now) => format!(
                            "STARTING IN {}",
                            starts_at_tick.saturating_sub(now).div_ceil(60)
                        ),
                        None => "SYNCING".to_string(),
                    },
                    MatchPhase::Active { ends_at_tick } => match now {
                        Some(now) => format!(
                            "{}:{:02}",
                            ends_at_tick.saturating_sub(now) / 3600,
                            (ends_at_tick.saturating_sub(now) / 60) % 60
                        ),
                        None => "SYNCING".to_string(),
                    },
                    MatchPhase::Completed { result, .. } => format!("{result:?}"),
                };
                let local = fighter.map_or_else(String::new, |(player, team)| {
                    format!(" | P{} T{}", player.0, team.0 + 1)
                });
                let roster = roster_presentation
                    .entries
                    .iter()
                    .map(|(player, entry)| {
                        roster_entry_text(
                            *player,
                            entry,
                            now.unwrap_or(0),
                            local_player == Some(*player),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let show_roster =
                    pending.scoreboard_held || !matches!(state.phase, MatchPhase::Active { .. });
                let roster = if show_roster {
                    format!("\n{roster}")
                } else {
                    String::new()
                };
                match state.mode_definition_id {
                    crate::map::WIPEOUT_MODE_DEFINITION => {
                        let scores = wipeout.map_or_else(
                            || "SYNCING".to_string(),
                            |wipeout| {
                                format!(
                                    "{}-{} / {}",
                                    wipeout.team_scores[0],
                                    wipeout.team_scores[1],
                                    wipeout.target_score
                                )
                            },
                        );
                        format!("WIPEOUT{local} | {scores} | {phase}{roster}")
                    }
                    crate::map::HOT_ZONE_MODE_DEFINITION => {
                        let hot_zone = hot_zones.iter().next().filter(|hot_zone| {
                            hot_zone.match_id == state.match_id
                                && clock_generation_matches(match_state, clock)
                        });
                        let objective = hot_zone.map_or_else(
                            || "SYNCING OBJECTIVE".to_string(),
                            |hot_zone| {
                                let percent = |progress: u16| {
                                    u32::from(progress) * 100
                                        / u32::from(hot_zone.target_progress_ticks)
                                };
                                let ownership = match hot_zone.status {
                                    crate::matchplay::HotZoneStatus::Empty => "EMPTY".to_string(),
                                    crate::matchplay::HotZoneStatus::Contested => {
                                        "CONTESTED".to_string()
                                    }
                                    crate::matchplay::HotZoneStatus::Controlled { team } => {
                                        format!("TEAM {} CONTROL", team.0 + 1)
                                    }
                                };
                                format!(
                                    "T1 {}% T2 {}% | {ownership}",
                                    percent(hot_zone.progress_ticks[0]),
                                    percent(hot_zone.progress_ticks[1]),
                                )
                            },
                        );
                        format!("HOT ZONE{local} | {objective} | {phase}{roster}")
                    }
                    _ => format!("UNKNOWN MODE{local} | {phase}{roster}"),
                }
            },
        );
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

fn clock_generation_matches(
    state: Option<(&MatchState, Option<&crate::matchplay::WipeoutState>)>,
    clock: Option<&crate::matchplay::MatchClock>,
) -> bool {
    state.is_some_and(|(state, _)| clock.is_some_and(|clock| clock.match_id == state.match_id))
}

fn sync_roster_and_collect_phase_facts<'a>(
    state: Option<MatchState>,
    local_player: Option<u64>,
    participants: impl Iterator<
        Item = (
            &'a PlayerId,
            &'a TeamId,
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
    for (player, team, participant, build, loadout, respawn, protection, defeated) in participants {
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

fn roster_entry_text(player: u64, entry: &CachedRosterEntry, now: u64, is_local: bool) -> String {
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
    format!("{local}P{player} T{} {weapon} {status}", entry.team.0 + 1)
}

fn readiness_status(
    join: Option<&ClientJoinPhase>,
    map: &crate::map::ClientMapReadiness,
    terrain: &crate::terrain::ClientTerrainReadiness,
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
    match (join, map, terrain, assets) {
        (_, _, _, assets::ClientAssetReadiness::Loading) => "LOADING CLIENT CONTENT".to_string(),
        (None | Some(ClientJoinPhase::Connecting), _, _, _) => "CONNECTING".to_string(),
        (Some(ClientJoinPhase::AwaitingOutcome), _, _, _) => "HANDSHAKING".to_string(),
        (
            Some(ClientJoinPhase::Active { .. }),
            crate::map::ClientMapReadiness::WaitingForSnapshot,
            _,
            _,
        ) => "WAITING FOR AUTHORITATIVE MAP".to_string(),
        (
            Some(ClientJoinPhase::Active { .. }),
            _,
            crate::terrain::ClientTerrainReadiness::WaitingForMap,
            _,
        ) => "SYNCING TERRAIN | WAITING FOR MAP".to_string(),
        (
            Some(ClientJoinPhase::Active { .. }),
            _,
            crate::terrain::ClientTerrainReadiness::SyncingTerrain,
            _,
        ) => "SYNCING TERRAIN".to_string(),
        (
            Some(ClientJoinPhase::Active { .. }),
            _,
            crate::terrain::ClientTerrainReadiness::RecoveringTerrain,
            _,
        ) => "RECOVERING TERRAIN".to_string(),
        (
            Some(ClientJoinPhase::Active { .. }),
            _,
            crate::terrain::ClientTerrainReadiness::Invalid(reason),
            _,
        ) => format!("TERRAIN REJECTED | {reason}"),
        _ => "PREPARING SANDBOX".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_map_has_priority_over_loading_state() {
        assert_eq!(
            readiness_status(
                None,
                &crate::map::ClientMapReadiness::Invalid("bad schema".to_string()),
                &crate::terrain::ClientTerrainReadiness::Ready,
                &assets::ClientAssetReadiness::Loading,
                false,
            ),
            "MAP REJECTED | bad schema"
        );
    }

    #[test]
    fn terrain_sync_states_are_distinct_and_exact() {
        let active_phase = ClientJoinPhase::Active {
            player_id: PlayerId(1),
            network_entity_id: NetworkEntityId(1),
        };
        let active = Some(&active_phase);
        let cases = [
            (
                crate::terrain::ClientTerrainReadiness::WaitingForMap,
                "SYNCING TERRAIN | WAITING FOR MAP",
            ),
            (
                crate::terrain::ClientTerrainReadiness::SyncingTerrain,
                "SYNCING TERRAIN",
            ),
            (
                crate::terrain::ClientTerrainReadiness::RecoveringTerrain,
                "RECOVERING TERRAIN",
            ),
            (
                crate::terrain::ClientTerrainReadiness::Invalid(
                    "recovery snapshot chunk set mismatch".to_string(),
                ),
                "TERRAIN REJECTED | recovery snapshot chunk set mismatch",
            ),
        ];
        for (terrain, expected) in cases {
            assert_eq!(
                readiness_status(
                    active,
                    &crate::map::ClientMapReadiness::Ready,
                    &terrain,
                    &assets::ClientAssetReadiness::Ready,
                    false,
                ),
                expected
            );
        }
    }

    #[test]
    fn degraded_assets_are_visible_but_do_not_block_play() {
        assert_eq!(
            readiness_status(
                None,
                &crate::map::ClientMapReadiness::Ready,
                &crate::terrain::ClientTerrainReadiness::Ready,
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
            weapon_preset: Some(3),
            status: CachedRosterStatus::Ready,
            connected: false,
        };
        assert_eq!(
            roster_entry_text(7, &entry, 100, false),
            "P7 T2 W3 disconnected"
        );
        assert_eq!(
            roster_entry_text(7, &entry, 100, true),
            "YOU P7 T2 W3 disconnected"
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
