# Outcome

Deferred from BRL-jgj4. Keyboard and gamepad players hear the same UI feedback as pointer/touch
players: confirm/back/click on activation and an optional quiet tick on focus movement.

# Context

BRL-jgj4 added `ui-click`, `ui-confirm`, `ui-back` cue keys and a UI audio producer
(`src/client/audio/producers/ui.rs`) that reads the shared BRL-drny `ReleaseActivation` press
model, so only undragged pointer/touch releases sound. Keyboard/gamepad activation is handled by
each screen owner (`client/shell.rs`, `client/flow/input.rs`, Map Maker, Practice maps), and there
is no shared semantic "control activated" fact for the producer to observe.

# Scope

- Introduce one shared, presentation-only activation fact (or reuse an existing one) that every
  screen's keyboard/gamepad activation path emits, without moving action meaning out of screens.
- Map it through the existing UI producer by surface role; decide whether focus movement gets a
  `ui-move` tick (new original clip from `asset_src/audio/build_audio.py`).

# Acceptance criteria

- Keyboard Enter/Space and gamepad South activation of shell, Dashboard and settings controls play
  the matching UI sound once; Escape/East back plays `ui-back`; no double sound with pointer.
- Focused tests for the emission paths; native check.


# Design decisions (2026-11-14)

- **Shared fact:** `ui::feedback::UiFeedback` (presentation-only `Message`, client crate):
  `Activated(Option<Surface>)`, `Back`, `Moved`, `Stepped`. Each screen owner writes it only where
  its keyboard/gamepad/platform-Back input actually acted (an action queued, focus changed, a
  slider value changed). The surface role is read by the screen at activation time and carried in
  the message, so the producer never looks up an entity that the activated action may already have
  despawned. Action meaning stays with the screens.
- **Producer:** the existing `ui` audio producer (rank 90) reads the fact beside the pointer/touch
  `ReleaseActivation` path and maps it through the same `ui_cue(surface)` role table plus a new
  `ui-move` cue (focus moves and slider steps). No direct playback; requests go through the
  existing `AudioRequest` boundary and the authored catalog.
- **Spacing (like the BRL-0174 hit spacing, but real time because the shell has no ticks):** a
  key/pad press within 50 ms of another UI press is the same gesture seen twice (for example flow
  and shell both reading Esc) and sounds once; pointer releases keep sounding as before and open
  that window; `ui-move` ticks are at most one per 70 ms and never right after a press. `ui-move`
  also has `concurrency_cap: 2`. Held arrows cannot repeat (`just_pressed`), a stick sweep or a
  slider drag is rate-limited.
- **Emitters:**
  - `flow/input.rs` (Dashboard, Choose Game, overlays, confirmations, Results, server select):
    `move_focus` helper ticks only when focus changes (edge presses are silent); Enter/Space/South
    activation carries the button's `Surface`; every Escape/East/platform-Back mapping that queues
    an action publishes `Back` (the root Dashboard has none and stays silent). The writer is paired
    with `PendingFlowActions` in one tuple parameter to stay within Bevy's 16-parameter limit.
  - `shell.rs` (Settings/Credits/Error): directional navigation and Tab/bumper focus transfer tick
    on a real move; activation carries the focused control's `Surface`; Back publishes only for
    shell-owned layers (flow-owned overlays publish their own).
  - `shell/settings_screen.rs::slider_step_feedback`: one `Stepped` when any slider-backed Settings
    value (UI scale, master/effects/music volume, camera shake, seven input calibration values)
    changed this frame while Settings is open, whichever input changed it: keyboard/gamepad steps,
    mouse drag, and the BRL-0201 touch gesture alike. Steps at a range end change nothing and stay
    silent. Reset/undo are activations and win over the tick.
  - `shell/controls/capture.rs`: Esc/platform Back cancelling a binding capture publishes `Back`
    (a clicked Cancel already sounds through the press model).
  - `flow/screens/practice_maps.rs`: carousel moves (arrows, D-pad, wheel) and Start/Map focus
    changes tick; Enter/South activation carries the control's role (Start confirms); Esc/East/Back
    publishes `Back`.
  - `flow/screens/map_maker.rs`: Esc/East/Back that closes the library, cancels create/rename, or
    returns to the library publishes `Back`. Editor shortcuts stay silent (BRL-yrys).
  - `presentation.rs` in-match menu: arrow moves tick; Enter/Space/South activation carries the
    button role (Resume confirms). Pointer clicks keep their press-model sound.
- **Asset:** one new original clip `ui-move` (0.035 s, 2.35 kHz sine tick, -28 dB RMS, 3.8 KB) from
  `asset_src/audio/build_audio.py`, encoded by the existing deterministic FFmpeg path (rebuilding
  `ui-click` was byte-identical). Manifest row `audio.original.ui-move` (license `Original`), README
  inventory, catalog mapping and profile. The existing original-audio license note already covers
  `brawler/audio/original/*.ogg`, so Credits are unchanged and their drift tests pass.
- Docs: `docs/11-art-and-presentation-direction.md` audio paragraph.

# Implementation (2026-11-14)

