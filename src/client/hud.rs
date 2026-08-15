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
        TextLayout::new(Justify::Right, LineBreak::WordBoundary),
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
}

#[allow(clippy::too_many_arguments)]
fn update_readiness_hud(
    joins: Query<&ClientJoinStatus>,
    map: Res<crate::map::ClientMapReadiness>,
    assets: Res<assets::ClientAssetReadiness>,
    playable: Res<ClientPlayableGate>,
    controlled: Query<(&PlayerId, &TeamId), (With<Fighter>, With<Controlled>)>,
    matches: Query<&MatchState, With<MatchRoot>>,
    participants: Query<
        (
            &PlayerId,
            &TeamId,
            &MatchParticipant,
            Option<&crate::combat::SelectedBuild>,
            Option<&RespawnState>,
            Option<&SpawnProtection>,
            Option<&crate::combat::Defeated>,
        ),
        With<Fighter>,
    >,
    ticks: Query<&AuthoritativeTick, (With<Fighter>, With<Controlled>)>,
    pending: Res<PendingLocalActions>,
    mut roster_presentation: ResMut<MatchRosterPresentation>,
    mut readiness_text: Query<
        &mut Text,
        (
            With<ReadinessHudText>,
            Without<MatchHudText>,
            Without<CountdownHudText>,
        ),
    >,
    mut match_text: Query<
        &mut Text,
        (
            With<MatchHudText>,
            Without<ReadinessHudText>,
            Without<CountdownHudText>,
        ),
    >,
    mut countdown_text: Query<
        (&mut Text, &mut Visibility),
        (
            With<CountdownHudText>,
            Without<ReadinessHudText>,
            Without<MatchHudText>,
        ),
    >,
) {
    let join = joins.iter().next().map(|status| &status.phase);
    let status = readiness_status(join, &map, &assets, playable.0);
    for mut text in &mut readiness_text {
        text.0 = if status == "READY" {
            String::new()
        } else {
            status.clone()
        };
    }
    let fighter = controlled.iter().next();
    let match_state = matches.iter().next();
    let now = ticks.iter().next().map_or(0, |tick| tick.0);
    for (mut text, mut visibility) in &mut countdown_text {
        if let Some(label) = countdown_label(match_state, now) {
            text.0 = label;
            *visibility = Visibility::Inherited;
        } else {
            text.0.clear();
            *visibility = Visibility::Hidden;
        }
    }
    if let Some(state) = match_state {
        if roster_presentation.match_id != Some(state.match_id) {
            roster_presentation.match_id = Some(state.match_id);
            roster_presentation.entries.clear();
        }
        for entry in roster_presentation.entries.values_mut() {
            entry.connected = false;
        }
        for (player, team, participant, build, respawn, protection, defeated) in &participants {
            if participant.match_id != state.match_id {
                continue;
            }
            roster_presentation.entries.insert(
                player.0,
                CachedRosterEntry {
                    team: *team,
                    weapon_preset: build
                        .and_then(|build| build.source_preset_id.map(|preset| preset.0)),
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
    }
    for mut text in &mut match_text {
        **text = match_state.map_or_else(
            || "WIPEOUT | waiting for match state".to_string(),
            |state| {
                let phase = match state.phase {
                    MatchPhase::Waiting => "WAITING".to_string(),
                    MatchPhase::Countdown { starts_at_tick } => format!(
                        "STARTING IN {}",
                        starts_at_tick.saturating_sub(now).div_ceil(60)
                    ),
                    MatchPhase::Active { ends_at_tick } => format!(
                        "{}:{:02}",
                        ends_at_tick.saturating_sub(now) / 3600,
                        (ends_at_tick.saturating_sub(now) / 60) % 60
                    ),
                    MatchPhase::Completed { result, .. } => format!("{result:?}"),
                };
                let local = fighter.map_or_else(String::new, |(player, team)| {
                    format!(" | P{} T{}", player.0, team.0 + 1)
                });
                let roster = roster_presentation
                    .entries
                    .iter()
                    .map(|(player, entry)| roster_entry_text(*player, entry, now))
                    .collect::<Vec<_>>()
                    .join(" | ");
                let show_roster =
                    pending.scoreboard_held || !matches!(state.phase, MatchPhase::Active { .. });
                let roster = if show_roster {
                    format!("\n{roster}")
                } else {
                    String::new()
                };
                format!(
                    "WIPEOUT{local} | {}-{} / {} | {phase}{roster}",
                    state.team_scores[0], state.team_scores[1], state.target_score
                )
            },
        );
    }
}

fn countdown_label(state: Option<&MatchState>, now: u64) -> Option<String> {
    let MatchPhase::Countdown { starts_at_tick } = state?.phase else {
        return None;
    };
    Some(
        starts_at_tick
            .saturating_sub(now)
            .div_ceil(60)
            .max(1)
            .to_string(),
    )
}

fn roster_entry_text(player: u64, entry: &CachedRosterEntry, now: u64) -> String {
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
    format!("P{player} T{} {weapon} {status}", entry.team.0 + 1)
}

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
            weapon_preset: Some(3),
            status: CachedRosterStatus::Ready,
            connected: false,
        };
        assert_eq!(roster_entry_text(7, &entry, 100), "P7 T2 W3 disconnected");
    }

    #[test]
    fn countdown_label_uses_authoritative_deadline_and_hides_outside_countdown() {
        let mut state = MatchState {
            match_id: crate::matchplay::MatchId(1),
            mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
            phase: MatchPhase::Countdown {
                starts_at_tick: 180,
            },
            team_scores: [0, 0],
            target_score: 10,
            rules_revision: 1,
        };
        assert_eq!(countdown_label(Some(&state), 0).as_deref(), Some("3"));
        assert_eq!(countdown_label(Some(&state), 120).as_deref(), Some("1"));
        state.phase = MatchPhase::Active {
            ends_at_tick: 1_000,
        };
        assert_eq!(countdown_label(Some(&state), 120), None);
    }
}
