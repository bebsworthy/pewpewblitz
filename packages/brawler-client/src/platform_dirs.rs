//! Player-local file locations. Desktop keeps the platform configuration directory; Android has
//! no XDG/home directory, so player files live in the app's private internal storage and F12
//! captures in the app-specific external directory that `adb pull` can read.

use std::path::PathBuf;

/// Directory for settings, saved connections, and map drafts.
#[cfg(not(target_os = "android"))]
pub(crate) fn config_directory() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "Brawler", "Brawler")
        .map(|dirs| dirs.config_dir().to_path_buf())
}

/// Directory for settings, saved connections, and map drafts.
#[cfg(target_os = "android")]
pub(crate) fn config_directory() -> Option<PathBuf> {
    bevy::android::ANDROID_APP
        .get()
        .and_then(bevy::android::android_activity::AndroidApp::internal_data_path)
}

/// Directory for F12 evidence captures on Android (`adb pull`-readable app-specific storage).
#[cfg(target_os = "android")]
pub(crate) fn android_capture_directory() -> Option<PathBuf> {
    bevy::android::ANDROID_APP
        .get()
        .and_then(bevy::android::android_activity::AndroidApp::external_data_path)
        .map(|directory| directory.join("captures"))
}
