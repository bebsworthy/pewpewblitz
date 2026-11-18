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


# Result (2026-11-18, size only; Pixel 8a not connected)

## Sizes (`scripts/android.sh build 192.168.31.47:5000`, arm64-v8a, stripped .so; cumulative)
| Build | APK bytes | .so bytes | .so vs HEAD | .so gzip -6 | .text / .eh_frame+hdr / .rodata / .data.rel.ro (MB) |
|---|---|---|---|---|---|
| HEAD `c59dac6e` (thin LTO, 16 CGU) | 126,749,444 | 115,700,680 | - | 36.5 MB | 84.1 / 16.5 / 6.9 / 7.7 |
| a. + `codegen-units = 1` | 96,590,756 | 85,541,992 | -26.1 % | 29.8 MB | 64.4 / 10.7 / 5.6 / 4.4 |
| **a+b. + `lto = "fat"` (applied)** | **89,020,372** | **77,971,608** | **-32.6 %** | 30.5 MB | 59.7 / 8.6 / 5.2 / 4.0 |
| a+b+c. + `opt-level = "s"` on 12 cold crates | 88,577,252 | 77,528,488 | -0.57 % vs a+b | 30.4 MB | 59.3 / 8.7 / 5.1 / 4.0 |
| a+b+d. feature trim (.so only) | - | 77,684,632 | -0.37 % vs a+b | 30.4 MB | 59.5 / 8.6 / 5.1 / 4.0 |
- APK total -29.8 % (126.7 -> 89.0 MB). The .so is stored uncompressed in the APK (AGP default
  page-aligned packaging), so the APK is .so + ~11 MB; a compressed download is ~30 MB + assets.
- c crates: naga, naga_oil, gltf, gltf-json, serde_json, ron, image, png, ktx2, ruzstd,
  regex-automata, regex-syntax. Under fat LTO with one CGU most of their code is generic
  instantiations codegened in the calling crates, so the per-package opt-level barely reaches it.
  0.44 MB is not worth any frame-time risk: **not applied**.
- d measured removing `bevy/reflect_auto_register`, `bevy/sysinfo_plugin` (with
  `SystemInformationDiagnosticsPlugin` temporarily commented out) and `bevy/bevy_scene` (unused;
  only `world_serialization` is). `bevy/serialize` stays on through avian2d regardless, and
  `bevy_reflect`'s default features keep `auto_register_inventory`. Gamepad/gilrs not touched.
  0.29 MB: **not worth a behaviour change, not applied, no owner question filed**. The temporary
  edits and the Cargo.lock change were reverted.
- e (assets) not pursued: native code dominates.

## Largest crates at HEAD (unstripped `CARGO_PROFILE_ANDROID_STRIP=false` build, `llvm-nm -C
## --print-size`, grouped by leading path; generic instantiations count toward the type's crate)
92.1 MB of symbols: bevy_ecs 20.9 MB (22.7 %, mostly system/query/command monomorphizations for
game types), core 10.8, bevy_reflect 9.4, erased_serde 4.7, anon consts 3.9, ron 3.0, hashbrown
2.1, alloc 1.9, std 1.7, bevy_asset 1.4, brawler_client 1.2, bevy_hanabi 1.2, indexmap 1.1,
bevy_render 1.0, naga 1.0, bevy_pbr 1.0, bevy_input 1.0, bevy_color 1.0, avian2d 1.0, serde_core 0.9,
brawler 0.9. Files: `target/5k0g/crates-head.txt`, `target/5k0g/nm-head.txt.gz`.

## Applied
`[profile.android]`: `lto = "fat"`, `codegen-units = 1` (root `Cargo.toml`; comment updated).
Nothing else changed; desktop, test and server profiles are untouched. Both settings usually help
speed (whole-program inlining) rather than hurt it. Build time for a full android relink rose
from ~2.5 min (thin, cached deps) to ~9 min.

## Verification
- Final APK `packages/brawler-android/android/app/build/outputs/apk/release/app-release.apk`
  (89,020,372 B) built from the final tree; .so byte-identical in size to the a+b run and contains
  the `192.168.31.47:5000` prefill. Logs: `target/5k0g/build-*.log`.
- `just lint`: pass (`target/5k0g/lint.log`). `cargo test -p brawler-client`: 834 passed, 0 failed
  (`target/5k0g/test-brawler-client.log`).
- Not verified: frame rate on device (no phone connected).

## Owner playtest: Pixel 8a frame-rate check (BRL-ynt5 scenario)
1. Install the final APK: `just android-install 192.168.31.47:5000` (or `adb install -r
   packages/brawler-android/android/app/build/outputs/apk/release/app-release.apk`), with
   `BRAWLER_ROUTED_BIND=0.0.0.0:5000 just server` on the Mac.
2. Display at 60 Hz (Smooth Display off). Settings > Display > Graphics quality = Auto (expect
   `AUTO (MEDIUM)`; logcat `render quality tier selected tier="MEDIUM" preference="AUTO"`).
3. Practice Wipeout 3v3 on Verdant Crossfire with bots; move and fire during the window.
4. `adb shell dumpsys SurfaceFlinger --timestats -clear -enable`, play 25-30 s, then
   `adb shell dumpsys SurfaceFlinger --timestats -dump > timestats-5k0g.txt` and
   `adb shell dumpsys SurfaceFlinger --timestats -disable`. Serialize adb calls (BRL-ynt5 lesson).
5. Pass: game-layer present median 16 ms (60 FPS) and average FPS not below the BRL-ynt5 HEAD
   numbers (Auto: median 16 / p95 33 ms, average 48-52 FPS; thermal status stays 1). If it
   regresses, revert to `lto = "thin"` and retry with `codegen-units = 1` only (-26 %).

## Follow-up observations (not in scope, no ticket filed)
- `.eh_frame` + `.eh_frame_hdr` are still 8.6 MB with `panic = "abort"`; dropping them needs
  `-C force-unwind-tables=no` in rustflags (not a profile key) and costs backtraces.
- `packaging.jniLibs.useLegacyPackaging = true` would compress the .so in the APK (~89 -> ~41 MB
  sideload) at the cost of install-time extraction and double on-device storage.
