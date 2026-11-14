//! Bounded persistence for the one current pre-release client settings shape.

use super::{ClientInputSettings, GamepadBindings, KeyboardBindings};
use atomic_write_file::AtomicWriteFile;
use bevy::prelude::{MouseButton, Resource};
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
    /// Camera shake intensity in percent (BRL-0173), independent of the reduced-motion and
    /// reduced-combat-effects toggles, which still disable shake entirely.
    pub camera_shake: u8,
    pub master_volume: u8,
    /// Combat, match, and interface one-shots, as a share of the master volume.
    pub effects_volume: u8,
    /// Menu and match music, as a share of the master volume.
    pub music_volume: u8,
    pub mute_when_unfocused: bool,
    pub fullscreen: bool,
    pub vsync: bool,
    /// Requested presentation quality tier (BRL-0161); `Auto` lets the client pick for the GPU.
    pub graphics_quality: GraphicsQuality,
}

/// The player's presentation quality preference. The client resolves `Auto` to a concrete tier
/// from detected GPU capability; the tier contents are client-only authored data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphicsQuality {
    #[default]
    Auto,
    Low,
    Medium,
    High,
}

impl GraphicsQuality {
    /// Stable settings-row order: the automatic choice first, then ascending cost.
    pub const ORDER: [Self; 4] = [Self::Auto, Self::Low, Self::Medium, Self::High];

    #[must_use]
    pub fn index(self) -> usize {
        Self::ORDER
            .iter()
            .position(|quality| *quality == self)
            .unwrap_or_default()
    }

    /// The next preference in [`Self::ORDER`], wrapping from `High` back to `Auto`.
    #[must_use]
    pub fn next(self) -> Self {
        Self::ORDER[(self.index() + 1) % Self::ORDER.len()]
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "AUTO",
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
        }
    }
}

pub const DEFAULT_EFFECTS_VOLUME: u8 = 100;
pub const DEFAULT_MUSIC_VOLUME: u8 = 70;
pub const DEFAULT_CAMERA_SHAKE: u8 = 100;

