Reviewed 2026-11-15 against HEAD 3437c0d8: partly done. Target SDK 37 and the cutout padding have shipped. Soft keyboard, store signing/AAB and the adaptive icon are still open.

# Outcome
The Android client can be published and fully used without a hardware keyboard.

# Already done (do not redo)
- Target SDK 37 + `ACCESS_LOCAL_NETWORK`: `packages/brawler-android/android/app/build.gradle` has `compileSdk 37` / `targetSdk 37`. The manifest declares the permission. `MainActivity.java` requests it at launch and on return from Settings (L27, L119-L160), with the denial dialog.
- Cutout: `MainActivity.keepSurfaceOutOfDisplayCutout()` (MainActivity.java L80-L95, BRL-0196) pads the content view by the `displayCutout` insets. The camera side shows a black strip. No shell or HUD UI sits under the cutout. What remains is optional: an in-app safe-area or full-bleed treatment. It is out of scope here unless the owner asks for it.

# Context (still true at HEAD)
- Soft keyboard: winit 0.30.13 / android-activity 0.6.1 (GameActivity, `androidx.games:games-activity:4.4.0`). Nothing calls `Window::set_ime_allowed` or bridges GameTextInput. A grep for `set_ime_allowed|Ime|GameTextInput` in `packages/` finds nothing. The text editors read `KeyboardInput` only:
  - Server Select address/name: `packages/brawler-client/src/flow/input.rs` `collect_flow_input` (L205), `edit_name_input` (L687), `insert_editor_text` (L604, address ASCII ≤255, name ≤64).
  - Map Maker map name: `packages/brawler-client/src/flow/screens/map_maker.rs` (~L1234, `MessageReader<KeyboardInput>`).
  - Workaround in use: the build-time prefill `just android-build HOST:PORT`, which becomes `BRAWLER_ANDROID_SERVER` → `packages/brawler-android/src/lib.rs` L13, plus persisted recents.
  - Paste: `arboard` is excluded on Android (`packages/brawler-client/Cargo.toml` L51-53), and `clipboard_text()` (flow/input.rs L590-L605) reports unavailable.
- Signing: `app/build.gradle` `buildTypes.release.signingConfig signingConfigs.debug`. There is no upload key and no AAB. `scripts/android.sh` runs only `gradle assembleRelease`.
- Icon: only legacy `res/mipmap-{m,h,xh,xxh,xxxh}dpi/ic_launcher.png`. There is no `mipmap-anydpi-v26`, adaptive XML or monochrome layer.

# Plan
1. Soft keyboard (largest item, likely its own commit):
   - When an editor gains editing focus, call `Window::set_ime_allowed(true)` (and set `ime_position`). Call it with false on commit or cancel. Verify that winit 0.30.13's Android backend shows the GameActivity soft input. If it does not, add a small JNI hook in `MainActivity` (`InputMethodManager.showSoftInput` / `hideSoftInputFromWindow`) driven from Rust, the same way `back.rs` / `nativeOnBackPressed` bridge Back.
   - Deliver committed text to the editors. Read `Ime::Commit` (Bevy `Ime` messages) and/or GameTextInput state, and route it through the existing `insert_editor_text` / map-maker insert so the validation and length limits stay shared. Backspace and Enter from the soft keyboard must map to the existing delete and commit paths.
   - Keep desktop behaviour unchanged. Add flow tests that feed `Ime::Commit` into the address, name and map-name editors.
2. Store signing / AAB: add a `release` `signingConfig` read from `keystore.properties` or env vars outside git (`.gitignore`). Keep the debug-signed fallback when the secrets are absent. Add `scripts/android.sh bundle` → `gradle bundleRelease` and a `just android-bundle` recipe. Document it in the README Android section.
3. Adaptive icon: `res/mipmap-anydpi-v26/ic_launcher.xml` with foreground/background/monochrome drawables made from original branding art, keeping the legacy PNGs for API < 26. minSdk is 31, so the adaptive icon always applies.

# Acceptance
- `cargo test -p brawler-client` passes, including the new IME commit tests for the address, name and map-name editors. `just android-build` still succeeds.
- `just android-bundle` (or an equivalent recipe) produces a signed `.aab` when the upload-key secrets are present, and fails with a clear message when they are absent. No secret is committed.
- **Pixel 8a device check:** tapping the Server Select address and name fields opens the on-screen keyboard. Typed text appears, Backspace and Done work, and the keyboard hides on commit or Back. A server can be joined by typing an address on a build made without the prefill.
- **Pixel 8a device check:** the launcher icon fills the launcher mask and reads clearly, including in themed (monochrome) icon mode.

