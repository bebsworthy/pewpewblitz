use bevy::prelude::*;

use crate::{
    client::{
        audio::{
            registry::{AudioProducerRegistration, AudioProducerRegistrationAppExt},
            request::{AudioCueKey, AudioRequest, AudioRequestOrder, cue_keys},
        },
        hud::ClientHeistReadiness,
    },
    map::HOT_ZONE_MODE_DEFINITION,
    matchplay::{
        HeistObjectiveCueKind, HotZoneState, HotZoneStatus, MatchId, MatchPhase, MatchRoot,
        MatchState, ReceivedHeistObjectiveCue, WipeoutState,
    },
};

use super::AudioProducerSequence;

const HEIST_RANK: u16 = 20;
const MATCH_RANK: u16 = 60;
const HOT_ZONE_RANK: u16 = 70;
const HEIST_KEYS: &[AudioCueKey] = &[
    cue_keys::OBJECTIVE_HIT,
    cue_keys::OBJECTIVE_CRITICAL,
    cue_keys::OBJECTIVE_DESTROYED,
];
const MATCH_KEYS: &[AudioCueKey] = &[cue_keys::DEFEAT, cue_keys::IMPACT, cue_keys::READY];
const HOT_ZONE_KEYS: &[AudioCueKey] = &[cue_keys::READY, cue_keys::IMPACT];

#[derive(Resource, Default)]
pub(in crate::client::audio::producers) struct HeistAudioMemory {
    last_objective_hit_tick: Option<u64>,
}

#[derive(Resource, Default)]
pub(in crate::client::audio::producers) struct MatchAudioMemory {
    last: Option<(MatchId, MatchPhase, Option<[u16; 2]>)>,
}

/// Per-match deduplication memory for objective cues; never gameplay truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HotZoneAudioSnapshot {
    match_id: MatchId,
    status: HotZoneStatus,
    progress_ticks: [u16; 2],
    target_progress_ticks: u16,
    completed: bool,
}

#[derive(Resource, Default)]
pub(in crate::client::audio::producers) struct HotZoneAudioMemory {
    last: Option<HotZoneAudioSnapshot>,
}

pub(super) fn register_heist(app: &mut App) {
    app.init_resource::<HeistAudioMemory>()
        .try_register_audio_producer(AudioProducerRegistration {
            id: "heist",
            rank: HEIST_RANK,
            cue_keys: HEIST_KEYS,
        })
        .expect("Heist audio producer registration must be valid");
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

pub(super) fn register_hot_zone(app: &mut App) {
    app.init_resource::<HotZoneAudioMemory>()
        .try_register_audio_producer(AudioProducerRegistration {
            id: "hot-zone",
            rank: HOT_ZONE_RANK,
            cue_keys: HOT_ZONE_KEYS,
        })
        .expect("Hot Zone audio producer registration must be valid");
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "ClientHeistReadiness is a Bevy system resource parameter"
)]
pub(super) fn produce_heist_audio_requests(
    mut cues: MessageReader<ReceivedHeistObjectiveCue>,
    readiness: Res<ClientHeistReadiness>,
    mut memory: ResMut<HeistAudioMemory>,
    mut requests: MessageWriter<AudioRequest>,
    mut sequence: Local<AudioProducerSequence>,
) {
    let ready = matches!(*readiness, ClientHeistReadiness::Ready);
    for ReceivedHeistObjectiveCue(cue) in cues.read() {
        if let Some(request) = heist_audio_request(
            cue,
            ready,
            &mut memory.last_objective_hit_tick,
            sequence.next(HEIST_RANK),
        ) {
            requests.write(request);
        }
    }
}

fn heist_audio_request(
    cue: &crate::matchplay::HeistObjectiveCue,
    ready: bool,
    last_objective_hit_tick: &mut Option<u64>,
    order: AudioRequestOrder,
) -> Option<AudioRequest> {
    if !ready {
        return None;
    }
    let cue_key = match cue.kind {
        HeistObjectiveCueKind::Damaged => {
            if last_objective_hit_tick.is_some_and(|last| cue.tick < last.saturating_add(6)) {
                return None;
            }
            *last_objective_hit_tick = Some(cue.tick);
            cue_keys::OBJECTIVE_HIT
        }
        HeistObjectiveCueKind::Critical => cue_keys::OBJECTIVE_CRITICAL,
        HeistObjectiveCueKind::Destroyed => cue_keys::OBJECTIVE_DESTROYED,
    };
    Some(AudioRequest::for_occurrence(cue_key, cue.event_id.0, order))
}

