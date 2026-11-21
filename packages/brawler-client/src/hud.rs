//! Readiness and match-shell HUD that remains independent from gameplay mutation.

use super::{
    Client, Controlled, Fighter, MatchPhase, MatchRoot, MatchState, MessageReceiver, PlayerId,
    default, touch, ui,
};
#[cfg(test)]
use crate::{ClientJoinPhase, assets};
use bevy::prelude::*;
use brawler::combat::TeamId;
use std::collections::BTreeMap;

mod controls;
mod kill_feed;
mod layout;
mod plate;
mod scoreboard;
#[cfg(test)]
pub(crate) mod screen_tests;

pub(crate) use layout::HudControlLayout;
pub(crate) use plate::PLATE_BOTTOM as HUD_PLATE_BOTTOM;
use plate::{PlateState, TeamStripRow};
pub(crate) use scoreboard::ScoreboardCloseButton;
use scoreboard::ScoreboardState;

/// HUD colors: observer relations match the overhead plates and ground rings.
mod palette {
    use bevy::prelude::Color;

    pub(super) const LOCAL: Color = Color::srgb(0.18, 0.92, 0.34);
    pub(super) const ALLY: Color = Color::srgb(0.12, 0.66, 0.98);
    pub(super) const ENEMY: Color = Color::srgb(0.95, 0.14, 0.12);
    /// Translucent `ink` behind HUD plates, feed rows and button tracks.
    pub(super) const PLATE: Color = Color::srgba(0.055, 0.086, 0.22, 0.86);
    /// The ultimate violet shared with the targeted-ultimate previews.
    pub(super) const ULTIMATE: Color = Color::srgb(0.74, 0.42, 1.0);
}

const HUD_Z_INDEX: i32 = 150;

/// Full-screen root of the persistent in-match HUD (plate, kill feed, controls).
#[derive(Component)]
pub(crate) struct HudRoot;

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum ClientVaultBreakReadiness {
    #[default]
    Waiting,
    Ready,
    Invalid(String),
}

#[derive(Resource, Default)]
struct SeenVaultBreakObjectiveCueIds(std::collections::VecDeque<brawler::combat::CombatEventId>);

#[derive(Component)]
struct CountdownHudText;

#[derive(Component)]
struct MatchPhaseOverlayText;

#[derive(Clone, Debug, PartialEq, Eq)]
struct CachedRosterEntry {
    team: TeamId,
    display_name: String,
    status: CachedRosterStatus,
    connected: bool,
    /// The server-published hold expiry while a bot drives this disconnected player's seat.
    held_until: Option<u64>,
    health: Option<(u16, u16)>,
    boost_stacks: u8,
}

/// Small presentation-only dispatch for the one shared top-right objective slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModeScoreView {
    Takedown {
        scores: [u16; 2],
        target: u16,
    },
    TurfHold {
        progress_percent: [u8; 2],
        status: brawler::matchplay::TurfHoldStatus,
    },
    VaultBreak {
        health: [u16; 2],
        maximum: [u16; 2],
        local_team: Option<TeamId>,
    },
    Zombie {
        live_zombies: u8,
        last_brawler_standing: bool,
        local_boost_stacks: u8,
    },
    ShardRush {
        shards: [u8; 2],
        target: u8,
        /// The holding team and whole seconds left on its countdown.
        hold: Option<(TeamId, u64)>,
    },
    LastStand {
        alive: u8,
        fighters: u8,
        zone: LastStandZone,
        /// The local fighter's finishing place once it is eliminated.
        local_place: Option<u8>,
        local_boost_stacks: u8,
    },
    Syncing,
}

/// What the Last Stand ring is doing now, in whole seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LastStandZone {
    ClosesIn(u64),
    Closing(u64),
    Final,
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
    match_id: Option<brawler::matchplay::MatchId>,
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

#[derive(bevy::ecs::system::SystemParam)]
struct ZombieHudFacts<'w, 's> {
    states: Query<'w, 's, &'static brawler::matchplay::ZombieModeState, With<MatchRoot>>,
    fighters: Query<
        'w,
        's,
        (
            &'static PlayerId,
            &'static brawler::combat::CurrentHealth,
            // Replicated on every fighter (the overhead plates read it too).
            &'static brawler::builds::ResolvedMatchLoadout,
            Option<&'static brawler::matchplay::BoostStacks>,
        ),
        With<Fighter>,
    >,
}

pub struct ClientReadinessHudPlugin;

impl Plugin for ClientReadinessHudPlugin {
    fn build(&self, app: &mut App) {
        ui::touch_target::add_touch_targets(app);
        app.init_resource::<MatchRosterPresentation>()
            .init_resource::<ClientVaultBreakReadiness>()
            .init_resource::<SeenVaultBreakObjectiveCueIds>()
            .add_message::<brawler::matchplay::ReceivedVaultBreakObjectiveCue>()
            .init_resource::<PlateState>()
            .init_resource::<ScoreboardState>()
            .init_resource::<kill_feed::KillFeed>()
            .init_resource::<kill_feed::SeatControlTransitions>()
            .init_resource::<touch::TouchControls>()
            .add_systems(Startup, spawn_readiness_hud)
            .add_systems(
                Update,
                (
                    converge_vault_break_readiness,
                    receive_vault_break_objective_cues,
                    (
                        update_readiness_hud,
                        plate::present_plate,
                        scoreboard::present_scoreboard,
                    )
                        .chain()
                        .after(converge_vault_break_readiness),
                    plate::place_team_strip,
                    (kill_feed::collect_kill_feed, kill_feed::present_kill_feed)
                        .chain()
                        .after(brawler::combat::client::CombatClientSet::Ingest),
                    (
                        controls::place_controls,
                        controls::present_button_rings,
                        controls::present_button_content,
                        controls::present_hints,
                    )
                        .chain(),
                ),
            );
    }
}

