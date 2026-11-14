//! Per-family held weapon models (BRL-q29d).
//!
//! The primary weapon's replicated delivery picks a [`WeaponFamily`]; the authored client catalog
//! `assets/catalogs/fighter_weapons.ron` maps each family to one original weapon GLB (built by
//! `asset_src/weapons/build_weapons.py`) and its shell-preview framing. A [`HeldWeapon`] on an
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
const MIN_PREVIEW_SCALE: f32 = 0.2;
const MAX_PREVIEW_SCALE: f32 = 3.0;
/// The preview centre is in the model's source units; weapons are under two units long.
const MAX_PREVIEW_CENTER: f32 = 2.0;

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
    preview_scale: f32,
    preview_center: (f32, f32, f32),
}

/// The held-weapon model and shell-preview framing of one family.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FighterWeaponModel {
    pub(crate) model: String,
    preview_scale: f32,
    preview_center: Vec3,
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

    /// Centres and scales the family's model for the shell weapon preview camera.
    pub(crate) fn preview_transform(&self, family: WeaponFamily) -> Transform {
        let model = self.model(family);
        Transform {
            translation: -model.preview_center * model.preview_scale,
            scale: Vec3::splat(model.preview_scale),
            ..default()
        }
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
    if !entry.preview_scale.is_finite()
        || !(MIN_PREVIEW_SCALE..=MAX_PREVIEW_SCALE).contains(&entry.preview_scale)
    {
        return Err(format!(
            "fighter weapon {family:?} preview_scale {} is outside {MIN_PREVIEW_SCALE}..={MAX_PREVIEW_SCALE}",
            entry.preview_scale
        ));
    }
    let center = Vec3::from(entry.preview_center);
    if !center.is_finite() || center.abs().max_element() > MAX_PREVIEW_CENTER {
        return Err(format!(
            "fighter weapon {family:?} preview_center {center} is outside +-{MAX_PREVIEW_CENTER}"
        ));
    }
    Ok(FighterWeaponModel {
        model,
        preview_scale: entry.preview_scale,
        preview_center: center,
    })
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
    catalog: &brawler::profiles::AdvertisedBrawlerCatalog,
    id: brawler::profiles::WeaponBaseId,
) -> WeaponFamily {
    catalog
        .weapon(id)
        .map_or(WeaponFamily::OneHanded, |weapon| {
            WeaponFamily::of(&weapon.configuration.recipe.delivery)
        })
}

/// The family of the lobby profile's selected saved brawler (one-handed when none is selected).
pub(crate) fn selected_weapon_family(membership: &crate::ClientLobbyMembership) -> WeaponFamily {
    let profile = &membership.profile;
    profile
        .selected_brawler_id
        .and_then(|id| profile.brawlers.iter().find(|brawler| brawler.id == id))
        .map_or(WeaponFamily::OneHanded, |brawler| {
            advertised_weapon_family(&membership.brawler_catalog, brawler.weapon_base_id)
        })
}

#[cfg(test)]
mod tests;
