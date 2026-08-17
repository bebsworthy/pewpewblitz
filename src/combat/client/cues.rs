//! Combat cue ingestion and deduplication.

#![allow(clippy::wildcard_imports)]
use super::effects::CombatEffect;
use super::*;
#[derive(Resource, Debug)]
pub struct ClientCombatEvidenceStatus {
    pub(crate) required: bool,
    pub ready: bool,
}

#[cfg(feature = "client")]
impl ClientCombatEvidenceStatus {
    #[must_use]
    pub fn permits_exit(&self) -> bool {
        !self.required || self.ready
    }
}

#[cfg(feature = "client")]
impl FromWorld for ClientCombatEvidenceStatus {
    fn from_world(_: &mut World) -> Self {
        Self {
            required: env::var_os("BRAWLER_NETWORK_COMBAT_CLIENT_READY_FILE").is_some(),
            ready: false,
        }
    }
}

#[cfg(feature = "client")]
#[derive(Resource, Default, Debug)]
pub(crate) struct RecentCombatEvents {
    pub(crate) ids: VecDeque<CombatEventId>,
}

/// Lets deterministic network tests consume the wire cue stream themselves instead of having
/// the presentation system drain it first.
#[cfg(feature = "client")]
#[derive(Resource, Debug, Default)]
pub struct CaptureCombatCues {
    pub cues: Vec<CombatCue>,
    pub dropped_cues: u64,
}

#[cfg(feature = "client")]
pub(crate) fn remember_combat_event(
    recent: &mut RecentCombatEvents,
    event_id: CombatEventId,
) -> bool {
    if recent.ids.contains(&event_id) {
        return false;
    }
    recent.ids.push_back(event_id);
    if recent.ids.len() > 256 {
        recent.ids.pop_front();
    }
    true
}

#[cfg(feature = "client")]
#[derive(Resource, Debug)]
pub struct ClientCombatObservation {
    pub(crate) saw_defeat: bool,
    pub(crate) saw_reset: bool,
    pub(crate) cue_timestamps: Vec<(ShotId, u128)>,
    pub(crate) cue_stream: Vec<CombatCue>,
    pub(crate) dropped_cue_timestamps: u64,
    pub(crate) dropped_cue_stream: u64,
    pub(crate) checkpoints: BTreeMap<String, CombatStateSnapshot>,
    pub(crate) checkpoint_matches: BTreeMap<String, Vec<CombatStateSnapshot>>,
    pub(crate) expected_checkpoints: Vec<CombatEvidenceCheckpoint>,
    pub(crate) snapshot_history: BTreeMap<u64, CombatStateSnapshot>,
    pub(crate) checkpoint_timestamps: BTreeMap<String, u128>,
    pub(crate) state_mutation_timestamps: Vec<(u64, u128)>,
    pub(crate) last_encoded_snapshot: Option<String>,
    pub(crate) ready_file: Option<PathBuf>,
    pub(crate) started_at: Instant,
    pub(crate) wrote_ready: bool,
    pub(crate) waiting_reported_at_tick: Option<u32>,
}

#[cfg(feature = "client")]
impl FromWorld for ClientCombatObservation {
    fn from_world(_: &mut World) -> Self {
        let ready_file = env::var_os("BRAWLER_NETWORK_COMBAT_CLIENT_READY_FILE").map(PathBuf::from);
        Self {
            saw_defeat: false,
            saw_reset: false,
            cue_timestamps: Vec::new(),
            cue_stream: Vec::new(),
            dropped_cue_timestamps: 0,
            dropped_cue_stream: 0,
            checkpoints: BTreeMap::new(),
            checkpoint_matches: BTreeMap::new(),
            expected_checkpoints: Vec::new(),
            snapshot_history: BTreeMap::new(),
            checkpoint_timestamps: BTreeMap::new(),
            state_mutation_timestamps: Vec::new(),
            last_encoded_snapshot: None,
            ready_file,
            started_at: Instant::now(),
            wrote_ready: false,
            waiting_reported_at_tick: None,
        }
    }
}

