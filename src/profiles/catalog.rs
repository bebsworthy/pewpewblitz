//! Server-authored, connection-scoped character-select catalog (BRL-egn6).

use super::{AccountId, ProfileModelError, ProfileRevision, ProfileSnapshot};
use crate::{
    builds::{
        BuildCatalog, PassiveDefinitionId, PassiveKind, ResolvedFighterStats,
        ULTIMATE_PARAMETER_BOUNDS, UltimateDefinitionId, UltimateKind, UltimateParameters,
    },
    characters::{
        CHARACTER_OPTION_COUNT, CharacterBody, CharacterCatalog, CharacterChoice, CharacterId,
        CharacterRole, MAX_CHARACTERS,
    },
    combat::{
        EngineWeaponLimits, WeaponCatalog, WeaponConfiguration, WeaponPresetId, WeaponRecipePolicy,
    },
    content::fnv1a64,
};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{SeqAccess, Visitor},
};
use std::collections::HashSet;

pub const MAX_ADVERTISED_CHARACTERS: usize = MAX_CHARACTERS;
pub const MAX_ADVERTISED_WEAPON_BASES: usize = crate::combat::MAX_WEAPON_PRESETS;
pub const MAX_ADVERTISED_ULTIMATES: usize = crate::builds::MAX_ULTIMATE_DEFINITIONS;
pub const MAX_ADVERTISED_PASSIVES: usize = crate::builds::MAX_PASSIVE_DEFINITIONS;
pub const MAX_ADVERTISED_CHARACTER_CATALOG_BYTES: usize = 16 * 1024;

const ADVERTISED_CHARACTER_CATALOG_FORMAT_VERSION: u16 = 6;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CharacterCatalogRevision(pub u64);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AdvertisedCharacter {
    pub id: CharacterId,
    #[serde(deserialize_with = "deserialize_catalog_key")]
    pub key: String,
    #[serde(deserialize_with = "crate::lobby::deserialize_presentation_name")]
    pub display_name: String,
    pub role: CharacterRole,
    pub body: CharacterBody,
    pub stats: ResolvedFighterStats,
    pub weapon_base: WeaponPresetId,
    pub ultimate_options: [UltimateDefinitionId; CHARACTER_OPTION_COUNT],
    pub passive_options: [PassiveDefinitionId; CHARACTER_OPTION_COUNT],
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AdvertisedWeaponBase {
    pub id: WeaponPresetId,
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
}

/// The roster a lobby offers: every character with its stats, signature weapon base, and option
/// IDs, plus the display metadata of the weapons, ultimates, and passives those reference.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AdvertisedCharacterCatalog {
    pub revision: CharacterCatalogRevision,
    pub fighter_body: crate::builds::FighterBody,
    pub weapon_policy: WeaponRecipePolicy,
    #[serde(deserialize_with = "deserialize_characters")]
    pub characters: Vec<AdvertisedCharacter>,
    #[serde(deserialize_with = "deserialize_weapon_bases")]
    pub weapon_bases: Vec<AdvertisedWeaponBase>,
    #[serde(deserialize_with = "deserialize_ultimates")]
    pub ultimates: Vec<AdvertisedUltimate>,
    #[serde(deserialize_with = "deserialize_passives")]
    pub passives: Vec<AdvertisedPassive>,
}

impl AdvertisedCharacterCatalog {
    pub fn from_content(
        characters: &CharacterCatalog,
        builds: &BuildCatalog,
        weapons: &WeaponCatalog,
    ) -> Result<Self, String> {
        builds.validate_weapon_references(weapons)?;
        characters.validate()?;
        let mut catalog = Self {
            revision: CharacterCatalogRevision(0),
            fighter_body: builds.fighter_body,
            weapon_policy: weapons.recipe_policy.clone(),
            characters: characters
                .characters
                .iter()
                .map(|definition| AdvertisedCharacter {
                    id: definition.id,
                    key: definition.key.clone(),
                    display_name: definition.display_name.clone(),
                    role: definition.role,
                    body: definition.body,
                    stats: definition.stats,
                    weapon_base: definition.weapon.base,
                    ultimate_options: definition.ultimate_options,
                    passive_options: definition.passive_options,
                })
                .collect(),
            weapon_bases: weapons
                .presets
                .iter()
                .map(|definition| AdvertisedWeaponBase {
                    id: definition.id,
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
                })
                .collect(),
        };
        catalog.revision = catalog.expected_revision()?;
        catalog.validate()?;
        Ok(catalog)
    }

