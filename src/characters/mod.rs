//! Authored roster characters: the player-facing pick that resolves into a match loadout.

mod definitions;

pub use definitions::{
    CHARACTER_CATALOG_SCHEMA_VERSION, CHARACTER_OPTION_COUNT, CharacterBody, CharacterCatalog,
    CharacterCatalogResource, CharacterContentPlugin, CharacterDefinition, CharacterId,
    CharacterRole, CharacterWeaponRecipe, MAX_CHARACTERS,
};

#[cfg(test)]
mod tests;
