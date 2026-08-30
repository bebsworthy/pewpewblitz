//! Combat cue ingestion and deduplication.

#![allow(clippy::wildcard_imports)]
use super::*;
use bevy::input::gamepad::{Gamepad, GamepadRumbleIntensity, GamepadRumbleRequest};
use std::time::Duration;
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
#[cfg(feature = "client")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn receive_combat_cues(
    mut recent: ResMut<RecentCombatEvents>,
    mut observation: ResMut<ClientCombatObservation>,
    mut capture: Option<ResMut<CaptureCombatCues>>,
    mut presented_cues: MessageWriter<DeduplicatedCombatCue>,
    mut receivers: Query<
        Option<&mut lightyear::prelude::MessageReceiver<CombatCue>>,
        With<lightyear::prelude::client::Client>,
    >,
) {
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
                | CombatCue::ConeSprayPulse { event_id, .. }
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
                | CombatCue::Reset { event_id, .. }
                | CombatCue::SelfCloakActivated { event_id, .. }
                | CombatCue::SelfCloakEnded { event_id, .. }
                | CombatCue::RevealScanActivated { event_id, .. }
                | CombatCue::DemolitionStrikeActivated { event_id, .. }
                | CombatCue::ElementalFieldActivated { event_id, .. }
                | CombatCue::ForcedRevealApplied { event_id, .. } => *event_id,
            };
            if !remember_combat_event(&mut recent, event_id) {
                continue;
            }
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
        }
    }
}

#[cfg(feature = "client")]
pub(crate) fn rumble_spray_feedback(
    mut cues: MessageReader<DeduplicatedCombatCue>,
    controlled: Query<&NetworkEntityId, (With<Fighter>, With<lightyear::prelude::Controlled>)>,
    gamepads: Query<Entity, With<Gamepad>>,
    mut requests: Option<MessageWriter<GamepadRumbleRequest>>,
) {
    let Some(requests) = requests.as_mut() else {
        return;
    };
    let controlled_ids = controlled.iter().copied().collect::<Vec<_>>();
    for DeduplicatedCombatCue(cue) in cues.read() {
        let intensity = match cue {
            CombatCue::AttackAccepted {
                source,
                weapon_definition_id: WeaponDefinitionId(6),
                ..
            } if controlled_ids.contains(source) => Some((0.08, 0.28, 0.08)),
            CombatCue::DamageApplied {
                target,
                source:
                    DamageSource::PlayerWeapon {
                        weapon_definition_id: WeaponDefinitionId(6),
                        ..
                    },
                ..
            } if controlled_ids.contains(target) => Some((0.16, 0.34, 0.07)),
            _ => None,
        };
        let Some((strong_motor, weak_motor, seconds)) = intensity else {
            continue;
        };
        for gamepad in &gamepads {
            requests.write(GamepadRumbleRequest::Add {
                gamepad,
                intensity: GamepadRumbleIntensity {
                    strong_motor,
                    weak_motor,
                },
                duration: Duration::from_secs_f32(seconds),
            });
        }
    }
}
