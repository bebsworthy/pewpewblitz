Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec refreshed. Nothing has been done on size. The APK and .so have grown since the spec was written.

# Outcome
A smaller Android download and native library, measured on the Pixel 8a, with no frame-rate regression.

# Context (verified at HEAD)
- Last local build (2026-11-15 11:49): `app-release.apk` 126.8 MB and `jniLibs/arm64-v8a/libbrawler_android.so` 115.2 MB stripped. These were 120 MB / 113.9 MB when the spec was written. `assets/` is only 6.9 MB, so nearly all the size is native code.
- Profile: root `Cargo.toml` L171-177, `[profile.android] inherits = "release"`, `lto = "thin"`, `strip = true`, `panic = "abort"`. There is no `[profile.release]` override, so opt-level is 3 and codegen-units 16. It is built by `scripts/android.sh` (`cargo ndk --target arm64-v8a --platform 31 ... --profile android`).
- Bevy features: the root `bevy-client` feature (Cargo.toml L71-101) enables, among others, `reflect_auto_register`, `serialize`, `bevy_gilrs`/`gamepad`, `sysinfo_plugin`, `default_font`, `bevy_scene`, `tonemapping_luts` + `zstd_rust`, `system_font_discovery`. Add `bevy_hanabi` (`3d`) from `packages/brawler-client/Cargo.toml` L40. `packages/brawler-android/Cargo.toml` adds `android-game-activity` and `touch`.
- Frame time is sensitive to code-gen. `packages/brawler-android/src/frame_pacing.rs` runs 2 compute threads plus an ADPF hint session (BRL-ynt5 / old ID BRL-0161). Each experiment is a full ~7-10 min rebuild.

# Plan
1. Baseline: `just android-build` at HEAD. Record APK and .so sizes. Run `cargo bloat --profile android --target aarch64-linux-android -p brawler-android --crates` (or `-n 50`), or a `twiggy`/`size` breakdown, to find the largest crates.
2. Candidates, one at a time, each measured on size alone first:
   a. `codegen-units = 1` in `[profile.android]`.
   b. `lto = "fat"`.
   c. `opt-level = "s"` for cold crates only, through `[profile.android.package.<crate>]` (for example naga, wgpu-core's shader tooling, gltf/serde_json, image decoders, ron, parley/swash), keeping hot gameplay/render crates at 3.
   d. An Android-only feature set that drops what the player build does not use (check `reflect_auto_register`, `sysinfo_plugin`, `bevy_scene`, `gilrs` if gamepads are not a phone target). This needs a `brawler-android` feature list instead of inheriting all of `bevy-client`. Get the owner's OK before removing gamepad support.
   e. Assets: only if a measurement shows a gain (6.9 MB of assets in total).
3. For each candidate that saves ≥ 5 % size, run the frame-rate check below. Keep only the ones with no regression.

# Acceptance
- A table of APK and .so sizes before and after each candidate, recorded on this ticket.
- `cargo test -p brawler-client` and the desktop build are unaffected (the profile changes are Android-only).
- **Pixel 8a device check:** Practice 3v3 at Auto (resolves to Medium on Android) keeps a 60 FPS median per `adb shell dumpsys SurfaceFlinger --timestats` with the BRL-ynt5 scenario, compared with the HEAD baseline.
