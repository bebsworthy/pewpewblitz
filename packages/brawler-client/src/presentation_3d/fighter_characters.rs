//! Roster character presentation (BRL-egn6).
//!
//! The replicated loadout names a fighter's stable [`CharacterId`]; the authored client catalog
//! `assets/catalogs/fighter_characters.ron` maps it to one character model (a GLB on Riff's clip,
//! socket and source-unit contract), a scale, and the weapon socket. Every launch character draws
//! its own production model; an unknown ID falls back to the fallback model at scale 1.
//! Presentation only: nothing here feeds simulation, the protocol, or the gameplay fingerprint.

use bevy::prelude::*;
use brawler::characters::CharacterId;
use serde::Deserialize;

const FIGHTER_CHARACTER_CATALOG: &str =
    include_str!("../../../../assets/catalogs/fighter_characters.ron");
const FIGHTER_CHARACTER_SCHEMA_VERSION: u16 = 1;
/// Character models are original builds; a model outside this directory is refused.
const CHARACTER_MODEL_ROOT: &str = "brawler/models/original/characters/";
/// Each model is one loaded GLB with its own animation graph, so the set stays small.
pub(crate) const MAX_CHARACTER_MODELS: usize = 12;
const MAX_CHARACTER_ENTRIES: usize = brawler::characters::MAX_CHARACTERS;
/// Scale multiplies Riff's source-to-world scale; the bound keeps a fighter readable and near
/// its one-cell collision footprint.
const MIN_CHARACTER_SCALE: f32 = 0.8;
const MAX_CHARACTER_SCALE: f32 = 1.2;
/// Soles sit at most this far above the source origin (source units).
const MAX_SOURCE_GROUND: f32 = 0.5;
/// The weapon socket every shipped character exports (`asset_src/03-characters_modeling_spec.md` section 10).
pub(crate) const DEFAULT_WEAPON_SOCKET: &str = "socket_weapon_r";

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct FighterCharacterCatalogSource {
    schema_version: u16,
    fallback_model: String,
    models: Vec<CharacterModelSource>,
    characters: Vec<CharacterLookSource>,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct CharacterModelSource {
    key: String,
    model: String,
    source_ground: f32,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
struct CharacterLookSource {
    character: u16,
    model: String,
    scale: f32,
    socket: String,
}

/// One character model GLB and its grounding offset.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CharacterModel {
    pub(crate) key: String,
    pub(crate) model: String,
    pub(crate) source_ground: f32,
}

/// How one roster character is drawn: its model index, scale and weapon socket.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CharacterLook {
    pub(crate) model: usize,
    pub(crate) scale: f32,
    pub(crate) socket: String,
}

/// Validated roster presentation, embedded from `assets/catalogs/fighter_characters.ron`.
#[derive(Resource, Clone, Debug, PartialEq)]
pub(crate) struct FighterCharacterCatalog {
    models: Vec<CharacterModel>,
    fallback: CharacterLook,
    characters: Vec<(CharacterId, CharacterLook)>,
}

impl FighterCharacterCatalog {
    pub(crate) fn embedded() -> Result<Self, String> {
        Self::from_ron(FIGHTER_CHARACTER_CATALOG)
    }

    fn from_ron(source: &str) -> Result<Self, String> {
        let source: FighterCharacterCatalogSource = ron::from_str(source)
            .map_err(|error| format!("fighter character catalog parse failed: {error}"))?;
        if source.schema_version != FIGHTER_CHARACTER_SCHEMA_VERSION {
            return Err(format!(
                "fighter character catalog schema {} is not {FIGHTER_CHARACTER_SCHEMA_VERSION}",
                source.schema_version
            ));
        }
        if source.models.is_empty()
            || source.models.len() > MAX_CHARACTER_MODELS
            || source.characters.len() > MAX_CHARACTER_ENTRIES
        {
            return Err("fighter character catalog sizes are outside their bounds".into());
        }
        let mut models: Vec<CharacterModel> = Vec::with_capacity(source.models.len());
        for model in source.models {
            if model.key.is_empty() || models.iter().any(|known| known.key == model.key) {
                return Err(format!(
                    "character model key {:?} is empty or repeated",
                    model.key
                ));
            }
            models.push(validated_model(model)?);
        }
        let model_index = |key: &str| {
            models
                .iter()
                .position(|model| model.key == key)
                .ok_or_else(|| format!("unknown character model {key:?}"))
        };
        let fallback = CharacterLook {
            model: model_index(&source.fallback_model)?,
            scale: 1.0,
            socket: DEFAULT_WEAPON_SOCKET.to_string(),
        };
        let mut characters: Vec<(CharacterId, CharacterLook)> =
            Vec::with_capacity(source.characters.len());
        for entry in source.characters {
            let id = CharacterId(entry.character);
            if entry.character == 0 || characters.iter().any(|(known, _)| *known == id) {
                return Err(format!("character {} is zero or repeated", entry.character));
            }
            let model = model_index(&entry.model)?;
            characters.push((id, validated_look(entry, model)?));
        }
        Ok(Self {
            models,
            fallback,
            characters,
        })
    }

