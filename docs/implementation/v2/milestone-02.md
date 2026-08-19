# V2 milestone 02 — Functional product client shell

## Status

| Field | Value |
|---|---|
| Status | Complete |
| Prepared | 2026-08-18, while M01 remains the current delivery milestone, by explicit user direction |
| Objective | Replace the windowed auto-connect/debug entry with a functional controller-friendly shell and persistent local settings |
| Entry dependency | M01 must complete before M02 enters `Implementing` |
| Scope authority | Validated by the user's explicit implementation direction on 2026-08-19 |

M02 was specified early while M01 finished. M01 is complete, and its final client/routing behavior
must remain intact while M02 is implemented.

## MVP outcome

A normal windowed client opens a Title screen instead of connecting automatically. With controller
or keyboard/mouse, the player can open Settings, change and save existing input options, inspect
Credits, recover from a local settings error, and quit. Existing headless, direct, routed, and
visual-test clients retain an explicit auto-connect path.

The shell should be straightforward code that can grow in later milestones. M02 does not build a
general UI framework, screen editor, future navigation graph, or animation framework.

## Decisions for validation

| Concern | MVP decision |
|---|---|
| UI | Native Bevy 0.19 UI with a few project-owned helper functions/components |
| Screen ownership | One Title root plus at most one active overlay: Settings, Credits, or Local Error |
| Navigation | Bevy focus/directional-navigation primitives, limited to the currently visible root |
| Styling | Small theme resource and reusable button styling for normal, hovered/focused, pressed, and disabled |
| Animation | Short non-blocking panel/screen entrance animation; closing and critical actions happen immediately |
| Motion option | One `reduced_motion: bool`; reduced motion makes shell animation immediate |
| Settings | Draft, Apply, Cancel, and Reset using the existing input-settings rules |
| Persistence | Versioned RON file, bounded load, platform-local path, atomic explicit save |
| Compatibility | Normal windowed launch shows Title; explicit auto-connect preserves automation |

## Scope

### Included

- Title with Settings, Credits, Quit, and visibly disabled Play and Practice entries;
- one Settings overlay containing the existing binding, calibration/deadzone, inversion, conflict,
  rebind, and reset behavior;
- UI scale and reduced-motion settings;
- one Credits overlay with the game/version, Bevy/Fira Mono license information, and current asset
  attribution;
- one local-error overlay for settings load/save failures;
- keyboard, mouse, and controller navigation with visible focus;
- simple responsive layout and scrolling where Settings or Credits does not fit;
- versioned settings load/save and safe defaults;
- explicit auto-connect for existing development and verification commands.

### Deferred

- server selection and connection errors (M03);
- build editing and queue UI (M04);
- loading, results, and requeue (M05–M06);
- combat HUD, final accessibility/readability work, audio/display options, and production glyphs
  (M07);
- functional Practice (M08);
- vendor-specific controller glyphs, localization, screen-reader claims, or a UI authoring tool.

## Design

### 1. Keep the code local and small

Add one windowed-only shell plugin under `src/client/`. Start with a compact layout such as:

```text
src/client/
  shell.rs                 Title, overlays, actions, focus, style, and simple animation
  settings/
    mod.rs                 existing runtime/input settings and Settings UI support
    persistence.rs         versioned file, validation, and load/save
```

Split `shell.rs` only after implementation demonstrates a distinct owner or makes focused tests
materially easier. Do not pre-create `model`, `reducer`, `screens`, `widgets`, `navigation`,
`animation`, and `theme` modules.

`ClientShellPlugin` is installed only for a normal windowed client. It owns presentation and emits
accepted client actions; it cannot mutate authoritative gameplay. Headless/server compositions do
not install it.

### 2. Minimal shell state

M02 has only one full-screen product destination, so it does not need a future-facing
`ClientFlow` state machine. M03 introduces screen-flow state when a second real destination exists.

Use one resource/component representing:

```rust
enum ShellOverlay {
    None,
    Settings,
    Credits,
    LocalError { return_to: ErrorReturn },
}
```

Only one overlay is visible and focusable. A settings save error can replace Settings while
retaining its draft, then return to it for Retry or Continue. This avoids a generic nested overlay
stack.

Buttons emit a small `ShellAction` enum. One system handles those actions and performs the obvious
overlay, settings, auto-connect, or exit operation. Do not add a generic transition algebra,
arbitrary callbacks, or a pure reducer unless later screens demonstrate the need.

Each screen/overlay owns one marked root entity. Replacing or closing it despawns that root and
restores focus to a stable control ID on the underlying Title screen.

### 3. Navigation and input

Reuse Bevy's `InputFocus` and `DirectionalNavigationMap`. The exact Bevy implementation treats
global automatic navigation as a flat set across UI layers, so generate/rebuild edges only for the
currently visible root after layout. For M02's small fixed menus, explicit neighbor edges are also
acceptable when simpler.

