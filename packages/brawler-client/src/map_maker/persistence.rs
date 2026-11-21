use super::{MapDraftDocument, MapDraftId, MapDraftLibrary};
use atomic_write_file::AtomicWriteFile;
use bevy::prelude::Resource;
use std::{
    fs,
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
};

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct MapDraftStorage(pub PathBuf);

impl MapDraftStorage {
    #[must_use]
    pub fn for_data_directory(directory: impl Into<PathBuf>) -> Self {
        Self(directory.into().join("maps"))
    }

    fn path_for(&self, id: MapDraftId) -> PathBuf {
        self.0.join(format!("{:016x}.ron", id.0))
    }
}

impl Default for MapDraftStorage {
    fn default() -> Self {
        if let Some(directory) =
            std::env::var_os(crate::connection_persistence::CLIENT_DATA_DIRECTORY_ENV)
                .filter(|value| !value.is_empty())
        {
            return Self::for_data_directory(directory);
        }
        Self(
            crate::platform_dirs::config_directory()
                .map_or_else(|| PathBuf::from("maps"), |directory| directory.join("maps")),
        )
    }
}

pub fn load_map_drafts(storage: &MapDraftStorage) -> MapDraftLibrary {
    let mut library = MapDraftLibrary::default();
    let Ok(entries) = fs::read_dir(&storage.0) else {
        return library;
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "ron"))
        .collect();
    paths.sort();
    for path in paths {
        match load_one(&path).and_then(|(document, migrated)| {
            if migrated {
                save_map_draft(storage, &document)?;
            }
            library.insert(document.clone())?;
            Ok(document)
        }) {
            Ok(_) => {}
            Err(error) => library.load_failures.push(format!(
                "{}: {error}",
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("map draft")
            )),
        }
    }
    library
}

/// Loads one draft. A schema-1 draft is migrated in memory (BRL-wg28) and reported so the caller
/// saves it back as the current schema.
fn load_one(path: &Path) -> Result<(MapDraftDocument, bool), String> {
    let mut file = fs::File::open(path).map_err(|error| format!("open failed: {error}"))?;
    let length = file
        .metadata()
        .map_err(|error| format!("metadata failed: {error}"))?
        .len();
    if length > brawler::map::authoring::MAX_AUTHORED_DOCUMENT_BYTES as u64 {
        return Err("document exceeds the 512 KiB limit".to_string());
    }
    let mut source = String::with_capacity(usize::try_from(length).unwrap_or(0));
    file.read_to_string(&mut source)
        .map_err(|error| format!("read failed: {error}"))?;
    let source = rename_schema_1_anchor_kinds(&source);
    let mut document: MapDraftDocument =
        ron::from_str(&source).map_err(|error| format!("parse failed: {error}"))?;
    let migrated = document.schema_version == 1;
    if migrated {
        migrate_schema_1_draft(&mut document)?;
    }
    document.validate_envelope()?;
    Ok((document, migrated))
}

/// Schema-1 mode-anchor variant names renamed by BRL-wg28. Schema-2 drafts never contain them.
const SCHEMA_1_ANCHOR_KINDS: [(&str, &str); 2] = [
    ("HotZoneCircle", "TurfHoldCircle"),
    ("HeistSafe", "VaultBreakSafe"),
];

/// Schema-1 game-type ID prefixes renamed by BRL-wg28.
const SCHEMA_1_GAME_TYPE_PREFIXES: [(&str, &str); 3] = [
    ("wipeout-", "takedown-"),
    ("hot-zone-", "turf-hold-"),
    ("heist-", "vault-break-"),
];

/// Renames the schema-1 anchor variant identifiers outside RON string literals, so an author's
/// map name is never rewritten.
fn rename_schema_1_anchor_kinds(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = String::with_capacity(source.len());
    let mut index = 0;
    let mut copied = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                index += 1;
                while index < bytes.len() && bytes[index] != b'"' {
                    index += if bytes[index] == b'\\' { 2 } else { 1 };
                }
                index += 1;
            }
            byte if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = index;
                while index < bytes.len()
                    && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
                {
                    index += 1;
                }
                let word = &source[start..index];
                if let Some((_, renamed)) =
                    SCHEMA_1_ANCHOR_KINDS.iter().find(|(old, _)| *old == word)
                {
                    output.push_str(&source[copied..start]);
                    output.push_str(renamed);
                    copied = index;
                }
            }
            _ => index += 1,
        }
    }
    output.push_str(&source[copied.min(source.len())..]);
    output
}

fn migrate_schema_1_draft(document: &mut MapDraftDocument) -> Result<(), String> {
    if let Some(game_type) = &document.intended_game_type_id {
        let id = game_type.as_str();
        if let Some((old, new)) = SCHEMA_1_GAME_TYPE_PREFIXES
            .iter()
            .find(|(old, _)| id.starts_with(old))
        {
            document.intended_game_type_id = Some(
                brawler::lobby::GameTypeId::new(format!("{new}{}", &id[old.len()..]))
                    .map_err(|_| "migrated game type ID is invalid".to_string())?,
            );
        }
    }
    document.schema_version = super::model::MAP_DRAFT_SCHEMA_VERSION;
    Ok(())
}

