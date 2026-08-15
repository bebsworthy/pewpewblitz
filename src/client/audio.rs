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
}

#[derive(Resource, Default)]
struct ClientAudioState {
    recent: VecDeque<(SoundKind, u64)>,
    was_playable: bool,
    was_error: bool,
    suppressed: u64,
    last_match: Option<(crate::matchplay::MatchId, MatchPhase, [u16; 2])>,
}

pub struct ClientAudioPlugin;

impl Plugin for ClientAudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientAudioState>().add_systems(
            Update,
            (
                play_combat_audio,
                play_ability_audio,
                play_reload_audio,
                play_session_audio,
                play_match_audio,
            )
                .after(crate::map::MapPresentationSet::Readiness),
        );
    }
}

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
            | crate::builds::AbilityPhase::Deployed { .. } => {}
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

fn play_match_audio(
    mut commands: Commands,
    handles: Option<Res<ClientAssetHandles>>,
    asset_server: Res<AssetServer>,
    matches: Query<&MatchState, (With<MatchRoot>, Changed<MatchState>)>,
    mut state: ResMut<ClientAudioState>,
    active: Query<(), With<ClientAudioOneShot>>,
) {
    let Some(current) = matches.iter().next() else {
        return;
    };
    let previous = state.last_match;
    state.last_match = Some((current.match_id, current.phase, current.team_scores));
    let Some(handles) = handles else {
        return;
    };
    if active.iter().count() >= MAX_ACTIVE_ONE_SHOTS {
        state.suppressed = state.suppressed.saturating_add(1);
        return;
    }
    let sound = if matches!(current.phase, MatchPhase::Completed { .. })
        && !previous.is_some_and(|(id, phase, _)| {
            id == current.match_id && matches!(phase, MatchPhase::Completed { .. })
        }) {
        Some(handles.defeat.clone())
    } else if previous
        .is_some_and(|(id, _, scores)| id == current.match_id && scores != current.team_scores)
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
            SoundKind::Impact => Some(handles.impact.clone()),
            SoundKind::Defeat => Some(handles.defeat.clone()),
            SoundKind::Reset | SoundKind::Sentry => Some(handles.ready.clone()),
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
        SoundKind::Defeat
        | SoundKind::Reset
        | SoundKind::Ready
        | SoundKind::Error
        | SoundKind::Dash
        | SoundKind::Sentry
        | SoundKind::ChargeReady
        | SoundKind::Passive => MAX_ACTIVE_ONE_SHOTS,
    }
}

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
            target: NetworkEntityId(8),
            position: crate::combat::WorldPoint { x: 20.0, y: 30.0 },
            presentation_profile_id: crate::combat::WeaponPresentationProfileId(1),
        };
        assert_eq!(combat_sound(&cue), Some((SoundKind::Sentry, 92)));
    }
}