#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
fn converge_vault_break_readiness(
    matches: Query<(&MatchState, Option<&brawler::matchplay::VaultBreakState>), With<MatchRoot>>,
    maps: Query<
        (
            &brawler::map::ResolvedMapSnapshot,
            &brawler::map::MapDynamicState,
        ),
        With<brawler::map::MapRoot>,
    >,
    safes: Query<(
        &brawler::matchplay::VaultBreakSafe,
        &brawler::map::DamageableTargetIdentity,
        &brawler::combat::CurrentHealth,
        &brawler::map::DamageableMaximumHealth,
        &brawler::map::DamageableLifeState,
    )>,
    mut readiness: ResMut<ClientVaultBreakReadiness>,
) {
    let Ok((state, vault_break)) = matches.single() else {
        *readiness = ClientVaultBreakReadiness::Waiting;
        return;
    };
    if state.mode_definition_id != brawler::map::VAULT_BREAK_MODE_DEFINITION {
        *readiness = ClientVaultBreakReadiness::Ready;
        return;
    }
    let Some(vault_break) = vault_break else {
        *readiness = ClientVaultBreakReadiness::Waiting;
        return;
    };
    let Ok((snapshot, map_state)) = maps.single() else {
        *readiness = ClientVaultBreakReadiness::Waiting;
        return;
    };
    let generation = map_state.generation_id();
    if vault_break.match_id != state.match_id
        || vault_break.generation != generation
        || snapshot.identity.instance_id != generation.map_instance_id
        || snapshot.mode_definition_id != brawler::map::VAULT_BREAK_MODE_DEFINITION
    {
        *readiness = ClientVaultBreakReadiness::Invalid(
            "Vault Break root and map generations disagree".to_string(),
        );
        return;
    }
    let expected: std::collections::BTreeSet<_> = vault_break
        .safes
        .iter()
        .map(|safe| (safe.anchor_id, safe.defending_team))
        .collect();
    if expected.len() != brawler::matchplay::VAULT_BREAK_SAFE_COUNT {
        *readiness = ClientVaultBreakReadiness::Invalid(
            "Vault Break root does not declare two unique safes".to_string(),
        );
        return;
    }
    let mut actual = std::collections::BTreeSet::new();
    for (safe, identity, health, maximum, life) in &safes {
        if safe.match_id != state.match_id || safe.generation != generation {
            continue;
        }
        if *identity
            != (brawler::map::DamageableTargetIdentity::VaultBreakSafe {
                match_id: state.match_id,
                anchor_id: safe.anchor_id,
                defending_team: safe.defending_team,
            })
            || maximum.0 == 0
            || health.0 > maximum.0
            || (health.0 == 0)
                != matches!(*life, brawler::map::DamageableLifeState::TerminalCommitted)
        {
            *readiness = ClientVaultBreakReadiness::Invalid(
                "replicated Vault Break safe state is inconsistent".to_string(),
            );
            return;
        }
        if !actual.insert((safe.anchor_id, safe.defending_team)) {
            *readiness = ClientVaultBreakReadiness::Invalid(
                "replicated Vault Break safes contain a duplicate identity".to_string(),
            );
            return;
        }
    }
    *readiness = if actual == expected {
        ClientVaultBreakReadiness::Ready
    } else if actual.is_subset(&expected) {
        ClientVaultBreakReadiness::Waiting
    } else {
        ClientVaultBreakReadiness::Invalid(
            "replicated Vault Break safes do not match the root".to_string(),
        )
    };
}

