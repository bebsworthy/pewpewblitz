use bevy::prelude::*;

use super::VfxRequestSet;
use crate::{
    local_attack::{LocalAttackKind, LocalAttackLedger, LocalAttackPresented},
    vfx::{
        COMBAT_DAMAGE_VFX, COMBAT_IMPACT_VFX, COMBAT_MUZZLE_VFX, COMBAT_RESET_VFX,
        COMBAT_VFX_PRODUCER_RANK, CombatVisualSignature, DEMOLITION_STRIKE_VFX,
        ELEMENTAL_FIELD_VFX, FORCED_REVEAL_VFX, REVEAL_SCAN_VFX, SELF_CLOAK_VFX,
        SENTRY_MUZZLE_PRESENTATION_OFFSET, VfxAccent, VfxAppExt, VfxAuthoritativeTiming,
        VfxCarrier, VfxContextOverride, VfxDeadline, VfxGeometry, VfxPhase,
        VfxPresentationIdentity, VfxPresentationVariant, VfxRelationship, VfxRequest,
        VfxRequestCapabilities, VfxRequestOrder, VfxRequestRegistration, VfxSignatureCache,
        accent_for_elemental_field, activation_geometry_for_recipe, carrier_for_delivery,
    },
};
use brawler::{
    combat::{AuthoritativeTick, CombatCue, DeduplicatedCombatCue, combat_cue_key},
    protocol::{Fighter, NetworkEntityId},
};

const REGISTRATIONS: [VfxRequestRegistration; 9] = [
    VfxRequestRegistration::new(
        COMBAT_MUZZLE_VFX,
        COMBAT_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::NONE,
    ),
    VfxRequestRegistration::new(
        COMBAT_IMPACT_VFX,
        COMBAT_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::RADIUS,
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
        SELF_CLOAK_VFX,
        COMBAT_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::NONE,
    ),
    VfxRequestRegistration::new(
        FORCED_REVEAL_VFX,
        COMBAT_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::NONE,
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
        app.add_message::<LocalAttackPresented>().add_systems(
            Update,
            (
                produce_combat_vfx_requests,
                produce_local_attack_vfx_requests,
            )
                .in_set(VfxRequestSet),
        );
    }
}

/// Request identity of locally presented attacks; disjoint from server combat event ids.
const LOCAL_ATTACK_VFX_IDENTITY: u64 = 1 << 63;
/// Request identity of locally presented ultimate casts; disjoint from shots and server events.
const LOCAL_ULTIMATE_VFX_IDENTITY: u64 = 3 << 62;

/// Accent of the launch flash at the caster of a locally presented ultimate.
const fn ultimate_launch_accent(kind: brawler::builds::UltimateKind) -> VfxAccent {
    use brawler::builds::UltimateKind as K;
    use brawler::combat::ElementalFieldKind as E;
    match kind {
        K::CryogenicField => accent_for_elemental_field(E::Cryogenic),
        K::FireField => accent_for_elemental_field(E::Fire),
        K::PoisonField => accent_for_elemental_field(E::Poison),
        K::RestorationField => accent_for_elemental_field(E::Restoration),
        K::RevealScan => VfxAccent::Reveal,
        K::DemolitionStrike => VfxAccent::Demolition,
        K::SelfCloak | K::ConcealmentField => VfxAccent::Concealment,
        K::Dash | K::Sentry | K::BigBlob => VfxAccent::Neutral,
    }
}

