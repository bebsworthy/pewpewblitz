use crate::{
    builds::{PassiveDefinitionId, UltimateDefinitionId},
    lobby::normalize_proposed_display_name,
};
use serde::{Deserialize, Serialize};
use std::{fmt, num::NonZeroU64, str::FromStr};

pub const MAX_BRAWLERS_PER_PROFILE: usize = 16;
pub const MAX_PROFILE_SNAPSHOT_BYTES: usize = 32 * 1024;

macro_rules! opaque_id {
    ($name:ident) => {
        #[derive(Serialize, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
        pub struct $name(u128);

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = u128::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }

        impl $name {
            pub fn new(value: u128) -> Result<Self, ProfileModelError> {
                (value != 0)
                    .then_some(Self(value))
                    .ok_or(ProfileModelError::ZeroId)
            }

            pub fn random() -> Result<Self, ProfileModelError> {
                let mut bytes = [0_u8; 16];
                getrandom::fill(&mut bytes).map_err(|_| ProfileModelError::EntropyUnavailable)?;
                Self::new(u128::from_be_bytes(bytes))
            }

            #[must_use]
            pub const fn get(self) -> u128 {
                self.0
            }

            #[must_use]
            pub const fn to_bytes(self) -> [u8; 16] {
                self.0.to_be_bytes()
            }

            pub fn from_bytes(bytes: [u8; 16]) -> Result<Self, ProfileModelError> {
                Self::new(u128::from_be_bytes(bytes))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{:032x}", self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(stringify!($name))?;
                formatter.write_str("(")?;
                fmt::Display::fmt(self, formatter)?;
                formatter.write_str(")")
            }
        }

        impl FromStr for $name {
            type Err = ProfileModelError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                if value.len() != 32
                    || !value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(ProfileModelError::MalformedId);
                }
                let raw =
                    u128::from_str_radix(value, 16).map_err(|_| ProfileModelError::MalformedId)?;
                Self::new(raw)
            }
        }
    };
}

opaque_id!(AccountId);
opaque_id!(SavedBrawlerId);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct FighterProfileId(pub u16);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct WeaponBaseId(pub u16);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ProfileRevision(NonZeroU64);

impl ProfileRevision {
    pub const INITIAL: Self = Self(NonZeroU64::MIN);

