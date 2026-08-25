//! Bounded client-only audio presentation driven by deduplicated gameplay facts.
#![allow(clippy::wildcard_imports)]

use super::*;
#[cfg(test)]
use crate::combat::AttackId;
use crate::combat::{CombatCue, DeduplicatedCombatCue, WeaponPhase, WeaponState};
use bevy::audio::{AudioPlayer, PlaybackSettings};
use std::collections::VecDeque;

const MAX_ACTIVE_ONE_SHOTS: usize = 24;
const MAX_RECENT_AUDIO_KEYS: usize = 128;

#[derive(Component)]
struct ClientAudioOneShot;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SoundKind {
    Fire,
    Impact,
    Defeat,
    Reset,
    Ready,
    Error,
    Dash,
    Sentry,
    ChargeReady,
    Passive,
    ObjectiveHit,
    ObjectiveCritical,
    ObjectiveDestroyed,
}

#[derive(Resource, Default)]
struct ClientAudioState {
    recent: VecDeque<(SoundKind, u64)>,
    was_playable: bool,
    was_error: bool,
    suppressed: u64,
    last_match: Option<(crate::matchplay::MatchId, MatchPhase, Option<[u16; 2]>)>,
    last_hot_zone: Option<HotZoneAudioMemory>,
    last_objective_hit_tick: Option<u64>,
}

/// Per-match deduplication memory for objective cues; never gameplay truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HotZoneAudioMemory {
    match_id: crate::matchplay::MatchId,
    status: crate::matchplay::HotZoneStatus,
    progress_ticks: [u16; 2],
    target_progress_ticks: u16,
    completed: bool,
}

pub struct ClientAudioPlugin;

impl Plugin for ClientAudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientAudioState>().add_systems(
            Update,
            (
                play_combat_audio,
                play_heist_objective_audio,
                play_ability_audio,
                play_reload_audio,
                play_session_audio,
                play_match_audio,
                play_hot_zone_audio,
            )
                .after(crate::map::MapPresentationSet::Readiness),
        );
    }
}

