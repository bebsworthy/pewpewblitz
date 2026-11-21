use bevy::prelude::*;

use crate::{
    audio::{
        registry::{AudioProducerRegistration, AudioProducerRegistrationAppExt},
        request::{AudioCueKey, AudioRequest, AudioRequestOrder, cue_keys},
    },
    hud::ClientVaultBreakReadiness,
};
use brawler::{
    combat::TeamId,
    map::TURF_HOLD_MODE_DEFINITION,
    matchplay::{
        MatchId, MatchPhase, MatchResult, MatchRoot, MatchState, ReceivedVaultBreakObjectiveCue,
        TakedownState, TurfHoldState, TurfHoldStatus, VaultBreakObjectiveCueKind,
    },
    protocol::Fighter,
};
use lightyear::prelude::Controlled;

use super::AudioProducerSequence;

const VAULT_BREAK_RANK: u16 = 20;
const MATCH_RANK: u16 = 60;
const TURF_HOLD_RANK: u16 = 70;
const VAULT_BREAK_KEYS: &[AudioCueKey] = &[
    cue_keys::OBJECTIVE_HIT,
    cue_keys::OBJECTIVE_CRITICAL,
    cue_keys::OBJECTIVE_DESTROYED,
];
const MATCH_KEYS: &[AudioCueKey] = &[
    cue_keys::MATCH_START,
    cue_keys::MATCH_SCORE,
    cue_keys::MATCH_VICTORY,
    cue_keys::MATCH_DEFEAT,
    cue_keys::MATCH_DRAW,
];
const TURF_HOLD_KEYS: &[AudioCueKey] = &[
    cue_keys::ZONE_CAPTURED,
    cue_keys::ZONE_LOST,
    cue_keys::ZONE_PROGRESS,
];

#[derive(Resource, Default)]
pub(in crate::audio::producers) struct VaultBreakAudioMemory {
    last_objective_hit_tick: Option<u64>,
}

#[derive(Resource, Default)]
pub(in crate::audio::producers) struct MatchAudioMemory {
    last: Option<(MatchId, MatchPhase, Option<[u16; 2]>)>,
}

/// Per-match deduplication memory for objective cues; never gameplay truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TurfHoldAudioSnapshot {
    match_id: MatchId,
    status: TurfHoldStatus,
    progress_ticks: [u16; 2],
    target_progress_ticks: u16,
    completed: bool,
}

#[derive(Resource, Default)]
pub(in crate::audio::producers) struct TurfHoldAudioMemory {
    last: Option<TurfHoldAudioSnapshot>,
}

pub(super) fn register_vault_break(app: &mut App) {
    app.init_resource::<VaultBreakAudioMemory>()
        .try_register_audio_producer(AudioProducerRegistration {
            id: "vault-break",
            rank: VAULT_BREAK_RANK,
            cue_keys: VAULT_BREAK_KEYS,
        })
        .expect("Vault Break audio producer registration must be valid");
}

pub(super) fn register_common(app: &mut App) {
    app.init_resource::<MatchAudioMemory>()
        .try_register_audio_producer(AudioProducerRegistration {
            id: "match",
            rank: MATCH_RANK,
            cue_keys: MATCH_KEYS,
        })
        .expect("common match audio producer registration must be valid");
}

pub(super) fn register_turf_hold(app: &mut App) {
    app.init_resource::<TurfHoldAudioMemory>()
        .try_register_audio_producer(AudioProducerRegistration {
            id: "turf-hold",
            rank: TURF_HOLD_RANK,
            cue_keys: TURF_HOLD_KEYS,
        })
        .expect("Turf Hold audio producer registration must be valid");
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "ClientVaultBreakReadiness is a Bevy system resource parameter"
)]
pub(super) fn produce_vault_break_audio_requests(
    mut cues: MessageReader<ReceivedVaultBreakObjectiveCue>,
    readiness: Res<ClientVaultBreakReadiness>,
    mut memory: ResMut<VaultBreakAudioMemory>,
    mut requests: MessageWriter<AudioRequest>,
    mut sequence: Local<AudioProducerSequence>,
) {
    let ready = matches!(*readiness, ClientVaultBreakReadiness::Ready);
    for ReceivedVaultBreakObjectiveCue(cue) in cues.read() {
        if let Some(request) = vault_break_audio_request(
            cue,
            ready,
            &mut memory.last_objective_hit_tick,
            sequence.next(VAULT_BREAK_RANK),
        ) {
            requests.write(request);
        }
    }
}

