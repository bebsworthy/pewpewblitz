//! The authored roster: stable identities, per-character stats, signature weapon recipes, and the
//! two ultimate and two passive options each character offers.

use crate::builds::{
    BuildCatalog, PassiveDefinitionId, ResolvedFighterStats, UltimateDefinitionId,
};
use crate::combat::{WeaponCatalog, WeaponPresetId};
use crate::content::{GameplayContentFingerprint, fnv1a64};
use crate::weapon_parts::{WEAPON_PART_SLOT_COUNT, WeaponPartCatalog, WeaponPartDefinitionId};
use bevy::prelude::{FromWorld, Plugin, Resource};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const CHARACTER_CATALOG_SCHEMA_VERSION: u16 = 1;
/// Representation ceiling for the roster: bounds storage rows, the advertised catalog, and UI.
pub const MAX_CHARACTERS: usize = 32;
/// Every character offers exactly this many ultimates and passives to choose from.
pub const CHARACTER_OPTION_COUNT: usize = 2;
const CHARACTER_FINGERPRINT_VERSION: u16 = 1;
const MAX_CHARACTER_CATALOG_BYTES: usize = 16 * 1024;
const MAX_CHARACTER_KEY_BYTES: usize = 32;
const MAX_CHARACTER_NAME_BYTES: usize = 32;

/// Permanent wire and storage identity of one roster character. Never reused or renumbered.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct CharacterId(pub u16);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CharacterRole {
    Damage,
    Tank,
    Helper,
    Controller,
}

impl CharacterRole {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Damage => "Damage",
            Self::Tank => "Tank",
            Self::Helper => "Helper",
            Self::Controller => "Controller",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CharacterBody {
    Light,
    Normal,
    Heavy,
}

impl CharacterBody {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Normal => "Normal",
            Self::Heavy => "Heavy",
        }
    }
}

