//! Client-only semantic VFX request registration and authored profile resolution.

mod catalog;
mod producers;
mod recipes;
mod registry;
mod request;
mod signature;

pub(crate) use producers::{
    CombatVfxProducerPlugin, HeistVfxProducerPlugin, PickupVfxProducerPlugin, VfxRequestSet,
    WorldObjectVfxProducerPlugin,
};

pub(crate) use catalog::{
    VfxLifetime, VfxMaterialKey, VfxRendererFamily, embedded_vfx_catalog_fingerprint,
};
pub(crate) use recipes::{
    ResolvedVfxRecipe, VfxMeshLayer, VfxParticleLayer, VfxParticleProfile, VfxReadabilityClass,
    VfxTimeline, vfx_counter_sample,
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
pub(crate) use signature::{
    CombatVisualSignature, VfxAccent, VfxAuthoritativeTiming, VfxCarrier, VfxContextOverride,
    VfxGeometry, VfxPhase, VfxPresentationIdentity, VfxPresentationVariant, VfxRelationship,
    VfxSignatureCache, VfxSignatureSet, accent_for_elemental_field, accents_for_payload_effects,
    update_vfx_signature_cache, vfx_presentation_variant,
};