#[allow(clippy::needless_pass_by_value)]
fn receive_vault_break_objective_cues(
    mut receivers: Query<
        Option<&mut MessageReceiver<brawler::matchplay::VaultBreakObjectiveCue>>,
        With<Client>,
    >,
    mut output: MessageWriter<brawler::matchplay::ReceivedVaultBreakObjectiveCue>,
    mut seen: ResMut<SeenVaultBreakObjectiveCueIds>,
) {
    for receiver in &mut receivers {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for cue in receiver.receive() {
            if seen.0.contains(&cue.event_id) {
                continue;
            }
            if seen.0.len() >= brawler::matchplay::MAX_VAULT_BREAK_OBJECTIVE_CUES {
                seen.0.pop_front();
            }
            seen.0.push_back(cue.event_id);
            output.write(brawler::matchplay::ReceivedVaultBreakObjectiveCue(cue));
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are passed by value"
)]
fn spawn_readiness_hud(mut commands: Commands, asset_server: Option<Res<AssetServer>>) {
    let font = ui::theme::display_font_handle(asset_server.as_deref());
    let root = commands
        .spawn((
            HudRoot,
            Name::new("Match HUD"),
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            },
            // Above world-anchored overheads, numbers and object bars (119-124), below the
            // phase panels (220+), scoreboard and match menu.
            GlobalZIndex(HUD_Z_INDEX),
        ))
        .id();
    let skull = asset_server
        .as_deref()
        .map(|server| server.load(ui::icons::path("skull")))
        .unwrap_or_default();
    controls::spawn_controls(&mut commands, &font, asset_server.as_deref(), root);
    plate::spawn_plate(&mut commands, &font, root);
    kill_feed::spawn_kill_feed(&mut commands, &font, &skull, root);
    scoreboard::spawn_scoreboard(&mut commands, &font);
    commands.spawn((
        CountdownHudText,
        Text::new(""),
        ui::theme::hud_text(&font, 72.0),
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
            ui::theme::hud_text(&font, 34.0),
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
    matches: Query<(&MatchState, Option<&brawler::matchplay::TakedownState>), With<MatchRoot>>,
    turf_holds: Query<&brawler::matchplay::TurfHoldState, With<MatchRoot>>,
    vault_break_states: Query<&brawler::matchplay::VaultBreakState, With<MatchRoot>>,
    shard_rush_states: Query<&brawler::matchplay::ShardRushState, With<MatchRoot>>,
    last_stand_states: Query<&brawler::matchplay::LastStandState, With<MatchRoot>>,
    vault_break_safes: Query<(
        &brawler::matchplay::VaultBreakSafe,
        &brawler::combat::CurrentHealth,
        &brawler::map::DamageableMaximumHealth,
    )>,
    clocks: Query<&brawler::matchplay::MatchClock, With<MatchRoot>>,
    participants: Query<&brawler::matchplay::PublicParticipantState>,
    vault_break_readiness: Res<ClientVaultBreakReadiness>,
    mut roster_presentation: ResMut<MatchRosterPresentation>,
    zombie: ZombieHudFacts,
    mut plate_state: ResMut<PlateState>,
    mut scoreboard: ResMut<ScoreboardState>,
    mut countdown_text: Query<
        (&mut Text, &mut Visibility),
        (With<CountdownHudText>, Without<MatchPhaseOverlayText>),
    >,
    mut phase_overlay: Query<
        (&mut Text, &mut Visibility),
        (With<MatchPhaseOverlayText>, Without<CountdownHudText>),
    >,
) {
    let fighter = controlled.iter().next();
    let match_state = matches.iter().next();
    let clock = clocks.iter().next();
    // Deadlines derive only from the phase minus the generation-tagged match clock; any other
    // component arrival order displays a syncing label instead of a stale or local countdown.
    let now = match_deadline_tick(match_state.map(|(state, _)| *state), clock);
    let zombie_overtime = match_state.is_some_and(|(state, _)| {
        state.mode_definition_id == brawler::map::ZOMBIE_MODE_DEFINITION
            && matches!(state.phase, MatchPhase::Active { ends_at_tick } if now.is_some_and(|now| now >= ends_at_tick))
    });
    let timer = match_state.map_or_else(String::new, |(state, _)| {
        plate_timer_label(state.phase, now)
    });
    for (mut text, mut visibility) in &mut countdown_text {
        let label = countdown_label(match_state.map(|(state, _)| *state), now).unwrap_or_default();
        set_retained_text(&mut text, &mut visibility, label, true);
    }
    let local_player = fighter.map(|(player, _)| player.0);
    let phase_facts = sync_roster_and_collect_phase_facts(
        match_state.map(|(state, _)| *state),
        local_player,
        participants.iter(),
        &mut roster_presentation,
    );
    for (player, health, loadout, stacks) in &zombie.fighters {
        if let Some(entry) = roster_presentation.entries.get_mut(&player.0) {
            let stacks = stacks.copied().unwrap_or_default();
            entry.health = Some((
                health.0,
                brawler::matchplay::effective_maximum_health(
                    loadout.fighter_stats.maximum_health,
                    stacks,
                ),
            ));
            entry.boost_stacks = stacks.0;
        }
    }
    let final_line = match_state.and_then(|(state, _)| {
        if state.mode_definition_id == brawler::map::VAULT_BREAK_MODE_DEFINITION {
            final_vault_break_objective_line(
                state,
                matches!(*vault_break_readiness, ClientVaultBreakReadiness::Ready)
                    .then(|| vault_break_states.iter().next())
                    .flatten(),
                vault_break_safes.iter(),
            )
        } else if state.mode_definition_id == brawler::map::SHARD_RUSH_MODE_DEFINITION {
            Some(final_shard_rush_line(
                state,
                shard_rush_states.iter().next(),
            ))
        } else if state.mode_definition_id == brawler::map::LAST_STAND_MODE_DEFINITION {
            Some(final_last_stand_line(
                state,
                last_stand_states.iter().next(),
                fighter.map(|(_, team)| *team),
            ))
        } else {
            final_objective_line(match_state, turf_holds.iter().next())
        }
    });
    for (mut text, mut visibility) in &mut phase_overlay {
        let label = phase_overlay_label(
            match_state.map(|(state, _)| *state),
            final_line.as_deref(),
            now,
            phase_facts.participant_count,
            phase_facts.ready_count,
            phase_facts.restart_ready_count,
            phase_facts.local_selected,
            phase_facts.local_ready,
            phase_facts.local_restart_ready,
        )
        .unwrap_or_default();
        set_retained_text(&mut text, &mut visibility, label, true);
    }
    let score_view = match_state.and_then(|(state, _)| {
        if state.mode_definition_id == brawler::map::ZOMBIE_MODE_DEFINITION {
            zombie
                .states
                .iter()
                .next()
                .map(|mode| ModeScoreView::Zombie {
                    live_zombies: mode.live_zombies,
                    last_brawler_standing: mode.last_brawler_standing,
                    local_boost_stacks: local_player
                        .and_then(|player| roster_presentation.entries.get(&player))
                        .map_or(0, |entry| entry.boost_stacks),
                })
        } else if state.mode_definition_id == brawler::map::VAULT_BREAK_MODE_DEFINITION {
            Some(build_vault_break_score_view(
                state,
                matches!(*vault_break_readiness, ClientVaultBreakReadiness::Ready)
                    .then(|| vault_break_states.iter().next())
                    .flatten(),
                vault_break_safes.iter(),
                fighter.map(|(_, team)| *team),
                clock,
            ))
        } else if state.mode_definition_id == brawler::map::SHARD_RUSH_MODE_DEFINITION {
            Some(build_shard_rush_score_view(
                state,
                shard_rush_states.iter().next(),
                clock,
            ))
        } else if state.mode_definition_id == brawler::map::LAST_STAND_MODE_DEFINITION {
            Some(build_last_stand_score_view(
                state,
                last_stand_states.iter().next(),
                clock,
                fighter.map(|(_, team)| *team),
                local_player
                    .and_then(|player| roster_presentation.entries.get(&player))
                    .map_or(0, |entry| entry.boost_stacks),
            ))
        } else {
            build_mode_score_view(match_state, turf_holds.iter().next(), clock)
        }
    });
    let match_hud_phase = match_state.is_some_and(|(state, _)| {
        matches!(
            state.phase,
            MatchPhase::Countdown { .. } | MatchPhase::Active { .. }
        )
    });
    let local_team = fighter.map(|(_, team)| *team);
    scoreboard.set_if_neq(scoreboard::scoreboard_state(
        &roster_presentation,
        local_player,
        local_team,
        now,
        match_hud_phase,
    ));
    let team_strip = if matches!(score_view, Some(ModeScoreView::Zombie { .. })) {
        team_strip_rows(&roster_presentation, local_player, local_team, now)
    } else {
        Vec::new()
    };
    plate_state.set_if_neq(PlateState {
        shown: match_hud_phase && score_view.is_some(),
        timer,
        content: score_view.map_or_else(plate::PlateContent::default, |view| {
            plate::plate_content(view, local_team, zombie_overtime)
        }),
        team_strip,
    });
}

/// The plate timer: start countdown, remaining time, or the Zombie overtime clock.
fn plate_timer_label(phase: MatchPhase, now: Option<u64>) -> String {
    let Some(now) = now else {
        return match phase {
            MatchPhase::Countdown { .. } | MatchPhase::Active { .. } => "--:--".to_string(),
            MatchPhase::Waiting | MatchPhase::Completed { .. } => String::new(),
        };
    };
    match phase {
        MatchPhase::Countdown { starts_at_tick } => format!(
            "START {}",
            starts_at_tick.saturating_sub(now).div_ceil(60).max(1)
        ),
        MatchPhase::Active { ends_at_tick } => format_match_time(ends_at_tick.saturating_sub(now)),
        MatchPhase::Waiting | MatchPhase::Completed { .. } => String::new(),
    }
}

/// Zombie teammate rows (own team, local first), with health, boosts and down/respawn state.
fn team_strip_rows(
    roster: &MatchRosterPresentation,
    local_player: Option<u64>,
    local_team: Option<TeamId>,
    now: Option<u64>,
) -> Vec<TeamStripRow> {
    let mut rows: Vec<_> = roster
        .entries
        .iter()
        .filter(|(_, entry)| local_team.is_none_or(|team| entry.team == team))
        .collect();
    rows.sort_by_key(|(player, _)| (local_player != Some(**player), **player));
    rows.into_iter()
        .take(plate::TEAM_STRIP_ROWS)
        .map(|(player, entry)| {
            let seconds = |tick: u64| tick.saturating_sub(now.unwrap_or(tick)).div_ceil(60);
            let status = if entry.connected {
                match entry.status {
                    _ if entry.held_until.is_some() => "BOT".to_string(),
                    CachedRosterStatus::Defeated => "DOWN".to_string(),
                    CachedRosterStatus::Respawning(at) => format!("{}s", seconds(at)),
                    _ => format!("+{}", entry.boost_stacks),
                }
            } else {
                "OFF".to_string()
            };
            let health_fraction = entry
                .health
                .map(|(current, maximum)| f32::from(current) / f32::from(maximum.max(1)));
            TeamStripRow {
                name: entry.display_name.clone(),
                health_fraction: if matches!(entry.status, CachedRosterStatus::Defeated) {
                    Some(0.0)
                } else {
                    health_fraction
                },
                status,
                local: local_player == Some(*player),
            }
        })
        .collect()
}

/// Writes retained HUD text and visibility only when they differ, so an unchanged label is not
/// marked changed (and re-laid out) every frame. Empty labels are always hidden.
fn set_retained_text(
    text: &mut Mut<Text>,
    visibility: &mut Mut<Visibility>,
    label: String,
    shown: bool,
) {
    let next_visibility = if shown && !label.is_empty() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    text.set_if_neq(Text(label));
    visibility.set_if_neq(next_visibility);
}

/// The shared deadline clock, presentable only when the clock, match envelope, and concrete
/// mode state carry the same match ID.
fn match_deadline_tick(
    state: Option<MatchState>,
    clock: Option<&brawler::matchplay::MatchClock>,
) -> Option<u64> {
    let state = state?;
    let clock = clock?;
    (clock.match_id == state.match_id).then_some(clock.completed_tick)
}

pub(crate) fn build_mode_score_view(
    state: Option<(&MatchState, Option<&brawler::matchplay::TakedownState>)>,
    turf_hold: Option<&brawler::matchplay::TurfHoldState>,
    clock: Option<&brawler::matchplay::MatchClock>,
) -> Option<ModeScoreView> {
    let (state, takedown) = state?;
    if clock.is_none_or(|clock| clock.match_id != state.match_id) {
        return Some(ModeScoreView::Syncing);
    }
    match state.mode_definition_id {
        brawler::map::TAKEDOWN_MODE_DEFINITION => Some(takedown.map_or(
            ModeScoreView::Syncing,
            |takedown| ModeScoreView::Takedown {
                scores: takedown.team_scores,
                target: takedown.target_score,
            },
        )),
        brawler::map::TURF_HOLD_MODE_DEFINITION => Some(
            turf_hold
                .filter(|turf_hold| turf_hold.match_id == state.match_id)
                .map_or(ModeScoreView::Syncing, |turf_hold| {
                    let percent = |progress: u16| {
                        let value = u32::from(progress) * 100
                            / u32::from(turf_hold.target_progress_ticks.max(1));
                        u8::try_from(value.min(100)).unwrap_or(100)
                    };
                    ModeScoreView::TurfHold {
                        progress_percent: [
                            percent(turf_hold.progress_ticks[0]),
                            percent(turf_hold.progress_ticks[1]),
                        ],
                        status: turf_hold.status,
                    }
                }),
        ),
        _ => None,
    }
}

pub(crate) fn mode_score_text(view: ModeScoreView) -> String {
    match view {
        ModeScoreView::Takedown { scores, target } => {
            format!("T1  {}  —  {}  T2\nFIRST TO {target}", scores[0], scores[1])
        }
        ModeScoreView::TurfHold {
            progress_percent,
            status,
        } => {
            let status = match status {
                brawler::matchplay::TurfHoldStatus::Empty => "EMPTY".to_string(),
                brawler::matchplay::TurfHoldStatus::Contested => "CONTESTED".to_string(),
                brawler::matchplay::TurfHoldStatus::Controlled { team } => {
                    format!("T{} CONTROL", team.0 + 1)
                }
            };
            format!(
                "T1  {}%  —  {}%  T2\n{status}",
                progress_percent[0], progress_percent[1]
            )
        }
        ModeScoreView::VaultBreak {
            health,
            maximum,
            local_team,
        } => {
            let percent =
                |team: usize| u32::from(health[team]) * 100 / u32::from(maximum[team].max(1));
            let labels = local_team.map_or(["T1", "T2"], |team| {
                if team.0 == 0 {
                    ["DEFEND T1", "ATTACK T2"]
                } else {
                    ["ATTACK T1", "DEFEND T2"]
                }
            });
            format!(
                "{}  {}/{} ({}%)\n{}  {}/{} ({}%)",
                labels[0],
                health[0],
                maximum[0],
                percent(0),
                labels[1],
                health[1],
                maximum[1],
                percent(1),
            )
        }
        ModeScoreView::Zombie {
            live_zombies,
            last_brawler_standing,
            local_boost_stacks,
        } => {
            let warning = if last_brawler_standing {
                "\nLAST BRAWLER STANDING"
            } else {
                ""
            };
            format!(
                "ZOMBIES  {live_zombies}\nBOOSTS  {local_boost_stacks} / +{}%{warning}",
                u16::from(local_boost_stacks) * 5
            )
        }
        ModeScoreView::ShardRush {
            shards,
            target,
            hold,
        } => {
            let hold = hold.map_or_else(
                || format!("HOLD {target} TO WIN"),
                |(team, seconds)| format!("T{} HOLDING  {seconds}s", team.0 + 1),
            );
            format!("T1  {}  —  {}  T2\n{hold}", shards[0], shards[1])
        }
        ModeScoreView::LastStand {
            alive,
            fighters,
            zone,
            local_place,
            local_boost_stacks,
        } => {
            let status = local_place.map_or_else(
                || last_stand_zone_label(zone),
                |place| format!("ELIMINATED #{place}"),
            );
            format!(
                "ALIVE  {alive}/{fighters}  BOOSTS +{}%\n{status}",
                u16::from(local_boost_stacks) * 5
            )
        }
        ModeScoreView::Syncing => "SYNCING OBJECTIVE".to_string(),
    }
}

pub(crate) fn last_stand_zone_label(zone: LastStandZone) -> String {
    match zone {
        LastStandZone::ClosesIn(seconds) => format!("ZONE CLOSES  {seconds}"),
        LastStandZone::Closing(seconds) => format!("ZONE CLOSING  {seconds}"),
        LastStandZone::Final => "FINAL ZONE".to_string(),
    }
}

/// Last Stand plate facts: standing count, the ring's next deadline, and the local place.
pub(crate) fn build_last_stand_score_view(
    state: &MatchState,
    last_stand: Option<&brawler::matchplay::LastStandState>,
    clock: Option<&brawler::matchplay::MatchClock>,
    local_team: Option<TeamId>,
    local_boost_stacks: u8,
) -> ModeScoreView {
    let (Some(last_stand), Some(clock)) = (last_stand, clock) else {
        return ModeScoreView::Syncing;
    };
    if last_stand.match_id != state.match_id || clock.match_id != state.match_id {
        return ModeScoreView::Syncing;
    }
    let second = brawler::timing::simulation_ticks_from_seconds(1).unwrap_or(1);
    let seconds_until = |tick: u64| {
        tick.saturating_sub(clock.completed_tick)
            .div_ceil(second)
            .max(1)
    };
    let boundary = last_stand.boundary;
    let zone = if boundary.is_final() {
        LastStandZone::Final
    } else if clock.completed_tick < boundary.shrink_starts_at_tick {
        LastStandZone::ClosesIn(seconds_until(boundary.shrink_starts_at_tick))
    } else {
        LastStandZone::Closing(seconds_until(boundary.shrink_ends_at_tick))
    };
    ModeScoreView::LastStand {
        alive: last_stand.alive,
        fighters: last_stand.fighters,
        zone,
        local_place: local_team
            .and_then(|team| last_stand.placements.get(usize::from(team.0)).copied())
            .filter(|place| *place != 0),
        local_boost_stacks,
    }
}

fn final_last_stand_line(
    state: &MatchState,
    last_stand: Option<&brawler::matchplay::LastStandState>,
    local_team: Option<TeamId>,
) -> String {
    last_stand
        .filter(|last_stand| last_stand.match_id == state.match_id)
        .map_or_else(
            || "SYNCING OBJECTIVE".to_string(),
            |last_stand| {
                let place = local_team
                    .and_then(|team| last_stand.placements.get(usize::from(team.0)).copied())
                    .filter(|place| *place != 0);
                place.map_or_else(
                    || format!("FINAL  {} STANDING", last_stand.alive),
                    |place| format!("FINAL  YOU PLACED #{place} OF {}", last_stand.fighters),
                )
            },
        )
}

/// Shard Rush plate facts: held totals plus the holding team's remaining whole seconds.
pub(crate) fn build_shard_rush_score_view(
    state: &MatchState,
    shard_rush: Option<&brawler::matchplay::ShardRushState>,
    clock: Option<&brawler::matchplay::MatchClock>,
) -> ModeScoreView {
    let (Some(shard_rush), Some(clock)) = (shard_rush, clock) else {
        return ModeScoreView::Syncing;
    };
    if shard_rush.match_id != state.match_id || clock.match_id != state.match_id {
        return ModeScoreView::Syncing;
    }
    ModeScoreView::ShardRush {
        shards: shard_rush.team_shards,
        target: shard_rush.target_shards,
        hold: shard_rush.countdown.map(|countdown| {
            (
                countdown.team,
                countdown
                    .ends_at_tick
                    .saturating_sub(clock.completed_tick)
                    .div_ceil(brawler::timing::simulation_ticks_from_seconds(1).unwrap_or(1))
                    .max(1),
            )
        }),
    }
}

fn final_shard_rush_line(
    state: &MatchState,
    shard_rush: Option<&brawler::matchplay::ShardRushState>,
) -> String {
    shard_rush
        .filter(|shard_rush| shard_rush.match_id == state.match_id)
        .map_or_else(
            || "SYNCING OBJECTIVE".to_string(),
            |shard_rush| {
                format!(
                    "FINAL  T1 {} SHARDS  T2 {} SHARDS",
                    shard_rush.team_shards[0], shard_rush.team_shards[1]
                )
            },
        )
}

fn build_vault_break_score_view<'a>(
    state: &MatchState,
    vault_break: Option<&brawler::matchplay::VaultBreakState>,
    safes: impl Iterator<
        Item = (
            &'a brawler::matchplay::VaultBreakSafe,
            &'a brawler::combat::CurrentHealth,
            &'a brawler::map::DamageableMaximumHealth,
        ),
    >,
    local_team: Option<TeamId>,
    clock: Option<&brawler::matchplay::MatchClock>,
) -> ModeScoreView {
    if clock.is_none_or(|clock| clock.match_id != state.match_id)
        || vault_break.is_none_or(|vault_break| vault_break.match_id != state.match_id)
    {
        return ModeScoreView::Syncing;
    }
    let mut health = [0; 2];
    let mut maximum = [0; 2];
    let mut seen = [false; 2];
    for (safe, current, max) in safes {
        if safe.match_id != state.match_id || safe.defending_team.0 > 1 {
            continue;
        }
        let index = usize::from(safe.defending_team.0);
        if seen[index] {
            return ModeScoreView::Syncing;
        }
        seen[index] = true;
        health[index] = current.0;
        maximum[index] = max.0;
    }
    if seen.into_iter().all(|value| value) {
        ModeScoreView::VaultBreak {
            health,
            maximum,
            local_team,
        }
    } else {
        ModeScoreView::Syncing
    }
}

fn format_match_time(remaining_ticks: u64) -> String {
    let total_seconds = remaining_ticks.div_ceil(60);
    format!("{}:{:02}", total_seconds / 60, total_seconds % 60)
}

fn sync_roster_and_collect_phase_facts<'a>(
    state: Option<MatchState>,
    local_player: Option<u64>,
    participants: impl Iterator<Item = &'a brawler::matchplay::PublicParticipantState>,
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
    for public in participants {
        let player = public.player_id;
        let team = public.team_id;
        let participant = public.participant;
        if participant.match_id != state.match_id {
            continue;
        }
        facts.participant_count += 1;
        facts.ready_count += usize::from(participant.ready && public.selected);
        facts.restart_ready_count += usize::from(participant.restart_ready);
        if local_player == Some(player.0) {
            facts.local_ready = participant.ready;
            facts.local_restart_ready = participant.restart_ready;
            facts.local_selected = public.selected;
        }
        roster.entries.insert(
            player.0,
            CachedRosterEntry {
                team,
                display_name: public.display_name.clone(),
                status: match public.status {
                    brawler::matchplay::PublicParticipantStatus::Alive => CachedRosterStatus::Alive,
                    brawler::matchplay::PublicParticipantStatus::Ready => CachedRosterStatus::Ready,
                    brawler::matchplay::PublicParticipantStatus::RestartReady => {
                        CachedRosterStatus::RestartReady
                    }
                    brawler::matchplay::PublicParticipantStatus::Defeated => {
                        CachedRosterStatus::Defeated
                    }
                    brawler::matchplay::PublicParticipantStatus::Respawning { respawn_at_tick } => {
                        CachedRosterStatus::Respawning(respawn_at_tick)
                    }
                    brawler::matchplay::PublicParticipantStatus::Protected { expires_at_tick } => {
                        CachedRosterStatus::Protected(expires_at_tick)
                    }
                },
                connected: true,
                held_until: match public.control {
                    brawler::matchplay::ParticipantControl::Player => None,
                    brawler::matchplay::ParticipantControl::HeldByBot { expires_at_tick } => {
                        Some(expires_at_tick)
                    }
                },
                health: None,
                boost_stacks: 0,
            },
        );
    }
    facts
}

