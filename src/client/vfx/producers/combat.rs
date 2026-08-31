use bevy::prelude::*;

use super::VfxRequestSet;
use crate::{
    client::vfx::{
        COMBAT_DAMAGE_VFX, COMBAT_IMPACT_VFX, COMBAT_MUZZLE_VFX, COMBAT_RESET_VFX,
        COMBAT_VFX_PRODUCER_RANK, CombatVisualSignature, DEMOLITION_STRIKE_VFX,
        ELEMENTAL_FIELD_VFX, REVEAL_SCAN_VFX, VfxAccent, VfxAppExt, VfxAuthoritativeTiming,
        VfxCarrier, VfxContextOverride, VfxDeadline, VfxGeometry, VfxPhase,
        VfxPresentationIdentity, VfxPresentationVariant, VfxRelationship, VfxRequest,
        VfxRequestCapabilities, VfxRequestOrder, VfxRequestRegistration, VfxSignatureCache,
        accent_for_elemental_field,
    },
    combat::{AuthoritativeTick, CombatCue, client::DeduplicatedCombatCue, combat_cue_key},
    protocol::{Fighter, NetworkEntityId},
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
        app.init_resource::<VfxSignatureCache>();
        for registration in REGISTRATIONS {
            app.try_register_vfx_request(registration)
                .expect("combat VFX request registration must be valid");
        }
        app.add_systems(Update, produce_combat_vfx_requests.in_set(VfxRequestSet));
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the signature cache is a read-only Bevy system resource"
)]
fn produce_combat_vfx_requests(
    mut cues: MessageReader<DeduplicatedCombatCue>,
    mut requests: MessageWriter<VfxRequest>,
    authoritative_ticks: Query<&AuthoritativeTick>,
    cache: Res<VfxSignatureCache>,
    fighters: Query<(&NetworkEntityId, &crate::combat::TeamId), With<Fighter>>,
) {
    let observed_at_tick = authoritative_ticks.iter().map(|tick| tick.0).max();
    for DeduplicatedCombatCue(cue) in cues.read() {
        let Some((key, position, radius, deadline, label)) =
            combat_vfx_request(cue, observed_at_tick)
        else {
            continue;
        };
        let event_id = combat_cue_key(cue).event_id.0;
        let signature =
            combat_vfx_signature(cue, position, radius, observed_at_tick, &cache, &fighters);
        if let Ok(request) = VfxRequest::try_new(
            key,
            VfxRequestOrder::new(COMBAT_VFX_PRODUCER_RANK, event_id),
            position,
            radius,
            deadline,
            label,
        )
        .and_then(|request| request.with_signature(signature))
        {
            requests.write(request);
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the exhaustive combat-cue match is the single auditable semantic-signature adapter"
)]
fn combat_vfx_signature(
    cue: &CombatCue,
    position: Vec2,
    radius: Option<f32>,
    observed_at_tick: Option<u64>,
    cache: &VfxSignatureCache,
    fighters: &Query<(&NetworkEntityId, &crate::combat::TeamId), With<Fighter>>,
) -> CombatVisualSignature {
    use CombatCue as C;
    let point = VfxGeometry::point(position);
    let instant = |tick| VfxAuthoritativeTiming {
        observed_at_tick,
        ..VfxAuthoritativeTiming::instant(tick)
    };
    let direct = |carrier, phase, accents, relationship, context, geometry, identity, timing| {
        CombatVisualSignature {
            carrier,
            phase,
            accents,
            relationship,
            context,
            geometry,
            identity,
            timing,
            variant: VfxPresentationVariant::Normal,
        }
    };
    match cue {
        C::AttackAccepted {
            tick, attack_id, ..
        } => cache.signature_for_attack(
            *attack_id,
            0,
            VfxCarrier::PointTransient,
            VfxPhase::Activation,
            point,
            instant(*tick),
            VfxPresentationVariant::Normal,
        ),
        C::DeliveryImpact {
            tick,
            attack_id,
            delivery_index,
            normal,
            ..
        } => cache.signature_for_attack(
            *attack_id,
            *delivery_index,
            VfxCarrier::PointTransient,
            VfxPhase::Impact,
            VfxGeometry::Point {
                position: position.to_array(),
                normal: Some(normal.as_vec2().to_array()),
                direction: None,
            },
            instant(*tick),
            VfxPresentationVariant::Normal,
        ),
        C::LobLanded {
            tick,
            attack_id,
            delivery_index,
            ..
        }
        | C::MeleeContact {
            tick,
            attack_id,
            delivery_index,
            ..
        } => cache.signature_for_attack(
            *attack_id,
            *delivery_index,
            VfxCarrier::PointTransient,
            VfxPhase::Impact,
            point,
            instant(*tick),
            VfxPresentationVariant::Normal,
        ),
        C::DamageApplied {
            tick, attack_id, ..
        }
        | C::EffectApplied {
            tick, attack_id, ..
        }
        | C::FighterDefeated {
            tick, attack_id, ..
        } => cache.signature_for_attack(
            *attack_id,
            0,
            VfxCarrier::AttachedStatus,
            VfxPhase::Impact,
            point,
            instant(*tick),
            VfxPresentationVariant::Normal,
        ),
        C::SentryFired {
            tick,
            owner,
            deployable_id,
            ..
        } => direct(
            VfxCarrier::Deployable,
            VfxPhase::Activation,
            [Some(VfxAccent::Neutral), None],
            relationship_for_owner(cache, fighters, *owner),
            Some(VfxContextOverride::Sentry),
            point,
            VfxPresentationIdentity::Deployable(deployable_id.0),
            instant(*tick),
        ),
        C::DeployableRemoved {
            tick,
            owner,
            deployable_id,
            reason,
            ..
        } => direct(
            VfxCarrier::Deployable,
            if matches!(reason, crate::abilities::SentryCleanupReason::Expired) {
                VfxPhase::Expiry
            } else {
                VfxPhase::Interruption
            },
            [Some(VfxAccent::Neutral), None],
            relationship_for_owner(cache, fighters, *owner),
            Some(VfxContextOverride::Sentry),
            point,
            VfxPresentationIdentity::Deployable(deployable_id.0),
            instant(*tick),
        ),
        C::FighterReset {
            event_id,
            tick,
            target,
            ..
        } => direct(
            VfxCarrier::AttachedStatus,
            VfxPhase::Expiry,
            [Some(VfxAccent::Neutral), None],
            relationship_for_owner(cache, fighters, *target),
            None,
            point,
            VfxPresentationIdentity::CombatEvent(event_id.0),
            instant(*tick),
        ),
        C::RevealScanActivated {
            event_id,
            tick,
            revealing_team,
            expires_at_tick,
            ..
        } => direct(
            VfxCarrier::PersistentArea,
            VfxPhase::Activation,
            [Some(VfxAccent::Reveal), None],
            relationship_for_team(cache, *revealing_team),
            Some(VfxContextOverride::RevealScan),
            VfxGeometry::circle(position, radius.unwrap_or(1.0)),
            VfxPresentationIdentity::CombatEvent(event_id.0),
            VfxAuthoritativeTiming::persistent(*tick, observed_at_tick, *expires_at_tick),
        ),
        C::ElementalFieldActivated {
            tick,
            source,
            field_id,
            kind,
            expires_at_tick,
            ..
        } => direct(
            VfxCarrier::PersistentArea,
            VfxPhase::Activation,
            [Some(accent_for_elemental_field(*kind)), None],
            relationship_for_owner(cache, fighters, *source),
            Some(VfxContextOverride::ElementalField(*kind)),
            VfxGeometry::circle(position, radius.unwrap_or(1.0)),
            VfxPresentationIdentity::ReplicatedField(field_id.0),
            VfxAuthoritativeTiming::persistent(*tick, observed_at_tick, *expires_at_tick),
        ),
        C::DemolitionStrikeActivated {
            event_id,
            tick,
            source,
            ..
        } => direct(
            VfxCarrier::PersistentArea,
            VfxPhase::Activation,
            [Some(VfxAccent::Demolition), None],
            relationship_for_owner(cache, fighters, *source),
            Some(VfxContextOverride::DemolitionStrike),
            VfxGeometry::circle(position, radius.unwrap_or(1.0)),
            VfxPresentationIdentity::CombatEvent(event_id.0),
            instant(*tick),
        ),
        C::ConeSprayPulse {
            tick,
            attack_id,
            delivery_index,
            origin,
            facing,
            reached_distance,
            angle_degrees,
            ..
        } => cache.signature_for_attack(
            *attack_id,
            *delivery_index,
            VfxCarrier::Cone,
            VfxPhase::Pulse,
            VfxGeometry::Cone {
                origin: origin.as_vec2().to_array(),
                facing: *facing,
                reach: (*reached_distance).max(f32::EPSILON),
                angle_degrees: *angle_degrees,
            },
            instant(*tick),
            VfxPresentationVariant::Normal,
        ),
        _ => CombatVisualSignature::neutral_point(combat_cue_key(cue).event_id.0, position),
    }
}

fn relationship_for_owner(
    cache: &VfxSignatureCache,
    fighters: &Query<(&NetworkEntityId, &crate::combat::TeamId), With<Fighter>>,
    owner: NetworkEntityId,
) -> VfxRelationship {
    fighters
        .iter()
        .find_map(|(network_id, team)| {
            (*network_id == owner).then(|| cache.observer().relationship(owner, *team))
        })
        .unwrap_or(VfxRelationship::Neutral)
}

fn relationship_for_team(
    cache: &VfxSignatureCache,
    team: crate::combat::TeamId,
) -> VfxRelationship {
    let observer = cache.observer();
    if team.0 == 0 || observer.team_id.is_none() {
        VfxRelationship::Neutral
    } else if observer.team_id == Some(team) {
        VfxRelationship::Ally
    } else {
        VfxRelationship::Enemy
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
