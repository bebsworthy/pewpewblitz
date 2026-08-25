//! Server-authored, connection-scoped saved-brawler selection catalog.

use super::{
    BrawlerDraft, BrawlerEdit, FighterProfileId, ProfileModelError, ProfileSnapshot, SavedBrawler,
    WeaponBaseId,
};
use crate::{
    builds::{
        BuildCatalog, PassiveDefinitionId, PassiveKind, ResolvedFighterStats, UltimateDefinitionId,
        UltimateKind, UltimateParameters,
    },
    combat::{EngineWeaponLimits, WeaponCatalog, WeaponConfiguration, WeaponRecipePolicy},
    content::fnv1a64,
};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{SeqAccess, Visitor},
};
use std::collections::HashSet;

pub const MAX_ADVERTISED_FIGHTER_PROFILES: usize = 16;
pub const MAX_ADVERTISED_WEAPON_BASES: usize = 16;
pub const MAX_ADVERTISED_ULTIMATES: usize = 32;
pub const MAX_ADVERTISED_PASSIVES: usize = 32;
pub const MAX_ADVERTISED_BRAWLER_CATALOG_BYTES: usize = 16 * 1024;

const ADVERTISED_BRAWLER_CATALOG_FORMAT_VERSION: u16 = 1;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BrawlerCatalogRevision(pub u64);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvertisedBrawlerLimits {
    pub maximum_saved_brawlers: u8,
    pub weapon_part_slot_count: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AdvertisedFighterProfile {
    pub id: FighterProfileId,
    #[serde(deserialize_with = "deserialize_catalog_key")]
    pub key: String,
    #[serde(deserialize_with = "crate::lobby::deserialize_presentation_name")]
    pub display_name: String,
    pub stats: ResolvedFighterStats,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AdvertisedWeaponBase {
    pub id: WeaponBaseId,
    #[serde(deserialize_with = "deserialize_catalog_key")]
    pub key: String,
    #[serde(deserialize_with = "crate::lobby::deserialize_presentation_name")]
    pub display_name: String,
    pub configuration: WeaponConfiguration,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AdvertisedUltimate {
    pub id: UltimateDefinitionId,
    #[serde(deserialize_with = "deserialize_catalog_key")]
    pub key: String,
    #[serde(deserialize_with = "crate::lobby::deserialize_presentation_name")]
    pub display_name: String,
    pub kind: UltimateKind,
    pub parameters: UltimateParameters,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AdvertisedPassive {
    pub id: PassiveDefinitionId,
    #[serde(deserialize_with = "deserialize_catalog_key")]
    pub key: String,
    #[serde(deserialize_with = "crate::lobby::deserialize_presentation_name")]
    pub display_name: String,
    pub kind: PassiveKind,
    pub saved_brawler_selectable: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AdvertisedBrawlerCatalog {
    pub revision: BrawlerCatalogRevision,
    pub limits: AdvertisedBrawlerLimits,
    pub weapon_policy: WeaponRecipePolicy,
    #[serde(deserialize_with = "deserialize_fighter_profiles")]
    pub fighter_profiles: Vec<AdvertisedFighterProfile>,
    #[serde(deserialize_with = "deserialize_weapon_bases")]
    pub weapon_bases: Vec<AdvertisedWeaponBase>,
    #[serde(deserialize_with = "deserialize_ultimates")]
    pub ultimates: Vec<AdvertisedUltimate>,
    #[serde(deserialize_with = "deserialize_passives")]
    pub passives: Vec<AdvertisedPassive>,
}

impl AdvertisedBrawlerCatalog {
    pub fn from_content(builds: &BuildCatalog, weapons: &WeaponCatalog) -> Result<Self, String> {
        let mut catalog = Self {
            revision: BrawlerCatalogRevision(0),
            limits: AdvertisedBrawlerLimits {
                maximum_saved_brawlers: u8::try_from(super::MAX_BRAWLERS_PER_PROFILE)
                    .map_err(|_| "saved-brawler limit does not fit wire metadata")?,
                weapon_part_slot_count: u8::try_from(crate::weapon_parts::WEAPON_PART_SLOT_COUNT)
                    .map_err(
                    |_| "weapon-part slot count does not fit wire metadata",
                )?,
            },
            weapon_policy: weapons.recipe_policy.clone(),
            fighter_profiles: vec![
                AdvertisedFighterProfile {
                    id: FighterProfileId(1),
                    key: "default".into(),
                    display_name: "Default".into(),
                    stats: builds.fighter_profiles.default,
                },
                AdvertisedFighterProfile {
                    id: FighterProfileId(2),
                    key: "lightweight".into(),
                    display_name: "Lightweight".into(),
                    stats: builds.fighter_profiles.lightweight,
                },
                AdvertisedFighterProfile {
                    id: FighterProfileId(3),
                    key: "reinforced".into(),
                    display_name: "Reinforced".into(),
                    stats: builds.fighter_profiles.reinforced,
                },
            ],
            weapon_bases: weapons
                .presets
                .iter()
                .map(|definition| AdvertisedWeaponBase {
                    id: WeaponBaseId(definition.id.0),
                    key: definition.key.clone(),
                    display_name: definition.display_name.clone(),
                    configuration: definition.configuration.clone(),
                })
                .collect(),
            ultimates: builds
                .ultimates
                .iter()
                .map(|definition| AdvertisedUltimate {
                    id: definition.id,
                    key: definition.key.clone(),
                    display_name: definition.display_name.clone(),
                    kind: definition.kind,
                    parameters: definition.parameters,
                })
                .collect(),
            passives: builds
                .passives
                .iter()
                .map(|definition| AdvertisedPassive {
                    id: definition.id,
                    key: definition.key.clone(),
                    display_name: definition.display_name.clone(),
                    kind: definition.kind,
                    saved_brawler_selectable: !matches!(
                        definition.kind,
                        PassiveKind::LightweightFrame | PassiveKind::ReinforcedFrame
                    ),
                })
                .collect(),
        };
        catalog.revision = catalog.expected_revision()?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.limits.maximum_saved_brawlers == 0
            || usize::from(self.limits.maximum_saved_brawlers) > super::MAX_BRAWLERS_PER_PROFILE
            || usize::from(self.limits.weapon_part_slot_count)
                != crate::weapon_parts::WEAPON_PART_SLOT_COUNT
            || self.fighter_profiles.is_empty()
            || self.fighter_profiles.len() > MAX_ADVERTISED_FIGHTER_PROFILES
            || self.weapon_bases.is_empty()
            || self.weapon_bases.len() > MAX_ADVERTISED_WEAPON_BASES
            || self.ultimates.is_empty()
            || self.ultimates.len() > MAX_ADVERTISED_ULTIMATES
            || self.passives.len() < 2
            || self.passives.len() > MAX_ADVERTISED_PASSIVES
        {
            return Err("invalid advertised brawler catalog envelope".into());
        }
        validate_ordered_metadata(
            self.fighter_profiles
                .iter()
                .map(|definition| (definition.id.0, &definition.key, &definition.display_name)),
        )?;
        validate_ordered_metadata(
            self.weapon_bases
                .iter()
                .map(|definition| (definition.id.0, &definition.key, &definition.display_name)),
        )?;
        validate_ordered_metadata(
            self.ultimates
                .iter()
                .map(|definition| (definition.id.0, &definition.key, &definition.display_name)),
        )?;
        validate_ordered_metadata(
            self.passives
                .iter()
                .map(|definition| (definition.id.0, &definition.key, &definition.display_name)),
        )?;
        if self.fighter_profiles.iter().any(|definition| {
            definition.stats.maximum_health == 0
                || !definition.stats.movement_speed.is_finite()
                || definition.stats.movement_speed <= 0.0
                || !definition.stats.reveal_proximity_radius.is_finite()
                || definition.stats.reveal_proximity_radius
                    < crate::builds::MIN_REVEAL_PROXIMITY_RADIUS
                || definition.stats.reveal_proximity_radius
                    > crate::builds::MAX_REVEAL_PROXIMITY_RADIUS
        }) {
            return Err("invalid advertised fighter profile".into());
        }
        for definition in &self.weapon_bases {
            definition.configuration.validate(
                &self.weapon_policy,
                EngineWeaponLimits::default(),
                None,
            )?;
        }
        validate_ultimate_parameters(&self.ultimates)?;
        let selectable_passives = self
            .passives
            .iter()
            .filter(|definition| definition.saved_brawler_selectable)
            .count();
        if selectable_passives < 2
            || self.passives.iter().any(|definition| {
                definition.saved_brawler_selectable
                    && matches!(
                        definition.kind,
                        PassiveKind::LightweightFrame | PassiveKind::ReinforcedFrame
                    )
            })
        {
            return Err("invalid saved-brawler passive eligibility".into());
        }
        if self.revision != self.expected_revision()? {
            return Err("advertised brawler catalog revision mismatch".into());
        }
        let bytes = postcard::to_allocvec(self).map_err(|error| error.to_string())?;
        if bytes.len() > MAX_ADVERTISED_BRAWLER_CATALOG_BYTES {
            return Err("advertised brawler catalog exceeds its wire bound".into());
        }
        Ok(())
    }

    #[must_use]
    pub fn fighter(&self, id: FighterProfileId) -> Option<&AdvertisedFighterProfile> {
        self.fighter_profiles
            .iter()
            .find(|definition| definition.id == id)
    }

    #[must_use]
    pub fn weapon(&self, id: WeaponBaseId) -> Option<&AdvertisedWeaponBase> {
        self.weapon_bases
            .iter()
            .find(|definition| definition.id == id)
    }

    #[must_use]
    pub fn ultimate(&self, id: UltimateDefinitionId) -> Option<&AdvertisedUltimate> {
        self.ultimates.iter().find(|definition| definition.id == id)
    }

    #[must_use]
    pub fn passive(&self, id: PassiveDefinitionId) -> Option<&AdvertisedPassive> {
        self.passives.iter().find(|definition| definition.id == id)
    }

    pub fn selectable_passives(&self) -> impl Iterator<Item = &AdvertisedPassive> {
        self.passives
            .iter()
            .filter(|definition| definition.saved_brawler_selectable)
    }

    pub fn validate_draft(&self, draft: &BrawlerDraft) -> Result<(), ProfileModelError> {
        self.validate_recipe(
            draft.fighter_profile_id,
            draft.weapon_base_id,
            draft.ultimate_id,
            draft.passive_ids,
        )
    }

    pub fn validate_edit(&self, edit: &BrawlerEdit) -> Result<(), ProfileModelError> {
        self.validate_mutable_recipe(edit.ultimate_id, edit.passive_ids)
    }

    pub fn validate_profile(&self, profile: &ProfileSnapshot) -> Result<(), ProfileModelError> {
        profile.validate_structure()?;
        if profile.brawlers.len() > usize::from(self.limits.maximum_saved_brawlers) {
            return Err(ProfileModelError::TooManyBrawlers);
        }
        for brawler in &profile.brawlers {
            self.validate_brawler(brawler)?;
        }
        Ok(())
    }

    pub fn validate_brawler(&self, brawler: &SavedBrawler) -> Result<(), ProfileModelError> {
        brawler.validate_structure()?;
        self.validate_recipe(
            brawler.fighter_profile_id,
            brawler.weapon_base_id,
            brawler.ultimate_id,
            brawler.passive_ids,
        )
    }

    fn validate_recipe(
        &self,
        fighter: FighterProfileId,
        weapon: WeaponBaseId,
        ultimate: UltimateDefinitionId,
        passives: [PassiveDefinitionId; 2],
    ) -> Result<(), ProfileModelError> {
        if self.fighter(fighter).is_none() {
            return Err(ProfileModelError::UnknownFighterProfile);
        }
        if self.weapon(weapon).is_none() {
            return Err(ProfileModelError::UnknownWeaponBase);
        }
        self.validate_mutable_recipe(ultimate, passives)
    }

    fn validate_mutable_recipe(
        &self,
        ultimate: UltimateDefinitionId,
        passives: [PassiveDefinitionId; 2],
    ) -> Result<(), ProfileModelError> {
        if self.ultimate(ultimate).is_none() {
            return Err(ProfileModelError::UnknownUltimate);
        }
        if passives.iter().any(|id| {
            self.passive(*id)
                .is_none_or(|definition| !definition.saved_brawler_selectable)
        }) {
            return Err(ProfileModelError::UnknownPassive);
        }
        if passives[0] == passives[1] {
            return Err(ProfileModelError::DuplicatePassive);
        }
        Ok(())
    }

    fn expected_revision(&self) -> Result<BrawlerCatalogRevision, String> {
        let bytes = postcard::to_allocvec(&(
            ADVERTISED_BRAWLER_CATALOG_FORMAT_VERSION,
            self.limits,
            &self.weapon_policy,
            &self.fighter_profiles,
            &self.weapon_bases,
            &self.ultimates,
            &self.passives,
        ))
        .map_err(|error| error.to_string())?;
        Ok(BrawlerCatalogRevision(fnv1a64(&bytes)))
    }
}

fn validate_ultimate_parameters(definitions: &[AdvertisedUltimate]) -> Result<(), String> {
    if definitions.iter().any(|definition| {
        !matches!(
            (definition.kind, definition.parameters),
            (UltimateKind::Dash, UltimateParameters::Dash)
                | (UltimateKind::Sentry, UltimateParameters::Sentry)
                | (
                    UltimateKind::SelfCloak,
                    UltimateParameters::SelfCloak {
                        duration_ticks: 1..=3_600
                    }
                )
                | (
                    UltimateKind::RevealScan,
                    UltimateParameters::RevealScan {
                        maximum_range_milliunits: 1..=4_096_000,
                        radius_milliunits: 1..=2_048_000,
                        reveal_ticks: 1..=3_600,
                    }
                )
                | (
                    UltimateKind::ConcealmentField,
                    UltimateParameters::ConcealmentField {
                        maximum_range_milliunits: 1..=4_096_000,
                        radius_milliunits: 1..=2_048_000,
                        duration_ticks: 1..=3_600,
                    }
                )
        )
    }) {
        return Err("invalid advertised ultimate parameters".into());
    }
    Ok(())
}

fn validate_ordered_metadata<'a>(
    entries: impl IntoIterator<Item = (u16, &'a String, &'a String)>,
) -> Result<(), String> {
    let mut previous = None;
    let mut keys = HashSet::new();
    for (id, key, display_name) in entries {
        if id == 0
            || previous.is_some_and(|previous| previous >= id)
            || !valid_key(key)
            || crate::lobby::validate_presentation_name(display_name).is_err()
            || !keys.insert(key)
        {
            return Err("invalid advertised brawler metadata".into());
        }
        previous = Some(id);
    }
    Ok(())
}

fn valid_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn deserialize_catalog_key<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct KeyVisitor;

    impl Visitor<'_> for KeyVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter.write_str("a canonical catalog key of at most 32 ASCII bytes")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if valid_key(value) {
                Ok(value.to_owned())
            } else {
                Err(E::custom("invalid advertised brawler catalog key"))
            }
        }
    }

    deserializer.deserialize_str(KeyVisitor)
}

