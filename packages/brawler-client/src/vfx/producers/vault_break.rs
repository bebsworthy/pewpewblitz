use bevy::prelude::*;

use super::VfxRequestSet;
use crate::{
    hud,
    vfx::{
        CombatVisualSignature, VAULT_BREAK_CRITICAL_VFX, VAULT_BREAK_DAMAGED_VFX,
        VAULT_BREAK_DESTROYED_VFX, VAULT_BREAK_VFX_PRODUCER_RANK, VfxAccent, VfxAppExt,
        VfxAuthoritativeTiming, VfxCarrier, VfxContextOverride, VfxGeometry, VfxPhase,
        VfxPresentationIdentity, VfxPresentationVariant, VfxRelationship, VfxRequest,
        VfxRequestCapabilities, VfxRequestOrder, VfxRequestRegistration,
    },
};
use brawler::matchplay::{MatchRoot, MatchState};

const REGISTRATIONS: [VfxRequestRegistration; 3] = [
    VfxRequestRegistration::new(
        VAULT_BREAK_DAMAGED_VFX,
        VAULT_BREAK_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::NONE,
    ),
    VfxRequestRegistration::new(
        VAULT_BREAK_CRITICAL_VFX,
        VAULT_BREAK_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::NONE,
    ),
    VfxRequestRegistration::new(
        VAULT_BREAK_DESTROYED_VFX,
        VAULT_BREAK_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::NONE,
    ),
];

pub(crate) struct VaultBreakVfxProducerPlugin;

impl Plugin for VaultBreakVfxProducerPlugin {
    fn build(&self, app: &mut App) {
        for registration in REGISTRATIONS {
            app.try_register_vfx_request(registration)
                .expect("Vault Break VFX request registration must be valid");
        }
        app.add_systems(
            Update,
            produce_vault_break_vfx_requests.in_set(VfxRequestSet),
        );
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "ClientVaultBreakReadiness is a Bevy system resource parameter"
)]
fn produce_vault_break_vfx_requests(
    mut received: MessageReader<brawler::matchplay::ReceivedVaultBreakObjectiveCue>,
    mut requests: MessageWriter<VfxRequest>,
    readiness: Res<hud::ClientVaultBreakReadiness>,
    matches: Query<&MatchState, With<MatchRoot>>,
    safes: Query<&brawler::map::DamageableTargetIdentity, With<brawler::matchplay::VaultBreakSafe>>,
) {
    let ready = matches!(*readiness, hud::ClientVaultBreakReadiness::Ready);
    let match_id = matches.single().ok().map(|state| state.match_id);
    for brawler::matchplay::ReceivedVaultBreakObjectiveCue(cue) in received.read() {
        let safe_present = safes.iter().any(|identity| *identity == cue.target);
        let Some(request) = vault_break_vfx_request(cue, ready, match_id, safe_present) else {
            continue;
        };
        requests.write(request);
    }
}

fn vault_break_vfx_request(
    cue: &brawler::matchplay::VaultBreakObjectiveCue,
    ready: bool,
    active_match_id: Option<brawler::matchplay::MatchId>,
    safe_present: bool,
) -> Option<VfxRequest> {
    if !ready
        || !matches!(
            cue.target,
            brawler::map::DamageableTargetIdentity::VaultBreakSafe {
                match_id: cue_match,
                ..
            } if Some(cue_match) == active_match_id
        )
        || !safe_present
    {
        return None;
    }
    let (key, phase, label) = match cue.kind {
        brawler::matchplay::VaultBreakObjectiveCueKind::Damaged => (
            VAULT_BREAK_DAMAGED_VFX,
            VfxPhase::Impact,
            "Vault Break safe hit cue",
        ),
        brawler::matchplay::VaultBreakObjectiveCueKind::Critical => (
            VAULT_BREAK_CRITICAL_VFX,
            VfxPhase::Interruption,
            "Vault Break safe critical cue",
        ),
        brawler::matchplay::VaultBreakObjectiveCueKind::Destroyed => (
            VAULT_BREAK_DESTROYED_VFX,
            VfxPhase::Expiry,
            "Vault Break safe destroyed cue",
        ),
    };
    VfxRequest::try_new(
        key,
        VfxRequestOrder::new(VAULT_BREAK_VFX_PRODUCER_RANK, cue.event_id.0),
        cue.position.as_vec2(),
        None,
        None,
        label,
    )
    .and_then(|request| {
        request.with_signature(CombatVisualSignature {
            carrier: VfxCarrier::WorldFeedback,
            phase,
            accents: [Some(VfxAccent::Neutral), None],
            relationship: VfxRelationship::Neutral,
            context: Some(VfxContextOverride::Environment),
            geometry: VfxGeometry::point(cue.position.as_vec2()),
            identity: VfxPresentationIdentity::CombatEvent(cue.event_id.0),
            timing: VfxAuthoritativeTiming::instant(cue.tick),
            variant: VfxPresentationVariant::Normal,
        })
    })
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use brawler::{
        combat::{AttackId, CombatEventId, TeamId, WorldPoint},
        map::{DamageableTargetIdentity, ModeAnchorId},
        matchplay::{MatchId, VaultBreakObjectiveCue, VaultBreakObjectiveCueKind},
    };

    fn cue(match_id: MatchId) -> VaultBreakObjectiveCue {
        VaultBreakObjectiveCue {
            event_id: CombatEventId(9),
            tick: 10,
            attack_id: AttackId(11),
            source_subject: None,
            target: DamageableTargetIdentity::VaultBreakSafe {
                match_id,
                anchor_id: ModeAnchorId(12),
                defending_team: TeamId(2),
            },
            position: WorldPoint { x: 2.0, y: 3.0 },
            amount: 4,
            health_after: 5,
            maximum_health: 6,
            kind: VaultBreakObjectiveCueKind::Critical,
        }
    }

    #[test]
    fn readiness_match_and_exact_safe_membership_gate_objective_vfx() {
        let expected_match = MatchId(7);
        let cue = cue(expected_match);
        assert!(vault_break_vfx_request(&cue, false, Some(expected_match), true).is_none());
        assert!(vault_break_vfx_request(&cue, true, Some(MatchId(8)), true).is_none());
        assert!(vault_break_vfx_request(&cue, true, Some(expected_match), false).is_none());

        let request = vault_break_vfx_request(&cue, true, Some(expected_match), true).unwrap();
        assert_eq!(request.key, VAULT_BREAK_CRITICAL_VFX);
        assert_eq!(request.order.event_id, 9);
        assert_eq!(request.position, Vec2::new(2.0, 3.0));
        assert_eq!(request.signature.carrier, VfxCarrier::WorldFeedback);
        assert_eq!(request.signature.phase, VfxPhase::Interruption);
        assert_eq!(
            request.signature.context,
            Some(VfxContextOverride::Environment)
        );
    }
}
