# Outcome

The Bevy client runs on Android as a first-class build target, without disturbing desktop or server builds.

# Context

- The client is only a `[[bin]]` with a large CLI parser (`src/bin/client.rs`). Android needs a `cdylib` with `#[bevy_main]`.
- Bevy 0.19 no longer enables `android-game-activity` by default.
- `arboard` has no Android backend (used at `src/client/flow/input.rs:270`).
- `directories::ProjectDirs` falls through to XDG paths that will not resolve on Android (`connection_persistence.rs`, `settings/persistence.rs`, `evidence_capture.rs`).
- Reference shell: `references/bevy/examples/mobile/android_example` (GameActivity, Gradle, cargo-ndk).
- Play Store requires target SDK 36 and 16 KB page alignment (NDK r28+).

# Scope

- A thin `packages/brawler-android` crate (`cdylib`, Bevy `android-game-activity` + `touch`) that calls the shared client app builder.
- A Gradle project: target SDK 36, min SDK 31, `arm64-v8a`, and `assets.srcDirs` pointed at `assets/`.
- Gate `arboard` and gamepad features on Android. Use `AndroidApp::internal_data_path()` for settings and connection persistence.
- Server address and connection configuration without CLI arguments.
- Lifecycle: on `AppLifecycle::Suspended`, pause audio. Define resume behavior (reconnect or return to Dashboard).
- Reactive `WinitSettings` in menus, continuous in matches.
- A `just android` recipe and README instructions.
- Release profile tuning (thin LTO, strip, `panic = "abort"`) for the Android build.

# Out of scope

Touch gameplay controls, mobile quality tiers, mobile network resilience (separate tickets), and store publishing.

# Acceptance criteria

- A clean checkout builds an APK/AAB with the documented command.
- The app installs on a physical device, reaches the Dashboard, and joins a Practice match against a LAN routed server.
- Background/foreground cycles do not crash or lose audio permanently.
- Desktop client, server, and `just ci` builds are unchanged.

# Verification

- A device run log.
- `just check`, `just lint`, `just test`.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.

# User decisions (2026-11-13)
- Target device: the owner's Google Pixel 10 on the latest Android updates (research its SoC/GPU, Android version, Vulkan support, and ABI to pick API level and backend).
- Toolchain: the owner approved installing the Android toolchain on this Mac (Android command-line tools/SDK platform, build-tools, platform-tools/adb, NDK via sdkmanager; the needed JDK if missing; `rustup target add` for the Android target; `cargo install cargo-ndk` or an equivalent packager). Download only from official sources (Google/Android, rustup, crates.io, Homebrew).

# Research and design (2026-11-13, before implementation)

## Target device research (Google Pixel 10, latest updates)
- SoC: Google Tensor G5 (TSMC 3 nm). ABI: `arm64-v8a` only (64-bit-only device); no 32-bit or x86 builds.
- GPU: Imagination PowerVR **DXT-48-1536** (Tensor G5 moved from Arm Mali to Imagination). Launch driver
  (24.x) exposed Vulkan 1.3; Google shipped the newer Imagination driver line (reported as 25.1 / Vulkan
  1.4) starting with Android 16 QPR3. Exact current driver build is unconfirmed — record it from the
  device log (`adb logcat | grep -i "AdapterInfo\|vulkan"`) at playtest. Launch-era press reported
  weak/old-driver GPU performance; treat Pixel 10 GPU throughput as a BRL-0161 quality-tier input.
  Sources: phonearena.com/news/pixel-10-gpu-update-confirmed-improvements-still-missing_id175111,
  notebookcheck.net (Pixel 10 GPU driver update), wccftech.com/pixel-10s-tensor-g5-soc-gets-a-performance-boost.
- OS: Android 17 (API 37) is the current stable release (released 16 Jun 2026; QPR1 Sep 2026) and is the
  latest update for Pixel 10 (source: developer.android.com/blog/posts/android-17-is-here).
