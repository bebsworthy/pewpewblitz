use bevy::prelude::*;
use lightyear::prelude::Controlled;

use crate::{
    ClientShellSettings,
    audio::{
        registry::{AudioProducerRegistration, AudioProducerRegistrationAppExt},
        request::{AudioCueKey, AudioRequest, AudioRequestOrder, cue_keys},
    },
    local_attack::{LocalAttackKind, LocalAttackLedger, LocalAttackPresented},
};
use brawler::{
    builds::{ResolvedMatchLoadout, UltimateKind},
    combat::{CombatCue, DeduplicatedCombatCue, DeliveryMethod},
    protocol::{Fighter, NetworkEntityId},
};

use super::AudioProducerSequence;

const COMBAT_RANK: u16 = 10;
const COMBAT_KEYS: &[AudioCueKey] = &[
    cue_keys::FIRE_STRAIGHT,
    cue_keys::FIRE_STICKY,
    cue_keys::FIRE_LOBBED,
    cue_keys::FIRE_SPLASH,
    cue_keys::FIRE_MELEE,
    cue_keys::FIRE_SPRAY,
    cue_keys::IMPACT_PROJECTILE,
    cue_keys::IMPACT_LOB,
    cue_keys::IMPACT_MELEE,
    cue_keys::IMPACT_SPRAY,
    cue_keys::HIT_DEALT,
    cue_keys::HIT_TAKEN,
    cue_keys::DEFEAT,
    cue_keys::RESPAWN,
    cue_keys::SENTRY_FIRE,
    cue_keys::SENTRY_REMOVED,
    cue_keys::ULTIMATE_CLOAK,
    cue_keys::ULTIMATE_UNCLOAK,
    cue_keys::ULTIMATE_SCAN,
    cue_keys::ULTIMATE_REVEALED,
    cue_keys::ULTIMATE_FIELD,
    cue_keys::ULTIMATE_STRIKE,
];

/// Minimum authoritative ticks between two hit confirmations of the same kind, so a spray or a
/// multi-target splash reads as one confirmation instead of a buzz.
const HIT_SPACING_TICKS: u64 = 4;
/// Reduced combat effects keep hit confirmations (they are required feedback) but thin repeats.
const REDUCED_HIT_SPACING_TICKS: u64 = 12;

pub(super) fn register(app: &mut App) {
    app.add_message::<LocalAttackPresented>();
    app.try_register_audio_producer(AudioProducerRegistration {
        id: "combat",
        rank: COMBAT_RANK,
        cue_keys: COMBAT_KEYS,
    })
    .expect("combat audio producer registration must be valid");
}

/// Fire-sound occurrence of a locally presented shot; disjoint from server attack ids.
const LOCAL_ATTACK_AUDIO_OCCURRENCE: u64 = 1 << 63;
/// Activation-sound occurrence of a locally presented ultimate cast; disjoint from server event
/// ids and local shots.
pub(super) const LOCAL_ULTIMATE_AUDIO_OCCURRENCE: u64 = 3 << 62;

/// The combat producer's activation sound of a locally cast ultimate, when it is a launch.
/// Sentry and Concealment Field spawn sounds belong to the ability producer.
const fn local_ultimate_activation_cue(kind: UltimateKind) -> Option<AudioCueKey> {
    if !crate::local_attack::presents_activation_sound_locally(kind) {
        return None;
    }
    match kind {
        UltimateKind::SelfCloak => Some(cue_keys::ULTIMATE_CLOAK),
        UltimateKind::RevealScan => Some(cue_keys::ULTIMATE_SCAN),
        UltimateKind::CryogenicField
        | UltimateKind::FireField
        | UltimateKind::PoisonField
        | UltimateKind::RestorationField => Some(cue_keys::ULTIMATE_FIELD),
        UltimateKind::Dash
        | UltimateKind::Sentry
        | UltimateKind::ConcealmentField
        | UltimateKind::DemolitionStrike
        | UltimateKind::BigBlob => None,
    }
}

/// Presentation-only memory for hit-confirmation spacing; never gameplay truth.
pub(super) struct HitAudioMemory {
    last_dealt_tick: Option<u64>,
    last_taken_tick: Option<u64>,
    spacing_ticks: u64,
}