# Implementation (2026-11-18, branch `worktree-agent-acc252c392493bc86`)

## Soft keyboard: research and chosen path
- winit 0.30.13 `platform_impl/android/mod.rs`: `set_ime_allowed(true)` calls `AndroidApp::show_soft_input(true)` (an *implicit* show), and `set_ime_cursor_area` / `set_ime_purpose` do nothing. Its input loop handles only `MotionEvent` and `KeyEvent`. `InputEvent::TextEvent` (GameTextInput state) and `InputEvent::TextAction` (IME Done) go to the `warn!("Unknown android_activity input event")` arm and are dropped. So winit never emits `Ime::Commit` on Android.
- games-activity 4.4.0 (`javap` of `gametextinput.InputConnection`): while the soft keyboard is active, `onKey` turns key presses, Backspace included, into edits of the GameTextInput editable, and single-line Enter into `sendEditorAction`. They never become native key events. Relying on winit `KeyboardInput` for soft Backspace or Enter would therefore be unreliable.
- **Chosen:** the Android shell drives GameTextInput directly through `android-activity` 0.6.1 (`bevy::android::ANDROID_APP`), plus two primitive-only JNI hooks in `MainActivity` in the `nativeOnBackPressed` style. The JNI approach (InputMethodManager from Java) was not needed for show/hide. `GameActivity_showSoftInput`, `hideSoftInput`, `setTextInputState` and `setImeEditorInfo` post to the UI thread in order (GameActivity.cpp `write_work`), so they are safe from the game thread. The show uses the explicit flag (0), not winit's implicit one.
- Client (`packages/brawler-client/src/text_entry.rs`, new):
  - `TypedText` SystemParam merges `KeyboardInput` presses and `Ime::Commit` into `TextEdit::{Backspace, Delete, Insert}`. The Server Select address/name editors, the brawler create/rename name editor, the Map Maker new/rename map-name editor and palette search all consume it. Committed text therefore goes through the existing `insert_editor_text` / `insert_name_text` / map-name bound paths.
  - `TextEntryFocus` (pub) is published in `Last` by `flow::input::publish_text_entry`. It carries the edited field (ServerAddress / PlayerName / BrawlerName / MapName), its text and its caret.
  - `TextEntry::edit_toward(ime_text)` is a pure, unit-tested diff. It turns the IME's text into Backspace presses (in each field's own deletion unit: bytes, graphemes or chars), an insertion, and a submit flag when a line break arrives.
- Android shell (`packages/brawler-android/src/soft_keyboard.rs`, new; system in `First`):
  - Focus starts: `set_ime_editor_info` (URI layout for the address; person-name + cap-words for names; cap-words for map names; suggestions off; action Done), then mirror the field text and UTF-16 caret into GameTextInput, then `show_soft_input(false)`.
  - Each frame: read `text_input_state()`, diff it against the mirrored text, then write the Backspace `KeyboardInput`s and one `Ime::Commit`. If the editor kept something different (a rejected character, a bound, a normalized name), its text is written back to the keyboard.
  - A mirror write is applied asynchronously on the UI thread. Diffs are suppressed until the keyboard reports the mirrored text, which avoids replaying stale text as deletions. After 30 frames the write is retried.
  - Focus ends: `hide_soft_input(false)`.
- `MainActivity.java` overrides:
  - `onEditorAction` → `nativeOnImeAction` → a synthetic Enter, so Done commits.
  - `onSoftwareKeyboardVisibilityChanged` → `nativeOnSoftKeyboardVisibilityChanged`. A hide after the keyboard was shown for the current field becomes `BackInput::request()`. **Decision:** Back with the keyboard open (consumed by the IME) or the keyboard's own hide control cancels the field exactly like Back/Escape: the address text is kept, the name reverts. This matches Back semantics everywhere else in the shell.
- Desktop is unchanged. Window IME is never enabled, so desktop gets no `Ime` messages and editing stays key-press driven.
- Scope note: the brawler name editors were included (same mechanism, needed for "keyboard-complete"). Palette search is keyboard-only (opened by `/`) and has no touch entry, so it gets no on-screen keyboard.
- Landscape: no `IME_FLAG_NO_EXTRACT_UI`. Gboard's landscape full-screen extract editor therefore shows the typed text even where the keyboard covers the field. If the owner prefers the inline keyboard, pass `ImeOptions::IME_FLAG_NO_FULLSCREEN | IMG_FLAG_NO_EXTRACT_UI` in `begin_editing`.