/// A character's signature weapon: one base plus authored parts (players never equip parts).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CharacterWeaponRecipe {
    pub base: WeaponPresetId,
    pub parts: Vec<WeaponPartDefinitionId>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CharacterDefinition {
    pub id: CharacterId,
    pub key: String,
    pub display_name: String,
    pub role: CharacterRole,
    pub body: CharacterBody,
    pub stats: ResolvedFighterStats,
    pub weapon: CharacterWeaponRecipe,
    pub ultimate_options: [UltimateDefinitionId; CHARACTER_OPTION_COUNT],
    pub passive_options: [PassiveDefinitionId; CHARACTER_OPTION_COUNT],
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CharacterCatalog {
    pub schema_version: u16,
    pub characters: Vec<CharacterDefinition>,
}

impl CharacterCatalog {
    pub fn embedded() -> Result<Self, String> {
        let catalog: Self = ron::from_str(include_str!("../../content/catalogs/characters.ron"))
            .map_err(|error| format!("embedded character catalog parse failed: {error}"))?;
        catalog.validate()?;
        Ok(catalog)
    }

    /// Structural rules that need no other catalog.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != CHARACTER_CATALOG_SCHEMA_VERSION
            || self.characters.is_empty()
            || self.characters.len() > MAX_CHARACTERS
            || self
                .characters
                .windows(2)
                .any(|pair| pair[0].id >= pair[1].id)
        {
            return Err("invalid character catalog envelope".into());
        }
        let mut keys = HashSet::new();
        for character in &self.characters {
            if character.id.0 == 0
                || !valid_key(&character.key)
                || !keys.insert(character.key.as_str())
                || character.display_name.trim().is_empty()
                || character.display_name.len() > MAX_CHARACTER_NAME_BYTES
            {
                return Err(format!("invalid character metadata for {}", character.key));
            }
            crate::builds::validate_fighter_stats(&character.stats)
                .map_err(|error| format!("character {}: {error}", character.key))?;
            if character.weapon.base.0 == 0
                || character.weapon.parts.len() > WEAPON_PART_SLOT_COUNT
                || character.ultimate_options[0] == character.ultimate_options[1]
                || character.passive_options[0] == character.passive_options[1]
            {
                return Err(format!("invalid character kit for {}", character.key));
            }
        }
        if postcard::to_allocvec(self)
            .map_or(true, |bytes| bytes.len() > MAX_CHARACTER_CATALOG_BYTES)
        {
            return Err("character catalog exceeds engine size ceiling".into());
        }
        Ok(())
    }

    /// References into the build, weapon, and weapon-part catalogs: every option combination of
    /// every character, and every direct-diagnostic pick, resolves.
    pub fn validate_references(
        &self,
        builds: &BuildCatalog,
        weapons: &WeaponCatalog,
        parts: &WeaponPartCatalog,
    ) -> Result<(), String> {
        self.validate_option_combinations(builds, weapons, parts)?;
        for pick in builds.direct_diagnostic_picks() {
            self.resolve(*pick, builds, weapons, parts)
                .map_err(|error| format!("direct diagnostic pick does not resolve: {error:?}"))?;
        }
        Ok(())
    }

    /// Every option combination of every character resolves against these catalogs.
    pub fn validate_option_combinations(
        &self,
        builds: &BuildCatalog,
        weapons: &WeaponCatalog,
        parts: &WeaponPartCatalog,
    ) -> Result<(), String> {
        self.validate()?;
        builds.validate_weapon_references(weapons)?;
        for character in &self.characters {
            for ultimate_id in character.ultimate_options {
                for passive_id in character.passive_options {
                    let choice = super::CharacterChoice {
                        character_id: character.id,
                        ultimate_id,
                        passive_id,
                    };
                    self.resolve(choice, builds, weapons, parts)
                        .map_err(|error| {
                            format!("character {} does not resolve: {error:?}", character.key)
                        })?;
                }
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn character(&self, id: CharacterId) -> Option<&CharacterDefinition> {
        self.characters.iter().find(|character| character.id == id)
    }

    pub fn canonical_fingerprint_material(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        postcard::to_allocvec(&(CHARACTER_FINGERPRINT_VERSION, self))
            .map_err(|error| format!("character fingerprint serialization failed: {error}"))
    }

    pub fn fingerprint(&self) -> Result<GameplayContentFingerprint, String> {
        Ok(GameplayContentFingerprint(fnv1a64(
            &self.canonical_fingerprint_material()?,
        )))
    }
}

fn valid_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_CHARACTER_KEY_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct CharacterCatalogResource(pub CharacterCatalog);

impl FromWorld for CharacterCatalogResource {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self(CharacterCatalog::embedded().expect("embedded character catalog is valid"))
    }
}

pub struct CharacterContentPlugin;

const FINGERPRINT_DOMAIN_SCHEMA_VERSION: u16 = 1;

impl Plugin for CharacterContentPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.init_resource::<CharacterCatalogResource>();
        crate::content::register_gameplay_fingerprint_contributor(
            app,
            crate::content::CHARACTERS_FINGERPRINT_DOMAIN,
            FINGERPRINT_DOMAIN_SCHEMA_VERSION,
            character_fingerprint_material,
        );
    }
}

fn character_fingerprint_material(world: &bevy::prelude::World) -> Result<Vec<u8>, String> {
    let characters = world
        .get_resource::<CharacterCatalogResource>()
        .ok_or_else(|| "character catalog resource is not installed".to_owned())?;
    let builds = world
        .get_resource::<crate::builds::BuildCatalogResource>()
        .ok_or_else(|| "build catalog resource is not installed".to_owned())?;
    let weapons = world
        .get_resource::<crate::combat::WeaponCatalogResource>()
        .ok_or_else(|| "weapon catalog resource is not installed".to_owned())?;
    let parts = world
        .get_resource::<crate::weapon_parts::WeaponPartCatalogResource>()
        .ok_or_else(|| "weapon-part catalog resource is not installed".to_owned())?;
    characters
        .0
        .validate_references(&builds.0, &weapons.0, &parts.0)?;
    characters.0.canonical_fingerprint_material()
}
