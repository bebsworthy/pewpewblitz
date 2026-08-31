use bevy::prelude::*;

use super::VfxRequestSet;
use crate::client::vfx::{
    PICKUP_COLLECTED_VFX, PICKUP_EXPIRED_VFX, PICKUP_SPAWNED_VFX, PICKUP_VFX_PRODUCER_RANK,
    VfxAppExt, VfxRequest, VfxRequestCapabilities, VfxRequestOrder, VfxRequestRegistration,
};

const REGISTRATIONS: [VfxRequestRegistration; 3] = [
    VfxRequestRegistration::new(
        PICKUP_SPAWNED_VFX,
        PICKUP_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::NONE,
    ),
    VfxRequestRegistration::new(
        PICKUP_COLLECTED_VFX,
        PICKUP_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::NONE,
    ),
    VfxRequestRegistration::new(
        PICKUP_EXPIRED_VFX,
        PICKUP_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::NONE,
    ),
];

pub(crate) struct PickupVfxProducerPlugin;

impl Plugin for PickupVfxProducerPlugin {
    fn build(&self, app: &mut App) {
        for registration in REGISTRATIONS {
            app.try_register_vfx_request(registration)
                .expect("pickup VFX request registration must be valid");
        }
        app.add_systems(Update, produce_pickup_vfx_requests.in_set(VfxRequestSet));
    }
}

fn produce_pickup_vfx_requests(
    received: Option<ResMut<crate::map::ReceivedPickupCues>>,
    mut requests: MessageWriter<VfxRequest>,
    map_states: Query<&crate::map::MapDynamicState, With<crate::map::MapRoot>>,
) {
    let Some(mut received) = received else { return };
    let Ok(map_state) = map_states.single() else {
        received.0.clear();
        return;
    };
    for cue in received.0.drain(..) {
        let Some(request) = pickup_vfx_request(cue, map_state.generation_id()) else {
            continue;
        };
        requests.write(request);
    }
}

fn pickup_vfx_request(
    cue: crate::map::PickupCue,
    active_generation: crate::map::MapDynamicGeneration,
) -> Option<VfxRequest> {
    let event_id = cue.event_id().0;
    let (identity, position, key, label) = match cue {
        crate::map::PickupCue::Spawned {
            identity, position, ..
        } => (
            identity,
            position.as_vec2(),
            PICKUP_SPAWNED_VFX,
            "V10 chest restoration drop",
        ),
        crate::map::PickupCue::Collected {
            identity, position, ..
        } => (
            identity,
            position.as_vec2(),
            PICKUP_COLLECTED_VFX,
            "V10 restoration collected",
        ),
        crate::map::PickupCue::Expired {
            identity, position, ..
        } => (
            identity,
            position.as_vec2(),
            PICKUP_EXPIRED_VFX,
            "V10 restoration expired",
        ),
    };
    if identity.generation != active_generation {
        return None;
    }
    VfxRequest::try_new(
        key,
        VfxRequestOrder::new(PICKUP_VFX_PRODUCER_RANK, event_id),
        position,
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
        combat::{CombatEventId, WorldPoint},
        map::{
            MapDynamicGeneration, MapInstanceId, MapPlacementId, PickupCue,
            RestorationPickupDefinitionId, RestorationPickupIdentity,
        },
    };

    fn generation(value: u64) -> MapDynamicGeneration {
        MapDynamicGeneration {
            map_instance_id: MapInstanceId(7),
            generation: value,
        }
    }

    fn spawned(value: u64) -> PickupCue {
        PickupCue::Spawned {
            event_id: CombatEventId(9),
            tick: 10,
            identity: RestorationPickupIdentity {
                generation: generation(value),
                source_placement_id: MapPlacementId(11),
            },
            definition_id: RestorationPickupDefinitionId(12),
            position: WorldPoint { x: 2.0, y: 3.0 },
        }
    }

    #[test]
    fn generation_gate_rejects_stale_and_maps_current_pickup_cue() {
        assert!(pickup_vfx_request(spawned(1), generation(2)).is_none());
        let request = pickup_vfx_request(spawned(2), generation(2)).unwrap();
        assert_eq!(request.key, PICKUP_SPAWNED_VFX);
        assert_eq!(request.order.event_id, 9);
        assert_eq!(request.position, Vec2::new(2.0, 3.0));
    }
}
