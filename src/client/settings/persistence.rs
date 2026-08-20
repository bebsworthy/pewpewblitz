//! Bounded persistence for the one current pre-release client settings shape.

use super::{ClientInputSettings, GamepadBindings, KeyboardBindings};
use atomic_write_file::AtomicWriteFile;
use bevy::prelude::{MouseButton, Resource};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_SETTINGS_BYTES: u64 = 64 * 1024;
pub const MIN_UI_SCALE: f32 = 0.8;
pub const MAX_UI_SCALE: f32 = 1.4;

/// Persistent shell preferences kept separate from input capture and focus state.
#[allow(
    clippy::struct_excessive_bools,
    reason = "these independent user-facing toggles are the complete current settings contract"
)]
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ClientShellSettings {
    pub ui_scale: f32,
    pub reduced_motion: bool,
    pub reduced_combat_effects: bool,
    pub master_volume: u8,
    pub mute_when_unfocused: bool,
    pub fullscreen: bool,
    pub vsync: bool,
}

impl Default for ClientShellSettings {
    fn default() -> Self {
        Self {
            ui_scale: 1.0,
            reduced_motion: false,
            reduced_combat_effects: false,
            master_volume: 100,
            mute_when_unfocused: true,
            fullscreen: false,
            vsync: true,
        }
    }
}

impl ClientShellSettings {
    pub fn validate(self) -> Result<(), String> {
        if !self.ui_scale.is_finite() || !(MIN_UI_SCALE..=MAX_UI_SCALE).contains(&self.ui_scale) {
            return Err(format!(
                "UI scale must be between {MIN_UI_SCALE:.1} and {MAX_UI_SCALE:.1}"
            ));
        }
        if self.master_volume > 100 || !self.master_volume.is_multiple_of(10) {
            return Err("Master volume must be between 0 and 100 in steps of 10".to_string());
        }
        Ok(())
    }
}

/// Injectable settings destination. Production uses the platform-local configuration directory;
/// focused tests provide a temporary directory without introducing a filesystem abstraction.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct ClientSettingsPath(pub PathBuf);

impl Default for ClientSettingsPath {
    fn default() -> Self {
        let path = ProjectDirs::from("com", "Brawler", "Brawler").map_or_else(
            || PathBuf::from("settings.ron"),
            |dirs| dirs.config_dir().join("settings.ron"),
        );
        Self(path)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the persisted shape directly mirrors the bounded current settings contract"
)]
pub struct SettingsFile {
    pub keyboard: KeyboardBindings,
    pub gamepad: GamepadBindings,
    pub mouse_primary: MouseButton,
    pub move_deadzone: f32,
    pub aim_deadzone: f32,
    pub aim_commit_threshold: f32,
    pub trigger_press: f32,
    pub trigger_release: f32,
    pub invert_move_y: bool,
    pub invert_aim_y: bool,
    pub ui_scale: f32,
    pub reduced_motion: bool,
    pub reduced_combat_effects: bool,
    pub master_volume: u8,
    pub mute_when_unfocused: bool,
    pub fullscreen: bool,
    pub vsync: bool,
}

impl SettingsFile {
    #[must_use]
    pub fn from_active(input: ClientInputSettings, shell: ClientShellSettings) -> Self {
        Self {
            keyboard: input.keyboard,
            gamepad: input.gamepad,
            mouse_primary: input.mouse_primary,
            move_deadzone: input.move_deadzone,
            aim_deadzone: input.aim_deadzone,
            aim_commit_threshold: input.aim_commit_threshold,
            trigger_press: input.trigger_press,
            trigger_release: input.trigger_release,
            invert_move_y: input.invert_move_y,
            invert_aim_y: input.invert_aim_y,
            ui_scale: shell.ui_scale,
            reduced_motion: shell.reduced_motion,
            reduced_combat_effects: shell.reduced_combat_effects,
            master_volume: shell.master_volume,
            mute_when_unfocused: shell.mute_when_unfocused,
            fullscreen: shell.fullscreen,
            vsync: shell.vsync,
        }
    }

    pub fn into_active(self) -> Result<(ClientInputSettings, ClientShellSettings), String> {
        let input = ClientInputSettings {
            keyboard: self.keyboard,
            gamepad: self.gamepad,
            mouse_primary: self.mouse_primary,
            move_deadzone: self.move_deadzone,
            aim_deadzone: self.aim_deadzone,
            aim_commit_threshold: self.aim_commit_threshold,
            trigger_press: self.trigger_press,
            trigger_release: self.trigger_release,
            invert_move_y: self.invert_move_y,
            invert_aim_y: self.invert_aim_y,
            revision: 0,
        };
        input.validate()?;
        let shell = ClientShellSettings {
            ui_scale: self.ui_scale,
            reduced_motion: self.reduced_motion,
            reduced_combat_effects: self.reduced_combat_effects,
            master_volume: self.master_volume,
            mute_when_unfocused: self.mute_when_unfocused,
            fullscreen: self.fullscreen,
            vsync: self.vsync,
        };
        shell.validate()?;
        Ok((input, shell))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsLoadError {
    Read(String),
    TooLarge(u64),
    Rejected(String),
}

impl core::fmt::Display for SettingsLoadError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "could not read settings: {error}"),
            Self::TooLarge(size) => write!(formatter, "settings file is too large ({size} bytes)"),
            Self::Rejected(error) => write!(formatter, "settings were rejected: {error}"),
        }
    }
}