/// The mode-specific final objective line for the completed overlay, or a syncing label.
fn final_objective_line(
    state: Option<(&MatchState, Option<&brawler::matchplay::TakedownState>)>,
    turf_hold: Option<&brawler::matchplay::TurfHoldState>,
) -> Option<String> {
    let (state, takedown) = state?;
    Some(match state.mode_definition_id {
        brawler::map::TAKEDOWN_MODE_DEFINITION => takedown.map_or_else(
            || "FINAL  SYNCING".to_string(),
            |takedown| {
                format!(
                    "FINAL  {} - {}  |  MARGIN {}",
                    takedown.team_scores[0],
                    takedown.team_scores[1],
                    takedown.team_scores[0].abs_diff(takedown.team_scores[1])
                )
            },
        ),
        brawler::map::TURF_HOLD_MODE_DEFINITION => turf_hold
            .filter(|turf_hold| turf_hold.match_id == state.match_id)
            .map_or_else(
                || "SYNCING OBJECTIVE".to_string(),
                |turf_hold| {
                    let percent = |progress: u16| {
                        u32::from(progress) * 100 / u32::from(turf_hold.target_progress_ticks)
                    };
                    format!(
                        "FINAL  T1 {}%  T2 {}%",
                        percent(turf_hold.progress_ticks[0]),
                        percent(turf_hold.progress_ticks[1]),
                    )
                },
            ),
        _ => "UNKNOWN MODE".to_string(),
    })
}

