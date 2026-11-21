//! A player's (or bot's) character pick and its resolution into the match loadout.

use super::{CharacterCatalog, CharacterDefinition, CharacterId};
use crate::builds::{
    BuildCatalog, BuildResolutionError, LoadoutRecipe, PassiveDefinitionId, ResolvedMatchLoadout,
    UltimateDefinitionId,
};
use crate::combat::WeaponCatalog;
use crate::weapon_parts::{CanonicalWeaponModifiers, WeaponPartCatalog};
use serde::{Deserialize, Serialize};

/// One character and the ultimate and passive chosen from its options. Stored per character in a
/// profile, sent by the client to select it, and carried through routing into the match.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct CharacterChoice {
    pub character_id: CharacterId,
    pub ultimate_id: UltimateDefinitionId,
    pub passive_id: PassiveDefinitionId,
}

impl CharacterChoice {
    /// The character's first ultimate and first passive.
    #[must_use]
    pub const fn default_for(character: &CharacterDefinition) -> Self {
        Self {
            character_id: character.id,
            ultimate_id: character.ultimate_options[0],
            passive_id: character.passive_options[0],
        }
    }

    /// This choice carried onto a revised definition of the same character: an option the revised
    /// character no longer offers becomes the revised option in the slot it held in `original`.
    #[must_use]
    pub fn carried_by_slot(
        self,
        original: &CharacterDefinition,
        revised: &CharacterDefinition,
    ) -> Self {
        let ultimate_id = if revised.ultimate_options.contains(&self.ultimate_id) {
            self.ultimate_id
        } else {
            original
                .ultimate_options
                .iter()
                .position(|id| *id == self.ultimate_id)
                .map_or(self.ultimate_id, |slot| revised.ultimate_options[slot])
        };
        let passive_id = if revised.passive_options.contains(&self.passive_id) {
            self.passive_id
        } else {
            original
                .passive_options
                .iter()
                .position(|id| *id == self.passive_id)
                .map_or(self.passive_id, |slot| revised.passive_options[slot])
        };
        Self {
            character_id: self.character_id,
            ultimate_id,
            passive_id,
        }
    }

    /// Whether this choice names `character` and one of each of its options.
    #[must_use]
    pub fn is_offered_by(self, character: &CharacterDefinition) -> bool {
        self.character_id == character.id
            && character.ultimate_options.contains(&self.ultimate_id)
            && character.passive_options.contains(&self.passive_id)
    }
}

impl CharacterCatalog {
    /// The default pick: the first character with its first options.
    #[must_use]
    pub fn default_choice(&self) -> CharacterChoice {
        CharacterChoice::default_for(&self.characters[0])
    }

    #[must_use]
    pub fn accepts(&self, choice: CharacterChoice) -> bool {
        self.character(choice.character_id)
            .is_some_and(|character| choice.is_offered_by(character))
    }

    /// The deterministic Practice bot pick for one bot ordinal in a roster started with
    /// `roster_seed` (BRL-abys). Bots of one roster take consecutive characters, so they never
    /// repeat while the roster is no larger than the catalog. The seed walks the character first,
    /// then the ultimate option, then the passive option, so any `2 × len` consecutive seeds give a
    /// single bot every character with each of its two ultimates.
    #[must_use]
    pub fn practice_bot_choice(&self, roster_seed: u64, bot_ordinal: usize) -> CharacterChoice {
        let count = self.characters.len();
        let seed = usize::try_from(roster_seed % (4 * count as u64)).unwrap_or(0);
        let character = &self.characters[(seed + bot_ordinal) % count];
        CharacterChoice {
            character_id: character.id,
            ultimate_id: character.ultimate_options[(seed / count + bot_ordinal) % 2],
            passive_id: character.passive_options[(seed / (2 * count) + bot_ordinal) % 2],
        }
    }

    /// The validated recipe for one pick: the character's stats, its signature weapon with the
    /// authored parts aggregated, and the chosen options.
    pub fn loadout_recipe(
        &self,
        choice: CharacterChoice,
        parts: &WeaponPartCatalog,
    ) -> Result<LoadoutRecipe, BuildResolutionError> {
        let character = self
            .character(choice.character_id)
            .ok_or(BuildResolutionError::UnknownId)?;
        if !choice.is_offered_by(character) {
            return Err(BuildResolutionError::InvalidCombination);
        }
        let effects = character
            .weapon
            .parts
            .iter()
            .map(|id| parts.definition(*id).ok_or(BuildResolutionError::UnknownId))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flat_map(|definition| definition.effects.iter().copied());
        let weapon_modifiers = if character.weapon.parts.is_empty() {
            CanonicalWeaponModifiers::default()
        } else {
            crate::weapon_parts::aggregate_weapon_part_effects(effects)
                .map_err(|_| BuildResolutionError::InvalidCombination)?
        };
        Ok(LoadoutRecipe {
            character_id: character.id,
            fighter_stats: character.stats,
            weapon_base: character.weapon.base,
            weapon_modifiers,
            ultimate: choice.ultimate_id,
            passive: choice.passive_id,
        })
    }

