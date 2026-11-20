//! Per-family held weapon models (BRL-q29d).
//!
//! The primary weapon's replicated delivery picks a [`WeaponFamily`]; the authored client catalog
//! `assets/catalogs/fighter_weapons.ron` maps each family to one original weapon GLB (built by
//! `asset_src/weapons/build_weapons.py`). A [`HeldWeapon`] on an
//! imported Riff scene keeps exactly one weapon instance on the weapon socket and swaps it when the
//! family changes, so a late or changed loadout never leaves a stale model in the hand. Riff's
//! two-handed clips already bake the support hand onto each model's `grip_secondary` anchor, so no
//! IK runs here. Presentation only: nothing here feeds simulation or the gameplay fingerprint.

use super::fighter_character::WeaponFamily;
use bevy::prelude::*;
use serde::Deserialize;

const FIGHTER_WEAPON_CATALOG: &str =
    include_str!("../../../../assets/catalogs/fighter_weapons.ron");
const FIGHTER_WEAPON_SCHEMA_VERSION: u16 = 1;
/// Held weapons are original builds; a model outside this directory is refused.
const WEAPON_MODEL_ROOT: &str = "brawler/models/original/weapons/";

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct FighterWeaponCatalogSource {
    schema_version: u16,
    weapons: Vec<FighterWeaponSource>,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct FighterWeaponSource {
    family: WeaponFamily,
    model: String,
}

/// The held-weapon model of one family.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FighterWeaponModel {
    pub(crate) model: String,
}

/// Validated per-family held weapons, embedded from `assets/catalogs/fighter_weapons.ron`.
#[derive(Resource, Clone, Debug, PartialEq)]
pub(crate) struct FighterWeaponCatalog {
    weapons: [FighterWeaponModel; 4],
}

impl FighterWeaponCatalog {
    pub(crate) fn embedded() -> Result<Self, String> {
        Self::from_ron(FIGHTER_WEAPON_CATALOG)
    }

    fn from_ron(source: &str) -> Result<Self, String> {
        let source: FighterWeaponCatalogSource = ron::from_str(source)
            .map_err(|error| format!("fighter weapon catalog parse failed: {error}"))?;
        if source.schema_version != FIGHTER_WEAPON_SCHEMA_VERSION {
            return Err(format!(
                "fighter weapon catalog schema {} is not {FIGHTER_WEAPON_SCHEMA_VERSION}",
                source.schema_version
            ));
        }
        let mut slots: [Option<FighterWeaponModel>; 4] = Default::default();
        for entry in source.weapons {
            let slot = &mut slots[entry.family.index()];
            if slot.is_some() {
                return Err(format!(
                    "fighter weapon family {:?} is listed twice",
                    entry.family
                ));
            }
            *slot = Some(validated(entry)?);
        }
        let mut weapons = Vec::with_capacity(slots.len());
        for (family, slot) in WeaponFamily::ALL.into_iter().zip(slots) {
            weapons.push(slot.ok_or_else(|| {
                format!("fighter weapon catalog has no model for family {family:?}")
            })?);
        }
        let weapons = weapons
            .try_into()
            .map_err(|_| "fighter weapon catalog must cover four families".to_owned())?;
        Ok(Self { weapons })
    }

    pub(crate) fn model(&self, family: WeaponFamily) -> &FighterWeaponModel {
        &self.weapons[family.index()]
    }
}

fn validated(entry: FighterWeaponSource) -> Result<FighterWeaponModel, String> {
    let family = entry.family;
    let model = entry.model;
    if !model.starts_with(WEAPON_MODEL_ROOT)
        || !std::path::Path::new(&model)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("glb"))
        || model.contains("..")
    {
        return Err(format!(
            "fighter weapon {family:?} model {model:?} must be a GLB under {WEAPON_MODEL_ROOT}"
        ));
    }
    Ok(FighterWeaponModel { model })
}

/// The one weapon model attached to an imported fighter's weapon socket.
#[derive(Component, Debug)]
pub(crate) struct HeldWeapon {
    socket: Entity,
    attached: Option<(WeaponFamily, Entity)>,
}

impl HeldWeapon {
    pub(crate) const fn new(socket: Entity) -> Self {
        Self {
            socket,
            attached: None,
        }
    }

    /// Despawns a model of another family and returns the socket to attach `family`'s model
    /// under, or `None` when that model is already attached.
    pub(crate) fn begin_swap(
        &mut self,
        commands: &mut Commands,
        family: WeaponFamily,
    ) -> Option<Entity> {
        match self.attached.take() {
            Some((current, entity)) if current == family => {
                self.attached = Some((current, entity));
                None
            }
            previous => {
                if let Some((_, entity)) = previous {
                    commands.entity(entity).try_despawn();
                }
                Some(self.socket)
            }
        }
    }

    pub(crate) fn attached(&mut self, family: WeaponFamily, entity: Entity) {
        self.attached = Some((family, entity));
    }
}

/// The family of an advertised weapon base; an unknown ID keeps the one-handed default.
pub(crate) fn advertised_weapon_family(
    catalog: &brawler::profiles::AdvertisedCharacterCatalog,
    id: brawler::combat::WeaponPresetId,
) -> WeaponFamily {
    catalog
        .weapon(id)
        .map_or(WeaponFamily::OneHanded, |weapon| {
            WeaponFamily::of(&weapon.configuration.recipe.delivery)
        })
}

/// The family of the lobby profile's selected character's signature weapon (one-handed when the
/// character is not advertised).
pub(crate) fn selected_weapon_family(membership: &crate::ClientLobbyMembership) -> WeaponFamily {
    membership
        .character_catalog
        .character(membership.profile.selected_character_id)
        .map_or(WeaponFamily::OneHanded, |character| {
            advertised_weapon_family(&membership.character_catalog, character.weapon_base)
        })
}

#[cfg(test)]
mod tests;
