//! Client-only semantic VFX request registration and authored profile resolution.

mod catalog;
mod producers;
mod registry;
mod request;

pub(crate) use producers::{
    CombatVfxProducerPlugin, HeistVfxProducerPlugin, PickupVfxProducerPlugin, VfxRequestSet,
    WorldObjectVfxProducerPlugin,
};

pub(crate) use catalog::{
    VfxLifetime, VfxMaterialKey, VfxProfile, VfxRendererFamily, embedded_vfx_catalog_fingerprint,
};
#[cfg(test)]
pub(crate) use registry::VfxRegistryPlugin;
pub(crate) use registry::{
    ProductionVfxPlugin, VfxAppExt, VfxRegistry, VfxRequestCapabilities, VfxRequestRegistration,
};
pub(crate) use request::{
    COMBAT_DAMAGE_VFX, COMBAT_IMPACT_VFX, COMBAT_MUZZLE_VFX, COMBAT_RESET_VFX,
    COMBAT_VFX_PRODUCER_RANK, DEMOLITION_STRIKE_VFX, ELEMENTAL_FIELD_VFX, HEIST_CRITICAL_VFX,
    HEIST_DAMAGED_VFX, HEIST_DESTROYED_VFX, HEIST_VFX_PRODUCER_RANK, PICKUP_COLLECTED_VFX,
    PICKUP_EXPIRED_VFX, PICKUP_SPAWNED_VFX, PICKUP_VFX_PRODUCER_RANK, REVEAL_SCAN_VFX, VfxDeadline,
    VfxRequest, VfxRequestKey, VfxRequestOrder, WORLD_OBJECT_DAMAGED_VFX,
    WORLD_OBJECT_EXPLOSION_VFX, WORLD_OBJECT_VFX_PRODUCER_RANK,
};
