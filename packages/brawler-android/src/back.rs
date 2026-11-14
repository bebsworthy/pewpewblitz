//! Android predictive-back adapter (BRL-0198).
//!
//! `MainActivity` registers an androidx `OnBackPressedCallback`; at targetSdk 37 the system
//! delivers the Back gesture only through that callback, never as a key event. The callback runs
//! on the Android UI thread and asks [`Java_com_pewpewblitz_client_MainActivity_nativeOnBackPressed`]
//! whether the shell can go back: at a root screen it returns false and Java moves the task to the
//! background; otherwise the press is queued here and replayed into the shared
//! [`brawler::client::BackInput`] on the Bevy thread.

use bevy::prelude::*;
use brawler::client::BackInput;
use std::{
    ffi::c_void,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

/// Back presses accepted by the UI thread and not yet replayed into Bevy.
static PENDING_BACK: AtomicU32 = AtomicU32::new(0);
/// Last published "Back here leaves the app"; true until the shell publishes, so Back during
/// startup backgrounds the app instead of being swallowed.
static BACK_LEAVES_APP: AtomicBool = AtomicBool::new(true);

/// JNI: `private native boolean nativeOnBackPressed()` on `MainActivity`.
///
/// Returns `JNI_TRUE` when the game consumed Back, `JNI_FALSE` when the activity should move to
/// the background. It touches only atomics, so it never blocks the UI thread on the game loop.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_pewpewblitz_client_MainActivity_nativeOnBackPressed(
    _env: *mut c_void,
    _activity: *mut c_void,
) -> u8 {
    if BACK_LEAVES_APP.load(Ordering::Acquire) {
        return 0;
    }
    PENDING_BACK.fetch_add(1, Ordering::AcqRel);
    1
}

pub(super) fn install(app: &mut App) {
    // `First` precedes `BackInputSystems` (PreUpdate) and follows the shell's `Last` publication.
    app.add_systems(First, bridge_back);
}

fn bridge_back(mut back: ResMut<BackInput>) {
    BACK_LEAVES_APP.store(back.leaves_app(), Ordering::Release);
    // Several presses within one frame still step back once; the next frame's screen decides
    // what a later press means.
    if PENDING_BACK.swap(0, Ordering::AcqRel) > 0 {
        back.request();
    }
}
