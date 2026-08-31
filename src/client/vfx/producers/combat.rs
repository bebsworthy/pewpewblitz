use bevy::prelude::*;

use super::VfxRequestSet;
use crate::{
    client::vfx::{
        COMBAT_DAMAGE_VFX, COMBAT_IMPACT_VFX, COMBAT_MUZZLE_VFX, COMBAT_RESET_VFX,
        COMBAT_VFX_PRODUCER_RANK, DEMOLITION_STRIKE_VFX, ELEMENTAL_FIELD_VFX, REVEAL_SCAN_VFX,
        VfxAppExt, VfxDeadline, VfxRequest, VfxRequestCapabilities, VfxRequestOrder,
        VfxRequestRegistration,
    },
    combat::{AuthoritativeTick, CombatCue, client::DeduplicatedCombatCue, combat_cue_key},
};

const REGISTRATIONS: [VfxRequestRegistration; 7] = [
    VfxRequestRegistration::new(
        COMBAT_MUZZLE_VFX,
        COMBAT_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::NONE,
    ),
    VfxRequestRegistration::new(
        COMBAT_IMPACT_VFX,
        COMBAT_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::NONE,
    ),
    VfxRequestRegistration::new(
        COMBAT_DAMAGE_VFX,
        COMBAT_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::NONE,
    ),
    VfxRequestRegistration::new(
        COMBAT_RESET_VFX,
        COMBAT_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::NONE,
    ),
    VfxRequestRegistration::new(
        REVEAL_SCAN_VFX,
        COMBAT_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::RADIUS_AND_DEADLINE,
    ),
    VfxRequestRegistration::new(
        ELEMENTAL_FIELD_VFX,
        COMBAT_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::RADIUS,
    ),
    VfxRequestRegistration::new(
        DEMOLITION_STRIKE_VFX,
        COMBAT_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::RADIUS,
    ),
];

pub(crate) struct CombatVfxProducerPlugin;

impl Plugin for CombatVfxProducerPlugin {
    fn build(&self, app: &mut App) {
        for registration in REGISTRATIONS {
            app.try_register_vfx_request(registration)
                .expect("combat VFX request registration must be valid");
        }
        app.add_systems(Update, produce_combat_vfx_requests.in_set(VfxRequestSet));
    }
}

fn produce_combat_vfx_requests(
    mut cues: MessageReader<DeduplicatedCombatCue>,
    mut requests: MessageWriter<VfxRequest>,
    authoritative_ticks: Query<&AuthoritativeTick>,
) {
    let observed_at_tick = authoritative_ticks.iter().map(|tick| tick.0).max();
    for DeduplicatedCombatCue(cue) in cues.read() {
        let Some((key, position, radius, deadline, label)) =
            combat_vfx_request(cue, observed_at_tick)
        else {
            continue;
        };
        let event_id = combat_cue_key(cue).event_id.0;
        if let Ok(request) = VfxRequest::try_new(
            key,
            VfxRequestOrder::new(COMBAT_VFX_PRODUCER_RANK, event_id),
            position,
            radius,
            deadline,
            label,
        ) {
            requests.write(request);
        }
    }
}