    pub(crate) fn models(&self) -> &[CharacterModel] {
        &self.models
    }

    /// The look of `character`, or the fallback model for an unknown character.
    pub(crate) fn look(&self, character: Option<CharacterId>) -> &CharacterLook {
        character
            .and_then(|id| {
                self.characters
                    .iter()
                    .find(|(known, _)| *known == id)
                    .map(|(_, look)| look)
            })
            .unwrap_or(&self.fallback)
    }
}

fn validated_model(model: CharacterModelSource) -> Result<CharacterModel, String> {
    if !model.model.starts_with(CHARACTER_MODEL_ROOT)
        || !std::path::Path::new(&model.model)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("glb"))
        || model.model.contains("..")
    {
        return Err(format!(
            "character model {:?} must be a GLB under {CHARACTER_MODEL_ROOT}",
            model.model
        ));
    }
    if !model.source_ground.is_finite() || !(0.0..=MAX_SOURCE_GROUND).contains(&model.source_ground)
    {
        return Err(format!(
            "character model {:?} source_ground is outside 0..={MAX_SOURCE_GROUND}",
            model.key
        ));
    }
    Ok(CharacterModel {
        key: model.key,
        model: model.model,
        source_ground: model.source_ground,
    })
}

fn validated_look(entry: CharacterLookSource, model: usize) -> Result<CharacterLook, String> {
    if !entry.scale.is_finite()
        || !(MIN_CHARACTER_SCALE..=MAX_CHARACTER_SCALE).contains(&entry.scale)
    {
        return Err(format!(
            "character {} scale {} is outside {MIN_CHARACTER_SCALE}..={MAX_CHARACTER_SCALE}",
            entry.character, entry.scale
        ));
    }
    if entry.socket.is_empty() {
        return Err(format!("character {} names no socket", entry.character));
    }
    Ok(CharacterLook {
        model,
        scale: entry.scale,
        socket: entry.socket,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_launch_character_draws_its_own_shipped_model() {
        let catalog = FighterCharacterCatalog::embedded().unwrap();
        let roster = brawler::characters::CharacterCatalog::embedded().unwrap();
        for character in &roster.characters {
            assert!(
                catalog.characters.iter().any(|(id, _)| *id == character.id),
                "{} has its own presentation entry",
                character.key
            );
            let look = catalog.look(Some(character.id));
            let model = &catalog.models()[look.model];
            assert_eq!(
                model.key, character.key,
                "{} draws its own model",
                character.key
            );
            let path = std::path::Path::new(crate::REPOSITORY_ROOT)
                .join("assets")
                .join(&model.model);
            assert!(
                path.is_file(),
                "{} model {} ships",
                character.key,
                model.model
            );
        }
    }

    #[test]
    fn unknown_characters_fall_back_to_riff_at_scale_one() {
        let catalog = FighterCharacterCatalog::embedded().unwrap();
        let unknown = catalog.look(Some(CharacterId(999)));
        assert_eq!(catalog.models()[unknown.model].key, "riff");
        assert!((unknown.scale - 1.0).abs() < f32::EPSILON);
        assert_eq!(catalog.look(None), unknown);
    }

    #[test]
    fn catalog_rejects_unknown_models_bad_scales_and_out_of_tree_paths() {
        let source = |models: &str, characters: &str| {
            format!(
                "(schema_version: 1, fallback_model: \"riff\", models: [{models}], characters: [{characters}])"
            )
        };
        let riff = "(key: \"riff\", model: \"brawler/models/original/characters/riff/chr_riff.glb\", source_ground: 0.04)";
        let look = |model: &str, scale: &str| {
            format!(
                "(character: 1, model: \"{model}\", scale: {scale}, socket: \"socket_weapon_r\")"
            )
        };
        assert!(FighterCharacterCatalog::from_ron(&source(riff, &look("riff", "1.0"))).is_ok());
        assert!(FighterCharacterCatalog::from_ron(&source(riff, &look("bumper", "1.0"))).is_err());
        assert!(FighterCharacterCatalog::from_ron(&source(riff, &look("riff", "1.5"))).is_err());
        let outside =
            "(key: \"riff\", model: \"brawler/models/kenney/x.glb\", source_ground: 0.04)";
        assert!(FighterCharacterCatalog::from_ron(&source(outside, "")).is_err());
    }
}
