//! Versioned, bounded persistence for the last server-accepted build selection.

use atomic_write_file::AtomicWriteFile;
use bevy::prelude::Resource;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
};

const BUILD_FILE_SCHEMA_VERSION: u16 = 1;
const MAX_BUILD_FILE_BYTES: u64 = 4 * 1024;

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct ClientBuildPath(pub PathBuf);

impl Default for ClientBuildPath {
    fn default() -> Self {
        let path = ProjectDirs::from("com", "Brawler", "Brawler").map_or_else(
            || PathBuf::from("build.ron"),
            |dirs| dirs.config_dir().join("build.ron"),
        );
        Self(path)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BuildFileV1 {
    pub schema_version: u16,
    pub build_revision: crate::builds::BuildRevision,
    pub selection: crate::builds::BuildSelection,
}

impl BuildFileV1 {
    #[must_use]
    pub const fn new(
        build_revision: crate::builds::BuildRevision,
        selection: crate::builds::BuildSelection,
    ) -> Self {
        Self {
            schema_version: BUILD_FILE_SCHEMA_VERSION,
            build_revision,
            selection,
        }
    }

    pub fn validate(
        self,
        builds: &crate::builds::BuildCatalog,
        weapons: &crate::combat::WeaponCatalog,
    ) -> Result<Self, String> {
        if self.schema_version != BUILD_FILE_SCHEMA_VERSION {
            return Err("unsupported build file schema".to_string());
        }
        if self.build_revision != builds.balance_revision {
            return Err("saved build revision is stale".to_string());
        }
        let fighter = crate::combat::FighterDefinitions::default()
            .get(crate::combat::STANDARD_FIGHTER_DEFINITION)
            .copied()
            .ok_or_else(|| "standard fighter definition is unavailable".to_string())?;
        let (recipe, source) = match self.selection {
            crate::builds::BuildSelection::Preset(id) => (
                builds
                    .preset(id)
                    .ok_or_else(|| "saved build preset is unknown".to_string())?
                    .recipe,
                Some(id),
            ),
            crate::builds::BuildSelection::Custom(recipe) => (recipe, None),
        };
        crate::builds::resolve_build_recipe(builds, weapons, &fighter, recipe, source)
            .map_err(|error| format!("saved build is invalid: {error:?}"))?;
        Ok(self)
    }
}

pub fn load_build(
    path: &Path,
    builds: &crate::builds::BuildCatalog,
    weapons: &crate::combat::WeaponCatalog,
) -> Result<Option<BuildFileV1>, String> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("could not read saved build: {error}")),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BUILD_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read saved build: {error}"))?;
    if bytes.len() > usize::try_from(MAX_BUILD_FILE_BYTES).unwrap_or(usize::MAX) {
        return Err("saved build file is too large".to_string());
    }
    let source = std::str::from_utf8(&bytes)
        .map_err(|error| format!("saved build is not UTF-8: {error}"))?;
    let file: BuildFileV1 =
        ron::from_str(source).map_err(|error| format!("saved build was rejected: {error}"))?;
    file.validate(builds, weapons).map(Some)
}

pub fn save_build(
    path: &Path,
    state: BuildFileV1,
    builds: &crate::builds::BuildCatalog,
    weapons: &crate::combat::WeaponCatalog,
) -> Result<(), String> {
    let state = state.validate(builds, weapons)?;
    let parent = path
        .parent()
        .ok_or_else(|| "saved build path has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create saved build directory: {error}"))?;
    let serialized = ron::ser::to_string_pretty(&state, ron::ser::PrettyConfig::default())
        .map_err(|error| format!("could not encode saved build: {error}"))?;
    if serialized.len() > usize::try_from(MAX_BUILD_FILE_BYTES).unwrap_or(usize::MAX) {
        return Err("saved build file is too large".to_string());
    }
    let mut file = AtomicWriteFile::open(path)
        .map_err(|error| format!("could not open atomic saved build file: {error}"))?;
    file.write_all(serialized.as_bytes())
        .map_err(|error| format!("could not write saved build: {error}"))?;
    file.commit()
        .map_err(|error| format!("could not replace saved build file: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "brawler-m04-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn content() -> (crate::builds::BuildCatalog, crate::combat::WeaponCatalog) {
        (
            crate::builds::BuildCatalog::embedded().unwrap(),
            crate::combat::WeaponCatalog::embedded().unwrap(),
        )
    }

    #[test]
    fn valid_preset_and_custom_round_trip() {
        let path = test_path("round-trip").join("build.ron");
        let (builds, weapons) = content();
        for selection in [
            crate::builds::BuildSelection::Preset(crate::builds::BuildPresetId(2)),
            crate::builds::BuildSelection::Custom(builds.presets[0].recipe),
        ] {
            let state = BuildFileV1::new(builds.balance_revision, selection);
            save_build(&path, state, &builds, &weapons).unwrap();
            assert_eq!(load_build(&path, &builds, &weapons).unwrap(), Some(state));
        }
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn missing_file_uses_default_without_error() {
        let path = test_path("missing").join("build.ron");
        let (builds, weapons) = content();
        assert_eq!(load_build(&path, &builds, &weapons), Ok(None));
    }

    #[test]
    fn malformed_stale_invalid_and_oversized_files_are_rejected() {
        let dir = test_path("rejected");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("build.ron");
        let (builds, weapons) = content();
        for source in [
            "not ron".to_string(),
            "(schema_version:2,build_revision:(1),selection:Preset((1)))".to_string(),
            "(schema_version:1,build_revision:(999),selection:Preset((1)))".to_string(),
            "(schema_version:1,build_revision:(1),selection:Preset((999)))".to_string(),
        ] {
            fs::write(&path, &source).unwrap();
            assert!(load_build(&path, &builds, &weapons).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), source);
        }
        fs::write(
            &path,
            vec![b'x'; usize::try_from(MAX_BUILD_FILE_BYTES + 1).unwrap()],
        )
        .unwrap();
        assert!(load_build(&path, &builds, &weapons).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_failure_does_not_replace_active_memory() {
        let dir = test_path("save-failure");
        fs::create_dir_all(&dir).unwrap();
        let (builds, weapons) = content();
        let active = BuildFileV1::new(
            builds.balance_revision,
            crate::builds::BuildSelection::Preset(crate::builds::BuildPresetId(1)),
        );
        let invalid_path = dir.join("directory-target");
        fs::create_dir_all(&invalid_path).unwrap();
        assert!(save_build(&invalid_path, active, &builds, &weapons).is_err());
        assert_eq!(
            active.selection,
            crate::builds::BuildSelection::Preset(crate::builds::BuildPresetId(1))
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
