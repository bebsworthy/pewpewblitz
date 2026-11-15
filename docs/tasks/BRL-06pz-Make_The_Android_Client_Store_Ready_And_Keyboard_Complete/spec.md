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
