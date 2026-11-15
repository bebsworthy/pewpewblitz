Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec refreshed with code refs. Nothing has been implemented. Android still updates continuously in every flow.

# Outcome
Lower power use outside matches on Android without making menus feel sluggish.

# Context (verified at HEAD)
- Deferred from BRL-zj53 to BRL-ynt5, then split out. With BRL-ynt5's ADPF hint session the Dashboard already presents at 60 FPS, so a lower menu rate is a power decision, not a frame-rate fix. Desktop update behaviour must stay unchanged.
- `WinitSettings` is chosen once at startup: `packages/brawler-client/src/lib.rs` `render_profile_settings` (L742-761), called from `compose_client_app` (L657). Android has no command line, so `RenderProfile::from_env()` (`src/config.rs` L21-28) is `Native`, which gives `WinitSettings::game()` (focused continuous). Nothing changes it per flow.
- Flows: `packages/brawler-client/src/flow/model.rs` `ClientFlow` (Connecting, ServerSelect, Dashboard, GameTypeSelect, PracticeMapSelect, MapLibrary, MapEditor, Queue, MatchLoading, Match, Results).
- Android pacing: `packages/brawler-android/src/frame_pacing.rs` votes 60 Hz on the surface (`request_game_frame_rate`). It reports each frame's work to an ADPF session with a fixed 16.7 ms target (`begin_frame` / `report_frame`). With reactive updates that report should stay correct, or be paused, so idle gaps are not reported as work.
- Things that move in menus and need the reduced rate to look acceptable: the Dashboard 3D weapon/fighter preview (`packages/brawler-client/src/dashboard/weapon_preview.rs`), shell background motifs (`ui/motifs.rs`, which follow Reduced motion), surface entrance and burst animations (`flow.rs` `begin_surface_entrance`, `screens/shared.rs` `apply_flow_burst`), and slider/scroll touch drags.

# Plan
1. Add an Android-only system, installed from `packages/brawler-android/src/lib.rs` next to `frame_pacing::install` so desktop is untouched. On `ClientFlow` changes it sets `WinitSettings`:
   - Match and MatchLoading: `focused_mode: Continuous`.
   - All other flows: `UpdateMode::reactive(1/30 s)` focused and `reactive_low_power` unfocused.
2. Input wakes immediately (reactive already wakes on window and touch events). Confirm that a touch drag on a slider or scroll list updates at the input rate, not at 30 Hz.
3. Optionally vote 30 Hz on the native window in menus (`ANativeWindow_setFrameRate`) so the panel can drop refresh. Decide this from the measurements.
4. Make `frame_pacing.rs` report only real frames, and keep the 16.7 ms target in Match.
5. Unit-test the flow → `WinitSettings` mapping as a pure function, on the host, without Android.

# Acceptance
- `cargo test -p brawler-client` (and the new mapping test) passes. The desktop `WinitSettings` is unchanged; check `render_profile_settings` callers.
- **Pixel 8a device check:** record the Dashboard frame rate (`dumpsys SurfaceFlinger --timestats`) and the power estimate (`dumpsys batterystats --reset`, 5 min idle on the Dashboard, `dumpsys batterystats` app estimate) before and after.
- **Pixel 8a device check:** the Match frame rate is unchanged (60 FPS median, BRL-ynt5 scenario). Menus respond to taps without visible delay, and the Dashboard preview motion stays smooth enough (owner judgement).
