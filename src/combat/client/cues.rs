//! Combat cue ingestion and deduplication.

use super::{
    CombatCue, CombatEventId, DamageSource, DeduplicatedCombatCue, Fighter,
    MAX_COMBAT_EVIDENCE_EVENTS, NetworkEntityId, VecDeque, WeaponDefinitionId,
};
use bevy::input::gamepad::{Gamepad, GamepadRumbleIntensity, GamepadRumbleRequest};
use bevy::prelude::*;
use std::time::Duration;

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
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn receive_combat_cues(
    mut recent: ResMut<RecentCombatEvents>,
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
            if is_superseded_legacy_cue(&cue) {
                continue;
            }
            presented_cues.write(DeduplicatedCombatCue(cue.clone()));
        }
    }
}

/// Legacy cues that the typed cues replace for presentation. Environment damage (barrels, effect
/// tiles) has no typed `DamageApplied` counterpart, so its legacy `Damage` cue is presented
/// (BRL-75dh); consumers that only read typed cues ignore it.
#[cfg(feature = "client")]
pub(super) fn is_superseded_legacy_cue(cue: &CombatCue) -> bool {
    match cue {
        CombatCue::Damage { source, .. } => !matches!(source, DamageSource::Environment { .. }),
        CombatCue::Muzzle { .. }
        | CombatCue::Impact { .. }
        | CombatCue::Defeat { .. }
        | CombatCue::Reset { .. } => true,
        _ => false,
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
