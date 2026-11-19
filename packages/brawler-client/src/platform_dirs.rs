//! Player-local file locations. Desktop keeps the platform configuration directory; Android has
//! no XDG/home directory, so player files live in the app's private internal storage and F12
//! captures in the app-specific external directory that `adb pull` can read.

use std::path::PathBuf;

/// Directory for settings, saved connections, and map drafts.
#[cfg(all(not(target_os = "android"), not(any(test, feature = "network-test"))))]
pub(crate) fn config_directory() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "Brawler", "Brawler")
        .map(|dirs| dirs.config_dir().to_path_buf())
}

/// Test builds never touch the player's real files: every call returns a fresh, empty directory
/// under the system temp directory, so a default-constructed settings, connections, or map-draft
/// path is isolated per test app. Tests that need a specific location still insert their own path.
#[cfg(all(not(target_os = "android"), any(test, feature = "network-test")))]
#[allow(
    clippy::unnecessary_wraps,
    reason = "keeps the production signature, whose platform lookup can fail"
)]
pub(crate) fn config_directory() -> Option<PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    Some(std::env::temp_dir().join(format!(
        "brawler-client-test-config-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )))
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