    /// The embedded catalogs as one lobby would advertise them.
    pub fn embedded() -> Result<Self, String> {
        Self::from_content(
            &CharacterCatalog::embedded()?,
            &BuildCatalog::embedded()?,
            &WeaponCatalog::embedded()?,
        )
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.characters.is_empty()
            || self.characters.len() > MAX_ADVERTISED_CHARACTERS
            || self.weapon_bases.is_empty()
            || self.weapon_bases.len() > MAX_ADVERTISED_WEAPON_BASES
            || self.ultimates.is_empty()
            || self.ultimates.len() > MAX_ADVERTISED_ULTIMATES
            || self.passives.is_empty()
            || self.passives.len() > MAX_ADVERTISED_PASSIVES
        {
            return Err("invalid advertised character catalog envelope".into());
        }
        if !self.fighter_body.radius.is_finite()
            || !(crate::builds::MIN_FIGHTER_BODY_RADIUS..=crate::builds::MAX_FIGHTER_BODY_RADIUS)
                .contains(&self.fighter_body.radius)
        {
            return Err("invalid advertised fighter body".into());
        }
        validate_ordered_metadata(
            self.characters
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
        for character in &self.characters {
            crate::builds::validate_fighter_stats(&character.stats)
                .map_err(|_| "invalid advertised character stats")?;
            if self.weapon(character.weapon_base).is_none()
                || character.ultimate_options[0] == character.ultimate_options[1]
                || character.passive_options[0] == character.passive_options[1]
                || character
                    .ultimate_options
                    .iter()
                    .any(|id| self.ultimate(*id).is_none())
                || character
                    .passive_options
                    .iter()
                    .any(|id| self.passive(*id).is_none())
            {
                return Err("invalid advertised character kit".into());
            }
        }
        for definition in &self.weapon_bases {
            definition.configuration.validate(
                &self.weapon_policy,
                EngineWeaponLimits::default(),
                Some(self.fighter_body.radius),
            )?;
        }
        validate_ultimate_parameters(&self.ultimates)?;
        if self.revision != self.expected_revision()? {
            return Err("advertised character catalog revision mismatch".into());
        }
        let bytes = postcard::to_allocvec(self).map_err(|error| error.to_string())?;
        if bytes.len() > MAX_ADVERTISED_CHARACTER_CATALOG_BYTES {
            return Err("advertised character catalog exceeds its wire bound".into());
        }
        Ok(())
    }

    #[must_use]
    pub fn character(&self, id: CharacterId) -> Option<&AdvertisedCharacter> {
        self.characters
            .iter()
            .find(|definition| definition.id == id)
    }

    #[must_use]
    pub fn weapon(&self, id: WeaponPresetId) -> Option<&AdvertisedWeaponBase> {
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

    /// Whether `choice` names an advertised character and one of each of its options.
    #[must_use]
    pub fn accepts(&self, choice: CharacterChoice) -> bool {
        self.character(choice.character_id)
            .is_some_and(|character| {
                character.ultimate_options.contains(&choice.ultimate_id)
                    && character.passive_options.contains(&choice.passive_id)
            })
    }

    /// A character's first ultimate and first passive.
    #[must_use]
    pub const fn default_choice_for(character: &AdvertisedCharacter) -> CharacterChoice {
        CharacterChoice {
            character_id: character.id,
            ultimate_id: character.ultimate_options[0],
            passive_id: character.passive_options[0],
        }
    }

    /// The pick a press on `character_id` selects: the profile's remembered options for it, or
    /// its defaults.
    #[must_use]
    pub fn choice_for(
        &self,
        profile: &ProfileSnapshot,
        character_id: CharacterId,
    ) -> Option<CharacterChoice> {
        let character = self.character(character_id)?;
        Some(
            profile
                .choice(character_id)
                .filter(|choice| self.accepts(*choice))
                .unwrap_or_else(|| Self::default_choice_for(character)),
        )
    }

    /// Builds the wire profile from stored rows: drops picks this catalog no longer offers and
    /// falls back to the first character with its defaults when the selection is gone.
    pub fn profile_from_stored(
        &self,
        account_id: AccountId,
        revision: ProfileRevision,
        selected: Option<CharacterId>,
        stored: impl IntoIterator<Item = CharacterChoice>,
    ) -> ProfileSnapshot {
        let mut choices: Vec<_> = stored
            .into_iter()
            .filter(|choice| self.accepts(*choice))
            .collect();
        choices.sort_by_key(|choice| choice.character_id);
        choices.dedup_by_key(|choice| choice.character_id);
        choices.truncate(MAX_CHARACTERS);
        let selected_character_id = selected
            .filter(|id| self.character(*id).is_some())
            .unwrap_or(self.characters[0].id);
        if !choices
            .iter()
            .any(|choice| choice.character_id == selected_character_id)
            && let Some(character) = self.character(selected_character_id)
        {
            choices.push(Self::default_choice_for(character));
            choices.sort_by_key(|choice| choice.character_id);
        }
        ProfileSnapshot {
            account_id,
            revision,
            selected_character_id,
            choices,
        }
    }

    pub fn validate_profile(&self, profile: &ProfileSnapshot) -> Result<(), ProfileModelError> {
        profile.validate()?;
        if profile.choices.iter().all(|choice| self.accepts(*choice)) {
            Ok(())
        } else {
            Err(ProfileModelError::InvalidChoice)
        }
    }

    fn expected_revision(&self) -> Result<CharacterCatalogRevision, String> {
        let bytes = postcard::to_allocvec(&(
            ADVERTISED_CHARACTER_CATALOG_FORMAT_VERSION,
            self.fighter_body,
            &self.weapon_policy,
            &self.characters,
            &self.weapon_bases,
            &self.ultimates,
            &self.passives,
        ))
        .map_err(|error| error.to_string())?;
        Ok(CharacterCatalogRevision(fnv1a64(&bytes)))
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the profile catalog deliberately keeps its partial ultimate compatibility matrix explicit"
)]
fn validate_ultimate_parameters(definitions: &[AdvertisedUltimate]) -> Result<(), String> {
    let bounds = ULTIMATE_PARAMETER_BOUNDS;
    let valid =
        definitions.iter().all(
            |definition| match (definition.kind, definition.parameters) {
                (UltimateKind::Dash, UltimateParameters::Dash { .. })
                | (UltimateKind::Sentry, UltimateParameters::Sentry { .. }) => true,
                (UltimateKind::SelfCloak, UltimateParameters::SelfCloak { duration_ticks }) => {
                    bounds.duration_ticks.contains(&duration_ticks)
                }
                (
                    UltimateKind::RevealScan,
                    UltimateParameters::RevealScan {
                        maximum_range_milliunits,
                        radius_milliunits,
                        reveal_ticks,
                    },
                ) => {
                    bounds
                        .world_distance_milliunits
                        .contains(&maximum_range_milliunits)
                        && bounds.field_radius_milliunits.contains(&radius_milliunits)
                        && bounds.duration_ticks.contains(&reveal_ticks)
                }
                (
                    UltimateKind::ConcealmentField,
                    UltimateParameters::ConcealmentField {
                        maximum_range_milliunits,
                        radius_milliunits,
                        duration_ticks,
                    },
                ) => {
                    bounds
                        .world_distance_milliunits
                        .contains(&maximum_range_milliunits)
                        && bounds.field_radius_milliunits.contains(&radius_milliunits)
                        && bounds.duration_ticks.contains(&duration_ticks)
                }
                (
                    UltimateKind::DemolitionStrike,
                    UltimateParameters::DemolitionStrike {
                        maximum_range_milliunits,
                        radius_milliunits,
                    },
                ) => {
                    bounds
                        .world_distance_milliunits
                        .contains(&maximum_range_milliunits)
                        && bounds
                            .demolition_radius_milliunits
                            .contains(&radius_milliunits)
                        && radius_milliunits.is_multiple_of(4_000)
                }
                (
                    UltimateKind::BigBlob,
                    UltimateParameters::BigBlob {
                        maximum_range_milliunits,
                        flight_ticks,
                        visual_arc_height_milliunits,
                        landing_clearance_milliunits,
                        child_speed_milliunits,
                        child_radius_milliunits,
                        child_range_milliunits,
                        child_lifetime_ticks,
                        child_fuse_ticks,
                        child_explosion_radius_milliunits,
                        child_damage,
                        max_active_per_owner,
                    },
                ) => {
                    bounds
                        .world_distance_milliunits
                        .contains(&maximum_range_milliunits)
                        && bounds.short_ticks.contains(&flight_ticks)
                        && bounds
                            .field_radius_milliunits
                            .contains(&visual_arc_height_milliunits)
                        && bounds
                            .compact_radius_milliunits
                            .contains(&landing_clearance_milliunits)
                        && bounds
                            .world_distance_milliunits
                            .contains(&child_speed_milliunits)
                        && bounds
                            .compact_radius_milliunits
                            .contains(&child_radius_milliunits)
                        && bounds
                            .world_distance_milliunits
                            .contains(&child_range_milliunits)
                        && bounds.short_ticks.contains(&child_lifetime_ticks)
                        && bounds.duration_ticks.contains(&child_fuse_ticks)
                        && bounds
                            .compact_radius_milliunits
                            .contains(&child_explosion_radius_milliunits)
                        && bounds.damage.contains(&child_damage)
                        && bounds.active_count.contains(&max_active_per_owner)
                }
                (
                    UltimateKind::CryogenicField
                    | UltimateKind::FireField
                    | UltimateKind::PoisonField
                    | UltimateKind::RestorationField,
                    UltimateParameters::ElementalField {
                        maximum_range_milliunits,
                        radius_milliunits,
                        duration_ticks,
                        pulse_interval_ticks,
                        effect: _,
                    },
                ) => {
                    bounds
                        .world_distance_milliunits
                        .contains(&maximum_range_milliunits)
                        && bounds.field_radius_milliunits.contains(&radius_milliunits)
                        && bounds.duration_ticks.contains(&duration_ticks)
                        && bounds.duration_ticks.contains(&pulse_interval_ticks)
                }
                _ => false,
            },
        );
    if !valid {
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
            return Err("invalid advertised character metadata".into());
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
                Err(E::custom("invalid advertised character catalog key"))
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

fn deserialize_characters<'de, D>(deserializer: D) -> Result<Vec<AdvertisedCharacter>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_bounded_vec::<D, _, MAX_ADVERTISED_CHARACTERS>(deserializer)
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
    use crate::builds::ElementalFieldEffect;

    fn embedded() -> AdvertisedCharacterCatalog {
        AdvertisedCharacterCatalog::embedded().unwrap()
    }

    fn advertised(kind: UltimateKind) -> AdvertisedUltimate {
        embedded()
            .ultimates
            .into_iter()
            .find(|definition| definition.kind == kind)
            .expect("embedded advertised ultimate exists")
    }

    #[test]
    fn partial_ultimate_validator_deliberately_ignores_dash_sentry_and_nested_effect_bounds() {
        let mut dash = advertised(UltimateKind::Dash);
        dash.parameters = UltimateParameters::Dash {
            maximum_distance_milliunits: 0,
            duration_ticks: 0,
            damage: 0,
            knockback_speed_milliunits: 0,
            knockback_duration_ticks: 0,
            maximum_targets: 0,
        };
        assert!(validate_ultimate_parameters(&[dash]).is_ok());

        let mut sentry = advertised(UltimateKind::Sentry);
        sentry.parameters = UltimateParameters::Sentry {
            placement_offsets_milliunits: [0; 6],
            body_radius_milliunits: 0,
            acquisition_range_milliunits: 0,
            acquisition_interval_ticks: 0,
            fire_interval_ticks: 0,
            lifetime_ticks: 0,
            maximum_health: 0,
            projectile_speed_milliunits: 0,
            projectile_radius_milliunits: 0,
            projectile_range_milliunits: 0,
            projectile_lifetime_ticks: 0,
            projectile_damage: 0,
        };
        assert!(validate_ultimate_parameters(&[sentry]).is_ok());

        let mut field = advertised(UltimateKind::FireField);
        let UltimateParameters::ElementalField { effect, .. } = &mut field.parameters else {
            panic!("Fire Field remains an elemental field")
        };
        *effect = ElementalFieldEffect::Cold { amount: 0 };
        assert!(validate_ultimate_parameters(&[field]).is_ok());
    }

    #[test]
    fn partial_ultimate_validator_rejects_owned_common_bounds_and_kind_mismatch() {
        let mut invalid = Vec::new();

        let mut mismatch = advertised(UltimateKind::Dash);
        mismatch.kind = UltimateKind::Sentry;
        invalid.push(mismatch);

        let mut self_cloak = advertised(UltimateKind::SelfCloak);
        self_cloak.parameters = UltimateParameters::SelfCloak { duration_ticks: 0 };
        invalid.push(self_cloak);

        let mut reveal = advertised(UltimateKind::RevealScan);
        let UltimateParameters::RevealScan {
            maximum_range_milliunits,
            ..
        } = &mut reveal.parameters
        else {
            panic!("Reveal Scan remains compatible")
        };
        *maximum_range_milliunits = 0;
        invalid.push(reveal);

        let mut concealment = advertised(UltimateKind::ConcealmentField);
        let UltimateParameters::ConcealmentField {
            radius_milliunits, ..
        } = &mut concealment.parameters
        else {
            panic!("Concealment Field remains compatible")
        };
        *radius_milliunits = 0;
        invalid.push(concealment);

        let mut demolition = advertised(UltimateKind::DemolitionStrike);
        let UltimateParameters::DemolitionStrike {
            radius_milliunits, ..
        } = &mut demolition.parameters
        else {
            panic!("Demolition Strike remains compatible")
        };
        *radius_milliunits = 10_000;
        invalid.push(demolition);

        let mut elemental = advertised(UltimateKind::CryogenicField);
        let UltimateParameters::ElementalField {
            pulse_interval_ticks,
            ..
        } = &mut elemental.parameters
        else {
            panic!("Cryogenic Field remains compatible")
        };
        *pulse_interval_ticks = 0;
        invalid.push(elemental);

        let mut big_blob = advertised(UltimateKind::BigBlob);
        let UltimateParameters::BigBlob {
            max_active_per_owner,
            ..
        } = &mut big_blob.parameters
        else {
            panic!("Big Blob remains compatible")
        };
        *max_active_per_owner = 0;
        invalid.push(big_blob);

        for definition in invalid {
            assert!(validate_ultimate_parameters(&[definition]).is_err());
        }
    }

    #[test]
    fn embedded_catalog_is_bounded_and_includes_current_targeted_ultimates() {
        let catalog = embedded();
        assert_eq!(
            catalog
                .ultimate(UltimateDefinitionId(5))
                .map(|definition| definition.display_name.as_str()),
            Some("Concealment Field")
        );
        assert_eq!(
            catalog
                .ultimate(UltimateDefinitionId(6))
                .map(|definition| definition.display_name.as_str()),
            Some("Demolition Strike")
        );
        assert!(
            postcard::to_allocvec(&catalog).unwrap().len()
                <= MAX_ADVERTISED_CHARACTER_CATALOG_BYTES
        );
        catalog.validate().unwrap();
    }

    #[test]
    fn choices_are_accepted_only_from_the_characters_own_options() {
        let catalog = embedded();
        let riff = catalog.character(CharacterId(1)).unwrap().clone();
        let default = AdvertisedCharacterCatalog::default_choice_for(&riff);
        assert!(catalog.accepts(default));
        let mut foreign_ultimate = default;
        foreign_ultimate.ultimate_id = UltimateDefinitionId(2);
        assert!(!catalog.accepts(foreign_ultimate));
        let mut foreign_passive = default;
        foreign_passive.passive_id = PassiveDefinitionId(1);
        assert!(!catalog.accepts(foreign_passive));
        let mut unknown = default;
        unknown.character_id = CharacterId(999);
        assert!(!catalog.accepts(unknown));
    }

    #[test]
    fn stored_profiles_are_normalized_against_the_catalog() {
        let catalog = embedded();
        let account = AccountId::new(7).unwrap();
        let fresh = catalog.profile_from_stored(account, ProfileRevision::INITIAL, None, []);
        assert_eq!(fresh.selected_character_id, CharacterId(1));
        assert_eq!(
            fresh.selection(),
            AdvertisedCharacterCatalog::default_choice_for(&catalog.characters[0])
        );
        let sumi = CharacterChoice {
            character_id: CharacterId(7),
            ultimate_id: UltimateDefinitionId(7),
            passive_id: PassiveDefinitionId(7),
        };
        let stale = CharacterChoice {
            character_id: CharacterId(2),
            ultimate_id: UltimateDefinitionId(1),
            passive_id: PassiveDefinitionId(9),
        };
        let stored = catalog.profile_from_stored(
            account,
            ProfileRevision::INITIAL,
            Some(CharacterId(7)),
            [stale, sumi],
        );
        assert_eq!(stored.selection(), sumi);
        assert_eq!(stored.choices, [sumi]);
        catalog.validate_profile(&stored).unwrap();
        let gone = catalog.profile_from_stored(
            account,
            ProfileRevision::INITIAL,
            Some(CharacterId(200)),
            [sumi],
        );
        assert_eq!(gone.selected_character_id, CharacterId(1));
        assert_eq!(gone.choices.len(), 2);
        assert_eq!(
            catalog.choice_for(&gone, CharacterId(7)),
            Some(sumi),
            "a remembered pick survives a lost selection"
        );
    }

    #[test]
    fn every_advertised_catalog_section_changes_the_revision() {
        let catalog = embedded();
        let mut variants = Vec::new();
        let mut changed = catalog.clone();
        changed.weapon_policy.max_capacity -= 1;
        variants.push(changed);
        let mut changed = catalog.clone();
        changed.characters[0].display_name.push('!');
        variants.push(changed);
        let mut changed = catalog.clone();
        changed.characters[0].stats.maximum_health += 1;
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
