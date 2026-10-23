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
