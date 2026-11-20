//! Authored roster characters: the player-facing pick that resolves into a match loadout.

mod definitions;
mod resolution;

pub use definitions::{
    CHARACTER_CATALOG_SCHEMA_VERSION, CHARACTER_OPTION_COUNT, CharacterBody, CharacterCatalog,
    CharacterCatalogResource, CharacterContentPlugin, CharacterDefinition, CharacterId,
    CharacterRole, CharacterWeaponRecipe, MAX_CHARACTERS,
};
pub use resolution::{
    CharacterChoice, EmbeddedLoadoutCatalogs, LoadoutCatalogResources, LoadoutCatalogs,
};

#[cfg(test)]
mod tests;
