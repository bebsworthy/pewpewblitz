//! Android on-screen keyboard adapter (BRL-06pz).
//!
//! winit 0.30's Android backend shows the keyboard for `Window::set_ime_allowed` but drops the
//! `GameActivity` text-input state and editor-action events, so typed text never reaches Bevy. This
//! adapter drives `GameTextInput` through `android-activity` instead:
//!
//! * When the client publishes a [`TextEntryFocus`], it configures the keyboard for the field,
//!   mirrors the field's text and caret into `GameTextInput`, and shows the keyboard; when the focus
//!   ends it hides the keyboard.
//! * Each frame it compares `GameTextInput`'s text with the mirrored field text and replays the
//!   difference with [`TextEntry::edit_toward`]: Backspace key messages, then an [`Ime::Commit`]
//!   with the inserted text. The editors' own validation decides what is kept; when the field
//!   differs from what the keyboard showed (a rejected character, a normalized name) the field's
//!   text is written back to the keyboard.
//! * `MainActivity` forwards the keyboard's action key (Done) and its dismissal (Back or the
//!   keyboard's hide control) through JNI. Done becomes Enter; a dismissal while editing becomes
//!   the platform Back request, which cancels the field exactly as Escape does.
//!
//! The JNI entry points run on the Android UI thread and only touch atomics.

use bevy::{
    android::{
        ANDROID_APP,
        android_activity::{
            AndroidApp,
            input::{ImeOptions, InputType, TextInputAction, TextInputState, TextSpan},
        },
    },
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
    window::PrimaryWindow,
};
use brawler_client::{BackInput, TextEntry, TextEntryField, TextEntryFocus};
use std::{
    ffi::c_void,
    sync::atomic::{AtomicBool, Ordering},
};

/// The keyboard's action key was pressed and not yet replayed as Enter.
static PENDING_ACTION: AtomicBool = AtomicBool::new(false);
/// The keyboard has been visible since the current field started editing.
static KEYBOARD_SHOWN: AtomicBool = AtomicBool::new(false);
/// The keyboard was hidden by the player after it had been shown for the current field.
static KEYBOARD_DISMISSED: AtomicBool = AtomicBool::new(false);

/// JNI: `private native void nativeOnImeAction()` on `MainActivity`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_pewpewblitz_client_MainActivity_nativeOnImeAction(
    _env: *mut c_void,
    _activity: *mut c_void,
) {
    PENDING_ACTION.store(true, Ordering::Release);
}

/// JNI: `private native void nativeOnSoftKeyboardVisibilityChanged(boolean visible)`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_pewpewblitz_client_MainActivity_nativeOnSoftKeyboardVisibilityChanged(
    _env: *mut c_void,
    _activity: *mut c_void,
    visible: u8,
) {
    if visible != 0 {
        KEYBOARD_SHOWN.store(true, Ordering::Release);
    } else if KEYBOARD_SHOWN.swap(false, Ordering::AcqRel) {
        KEYBOARD_DISMISSED.store(true, Ordering::Release);
    }
}

pub(super) fn install(app: &mut App) {
    // `First` precedes Bevy's keyboard input update and the client's `BackInputSystems`
    // (`PreUpdate`), so replayed keys and commits reach this frame's editors.
    app.add_systems(First, bridge_soft_keyboard);
}

/// Frames to wait for `GameTextInput` to apply a mirrored text before writing it again.
const MIRROR_SETTLE_FRAMES: u8 = 30;

/// The field the keyboard is editing and the text the keyboard was last known to agree on.
struct KeyboardSession {
    field: TextEntryField,
    mirrored: String,
    /// Frames since the field's text was written to `GameTextInput`, until the keyboard reports
    /// it. The write is applied asynchronously on the UI thread; until then the keyboard still
    /// reports older text, which must not be replayed as an edit.
    settling: Option<u8>,
}

impl KeyboardSession {
    fn mirror(&mut self, android: &AndroidApp, entry: &TextEntry) {
        mirror_into_keyboard(android, entry);
        self.mirrored = entry.text().to_string();
        self.settling = Some(0);
    }