## Store signing / AAB
- `app/build.gradle`: the upload key comes from the gitignored `packages/brawler-android/android/keystore.properties` (storeFile/storePassword/keyAlias/keyPassword) or the `BRAWLER_UPLOAD_STORE_FILE` / `_STORE_PASSWORD` / `_KEY_ALIAS` / `_KEY_PASSWORD` env vars (env overrides the file).
  - Release signs with it when it is complete, otherwise with the debug key.
  - A `taskGraph.whenReady` guard fails `bundleRelease` with a clear message when the key is incomplete.
- `.gitignore` adds `keystore.properties`, `*.jks`, `*.keystore`.
- `scripts/android.sh bundle` runs `gradle bundleRelease`. It fails before the native build when neither the properties file nor `BRAWLER_UPLOAD_STORE_FILE` exists. `just android-bundle [HOST:PORT]` wraps it.
- README Android section documents the key setup, keytool command, Play App Signing and the versionCode bump.
- Follow-up not done: versionCode is still hand-edited (1). Bump it before the second Play upload.

## Adaptive icon
- `res/mipmap-anydpi-v26/ic_launcher.xml` has these layers:
  - background: `drawable/ic_launcher_background.xml`, a navy radial vector matching the legacy icon.
  - foreground: `mipmap-*dpi/ic_launcher_foreground.png`, the original `assets/brawler/ui/branding/pewpew-blitz-lockup.png` at 64 dp wide centred in 108 dp, with letters, blaster and spark inside the 66 dp safe circle.
  - monochrome: `mipmap-*dpi/ic_launcher_monochrome.png`, the lockup's white/yellow artwork as an alpha mask.
- Generated by `swift asset_src/android/build_launcher_icons.swift` (run from repo root). Legacy `ic_launcher.png` files are kept.
- The manifest has no `roundIcon`, so the adaptive icon covers round launchers.

## Verification
- `cargo test -p brawler-client --lib -- text_entry ime_commits`: 4 passed. Coverage:
  - `ime_commits_edit_the_server_address_and_name_within_their_bounds`: address append, ASCII rejection, name insertion, 64-byte bound, focus publish/clear.
  - `ime_commits_type_a_new_map_name_within_its_bound`: Backspace + commit, control filtering, byte bound.
  - `edit_toward` unit tests: caret, autocorrect replacement, after-caret guard, grapheme deletion, line-break submit, UTF-16 caret.
- `cargo ndk --target arm64-v8a --platform 31 clippy --locked -p brawler-android -- -D warnings`: clean.
- `just lint`: pass. `just test`: pass (24 suites, 2503 tests, 0 failed, including network and performance gates). Commits: `53407970` (soft keyboard), `1407d107` (signing/AAB/icon).
- `just android-bundle 192.168.31.47:5000` with a throwaway keystore under `target/brl-06pz/` (env vars, not committed) produced `app-release.aab` (42 MB). It contains `base/lib/arm64-v8a/libbrawler_android.so`, `keytool -printcert -jarfile` shows the throwaway signer, and the icon resources are present.
- `just android-bundle` without a key fails immediately with the missing-key message. `gradle bundleRelease` without a key fails with the Gradle message.
- `just android-build 192.168.31.47:5000` (no key) succeeds. `apksigner verify --print-certs` shows `CN=Android Debug`, which confirms the fallback.
- Not run: anything on the device. adb and the phone were reserved for another agent.

## Device-check plan (Pixel 8a)
Build an APK **without** the prefill (`just android-install`, no address) so the address must be typed. Start `BRAWLER_ROUTED_BIND=0.0.0.0:5000 just server` on the Mac.
1. Server Select: tap the address field.
   - Expect: the keyboard opens with a URL layout, possibly as Gboard's landscape full-screen editor.
   - Type `192.168.31.47:5000`. Expect: the field shows the text.
   - Backspace twice and retype. Expect: the field follows.
   - Press Done. Expect: the keyboard hides and editing ends.
   - Connect. Expect: the join succeeds.
2. Tap the name field and type a name with an accent (e.g. `Zoë`).
   - Done: the name commits and persists.
   - Re-open, type, then press Back. Expect: the keyboard hides, the name reverts, and the app stays on Server Select.
   - A second Back then backgrounds the app.