impl Default for HitAudioMemory {
    fn default() -> Self {
        Self {
            last_dealt_tick: None,
            last_taken_tick: None,
            spacing_ticks: HIT_SPACING_TICKS,
        }
    }
}

fn hit_spacing_ticks(settings: Option<&ClientShellSettings>) -> u64 {
    if settings.is_some_and(|settings| settings.reduced_combat_effects) {
        REDUCED_HIT_SPACING_TICKS
    } else {
        HIT_SPACING_TICKS
    }
}

type CombatAudioFighters<'w, 's> = Query<
    'w,
    's,
    (
        &'static NetworkEntityId,
        &'static ResolvedMatchLoadout,
        Has<Controlled>,
    ),
    With<Fighter>,
>;

/// Plays the local player's own fire sound and launch-type ultimate activation sound on its
/// input frame and every other combat cue as it arrives. The authoritative `AttackAccepted` echo
/// of a locally presented shot and the activation cue echo of a locally presented cast are
/// skipped.
/// Fire sounds follow the shooter's replicated weapon delivery family; hit confirmations are
/// relative to the controlled fighter and spaced further apart under reduced combat effects.
#[allow(
    clippy::needless_pass_by_value,
    reason = "the ledger is a read-only Bevy system resource"
)]
pub(super) fn produce_combat_audio_requests(
    mut local_attacks: MessageReader<LocalAttackPresented>,
    mut cues: MessageReader<DeduplicatedCombatCue>,
    ledger: Option<Res<LocalAttackLedger>>,
    fighters: CombatAudioFighters,
    settings: Option<Res<ClientShellSettings>>,
    mut requests: MessageWriter<AudioRequest>,
    mut memory: Local<(AudioProducerSequence, HitAudioMemory)>,
) {
    let (sequence, hits) = &mut *memory;
    hits.spacing_ticks = hit_spacing_ticks(settings.as_deref());
    for local in local_attacks.read() {
        match local.kind {
            LocalAttackKind::PrimaryFire(shot) => {
                let delivery = fighters
                    .iter()
                    .find(|(network_id, ..)| **network_id == local.owner)
                    .map(|(_, loadout, _)| loadout.primary_weapon.recipe.delivery);
                requests.write(AudioRequest::for_occurrence(
                    fire_cue(delivery),
                    LOCAL_ATTACK_AUDIO_OCCURRENCE | shot.0,
                    sequence.next(COMBAT_RANK),
                ));
            }
            LocalAttackKind::Ultimate { kind, cast } => {
                if let Some(cue_key) = local_ultimate_activation_cue(kind) {
                    requests.write(AudioRequest::for_occurrence(
                        cue_key,
                        LOCAL_ULTIMATE_AUDIO_OCCURRENCE | cast.0,
                        sequence.next(COMBAT_RANK),
                    ));
                }
            }
        }
    }
    let controlled = fighters
        .iter()
        .filter(|(.., controlled)| *controlled)
        .map(|(network_id, ..)| *network_id)
        .min_by_key(|network_id| network_id.0);
    for DeduplicatedCombatCue(cue) in cues.read() {
        if ledger
            .as_deref()
            .is_some_and(|ledger| is_local_presentation_echo(ledger, cue))
        {
            continue;
        }
        let delivery = |source: NetworkEntityId| {
            fighters
                .iter()
                .find(|(network_id, ..)| **network_id == source)
                .map(|(_, loadout, _)| loadout.primary_weapon.recipe.delivery)
        };
        let request =
            combat_audio_request(cue, controlled, delivery, hits, sequence.next(COMBAT_RANK));
        if let Some(request) = request {
            requests.write(request);
        }
    }
}

/// Whether this cue's sound already played on the local input frame.
fn is_local_presentation_echo(ledger: &LocalAttackLedger, cue: &CombatCue) -> bool {
    match cue {
        CombatCue::AttackAccepted { attack_id, .. } => ledger.is_echo(*attack_id),
        CombatCue::SelfCloakActivated { event_id, .. }
        | CombatCue::RevealScanActivated { event_id, .. }
        | CombatCue::ElementalFieldActivated { event_id, .. } => {
            ledger.is_activation_echo_cue(*event_id)
        }
        _ => false,
    }
}