/// Short launch flash at the caster of a locally presented ultimate. It is a bounded transient
/// with no authoritative counterpart: the area outcome effects stay cue-driven.
fn local_ultimate_launch_request(
    local: &LocalAttackPresented,
    kind: brawler::builds::UltimateKind,
    cast: crate::local_attack::LocalCastId,
    ultimate_id: brawler::builds::UltimateDefinitionId,
    cache: &VfxSignatureCache,
) -> Option<VfxRequest> {
    let identity = LOCAL_ULTIMATE_VFX_IDENTITY | cast.0;
    let signature = CombatVisualSignature {
        carrier: VfxCarrier::PointTransient,
        phase: VfxPhase::Activation,
        accents: [Some(ultimate_launch_accent(kind)), None],
        relationship: cache.observer().relationship(local.owner, local.team),
        context: Some(VfxContextOverride::Ultimate(ultimate_id)),
        geometry: VfxGeometry::Point {
            position: local.origin.to_array(),
            normal: None,
            direction: Some(Vec2::from_angle(local.facing).to_array()),
        },
        identity: VfxPresentationIdentity::Neutral(identity),
        timing: VfxAuthoritativeTiming::instant(local.simulation_tick),
        variant: VfxPresentationVariant::Normal,
    };
    VfxRequest::try_new(
        COMBAT_MUZZLE_VFX,
        VfxRequestOrder::new(COMBAT_VFX_PRODUCER_RANK, identity),
        local.origin,
        None,
        None,
        "local input-frame ultimate launch effect",
    )
    .and_then(|request| request.with_signature(signature))
    .ok()
}