fn vault_break_audio_request(
    cue: &brawler::matchplay::VaultBreakObjectiveCue,
    ready: bool,
    last_objective_hit_tick: &mut Option<u64>,
    order: AudioRequestOrder,
) -> Option<AudioRequest> {
    if !ready {
        return None;
    }
    let cue_key = match cue.kind {
        VaultBreakObjectiveCueKind::Damaged => {
            if last_objective_hit_tick.is_some_and(|last| cue.tick < last.saturating_add(6)) {
                return None;
            }
            *last_objective_hit_tick = Some(cue.tick);
            cue_keys::OBJECTIVE_HIT
        }
        VaultBreakObjectiveCueKind::Critical => cue_keys::OBJECTIVE_CRITICAL,
        VaultBreakObjectiveCueKind::Destroyed => cue_keys::OBJECTIVE_DESTROYED,
    };
    Some(AudioRequest::for_occurrence(cue_key, cue.event_id.0, order))
}

#[allow(
    clippy::type_complexity,
    reason = "the query declares common match and optional Takedown transition inputs"
)]
pub(super) fn produce_match_audio_requests(
    matches: Query<
        (&MatchState, Option<&TakedownState>),
        (
            With<MatchRoot>,
            Or<(Changed<MatchState>, Changed<TakedownState>)>,
        ),
    >,
    local_team: Query<&TeamId, (With<Fighter>, With<Controlled>)>,
    mut memory: ResMut<MatchAudioMemory>,
    mut requests: MessageWriter<AudioRequest>,
    mut sequence: Local<AudioProducerSequence>,
) {
    let Some((current, takedown)) = matches.iter().next() else {
        return;
    };
    let takedown_scores = takedown.map(|takedown| takedown.team_scores);
    let previous = memory.last;
    memory.last = Some((current.match_id, current.phase, takedown_scores));
    let local_team = local_team.iter().next().copied();
    if let Some(cue_key) = match_audio_cue(current, takedown_scores, previous, local_team) {
        requests.write(AudioRequest::once(cue_key, sequence.next(MATCH_RANK)));
    }
}

fn match_audio_cue(
    current: &MatchState,
    takedown_scores: Option<[u16; 2]>,
    previous: Option<(MatchId, MatchPhase, Option<[u16; 2]>)>,
    local_team: Option<TeamId>,
) -> Option<AudioCueKey> {
    let turf_hold_mode = current.mode_definition_id == TURF_HOLD_MODE_DEFINITION;
    if let MatchPhase::Completed { result, .. } = current.phase
        && !previous.is_some_and(|(id, phase, _)| {
            id == current.match_id && matches!(phase, MatchPhase::Completed { .. })
        })
    {
        Some(result_sting(result, local_team))
    } else if !turf_hold_mode
        && previous.is_some_and(|(id, _, scores)| {
            id == current.match_id && scores.is_some_and(|scores| Some(scores) != takedown_scores)
        })
    {
        Some(cue_keys::MATCH_SCORE)
    } else if previous.is_none_or(|(id, phase, _)| id != current.match_id || phase != current.phase)
        && matches!(
            current.phase,
            MatchPhase::Countdown { .. } | MatchPhase::Active { .. }
        )
    {
        Some(cue_keys::MATCH_START)
    } else {
        None
    }
}

/// The completion sting as heard by the controlled fighter's team. Without a controlled fighter
/// (observer) a decided match plays the neutral sting.
fn result_sting(result: MatchResult, local_team: Option<TeamId>) -> AudioCueKey {
    let winner = match result {
        MatchResult::TeamVictory { team } | MatchResult::Forfeit { winner: team, .. } => team,
        MatchResult::Draw => return cue_keys::MATCH_DRAW,
        MatchResult::Cleared => return cue_keys::MATCH_VICTORY,
        MatchResult::Overrun => return cue_keys::MATCH_DEFEAT,
    };
    match local_team {
        Some(team) if team == winner => cue_keys::MATCH_VICTORY,
        Some(_) => cue_keys::MATCH_DEFEAT,
        None => cue_keys::MATCH_DRAW,
    }
}

