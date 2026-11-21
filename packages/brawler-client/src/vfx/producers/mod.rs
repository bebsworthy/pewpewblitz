//! Feature-owned gameplay-fact to semantic-VFX request adapters.

mod combat;
mod pickups;
mod vault_break;
mod world_objects;

use bevy::prelude::SystemSet;

pub(crate) use combat::CombatVfxProducerPlugin;
pub(crate) use pickups::PickupVfxProducerPlugin;
pub(crate) use vault_break::VaultBreakVfxProducerPlugin;
pub(crate) use world_objects::WorldObjectVfxProducerPlugin;

/// All feature-owned request producers run before renderer materialization.
///
/// Requests carry a stable producer rank and event identity, so systems in this set may execute
/// in parallel without making capacity eviction depend on Bevy's message merge order.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct VfxRequestSet;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vfx::{
        COMBAT_MUZZLE_VFX, COMBAT_VFX_PRODUCER_RANK, VfxRegistryPlugin, VfxRequest, VfxRequestOrder,
    };
    use bevy::prelude::*;

    #[derive(Resource, Default)]
    struct ObservedRequests(Vec<&'static str>);

    fn synthetic_producer(mut requests: MessageWriter<VfxRequest>) {
        requests.write(
            VfxRequest::try_new(
                COMBAT_MUZZLE_VFX,
                VfxRequestOrder::new(COMBAT_VFX_PRODUCER_RANK, 1),
                Vec2::ZERO,
                None,
                None,
                "synthetic producer",
            )
            .unwrap(),
        );
    }

    fn observe_requests(
        mut requests: MessageReader<VfxRequest>,
        mut observed: ResMut<ObservedRequests>,
    ) {
        observed
            .0
            .extend(requests.read().map(|request| request.key.as_str()));
    }

    #[test]
    fn producer_set_runs_before_a_same_frame_request_consumer() {
        let mut app = App::new();
        app.add_message::<brawler::combat::client::DeduplicatedCombatCue>()
            .add_message::<brawler::matchplay::ReceivedVaultBreakObjectiveCue>()
            .insert_resource(crate::hud::ClientVaultBreakReadiness::default())
            .init_resource::<ObservedRequests>()
            .add_plugins(VfxRegistryPlugin)
            .add_plugins((
                CombatVfxProducerPlugin,
                WorldObjectVfxProducerPlugin,
                PickupVfxProducerPlugin,
                VaultBreakVfxProducerPlugin,
            ))
            .add_systems(Update, synthetic_producer.in_set(VfxRequestSet))
            .add_systems(Update, observe_requests.after(VfxRequestSet));
        brawler::test_app::finalize(&mut app);

        app.update();

        assert_eq!(
            app.world().resource::<ObservedRequests>().0,
            [COMBAT_MUZZLE_VFX.as_str()]
        );
    }
}