3. Address field: type a non-ASCII character (e.g. `é` from a long-press). Expect: the field rejects it with the inline error, and the keyboard text snaps back to the field's value.
4. Brawlers → Create Brawler / rename in Details. The keyboard opens for the name field, Done confirms, Back cancels.
5. Map Maker library → New Map / Rename. The keyboard opens with the name prefilled, Done confirms the map, Back cancels.
6. Switching directly between the address and name fields keeps the keyboard up and edits the right field.
7. Launcher: the adaptive icon fills the mask on the home screen. With Wallpaper & style → Themed icons on, the monochrome "PewPew Blitz" artwork reads clearly.
8. Logcat if something fails: `adb logcat -s gti.InputConnection GameActivity RustStdoutStderr`. GameTextInput logs every key and state change.

Known limits: the 30-frame mirror-settle window means a keystroke landing in the same frame as a rejected-character resync can be dropped. Composition-heavy IMEs (CJK) are untested; suggestions are off.

## Rebase onto main (2026-11-18)
Rebased onto `main` 40fc1563. The only conflict was in `flow/screens/map_maker.rs`: BRL-yrys `KeyedMapNameCommit` sat next to the new `edit_map_name` helper. Kept both, and added `Ime` to the new BRL-yrys map-library test app. The new commits are `34f42cd1` (soft keyboard) and `2da726b3` (signing/AAB/icon). After the rebase: `just lint` passes, `just test` passes (24 suites, 2572 tests, 0 failed), and Android clippy (`-D warnings`) is clean. The AAB was not rebuilt.

## Device fix: blank landscape extract editor and EXECUTE key (2026-11-18, commit `4df8736f`)

**Owner device check (main c6e18e2b).** These worked:
- The URL layout, and the field text pre-filled in the keyboard.
- Backspace and digits.
- Done commits and hides the keyboard.
- Back cancels and reverts the name.
- Typing an address and joining the server.
- The adaptive icon.

**Bug:** in landscape, Gboard's full-screen extract editor was blank, so the player typed blind (owner screenshots `/Users/boyd/wip/brawler/target/brl-06pz/s2.png` and `s3.png`). The action key also read "EXECUTE".

**Decision: option (b), no extract UI, plus a game-drawn strip.** Option (a) would need GameTextInput's `getExtractedText` and `updateExtractedText` support, which we do not own. Option (b) is small and does not depend on any one keyboard app.
- `soft_keyboard.rs`: the keyboard opens with `IME_FLAG_NO_FULLSCREEN | IMG_FLAG_NO_EXTRACT_UI`. The Enter key's action Done is now set in the imeOptions action bits through `ImeOptions::set_action`. Before this change only `actionId` was set, and the keyboard reads the action from imeOptions. On the device, `dumpsys input_method` now shows `imeOptions=0x12000006` and the action key is a check mark.
- The shell sets `TextEntryFocus::set_on_screen_keyboard(true)` while a field is being edited.
- `text_entry.rs`: `present_text_entry_strip` (PostUpdate, before `UiSystems::Prepare`) shows the field label and its text with a caret in an INK strip across the top of the screen. That strip sits in the area the landscape keyboard leaves uncovered, so fields that the keyboard hides (the name at y≈476, Map Maker dialogs) stay readable. Desktop never sets the flag, so it never shows the strip.
- New test: `the_strip_shows_the_field_only_while_an_on_screen_keyboard_edits_it`.

**Device verification** (Pixel 8a, `just android-install` with no address prefill; screenshots in `target/brl-06pz/` of the BRL-06pz worktree):

| Field | Screenshots | Result |
|---|---|---|
| Server address | `after-1-address-open.png`, `after-2-address-typed.png`, `after-3-address-done.png` | The keyboard opens inline below the game. The strip reads `Server address: 192.168.31.47:5000\|`. Typing `0` and two Backspaces leaves `…:500` in both the strip and the field. Done (check mark) commits and hides the keyboard. |
| Player name | `after-4-name-typed.png`, `after-5-name-back.png` | The field sits under the keyboard; the strip shows `Your name: Brawler-42DDX\|`. Back hides the keyboard and reverts the name to `Brawler-42DD`. |
| Join | `after-6-dashboard.png` | Connect joined the port-5000 server. |
| Map Maker, Create Map name | `after-10-mapname-open.png`, `after-11-mapname-typed.png`, `after-12-mapname-back.png` | The strip shows `Map name: Untitled Map Z\|`, after one Backspace removed `1` and `Z` was typed. Back cancels the dialog. |

`just lint` passes. `just test` passes: 24 suites, 2574 tests, 0 failed.

Known limits:
- The strip covers the top ~50 logical px of the screen (titles) while the keyboard is up.
- Gboard still shows word suggestions for name fields, even though NO_SUGGESTIONS is set.