pub fn load_settings(
    path: &Path,
) -> Result<Option<(ClientInputSettings, ClientShellSettings)>, SettingsLoadError> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(SettingsLoadError::Read(error.to_string())),
    };
    let metadata = file
        .metadata()
        .map_err(|error| SettingsLoadError::Read(error.to_string()))?;
    if metadata.len() > MAX_SETTINGS_BYTES {
        return Err(SettingsLoadError::TooLarge(metadata.len()));
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len().min(MAX_SETTINGS_BYTES)).unwrap_or_default(),
    );
    file.take(MAX_SETTINGS_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| SettingsLoadError::Read(error.to_string()))?;
    if bytes.len() > usize::try_from(MAX_SETTINGS_BYTES).unwrap_or(usize::MAX) {
        return Err(SettingsLoadError::TooLarge(
            u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        ));
    }
    let source =
        String::from_utf8(bytes).map_err(|error| SettingsLoadError::Rejected(error.to_string()))?;
    let file: SettingsFile =
        ron::from_str(&source).map_err(|error| SettingsLoadError::Rejected(error.to_string()))?;
    file.into_active()
        .map(Some)
        .map_err(SettingsLoadError::Rejected)
}

pub fn save_settings(
    path: &Path,
    input: ClientInputSettings,
    shell: ClientShellSettings,
) -> Result<(), String> {
    input.validate()?;
    shell.validate()?;
    let parent = path
        .parent()
        .ok_or_else(|| "settings path has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create settings directory: {error}"))?;
    let serialized = ron::ser::to_string_pretty(
        &SettingsFile::from_active(input, shell),
        ron::ser::PrettyConfig::default(),
    )
    .map_err(|error| format!("could not encode settings: {error}"))?;
    let mut file = AtomicWriteFile::open(path)
        .map_err(|error| format!("could not open atomic settings file: {error}"))?;
    file.write_all(serialized.as_bytes())
        .map_err(|error| format!("could not write settings: {error}"))?;
    file.commit()
        .map_err(|error| format!("could not replace settings file: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("brawler-m02-{}-{nonce}-{name}", std::process::id()))
    }

    #[test]
    fn valid_settings_round_trip_without_runtime_revision() {
        let path = test_path("round-trip").join("settings.ron");
        let input = ClientInputSettings {
            invert_aim_y: true,
            revision: 91,
            ..ClientInputSettings::default()
        };
        let shell = ClientShellSettings {
            ui_scale: 1.2,
            reduced_motion: true,
            reduced_combat_effects: true,
            master_volume: 60,
            fullscreen: true,
            vsync: false,
            ..ClientShellSettings::default()
        };
        save_settings(&path, input, shell).unwrap();
        assert!(
            !fs::read_to_string(&path)
                .unwrap()
                .contains("schema_version")
        );
        let (loaded_input, loaded_shell) = load_settings(&path).unwrap().unwrap();
        assert!(loaded_input.invert_aim_y);
        assert_eq!(loaded_input.revision, 0);
        assert_eq!(loaded_shell, shell);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn missing_file_uses_defaults_without_error() {
        let path = test_path("missing").join("settings.ron");
        assert_eq!(load_settings(&path), Ok(None));
    }

    #[test]
    fn malformed_stale_invalid_and_oversized_files_are_rejected_without_replacement() {
        let dir = test_path("rejected");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.ron");
        fs::write(&path, "this is not ron").unwrap();
        assert!(matches!(
            load_settings(&path),
            Err(SettingsLoadError::Rejected(_))
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), "this is not ron");

        let stale_source = "(schema_version:1,ui_scale:1.0,reduced_motion:false)";
        fs::write(&path, stale_source).unwrap();
        assert!(matches!(
            load_settings(&path),
            Err(SettingsLoadError::Rejected(_))
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), stale_source);

        let mut invalid = SettingsFile::from_active(
            ClientInputSettings::default(),
            ClientShellSettings::default(),
        );
        invalid.ui_scale = MAX_UI_SCALE + 1.0;
        let invalid_source = ron::to_string(&invalid).unwrap();
        fs::write(&path, &invalid_source).unwrap();
        assert!(matches!(
            load_settings(&path),
            Err(SettingsLoadError::Rejected(_))
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), invalid_source);

        fs::write(
            &path,
            "x".repeat(usize::try_from(MAX_SETTINGS_BYTES + 1).unwrap()),
        )
        .unwrap();
        assert!(matches!(
            load_settings(&path),
            Err(SettingsLoadError::TooLarge(_))
        ));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_failure_does_not_change_in_memory_values() {
        let dir = test_path("save-failure");
        fs::create_dir_all(&dir).unwrap();
        let invalid_target = dir.join("directory-target");
        fs::create_dir_all(&invalid_target).unwrap();
        let input = ClientInputSettings::default();
        let shell = ClientShellSettings::default();
        assert!(save_settings(&invalid_target, input, shell).is_err());
        assert_eq!(input, ClientInputSettings::default());
        assert_eq!(shell, ClientShellSettings::default());
        fs::remove_dir_all(dir).unwrap();
    }
}