Requirements:

- arrows/WASD, D-pad, and left stick move focus;
- Enter/Space and the generic south face button activate;
- Escape and the generic east face button go back;
- disabled Play/Practice entries are skipped;
- opening an overlay focuses its first useful control; closing restores the originating Title
  control;
- mouse hover/click works without leaving controller navigation unusable;
- rebinding capture temporarily consumes input and retains a cancel action;
- while a menu owns input, the client sends neutral gameplay intent—the server is never paused.

Reuse the existing active-input-device tracking. Text prompts or simple generic keyboard/gamepad
labels are sufficient; M02 does not need a controller-brand glyph system.

### 4. Styling and components

Use native Bevy UI. A small theme resource may contain the handful of colors, spacing values, font
sizes, and animation duration used by the delivered screens.

The reusable button helper needs only:

- primary and secondary appearance;
- normal, hovered/focused, pressed, and disabled states;
- a visible non-color-only focus treatment such as an outline;
- optional selected/invalid treatment only for controls that actually need it.

One style system resolves the final button appearance from `Interaction`, focus, and disabled state.
Avoid a generic token system, broad widget variant taxonomy, scene-based UI, experimental Bevy
widgets/Feathers, egui product UI, or a custom renderer.

### 5. Animation

M02 proves that shell animation can be added without controlling application state:

- Title and overlay roots may animate from a small offset/scale to their resting `UiTransform`;
- overlay dimming may fade in if the native component supports it cleanly;
- target duration is approximately 120–180 ms;
- `reduced_motion = true` applies the final value immediately;
- actions take effect immediately; animation never delays saving, quitting, connection work, or
  error presentation;
- closing may despawn immediately rather than implementing exit animation.

Use `Time<Real>` in `Update`. Do not add a curtain, animation graph, transition queue, exit/swap/
enter state machine, critical-interruption model, or generic curve library in M02. Later screens can
justify those mechanisms if simple entrance animation becomes insufficient.

### 6. Settings behavior

Opening Settings clones current values into a draft. UI scale and reduced motion can preview while
the overlay is open.

- Apply validates the complete draft, makes it active, and saves it.
- Cancel restores the values active before opening Settings.
- Reset changes the draft to defaults; the player must still Apply.
- If save fails, valid values remain active for this session and Local Error offers Retry or
  Continue Without Saving.

Keep runtime-only input capture, active device, focus, and revision counters out of the file. Do not
add speculative sensitivity, audio, display, favorite-server, build, or cloud fields.

UI scale supports a small bounded range sufficient for the supported layouts; choose and test the
exact step during implementation rather than treating it as an enduring public contract.

### 7. Persistence

Define a small serializable `SettingsFileV1` separate from mutable ECS runtime state, with
`schema_version = 1`. RON remains appropriate because it is already used in the project. Enabling
Bevy's client-only serialization feature for input enums is acceptable; do not manually map every
`KeyCode` unless exact-version support proves unavailable.