fn deserialize_bounded_vec<'de, D, T, const MAXIMUM: usize>(
    deserializer: D,
) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct BoundedVecVisitor<T, const MAXIMUM: usize>(core::marker::PhantomData<T>);

    impl<'de, T, const MAXIMUM: usize> Visitor<'de> for BoundedVecVisitor<T, MAXIMUM>
    where
        T: Deserialize<'de>,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(formatter, "a sequence with at most {MAXIMUM} entries")
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            if sequence.size_hint().is_some_and(|length| length > MAXIMUM) {
                return Err(serde::de::Error::invalid_length(
                    sequence.size_hint().unwrap_or(MAXIMUM + 1),
                    &self,
                ));
            }
            let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(MAXIMUM));
            while let Some(value) = sequence.next_element()? {
                if values.len() == MAXIMUM {
                    return Err(serde::de::Error::invalid_length(MAXIMUM + 1, &self));
                }
                values.push(value);
            }
            Ok(values)
        }
    }

    deserializer.deserialize_seq(BoundedVecVisitor::<T, MAXIMUM>(core::marker::PhantomData))
}

fn deserialize_fighter_profiles<'de, D>(
    deserializer: D,
) -> Result<Vec<AdvertisedFighterProfile>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, _, MAX_ADVERTISED_FIGHTER_PROFILES>(deserializer)
}