/// The fire sound of one weapon delivery family. An unknown shooter (not yet replicated, or a
/// concealed source) falls back to the straight-shot sound rather than staying silent.
const fn fire_cue(delivery: Option<DeliveryMethod>) -> AudioCueKey {
    match delivery {
        Some(DeliveryMethod::Straight { .. }) | None => cue_keys::FIRE_STRAIGHT,
        Some(DeliveryMethod::StickyStraight { .. }) => cue_keys::FIRE_STICKY,
        Some(DeliveryMethod::Lobbed { .. }) => cue_keys::FIRE_LOBBED,
        Some(DeliveryMethod::Splash { .. }) => cue_keys::FIRE_SPLASH,
        Some(DeliveryMethod::MeleeArc { .. }) => cue_keys::FIRE_MELEE,
        Some(DeliveryMethod::ConeSpray { .. }) => cue_keys::FIRE_SPRAY,
    }
}

fn combat_audio_request(
    cue: &CombatCue,
    controlled: Option<NetworkEntityId>,
    delivery: impl Fn(NetworkEntityId) -> Option<DeliveryMethod>,
    hits: &mut HitAudioMemory,
    order: AudioRequestOrder,
) -> Option<AudioRequest> {
    let (cue_key, occurrence) = match cue {
        CombatCue::AttackAccepted {
            attack_id, source, ..
        } => (fire_cue(delivery(*source)), attack_id.0),
        CombatCue::DeliveryImpact { attack_id, .. } => (cue_keys::IMPACT_PROJECTILE, attack_id.0),
        CombatCue::LobLanded { attack_id, .. } => (cue_keys::IMPACT_LOB, attack_id.0),
        CombatCue::MeleeContact { attack_id, .. } => (cue_keys::IMPACT_MELEE, attack_id.0),
        CombatCue::ConeSprayPulse { event_id, .. } => (cue_keys::IMPACT_SPRAY, event_id.0),
        CombatCue::DamageApplied {
            attack_id,
            tick,
            source,
            target,
            ..
        } => {
            let controlled = controlled?;
            // Taking damage outranks dealing it when both apply (self-inflicted area damage).
            let (cue_key, last) = if *target == controlled {
                (cue_keys::HIT_TAKEN, &mut hits.last_taken_tick)
            } else if source.initiating_fighter() == Some(controlled) {
                (cue_keys::HIT_DEALT, &mut hits.last_dealt_tick)
            } else {
                return None;
            };
            if last.is_some_and(|last| *tick < last.saturating_add(hits.spacing_ticks)) {
                return None;
            }
            *last = Some(*tick);
            // One confirmation per attack even when it reaches several targets.
            (cue_key, attack_id.0)
        }
        // One attack can defeat several fighters; every defeat plays its own sound.
        CombatCue::FighterDefeated { event_id, .. } => (cue_keys::DEFEAT, event_id.0),
        CombatCue::FighterReset { event_id, .. } => (cue_keys::RESPAWN, event_id.0),
        CombatCue::SentryFired { event_id, .. } => (cue_keys::SENTRY_FIRE, event_id.0),
        CombatCue::DeployableRemoved { event_id, .. } => (cue_keys::SENTRY_REMOVED, event_id.0),
        CombatCue::SelfCloakActivated { event_id, .. } => (cue_keys::ULTIMATE_CLOAK, event_id.0),
        CombatCue::SelfCloakEnded { event_id, .. } => (cue_keys::ULTIMATE_UNCLOAK, event_id.0),
        CombatCue::RevealScanActivated { event_id, .. } => (cue_keys::ULTIMATE_SCAN, event_id.0),
        CombatCue::ForcedRevealApplied { event_id, .. } => {
            (cue_keys::ULTIMATE_REVEALED, event_id.0)
        }
        CombatCue::ElementalFieldActivated { event_id, .. } => {
            (cue_keys::ULTIMATE_FIELD, event_id.0)
        }
        CombatCue::DemolitionStrikeActivated { event_id, .. } => {
            (cue_keys::ULTIMATE_STRIKE, event_id.0)
        }
        CombatCue::EffectApplied { .. }
        | CombatCue::Muzzle { .. }
        | CombatCue::Impact { .. }
        | CombatCue::Damage { .. }
        | CombatCue::Defeat { .. }
        | CombatCue::Reset { .. } => return None,
    };
    Some(AudioRequest::for_occurrence(cue_key, occurrence, order))
}