#[allow(
    clippy::type_complexity,
    reason = "the tuple is the compact renderer-neutral VFX request payload"
)]
fn combat_vfx_request(
    cue: &CombatCue,
    observed_at_tick: Option<u64>,
) -> Option<(
    crate::client::vfx::VfxRequestKey,
    Vec2,
    Option<f32>,
    Option<VfxDeadline>,
    &'static str,
)> {
    use CombatCue as C;
    match cue {
        C::AttackAccepted { position, .. } | C::SentryFired { position, .. } => Some((
            COMBAT_MUZZLE_VFX,
            position.as_vec2(),
            None,
            None,
            "V3 bounded combat cue effect",
        )),
        C::DeliveryImpact { position, .. }
        | C::LobLanded { position, .. }
        | C::MeleeContact { position, .. }
        | C::DeployableRemoved { position, .. } => Some((
            COMBAT_IMPACT_VFX,
            position.as_vec2(),
            None,
            None,
            "V3 bounded combat cue effect",
        )),
        C::DamageApplied { position, .. }
        | C::EffectApplied { position, .. }
        | C::FighterDefeated { position, .. } => Some((
            COMBAT_DAMAGE_VFX,
            position.as_vec2(),
            None,
            None,
            "V3 bounded combat cue effect",
        )),
        C::FighterReset { position, .. } => Some((
            COMBAT_RESET_VFX,
            position.as_vec2(),
            None,
            None,
            "V3 bounded combat cue effect",
        )),
        C::RevealScanActivated {
            event_id: _,
            tick,
            center,
            radius_milliunits,
            expires_at_tick,
            ..
        } => Some((
            REVEAL_SCAN_VFX,
            center.as_vec2(),
            crate::builds::world_units_from_milliunits(*radius_milliunits),
            Some(VfxDeadline::new(*tick, *expires_at_tick, observed_at_tick)),
            "V9 active Reveal Scan area",
        )),
        C::ElementalFieldActivated {
            center,
            radius_milliunits,
            ..
        } => Some((
            ELEMENTAL_FIELD_VFX,
            center.as_vec2(),
            crate::builds::world_units_from_milliunits(*radius_milliunits),
            None,
            "V3 bounded combat cue effect",
        )),
        C::DemolitionStrikeActivated {
            center,
            radius_milliunits,
            ..
        } => Some((
            DEMOLITION_STRIKE_VFX,
            center.as_vec2(),
            crate::builds::world_units_from_milliunits(*radius_milliunits),
            None,
            "V3 bounded combat cue effect",
        )),
        C::Muzzle { .. }
        | C::ConeSprayPulse { .. }
        | C::Impact { .. }
        | C::Damage { .. }
        | C::Defeat { .. }
        | C::Reset { .. }
        | C::SelfCloakActivated { .. }
        | C::SelfCloakEnded { .. }
        | C::ForcedRevealApplied { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        combat::{AttackId, CombatEventId, WeaponDefinitionId, WorldPoint},
        protocol::NetworkEntityId,
    };

    #[test]
    fn built_in_combat_keys_have_one_owner_and_expected_capabilities() {
        assert_eq!(REGISTRATIONS.len(), 7);
        assert!(
            REGISTRATIONS
                .iter()
                .all(|entry| entry.producer_rank == COMBAT_VFX_PRODUCER_RANK)
        );
        assert_eq!(
            REGISTRATIONS
                .iter()
                .filter(|entry| entry.capabilities.authoritative_radius)
                .count(),
            3
        );
        assert_eq!(
            REGISTRATIONS
                .iter()
                .filter(|entry| entry.capabilities.authoritative_deadline)
                .count(),
            1
        );
    }

    #[test]
    fn combat_mapping_preserves_muzzle_scan_and_legacy_omission() {
        let attack = CombatCue::AttackAccepted {
            event_id: CombatEventId(9),
            tick: 10,
            attack_id: AttackId(11),
            source: NetworkEntityId(12),
            position: WorldPoint { x: 2.0, y: 3.0 },
            weapon_definition_id: WeaponDefinitionId(13),
        };
        let (key, position, radius, deadline, _) = combat_vfx_request(&attack, None).unwrap();
        assert_eq!(key, COMBAT_MUZZLE_VFX);
        assert_eq!(position, Vec2::new(2.0, 3.0));
        assert_eq!(radius, None);
        assert_eq!(deadline, None);

        let scan = CombatCue::RevealScanActivated {
            event_id: CombatEventId(14),
            tick: 20,
            revealing_team: crate::combat::TeamId(1),
            center: WorldPoint { x: 4.0, y: 5.0 },
            radius_milliunits: 6_000,
            expires_at_tick: 30,
        };
        let (key, _, radius, deadline, _) = combat_vfx_request(&scan, Some(22)).unwrap();
        assert_eq!(key, REVEAL_SCAN_VFX);
        assert_eq!(radius, Some(6.0));
        assert_eq!(deadline, Some(VfxDeadline::new(20, 30, Some(22))));

        let legacy = CombatCue::Muzzle {
            event_id: CombatEventId(15),
            tick: 21,
            source: NetworkEntityId(12),
            shot_id: crate::combat::ShotId(16),
            weapon_definition_id: WeaponDefinitionId(13),
            position: WorldPoint { x: 2.0, y: 3.0 },
        };
        assert!(combat_vfx_request(&legacy, None).is_none());
    }
}