fn deserialize_weapon_bases<'de, D>(deserializer: D) -> Result<Vec<AdvertisedWeaponBase>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, _, MAX_ADVERTISED_WEAPON_BASES>(deserializer)
}

fn deserialize_ultimates<'de, D>(deserializer: D) -> Result<Vec<AdvertisedUltimate>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, _, MAX_ADVERTISED_ULTIMATES>(deserializer)
}

fn deserialize_passives<'de, D>(deserializer: D) -> Result<Vec<AdvertisedPassive>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, _, MAX_ADVERTISED_PASSIVES>(deserializer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn embedded() -> AdvertisedBrawlerCatalog {
        AdvertisedBrawlerCatalog::from_content(
            &BuildCatalog::embedded().unwrap(),
            &WeaponCatalog::embedded().unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn embedded_catalog_is_bounded_and_includes_concealment_field() {
        let catalog = embedded();
        assert_eq!(
            catalog
                .ultimate(UltimateDefinitionId(5))
                .map(|definition| definition.display_name.as_str()),
            Some("Concealment Field")
        );
        assert!(
            postcard::to_allocvec(&catalog).unwrap().len() <= MAX_ADVERTISED_BRAWLER_CATALOG_BYTES
        );
        catalog.validate().unwrap();
    }

    #[test]
    fn catalog_validation_and_selection_do_not_assume_contiguous_ids() {
        let mut catalog = embedded();
        catalog.fighter_profiles[0].id = FighterProfileId(10);
        catalog.fighter_profiles[1].id = FighterProfileId(20);
        catalog.fighter_profiles[2].id = FighterProfileId(30);
        catalog.weapon_bases[0].id = WeaponBaseId(10);
        catalog.weapon_bases[1].id = WeaponBaseId(20);
        catalog.weapon_bases[2].id = WeaponBaseId(30);
        catalog.weapon_bases[3].id = WeaponBaseId(40);
        catalog.revision = catalog.expected_revision().unwrap();
        catalog.validate().unwrap();
        let draft = BrawlerDraft {
            name: "Sparse IDs".into(),
            fighter_profile_id: FighterProfileId(20),
            weapon_base_id: WeaponBaseId(30),
            ultimate_id: catalog.ultimates[0].id,
            passive_ids: [
                catalog.selectable_passives().next().unwrap().id,
                catalog.selectable_passives().nth(1).unwrap().id,
            ],
        };
        catalog.validate_draft(&draft).unwrap();
    }

    #[test]
    fn every_advertised_catalog_section_changes_the_revision() {
        let catalog = embedded();
        let mut variants = Vec::new();
        let mut changed = catalog.clone();
        changed.limits.maximum_saved_brawlers -= 1;
        variants.push(changed);
        let mut changed = catalog.clone();
        changed.weapon_policy.max_capacity -= 1;
        variants.push(changed);
        let mut changed = catalog.clone();
        changed.fighter_profiles[0].display_name.push('!');
        variants.push(changed);
        let mut changed = catalog.clone();
        changed.weapon_bases[0].display_name.push('!');
        variants.push(changed);
        let mut changed = catalog.clone();
        changed.ultimates[0].display_name.push('!');
        variants.push(changed);
        let mut changed = catalog.clone();
        changed.passives[0].display_name.push('!');
        variants.push(changed);

        for changed in variants {
            assert_ne!(changed.expected_revision().unwrap(), catalog.revision);
            assert!(changed.validate().is_err());
        }
    }
}