New `packages/brawler-client/src/ui/feedback.rs`; changes in `audio/producers/{ui,mod}.rs`,
`audio/request.rs` (`UI_MOVE`, 47 built-in keys), `audio/catalog.rs` (cap test), `flow.rs`,
`flow/input.rs`, `flow/screens/{practice_maps,map_maker}.rs`, `presentation.rs`, `shell.rs`,
`shell/settings_screen.rs`, `shell/controls/capture.rs`, `assets/catalogs/audio_profiles.ron`,
`assets/manifest.ron`, `asset_src/audio/{build_audio.py,README.md}`,
`assets/brawler/audio/original/ui-move.ogg`. A Map Maker test's runtime setup moved into a helper
(it crossed Clippy's line limit after the new message registration).

# Verification (2026-11-14)

Scratch `CARGO_TARGET_DIR`; the user's running server was not touched or rebuilt.

- Focused tests (pass): producer spacing (`spacing_drops_repeated_presses_and_bounds_ticks`,
  `a_held_arrow_sweep_ticks_at_most_once_per_spacing_window`); producer ECS
  `keyboard_and_gamepad_feedback_requests_ui_sounds_with_spacing` (confirm/back/click/move, a
  duplicated press sounds once, per-frame steps rate-limited); flow ECS
  `escape_and_platform_back_publish_one_back_feedback_on_every_shell_screen` (every BRL-0198 back
  case, Esc and platform Back, exactly one `Back`; root Dashboard silent) and
  `dashboard_keyboard_moves_and_activation_publish_feedback_with_the_surface_role` (move, silent
  edge, Play activation carries `Surface::Primary`); shell ECS
  `settings_keyboard_input_publishes_step_move_activation_and_back_feedback` (step at max silent,
  step down one `Stepped`, move, toggle activation with its role, Esc `Back`); touch slider test
  extended to assert the drag publishes `Stepped`; existing catalog distinct-clip, manifest
  decode (lewton), Original-provenance and Credits drift tests pass with the new clip.
- `just check`: pass.
- `just lint` (VS Code's bundled ripgrep first on PATH): pass (fmt, Clippy for every role, server-features guard with no bevy_audio/rodio/lewton in the server graph, lint-exception check).
- `just test` (after the last edit): pass, 2395 tests, 0 failed (client lib 787, server 583/533, network 115, routing, performance, balance lab).
- Native: not run (no adb/phone; native windows kept to none). Audibility needs the owner playtest.

# Playtest request

Run `just run 1` (or `just balance-lab` for Practice), then with keyboard and with a controller:
- Dashboard: arrows/D-pad give a soft tick per focus move, nothing at an edge; Enter/Space/South on
  PLAY gives the confirm sound, on other tiles the click; Esc/East/Android Back on any inner screen
  gives the back sound, and nothing on the root Dashboard.
- Choose Game, Brawlers, equipment, confirmations, Results, Choose Practice Map (carousel arrows
  tick, START confirms), Map Maker library Esc.
- Settings: arrows/Tab/bumpers tick; Left/Right on a slider ticks per step and is silent at the
  ends; a mouse or touch drag ticks but never buzzes; toggles click; Esc backs; Controls binding
  capture Esc backs.
- In-match menu: arrows tick, Enter on RESUME confirms.
- Pointer/touch clicks still sound exactly once, and a key press never doubles a sound.
- Is `ui-move` audible but unobtrusive? Too quiet, too sharp, or annoying on repetition?

Known limitations (BRL-yrys): text-field commit/cancel, Map Maker editor shortcuts and grid
cursor, opening/closing the in-match menu with Esc, and the automation UI gallery stay silent. UI
and combat share one Effects volume category (unchanged from BRL-jgj4).

# Feedback disposition

Awaiting owner playtest.

# Learn from errors

- Mistake: the first flow test expected Dashboard buttons right after `configured_customization_app`,
  but that harness enters the Dashboard before the lobby membership exists, so no controls were
  spawned. Prevention: re-enter the screen (or assert the controls exist first) before driving
  keyboard input in flow tests.
- Mistake: the first shell test pressed Enter on a slider row; sliders adjust and never activate.
  Prevention: pick the control by id for activation assertions instead of relying on spatial order.
- Mistake: shell Back was first published for flow-owned overlays too, doubling the flow's own
  Back. Prevention: emit feedback at the owner that resolves the action, gated by the same layer
  ownership rule the actions use; the producer's press window is a backstop, not the design.
- Mistake: an earlier, stale background test run kept the build lock while edits continued.
  Prevention: stop superseded background runs before starting the next one.
- Lint caught `f32` array equality (switched to exact bit patterns, which is the intended change
  detection) and two functions over the line limit (extracted `navigate_with_feedback` and a Map
  Maker test runtime helper rather than adding allows).


# Owner playtest (2026-11-14)

Owner played `just client` against the routed server and accepted the button sound effects ("i'm ok with the sound effect on button"). The shared ui-move tick, pointer/touch slider ticks, carousel wheel ticks and tick volume stay as implemented; no change requested. Remaining silent paths stay in BRL-yrys.

# Device check 2026-11-15 (Pixel 8a)
Setup: Pixel 8a (45171JEKB03025, Android 17), release APK built from HEAD 6a6d6599. A scratch protocol-48 supervisor ran on 0.0.0.0:5001 and the phone reached it over Wi-Fi. Input came from adb. Evidence is under /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/dc/.
- The audio path initialises on device: on every launch `dumpsys audio` shows an AAudio player for the app (`usage=USAGE_MEDIA`, 44.1 kHz stereo, `state:started`). `dumpsys media.audio_flinger` shows the app's track active with non-silent level readings.
- Cues play: during a 1v1 the client logged `client audio one-shot cap suppressed a cue cue_key="hit-taken"`, so gameplay one-shots were being voiced up to the cap.
- No audio, decoder or rodio/cpal errors in logcat across 6 launches. The only app ERROR is `bevy_gilrs: Failed to start Gilrs` (no gamepad backend on Android; harmless).
- UI interactions exercised: Dashboard/menus/Settings sliders/toggles by touch, and Back on every shell screen. I cannot hear them. **Owner must judge by ear** that UI move/click/confirm/back sounds play on the phone. Evidence: `dumpsys-audio-01.txt`, `af-01.txt`, `logcat-final.txt`.
