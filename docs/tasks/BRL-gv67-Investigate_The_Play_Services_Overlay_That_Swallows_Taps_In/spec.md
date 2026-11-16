# Found (BRL-wr0s device check, Pixel 8a, 2026-11-16)
For about 2.2 s after a cold launch, an input `tap` never reaches the app. Right after the tap, the app gets `WindowFocusChanged -- 0`. `dumpsys input` then shows the focused window as `name=Embedded{}`, inputConfig=TRUSTED_OVERLAY, frame `[-1320,0][1080,1080]` (rotated), owner `com.google.android.gms.ui` (uid 10161). The IME starts input for `com.google.android.gms`. Screenshots taken right after show only the game UI, so the overlay looks invisible or transparent. Without a tap, the app keeps focus. Taps from 2.4 s on are delivered normally.

# Hypothesis
This is a Play Services game overlay (Pixel Game Dashboard / Play Games entry) attached because the manifest declares `android:appCategory="game"` (`packages/brawler-android/android/app/src/main/AndroidManifest.xml:11`).

# Plan
1. Confirm it: build a variant without `appCategory="game"` (or turn Game Dashboard off for the app in the phone's Game settings, which needs the owner because it is a phone setting) and repeat the timed taps.
2. If confirmed, the owner decides: keep the game category (and its dashboard), or drop it. Check whether a Game Mode / `android:isGame`-style setting is needed for Play Store categorization.

# Acceptance
- The cause is identified with evidence. If the category is dropped, a cold-launch tap at 1.8–2.2 s reaches the app (Pixel 8a).