#[allow(
    clippy::type_complexity,
    reason = "the query declares common match and optional Wipeout transition inputs"
)]
pub(super) fn produce_match_audio_requests(
    matches: Query<
        (&MatchState, Option<&WipeoutState>),
        (
            With<MatchRoot>,
            Or<(Changed<MatchState>, Changed<WipeoutState>)>,
        ),
    >,
    mut memory: ResMut<MatchAudioMemory>,
    mut requests: MessageWriter<AudioRequest>,
    mut sequence: Local<AudioProducerSequence>,
) {
    let Some((current, wipeout)) = matches.iter().next() else {
        return;
    };
    let wipeout_scores = wipeout.map(|wipeout| wipeout.team_scores);
    let previous = memory.last;
    memory.last = Some((current.match_id, current.phase, wipeout_scores));
    if let Some(cue_key) = match_audio_cue(current, wipeout_scores, previous) {
        requests.write(AudioRequest::once(cue_key, sequence.next(MATCH_RANK)));
    }
}

fn match_audio_cue(
    current: &MatchState,
    wipeout_scores: Option<[u16; 2]>,
    previous: Option<(MatchId, MatchPhase, Option<[u16; 2]>)>,
) -> Option<AudioCueKey> {
    let hot_zone_mode = current.mode_definition_id == HOT_ZONE_MODE_DEFINITION;
    if matches!(current.phase, MatchPhase::Completed { .. })
        && !previous.is_some_and(|(id, phase, _)| {
            id == current.match_id && matches!(phase, MatchPhase::Completed { .. })
        })
    {
        Some(cue_keys::DEFEAT)
    } else if !hot_zone_mode
        && previous.is_some_and(|(id, _, scores)| {
            id == current.match_id && scores.is_some_and(|scores| Some(scores) != wipeout_scores)
        })
    {
        Some(cue_keys::IMPACT)
    } else if previous.is_none_or(|(id, phase, _)| id != current.match_id || phase != current.phase)
        && matches!(
            current.phase,
            MatchPhase::Countdown { .. } | MatchPhase::Active { .. }
        )
    {
        Some(cue_keys::READY)
    } else {
        None
    }
}

/// Bounded objective feedback: control gained/lost, contested entry, and 50%/90% thresholds.
/// Match completion remains owned by `produce_match_audio_requests`.
#[allow(
    clippy::type_complexity,
    reason = "the query declares the complete Hot Zone transition view"
)]
pub(super) fn produce_hot_zone_audio_requests(
    zones: Query<
        (&HotZoneState, &MatchState),
        (
            With<MatchRoot>,
            Or<(Changed<HotZoneState>, Changed<MatchState>)>,
        ),
    >,
    mut memory: ResMut<HotZoneAudioMemory>,
    mut requests: MessageWriter<AudioRequest>,
    mut sequence: Local<AudioProducerSequence>,
) {
    let Some((hot_zone, match_state)) = zones.iter().next() else {
        return;
    };
    if hot_zone.match_id != match_state.match_id {
        return;
    }
    let snapshot = HotZoneAudioSnapshot {
        match_id: hot_zone.match_id,
        status: hot_zone.status,
        progress_ticks: hot_zone.progress_ticks,
        target_progress_ticks: hot_zone.target_progress_ticks,
        completed: matches!(match_state.phase, MatchPhase::Completed { .. }),
    };
    let previous = memory.last.replace(snapshot);
    let Some(previous) = previous.filter(|previous| previous.match_id == snapshot.match_id) else {
        return;
    };
    if let Some(cue_key) = hot_zone_audio_cue(snapshot, previous) {
        requests.write(AudioRequest::once(cue_key, sequence.next(HOT_ZONE_RANK)));
    }
}