/// Bounded objective feedback: control gained/lost, contested entry, and 50%/90% thresholds.
/// Match completion remains owned by `produce_match_audio_requests`.
#[allow(
    clippy::type_complexity,
    reason = "the query declares the complete Turf Hold transition view"
)]
pub(super) fn produce_turf_hold_audio_requests(
    zones: Query<
        (&TurfHoldState, &MatchState),
        (
            With<MatchRoot>,
            Or<(Changed<TurfHoldState>, Changed<MatchState>)>,
        ),
    >,
    mut memory: ResMut<TurfHoldAudioMemory>,
    mut requests: MessageWriter<AudioRequest>,
    mut sequence: Local<AudioProducerSequence>,
) {
    let Some((turf_hold, match_state)) = zones.iter().next() else {
        return;
    };
    if turf_hold.match_id != match_state.match_id {
        return;
    }
    let snapshot = TurfHoldAudioSnapshot {
        match_id: turf_hold.match_id,
        status: turf_hold.status,
        progress_ticks: turf_hold.progress_ticks,
        target_progress_ticks: turf_hold.target_progress_ticks,
        completed: matches!(match_state.phase, MatchPhase::Completed { .. }),
    };
    let previous = memory.last.replace(snapshot);
    let Some(previous) = previous.filter(|previous| previous.match_id == snapshot.match_id) else {
        return;
    };
    if let Some(cue_key) = turf_hold_audio_cue(snapshot, previous) {
        requests.write(AudioRequest::once(cue_key, sequence.next(TURF_HOLD_RANK)));
    }
}