    pub fn new(value: u64) -> Result<Self, ProfileModelError> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(ProfileModelError::InvalidRevision)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    pub fn next(self) -> Result<Self, ProfileModelError> {
        self.get()
            .checked_add(1)
            .ok_or(ProfileModelError::InvalidRevision)
            .and_then(Self::new)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileModelError {
    ZeroId,
    MalformedId,
    EntropyUnavailable,
    InvalidRevision,
    InvalidName,
    UnknownFighterProfile,
    UnknownWeaponBase,
    UnknownUltimate,
    UnknownPassive,
    DuplicatePassive,
    TooManyBrawlers,
    MissingSelection,
    InvalidSelection,
    DuplicateBrawler,
    InvalidCreationOrdinal,
    TooManyParts,
    DuplicatePart,
    InvalidPart,
    InvalidEquipment,
}

impl fmt::Display for ProfileModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BrawlerDraft {
    pub name: String,
    pub fighter_profile_id: FighterProfileId,
    pub weapon_base_id: WeaponBaseId,
    pub ultimate_id: UltimateDefinitionId,
    pub passive_ids: [PassiveDefinitionId; 2],
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BrawlerEdit {
    pub name: String,
    pub ultimate_id: UltimateDefinitionId,
    pub passive_ids: [PassiveDefinitionId; 2],
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SavedBrawler {
    pub id: SavedBrawlerId,
    pub creation_ordinal: u64,
    pub name: String,
    pub fighter_profile_id: FighterProfileId,
    pub weapon_base_id: WeaponBaseId,
    pub ultimate_id: UltimateDefinitionId,
    pub passive_ids: [PassiveDefinitionId; 2],
    pub equipped_part_ids: [Option<crate::weapon_parts::WeaponPartInstanceId>;
        crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
    pub revision: ProfileRevision,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ProfileSnapshot {
    pub account_id: AccountId,
    pub revision: ProfileRevision,
    pub next_brawler_ordinal: u64,
    pub selected_brawler_id: Option<SavedBrawlerId>,
    pub brawlers: Vec<SavedBrawler>,
    pub inventory: Vec<crate::weapon_parts::WeaponPartInstance>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ProfileCommand {
    CreateBrawler {
        request_id: u64,
        expected_profile_revision: ProfileRevision,
        draft: BrawlerDraft,
    },
    EditBrawler {
        request_id: u64,
        expected_profile_revision: ProfileRevision,
        brawler_id: SavedBrawlerId,
        expected_brawler_revision: ProfileRevision,
        edit: BrawlerEdit,
    },
    SelectBrawler {
        request_id: u64,
        expected_profile_revision: ProfileRevision,
        brawler_id: SavedBrawlerId,
    },
    DeleteBrawler {
        request_id: u64,
        expected_profile_revision: ProfileRevision,
        brawler_id: SavedBrawlerId,
        expected_brawler_revision: ProfileRevision,
    },
    EquipWeaponParts {
        request_id: u64,
        expected_profile_revision: ProfileRevision,
        brawler_id: SavedBrawlerId,
        expected_brawler_revision: ProfileRevision,
        equipped_part_ids: [Option<crate::weapon_parts::WeaponPartInstanceId>;
            crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ProfileDecision {
    Accepted,
    InvalidRequest,
    StaleRevision,
    MissingBrawler,
    CapacityReached,
    QueueLocked,
    TemporarilyUnavailable,
    StorageFault,
    MissingPart,
    PartAlreadyEquipped,
    IncompatibleWeapon,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ProfileOutcome {
    pub request_id: u64,
    pub decision: ProfileDecision,
    pub snapshot: Option<ProfileSnapshot>,
}

impl ProfileSnapshot {
    #[must_use]
    pub const fn empty(account_id: AccountId) -> Self {
        Self {
            account_id,
            revision: ProfileRevision::INITIAL,
            next_brawler_ordinal: 1,
            selected_brawler_id: None,
            brawlers: Vec::new(),
            inventory: Vec::new(),
        }
    }

    pub fn validate_bounded(&self) -> Result<(), ProfileModelError> {
        self.validate()?;
        if postcard::to_allocvec(self)
            .map_or(true, |bytes| bytes.len() > MAX_PROFILE_SNAPSHOT_BYTES)
        {
            return Err(ProfileModelError::TooManyBrawlers);
        }
        Ok(())
    }
}

/// Immutable V7 loadout handed through routing without account or storage authority.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchBuildSnapshotV3 {
    pub schema_version: u8,
    pub brawler_id: SavedBrawlerId,
    pub brawler_revision: ProfileRevision,
    pub fighter_profile_id: FighterProfileId,
    pub weapon_base_id: WeaponBaseId,
    pub ultimate_id: UltimateDefinitionId,
    pub passive_ids: [PassiveDefinitionId; 2],
    pub weapon_modifiers: crate::weapon_parts::CanonicalWeaponModifiers,
    pub accepted_identity: crate::builds::SelectedBuild,
}

impl MatchBuildSnapshotV3 {
    pub const SCHEMA_VERSION: u8 = 7;

    pub fn from_brawler(
        brawler: &SavedBrawler,
        builds: &crate::builds::BuildCatalog,
        weapons: &crate::combat::WeaponCatalog,
        fighter: &crate::combat::FighterDefinition,
    ) -> Result<Self, crate::builds::BuildResolutionError> {
        Self::from_brawler_and_modifiers(
            brawler,
            crate::weapon_parts::CanonicalWeaponModifiers::default(),
            builds,
            weapons,
            fighter,
        )
    }

    pub fn from_profile_brawler(
        profile: &ProfileSnapshot,
        brawler: &SavedBrawler,
        builds: &crate::builds::BuildCatalog,
        weapons: &crate::combat::WeaponCatalog,
        fighter: &crate::combat::FighterDefinition,
    ) -> Result<Self, crate::builds::BuildResolutionError> {
        let modifiers = profile.weapon_modifiers(brawler)?;
        Self::from_brawler_and_modifiers(brawler, modifiers, builds, weapons, fighter)
    }

    pub(crate) fn from_brawler_and_modifiers(
        brawler: &SavedBrawler,
        weapon_modifiers: crate::weapon_parts::CanonicalWeaponModifiers,
        builds: &crate::builds::BuildCatalog,
        weapons: &crate::combat::WeaponCatalog,
        fighter: &crate::combat::FighterDefinition,
    ) -> Result<Self, crate::builds::BuildResolutionError> {
        let resolved =
            brawler.resolve_loadout_with_modifiers(builds, weapons, fighter, weapon_modifiers)?;
        Ok(Self {
            schema_version: Self::SCHEMA_VERSION,
            brawler_id: brawler.id,
            brawler_revision: brawler.revision,
            fighter_profile_id: brawler.fighter_profile_id,
            weapon_base_id: brawler.weapon_base_id,
            ultimate_id: brawler.ultimate_id,
            passive_ids: brawler.passive_ids,
            weapon_modifiers,
            accepted_identity: resolved.identity,
        })
    }

    pub fn encode(self) -> Result<brawler_routing::MatchBuildSnapshot, String> {
        let bytes = postcard::to_allocvec(&self)
            .map_err(|error| format!("match build snapshot encode failed: {error}"))?;
        brawler_routing::MatchBuildSnapshot::new(&bytes)
            .map_err(|error| format!("match build snapshot exceeds bound: {error:?}"))
    }

    pub fn decode(snapshot: &brawler_routing::MatchBuildSnapshot) -> Result<Self, String> {
        let value: Self = postcard::from_bytes(snapshot.as_bytes())
            .map_err(|error| format!("match build snapshot decode failed: {error}"))?;
        if value.schema_version != Self::SCHEMA_VERSION {
            return Err("unsupported match build snapshot version".to_string());
        }
        Ok(value)
    }

    pub fn resolve(
        self,
        builds: &crate::builds::BuildCatalog,
        weapons: &crate::combat::WeaponCatalog,
        fighter: &crate::combat::FighterDefinition,
    ) -> Result<crate::builds::ResolvedMatchLoadout, crate::builds::BuildResolutionError> {
        let mut resolved = crate::builds::resolve_saved_brawler_recipe(
            builds,
            weapons,
            fighter,
            self.fighter_profile_id,
            self.weapon_base_id,
            self.ultimate_id,
            self.passive_ids,
        )?;
        let weapon = crate::weapon_parts::resolve_weapon_parts(
            weapons,
            fighter,
            crate::combat::WeaponPresetId(self.weapon_base_id.0),
            self.weapon_modifiers,
        )
        .map_err(|_| crate::builds::BuildResolutionError::InvalidCombination)?;
        if self.weapon_modifiers != crate::weapon_parts::CanonicalWeaponModifiers::default() {
            resolved.identity.recipe_fingerprint =
                crate::builds::BuildRecipeFingerprint(weapon.recipe_fingerprint.0);
        }
        resolved.primary_weapon = weapon;
        if resolved.identity != self.accepted_identity {
            return Err(crate::builds::BuildResolutionError::InvalidCombination);
        }
        Ok(resolved)
    }
}

impl BrawlerDraft {
    pub fn normalized(mut self) -> Result<Self, ProfileModelError> {
        self.name = normalize_proposed_display_name(&self.name)
            .map_err(|_| ProfileModelError::InvalidName)?;
        validate_recipe_structure(
            self.fighter_profile_id,
            self.weapon_base_id,
            self.ultimate_id,
            self.passive_ids,
        )?;
        Ok(self)
    }
}

impl BrawlerEdit {
    pub fn normalized(mut self) -> Result<Self, ProfileModelError> {
        self.name = normalize_proposed_display_name(&self.name)
            .map_err(|_| ProfileModelError::InvalidName)?;
        validate_mutable_recipe_structure(self.ultimate_id, self.passive_ids)?;
        Ok(self)
    }
}

impl SavedBrawler {
    pub fn validate(&self) -> Result<(), ProfileModelError> {
        self.validate_structure()
    }

    pub fn validate_structure(&self) -> Result<(), ProfileModelError> {
        if self.creation_ordinal == 0 {
            return Err(ProfileModelError::InvalidCreationOrdinal);
        }
        if normalize_proposed_display_name(&self.name).as_deref() != Ok(self.name.as_str()) {
            return Err(ProfileModelError::InvalidName);
        }
        validate_recipe_structure(
            self.fighter_profile_id,
            self.weapon_base_id,
            self.ultimate_id,
            self.passive_ids,
        )?;
        let mut equipped = std::collections::BTreeSet::new();
        if self
            .equipped_part_ids
            .iter()
            .flatten()
            .any(|id| !equipped.insert(*id))
        {
            return Err(ProfileModelError::InvalidEquipment);
        }
        Ok(())
    }

    pub fn resolve_loadout(
        &self,
        builds: &crate::builds::BuildCatalog,
        weapons: &crate::combat::WeaponCatalog,
        fighter: &crate::combat::FighterDefinition,
    ) -> Result<crate::builds::ResolvedMatchLoadout, crate::builds::BuildResolutionError> {
        self.validate()
            .map_err(|_| crate::builds::BuildResolutionError::InvalidCombination)?;
        crate::builds::resolve_saved_brawler_recipe(
            builds,
            weapons,
            fighter,
            self.fighter_profile_id,
            self.weapon_base_id,
            self.ultimate_id,
            self.passive_ids,
        )
    }

    pub fn resolve_loadout_with_modifiers(
        &self,
        builds: &crate::builds::BuildCatalog,
        weapons: &crate::combat::WeaponCatalog,
        fighter: &crate::combat::FighterDefinition,
        modifiers: crate::weapon_parts::CanonicalWeaponModifiers,
    ) -> Result<crate::builds::ResolvedMatchLoadout, crate::builds::BuildResolutionError> {
        let mut loadout = self.resolve_loadout(builds, weapons, fighter)?;
        let weapon = crate::weapon_parts::resolve_weapon_parts(
            weapons,
            fighter,
            crate::combat::WeaponPresetId(self.weapon_base_id.0),
            modifiers,
        )
        .map_err(|_| crate::builds::BuildResolutionError::InvalidCombination)?;
        if modifiers != crate::weapon_parts::CanonicalWeaponModifiers::default() {
            loadout.identity.recipe_fingerprint =
                crate::builds::BuildRecipeFingerprint(weapon.recipe_fingerprint.0);
        }
        loadout.primary_weapon = weapon;
        Ok(loadout)
    }
}

impl ProfileSnapshot {
    pub fn validate(&self) -> Result<(), ProfileModelError> {
        self.validate_structure()
    }

    pub fn validate_structure(&self) -> Result<(), ProfileModelError> {
        if self.brawlers.len() > MAX_BRAWLERS_PER_PROFILE {
            return Err(ProfileModelError::TooManyBrawlers);
        }
        if self.next_brawler_ordinal == 0 {
            return Err(ProfileModelError::InvalidCreationOrdinal);
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut ordinals = std::collections::BTreeSet::new();
        for brawler in &self.brawlers {
            brawler.validate()?;
            if !ids.insert(brawler.id) || !ordinals.insert(brawler.creation_ordinal) {
                return Err(ProfileModelError::DuplicateBrawler);
            }
        }
        if self.inventory.len() > crate::weapon_parts::MAX_WEAPON_PARTS_PER_PROFILE {
            return Err(ProfileModelError::TooManyParts);
        }
        let mut part_ids = std::collections::BTreeSet::new();
        let mut part_ordinals = std::collections::BTreeSet::new();
        for part in &self.inventory {
            part.validate()
                .map_err(|_| ProfileModelError::InvalidPart)?;
            if !part_ids.insert(part.id) || !part_ordinals.insert(part.inventory_ordinal) {
                return Err(ProfileModelError::DuplicatePart);
            }
        }
        let mut equipped = std::collections::BTreeSet::new();
        for brawler in &self.brawlers {
            for id in brawler.equipped_part_ids.iter().flatten() {
                if !part_ids.contains(id) || !equipped.insert(*id) {
                    return Err(ProfileModelError::InvalidEquipment);
                }
            }
        }
        match self.selected_brawler_id {
            Some(selected) if !ids.contains(&selected) => Err(ProfileModelError::InvalidSelection),
            None if !self.brawlers.is_empty() => Err(ProfileModelError::MissingSelection),
            _ => Ok(()),
        }
    }

    pub fn weapon_modifiers(
        &self,
        brawler: &SavedBrawler,
    ) -> Result<crate::weapon_parts::CanonicalWeaponModifiers, crate::builds::BuildResolutionError>
    {
        let effects = brawler
            .equipped_part_ids
            .iter()
            .flatten()
            .map(|id| {
                self.inventory
                    .iter()
                    .find(|part| part.id == *id)
                    .ok_or(crate::builds::BuildResolutionError::InvalidCombination)
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flat_map(|part| part.effects.iter().copied());
        crate::weapon_parts::aggregate_weapon_part_effects(effects)
            .map_err(|_| crate::builds::BuildResolutionError::InvalidCombination)
    }
}

fn validate_recipe_structure(
    fighter: FighterProfileId,
    weapon: WeaponBaseId,
    ultimate: UltimateDefinitionId,
    passives: [PassiveDefinitionId; 2],
) -> Result<(), ProfileModelError> {
    if fighter.0 == 0 {
        return Err(ProfileModelError::UnknownFighterProfile);
    }
    if weapon.0 == 0 {
        return Err(ProfileModelError::UnknownWeaponBase);
    }
    validate_mutable_recipe_structure(ultimate, passives)
}

fn validate_mutable_recipe_structure(
    ultimate: UltimateDefinitionId,
    passives: [PassiveDefinitionId; 2],
) -> Result<(), ProfileModelError> {
    if ultimate.0 == 0 {
        return Err(ProfileModelError::UnknownUltimate);
    }
    if passives.iter().any(|id| id.0 == 0) {
        return Err(ProfileModelError::UnknownPassive);
    }
    if passives[0] == passives[1] {
        return Err(ProfileModelError::DuplicatePassive);
    }
    Ok(())
}