fn final_vault_break_objective_line<'a>(
    state: &MatchState,
    vault_break: Option<&brawler::matchplay::VaultBreakState>,
    safes: impl Iterator<
        Item = (
            &'a brawler::matchplay::VaultBreakSafe,
            &'a brawler::combat::CurrentHealth,
            &'a brawler::map::DamageableMaximumHealth,
        ),
    >,
) -> Option<String> {
    let vault_break = vault_break.filter(|vault_break| vault_break.match_id == state.match_id)?;
    let mut health = [0; 2];
    let mut maximum = [0; 2];
    let mut seen = [false; 2];
    for (safe, current, max) in safes {
        if safe.match_id == state.match_id && safe.defending_team.0 <= 1 {
            let index = usize::from(safe.defending_team.0);
            if seen[index] {
                return Some("SYNCING OBJECTIVE".to_string());
            }
            seen[index] = true;
            health[index] = current.0;
            maximum[index] = max.0;
        }
    }
    if !seen.into_iter().all(|value| value) {
        return Some("SYNCING OBJECTIVE".to_string());
    }
    let cause = match vault_break.completion {
        Some(brawler::matchplay::VaultBreakCompletion::SafeDestroyed {
            destroyed_teams: [true, true],
        }) => "BOTH IDOLS DESTROYED — DRAW",
        Some(brawler::matchplay::VaultBreakCompletion::SafeDestroyed { .. }) => "IDOL DESTROYED",
        Some(brawler::matchplay::VaultBreakCompletion::Timeout { .. }) => "TIME EXPIRED",
        None => "VAULT BREAK COMPLETE",
    };
    Some(format!(
        "{cause}\nFINAL  T1 {}/{}  T2 {}/{}",
        health[0], maximum[0], health[1], maximum[1]
    ))
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
                brawler::matchplay::MatchResult::TeamVictory { team } => {
                    format!("TEAM {} WINS", team.0 + 1)
                }
                brawler::matchplay::MatchResult::Draw => "DRAW".to_string(),
                brawler::matchplay::MatchResult::Cleared => "HORDE CLEARED".to_string(),
                brawler::matchplay::MatchResult::Overrun => "ALL PLAYERS DOWN".to_string(),
                brawler::matchplay::MatchResult::Forfeit {
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

#[cfg(test)]
fn readiness_status(
    join: Option<&ClientJoinPhase>,
    map: &brawler::map::ClientMapReadiness,
    assets: &assets::ClientAssetReadiness,
    playable: bool,
) -> String {
    if let brawler::map::ClientMapReadiness::Invalid(reason) = map {
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
            brawler::map::ClientMapReadiness::WaitingForSnapshot,
            _,
        ) => "WAITING FOR AUTHORITATIVE MAP".to_string(),
        _ => "PREPARING SANDBOX".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BRL-dwqx: a seat the server publishes as held shows BOT in the Zombie team strip, which
    /// stays distinct from OFF (projection missing this frame).
    #[test]
    fn team_strip_marks_a_bot_held_seat() {
        let entry = |held_until, connected| CachedRosterEntry {
            team: TeamId(0),
            display_name: "Ana".to_string(),
            status: CachedRosterStatus::Alive,
            connected,
            held_until,
            health: Some((50, 100)),
            boost_stacks: 1,
        };
        let roster = MatchRosterPresentation {
            match_id: None,
            entries: [
                (1, entry(None, true)),
                (2, entry(Some(900), true)),
                (3, entry(None, false)),
            ]
            .into_iter()
            .collect(),
        };
        let statuses: Vec<_> = team_strip_rows(&roster, Some(1), Some(TeamId(0)), Some(0))
            .into_iter()
            .map(|row| row.status)
            .collect();
        assert_eq!(statuses, ["+1", "BOT", "OFF"]);
    }

    #[test]
    fn retained_hud_text_queries_are_disjoint_in_runtime_composition() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<MatchRosterPresentation>()
            .init_resource::<ClientVaultBreakReadiness>()
            .init_resource::<PlateState>()
            .init_resource::<ScoreboardState>()
            .add_systems(Update, update_readiness_hud);
        app.world_mut()
            .spawn((CountdownHudText, Text::new(""), Visibility::Hidden));
        app.world_mut()
            .spawn((MatchPhaseOverlayText, Text::new(""), Visibility::Hidden));

        app.update();
    }

    #[test]
    fn retained_hud_text_changes_only_when_its_content_changes() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<MatchRosterPresentation>()
            .init_resource::<ClientVaultBreakReadiness>()
            .init_resource::<PlateState>()
            .init_resource::<ScoreboardState>()
            .add_systems(Update, update_readiness_hud);
        let match_id = brawler::matchplay::MatchId(7);
        let root = app
            .world_mut()
            .spawn((
                MatchRoot,
                MatchState {
                    match_id,
                    mode_definition_id: brawler::map::TAKEDOWN_MODE_DEFINITION,
                    phase: MatchPhase::Active { ends_at_tick: 600 },
                    rules_revision: 1,
                },
                brawler::matchplay::TakedownState {
                    team_scores: [2, 1],
                    target_score: 10,
                    ..default()
                },
                brawler::matchplay::MatchClock {
                    match_id,
                    completed_tick: 10,
                },
            ))
            .id();
        let changed_ticks = |app: &App| {
            (
                app.world().resource_ref::<PlateState>().last_changed(),
                app.world().resource_ref::<ScoreboardState>().last_changed(),
            )
        };

        app.update();
        let plate = app.world().resource::<PlateState>();
        assert!(plate.shown);
        assert_eq!(plate.timer, "0:10");
        assert_eq!(
            (
                plate.content.left.value.as_str(),
                plate.content.subline.as_str()
            ),
            ("2", "FIRST TO 10")
        );
        let first = changed_ticks(&app);
        app.update();
        assert_eq!(
            changed_ticks(&app),
            first,
            "unchanged HUD content was rewritten"
        );

        app.world_mut()
            .get_mut::<brawler::matchplay::MatchClock>(root)
            .unwrap()
            .completed_tick = 70;
        app.update();
        assert_eq!(app.world().resource::<PlateState>().timer, "0:09");
        let after = changed_ticks(&app);
        assert_ne!(after.0, first.0, "a new timer value must be written");
        assert_eq!(after.1, first.1);
    }

    /// Every string literal in the in-match HUD and overhead sources must render in Lilita One.
    #[test]
    fn hud_and_overhead_string_templates_use_only_display_font_glyphs() {
        let sources = [
            ("packages/brawler-client/src/hud.rs", include_str!("hud.rs")),
            (
                "packages/brawler-client/src/presentation.rs",
                include_str!("presentation.rs"),
            ),
            (
                "packages/brawler-client/src/hud/plate.rs",
                include_str!("hud/plate.rs"),
            ),
            (
                "packages/brawler-client/src/hud/scoreboard.rs",
                include_str!("hud/scoreboard.rs"),
            ),
            (
                "packages/brawler-client/src/hud/kill_feed.rs",
                include_str!("hud/kill_feed.rs"),
            ),
            (
                "packages/brawler-client/src/hud/controls.rs",
                include_str!("hud/controls.rs"),
            ),
            (
                "packages/brawler-client/src/presentation_3d/combat/fighter_ui.rs",
                include_str!("presentation_3d/combat/fighter_ui.rs"),
            ),
        ];
        for (path, source) in sources {
            let missing = string_literal_characters(source)
                .filter(|character| {
                    !character.is_control() && !ui::theme::display_font_has_glyph(*character)
                })
                .collect::<std::collections::BTreeSet<_>>();
            assert!(missing.is_empty(), "{path} uses missing glyphs {missing:?}");
        }
        assert_eq!(
            string_literal_characters("let c = '\"'; // \"x\"\nf(\"T1 \\\"—\\\" 2\", 'a)")
                .collect::<String>(),
            "T1 \"—\" 2"
        );
        assert!(ui::theme::display_font_has_glyph('A'));
        assert!(!ui::theme::display_font_has_glyph('\u{25AF}'));
    }

    /// Characters inside `"..."` literals, skipping comments, char literals, and lifetimes. The
    /// scanned sources contain no raw strings.
    fn string_literal_characters(source: &str) -> impl Iterator<Item = char> + '_ {
        assert!(!source.contains("r#\u{22}"), "raw strings are not scanned");
        let mut characters = source.chars().peekable();
        let mut literal = Vec::new();
        while let Some(character) = characters.next() {
            match character {
                '/' if characters.peek() == Some(&'/') => {
                    characters
                        .by_ref()
                        .take_while(|next| *next != '\n')
                        .for_each(drop);
                }
                '\'' => {
                    let mut lookahead = characters.clone();
                    let first = lookahead.next();
                    if first == Some('\\') || lookahead.next() == Some('\'') {
                        characters
                            .by_ref()
                            .skip(1)
                            .take_while(|next| *next != '\'')
                            .for_each(drop);
                    }
                }
                '"' => {
                    while let Some(next) = characters.next() {
                        match next {
                            '"' => break,
                            // Keep the escaped character itself (`\"` renders `"`).
                            '\\' => literal.extend(characters.next()),
                            other => literal.push(other),
                        }
                    }
                }
                _ => {}
            }
        }
        literal.into_iter()
    }

    #[test]
    fn invalid_map_has_priority_over_loading_state() {
        assert_eq!(
            readiness_status(
                None,
                &brawler::map::ClientMapReadiness::Invalid("bad schema".to_string()),
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
                &brawler::map::ClientMapReadiness::Ready,
                &assets::ClientAssetReadiness::Degraded(vec!["audio.fire"]),
                true,
            ),
            "READY | FALLBACKS ACTIVE: audio.fire"
        );
    }

    #[test]
    fn countdown_label_uses_authoritative_deadline_and_hides_outside_countdown() {
        let mut state = MatchState {
            match_id: brawler::matchplay::MatchId(1),
            mode_definition_id: brawler::map::TAKEDOWN_MODE_DEFINITION,
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
        let takedown_state = brawler::matchplay::TakedownState {
            team_scores: [3, 2],
            target_score: 5,
            ..default()
        };
        let mut state = MatchState {
            match_id: brawler::matchplay::MatchId(7),
            mode_definition_id: brawler::map::TAKEDOWN_MODE_DEFINITION,
            phase: MatchPhase::Active { ends_at_tick: 600 },
            rules_revision: 1,
        };
        let clock = brawler::matchplay::MatchClock {
            match_id: state.match_id,
            completed_tick: 10,
        };
        assert_eq!(
            build_mode_score_view(Some((&state, Some(&takedown_state))), None, Some(&clock)),
            Some(ModeScoreView::Takedown {
                scores: [3, 2],
                target: 5,
            })
        );
        assert_eq!(
            mode_score_text(ModeScoreView::Takedown {
                scores: [3, 2],
                target: 5,
            }),
            "T1  3  —  2  T2\nFIRST TO 5"
        );

        state.mode_definition_id = brawler::map::TURF_HOLD_MODE_DEFINITION;
        let turf_hold = brawler::matchplay::TurfHoldState {
            match_id: state.match_id,
            zone_anchor_id: brawler::map::ModeAnchorId(1),
            occupants: [1, 0],
            status: brawler::matchplay::TurfHoldStatus::Controlled { team: TeamId(0) },
            progress_ticks: [30, 15],
            target_progress_ticks: 60,
            next_evaluation_tick: 11,
        };
        assert_eq!(
            build_mode_score_view(Some((&state, None)), Some(&turf_hold), Some(&clock)),
            Some(ModeScoreView::TurfHold {
                progress_percent: [50, 25],
                status: brawler::matchplay::TurfHoldStatus::Controlled { team: TeamId(0) },
            })
        );
        let stale_clock = brawler::matchplay::MatchClock {
            match_id: brawler::matchplay::MatchId(8),
            ..clock
        };
        assert_eq!(
            build_mode_score_view(Some((&state, None)), Some(&turf_hold), Some(&stale_clock)),
            Some(ModeScoreView::Syncing)
        );
    }

    #[test]
    fn phase_overlay_explains_ready_and_completed_restart_states() {
        let mut state = MatchState {
            match_id: brawler::matchplay::MatchId(1),
            mode_definition_id: brawler::map::TAKEDOWN_MODE_DEFINITION,
            phase: MatchPhase::Waiting,
            rules_revision: 1,
        };
        let takedown_state = brawler::matchplay::TakedownState {
            team_scores: [3, 1],
            target_score: 10,
            ..default()
        };
        assert_eq!(
            phase_overlay_label(Some(state), None, Some(0), 4, 0, 0, false, false, false),
            None
        );
        let waiting = phase_overlay_label(
            Some(state),
            final_objective_line(Some((&state, Some(&takedown_state))), None).as_deref(),
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
            result: brawler::matchplay::MatchResult::Forfeit {
                winner: TeamId(0),
                departed_team: TeamId(1),
            },
        };
        let locked = phase_overlay_label(
            Some(state),
            final_objective_line(Some((&state, Some(&takedown_state))), None).as_deref(),
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
    fn turf_hold_completed_overlay_shows_final_percentages() {
        let state = MatchState {
            match_id: brawler::matchplay::MatchId(1),
            mode_definition_id: brawler::map::TURF_HOLD_MODE_DEFINITION,
            phase: MatchPhase::Completed {
                completed_at_tick: 100,
                restart_unlocked_at_tick: 160,
                result: brawler::matchplay::MatchResult::TeamVictory { team: TeamId(0) },
            },
            rules_revision: 1,
        };
        let turf_hold = brawler::matchplay::TurfHoldState {
            match_id: brawler::matchplay::MatchId(1),
            zone_anchor_id: brawler::map::ModeAnchorId(1),
            occupants: [1, 0],
            status: brawler::matchplay::TurfHoldStatus::Controlled { team: TeamId(0) },
            progress_ticks: [15, 3],
            target_progress_ticks: 30,
            next_evaluation_tick: 100,
        };
        let label = phase_overlay_label(
            Some(state),
            final_objective_line(Some((&state, None)), Some(&turf_hold)).as_deref(),
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

        let mismatched = brawler::matchplay::TurfHoldState {
            match_id: brawler::matchplay::MatchId(9),
            ..turf_hold
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

    #[test]
    fn vault_break_readiness_requires_matching_root_map_generation_and_two_safes() {
        let resolved = brawler::map::MapContentCatalog::embedded()
            .unwrap()
            .resolve_preset(
                brawler::map::FEATURE_YARD_VAULT_BREAK_PRESET,
                brawler::map::MapInstanceId(9),
            )
            .unwrap();
        let generation = brawler::map::MapDynamicGeneration {
            map_instance_id: brawler::map::MapInstanceId(9),
            generation: 1,
        };
        let match_id = brawler::matchplay::MatchId(7);
        let safes = [
            brawler::matchplay::VaultBreakSafeIdentity {
                anchor_id: brawler::map::ModeAnchorId(1),
                defending_team: TeamId(0),
            },
            brawler::matchplay::VaultBreakSafeIdentity {
                anchor_id: brawler::map::ModeAnchorId(2),
                defending_team: TeamId(1),
            },
        ];
        let mut app = App::new();
        app.init_resource::<ClientVaultBreakReadiness>()
            .add_systems(Update, converge_vault_break_readiness);
        app.world_mut().spawn((
            MatchRoot,
            MatchState {
                match_id,
                mode_definition_id: brawler::map::VAULT_BREAK_MODE_DEFINITION,
                phase: MatchPhase::Waiting,
                rules_revision: 1,
            },
            brawler::matchplay::VaultBreakState {
                match_id,
                rules_revision: brawler::matchplay::VAULT_BREAK_RULES_REVISION,
                generation,
                critical_health_percent: 25,
                safes,
                completion: None,
            },
        ));
        app.world_mut().spawn((
            brawler::map::MapRoot,
            resolved.snapshot,
            brawler::map::MapDynamicState {
                map_instance_id: generation.map_instance_id,
                generation: generation.generation,
                revision: 0,
                terminal_states: Vec::new(),
            },
        ));
        app.update();
        assert_eq!(
            *app.world().resource::<ClientVaultBreakReadiness>(),
            ClientVaultBreakReadiness::Waiting
        );

        for safe in safes {
            app.world_mut().spawn((
                brawler::matchplay::VaultBreakSafe {
                    match_id,
                    anchor_id: safe.anchor_id,
                    defending_team: safe.defending_team,
                    generation,
                },
                brawler::map::DamageableTargetIdentity::VaultBreakSafe {
                    match_id,
                    anchor_id: safe.anchor_id,
                    defending_team: safe.defending_team,
                },
                brawler::combat::CurrentHealth(2_000),
                brawler::map::DamageableMaximumHealth(2_000),
                brawler::map::DamageableLifeState::Live,
            ));
        }
        app.update();
        assert_eq!(
            *app.world().resource::<ClientVaultBreakReadiness>(),
            ClientVaultBreakReadiness::Ready
        );

        let entity = {
            let world = app.world_mut();
            let mut query =
                world.query_filtered::<Entity, With<brawler::matchplay::VaultBreakSafe>>();
            query.iter(world).next().unwrap()
        };
        app.world_mut()
            .entity_mut(entity)
            .insert(brawler::combat::CurrentHealth(2_001));
        app.update();
        assert!(matches!(
            *app.world().resource::<ClientVaultBreakReadiness>(),
            ClientVaultBreakReadiness::Invalid(_)
        ));
    }
}