pub fn save_map_draft(
    storage: &MapDraftStorage,
    document: &MapDraftDocument,
) -> Result<(), String> {
    document.validate_envelope()?;
    let pretty = ron::ser::PrettyConfig::new().struct_names(true);
    let source = ron::ser::to_string_pretty(document, pretty)
        .map_err(|error| format!("map draft serialization failed: {error}"))?;
    if source.len() > brawler::map::authoring::MAX_AUTHORED_DOCUMENT_BYTES {
        return Err("document exceeds the 512 KiB limit".to_string());
    }
    fs::create_dir_all(&storage.0)
        .map_err(|error| format!("map draft directory creation failed: {error}"))?;
    let path = storage.path_for(document.id);
    let mut file = AtomicWriteFile::options()
        .open(&path)
        .map_err(|error| format!("map draft save failed: {error}"))?;
    file.write_all(source.as_bytes())
        .map_err(|error| format!("map draft save failed: {error}"))?;
    file.commit()
        .map_err(|error| format!("map draft save failed: {error}"))
}

pub fn delete_map_draft(storage: &MapDraftStorage, id: MapDraftId) -> Result<(), String> {
    let path = storage.path_for(id);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("map draft delete failed: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brawler::map::{MapDimensions, MapPresentationThemeId};

    #[test]
    fn saved_draft_round_trips_and_malformed_neighbor_isolated() {
        let directory =
            std::env::temp_dir().join(format!("brawler-map-maker-{}", std::process::id()));
        let storage = MapDraftStorage(directory.clone());
        let document = MapDraftDocument::blank(
            MapDraftId(1),
            "First Arena",
            brawler::map::TAKEDOWN_MODE_DEFINITION,
            MapDimensions {
                width: 32,
                height: 24,
            },
            MapPresentationThemeId(1),
            brawler::map::GROUND_ASSET,
        )
        .unwrap();
        save_map_draft(&storage, &document).unwrap();
        fs::write(storage.0.join("broken.ron"), "not ron").unwrap();
        let loaded = load_map_drafts(&storage);
        assert_eq!(loaded.drafts.get(&MapDraftId(1)), Some(&document));
        assert_eq!(loaded.load_failures.len(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    /// A schema-1 draft (BRL-wg28) loads under the renamed anchor kinds and game type, keeps an
    /// author name that mentions an old variant, and is saved back as the current schema.
    #[test]
    fn schema_1_draft_migrates_to_renamed_modes_and_is_saved_back() {
        let directory =
            std::env::temp_dir().join(format!("brawler-map-maker-v1-{}", std::process::id()));
        let storage = MapDraftStorage(directory.clone());
        fs::create_dir_all(&storage.0).unwrap();
        let mut expected = MapDraftDocument::blank(
            MapDraftId(9),
            "HeistSafe Yard",
            brawler::map::VAULT_BREAK_MODE_DEFINITION,
            MapDimensions {
                width: 32,
                height: 24,
            },
            MapPresentationThemeId(1),
            brawler::map::GROUND_ASSET,
        )
        .unwrap();
        expected.intended_game_type_id =
            Some(brawler::lobby::GameTypeId::new("vault-break-2v2").unwrap());
        expected.recipe.mode_anchors = vec![
            brawler::map::MapModeAnchorPlacement {
                placement_id: brawler::map::MapPlacementId(1),
                anchor_id: brawler::map::ModeAnchorId(1),
                kind: brawler::map::MapModeAnchorKind::VaultBreakSafe {
                    team_slot: 0,
                    origin_cell: brawler::map::MapCell { x: 2, y: 3 },
                    width_cells: 2,
                    height_cells: 2,
                    quarter_turns: 0,
                    objective_visual_profile_id: brawler::map::VAULT_BREAK_SAFE_VISUAL_PROFILE,
                },
            },
            brawler::map::MapModeAnchorPlacement {
                placement_id: brawler::map::MapPlacementId(2),
                anchor_id: brawler::map::ModeAnchorId(2),
                kind: brawler::map::MapModeAnchorKind::TurfHoldCircle {
                    center_half_cell: brawler::map::MapHalfCellPoint { x: 32, y: 24 },
                    radius_half_cells: 6,
                },
            },
        ];
        let pretty = ron::ser::PrettyConfig::new().struct_names(true);
        let v1 = ron::ser::to_string_pretty(&expected, pretty)
            .unwrap()
            .replace("schema_version: 2", "schema_version: 1")
            .replace("VaultBreakSafe(", "HeistSafe(")
            .replace("TurfHoldCircle(", "HotZoneCircle(")
            .replace("\"vault-break-2v2\"", "\"heist-2v2\"");
        assert!(v1.contains("HeistSafe(") && v1.contains("\"heist-2v2\""));
        let path = storage.path_for(MapDraftId(9));
        fs::write(&path, &v1).unwrap();

        let loaded = load_map_drafts(&storage);
        assert!(
            loaded.load_failures.is_empty(),
            "{:?}",
            loaded.load_failures
        );
        assert_eq!(loaded.drafts.get(&MapDraftId(9)), Some(&expected));
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.contains("schema_version: 2"));
        assert!(!saved.contains("HotZoneCircle") && !saved.contains("heist-2v2"));
        fs::remove_dir_all(directory).unwrap();
    }
}