Use [`directories::ProjectDirs` 6.0.0](https://docs.rs/directories/6.0.0/directories/struct.ProjectDirs.html)
for an application-local `settings.ron` path. Supply the resolved path through a resource so tests
use a temporary directory.

Load behavior:

- missing file: use defaults without an error;
- valid v1: validate and apply it;
- malformed, unsupported, invalid, or clearly oversized file: leave it untouched, use defaults,
  and show Local Error.

Save only after Apply. Use
[`atomic-write-file` 0.3.0](https://docs.rs/atomic-write-file/0.3.0/atomic_write_file/)
for same-directory atomic replacement. This is a small dependency with less cross-platform risk
than custom replacement code. Saving may remain synchronous because the file is tiny and saves are
explicit and rare.

Do not build a migration registry in M02. Version 1 loads; unknown versions fail safely. Add a
migration only when version 2 exists.

### 8. Credits

Credits can be constructed directly from static, reviewed text and the existing asset manifest.
Retain the Fira Mono OFL license in shipped assets and show its attribution. Do not create a generic
credit database, generated catalog, URL launcher, or license framework for this milestone.

### 9. Startup compatibility

Introduce a simple validated startup option:

- normal windowed invocation starts the shell and does not connect;
- an explicit `--auto-connect` uses the accepted existing connection path;
- headless clients always follow the existing noninteractive auto-connect path;
- canonical direct, routed, combat-demo, and controller-demo commands opt into auto-connect.

The network plugin should act on startup configuration or a later user connection request instead
of connecting unconditionally. M02 does not change wire types, routing, authority, or worker
lifecycle. Preserve the current client-ID behavior until its owning milestone changes it.

## Implementation plan

Implementation starts only after M01 completes and the user validates this reduced specification.

### Slice 1 — Preserve the baseline

- [x] Reconcile M01's final client startup changes.
- [x] Add shell versus auto-connect startup configuration.
- [x] Prove headless, direct, and routed commands still use the same accepted connection path.

### Slice 2 — Make settings durable

- [x] Add `SettingsFileV1`, validation, platform path, bounded load, and atomic Apply save.
- [x] Add draft/apply/cancel/reset and load/save error behavior around existing input settings.
- [x] Retain the Fira Mono license and static credits content.

### Slice 3 — Build the functional shell

- [x] Build Title and the single-overlay lifecycle.
- [x] Add minimal button styling, keyboard/controller focus, pointer interaction, and scrolling.
- [x] Add simple entrance animation and the reduced-motion switch.
- [x] Move the existing settings UI into the product overlay and add Credits/Local Error.

### Slice 4 — Verify and hand off

- [x] Update canonical commands and README startup instructions.
- [x] Run the focused automated checks below plus existing role/network regressions.
- [x] Perform the user's physical-controller walkthrough. The live keyboard walkthrough and a
  real-`Gamepad` D-pad/South runtime test pass; the user accepted M02 without further changes on
  2026-08-19.
- [x] Deliver the normal shell and explicit auto-connect playtest paths.

## Focused verification

Automate behavior where regression would be costly or hard to notice:

- settings: valid round trip; missing file; one combined rejected-file case; save failure preserving
  active session values; Apply/Cancel/Reset;
- navigation: overlay traps focus, disabled entries are skipped, opening/closing restores focus, and
  activation occurs once;
- composition: normal windowed startup does not connect; headless/server do not install the shell;
- regression: the canonical direct and routed auto-connect smoke/evidence paths still pass;
- animation: final transform is correct with motion enabled and disabled. Do not exhaustively test
  intermediate animation samples.

Avoid an injected filesystem abstraction solely for tests. Use a temporary directory and a
predictable invalid target for the save-error case. Reuse existing harnesses and commands.

Visual/manual checks are intentionally small:

- inspect 960×540, 1280×720, and one 16:10 or ultrawide layout;
- inspect default and maximum UI scale, not every scale/resolution combination;
- walk Title → Settings → Apply/Cancel → Credits → Quit with keyboard/mouse and controller;
- confirm malformed settings and save failure remain understandable and usable;
- confirm controller disconnect does not trap the UI.

These checks prove reachability and readability, not production polish. Record obvious defects;
defer broader combat accessibility and final visual tuning to M07.

## Exit criteria

- [x] M01 is complete and its final client startup behavior is reconciled.
- [x] Normal windowed launch reaches Title without attempting a connection.
- [x] Keyboard/mouse and controller can operate every delivered action with visible focus.
- [x] Settings Apply, Cancel, Reset, restart persistence, and safe failure behavior work.
- [x] Disabled future entries cannot be activated.
- [x] Credits contain the required current engine, font, and asset attribution.
- [x] The three representative layouts remain readable and all controls are reachable.
- [x] Headless, direct auto-connect, routed auto-connect, and server-only composition retain accepted
  behavior.
- [x] User playtest feedback is recorded and triaged before M02 is marked `Complete`.

## Implementation evidence — 2026-08-19

The first implementation review returned M02 to `Implementing`. It found that Escape could leak
from shell Back into gameplay pause handling, Escape did not cancel keyboard rebinding as promised,
navigation lacked horizontal inputs/edges, programmatic demo configurations could compose the shell
while connecting, save-error copy misstated the active session values, the settings read was not
strictly bounded to one opened file, and focused error-path tests did not prove every claimed case.
The evidence immediately below is retained as the pre-correction baseline; corrective evidence
follows it.

- `cargo fmt --all -- --check` passed.
- Client Clippy passed with warnings denied.
- All 279 client tests passed, including versioned settings round-trip/rejection/save failure,
  draft Apply/Cancel/Reset, overlay focus trapping, disabled-control skipping, controller
  D-pad/South activation, and reduced-motion final transforms.
- All 77 deterministic/UDP network integration tests passed.
- The isolated server check and `scripts/check-server-features.sh` passed; client persistence and
  UI dependencies do not enter the server feature graph.
- `just network-direct-smoke` passed with both clients exiting cleanly.
- `just network-routed-smoke` passed the two-client lobby-to-match-to-fresh-lobby transition and
  worker cleanup.
- Rendered Title captures at 960x540, 1280x720, and 1680x1050 are readable. A first-pass missing
  em-dash glyph was found visually and fixed. The live macOS keyboard walkthrough reached every
  Settings action through focus-following scroll, returned focus on Cancel, and opened Credits.

## Corrective implementation evidence — 2026-08-19

- The product shell now owns a distinct local input context. Shell-owned fixed-tick writes are
  neutral, Escape cannot toggle gameplay pause underneath the shell, and the underlying diagnostic
  HUD reports `shell` instead of `gameplay`.
- Settings capture, shell collection/action handling, and settings presentation use an explicit
  ordered set chain. Escape/B/East cancels capture exactly once, and the South/click activation that
  starts capture cannot become the captured binding in the same frame.
- Keyboard arrows/WASD, all D-pad directions, and both left-stick axes navigate. The active layer's
  cardinal edges are rebuilt after Bevy UI layout from computed control positions with stable
  fallback ordering; disabled and covered-layer controls never enter the graph.
- Shell presentation and initial connection now use complementary methods on the same startup
  configuration. Normal windowed, explicit auto-connect, headless, combat-demo, and controller-demo
  cases are covered.
- Settings load opens one file and reads at most 64 KiB plus one rejection byte. Tests now cover
  malformed RON, unsupported schema, invalid values, oversized content, missing files, and a valid
  round trip without replacing rejected files.
- Load, validation, and save failures now show distinct accurate outcomes. Shell-level tests prove
  validation draft retention, active-session preservation after save failure, repeated Retry
  failure, successful Retry after destination repair, and Continue Without Saving.
- `cargo fmt --all -- --check`, strict client Clippy, `git diff --check`, the isolated server build,
  and `scripts/check-server-features.sh` passed.
- All 287 client/library tests and all 77 deterministic/UDP network integration tests passed.
- `just network-direct-smoke` passed with both clients exiting cleanly.
- `just network-routed-smoke` passed the two-client lobby-to-match-to-fresh-lobby transition and
  orderly lobby/match-worker cleanup.
- The correction did not change UI sizing or layout construction, so the accepted 960x540,
  1280x720, and 1680x1050 visual captures remain applicable. The new post-layout spatial-edge test
  covers the responsive relationship added by this correction. Physical-controller feel remains
  the user-playtest item.

## User playtest handoff

Normal product shell:

```sh
cargo run --locked --no-default-features --features client --bin brawler-client
```

Use arrows/WASD or D-pad/left stick to move focus, Enter/Space or South to activate, and Escape or
East to go back. In Settings, use the focusable field/value/rebind/toggle buttons; Apply saves,
Cancel discards, and Reset changes only the draft until Apply. Please check physical-controller
feel, pointer clicks, maximum UI scale, reduced motion, and whether the compact Settings text is
understandable at 960x540.

Explicit network compatibility paths remain `just network-direct` and `just network`; both launch
windowed clients with `--auto-connect`. The headless gates are `just network-direct-smoke` and
`just network-routed-smoke`.

## User feedback and learning review — 2026-08-19

The user reported that M02 is fine and directed M03 implementation. No corrective feedback was
requested, so the physical-controller feel item is accepted and no work is deferred from this
review.

The implementation review found schedule/input ownership, bounded file reads, and failure-copy
accuracy issues before playtest. The corrective pass fixed those issues and added focused coverage.
The reusable lesson for M03 is to make input ownership, schedule ordering, terminal policy, and
bounded external input explicit at the composition boundary before expanding presentation. The
existing repository guidance already captures those rules, so no new project skill is warranted.

## Explicit non-goals for implementation review

Reject or return to specification review if implementation starts adding any of the following
without new evidence:

- a general screen/navigation framework for unimplemented M03–M08 screens;
- more than one active overlay or a generic overlay stack;
- a reducer/command architecture beyond the small shell action handler;
- a transition curtain, animation graph, queued transitions, or exit-animation lifecycle;
- a generic design-token, widget, credits, migration, or filesystem abstraction;
- a new crate or public UI API;
- a Cartesian test matrix across every resolution, scale, input, state, and animation sample.

## Research basis

The reduced design is grounded in the current client/settings code, canonical scripts, Bevy's
checked-in UI/focus examples, `references/bevy/crates/bevy_ui/src/`, the current asset manifest,
Fira Mono license, and `docs/13-player-ux.md`. The local Bevy snapshot contains 0.20-dev material in
places, so implementation must confirm transferred symbols against the pinned Bevy 0.19 API.

Primary persistence references are the linked `directories` and `atomic-write-file` documentation.
No other external UI framework is required.

## Specification review questions

Approval is requested for the reduced boundary:

1. one Title root and one active overlay, with flow state deferred until M03;
2. lightweight entrance animation only, controlled by one reduced-motion boolean;
3. minimal native-Bevy button helpers rather than a reusable widget framework;
4. static reviewed Credits instead of a generated credit catalog;
5. the focused verification set rather than a full layout/state/input Cartesian matrix.