/// Muzzle effect for the local player's own primary fire, and a launch flash for its ultimate
/// casts, on the input frame. The later authoritative `AttackAccepted` for the same attack is an
/// echo and is skipped below.
#[allow(
    clippy::needless_pass_by_value,
    reason = "the signature cache and variant settings are read-only Bevy system resources"
)]
fn produce_local_attack_vfx_requests(
    mut local_attacks: MessageReader<LocalAttackPresented>,
    mut requests: MessageWriter<VfxRequest>,
    cache: Res<VfxSignatureCache>,
    loadouts: Query<&brawler::builds::ResolvedMatchLoadout>,
) {
    for local in local_attacks.read() {
        let Ok(loadout) = loadouts.get(local.fighter) else {
            continue;
        };
        let shot = match local.kind {
            LocalAttackKind::PrimaryFire(shot) => shot,
            LocalAttackKind::Ultimate { kind, cast } => {
                if crate::local_attack::presents_cast_motion_locally(kind)
                    && let Some(request) = local_ultimate_launch_request(
                        local,
                        kind,
                        cast,
                        loadout.ultimate.id,
                        &cache,
                    )
                {
                    requests.write(request);
                }
                continue;
            }
        };
        let recipe = &loadout.primary_weapon.recipe;
        let identity = LOCAL_ATTACK_VFX_IDENTITY | shot.0;
        let signature = cache.signature_for_local_attack(
            local.owner,
            local.team,
            recipe,
            carrier_for_delivery(recipe.delivery),
            identity,
            0,
            VfxPhase::Activation,
            activation_geometry_for_recipe(recipe, local.origin, local.facing),
            VfxAuthoritativeTiming::instant(local.simulation_tick),
            VfxPresentationVariant::Normal,
        );
        if let Ok(request) = VfxRequest::try_new(
            COMBAT_MUZZLE_VFX,
            VfxRequestOrder::new(COMBAT_VFX_PRODUCER_RANK, identity),
            local.origin,
            None,
            None,
            "local input-frame muzzle effect",
        )
        .and_then(|request| request.with_signature(signature))
        {
            requests.write(request);
        }
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
    ledger: Option<Res<LocalAttackLedger>>,
    fighters: Query<
        (
            &NetworkEntityId,
            &brawler::combat::TeamId,
            &avian2d::prelude::Position,
            Option<&avian2d::prelude::Rotation>,
        ),
        With<Fighter>,
    >,
    sentries: Query<
        (
            &brawler::abilities::SentryIdentity,
            &avian2d::prelude::Position,
            &avian2d::prelude::Rotation,
        ),
        With<brawler::abilities::Sentry>,
    >,
) {
    let observed_at_tick = authoritative_ticks.iter().map(|tick| tick.0).max();
    for DeduplicatedCombatCue(cue) in cues.read() {
        if let CombatCue::AttackAccepted { attack_id, .. } = cue
            && ledger
                .as_deref()
                .is_some_and(|ledger| ledger.is_echo(*attack_id))
        {
            continue;
        }
        let Some((key, cue_position, radius, deadline, label)) =
            combat_vfx_request(cue, observed_at_tick, &cache, |target| {
                replicated_fighter_position(&fighters, target)
            })
        else {
            continue;
        };
        let position = sentry_fire_presentation_position(cue, cue_position, &sentries);
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

fn sentry_fire_presentation_position(
    cue: &CombatCue,
    fallback: Vec2,
    sentries: &Query<
        (
            &brawler::abilities::SentryIdentity,
            &avian2d::prelude::Position,
            &avian2d::prelude::Rotation,
        ),
        With<brawler::abilities::Sentry>,
    >,
) -> Vec2 {
    let CombatCue::SentryFired { deployable_id, .. } = cue else {
        return fallback;
    };
    sentries
        .iter()
        .find_map(|(identity, position, rotation)| {
            (identity.deployable_id == *deployable_id).then(|| {
                position.0
                    + Vec2::from_angle(rotation.as_radians()) * SENTRY_MUZZLE_PRESENTATION_OFFSET
            })
        })
        .unwrap_or(fallback)
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
    fighters: &Query<
        (
            &NetworkEntityId,
            &brawler::combat::TeamId,
            &avian2d::prelude::Position,
            Option<&avian2d::prelude::Rotation>,
        ),
        With<Fighter>,
    >,
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
            tick,
            attack_id,
            source,
            ..
        } => {
            let facing = fighters.iter().find_map(|(network_id, _, _, rotation)| {
                (*network_id == *source)
                    .then(|| rotation.map_or(0.0, |rotation| rotation.as_radians()))
            });
            let geometry = facing
                .and_then(|facing| cache.melee_activation_geometry(*attack_id, position, facing))
                .unwrap_or(point);
            cache.signature_for_attack(
                *attack_id,
                0,
                VfxCarrier::PointTransient,
                VfxPhase::Activation,
                geometry,
                instant(*tick),
                VfxPresentationVariant::Normal,
            )
        }
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
        } => cache.signature_for_attack(
            *attack_id,
            *delivery_index,
            VfxCarrier::LobbedProjectile,
            VfxPhase::Impact,
            radius.map_or(point, |radius| VfxGeometry::circle(position, radius)),
            instant(*tick),
            VfxPresentationVariant::Normal,
        ),
        C::MeleeContact {
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
        } => cache.signature_for_attack(
            *attack_id,
            0,
            VfxCarrier::AttachedStatus,
            VfxPhase::Impact,
            point,
            instant(*tick),
            VfxPresentationVariant::Normal,
        ),
        C::FighterDefeated {
            event_id,
            tick,
            target,
            ..
        } => direct(
            VfxCarrier::AttachedStatus,
            VfxPhase::Interruption,
            [Some(VfxAccent::Neutral), None],
            relationship_for_owner(cache, fighters, *target),
            None,
            point,
            VfxPresentationIdentity::CombatEvent(event_id.0),
            instant(*tick),
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
            if matches!(reason, brawler::abilities::SentryCleanupReason::Expired) {
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
        C::SelfCloakActivated {
            tick,
            source,
            generation,
            expires_at_tick,
            ..
        } => direct(
            VfxCarrier::AttachedStatus,
            VfxPhase::Activation,
            [Some(VfxAccent::Concealment), None],
            relationship_for_owner(cache, fighters, *source),
            None,
            point,
            VfxPresentationIdentity::NetworkEntity {
                network_id: source.0,
                generation: *generation,
            },
            VfxAuthoritativeTiming::persistent(*tick, observed_at_tick, *expires_at_tick),
        ),
        C::SelfCloakEnded {
            tick,
            source,
            generation,
            reason,
            ..
        } => direct(
            VfxCarrier::AttachedStatus,
            if matches!(reason, brawler::combat::SelfCloakEndReason::Expired) {
                VfxPhase::Expiry
            } else {
                VfxPhase::Interruption
            },
            [Some(VfxAccent::Concealment), None],
            relationship_for_owner(cache, fighters, *source),
            None,
            point,
            VfxPresentationIdentity::NetworkEntity {
                network_id: source.0,
                generation: *generation,
            },
            instant(*tick),
        ),
        C::ForcedRevealApplied {
            tick,
            target,
            source_generation,
            expires_at_tick,
            ..
        } => direct(
            VfxCarrier::AttachedStatus,
            VfxPhase::Activation,
            [Some(VfxAccent::Reveal), None],
            relationship_for_owner(cache, fighters, *target),
            None,
            point,
            VfxPresentationIdentity::NetworkEntity {
                network_id: target.0,
                generation: *source_generation,
            },
            VfxAuthoritativeTiming::persistent(*tick, observed_at_tick, *expires_at_tick),
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
    fighters: &Query<
        (
            &NetworkEntityId,
            &brawler::combat::TeamId,
            &avian2d::prelude::Position,
            Option<&avian2d::prelude::Rotation>,
        ),
        With<Fighter>,
    >,
    owner: NetworkEntityId,
) -> VfxRelationship {
    fighters
        .iter()
        .find_map(|(network_id, team, _, _)| {
            (*network_id == owner).then(|| cache.observer().relationship(owner, *team))
        })
        .unwrap_or(VfxRelationship::Neutral)
}

fn relationship_for_team(
    cache: &VfxSignatureCache,
    team: brawler::combat::TeamId,
) -> VfxRelationship {
    cache.observer().team_relationship(team)
}

#[allow(
    clippy::type_complexity,
    reason = "the tuple is the compact renderer-neutral VFX request payload"
)]
fn combat_vfx_request(
    cue: &CombatCue,
    observed_at_tick: Option<u64>,
    cache: &VfxSignatureCache,
    fighter_position: impl Fn(NetworkEntityId) -> Option<Vec2>,
) -> Option<(
    crate::vfx::VfxRequestKey,
    Vec2,
    Option<f32>,
    Option<VfxDeadline>,
    &'static str,
)> {
    use CombatCue as C;
    if matches!(
        cue,
        C::SelfCloakActivated { .. } | C::SelfCloakEnded { .. } | C::ForcedRevealApplied { .. }
    ) {
        return concealment_vfx_request(cue, fighter_position);
    }
    match cue {
        C::AttackAccepted { position, .. } | C::SentryFired { position, .. } => Some((
            COMBAT_MUZZLE_VFX,
            position.as_vec2(),
            None,
            None,
            "V3 bounded combat cue effect",
        )),
        C::LobLanded {
            attack_id,
            position,
            ..
        } => Some((
            COMBAT_IMPACT_VFX,
            position.as_vec2(),
            cache.impact_radius_for_attack(*attack_id),
            None,
            "V3 bounded lobbed landing effect",
        )),
        C::DeliveryImpact { position, .. }
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
            brawler::builds::world_units_from_milliunits(*radius_milliunits),
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
            brawler::builds::world_units_from_milliunits(*radius_milliunits),
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
            brawler::builds::world_units_from_milliunits(*radius_milliunits),
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

#[allow(
    clippy::type_complexity,
    reason = "the tuple is the compact renderer-neutral VFX request payload"
)]
fn concealment_vfx_request(
    cue: &CombatCue,
    fighter_position: impl Fn(NetworkEntityId) -> Option<Vec2>,
) -> Option<(
    crate::vfx::VfxRequestKey,
    Vec2,
    Option<f32>,
    Option<VfxDeadline>,
    &'static str,
)> {
    match cue {
        CombatCue::SelfCloakActivated { source, .. } => fighter_position(*source).map(|position| {
            (
                SELF_CLOAK_VFX,
                position,
                None,
                None,
                "Self Cloak activation veil",
            )
        }),
        CombatCue::SelfCloakEnded { source, .. } => fighter_position(*source).map(|position| {
            (
                SELF_CLOAK_VFX,
                position,
                None,
                None,
                "Self Cloak lifecycle ending",
            )
        }),
        CombatCue::ForcedRevealApplied { target, .. } => {
            fighter_position(*target).map(|position| {
                (
                    FORCED_REVEAL_VFX,
                    position,
                    None,
                    None,
                    "Forced Reveal attached acquisition",
                )
            })
        }
        _ => None,
    }
}

fn replicated_fighter_position(
    fighters: &Query<
        (
            &NetworkEntityId,
            &brawler::combat::TeamId,
            &avian2d::prelude::Position,
            Option<&avian2d::prelude::Rotation>,
        ),
        With<Fighter>,
    >,
    target: NetworkEntityId,
) -> Option<Vec2> {
    fighters
        .iter()
        .find_map(|(network_id, _, position, _)| (*network_id == target).then_some(position.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use brawler::{
        combat::{AttackId, CombatEventId, WeaponDefinitionId, WorldPoint},
        protocol::NetworkEntityId,
    };

    #[derive(Resource, Default)]
    struct CapturedRequests(Vec<VfxRequest>);

    fn capture_requests(
        mut requests: MessageReader<VfxRequest>,
        mut captured: ResMut<CapturedRequests>,
    ) {
        captured.0.extend(requests.read().copied());
    }

    #[test]
    fn built_in_combat_keys_have_one_owner_and_expected_capabilities() {
        assert_eq!(REGISTRATIONS.len(), 9);
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
            4
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
        let cache = VfxSignatureCache::default();
        let attack = CombatCue::AttackAccepted {
            event_id: CombatEventId(9),
            tick: 10,
            attack_id: AttackId(11),
            source: NetworkEntityId(12),
            position: WorldPoint { x: 2.0, y: 3.0 },
            weapon_definition_id: WeaponDefinitionId(13),
        };
        let (key, position, radius, deadline, _) =
            combat_vfx_request(&attack, None, &cache, |_| None).unwrap();
        assert_eq!(key, COMBAT_MUZZLE_VFX);
        assert_eq!(position, Vec2::new(2.0, 3.0));
        assert_eq!(radius, None);
        assert_eq!(deadline, None);

        let scan = CombatCue::RevealScanActivated {
            event_id: CombatEventId(14),
            tick: 20,
            revealing_team: brawler::combat::TeamId(1),
            center: WorldPoint { x: 4.0, y: 5.0 },
            radius_milliunits: 6_000,
            expires_at_tick: 30,
        };
        let (key, _, radius, deadline, _) =
            combat_vfx_request(&scan, Some(22), &cache, |_| None).unwrap();
        assert_eq!(key, REVEAL_SCAN_VFX);
        assert_eq!(radius, Some(6.0));
        assert_eq!(deadline, Some(VfxDeadline::new(20, 30, Some(22))));

        let legacy = CombatCue::Muzzle {
            event_id: CombatEventId(15),
            tick: 21,
            source: NetworkEntityId(12),
            shot_id: brawler::combat::ShotId(16),
            weapon_definition_id: WeaponDefinitionId(13),
            position: WorldPoint { x: 2.0, y: 3.0 },
        };
        assert!(combat_vfx_request(&legacy, None, &cache, |_| None).is_none());

        let defeated = CombatCue::FighterDefeated {
            event_id: CombatEventId(16),
            tick: 22,
            attack_id: AttackId(11),
            source: None,
            target: NetworkEntityId(12),
            position: WorldPoint { x: 7.0, y: 8.0 },
        };
        let (key, _, _, _, _) = combat_vfx_request(&defeated, None, &cache, |_| None).unwrap();
        assert_eq!(key, COMBAT_DAMAGE_VFX);

        let reset = CombatCue::FighterReset {
            event_id: CombatEventId(17),
            tick: 23,
            target: NetworkEntityId(12),
            position: WorldPoint { x: 7.0, y: 8.0 },
        };
        let (key, _, _, _, _) = combat_vfx_request(&reset, None, &cache, |_| None).unwrap();
        assert_eq!(key, COMBAT_RESET_VFX);
    }

    fn preset_loadout() -> brawler::builds::ResolvedMatchLoadout {
        let builds = brawler::builds::BuildCatalog::embedded().expect("embedded build catalog");
        let weapons = brawler::combat::WeaponCatalog::embedded().expect("embedded weapon catalog");
        brawler::builds::resolve_loadout(
            &builds,
            &weapons,
            brawler::builds::LoadoutRecipe::test_default(
                brawler::combat::WeaponPresetId(1),
                brawler::builds::UltimateDefinitionId(1),
                brawler::builds::PassiveDefinitionId(6),
            ),
        )
        .expect("preset loadout resolves")
    }

    #[test]
    fn local_fire_presents_its_muzzle_once_and_the_server_echo_is_skipped() {
        let mut app = App::new();
        app.add_message::<DeduplicatedCombatCue>()
            .add_message::<VfxRequest>()
            .add_message::<LocalAttackPresented>()
            .init_resource::<VfxSignatureCache>()
            .init_resource::<CapturedRequests>()
            .add_systems(
                Update,
                (
                    (
                        produce_combat_vfx_requests,
                        produce_local_attack_vfx_requests,
                    ),
                    capture_requests,
                )
                    .chain(),
            );
        let mut ledger = LocalAttackLedger::default();
        let shot = ledger.record(lightyear::prelude::Tick(1), 100);
        ledger.observe_own_attack(AttackId(11), 100);
        app.insert_resource(ledger);
        let fighter = app.world_mut().spawn(preset_loadout()).id();
        app.world_mut().write_message(LocalAttackPresented {
            fighter,
            owner: NetworkEntityId(12),
            team: brawler::combat::TeamId(1),
            kind: LocalAttackKind::PrimaryFire(shot),
            origin: Vec2::new(2.0, 3.0),
            facing: 0.0,
            simulation_tick: 100,
        });
        for (attack_id, source) in [(11, 12), (21, 22)] {
            app.world_mut()
                .write_message(DeduplicatedCombatCue(CombatCue::AttackAccepted {
                    event_id: CombatEventId(attack_id),
                    tick: 100,
                    attack_id: AttackId(attack_id),
                    source: NetworkEntityId(source),
                    position: WorldPoint { x: 2.0, y: 3.0 },
                    weapon_definition_id: WeaponDefinitionId(1),
                }));
        }

        app.update();

        let captured = &app.world().resource::<CapturedRequests>().0;
        assert_eq!(
            captured.len(),
            2,
            "local muzzle plus the remote attack only"
        );
        assert!(
            captured
                .iter()
                .all(|request| request.key == COMBAT_MUZZLE_VFX)
        );
        let local = captured
            .iter()
            .find(|request| request.order.event_id & LOCAL_ATTACK_VFX_IDENTITY != 0)
            .expect("local muzzle request");
        assert_eq!(local.position, Vec2::new(2.0, 3.0));
        assert!(
            captured.iter().any(|request| request.order.event_id == 21),
            "a remote attack still presents from its cue"
        );
    }

    #[test]
    fn local_ultimate_presents_one_launch_flash_and_its_area_outcome_stays_cue_driven() {
        let mut app = App::new();
        app.add_message::<DeduplicatedCombatCue>()
            .add_message::<VfxRequest>()
            .add_message::<LocalAttackPresented>()
            .init_resource::<VfxSignatureCache>()
            .init_resource::<CapturedRequests>()
            .add_systems(
                Update,
                (
                    (
                        produce_combat_vfx_requests,
                        produce_local_attack_vfx_requests,
                    ),
                    capture_requests,
                )
                    .chain(),
            );
        let mut ledger = LocalAttackLedger::default();
        let cast = ledger.record_cast(
            brawler::builds::UltimateKind::FireField,
            lightyear::prelude::Tick(1),
            100,
        );
        app.insert_resource(ledger);
        let fighter = app.world_mut().spawn(preset_loadout()).id();
        app.world_mut().write_message(LocalAttackPresented {
            fighter,
            owner: NetworkEntityId(12),
            team: brawler::combat::TeamId(1),
            kind: LocalAttackKind::Ultimate {
                kind: brawler::builds::UltimateKind::FireField,
                cast,
            },
            origin: Vec2::new(2.0, 3.0),
            facing: 0.0,
            simulation_tick: 100,
        });
        // A local Dash is sound only: no launch flash before the authoritative dash motion.
        app.world_mut().write_message(LocalAttackPresented {
            fighter,
            owner: NetworkEntityId(12),
            team: brawler::combat::TeamId(1),
            kind: LocalAttackKind::Ultimate {
                kind: brawler::builds::UltimateKind::Dash,
                cast: crate::local_attack::LocalCastId(cast.0 + 1),
            },
            origin: Vec2::new(2.0, 3.0),
            facing: 0.0,
            simulation_tick: 100,
        });
        app.world_mut()
            .write_message(DeduplicatedCombatCue(CombatCue::ElementalFieldActivated {
                event_id: CombatEventId(40),
                tick: 102,
                source: NetworkEntityId(12),
                field_id: brawler::combat::ElementalFieldId(1),
                kind: brawler::combat::ElementalFieldKind::Fire,
                center: WorldPoint { x: 50.0, y: 60.0 },
                radius_milliunits: 150_000,
                expires_at_tick: 400,
            }));

        app.update();

        let captured = &app.world().resource::<CapturedRequests>().0;
        assert_eq!(
            captured.len(),
            2,
            "launch flash plus the authoritative field (no Dash flash)"
        );
        let launch = captured
            .iter()
            .find(|request| request.key == COMBAT_MUZZLE_VFX)
            .expect("local launch flash");
        assert_eq!(launch.position, Vec2::new(2.0, 3.0));
        assert_eq!(launch.order.event_id, LOCAL_ULTIMATE_VFX_IDENTITY | cast.0);
        assert_eq!(launch.signature.accents[0], Some(VfxAccent::Fire));
        let catalog = crate::vfx::catalog::VfxCatalog::embedded().expect("embedded VFX catalog");
        let recipe = catalog
            .resolve(COMBAT_MUZZLE_VFX.as_str(), &launch.signature, false, false)
            .expect("the launch flash resolves");
        assert_eq!(recipe.id, "fire_muzzle");
        assert!(
            captured
                .iter()
                .any(|request| request.key == ELEMENTAL_FIELD_VFX
                    && request.position == Vec2::new(50.0, 60.0)),
            "the field outcome is presented from its cue at the authoritative center"
        );
    }

    #[test]
    fn fighter_outcome_signatures_distinguish_defeat_from_reset() {
        let mut app = App::new();
        app.add_message::<DeduplicatedCombatCue>()
            .add_message::<VfxRequest>()
            .init_resource::<VfxSignatureCache>()
            .init_resource::<CapturedRequests>()
            .add_systems(
                Update,
                (produce_combat_vfx_requests, capture_requests).chain(),
            );
        for cue in [
            CombatCue::FighterDefeated {
                event_id: CombatEventId(16),
                tick: 22,
                attack_id: AttackId(11),
                source: None,
                target: NetworkEntityId(12),
                position: WorldPoint { x: 7.0, y: 8.0 },
            },
            CombatCue::FighterReset {
                event_id: CombatEventId(17),
                tick: 23,
                target: NetworkEntityId(12),
                position: WorldPoint { x: 7.0, y: 8.0 },
            },
        ] {
            app.world_mut().write_message(DeduplicatedCombatCue(cue));
        }

        app.update();

        let requests = &app.world().resource::<CapturedRequests>().0;
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].key, COMBAT_DAMAGE_VFX);
        assert_eq!(requests[0].signature.carrier, VfxCarrier::AttachedStatus);
        assert_eq!(requests[0].signature.phase, VfxPhase::Interruption);
        assert_eq!(
            requests[0].signature.identity,
            VfxPresentationIdentity::CombatEvent(16)
        );
        assert_eq!(requests[1].key, COMBAT_RESET_VFX);
        assert_eq!(requests[1].signature.carrier, VfxCarrier::AttachedStatus);
        assert_eq!(requests[1].signature.phase, VfxPhase::Expiry);
    }

    #[test]
    fn concealment_cues_resolve_fighter_positions_and_fail_closed_when_absent() {
        let cache = VfxSignatureCache::default();
        let cue = CombatCue::SelfCloakActivated {
            event_id: CombatEventId(16),
            tick: 22,
            source: NetworkEntityId(12),
            generation: 3,
            expires_at_tick: 80,
        };
        let resolved = combat_vfx_request(&cue, Some(22), &cache, |target| {
            (target == NetworkEntityId(12)).then_some(Vec2::new(40.0, 50.0))
        })
        .unwrap();
        assert_eq!(resolved.0, SELF_CLOAK_VFX);
        assert_eq!(resolved.1, Vec2::new(40.0, 50.0));
        assert!(combat_vfx_request(&cue, Some(22), &cache, |_| None).is_none());
    }

    #[test]
    fn concealment_lifecycle_signatures_preserve_semantics() {
        let mut app = App::new();
        app.add_message::<DeduplicatedCombatCue>()
            .add_message::<VfxRequest>()
            .init_resource::<VfxSignatureCache>()
            .init_resource::<CapturedRequests>()
            .add_systems(
                Update,
                (produce_combat_vfx_requests, capture_requests).chain(),
            );
        app.world_mut().spawn((
            Fighter,
            NetworkEntityId(12),
            brawler::combat::TeamId(1),
            avian2d::prelude::Position(Vec2::new(40.0, 50.0)),
            avian2d::prelude::Rotation::radians(0.0),
            AuthoritativeTick(22),
        ));
        for cue in [
            CombatCue::SelfCloakActivated {
                event_id: CombatEventId(20),
                tick: 22,
                source: NetworkEntityId(12),
                generation: 3,
                expires_at_tick: 80,
            },
            CombatCue::SelfCloakEnded {
                event_id: CombatEventId(21),
                tick: 40,
                source: NetworkEntityId(12),
                generation: 3,
                reason: brawler::combat::SelfCloakEndReason::Expired,
            },
            CombatCue::SelfCloakEnded {
                event_id: CombatEventId(22),
                tick: 41,
                source: NetworkEntityId(12),
                generation: 4,
                reason: brawler::combat::SelfCloakEndReason::Attack,
            },
            CombatCue::ForcedRevealApplied {
                event_id: CombatEventId(23),
                tick: 42,
                target: NetworkEntityId(12),
                revealing_team: brawler::combat::TeamId(2),
                source_generation: 8,
                expires_at_tick: 100,
            },
        ] {
            app.world_mut().write_message(DeduplicatedCombatCue(cue));
        }

        app.update();

        let requests = &app.world().resource::<CapturedRequests>().0;
        assert_eq!(requests.len(), 4);
        assert!(
            requests
                .iter()
                .all(|request| request.position == Vec2::new(40.0, 50.0))
        );
        assert_eq!(requests[0].key, SELF_CLOAK_VFX);
        assert_eq!(requests[0].signature.phase, VfxPhase::Activation);
        assert_eq!(
            requests[0].signature.accents[0],
            Some(VfxAccent::Concealment)
        );
        assert_eq!(requests[1].signature.phase, VfxPhase::Expiry);
        assert_eq!(requests[2].signature.phase, VfxPhase::Interruption);
        assert_eq!(requests[3].key, FORCED_REVEAL_VFX);
        assert_eq!(requests[3].signature.accents[0], Some(VfxAccent::Reveal));
        assert_eq!(requests[3].signature.phase, VfxPhase::Activation);
    }

    #[test]
    fn sentry_fire_request_is_attached_to_the_replicated_muzzle_rail() {
        let mut app = App::new();
        app.add_message::<DeduplicatedCombatCue>()
            .add_message::<VfxRequest>()
            .init_resource::<VfxSignatureCache>()
            .init_resource::<CapturedRequests>()
            .add_systems(
                Update,
                (produce_combat_vfx_requests, capture_requests).chain(),
            );
        app.world_mut().spawn((
            brawler::abilities::Sentry,
            brawler::abilities::SentryIdentity {
                deployable_id: brawler::builds::DeployableId(7),
                owner_player_id: brawler::protocol::PlayerId(1),
                owner_network_id: NetworkEntityId(1),
                team_id: brawler::combat::TeamId(0),
                ultimate_id: brawler::builds::UltimateDefinitionId(2),
                match_id: brawler::matchplay::MatchId(1),
            },
            avian2d::prelude::Position(Vec2::new(10.0, 20.0)),
            avian2d::prelude::Rotation::radians(core::f32::consts::FRAC_PI_2),
        ));
        app.world_mut()
            .write_message(DeduplicatedCombatCue(CombatCue::SentryFired {
                event_id: CombatEventId(22),
                tick: 30,
                owner: NetworkEntityId(1),
                deployable_id: brawler::builds::DeployableId(7),
                target: None,
                position: WorldPoint { x: 10.0, y: 20.0 },
            }));

        app.update();

        let captured = app.world().resource::<CapturedRequests>();
        assert_eq!(captured.0.len(), 1);
        let request = &captured.0[0];
        assert!(request.position.abs_diff_eq(Vec2::new(10.0, 67.0), 0.000_1));
        assert_eq!(request.signature.carrier, VfxCarrier::Deployable);
        assert_eq!(request.signature.phase, VfxPhase::Activation);
        assert_eq!(request.signature.context, Some(VfxContextOverride::Sentry));
    }
}