fn hot_zone_audio_cue(
    snapshot: HotZoneAudioSnapshot,
    previous: HotZoneAudioSnapshot,
) -> Option<AudioCueKey> {
    let percent = |progress: u16| {
        u32::from(progress) * 100 / u32::from(snapshot.target_progress_ticks.max(1))
    };
    let controlled_now = matches!(snapshot.status, HotZoneStatus::Controlled { .. });
    if snapshot.status == previous.status {
        [0_usize, 1].into_iter().find_map(|team| {
            let crossed = |progress: u16| {
                [50_u32, 90].into_iter().any(|mark| {
                    percent(progress) >= mark && percent(previous.progress_ticks[team]) < mark
                })
            };
            (snapshot.progress_ticks[team] > previous.progress_ticks[team]
                && crossed(snapshot.progress_ticks[team]))
            .then_some(cue_keys::IMPACT)
        })
    } else if controlled_now {
        Some(cue_keys::READY)
    } else {
        Some(cue_keys::IMPACT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        combat::{AttackId, CombatEventId, TeamId, WorldPoint},
        map::{DamageableTargetIdentity, ModeAnchorId, WIPEOUT_MODE_DEFINITION},
        matchplay::{HeistObjectiveCue, MatchResult},
    };

    fn heist_cue(event_id: u64, tick: u64, kind: HeistObjectiveCueKind) -> HeistObjectiveCue {
        HeistObjectiveCue {
            event_id: CombatEventId(event_id),
            tick,
            attack_id: AttackId(event_id + 10),
            source_subject: None,
            target: DamageableTargetIdentity::HeistSafe {
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
    fn heist_damage_uses_six_tick_suppression_but_terminal_cues_are_immediate() {
        let mut last_hit = None;
        assert_eq!(
            heist_audio_request(
                &heist_cue(1, 100, HeistObjectiveCueKind::Damaged),
                true,
                &mut last_hit,
                AudioRequestOrder::new(HEIST_RANK, 0),
            ),
            Some(AudioRequest::for_occurrence(
                cue_keys::OBJECTIVE_HIT,
                1,
                AudioRequestOrder::new(HEIST_RANK, 0),
            ))
        );
        assert_eq!(
            heist_audio_request(
                &heist_cue(2, 105, HeistObjectiveCueKind::Damaged),
                true,
                &mut last_hit,
                AudioRequestOrder::new(HEIST_RANK, 1),
            ),
            None
        );
        assert_eq!(
            heist_audio_request(
                &heist_cue(3, 105, HeistObjectiveCueKind::Critical),
                true,
                &mut last_hit,
                AudioRequestOrder::new(HEIST_RANK, 2),
            ),
            Some(AudioRequest::for_occurrence(
                cue_keys::OBJECTIVE_CRITICAL,
                3,
                AudioRequestOrder::new(HEIST_RANK, 2),
            ))
        );
        assert_eq!(
            heist_audio_request(
                &heist_cue(4, 105, HeistObjectiveCueKind::Destroyed),
                true,
                &mut last_hit,
                AudioRequestOrder::new(HEIST_RANK, 3),
            ),
            Some(AudioRequest::for_occurrence(
                cue_keys::OBJECTIVE_DESTROYED,
                4,
                AudioRequestOrder::new(HEIST_RANK, 3),
            ))
        );
        assert_eq!(
            heist_audio_request(
                &heist_cue(5, 106, HeistObjectiveCueKind::Damaged),
                true,
                &mut last_hit,
                AudioRequestOrder::new(HEIST_RANK, 4),
            ),
            Some(AudioRequest::for_occurrence(
                cue_keys::OBJECTIVE_HIT,
                5,
                AudioRequestOrder::new(HEIST_RANK, 4),
            ))
        );
    }

    #[test]
    fn common_match_and_hot_zone_transition_selectors_keep_exact_keys() {
        let current = MatchState {
            match_id: MatchId(7),
            mode_definition_id: WIPEOUT_MODE_DEFINITION,
            phase: MatchPhase::Active { ends_at_tick: 50 },
            rules_revision: 1,
        };
        assert_eq!(
            match_audio_cue(
                &current,
                Some([1, 0]),
                Some((current.match_id, current.phase, Some([0, 0]))),
            ),
            Some(cue_keys::IMPACT)
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
            ),
            Some(cue_keys::DEFEAT)
        );

        let previous = HotZoneAudioSnapshot {
            match_id: MatchId(7),
            status: HotZoneStatus::Empty,
            progress_ticks: [49, 0],
            target_progress_ticks: 100,
            completed: false,
        };
        assert_eq!(
            hot_zone_audio_cue(
                HotZoneAudioSnapshot {
                    status: HotZoneStatus::Controlled { team: TeamId(0) },
                    ..previous
                },
                previous,
            ),
            Some(cue_keys::READY)
        );
        assert_eq!(
            hot_zone_audio_cue(
                HotZoneAudioSnapshot {
                    progress_ticks: [50, 0],
                    ..previous
                },
                previous,
            ),
            Some(cue_keys::IMPACT)
        );
    }
}