    /// Replays the keyboard's change to the field, or restores the field's text to the keyboard.
    fn sync(
        &mut self,
        android: &AndroidApp,
        entry: &TextEntry,
        window: Entity,
        keys: &mut MessageWriter<KeyboardInput>,
        ime: &mut MessageWriter<Ime>,
    ) {
        if self.mirrored != entry.text() {
            // The field kept something other than what the keyboard showed.
            self.mirror(android, entry);
            return;
        }
        let typed = android.text_input_state().text;
        if let Some(frames) = self.settling {
            if typed == self.mirrored {
                self.settling = None;
            } else if frames >= MIRROR_SETTLE_FRAMES {
                self.mirror(android, entry);
            } else {
                self.settling = Some(frames + 1);
            }
            return;
        }
        if typed == self.mirrored {
            return;
        }
        let Some(edit) = entry.edit_toward(&typed) else {
            self.mirror(android, entry);
            return;
        };
        for _ in 0..edit.backspaces {
            press(keys, window, KeyCode::Backspace, &Key::Backspace);
        }
        if !edit.insert.is_empty() {
            ime.write(Ime::Commit {
                window,
                value: edit.insert,
            });
        }
        if edit.submit {
            PENDING_ACTION.store(true, Ordering::Release);
        }
        self.mirrored = typed;
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn bridge_soft_keyboard(
    mut focus: Option<ResMut<TextEntryFocus>>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut session: Local<Option<KeyboardSession>>,
    mut keys: MessageWriter<KeyboardInput>,
    mut ime: MessageWriter<Ime>,
    mut back: ResMut<BackInput>,
) {
    let Some(android) = ANDROID_APP.get() else {
        return;
    };
    let entry = focus.as_deref().and_then(TextEntryFocus::active).cloned();
    if let Some(focus) = focus.as_mut() {
        // The keyboard is requested whenever a field is edited; the client mirrors that field
        // above it.
        focus.set_on_screen_keyboard(entry.is_some());
    }
    let Some(entry) = entry.as_ref() else {
        if session.take().is_some() {
            android.hide_soft_input(false);
        }
        PENDING_ACTION.store(false, Ordering::Release);
        KEYBOARD_DISMISSED.store(false, Ordering::Release);
        return;
    };
    let Some(current) = session
        .as_mut()
        .filter(|current| current.field == entry.field())
    else {
        let mut started = KeyboardSession {
            field: entry.field(),
            mirrored: String::new(),
            settling: None,
        };
        begin_editing(android, entry);
        started.mirror(android, entry);
        // Explicit (not implicit) show: the player tapped the field.
        android.show_soft_input(false);
        *session = Some(started);
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    current.sync(android, entry, window, &mut keys, &mut ime);
    if PENDING_ACTION.swap(false, Ordering::AcqRel) {
        press(&mut keys, window, KeyCode::Enter, &Key::Enter);
    }
    if KEYBOARD_DISMISSED.swap(false, Ordering::AcqRel) {
        back.request();
    }
}

fn begin_editing(android: &AndroidApp, entry: &TextEntry) {
    PENDING_ACTION.store(false, Ordering::Release);
    KEYBOARD_DISMISSED.store(false, Ordering::Release);
    // GameActivity applies the layout, the text, and the show request on the UI thread in order.
    // GameTextInput leaves the landscape full-screen extract editor blank, so the keyboard stays
    // below the game and the client mirrors the field in a strip above it. The Enter key's action
    // lives in the options' action bits.
    let mut options = ImeOptions::IME_FLAG_NO_FULLSCREEN | ImeOptions::IMG_FLAG_NO_EXTRACT_UI;
    options.set_action(TextInputAction::Done);
    android.set_ime_editor_info(input_type(entry.field()), TextInputAction::Done, options);
}

fn input_type(field: TextEntryField) -> InputType {
    // Suggestions off: input methods then commit characters directly instead of composing words,
    // so the field updates as the player types.
    let text = InputType::TYPE_CLASS_TEXT | InputType::TYPE_TEXT_FLAG_NO_SUGGESTIONS;
    match field {
        TextEntryField::ServerAddress => text | InputType::TYPE_TEXT_VARIATION_URI,
        TextEntryField::PlayerName | TextEntryField::BrawlerName => {
            text | InputType::TYPE_TEXT_VARIATION_PERSON_NAME | InputType::TYPE_TEXT_FLAG_CAP_WORDS
        }
        TextEntryField::MapName => text | InputType::TYPE_TEXT_FLAG_CAP_WORDS,
    }
}

fn mirror_into_keyboard(android: &AndroidApp, entry: &TextEntry) {
    let caret = entry.caret_utf16();
    android.set_text_input_state(TextInputState {
        text: entry.text().to_string(),
        selection: TextSpan {
            start: caret,
            end: caret,
        },
        compose_region: None,
    });
}

/// One synthetic press and release, as a hardware key would arrive.
fn press(keys: &mut MessageWriter<KeyboardInput>, window: Entity, key_code: KeyCode, key: &Key) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        keys.write(KeyboardInput {
            key_code,
            logical_key: key.clone(),
            state,
            text: None,
            repeat: false,
            window,
        });
    }
}
