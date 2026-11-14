# Outcome
The Android client can be published and fully used without a hardware keyboard.

# Context (deferred from BRL-zj53)
- Soft keyboard: winit 0.30 never shows the IME for GameActivity unless `set_ime_allowed` is called, and
  it does not bridge GameTextInput text events, so the Server Select address and name editors cannot be
  typed into with the on-screen keyboard. BRL-zj53 works around this with a build-time server prefill
  (`just android-build HOST:PORT`) plus persisted recents. Android paste (`arboard` has no Android
  backend) reports unavailable.
- Target SDK: BRL-zj53 targets API 36 because API 37 blocks LAN access unless the app requests the
  `ACCESS_LOCAL_NETWORK` runtime permission (needs a Java/JNI permission flow).
- Signing: BRL-zj53 signs the release APK with the local debug keystore; no upload key, no AAB.
- Icon: BRL-zj53 ships a legacy (non-adaptive) launcher icon generated from the branding lockup with
  `sips`; it is small inside the launcher mask.
- Cutout/safe areas: the full-screen landscape surface may place shell UI under the Pixel camera cutout.

# Scope
- Show/hide the on-screen keyboard for the shell's text editors and deliver committed text to them.
- Target SDK 37 with an `ACCESS_LOCAL_NETWORK` request flow (or picker) and a clear denied state.
- Upload-key signing configuration (secrets outside git) and an AAB build recipe.
- Adaptive launcher icon (foreground/background/monochrome) from original art.
- Respect display-cutout safe insets in shell and HUD layout.

# Acceptance criteria
- A Pixel 10 user can enter a server address and names with the on-screen keyboard.
- The API-37 build connects to a LAN server after the permission prompt.
- `just` produces a signed AAB; the launcher icon reads clearly; no UI is hidden by the cutout.

# Update 2026-11-13: target SDK 37 moved into BRL-zj53
The owner chose "Target 37 now". BRL-zj53 now targets/compiles against API 37 and its `MainActivity`
requests `ACCESS_LOCAL_NETWORK` at launch (and on return from Settings), with a player-facing denial
dialog offering Try again / Open settings / Not now. Remove "Target SDK 37 with an ACCESS_LOCAL_NETWORK
request flow" and "The API-37 build connects to a LAN server after the permission prompt" from this
ticket's scope and acceptance criteria; the remaining items (soft keyboard, store signing/AAB, adaptive
icon, cutout safe areas) stay here.
