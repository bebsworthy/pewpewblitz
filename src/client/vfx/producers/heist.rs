use bevy::prelude::*;

use super::VfxRequestSet;
use crate::{
    client::{
        hud,
        vfx::{
            HEIST_CRITICAL_VFX, HEIST_DAMAGED_VFX, HEIST_DESTROYED_VFX, HEIST_VFX_PRODUCER_RANK,
            VfxAppExt, VfxRequest, VfxRequestCapabilities, VfxRequestOrder, VfxRequestRegistration,
        },
    },
    matchplay::{MatchRoot, MatchState},
};

const REGISTRATIONS: [VfxRequestRegistration; 3] = [
    VfxRequestRegistration::new(
        HEIST_DAMAGED_VFX,
        HEIST_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::NONE,
    ),
    VfxRequestRegistration::new(
        HEIST_CRITICAL_VFX,
        HEIST_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::NONE,
    ),
    VfxRequestRegistration::new(
        HEIST_DESTROYED_VFX,
        HEIST_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::NONE,
    ),
];

pub(crate) struct HeistVfxProducerPlugin;

impl Plugin for HeistVfxProducerPlugin {
    fn build(&self, app: &mut App) {
        for registration in REGISTRATIONS {
            app.try_register_vfx_request(registration)
                .expect("Heist VFX request registration must be valid");
        }
        app.add_systems(Update, produce_heist_vfx_requests.in_set(VfxRequestSet));
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "ClientHeistReadiness is a Bevy system resource parameter"
)]
fn produce_heist_vfx_requests(
    mut received: MessageReader<crate::matchplay::ReceivedHeistObjectiveCue>,
    mut requests: MessageWriter<VfxRequest>,
    readiness: Res<hud::ClientHeistReadiness>,
    matches: Query<&MatchState, With<MatchRoot>>,
    safes: Query<&crate::map::DamageableTargetIdentity, With<crate::matchplay::HeistSafe>>,
) {
    let ready = matches!(*readiness, hud::ClientHeistReadiness::Ready);
    let match_id = matches.single().ok().map(|state| state.match_id);
    for crate::matchplay::ReceivedHeistObjectiveCue(cue) in received.read() {
        let safe_present = safes.iter().any(|identity| *identity == cue.target);
        let Some(request) = heist_vfx_request(cue, ready, match_id, safe_present) else {
            continue;
        };
        requests.write(request);
    }
}

fn heist_vfx_request(
    cue: &crate::matchplay::HeistObjectiveCue,
    ready: bool,
    active_match_id: Option<crate::matchplay::MatchId>,
    safe_present: bool,
) -> Option<VfxRequest> {
    if !ready
        || !matches!(
            cue.target,
            crate::map::DamageableTargetIdentity::HeistSafe {
                match_id: cue_match,
                ..
            } if Some(cue_match) == active_match_id
        )
        || !safe_present
    {
        return None;
    }
    let (key, label) = match cue.kind {
        crate::matchplay::HeistObjectiveCueKind::Damaged => {
            (HEIST_DAMAGED_VFX, "Heist safe hit cue")
        }
        crate::matchplay::HeistObjectiveCueKind::Critical => {
            (HEIST_CRITICAL_VFX, "Heist safe critical cue")
        }
        crate::matchplay::HeistObjectiveCueKind::Destroyed => {
            (HEIST_DESTROYED_VFX, "Heist safe destroyed cue")
        }
    };
    VfxRequest::try_new(
        key,
        VfxRequestOrder::new(HEIST_VFX_PRODUCER_RANK, cue.event_id.0),
        cue.position.as_vec2(),
        None,
        None,
        label,
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        combat::{AttackId, CombatEventId, TeamId, WorldPoint},
        map::{DamageableTargetIdentity, ModeAnchorId},
        matchplay::{HeistObjectiveCue, HeistObjectiveCueKind, MatchId},
    };

    fn cue(match_id: MatchId) -> HeistObjectiveCue {
        HeistObjectiveCue {
            event_id: CombatEventId(9),
            tick: 10,
            attack_id: AttackId(11),
            source_subject: None,
            target: DamageableTargetIdentity::HeistSafe {
                match_id,
                anchor_id: ModeAnchorId(12),
                defending_team: TeamId(2),
            },
            position: WorldPoint { x: 2.0, y: 3.0 },
            amount: 4,
            health_after: 5,
            maximum_health: 6,
            kind: HeistObjectiveCueKind::Critical,
        }
    }

    #[test]
    fn readiness_match_and_exact_safe_membership_gate_objective_vfx() {
        let expected_match = MatchId(7);
        let cue = cue(expected_match);
        assert!(heist_vfx_request(&cue, false, Some(expected_match), true).is_none());
        assert!(heist_vfx_request(&cue, true, Some(MatchId(8)), true).is_none());
        assert!(heist_vfx_request(&cue, true, Some(expected_match), false).is_none());

        let request = heist_vfx_request(&cue, true, Some(expected_match), true).unwrap();
        assert_eq!(request.key, HEIST_CRITICAL_VFX);
        assert_eq!(request.order.event_id, 9);
        assert_eq!(request.position, Vec2::new(2.0, 3.0));
    }
}
