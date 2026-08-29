//! Authored weapon-part content and deterministic base-weapon transformation.

mod definitions;
mod model;
mod resolver;

pub use definitions::{
    WEAPON_PART_CATALOG_SCHEMA_VERSION, WeaponPartCatalog, WeaponPartCatalogResource,
    WeaponPartContentPlugin, WeaponPartDefinition,
};
pub use model::{
    CanonicalDamageOverTimeModifier, CanonicalScalarModifier, CanonicalSlowModifier,
    CanonicalWeaponModifiers, MAX_PART_EFFECTS_PER_INSTANCE, MAX_WEAPON_PARTS_PER_PROFILE,
    WEAPON_PART_SLOT_COUNT, WeaponPartDefinitionId, WeaponPartEffect, WeaponPartInstance,
    WeaponPartInstanceId, WeaponPartModelError,
};
pub use resolver::{
    aggregate_weapon_part_effects, resolve_advertised_weapon_parts, resolve_weapon_parts,
};

#[cfg(test)]
mod tests;