#[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
fn play_heist_objective_audio(
    mut commands: Commands,
    handles: Option<Res<ClientAssetHandles>>,
    asset_server: Res<AssetServer>,
    mut cues: MessageReader<crate::matchplay::ReceivedHeistObjectiveCue>,
    readiness: Res<hud::ClientHeistReadiness>,
    mut state: ResMut<ClientAudioState>,
    active: Query<(), With<ClientAudioOneShot>>,
) {
    let Some(handles) = handles else {
        cues.clear();
        return;
    };
    let ready = matches!(*readiness, hud::ClientHeistReadiness::Ready);
    let mut active_count = active.iter().count();
    for crate::matchplay::ReceivedHeistObjectiveCue(cue) in cues.read() {
        if !ready {
            continue;
        }
        let (kind, handle) = match cue.kind {
            crate::matchplay::HeistObjectiveCueKind::Damaged => {
                if state
                    .last_objective_hit_tick
                    .is_some_and(|last| cue.tick < last.saturating_add(6))
                {
                    continue;
                }
                state.last_objective_hit_tick = Some(cue.tick);
                (SoundKind::ObjectiveHit, handles.impact.clone())
            }
            crate::matchplay::HeistObjectiveCueKind::Critical => {
                (SoundKind::ObjectiveCritical, handles.ready.clone())
            }
            crate::matchplay::HeistObjectiveCueKind::Destroyed => {
                (SoundKind::ObjectiveDestroyed, handles.defeat.clone())
            }
        };
        let key = (kind, cue.event_id.0);
        if state.recent.contains(&key) {
            continue;
        }
        remember_audio_key(&mut state.recent, key);
        if active_count >= live_limit_for(kind) || !asset_server.is_loaded(&handle) {
            state.suppressed = state.suppressed.saturating_add(1);
            continue;
        }
        commands.spawn((
            ClientAudioOneShot,
            AudioPlayer::new(handle),
            PlaybackSettings::DESPAWN,
        ));
        active_count += 1;
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn play_ability_audio(
    mut commands: Commands,
    handles: Option<Res<ClientAssetHandles>>,
    asset_server: Res<AssetServer>,
    abilities: Query<
        &crate::builds::AbilityState,
        (
            With<Fighter>,
            With<Controlled>,
            Changed<crate::builds::AbilityState>,
        ),
    >,
    passives: Query<
        &crate::builds::PassiveRuntimeState,
        (
            With<Fighter>,
            With<Controlled>,
            Changed<crate::builds::PassiveRuntimeState>,
        ),
    >,
    sentries: Query<&crate::abilities::SentryIdentity, Added<crate::abilities::Sentry>>,
    concealment_fields: Query<
        &crate::concealment::ConcealmentFieldState,
        Added<crate::concealment::ConcealmentFieldState>,
    >,
    active: Query<(), With<ClientAudioOneShot>>,
) {
    let Some(handles) = handles else { return };
    let mut sounds = Vec::new();
    for ability in &abilities {
        match ability.phase {
            crate::builds::AbilityPhase::Dashing { .. } => {
                sounds.push((SoundKind::Dash, handles.defeat.clone(), 1.45));
            }
            crate::builds::AbilityPhase::Ready => {
                sounds.push((SoundKind::ChargeReady, handles.ready.clone(), 1.25));
            }
            crate::builds::AbilityPhase::Charging
            | crate::builds::AbilityPhase::Deployed { .. }
            | crate::builds::AbilityPhase::Cloaked { .. }
            | crate::builds::AbilityPhase::FieldActive { .. } => {}
        }
    }
    if passives
        .iter()
        .any(|state| state.adrenaline_until_tick.is_some() || state.quick_cycle_primed)
    {
        sounds.push((SoundKind::Passive, handles.impact.clone(), 1.35));
    }
    for _ in &sentries {
        sounds.push((SoundKind::Sentry, handles.ready.clone(), 0.75));
    }
    for _ in &concealment_fields {
        sounds.push((SoundKind::Sentry, handles.ready.clone(), 0.9));
    }
    let available = MAX_ACTIVE_ONE_SHOTS.saturating_sub(active.iter().count());
    for (_kind, handle, speed) in sounds.into_iter().take(available) {
        if asset_server.is_loaded(&handle) {
            commands.spawn((
                ClientAudioOneShot,
                AudioPlayer::new(handle),
                PlaybackSettings {
                    speed,
                    ..PlaybackSettings::DESPAWN
                },
            ));
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn play_match_audio(
    mut commands: Commands,
    handles: Option<Res<ClientAssetHandles>>,
    asset_server: Res<AssetServer>,
    matches: Query<
        (&MatchState, Option<&crate::matchplay::WipeoutState>),
        (
            With<MatchRoot>,
            Or<(Changed<MatchState>, Changed<crate::matchplay::WipeoutState>)>,
        ),
    >,
    mut state: ResMut<ClientAudioState>,
    active: Query<(), With<ClientAudioOneShot>>,
) {
    let Some((current, wipeout)) = matches.iter().next() else {
        return;
    };
    let wipeout_scores = wipeout.map(|wipeout| wipeout.team_scores);
    let previous = state.last_match;
    state.last_match = Some((current.match_id, current.phase, wipeout_scores));
    let Some(handles) = handles else {
        return;
    };
    if active.iter().count() >= MAX_ACTIVE_ONE_SHOTS {
        state.suppressed = state.suppressed.saturating_add(1);
        return;
    }
    let hot_zone_mode = current.mode_definition_id == crate::map::HOT_ZONE_MODE_DEFINITION;
    let sound = if matches!(current.phase, MatchPhase::Completed { .. })
        && !previous.is_some_and(|(id, phase, _)| {
            id == current.match_id && matches!(phase, MatchPhase::Completed { .. })
        }) {
        Some(handles.defeat.clone())
    } else if !hot_zone_mode
        && previous.is_some_and(|(id, _, scores)| {
            id == current.match_id && scores.is_some_and(|scores| Some(scores) != wipeout_scores)
        })
    {
        Some(handles.impact.clone())
    } else if previous.is_none_or(|(id, phase, _)| id != current.match_id || phase != current.phase)
        && matches!(
            current.phase,
            MatchPhase::Countdown { .. } | MatchPhase::Active { .. }
        )
    {
        Some(handles.ready.clone())
    } else {
        None
    };
    if let Some(handle) = sound.filter(|handle| asset_server.is_loaded(handle)) {
        commands.spawn((
            ClientAudioOneShot,
            AudioPlayer::new(handle),
            PlaybackSettings::DESPAWN,
        ));
    }
}

/// Bounded objective feedback: control gained/lost, contested entry, and 50%/90% thresholds.
/// Cues are deduplicated per match/team from durable replicated state. Match completion is
/// owned by `play_match_audio` so exactly one completion cue can play per match.
#[allow(
    clippy::type_complexity,
    reason = "the query declares this system's complete world view inline at its schedule boundary"
)]
#[allow(clippy::needless_pass_by_value)]
fn play_hot_zone_audio(
    mut commands: Commands,
    handles: Option<Res<ClientAssetHandles>>,
    asset_server: Res<AssetServer>,
    zones: Query<
        (&crate::matchplay::HotZoneState, &MatchState),
        (
            With<MatchRoot>,
            Or<(Changed<crate::matchplay::HotZoneState>, Changed<MatchState>)>,
        ),
    >,
    mut state: ResMut<ClientAudioState>,
    active: Query<(), With<ClientAudioOneShot>>,
) {
    let Some((hot_zone, match_state)) = zones.iter().next() else {
        return;
    };
    if hot_zone.match_id != match_state.match_id {
        return;
    }
    let completed = matches!(match_state.phase, MatchPhase::Completed { .. });
    let memory = HotZoneAudioMemory {
        match_id: hot_zone.match_id,
        status: hot_zone.status,
        progress_ticks: hot_zone.progress_ticks,
        target_progress_ticks: hot_zone.target_progress_ticks,
        completed,
    };
    let previous = state.last_hot_zone.replace(memory);
    let Some(handles) = handles else {
        return;
    };
    let Some(previous) = previous.filter(|previous| previous.match_id == memory.match_id) else {
        return;
    };
    if active.iter().count() >= MAX_ACTIVE_ONE_SHOTS {
        state.suppressed = state.suppressed.saturating_add(1);
        return;
    }
    let percent = |progress: u16| {
        u32::from(progress) * 100 / u32::from(hot_zone.target_progress_ticks.max(1))
    };
    let controlled_now = matches!(
        memory.status,
        crate::matchplay::HotZoneStatus::Controlled { .. }
    );
    let sound = if memory.status == previous.status {
        // Threshold crossings within one team's progress only.
        [0_usize, 1].into_iter().find_map(|team| {
            let crossed = |progress: u16| {
                [50_u32, 90].into_iter().any(|mark| {
                    percent(progress) >= mark && percent(previous.progress_ticks[team]) < mark
                })
            };
            (memory.progress_ticks[team] > previous.progress_ticks[team]
                && crossed(memory.progress_ticks[team]))
            .then_some(handles.impact.clone())
        })
    } else {
        Some(if controlled_now {
            handles.ready.clone()
        } else {
            handles.impact.clone()
        })
    };
    if let Some(handle) = sound.filter(|handle| asset_server.is_loaded(handle)) {
        commands.spawn((
            ClientAudioOneShot,
            AudioPlayer::new(handle),
            PlaybackSettings::DESPAWN,
        ));
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn play_combat_audio(
    mut commands: Commands,
    handles: Option<Res<ClientAssetHandles>>,
    asset_server: Res<AssetServer>,
    mut cues: MessageReader<DeduplicatedCombatCue>,
    mut state: ResMut<ClientAudioState>,
    active: Query<(), With<ClientAudioOneShot>>,
) {
    let Some(handles) = handles else {
        cues.clear();
        return;
    };
    let mut active_count = active.iter().count();
    for DeduplicatedCombatCue(cue) in cues.read() {
        let Some((kind, key)) = combat_sound(cue) else {
            continue;
        };
        if state.recent.contains(&(kind, key)) {
            continue;
        }
        remember_audio_key(&mut state.recent, (kind, key));
        if active_count >= live_limit_for(kind) {
            state.suppressed = state.suppressed.saturating_add(1);
            if state.suppressed.is_power_of_two() {
                warn!(
                    suppressed = state.suppressed,
                    ?kind,
                    live_limit = live_limit_for(kind),
                    "client audio one-shot cap suppressed a cue"
                );
            }
            continue;
        }
        let Some(handle) = (match kind {
            SoundKind::Fire => Some(handles.fire.clone()),
            SoundKind::Impact | SoundKind::ObjectiveHit => Some(handles.impact.clone()),
            SoundKind::Defeat | SoundKind::ObjectiveDestroyed => Some(handles.defeat.clone()),
            SoundKind::Reset | SoundKind::Sentry | SoundKind::ObjectiveCritical => {
                Some(handles.ready.clone())
            }
            SoundKind::Ready
            | SoundKind::Error
            | SoundKind::Dash
            | SoundKind::ChargeReady
            | SoundKind::Passive => None,
        }) else {
            continue;
        };
        if !asset_server.is_loaded(&handle) {
            continue;
        }
        commands.spawn((
            ClientAudioOneShot,
            AudioPlayer::new(handle),
            PlaybackSettings::DESPAWN,
        ));
        active_count += 1;
    }
}

const fn live_limit_for(kind: SoundKind) -> usize {
    match kind {
        SoundKind::Fire => MAX_ACTIVE_ONE_SHOTS - 4,
        SoundKind::Impact => MAX_ACTIVE_ONE_SHOTS - 2,
        SoundKind::ObjectiveHit => 6,
        SoundKind::Defeat
        | SoundKind::Reset
        | SoundKind::Ready
        | SoundKind::Error
        | SoundKind::Dash
        | SoundKind::Sentry
        | SoundKind::ChargeReady
        | SoundKind::Passive
        | SoundKind::ObjectiveCritical
        | SoundKind::ObjectiveDestroyed => MAX_ACTIVE_ONE_SHOTS,
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn play_reload_audio(
    mut commands: Commands,
    handles: Option<Res<ClientAssetHandles>>,
    asset_server: Res<AssetServer>,
    weapons: Query<&WeaponState, (With<Fighter>, With<Controlled>, Changed<WeaponState>)>,
    active: Query<(), With<ClientAudioOneShot>>,
) {
    let Some(handles) = handles else {
        return;
    };
    if !asset_server.is_loaded(&handles.ready) {
        return;
    }
    let reloads = weapons
        .iter()
        .filter(|state| matches!(state.phase, WeaponPhase::Reloading { .. }))
        .count();
    let available = MAX_ACTIVE_ONE_SHOTS.saturating_sub(active.iter().count());
    for _ in 0..reloads.min(available) {
        commands.spawn((
            ClientAudioOneShot,
            AudioPlayer::new(handles.ready.clone()),
            PlaybackSettings::DESPAWN,
        ));
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
#[allow(clippy::too_many_arguments)]
fn play_session_audio(
    mut commands: Commands,
    handles: Option<Res<ClientAssetHandles>>,
    asset_server: Res<AssetServer>,
    playable: Res<ClientPlayableGate>,
    map: Res<crate::map::ClientMapReadiness>,
    joins: Query<&ClientJoinStatus>,
    mut state: ResMut<ClientAudioState>,
    active: Query<(), With<ClientAudioOneShot>>,
) {
    let is_error = matches!(*map, crate::map::ClientMapReadiness::Invalid(_))
        || joins.iter().any(|status| {
            matches!(
                status.phase,
                ClientJoinPhase::Rejected(_) | ClientJoinPhase::Disconnected
            )
        });
    let Some(handles) = handles else {
        state.was_playable = playable.0;
        state.was_error = is_error;
        return;
    };
    if active.iter().count() < MAX_ACTIVE_ONE_SHOTS {
        let sound = if playable.0 && !state.was_playable {
            Some((SoundKind::Ready, handles.ready.clone()))
        } else if is_error && !state.was_error {
            Some((SoundKind::Error, handles.error.clone()))
        } else {
            None
        };
        if let Some((_kind, handle)) = sound.filter(|(_, handle)| asset_server.is_loaded(handle)) {
            commands.spawn((
                ClientAudioOneShot,
                AudioPlayer::new(handle),
                PlaybackSettings::DESPAWN,
            ));
        }
    }
    state.was_playable = playable.0;
    state.was_error = is_error;
}

fn combat_sound(cue: &CombatCue) -> Option<(SoundKind, u64)> {
    match cue {
        CombatCue::AttackAccepted { attack_id, .. } => Some((SoundKind::Fire, attack_id.0)),
        CombatCue::DeliveryImpact { attack_id, .. }
        | CombatCue::LobLanded { attack_id, .. }
        | CombatCue::MeleeContact { attack_id, .. } => Some((SoundKind::Impact, attack_id.0)),
        CombatCue::FighterDefeated { attack_id, .. } => Some((SoundKind::Defeat, attack_id.0)),
        CombatCue::FighterReset { event_id, .. } => Some((SoundKind::Reset, event_id.0)),
        CombatCue::SentryFired { event_id, .. } | CombatCue::DeployableRemoved { event_id, .. } => {
            Some((SoundKind::Sentry, event_id.0))
        }
        CombatCue::DamageApplied { .. }
        | CombatCue::EffectApplied { .. }
        | CombatCue::Muzzle { .. }
        | CombatCue::Impact { .. }
        | CombatCue::Damage { .. }
        | CombatCue::Defeat { .. }
        | CombatCue::Reset { .. } => None,
        CombatCue::SelfCloakActivated { event_id, .. }
        | CombatCue::RevealScanActivated { event_id, .. }
        | CombatCue::ForcedRevealApplied { event_id, .. } => Some((SoundKind::Ready, event_id.0)),
        CombatCue::SelfCloakEnded { event_id, .. } => Some((SoundKind::Impact, event_id.0)),
    }
}

fn remember_audio_key(recent: &mut VecDeque<(SoundKind, u64)>, key: (SoundKind, u64)) {
    if recent.len() == MAX_RECENT_AUDIO_KEYS {
        recent.pop_front();
    }
    recent.push_back(key);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_key_history_is_bounded_and_preserves_newest_key() {
        let mut recent = VecDeque::new();
        for key in 0..=MAX_RECENT_AUDIO_KEYS {
            remember_audio_key(&mut recent, (SoundKind::Impact, key as u64));
        }
        assert_eq!(recent.len(), MAX_RECENT_AUDIO_KEYS);
        assert_eq!(recent.front(), Some(&(SoundKind::Impact, 1)));
        assert_eq!(
            recent.back(),
            Some(&(SoundKind::Impact, MAX_RECENT_AUDIO_KEYS as u64))
        );
    }

    #[test]
    fn each_attack_sound_kind_coalesces_independently() {
        let mut recent = VecDeque::new();
        let attack = AttackId(42);
        remember_audio_key(&mut recent, (SoundKind::Fire, attack.0));
        remember_audio_key(&mut recent, (SoundKind::Impact, attack.0));
        assert!(recent.contains(&(SoundKind::Fire, 42)));
        assert!(recent.contains(&(SoundKind::Impact, 42)));
    }

    #[test]
    fn low_priority_sounds_reserve_capacity_for_defeat_and_session_feedback() {
        assert!(live_limit_for(SoundKind::Fire) < live_limit_for(SoundKind::Impact));
        assert!(live_limit_for(SoundKind::Impact) < live_limit_for(SoundKind::Defeat));
        assert_eq!(live_limit_for(SoundKind::Error), MAX_ACTIVE_ONE_SHOTS);
    }

    #[test]
    fn deployable_removal_is_a_supported_combat_sound() {
        let cue = CombatCue::DeployableRemoved {
            event_id: crate::combat::CombatEventId(91),
            tick: 100,
            owner: NetworkEntityId(7),
            deployable_id: crate::builds::DeployableId(3),
            position: crate::combat::WorldPoint { x: 20.0, y: 30.0 },
            reason: crate::abilities::SentryCleanupReason::Destroyed,
        };
        assert_eq!(combat_sound(&cue), Some((SoundKind::Sentry, 91)));
    }

    #[test]
    fn sentry_fire_is_a_supported_combat_sound() {
        let cue = CombatCue::SentryFired {
            event_id: crate::combat::CombatEventId(92),
            tick: 100,
            owner: NetworkEntityId(7),
            deployable_id: crate::builds::DeployableId(3),
            target: Some(NetworkEntityId(8)),
            position: crate::combat::WorldPoint { x: 20.0, y: 30.0 },
            presentation_profile_id: crate::combat::WeaponPresentationProfileId(1),
        };
        assert_eq!(combat_sound(&cue), Some((SoundKind::Sentry, 92)));
    }
}