- Graphics backend: wgpu **Vulkan** (Bevy's Android default; no GL fallback configured). Known risk:
  PowerVR drivers have historically had wgpu quirks (compute/storage-buffer limits) — bevy_hanabi GPU
  particles are the most likely failure point; any device failure is recorded and routed to BRL-0161.

## Chosen SDK levels
- `minSdk 31` (Android 12; keeps the spec value, Vulkan 1.1 baseline, no need to go lower for one device).
- `targetSdk 36` / `compileSdk 36`. **Not 37**: apps targeting API 37 have local-network access
  blocked by default and need the `ACCESS_LOCAL_NETWORK` *runtime* permission, which would require Java
  permission-request code just to reach the Mac's LAN server. API 36 is the current Play minimum
  (Aug 31 2026), so it remains store-valid. Moving to 37 is deferred work (needs a permission flow).
- NDK r29 (`29.0.14206865`; r28+ = 16 KB page alignment by default), build-tools 36.1.0, platform-tools.

## Packaging decision: Gradle + GameActivity + cargo-ndk (the spec's plan, Bevy 0.19's reference shell)
- Verified against Bevy 0.19.1 in ~/.cargo/registry: `android-game-activity` is an explicit `bevy`
  feature (not default); `#[bevy_main]` generates `android_main` and sets `bevy::android::ANDROID_APP`;
  Bevy's `AndroidAssetReader` reads from the APK `AssetManager` (AssetPlugin `file_path` is ignored on
  Android). `android-activity 0.6.1` (Bevy 0.19's) requires `androidx.games:games-activity:4.4.0`
  exactly. cpal 0.17 uses AAudio via `ndk` (no Oboe/C++), and GameActivity glue links `c++_static`,
  so `android_shared_stdcxx` is **not** needed. gilrs-core has a dummy backend on Android, so the
  gamepad feature compiles (no gamepads; acceptable). `bevy/touch` must be enabled so `Touches` feeds
  `bevy_ui` `Interaction` and the shell's press feedback (taps work as UI clicks).
- Rejected: `cargo-apk`/`xbuild` (NativeActivity-oriented, unmaintained/less aligned with Bevy's
  GameActivity default and lifecycle fixes).
- Layout: `packages/brawler-android/` = thin workspace crate (`cdylib`, `libbrawler_android.so`) that
  depends on `brawler` with `default-features = false, features = ["client"]` (the BRL-0165 player
  configuration; automation never reaches the APK) plus `bevy` features `android-game-activity` and
  `touch` (no new third-party crates). `packages/brawler-android/android/` = Gradle project
  (AGP 9.5.1, Gradle from Homebrew, JDK 21) with a 10-line `MainActivity extends GameActivity`
  (loads the library, immersive fullscreen). Manifest: label "PewPew Blitz", existing app icon
  (`assets/brawler/ui/branding`), `sensorLandscape`, `INTERNET` permission, `configChanges` per Bevy's
  example so rotation/keyboard changes do not recreate the activity.
- Assets: Gradle `assets.srcDirs` = repo `assets/` (≈6.8 MB). Gameplay content is already build-embedded.
- Rust profile `[profile.android]` (inherits release; thin LTO, `strip`, `panic = "abort"`), used only by
  the Android recipe; desktop profiles unchanged.
- Signing: the APK is built as Gradle `release` but signed with the local debug keystore
  (`~/.android/debug.keystore`, auto-generated by AGP) — installable via `adb`, not store-signed.
  Store/upload signing and AAB are deferred.
- Recipes: `just android-build [server]` (cargo-ndk → `jniLibs/arm64-v8a`, then `gradle assembleRelease`)
  and `just android-install [server]` (build + `adb install -r` + launch).

## Platform seams in the shared client (small, cfg-gated)
- Player-local files: on Android, settings, connections, and map drafts live in
  `AndroidApp::internal_data_path()`; F12 captures (kept in player builds per BRL-0165 q-000020) go to
  `external_data_path()/captures` so `adb pull` can retrieve them. Desktop paths unchanged.
- `arboard` becomes a non-Android target dependency; Ctrl/Cmd+V paste reports "unavailable" on Android.
- Server address: no CLI on Android. `just android-build server=HOST:PORT` bakes an optional editable
  prefill (`BRAWLER_ANDROID_SERVER`, compile-time, only in the thin crate); otherwise the shell's normal
  recents/`127.0.0.1:5000` fallback applies and recents persist after the first connect. The Mac
  serves the LAN with `BRAWLER_ROUTED_BIND=0.0.0.0:5000 just server` (the product lobby connection
  already binds a wildcard local socket). Soft-keyboard text entry is not wired by winit 0.30 for
  GameActivity (no `set_ime_allowed` caller, no GameTextInput bridge) → deferred.
- The product-shell `ClientNetworkConfig` construction moves into the library
  (`ClientNetworkConfig::product_shell`) so the desktop player binary and the Android entry share it.
- Lifecycle: on `AppLifecycle::Suspended` pause every playing audio sink and resume exactly those on
  `Running`. Resume behavior: the session is not kept alive in the background (BRL-0162); the existing
  timeout/disconnect handling returns the player to the shell. Rendering surface recreation is Bevy's.
- `WinitSettings` reactive-in-menus is deferred to BRL-0161 (power/quality tiers) to keep desktop
  behavior unchanged here.

# Toolchain installed (2026-11-13)
- Homebrew: `openjdk@21` 21.0.13 (keg-only, 331 MB), `gradle` 9.9.1 (208 MB) which pulled `openjdk` 27.0.1
  (374 MB) and `gradle-completion` 9.8.0, plus dependencies installed/upgraded the same run: freetype
  2.15.0, giflib 6.1.3, harfbuzz 15.1.0, jpeg-turbo 3.2.1, libpng 1.6.59, little-cms2 2.20. (~0.95 GB)
- Android command-line tools 23.0 (`commandlinetools-mac_arm64-16111833_latest.zip` from
  dl.google.com, SHA-1 `ad03dc49…e830` verified against Google's repository2-3.xml) unpacked to
  `$ANDROID_HOME/cmdline-tools/latest` (172 MB).
- `rustup target add aarch64-linux-android` for toolchain 1.95.0 (127 MB rust-std).
- `cargo install cargo-ndk --locked` → cargo-ndk 4.1.2 (crates.io, ~5 MB).
- **Not yet installed: `platform-tools`, `platforms;android-36`, `build-tools;36.1.0`,
  `ndk;29.0.14206865`.** sdkmanager requires accepting the Android SDK License Agreement; the agent did
  not accept terms on the owner's behalf (question q-000021). Expected additional disk ≈ 2.5–3 GB (NDK
  r29 zip alone is 1.05 GB). Gradle will also download AGP 9.5.1, games-activity 4.4.0, appcompat
  1.7.1 from Google Maven (dl.google.com) and AGP's transitive dependencies from Maven Central on the
  first build.

# Implementation (2026-11-13)
- `packages/brawler-android/` (new workspace member): `Cargo.toml` (`cdylib` `brawler_android`;
  deps `brawler` [player `client` feature only] + `bevy` features `android-game-activity`, `touch`; no
  new third-party crates — Cargo.lock only gains the member and android-activity's already-locked
  `simd_cesu8` edge), `src/lib.rs` (`#[bevy_main]` → `ClientNetworkConfig::product_shell(...)` →
  `build_app_with_config`; optional compile-time `BRAWLER_ANDROID_SERVER` prefill).
- `packages/brawler-android/android/`: Gradle project (settings/build/gradle.properties, `app/build.gradle`
  min 31 / target+compile 36 / arm64-v8a / assets → repo `assets/`, release signed with debug key),
  `AndroidManifest.xml` (label "PewPew Blitz", `INTERNET`, Vulkan 1.1 feature, `sensorLandscape`,
  `appCategory=game`, Bevy's `configChanges`), `MainActivity.java` (GameActivity + immersive system bars),
  `res/mipmap-*/ic_launcher.png` (lockup on #0B2A6F, generated with `sips --resampleWidth` 92% +
  `--padToHeightWidth`), `.gitignore` (build outputs, jniLibs).
- `scripts/android.sh` + `just android-build [HOST:PORT]` / `just android-install [HOST:PORT]`
  (prerequisite checks; cargo-ndk `--platform 31 --profile android` into jniLibs; `gradle assembleRelease`;
  install: `adb install -r` + `am start`).
- Root `Cargo.toml`: workspace member; `arboard` moved to a `cfg(not(target_os = "android"))` target
  dependency (still behind `client`); new `[profile.android]` (inherits release, thin LTO, strip,
  `panic = "abort"`).
- Shared client: `src/client/platform_dirs.rs` (desktop ProjectDirs config dir unchanged; Android
  internal data path; Android captures in external app dir), used by settings, connections, map drafts,
  and evidence capture; `src/client/flow/input.rs` `clipboard_text()` (Android: inline "unavailable");
  `src/client/audio/lifecycle.rs` pauses playing `AudioSink`/`SpatialAudioSink` on
  `AppLifecycle::Suspended` and resumes exactly those (marker component) on `Running`;
  `src/config.rs` gains `random_nonzero_client_id` and `ClientNetworkConfig::product_shell`, which the
  desktop player binary (`src/bin/client.rs`) now uses (same resulting config); README "Android client".

# Verification (2026-11-13)
- `just lint` (real ripgrep 15.0.0 first on PATH): **pass** — includes player-client automation exclusion
  and server feature isolation.
- `just test`: **pass** (23 suites `test result: ok`, 0 failed).
- `just e2e 2`: **pass** (1 exact 1v1 roster reached Active).
- Desktop `just run 1` with scratch `BRAWLER_DEV_DATA_DIR` and bind 127.0.0.1:5093: window opened
  (Apple M6 adapter, native profile), no warnings/errors, connections file written to the scratch dir.
- LAN path rehearsal for the phone: supervisor bound `0.0.0.0:5091` (`BRAWLER_ROUTED_BIND`), headless
  automation client `--server 192.168.31.47:5091 --local-addr 0.0.0.0:0 --product-practice-smoke
  --product-game-type wipeout-1v1` reached authoritative Active, exit 0. (Without `--local-addr` the
  automation client binds 127.0.0.1 and times out against a LAN IP; the product shell — and therefore
  Android — already binds a wildcard socket.)
- **Not yet done (blocked on q-000021):** Android cross-compile, APK build/size, 16 KB ELF alignment
  check (`llvm-readelf -l` LOAD align 0x4000), device install/run log, background/foreground cycle.

# Owner steps: install and connect (Pixel 10)
1. Phone: Settings → About phone → tap *Build number* 7× → Settings → System → Developer options →
   enable *USB debugging*; connect by USB-C and accept the Mac's RSA prompt (`adb devices` lists it).
2. Mac (after q-000021 and the sdkmanager install): find the LAN IP (`ipconfig getifaddr en0`), start
   `BRAWLER_ROUTED_BIND=0.0.0.0:5000 just server` (allow incoming connections if macOS asks), then
   `just android-install <MAC_IP>:5000`. The APK is
   `packages/brawler-android/android/app/build/outputs/apk/release/app-release.apk`; a manual install
   is `~/Library/Android/sdk/platform-tools/adb install -r <apk>`.
3. Phone and Mac must be on the same Wi-Fi without client isolation. Logs: `adb logcat -s RustStdoutStderr:* brawler:* bevy_render:*`
   (or `adb logcat | grep -i "brawler\|bevy\|wgpu"`).

# Playtest request (once the APK is built)
Tap through Server Select (prefilled address) → Connect → Dashboard → create a brawler → Practice
Wipeout 1v1. Observe: launch time; icon/name "PewPew Blitz"; landscape lock; taps register on shell
buttons; Dashboard reached; Practice match loads and renders (touch movement/aim is BRL-0160, so the
fighter will not move by touch); audio plays; press Home during the Dashboard and in a match, wait
10–60 s, return — no crash, audio resumes; note whether the session survived (BRL-0162 owns survival).
Report the `AdapterInfo` logcat line, any visual corruption (particles), and frame smoothness.

# Learn-from-errors (so far)
- Assumed the earlier "toolchain installs approved" decision covered license acceptance; it does not —
  accepting SDK terms is a separate explicit consent. Prevention: list license/terms steps as their own
  question when planning any SDK install.
- First LAN rehearsal timed out because the automation client binds 127.0.0.1 by default; check the
  socket bind family/interface before blaming firewall or routing.
- `brew cat` silently enabled Homebrew developer mode; it was turned back off (`brew developer off`).
  Prefer `brew info --json` for inspection.

# Deferred
- BRL-0195 (related, parent BRL-0171): soft-keyboard text entry, target SDK 37 + ACCESS_LOCAL_NETWORK,
  store signing/AAB, adaptive icon, display-cutout safe areas.
- BRL-0161: reactive WinitSettings in menus; Pixel 10 PowerVR GPU checks (appended to its spec).

# User decision (2026-11-13): target SDK
- The owner chose "Target 37 now": targetSdk/compileSdk 37 with the ACCESS_LOCAL_NETWORK runtime-permission flow in this ticket (moved out of BRL-0195's deferred list). Debug-keystore signing for adb installs remains acceptable; store signing/AAB stay in BRL-0195.

# Update 2026-11-13 (owner decisions after q-000021)
- Owner answered q-000021 in chat: "I accept; you run it". The agent's attempt to run
  `yes | sdkmanager --licenses` was **denied by the Claude Code permission system** (auto-mode
  classifier), so the licenses are still unaccepted and the SDK packages are still not installed. The
  owner must either run it in their own terminal or add a permission rule allowing it:
  `JAVA_HOME=/opt/homebrew/opt/openjdk@21 yes | ~/Library/Android/sdk/cmdline-tools/latest/bin/sdkmanager --licenses`
  then `… sdkmanager "platform-tools" "platforms;android-37.0" "build-tools;37.0.0" "ndk;29.0.14206865"`.
  q-000021 stays open until the licenses are accepted and the install is done.
- Owner chose **target SDK 37 now** (supersedes the "Chosen SDK levels" target-36 rationale above):
  `compileSdk 37`, `targetSdk 37`, `minSdk 31`; SDK platform `android-37.0`, build-tools 37.0.0.
  `ACCESS_LOCAL_NETWORK` is declared in the manifest, and `MainActivity` (Java only — no JNI, no new
  crates) requests it in `onResume` on API 37+ devices, i.e. at launch and on return from Settings,
  before the player can press Connect. On denial it shows a non-cancelable dialog "Local network access
  needed" explaining that servers on the player's Wi-Fi need it, with **Try again** (re-prompts while
  Android still allows the prompt; otherwise **Open settings** to the app-details page) and **Not now**
  (asks again next launch). Granting in Settings dismisses the dialog on resume. Removed from BRL-0195.
  Known limitation: the Rust shell does not see the permission state; with "Not now" a LAN connect
  shows the existing connection-timeout error.
- `scripts/android.sh` checks for platform android-37.0; README updated.

# Verification rerun after the API-37 changes (2026-11-13)
- `just lint` **pass**, `just test` **pass** (23 suites ok, 0 failed), `just e2e 2` **pass** (1v1 roster
  reached Active). Rust sources were unchanged by this update (Java/Gradle/manifest/script/README only).
- Still blocked by the denied `sdkmanager --licenses` run: no NDK/platform/adb, so no Android compile,
  APK, 16 KB alignment check, `adb devices`, or device smoke yet.