fn turf_hold_audio_cue(
    snapshot: TurfHoldAudioSnapshot,
    previous: TurfHoldAudioSnapshot,
) -> Option<AudioCueKey> {
    let percent = |progress: u16| {
        u32::from(progress) * 100 / u32::from(snapshot.target_progress_ticks.max(1))
    };
    let controlled_now = matches!(snapshot.status, TurfHoldStatus::Controlled { .. });
    if snapshot.status == previous.status {
        [0_usize, 1].into_iter().find_map(|team| {
            let crossed = |progress: u16| {
                [50_u32, 90].into_iter().any(|mark| {
                    percent(progress) >= mark && percent(previous.progress_ticks[team]) < mark
                })
            };
            (snapshot.progress_ticks[team] > previous.progress_ticks[team]
                && crossed(snapshot.progress_ticks[team]))
            .then_some(cue_keys::ZONE_PROGRESS)
        })
    } else if controlled_now {
        Some(cue_keys::ZONE_CAPTURED)
    } else {
        Some(cue_keys::ZONE_LOST)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brawler::{
        combat::{AttackId, CombatEventId, TeamId, WorldPoint},
        map::{DamageableTargetIdentity, ModeAnchorId, TAKEDOWN_MODE_DEFINITION},
        matchplay::{MatchResult, VaultBreakObjectiveCue},
    };

    fn vault_break_cue(
        event_id: u64,
        tick: u64,
        kind: VaultBreakObjectiveCueKind,
    ) -> VaultBreakObjectiveCue {
        VaultBreakObjectiveCue {
            event_id: CombatEventId(event_id),
            tick,
            attack_id: AttackId(event_id + 10),
            source_subject: None,
            target: DamageableTargetIdentity::VaultBreakSafe {
                match_id: MatchId(1),
                anchor_id: ModeAnchorId(2),
                defending_team: TeamId(1),
            },
            position: WorldPoint { x: 0.0, y: 0.0 },
            amount: 1,
            health_after: 9,
            maximum_health: 10,
            kind,
        }
    }

    #[test]
    fn vault_break_damage_uses_six_tick_suppression_but_terminal_cues_are_immediate() {
        let mut last_hit = None;
        assert_eq!(
            vault_break_audio_request(
                &vault_break_cue(1, 100, VaultBreakObjectiveCueKind::Damaged),
                true,
                &mut last_hit,
                AudioRequestOrder::new(VAULT_BREAK_RANK, 0),
            ),
            Some(AudioRequest::for_occurrence(
                cue_keys::OBJECTIVE_HIT,
                1,
                AudioRequestOrder::new(VAULT_BREAK_RANK, 0),
            ))
        );
        assert_eq!(
            vault_break_audio_request(
                &vault_break_cue(2, 105, VaultBreakObjectiveCueKind::Damaged),
                true,
                &mut last_hit,
                AudioRequestOrder::new(VAULT_BREAK_RANK, 1),
            ),
            None
        );
        assert_eq!(
            vault_break_audio_request(
                &vault_break_cue(3, 105, VaultBreakObjectiveCueKind::Critical),
                true,
                &mut last_hit,
                AudioRequestOrder::new(VAULT_BREAK_RANK, 2),
            ),
            Some(AudioRequest::for_occurrence(
                cue_keys::OBJECTIVE_CRITICAL,
                3,
                AudioRequestOrder::new(VAULT_BREAK_RANK, 2),
            ))
        );
        assert_eq!(
            vault_break_audio_request(
                &vault_break_cue(4, 105, VaultBreakObjectiveCueKind::Destroyed),
                true,
                &mut last_hit,
                AudioRequestOrder::new(VAULT_BREAK_RANK, 3),
            ),
            Some(AudioRequest::for_occurrence(
                cue_keys::OBJECTIVE_DESTROYED,
                4,
                AudioRequestOrder::new(VAULT_BREAK_RANK, 3),
            ))
        );
        assert_eq!(
            vault_break_audio_request(
                &vault_break_cue(5, 106, VaultBreakObjectiveCueKind::Damaged),
                true,
                &mut last_hit,
                AudioRequestOrder::new(VAULT_BREAK_RANK, 4),
            ),
            Some(AudioRequest::for_occurrence(
                cue_keys::OBJECTIVE_HIT,
                5,
                AudioRequestOrder::new(VAULT_BREAK_RANK, 4),
            ))
        );
    }

    #[test]
    fn completion_sting_is_relative_to_the_controlled_fighters_team() {
        for (result, team, sting) in [
            (
                MatchResult::TeamVictory { team: TeamId(1) },
                Some(TeamId(1)),
                cue_keys::MATCH_VICTORY,
            ),
            (
                MatchResult::TeamVictory { team: TeamId(1) },
                Some(TeamId(2)),
                cue_keys::MATCH_DEFEAT,
            ),
            (
                MatchResult::Forfeit {
                    winner: TeamId(2),
                    departed_team: TeamId(1),
                },
                Some(TeamId(2)),
                cue_keys::MATCH_VICTORY,
            ),
            (
                MatchResult::TeamVictory { team: TeamId(1) },
                None,
                cue_keys::MATCH_DRAW,
            ),
            (MatchResult::Cleared, None, cue_keys::MATCH_VICTORY),
            (
                MatchResult::Overrun,
                Some(TeamId(1)),
                cue_keys::MATCH_DEFEAT,
            ),
        ] {
            assert_eq!(result_sting(result, team), sting, "{result:?}");
        }
    }

    #[test]
    fn common_match_and_turf_hold_transition_selectors_keep_exact_keys() {
        let current = MatchState {
            match_id: MatchId(7),
            mode_definition_id: TAKEDOWN_MODE_DEFINITION,
            phase: MatchPhase::Active { ends_at_tick: 50 },
            rules_revision: 1,
        };
        assert_eq!(
            match_audio_cue(
                &current,
                Some([1, 0]),
                Some((current.match_id, current.phase, Some([0, 0]))),
                Some(TeamId(1)),
            ),
            Some(cue_keys::MATCH_SCORE)
        );
        let completed = MatchState {
            phase: MatchPhase::Completed {
                completed_at_tick: 50,
                restart_unlocked_at_tick: 60,
                result: MatchResult::Draw,
            },
            ..current
        };
        assert_eq!(
            match_audio_cue(
                &completed,
                Some([1, 0]),
                Some((current.match_id, current.phase, Some([1, 0]))),
                Some(TeamId(1)),
            ),
            Some(cue_keys::MATCH_DRAW)
        );
        let countdown = MatchState {
            phase: MatchPhase::Countdown { starts_at_tick: 10 },
            ..current
        };
        assert_eq!(
            match_audio_cue(&countdown, Some([0, 0]), None, None),
            Some(cue_keys::MATCH_START)
        );

        let previous = TurfHoldAudioSnapshot {
            match_id: MatchId(7),
            status: TurfHoldStatus::Empty,
            progress_ticks: [49, 0],
            target_progress_ticks: 100,
            completed: false,
        };
        assert_eq!(
            turf_hold_audio_cue(
                TurfHoldAudioSnapshot {
                    status: TurfHoldStatus::Controlled { team: TeamId(0) },
                    ..previous
                },
                previous,
            ),
            Some(cue_keys::ZONE_CAPTURED)
        );
        assert_eq!(
            turf_hold_audio_cue(
                previous,
                TurfHoldAudioSnapshot {
                    status: TurfHoldStatus::Controlled { team: TeamId(0) },
                    ..previous
                },
            ),
            Some(cue_keys::ZONE_LOST)
        );
        assert_eq!(
            turf_hold_audio_cue(
                TurfHoldAudioSnapshot {
                    progress_ticks: [50, 0],
                    ..previous
                },
                previous,
            ),
            Some(cue_keys::ZONE_PROGRESS)
        );
    }
}
