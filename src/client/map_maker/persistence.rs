use super::{MapDraftDocument, MapDraftId, MapDraftLibrary};
use atomic_write_file::AtomicWriteFile;
use bevy::prelude::Resource;
use directories::ProjectDirs;
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
            std::env::var_os(crate::client::connection_persistence::CLIENT_DATA_DIRECTORY_ENV)
                .filter(|value| !value.is_empty())
        {
            return Self::for_data_directory(directory);
        }
        Self(ProjectDirs::from("com", "Brawler", "Brawler").map_or_else(
            || PathBuf::from("maps"),
            |dirs| dirs.config_dir().join("maps"),
        ))
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
        match load_one(&path).and_then(|document| {
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

fn load_one(path: &Path) -> Result<MapDraftDocument, String> {
    let mut file = fs::File::open(path).map_err(|error| format!("open failed: {error}"))?;
    let length = file
        .metadata()
        .map_err(|error| format!("metadata failed: {error}"))?
        .len();
    if length > crate::map::authoring::MAX_AUTHORED_DOCUMENT_BYTES as u64 {
        return Err("document exceeds the 512 KiB limit".to_string());
    }
    let mut source = String::with_capacity(usize::try_from(length).unwrap_or(0));
    file.read_to_string(&mut source)
        .map_err(|error| format!("read failed: {error}"))?;
    let document: MapDraftDocument =
        ron::from_str(&source).map_err(|error| format!("parse failed: {error}"))?;
    document.validate_envelope()?;
    Ok(document)
}

pub fn save_map_draft(
    storage: &MapDraftStorage,
    document: &MapDraftDocument,
) -> Result<(), String> {
    document.validate_envelope()?;
    let pretty = ron::ser::PrettyConfig::new().struct_names(true);
    let source = ron::ser::to_string_pretty(document, pretty)
        .map_err(|error| format!("map draft serialization failed: {error}"))?;
    if source.len() > crate::map::authoring::MAX_AUTHORED_DOCUMENT_BYTES {
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
    use crate::map::{MapDimensions, MapPresentationThemeId};

    #[test]
    fn saved_draft_round_trips_and_malformed_neighbor_isolated() {
        let directory =
            std::env::temp_dir().join(format!("brawler-map-maker-{}", std::process::id()));
        let storage = MapDraftStorage(directory.clone());
        let document = MapDraftDocument::blank(
            MapDraftId(1),
            "First Arena",
            crate::map::WIPEOUT_MODE_DEFINITION,
            MapDimensions {
                width: 32,
                height: 24,
            },
            MapPresentationThemeId(1),
            crate::map::GROUND_ASSET,
        )
        .unwrap();
        save_map_draft(&storage, &document).unwrap();
        fs::write(storage.0.join("broken.ron"), "not ron").unwrap();
        let loaded = load_map_drafts(&storage);
        assert_eq!(loaded.drafts.get(&MapDraftId(1)), Some(&document));
        assert_eq!(loaded.load_failures.len(), 1);
        fs::remove_dir_all(directory).unwrap();
    }
}
