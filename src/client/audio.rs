//! Bounded client-only audio presentation driven by deduplicated gameplay facts.
#![allow(clippy::wildcard_imports)]

use super::*;
#[cfg(test)]
use crate::combat::AttackId;
use crate::combat::{CombatCue, DeduplicatedCombatCue, WeaponState};
use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use std::collections::{HashMap, VecDeque};

const MAX_RECENT_AUDIO_KEYS: usize = 128;

mod catalog;
use catalog::{AudioAssetKey, AudioCueFamily, AudioProfileCatalog};

#[derive(Component)]
struct ClientAudioOneShot;

#[derive(Resource, Default)]
struct ClientAudioState {
    recent: VecDeque<(AudioCueFamily, u64)>,
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
        app.insert_resource(
            AudioProfileCatalog::embedded().expect("embedded audio profile catalog is valid"),
        )
        .init_resource::<ClientAudioState>()
        .add_systems(
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
    catalog: Res<AudioProfileCatalog>,
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
        let family = match cue.kind {
            crate::matchplay::HeistObjectiveCueKind::Damaged => {
                if state
                    .last_objective_hit_tick
                    .is_some_and(|last| cue.tick < last.saturating_add(6))
                {
                    continue;
                }
                state.last_objective_hit_tick = Some(cue.tick);
                AudioCueFamily::ObjectiveHit
            }
            crate::matchplay::HeistObjectiveCueKind::Critical => AudioCueFamily::ObjectiveCritical,
            crate::matchplay::HeistObjectiveCueKind::Destroyed => {
                AudioCueFamily::ObjectiveDestroyed
            }
        };
        let key = (family, cue.event_id.0);
        if state.recent.contains(&key) {
            continue;
        }
        remember_audio_key(&mut state.recent, key);
        if !matches!(
            try_play_audio(
                &mut commands,
                &catalog,
                &handles,
                &asset_server,
                family,
                &mut active_count,
            ),
            PlaybackAttempt::Played
        ) {
            state.suppressed = state.suppressed.saturating_add(1);
        }
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
    catalog: Res<AudioProfileCatalog>,
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
                sounds.push(AudioCueFamily::Dash);
            }
            crate::builds::AbilityPhase::Ready => {
                sounds.push(AudioCueFamily::ChargeReady);
            }
            crate::builds::AbilityPhase::Charging
            | crate::builds::AbilityPhase::Deployed { .. }
            | crate::builds::AbilityPhase::Cloaked { .. }
            | crate::builds::AbilityPhase::FieldActive { .. }
            | crate::builds::AbilityPhase::ElementalFieldActive { .. } => {}
        }
    }
    if passives
        .iter()
        .any(|state| state.adrenaline_until_tick.is_some() || state.quick_cycle_primed)
    {
        sounds.push(AudioCueFamily::Passive);
    }
    for _ in &sentries {
        sounds.push(AudioCueFamily::SentrySpawn);
    }
    for _ in &concealment_fields {
        sounds.push(AudioCueFamily::ConcealmentFieldSpawn);
    }
    let mut active_count = active.iter().count();
    for family in sounds {
        let _ = try_play_audio(
            &mut commands,
            &catalog,
            &handles,
            &asset_server,
            family,
            &mut active_count,
        );
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
    catalog: Res<AudioProfileCatalog>,
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
    let hot_zone_mode = current.mode_definition_id == crate::map::HOT_ZONE_MODE_DEFINITION;
    let family = if matches!(current.phase, MatchPhase::Completed { .. })
        && !previous.is_some_and(|(id, phase, _)| {
            id == current.match_id && matches!(phase, MatchPhase::Completed { .. })
        }) {
        Some(AudioCueFamily::Defeat)
    } else if !hot_zone_mode
        && previous.is_some_and(|(id, _, scores)| {
            id == current.match_id && scores.is_some_and(|scores| Some(scores) != wipeout_scores)
        })
    {
        Some(AudioCueFamily::Impact)
    } else if previous.is_none_or(|(id, phase, _)| id != current.match_id || phase != current.phase)
        && matches!(
            current.phase,
            MatchPhase::Countdown { .. } | MatchPhase::Active { .. }
        )
    {
        Some(AudioCueFamily::Ready)
    } else {
        None
    };
    let mut active_count = active.iter().count();
    if family.is_some_and(|family| {
        matches!(
            try_play_audio(
                &mut commands,
                &catalog,
                &handles,
                &asset_server,
                family,
                &mut active_count,
            ),
            PlaybackAttempt::Capped
        )
    }) {
        state.suppressed = state.suppressed.saturating_add(1);
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
    catalog: Res<AudioProfileCatalog>,
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
    let percent = |progress: u16| {
        u32::from(progress) * 100 / u32::from(hot_zone.target_progress_ticks.max(1))
    };
    let controlled_now = matches!(
        memory.status,
        crate::matchplay::HotZoneStatus::Controlled { .. }
    );
    let family = if memory.status == previous.status {
        // Threshold crossings within one team's progress only.
        [0_usize, 1].into_iter().find_map(|team| {
            let crossed = |progress: u16| {
                [50_u32, 90].into_iter().any(|mark| {
                    percent(progress) >= mark && percent(previous.progress_ticks[team]) < mark
                })
            };
            (memory.progress_ticks[team] > previous.progress_ticks[team]
                && crossed(memory.progress_ticks[team]))
            .then_some(AudioCueFamily::Impact)
        })
    } else {
        Some(if controlled_now {
            AudioCueFamily::Ready
        } else {
            AudioCueFamily::Impact
        })
    };
    let mut active_count = active.iter().count();
    if family.is_some_and(|family| {
        matches!(
            try_play_audio(
                &mut commands,
                &catalog,
                &handles,
                &asset_server,
                family,
                &mut active_count,
            ),
            PlaybackAttempt::Capped
        )
    }) {
        state.suppressed = state.suppressed.saturating_add(1);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn play_combat_audio(
    mut commands: Commands,
    handles: Option<Res<ClientAssetHandles>>,
    catalog: Res<AudioProfileCatalog>,
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
        let Some((family, key)) = combat_sound(cue) else {
            continue;
        };
        if state.recent.contains(&(family, key)) {
            continue;
        }
        remember_audio_key(&mut state.recent, (family, key));
        if matches!(
            try_play_audio(
                &mut commands,
                &catalog,
                &handles,
                &asset_server,
                family,
                &mut active_count,
            ),
            PlaybackAttempt::Capped
        ) {
            state.suppressed = state.suppressed.saturating_add(1);
            if state.suppressed.is_power_of_two() {
                warn!(
                    suppressed = state.suppressed,
                    ?family,
                    live_limit = catalog
                        .playback_plan(family, |_| true)
                        .map_or(0, |plan| plan.concurrency_cap),
                    "client audio one-shot cap suppressed a cue"
                );
            }
        }
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
    catalog: Res<AudioProfileCatalog>,
    asset_server: Res<AssetServer>,
    weapons: Query<(Entity, &WeaponState), (With<Fighter>, With<Controlled>)>,
    active: Query<(), With<ClientAudioOneShot>>,
    mut observed_ammo: Local<HashMap<Entity, u8>>,
) {
    let Some(handles) = handles else {
        return;
    };
    let mut reloads = 0;
    for (entity, state) in &weapons {
        if observed_ammo
            .insert(entity, state.ammo)
            .is_some_and(|previous| state.ammo > previous)
        {
            reloads += 1;
        }
    }
    observed_ammo.retain(|entity, _| weapons.get(*entity).is_ok());
    let mut active_count = active.iter().count();
    for _ in 0..reloads {
        let _ = try_play_audio(
            &mut commands,
            &catalog,
            &handles,
            &asset_server,
            AudioCueFamily::Reload,
            &mut active_count,
        );
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
    catalog: Res<AudioProfileCatalog>,
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
    let family = if playable.0 && !state.was_playable {
        Some(AudioCueFamily::Ready)
    } else if is_error && !state.was_error {
        Some(AudioCueFamily::Error)
    } else {
        None
    };
    let mut active_count = active.iter().count();
    if let Some(family) = family {
        let _ = try_play_audio(
            &mut commands,
            &catalog,
            &handles,
            &asset_server,
            family,
            &mut active_count,
        );
    }
    state.was_playable = playable.0;
    state.was_error = is_error;
}

fn combat_sound(cue: &CombatCue) -> Option<(AudioCueFamily, u64)> {
    match cue {
        CombatCue::AttackAccepted { attack_id, .. } => Some((AudioCueFamily::Fire, attack_id.0)),
        CombatCue::DeliveryImpact { attack_id, .. }
        | CombatCue::LobLanded { attack_id, .. }
        | CombatCue::MeleeContact { attack_id, .. } => Some((AudioCueFamily::Impact, attack_id.0)),
        CombatCue::ConeSprayPulse { event_id, .. }
        | CombatCue::SelfCloakEnded { event_id, .. }
        | CombatCue::DemolitionStrikeActivated { event_id, .. } => {
            Some((AudioCueFamily::Impact, event_id.0))
        }
        CombatCue::FighterDefeated { attack_id, .. } => Some((AudioCueFamily::Defeat, attack_id.0)),
        CombatCue::FighterReset { event_id, .. } => Some((AudioCueFamily::Reset, event_id.0)),
        CombatCue::SentryFired { event_id, .. } | CombatCue::DeployableRemoved { event_id, .. } => {
            Some((AudioCueFamily::Sentry, event_id.0))
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
        | CombatCue::ForcedRevealApplied { event_id, .. }
        | CombatCue::ElementalFieldActivated { event_id, .. } => {
            Some((AudioCueFamily::Ready, event_id.0))
        }
    }
}

fn remember_audio_key(recent: &mut VecDeque<(AudioCueFamily, u64)>, key: (AudioCueFamily, u64)) {
    if recent.len() == MAX_RECENT_AUDIO_KEYS {
        recent.pop_front();
    }
    recent.push_back(key);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlaybackAttempt {
    Played,
    Capped,
    Unavailable,
}

fn try_play_audio(
    commands: &mut Commands,
    catalog: &AudioProfileCatalog,
    handles: &ClientAssetHandles,
    asset_server: &AssetServer,
    family: AudioCueFamily,
    active_count: &mut usize,
) -> PlaybackAttempt {
    let Some(plan) = catalog.playback_plan(family, |asset| {
        asset_server.is_loaded(audio_handle(handles, asset))
    }) else {
        return PlaybackAttempt::Unavailable;
    };
    if *active_count >= plan.concurrency_cap {
        return PlaybackAttempt::Capped;
    }
    commands.spawn((
        ClientAudioOneShot,
        AudioPlayer::new(audio_handle(handles, plan.asset).clone()),
        PlaybackSettings {
            speed: plan.speed,
            volume: Volume::Linear(plan.volume),
            ..PlaybackSettings::DESPAWN
        },
    ));
    *active_count += 1;
    PlaybackAttempt::Played
}

const fn audio_handle(handles: &ClientAssetHandles, key: AudioAssetKey) -> &Handle<AudioSource> {
    match key {
        AudioAssetKey::Fire => &handles.fire,
        AudioAssetKey::Impact => &handles.impact,
        AudioAssetKey::Defeat => &handles.defeat,
        AudioAssetKey::Ready => &handles.ready,
        AudioAssetKey::Error => &handles.error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_key_history_is_bounded_and_preserves_newest_key() {
        let mut recent = VecDeque::new();
        for key in 0..=MAX_RECENT_AUDIO_KEYS {
            remember_audio_key(&mut recent, (AudioCueFamily::Impact, key as u64));
        }
        assert_eq!(recent.len(), MAX_RECENT_AUDIO_KEYS);
        assert_eq!(recent.front(), Some(&(AudioCueFamily::Impact, 1)));
        assert_eq!(
            recent.back(),
            Some(&(AudioCueFamily::Impact, MAX_RECENT_AUDIO_KEYS as u64))
        );
    }

    #[test]
    fn each_attack_sound_kind_coalesces_independently() {
        let mut recent = VecDeque::new();
        let attack = AttackId(42);
        remember_audio_key(&mut recent, (AudioCueFamily::Fire, attack.0));
        remember_audio_key(&mut recent, (AudioCueFamily::Impact, attack.0));
        assert!(recent.contains(&(AudioCueFamily::Fire, 42)));
        assert!(recent.contains(&(AudioCueFamily::Impact, 42)));
    }

    #[test]
    fn low_priority_sounds_reserve_capacity_for_defeat_and_session_feedback() {
        let catalog = AudioProfileCatalog::embedded().unwrap();
        let cap = |family| {
            catalog
                .playback_plan(family, |_| true)
                .unwrap()
                .concurrency_cap
        };
        assert!(cap(AudioCueFamily::Fire) < cap(AudioCueFamily::Impact));
        assert!(cap(AudioCueFamily::Impact) < cap(AudioCueFamily::Defeat));
        assert_eq!(cap(AudioCueFamily::Error), 24);
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
        assert_eq!(combat_sound(&cue), Some((AudioCueFamily::Sentry, 91)));
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
        assert_eq!(combat_sound(&cue), Some((AudioCueFamily::Sentry, 92)));
    }
}
