# Found (BRL-wr0s device check, Pixel 8a, 2026-11-16)
For about 2.2 s after a cold launch, an input `tap` never reaches the app. Right after the tap, the app gets `WindowFocusChanged -- 0`. `dumpsys input` then shows the focused window as `name=Embedded{}`, inputConfig=TRUSTED_OVERLAY, frame `[-1320,0][1080,1080]` (rotated), owner `com.google.android.gms.ui` (uid 10161). The IME starts input for `com.google.android.gms`. Screenshots taken right after show only the game UI, so the overlay looks invisible or transparent. Without a tap, the app keeps focus. Taps from 2.4 s on are delivered normally.

# Hypothesis
This is a Play Services game overlay (Pixel Game Dashboard / Play Games entry) attached because the manifest declares `android:appCategory="game"` (`packages/brawler-android/android/app/src/main/AndroidManifest.xml:11`).

# Plan
1. Confirm it: build a variant without `appCategory="game"` (or turn Game Dashboard off for the app in the phone's Game settings, which needs the owner because it is a phone setting) and repeat the timed taps.
2. If confirmed, the owner decides: keep the game category (and its dashboard), or drop it. Check whether a Game Mode / `android:isGame`-style setting is needed for Play Store categorization.

# Acceptance
- The cause is identified with evidence. If the category is dropped, a cold-launch tap at 1.8–2.2 s reaches the app (Pixel 8a).

# Device check 2026-11-19 (Pixel 8a, Android 17, BRL-fs5k experiment build of main caee2a43)
Method: two APKs that differ only in the manifest. `game` is the shipped manifest; `nogame` removes `android:appCategory="game"` (checked with `aapt2 dump xmltree`). Both carry a temporary `<profileable android:shell="true"/>` and the same native library. For each run: `am force-stop`, then one device-side shell `am start …; sleep D; input tap 1045 833` (the first saved-server row on Select Server), `dumpsys input` FocusedWindows 0.4 s after the tap, then the full logcat. "Reached the app" means the app kept window focus and started connecting (`socket bound` / `connecting to server` from the app's pid).

| tap at | `game` (2 runs each) | `nogame` (2 runs each) |
|---|---|---|
| 1.8 s | 2/2 swallowed: app gets `WindowFocusChanged -- 0`, focused window `Embedded{}` | 2/2 app keeps focus; run 2 connected, run 1 tapped before the row existed (no connect, focus kept) |
| 2.0 s | 2/2 swallowed (same signature) | 2/2 connected |
| 2.2 s | 2/2 swallowed (same signature) | 2/2 connected |
| 2.6 s | 1/1 connected | 2/2 connected |

What the category switches on (logcat, `game` launch only; none of these lines appear for `nogame`):
- Play Services `GameService` (com.google.android.gms): `onGameStarted … requesting createGameSession` about 0.3 s after launch. That game session is the `Embedded{}` TRUSTED_OVERLAY window that takes focus on the first tap in its first ~2.4 s.
- `GameManagerService: setDefaultFrameRateOverride : <uid>, 60.0` and `Game power mode ON (only games in foreground)`.

**Cause confirmed:** the overlay is the Play Services game session (Game Dashboard entry) that Android attaches because the manifest declares `android:appCategory="game"`. Without the category, cold-launch taps at 1.8–2.2 s reach the app. The shipped manifest is unchanged; keep vs drop is an owner decision (Ticket question on this ticket). Evidence: session scratchpad `gv67/` (`results.txt`, per-run `*-input.txt`, `*-logcat.txt`, screenshots).