    pub fn resolve(
        &self,
        choice: CharacterChoice,
        builds: &BuildCatalog,
        weapons: &WeaponCatalog,
        parts: &WeaponPartCatalog,
    ) -> Result<ResolvedMatchLoadout, BuildResolutionError> {
        crate::builds::resolve_loadout(builds, weapons, self.loadout_recipe(choice, parts)?)
    }

    /// A pick admitted against the canonical roster, carried onto this Balance Lab-revised roster
    /// by option slot (BRL-5v3c).
    #[cfg(feature = "balance-lab")]
    #[must_use]
    pub fn carry_balance_lab_choice(&self, choice: CharacterChoice) -> CharacterChoice {
        static CANONICAL: std::sync::OnceLock<Option<CharacterCatalog>> =
            std::sync::OnceLock::new();
        let canonical = CANONICAL.get_or_init(|| CharacterCatalog::embedded().ok());
        match (
            canonical
                .as_ref()
                .and_then(|catalog| catalog.character(choice.character_id)),
            self.character(choice.character_id),
        ) {
            (Some(original), Some(revised)) => choice.carried_by_slot(original, revised),
            _ => choice,
        }
    }

    /// Resolve the authored direct-diagnostic pick for one player through the ordinary path.
    pub fn resolve_direct_diagnostic(
        &self,
        builds: &BuildCatalog,
        weapons: &WeaponCatalog,
        parts: &WeaponPartCatalog,
        player_id: u64,
    ) -> Result<ResolvedMatchLoadout, BuildResolutionError> {
        let picks = builds.direct_diagnostic_picks();
        let index = player_id
            .checked_sub(1)
            .and_then(|index| usize::try_from(index).ok())
            .ok_or(BuildResolutionError::InvalidCombination)?
            % picks.len().max(1);
        let pick = picks
            .get(index)
            .copied()
            .ok_or(BuildResolutionError::ResolutionFailed)?;
        self.resolve(pick, builds, weapons, parts)
    }
}
/// The catalogs one character pick resolves against.
#[derive(Clone, Copy)]
pub struct LoadoutCatalogs<'a> {
    pub characters: &'a CharacterCatalog,
    pub builds: &'a BuildCatalog,
    pub weapons: &'a WeaponCatalog,
    pub parts: &'a WeaponPartCatalog,
}

impl LoadoutCatalogs<'_> {
    pub fn resolve(
        self,
        choice: CharacterChoice,
    ) -> Result<ResolvedMatchLoadout, BuildResolutionError> {
        self.characters
            .resolve(choice, self.builds, self.weapons, self.parts)
    }
}

/// The embedded catalogs, owned, for code paths without a Bevy world (lobby, routing tests).
pub struct EmbeddedLoadoutCatalogs {
    pub characters: CharacterCatalog,
    pub builds: BuildCatalog,
    pub weapons: WeaponCatalog,
    pub parts: WeaponPartCatalog,
}

impl EmbeddedLoadoutCatalogs {
    pub fn load() -> Result<Self, String> {
        Ok(Self {
            characters: CharacterCatalog::embedded()?,
            builds: BuildCatalog::embedded()?,
            weapons: WeaponCatalog::embedded()?,
            parts: WeaponPartCatalog::embedded()?,
        })
    }

    #[must_use]
    pub const fn view(&self) -> LoadoutCatalogs<'_> {
        LoadoutCatalogs {
            characters: &self.characters,
            builds: &self.builds,
            weapons: &self.weapons,
            parts: &self.parts,
        }
    }
}

/// The installed catalogs one pick resolves against, as one Bevy system parameter.
#[derive(bevy::ecs::system::SystemParam)]
pub struct LoadoutCatalogResources<'w> {
    characters: bevy::prelude::Res<'w, super::CharacterCatalogResource>,
    builds: bevy::prelude::Res<'w, crate::builds::BuildCatalogResource>,
    weapons: bevy::prelude::Res<'w, crate::combat::WeaponCatalogResource>,
    parts: bevy::prelude::Res<'w, crate::weapon_parts::WeaponPartCatalogResource>,
}

impl LoadoutCatalogResources<'_> {
    #[must_use]
    pub fn view(&self) -> LoadoutCatalogs<'_> {
        LoadoutCatalogs {
            characters: &self.characters.0,
            builds: &self.builds.0,
            weapons: &self.weapons.0,
            parts: &self.parts.0,
        }
    }
}