#[cfg(feature = "client")]
fn combat_cue_profile_id(cue: &CombatCue) -> u16 {
    match cue {
        CombatCue::AttackAccepted {
            presentation_profile_id,
            ..
        }
        | CombatCue::DeliveryImpact {
            presentation_profile_id,
            ..
        }
        | CombatCue::LobLanded {
            presentation_profile_id,
            ..
        }
        | CombatCue::MeleeContact {
            presentation_profile_id,
            ..
        }
        | CombatCue::DamageApplied {
            presentation_profile_id,
            ..
        }
        | CombatCue::EffectApplied {
            presentation_profile_id,
            ..
        }
        | CombatCue::SentryFired {
            presentation_profile_id,
            ..
        } => presentation_profile_id.0,
        CombatCue::FighterDefeated {
            presentation_profile_id,
            ..
        } => presentation_profile_id.map_or(1, |profile| profile.0),
        _ => 1,
    }
}

#[cfg(feature = "client")]
fn combat_profile_color(profile_id: u16, fallback: Color) -> Color {
    match profile_id {
        2 => Color::srgb(1.0, 0.45, 0.12),
        3 => Color::srgb(0.25, 0.7, 1.0),
        4 => Color::srgb(0.85, 0.25, 1.0),
        _ => fallback,
    }
}

#[cfg(feature = "client")]
fn combat_profile_size(profile_id: u16, fallback: Vec2) -> Vec2 {
    match profile_id {
        2 => fallback * 0.8,
        3 => fallback * 1.25,
        4 => fallback * 1.1,
        _ => fallback,
    }
}