#[cfg(test)]
mod tests {
    use super::*;
    use brawler::{
        abilities::SentryCleanupReason,
        builds::DeployableId,
        combat::{
            AttackId, CombatEventId, DamageSource, DistanceBand, ShotId, WeaponDefinitionId,
            WorldPoint,
        },
        protocol::PlayerId,
    };

    const ORDER: AudioRequestOrder = AudioRequestOrder::new(COMBAT_RANK, 0);

    fn request(cue: &CombatCue, controlled: Option<NetworkEntityId>) -> Option<AudioRequest> {
        combat_audio_request(
            cue,
            controlled,
            |_| None,
            &mut HitAudioMemory::default(),
            ORDER,
        )
    }

    #[test]
    fn sentry_fire_and_removal_keep_their_event_occurrences() {
        let sentry_fire = CombatCue::SentryFired {
            event_id: CombatEventId(92),
            tick: 100,
            owner: NetworkEntityId(7),
            deployable_id: DeployableId(3),
            target: Some(NetworkEntityId(8)),
            position: WorldPoint { x: 20.0, y: 30.0 },
        };
        let removal = CombatCue::DeployableRemoved {
            event_id: CombatEventId(93),
            tick: 101,
            owner: NetworkEntityId(7),
            deployable_id: DeployableId(3),
            position: WorldPoint { x: 20.0, y: 30.0 },
            reason: SentryCleanupReason::Destroyed,
        };

        assert_eq!(
            request(&sentry_fire, None),
            Some(AudioRequest::for_occurrence(
                cue_keys::SENTRY_FIRE,
                92,
                ORDER
            ))
        );
        assert_eq!(
            request(&removal, None),
            Some(AudioRequest::for_occurrence(
                cue_keys::SENTRY_REMOVED,
                93,
                ORDER
            ))
        );
    }

    #[test]
    fn every_weapon_delivery_family_selects_its_own_fire_sound() {
        // Splash (FIRE_SPLASH) carries an authored area shape and is covered by the exhaustive
        // match in `fire_cue`.
        let all = [
            fire_cue(Some(DeliveryMethod::Straight {
                speed: 1.0,
                radius: 1.0,
                range: 1.0,
                lifetime_ticks: 1,
                muzzle_offset: 0.0,
            })),
            fire_cue(Some(DeliveryMethod::StickyStraight {
                speed: 1.0,
                radius: 1.0,
                range: 1.0,
                lifetime_ticks: 1,
                muzzle_offset: 0.0,
                fuse_ticks: 1,
                max_active_per_owner: 1,
            })),
            fire_cue(Some(DeliveryMethod::Lobbed {
                distance: 1.0,
                max_flight_ticks: 1,
                visual_arc_height: 1.0,
                landing_clearance_radius: 1.0,
                muzzle_offset: 0.0,
            })),
            fire_cue(Some(DeliveryMethod::MeleeArc {
                reach: 1.0,
                angle_degrees: 1.0,
            })),
            fire_cue(Some(DeliveryMethod::ConeSpray {
                propagation_speed: 1.0,
                reach: 1.0,
                angle_degrees: 1.0,
                linger_ticks: 1,
                pulse_interval_ticks: 1,
                map_occlusion: false,
                max_targets: 1,
            })),
        ];
        assert_eq!(
            all,
            [
                cue_keys::FIRE_STRAIGHT,
                cue_keys::FIRE_STICKY,
                cue_keys::FIRE_LOBBED,
                cue_keys::FIRE_MELEE,
                cue_keys::FIRE_SPRAY,
            ]
        );
        assert_eq!(fire_cue(None), cue_keys::FIRE_STRAIGHT);
    }

