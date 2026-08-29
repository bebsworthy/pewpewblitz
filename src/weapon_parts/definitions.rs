use super::model::{
    MAX_PART_TYPE_BYTES, WeaponPartDefinitionId, WeaponPartEffect, WeaponPartInstance,
    WeaponPartInstanceId, WeaponPartModelError, valid_text,
};
use crate::content::{GameplayContentFingerprint, fnv1a64};
use bevy::prelude::{FromWorld, Plugin, Resource};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const WEAPON_PART_CATALOG_SCHEMA_VERSION: u16 = 2;
const WEAPON_PART_FINGERPRINT_VERSION: u16 = 2;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WeaponPartDefinition {
    pub id: WeaponPartDefinitionId,
    pub key: String,
    pub display_name: String,
    pub presentation_type: String,
    pub effects: Vec<WeaponPartEffect>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WeaponPartCatalog {
    pub schema_version: u16,
    pub starter_set_revision: u16,
    pub definitions: Vec<WeaponPartDefinition>,
}

impl WeaponPartCatalog {
    pub fn embedded() -> Result<Self, String> {
        let catalog: Self = ron::from_str(include_str!("../../content/catalogs/weapon_parts.ron"))
            .map_err(|error| format!("embedded weapon-part catalog parse failed: {error}"))?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != WEAPON_PART_CATALOG_SCHEMA_VERSION
            || self.starter_set_revision != 2
            || self.definitions.len() != 12
            || self
                .definitions
                .windows(2)
                .any(|pair| pair[0].id >= pair[1].id)
        {
            return Err("invalid weapon-part catalog envelope".into());
        }
        let mut keys = HashSet::new();
        for definition in &self.definitions {
            if definition.id.0 == 0
                || !valid_key(&definition.key)
                || !keys.insert(definition.key.as_str())
                || !valid_text(&definition.display_name, super::model::MAX_PART_NAME_BYTES)
                || !valid_text(&definition.presentation_type, MAX_PART_TYPE_BYTES)
            {
                return Err(format!(
                    "invalid weapon-part metadata for {}",
                    definition.key
                ));
            }
            let instance = WeaponPartInstance {
                id: WeaponPartInstanceId::new(u128::from(definition.id.0))
                    .map_err(|error| error.to_string())?,
                inventory_ordinal: u64::from(definition.id.0),
                definition_id: definition.id,
                display_name: definition.display_name.clone(),
                effects: definition.effects.clone(),
            };
            instance.validate().map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    #[must_use]
    pub fn definition(&self, id: WeaponPartDefinitionId) -> Option<&WeaponPartDefinition> {
        self.definitions
            .iter()
            .find(|definition| definition.id == id)
    }

    pub fn starter_instances(
        &self,
        mut next_id: impl FnMut() -> Result<WeaponPartInstanceId, WeaponPartModelError>,
    ) -> Result<Vec<WeaponPartInstance>, WeaponPartModelError> {
        self.definitions
            .iter()
            .enumerate()
            .map(|(index, definition)| {
                let instance = WeaponPartInstance {
                    id: next_id()?,
                    inventory_ordinal: u64::try_from(index + 1)
                        .map_err(|_| WeaponPartModelError::ArithmeticOverflow)?,
                    definition_id: definition.id,
                    display_name: definition.display_name.clone(),
                    effects: definition.effects.clone(),
                };
                instance.validate()?;
                Ok(instance)
            })
            .collect()
    }

    pub fn canonical_fingerprint_material(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let gameplay: Vec<_> = self
            .definitions
            .iter()
            .map(|definition| (definition.id, &definition.effects))
            .collect();
        postcard::to_allocvec(&(
            WEAPON_PART_FINGERPRINT_VERSION,
            self.starter_set_revision,
            gameplay,
        ))
        .map_err(|error| error.to_string())
    }

    pub fn fingerprint(&self) -> Result<GameplayContentFingerprint, String> {
        Ok(GameplayContentFingerprint(fnv1a64(
            &self.canonical_fingerprint_material()?,
        )))
    }
}

fn valid_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct WeaponPartCatalogResource(pub WeaponPartCatalog);

impl FromWorld for WeaponPartCatalogResource {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self(WeaponPartCatalog::embedded().expect("embedded weapon-part catalog is valid"))
    }
}

pub struct WeaponPartContentPlugin;

impl Plugin for WeaponPartContentPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.init_resource::<WeaponPartCatalogResource>();
    }
}