impl Default for ClientShellSettings {
    fn default() -> Self {
        Self {
            ui_scale: 1.0,
            reduced_motion: false,
            reduced_combat_effects: false,
            camera_shake: DEFAULT_CAMERA_SHAKE,
            master_volume: 100,
            effects_volume: DEFAULT_EFFECTS_VOLUME,
            music_volume: DEFAULT_MUSIC_VOLUME,
            mute_when_unfocused: true,
            fullscreen: false,
            vsync: true,
            graphics_quality: GraphicsQuality::Auto,
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
        for (name, volume) in [
            ("Master", self.master_volume),
            ("Effects", self.effects_volume),
            ("Music", self.music_volume),
        ] {
            if volume > 100 || !volume.is_multiple_of(10) {
                return Err(format!(
                    "{name} volume must be between 0 and 100 in steps of 10"
                ));
            }
        }
        if self.camera_shake > 100 || !self.camera_shake.is_multiple_of(10) {
            return Err("Camera shake must be between 0 and 100 in steps of 10".to_string());
        }
        Ok(())
    }

    /// Camera shake amplitude scale in [0, 1]. Reduced motion and reduced combat effects are
    /// applied separately by the camera rig.
    #[must_use]
    pub fn camera_shake_scale(self) -> f32 {
        f32::from(self.camera_shake.min(100)) / 100.0
    }
}

/// Injectable settings destination. Production uses the platform-local configuration directory;
/// focused tests provide a temporary directory without introducing a filesystem abstraction.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct ClientSettingsPath(pub PathBuf);

impl Default for ClientSettingsPath {
    fn default() -> Self {
        let path = crate::platform_dirs::config_directory().map_or_else(
            || PathBuf::from("settings.ron"),
            |directory| directory.join("settings.ron"),
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
    /// Added by BRL-0160; defaulted so earlier settings files still load.
    #[serde(default = "default_touch_deadzone")]
    pub touch_deadzone: f32,
    #[serde(default = "default_touch_sensitivity")]
    pub touch_sensitivity: f32,
    pub ui_scale: f32,
    pub reduced_motion: bool,
    pub reduced_combat_effects: bool,
    /// Added by BRL-0173; defaulted so earlier settings files still load at full shake.
    #[serde(default = "default_camera_shake")]
    pub camera_shake: u8,
    pub master_volume: u8,
    /// Added by BRL-0158. Defaulted so a settings file saved before the audio categories existed
    /// still loads instead of being rejected with every binding; unknown fields stay rejected.
    #[serde(default = "default_effects_volume")]
    pub effects_volume: u8,
    #[serde(default = "default_music_volume")]
    pub music_volume: u8,
    pub mute_when_unfocused: bool,
    pub fullscreen: bool,
    pub vsync: bool,
    /// Added by BRL-0161; defaulted so earlier settings files still load.
    #[serde(default)]
    pub graphics_quality: GraphicsQuality,
}

const fn default_effects_volume() -> u8 {
    DEFAULT_EFFECTS_VOLUME
}

const fn default_music_volume() -> u8 {
    DEFAULT_MUSIC_VOLUME
}

const fn default_camera_shake() -> u8 {
    DEFAULT_CAMERA_SHAKE
}

const fn default_touch_deadzone() -> f32 {
    super::DEFAULT_TOUCH_DEADZONE
}

const fn default_touch_sensitivity() -> f32 {
    super::DEFAULT_TOUCH_SENSITIVITY
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
            touch_deadzone: input.touch_deadzone,
            touch_sensitivity: input.touch_sensitivity,
            ui_scale: shell.ui_scale,
            reduced_motion: shell.reduced_motion,
            reduced_combat_effects: shell.reduced_combat_effects,
            camera_shake: shell.camera_shake,
            master_volume: shell.master_volume,
            effects_volume: shell.effects_volume,
            music_volume: shell.music_volume,
            mute_when_unfocused: shell.mute_when_unfocused,
            fullscreen: shell.fullscreen,
            vsync: shell.vsync,
            graphics_quality: shell.graphics_quality,
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
            touch_deadzone: self.touch_deadzone,
            touch_sensitivity: self.touch_sensitivity,
            revision: 0,
        };
        input.validate()?;
        let shell = ClientShellSettings {
            ui_scale: self.ui_scale,
            reduced_motion: self.reduced_motion,
            reduced_combat_effects: self.reduced_combat_effects,
            camera_shake: self.camera_shake,
            master_volume: self.master_volume,
            effects_volume: self.effects_volume,
            music_volume: self.music_volume,
            mute_when_unfocused: self.mute_when_unfocused,
            fullscreen: self.fullscreen,
            vsync: self.vsync,
            graphics_quality: self.graphics_quality,
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
            effects_volume: 80,
            music_volume: 30,
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
    fn settings_saved_before_audio_categories_load_with_default_category_volumes() {
        let dir = test_path("pre-categories");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.ron");
        let current = ron::ser::to_string_pretty(
            &SettingsFile::from_active(
                ClientInputSettings::default(),
                ClientShellSettings {
                    master_volume: 40,
                    ..ClientShellSettings::default()
                },
            ),
            ron::ser::PrettyConfig::default(),
        )
        .unwrap();
        let legacy = current
            .lines()
            .filter(|line| !line.contains("effects_volume") && !line.contains("music_volume"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_ne!(legacy, current);
        fs::write(&path, legacy).unwrap();
        let (_, shell) = load_settings(&path).unwrap().unwrap();
        assert_eq!(shell.master_volume, 40);
        assert_eq!(shell.effects_volume, DEFAULT_EFFECTS_VOLUME);
        assert_eq!(shell.music_volume, DEFAULT_MUSIC_VOLUME);

        let mut invalid = SettingsFile::from_active(
            ClientInputSettings::default(),
            ClientShellSettings::default(),
        );
        invalid.music_volume = 75;
        fs::write(&path, ron::to_string(&invalid).unwrap()).unwrap();
        assert!(matches!(
            load_settings(&path),
            Err(SettingsLoadError::Rejected(_))
        ));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn graphics_quality_round_trips_and_older_files_load_as_auto() {
        let dir = test_path("pre-quality");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.ron");
        let low = ClientShellSettings {
            graphics_quality: GraphicsQuality::Low,
            ..ClientShellSettings::default()
        };
        save_settings(&path, ClientInputSettings::default(), low).unwrap();
        let current = fs::read_to_string(&path).unwrap();
        let (_, shell) = load_settings(&path).unwrap().unwrap();
        assert_eq!(shell, low);

        let legacy = current
            .lines()
            .filter(|line| !line.contains("graphics_quality"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_ne!(legacy, current);
        fs::write(&path, legacy).unwrap();
        let (_, shell) = load_settings(&path).unwrap().unwrap();
        assert_eq!(shell.graphics_quality, GraphicsQuality::Auto);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn camera_shake_round_trips_validates_and_older_files_load_at_full_shake() {
        let dir = test_path("pre-camera-shake");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.ron");
        let low = ClientShellSettings {
            camera_shake: 30,
            ..ClientShellSettings::default()
        };
        save_settings(&path, ClientInputSettings::default(), low).unwrap();
        let current = fs::read_to_string(&path).unwrap();
        let (_, shell) = load_settings(&path).unwrap().unwrap();
        assert_eq!(shell, low);
        assert!((shell.camera_shake_scale() - 0.3).abs() < 1e-6);

        let legacy = current
            .lines()
            .filter(|line| !line.contains("camera_shake"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_ne!(legacy, current);
        fs::write(&path, legacy).unwrap();
        let (_, shell) = load_settings(&path).unwrap().unwrap();
        assert_eq!(shell.camera_shake, DEFAULT_CAMERA_SHAKE);
        assert!((shell.camera_shake_scale() - 1.0).abs() < f32::EPSILON);

        for invalid_value in [35, 110] {
            let mut invalid = SettingsFile::from_active(
                ClientInputSettings::default(),
                ClientShellSettings::default(),
            );
            invalid.camera_shake = invalid_value;
            fs::write(&path, ron::to_string(&invalid).unwrap()).unwrap();
            assert!(matches!(
                load_settings(&path),
                Err(SettingsLoadError::Rejected(_))
            ));
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn settings_saved_before_touch_controls_load_with_default_touch_tuning() {
        let dir = test_path("pre-touch");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.ron");
        let tuned = ClientInputSettings {
            touch_deadzone: 0.3,
            touch_sensitivity: 0.9,
            ..ClientInputSettings::default()
        };
        let current = ron::ser::to_string_pretty(
            &SettingsFile::from_active(tuned, ClientShellSettings::default()),
            ron::ser::PrettyConfig::default(),
        )
        .unwrap();
        fs::write(&path, &current).unwrap();
        let (input, _) = load_settings(&path).unwrap().unwrap();
        assert_eq!(input, tuned);

        let legacy = current
            .lines()
            .filter(|line| !line.contains("touch_"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_ne!(legacy, current);
        fs::write(&path, legacy).unwrap();
        let (input, _) = load_settings(&path).unwrap().unwrap();
        assert_eq!(input, ClientInputSettings::default());
        fs::remove_dir_all(dir).unwrap();
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