    #[test]
    fn remote_fire_uses_the_shooters_replicated_delivery_family() {
        let accepted = CombatCue::AttackAccepted {
            event_id: CombatEventId(1),
            tick: 10,
            attack_id: AttackId(44),
            source: NetworkEntityId(9),
            position: WorldPoint { x: 0.0, y: 0.0 },
            weapon_definition_id: WeaponDefinitionId(1),
        };
        let melee = DeliveryMethod::MeleeArc {
            reach: 1.0,
            angle_degrees: 90.0,
        };
        let request = combat_audio_request(
            &accepted,
            None,
            |source| (source == NetworkEntityId(9)).then_some(melee),
            &mut HitAudioMemory::default(),
            ORDER,
        );
        assert_eq!(
            request,
            Some(AudioRequest::for_occurrence(
                cue_keys::FIRE_MELEE,
                44,
                ORDER
            ))
        );
    }

    fn damage(tick: u64, attack: u64, shooter: u64, target: u64) -> CombatCue {
        CombatCue::DamageApplied {
            event_id: CombatEventId(tick * 10 + target),
            tick,
            attack_id: AttackId(attack),
            source: DamageSource::PlayerWeapon {
                player_id: PlayerId(shooter),
                fighter_id: NetworkEntityId(shooter),
                weapon_definition_id: WeaponDefinitionId(1),
                shot_id: ShotId(attack),
            },
            target: NetworkEntityId(target),
            position: WorldPoint { x: 0.0, y: 0.0 },
            amount: 10,
            health_after: 90,
            distance_band: DistanceBand::Close,
        }
    }

    #[test]
    fn damage_confirms_hits_dealt_and_taken_relative_to_the_controlled_fighter_only() {
        let me = Some(NetworkEntityId(1));
        let mut memory = HitAudioMemory::default();
        let mut hit = |cue: &CombatCue| combat_audio_request(cue, me, |_| None, &mut memory, ORDER);

        assert_eq!(
            hit(&damage(100, 7, 1, 2)).map(|request| request.cue_key),
            Some(cue_keys::HIT_DEALT)
        );
        assert_eq!(
            hit(&damage(100, 8, 2, 1)).map(|request| request.cue_key),
            Some(cue_keys::HIT_TAKEN)
        );
        // Spacing keeps a spray or multi-target splash to one confirmation per window.
        assert_eq!(hit(&damage(102, 7, 1, 3)), None);
        assert_eq!(hit(&damage(103, 9, 2, 1)), None);
        assert_eq!(
            hit(&damage(104, 10, 1, 2)).map(|request| request.cue_key),
            Some(cue_keys::HIT_DEALT)
        );
        // Bystander damage is silent, and nothing plays before a fighter is controlled.
        assert_eq!(hit(&damage(200, 11, 2, 3)), None);
        assert_eq!(request(&damage(300, 12, 1, 2), None), None);
    }

    #[test]
    fn reduced_combat_effects_thin_hit_confirmations_without_muting_them() {
        let reduced = ClientShellSettings {
            reduced_combat_effects: true,
            ..ClientShellSettings::default()
        };
        assert_eq!(hit_spacing_ticks(None), HIT_SPACING_TICKS);
        assert_eq!(
            hit_spacing_ticks(Some(&ClientShellSettings::default())),
            HIT_SPACING_TICKS
        );
        let me = Some(NetworkEntityId(1));
        let mut memory = HitAudioMemory {
            spacing_ticks: hit_spacing_ticks(Some(&reduced)),
            ..HitAudioMemory::default()
        };
        let mut hit = |cue: &CombatCue| combat_audio_request(cue, me, |_| None, &mut memory, ORDER);
        assert_eq!(
            hit(&damage(100, 7, 1, 2)).map(|request| request.cue_key),
            Some(cue_keys::HIT_DEALT)
        );
        // A hit the normal spacing would confirm is thinned under reduced effects.
        assert_eq!(hit(&damage(104, 8, 1, 2)), None);
        assert_eq!(hit(&damage(111, 9, 1, 2)), None);
        assert_eq!(
            hit(&damage(112, 10, 1, 2)).map(|request| request.cue_key),
            Some(cue_keys::HIT_DEALT)
        );
        assert_eq!(
            hit(&damage(113, 11, 2, 1)).map(|request| request.cue_key),
            Some(cue_keys::HIT_TAKEN)
        );
    }
}
