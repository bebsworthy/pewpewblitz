use bevy::prelude::*;

use super::VfxRequestSet;
use crate::client::vfx::{
    VfxAppExt, VfxRequest, VfxRequestCapabilities, VfxRequestOrder, VfxRequestRegistration,
    WORLD_OBJECT_DAMAGED_VFX, WORLD_OBJECT_EXPLOSION_VFX, WORLD_OBJECT_VFX_PRODUCER_RANK,
};

const REGISTRATIONS: [VfxRequestRegistration; 2] = [
    VfxRequestRegistration::new(
        WORLD_OBJECT_DAMAGED_VFX,
        WORLD_OBJECT_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::NONE,
    ),
    VfxRequestRegistration::new(
        WORLD_OBJECT_EXPLOSION_VFX,
        WORLD_OBJECT_VFX_PRODUCER_RANK,
        VfxRequestCapabilities::RADIUS,
    ),
];

pub(crate) struct WorldObjectVfxProducerPlugin;

impl Plugin for WorldObjectVfxProducerPlugin {
    fn build(&self, app: &mut App) {
        for registration in REGISTRATIONS {
            app.try_register_vfx_request(registration)
                .expect("world-object VFX request registration must be valid");
        }
        app.add_systems(
            Update,
            produce_world_object_vfx_requests.in_set(VfxRequestSet),
        );
    }
}

fn produce_world_object_vfx_requests(
    received: Option<ResMut<crate::map::ReceivedWorldObjectCues>>,
    mut requests: MessageWriter<VfxRequest>,
    map_states: Query<&crate::map::MapDynamicState, With<crate::map::MapRoot>>,
) {
    let Some(mut received) = received else { return };
    let Ok(map_state) = map_states.single() else {
        received.0.clear();
        return;
    };
    for cue in received.0.drain(..) {
        let Some(request) = world_object_vfx_request(cue, map_state.generation_id()) else {
            continue;
        };
        requests.write(request);
    }
}

fn world_object_vfx_request(
    cue: crate::map::WorldObjectCue,
    active_generation: crate::map::MapDynamicGeneration,
) -> Option<VfxRequest> {
    if cue.target().generation() != active_generation {
        return None;
    }
    let event_id = cue.event_id().0;
    let (key, position, radius, label) = match cue {
        crate::map::WorldObjectCue::Damaged { position, .. } => (
            WORLD_OBJECT_DAMAGED_VFX,
            position.as_vec2(),
            None,
            "V10 oil-barrel damage response",
        ),
        crate::map::WorldObjectCue::Exploded {
            position,
            radius_world_units,
            ..
        } => (
            WORLD_OBJECT_EXPLOSION_VFX,
            position.as_vec2(),
            Some(f32::from(radius_world_units)),
            "V10 authoritative oil-barrel blast",
        ),
    };
    VfxRequest::try_new(
        key,
        VfxRequestOrder::new(WORLD_OBJECT_VFX_PRODUCER_RANK, event_id),
        position,
        radius,
        None,
        label,
    )
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        combat::{AttackId, CombatEventId, WorldPoint},
        map::{
            DamageableTargetIdentity, MapDynamicGeneration, MapInstanceId, MapPlacementId,
            WorldObjectCue,
        },
    };

    fn generation(value: u64) -> MapDynamicGeneration {
        MapDynamicGeneration {
            map_instance_id: MapInstanceId(7),
            generation: value,
        }
    }

    fn damaged(value: u64) -> WorldObjectCue {
        WorldObjectCue::Damaged {
            event_id: CombatEventId(9),
            tick: 10,
            attack_id: AttackId(11),
            source_subject: None,
            target: DamageableTargetIdentity::MapObject {
                generation: generation(value),
                placement_id: MapPlacementId(12),
            },
            position: WorldPoint { x: 2.0, y: 3.0 },
            amount: 4,
            health_after: 5,
        }
    }

    #[test]
    fn generation_gate_rejects_stale_and_maps_current_world_object_cue() {
        assert!(world_object_vfx_request(damaged(1), generation(2)).is_none());
        let request = world_object_vfx_request(damaged(2), generation(2)).unwrap();
        assert_eq!(request.key, WORLD_OBJECT_DAMAGED_VFX);
        assert_eq!(request.order.event_id, 9);
        assert_eq!(request.position, Vec2::new(2.0, 3.0));
    }
}