#[cfg(feature = "client")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn receive_combat_cues(
    mut commands: Commands,
    mut recent: ResMut<RecentCombatEvents>,
    mut observation: ResMut<ClientCombatObservation>,
    mut capture: Option<ResMut<CaptureCombatCues>>,
    mut presented_cues: MessageWriter<DeduplicatedCombatCue>,
    mut receivers: Query<
        Option<&mut lightyear::prelude::MessageReceiver<CombatCue>>,
        With<lightyear::prelude::client::Client>,
    >,
    local_fighter: Query<&PlayerId, (With<Fighter>, With<lightyear::prelude::Controlled>)>,
) {
    let local_player = local_fighter.iter().next().copied();
    for receiver in &mut receivers {
        let Some(mut receiver) = receiver else {
            continue;
        };
        let cues: Vec<_> = receiver.receive().collect();
        for cue in cues {
            match &cue {
                CombatCue::Defeat { .. } | CombatCue::FighterDefeated { .. } => {
                    observation.saw_defeat = true;
                }
                CombatCue::Reset { .. } | CombatCue::FighterReset { .. } => {
                    observation.saw_reset = true;
                }
                _ => {}
            }
            let event_id = match &cue {
                CombatCue::AttackAccepted { event_id, .. }
                | CombatCue::DeliveryImpact { event_id, .. }
                | CombatCue::LobLanded { event_id, .. }
                | CombatCue::MeleeContact { event_id, .. }
                | CombatCue::DamageApplied { event_id, .. }
                | CombatCue::EffectApplied { event_id, .. }
                | CombatCue::FighterDefeated { event_id, .. }
                | CombatCue::FighterReset { event_id, .. }
                | CombatCue::SentryFired { event_id, .. }
                | CombatCue::DeployableRemoved { event_id, .. }
                | CombatCue::Muzzle { event_id, .. }
                | CombatCue::Impact { event_id, .. }
                | CombatCue::Damage { event_id, .. }
                | CombatCue::Defeat { event_id, .. }
                | CombatCue::Reset { event_id, .. } => *event_id,
            };
            if !remember_combat_event(&mut recent, event_id) {
                continue;
            }
            let profile_id = combat_cue_profile_id(&cue);
            if let Some(capture) = capture.as_mut() {
                if capture.cues.len() < MAX_COMBAT_EVIDENCE_EVENTS {
                    capture.cues.push(cue.clone());
                } else {
                    capture.dropped_cues = capture.dropped_cues.saturating_add(1);
                }
            }
            if observation.ready_file.is_some() {
                if observation.cue_stream.len() < MAX_COMBAT_EVIDENCE_EVENTS {
                    observation.cue_stream.push(cue.clone());
                } else {
                    observation.dropped_cue_stream =
                        observation.dropped_cue_stream.saturating_add(1);
                }
                let timestamp = match &cue {
                    CombatCue::Muzzle { shot_id, .. } => Some(*shot_id),
                    CombatCue::AttackAccepted { attack_id, .. } => Some(ShotId(attack_id.0)),
                    _ => None,
                };
                if let Some(shot_id) = timestamp {
                    if observation.cue_timestamps.len() < MAX_COMBAT_EVIDENCE_EVENTS {
                        observation
                            .cue_timestamps
                            .push((shot_id, unix_epoch_micros()));
                    } else {
                        observation.dropped_cue_timestamps =
                            observation.dropped_cue_timestamps.saturating_add(1);
                    }
                }
            }
            if matches!(
                &cue,
                CombatCue::Muzzle { .. }
                    | CombatCue::Impact { .. }
                    | CombatCue::Damage { .. }
                    | CombatCue::Defeat { .. }
                    | CombatCue::Reset { .. }
            ) {
                continue;
            }
            presented_cues.write(DeduplicatedCombatCue(cue.clone()));
            let local_hit = match &cue {
                CombatCue::Damage {
                    source: DamageSource::PlayerWeapon { player_id, .. },
                    ..
                }
                | CombatCue::DamageApplied {
                    source: DamageSource::PlayerWeapon { player_id, .. },
                    ..
                }
                | CombatCue::DamageApplied {
                    source:
                        DamageSource::Ultimate { player_id, .. }
                        | DamageSource::Deployable { player_id, .. },
                    ..
                } => local_player == Some(*player_id),
                _ => false,
            };
            let (position, color, size) = match cue {
                CombatCue::AttackAccepted { position, .. } => (
                    position.as_vec2(),
                    combat_profile_color(profile_id, Color::srgb(1.0, 0.8, 0.2)),
                    combat_profile_size(profile_id, Vec2::splat(16.0)),
                ),
                CombatCue::DeliveryImpact { position, .. }
                | CombatCue::LobLanded { position, .. }
                | CombatCue::MeleeContact { position, .. }
                | CombatCue::Impact { position, .. } => (
                    position.as_vec2(),
                    combat_profile_color(profile_id, Color::srgb(1.0, 0.35, 0.1)),
                    combat_profile_size(profile_id, Vec2::splat(28.0)),
                ),
                CombatCue::DamageApplied { position, .. } => (
                    position.as_vec2(),
                    combat_profile_color(
                        profile_id,
                        if local_hit {
                            Color::srgb(1.0, 0.9, 0.2)
                        } else {
                            Color::srgb(1.0, 0.1, 0.1)
                        },
                    ),
                    combat_profile_size(profile_id, Vec2::splat(18.0)),
                ),
                CombatCue::EffectApplied { position, .. } => (
                    position.as_vec2(),
                    combat_profile_color(profile_id, Color::srgb(0.3, 0.8, 1.0)),
                    combat_profile_size(profile_id, Vec2::splat(24.0)),
                ),
                CombatCue::FighterDefeated { position, .. } => (
                    position.as_vec2(),
                    combat_profile_color(profile_id, Color::srgb(0.9, 0.05, 0.05)),
                    combat_profile_size(profile_id, Vec2::splat(64.0)),
                ),
                CombatCue::FighterReset { position, .. } | CombatCue::Reset { position, .. } => (
                    position.as_vec2(),
                    Color::srgb(0.2, 1.0, 0.4),
                    Vec2::splat(42.0),
                ),
                CombatCue::Muzzle { position, .. } => (
                    position.as_vec2(),
                    Color::srgb(1.0, 0.8, 0.2),
                    Vec2::splat(22.0),
                ),
                CombatCue::SentryFired { position, .. } => (
                    position.as_vec2(),
                    Color::srgb(0.25, 0.8, 1.0),
                    Vec2::new(20.0, 10.0),
                ),
                CombatCue::DeployableRemoved {
                    position, reason, ..
                } => (
                    position.as_vec2(),
                    if matches!(reason, crate::abilities::SentryCleanupReason::Destroyed) {
                        Color::srgb(1.0, 0.25, 0.1)
                    } else {
                        Color::srgb(0.35, 0.75, 1.0)
                    },
                    Vec2::splat(46.0),
                ),
                CombatCue::Damage { .. } | CombatCue::Defeat { .. } => {
                    continue;
                }
            };
            commands.spawn((
                CombatEffect {
                    timer: Timer::from_seconds(0.18, TimerMode::Once),
                },
                Sprite::from_color(color, size),
                Transform::from_translation(position.extend(30.0)),
            ));
        }
    }
}
