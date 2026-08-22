# V5 Milestone 03 — Product-shell hardening and closeout

| Field | Value |
|---|---|
| Status | Complete |
| Depends on | V5 M02 accepted and completed on 2026-08-22 |
| Outcome | A responsive, accessible, lifecycle-safe, and verified dashboard-centered product shell, followed by V5 closeout |

## Player-visible goal

Harden the accepted Dashboard, selection, recovery, queue, loading, match-return, and Results flow
across the supported desktop resolution and UI-scale matrix. Make focus, disabled, busy, recovery,
and reduced-motion states consistently readable and operable by pointer, keyboard, and gamepad.
Complete restrained transition/audio polish only where it improves navigation clarity, then close
V5 with native performance, lifecycle, routed E2E, documentation, feedback, and learning evidence.

## Scope boundary

M03 owns responsive presentation, input/accessibility parity, recovery hardening, UI/preview entity
lifecycle, native performance, the remaining original dashboard-art decision, the manual input
matrix, full routed closeout evidence, documentation reconciliation, feedback triage, and the V5
learning review.

M03 does not add accounts, progression, social features, client-selected maps, fabricated server
facts, mobile-specific controls, a general UI framework, or any client gameplay authority. It does
not reopen the accepted M01 information hierarchy or M02 navigation ownership without new user
feedback and an explicit specification change.

## Entry gate

Satisfied. M02 completed after the corrected Dashboard Menu → Change Server → Server Select →
Connect path was accepted by the user on 2026-08-22.

## Research question

What is the smallest hardening pass that makes the accepted dashboard-centered loop operable at
every currently supported desktop window size and UI scale, gives every action an unambiguous focus,
busy, disabled, and accessible state, proves repeated-entry cleanup and native rendering cost, and
closes V5 without creating a general UI framework or changing network authority?

## Research sources

### Repository and exact-version local sources

- `docs/implementation/v5/milestone-01.md` — accepted Dashboard hierarchy, actual-model preview,
  shader, focus, recovery, and deferred M03 matrix.
- `docs/implementation/v5/milestone-02.md` — accepted child-screen, queue/match/results convergence,
  recovery, and overlay ownership.
- `docs/13-player-ux.md` and `docs/screen-flow-map.md` — current product flow, settings,
  accessibility, recovery, and navigation contracts.
- `src/client/flow.rs` — current product-flow schedule, fixed Dashboard layout, logical focus index,
  busy/disabled presentation, and focus-following Build Editor scroll.
- `src/client/shell.rs` — existing Bevy directional navigation, scroll, focus visibility, UI-scale,
  reduced-motion, reduced-effects, and Settings/Credits overlay patterns.
- `src/client/dashboard.rs` — Dashboard UI viewport, actual character/weapon presentation, procedural
  background, reduced-motion behavior, render target ownership, and cleanup.
- `src/client/presentation_3d/diagnostics.rs` and `scripts/v3-render-evidence.sh` — existing bounded
  native frame-time and entity/asset evidence patterns.
- `src/client/settings/persistence.rs` — supported UI scale range `0.8..=1.4`.
- `src/config.rs` — supported window range `640x360..=3840x2160`.
- `$HOME/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.19.1/examples/ui/widgets/viewport_node.rs`
  — exact Bevy 0.19.1 viewport ownership pattern.
- `$HOME/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.19.1/examples/ui/scroll_and_overflow/scroll.rs`
  — exact `Overflow::scroll_y`, `ScrollPosition`, and logical scroll-bound pattern.
- `$HOME/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.19.1/examples/ui/navigation/directional_navigation.rs`
  and `directional_navigation_overrides.rs` — exact automatic and explicit spatial-focus patterns.
- `$HOME/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ui-0.19.1/src/widget/viewport.rs`
  — `ViewportNode` target resizing and the explicit rule that the camera remains app-owned.
- `$HOME/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ui-0.19.1/src/accessibility.rs`
  and `interaction_states.rs` — automatic Button/Label accessibility nodes, explicit
  `AccessibleLabel`, and disabled-state synchronization.

### Primary external references

- [Bevy 0.19.1 viewport-node example](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/ui/widgets/viewport_node.rs)
- [Bevy 0.19.1 UI scrolling example](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/ui/scroll_and_overflow/scroll.rs)
- [Bevy 0.19.1 directional-navigation example](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/ui/navigation/directional_navigation.rs)
- [Bevy 0.19.1 UI accessibility implementation](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_ui/src/accessibility.rs)
- [Bevy 0.19.1 interaction-state implementation](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_ui/src/interaction_states.rs)

The checked-in Bevy reference tree is a 0.20-development snapshot, so exact UI APIs were confirmed
against the locally installed Bevy 0.19.1 crate source and the upstream `v0.19.1` tag.

## As-built audit and evidence

### Native resolution capture

The routed product client was captured through its built-in scheduled screenshot path after the
Dashboard had authenticated and settled. The research captures were temporary and are not shipped
assets.

| Logical window | Result | Observation |
|---|---|---|
| `640x360` | Fail | Header exceeds the width; Menu is off-screen; the preview is clipped; build, game-type, Practice, and Play are below the visible area; the Dashboard root does not scroll |
| `1280x720` | Pass | The complete accepted hierarchy is visible, readable, and balanced |
| `1920x1080` | Pass with polish note | The hierarchy remains readable; the bounded preview avoids uncontrolled growth, though the layout intentionally has more breathing room |

The failure is not a cosmetic edge case: `640x360` is accepted by `--window-size`, and UI scale
`1.4` reduces the effective UI canvas further. The current Dashboard uses fixed minimum widths and
heights inside a non-scrolling `height: 100%` root. Flex sizing alone therefore cannot satisfy the
supported matrix.

### Input and focus

- Pointer activation updates the logical `FlowNavigation` index and correctly uses the same action
  queue as keyboard/gamepad activation.
- Flow navigation currently sorts numeric indices and implements only Up/Down or W/S/D-pad
  movement. Left/Right is reserved for text editing, so the wide three-card footer does not behave
  spatially.
- The preview and build card intentionally share one logical action/index. This is correct for
  activation, but any focus solution must treat both render surfaces as one semantic target.
- Settings/Credits and the Build Editor already contain proven focus-following scroll patterns that
  can be reused locally. The Dashboard has no equivalent.
- Settings uses Bevy's `InputFocus`; the product flow uses its older integer focus owner. Replacing
  both systems is unnecessary for M03 and would expand risk across accepted screens.

### Accessibility and state communication

- Bevy 0.19 automatically gives `Button`, `Label`, and `ImageNode` suitable accessibility nodes.
- Composite card buttons derive noisy labels from child text, and the model-only preview button has
  no text-derived label. Explicit current-fact labels are required for every Dashboard action.
- `InteractionDisabled` correctly feeds Bevy's accessible disabled state, but disabled elements must
  also be excluded from the product-flow focus graph. The current numeric filter already does this.
- Play communicates `JOINING...` and `MATCH IN PROGRESS`; Practice communicates `STARTING...`.
  Build and game-type controls become unavailable during admission but lack their own changing copy.
  The admission status on the primary action is sufficient if the disabled styling remains obvious
  and both controls receive accessible labels that include the reason.
- Full assistive-technology action routing is not introduced in M03. The acceptance surface is an
  accurately labeled/stateful accessibility tree plus complete pointer, keyboard, and gamepad
  operation. A direct AccessKit dependency solely for untested native screen-reader activation
  would be speculative scope.

### Recovery and overlay ownership

M02 repaired the stale Dashboard Menu overlay that could survive a committed change to Server
Select and block Connect. The current action resolver and overlay gate now have one authoritative
path, but the recovery matrix is distributed across tests. M03 must turn the accepted matrix into
one explicit regression table and assert that no modal root survives a destination-screen commit.

### Preview lifecycle and performance

- The Dashboard preview has one app-owned camera, one image render target, one presentation root,
  three lights, one actual imported character/weapon hierarchy when available, and bounded
  primitive fallbacks.
- `ViewportNode` automatically resizes its target to the computed UI node size. It does not own or
  despawn the camera, so `DespawnOnExit` plus explicit asset removal remains the correct boundary.
- The procedural background owns one material asset and freezes its time input when reduced motion
  is enabled.
- Cleanup removes the preview `Image` and background `UiMaterial` handles. There is not yet a
  repeated Dashboard-entry test that proves stable counts after several selection/queue/recovery
  loops.
- The existing opt-in `--render-report` machinery is match/map-ready-specific even though its frame,
  system, entity, and asset sampling is reusable. M03 will give that existing opt-in path an explicit
  `dashboard` versus `gameplay` measurement context and permit authenticated Dashboard readiness.
  This is the demonstrated second use of the bounded evidence path; it does not add another CLI mode
  or enable diagnostics during ordinary launches.

### Motion, effects, audio, and art

- The Dashboard shader already freezes with Reduced Motion. Shell overlays already suppress their
  entrance animation with Reduced Motion.
- The existing session audio already emits one loaded `ready` cue when the authenticated client
  becomes playable and one `error` cue on a new map/join failure. Both degrade to silence.
- Reusing combat sounds for button hover/click would make the interface semantically noisy. M03 adds
  no hover sound and does not make optional audio a navigation dependency.
- The accepted Dashboard now uses the actual game model, PewPew Blitz wordmark, Lilita One display
  face, Kenney CC0 icons, quiet code-owned shader, and localized fighter glow. These assets are
  coherent enough for V5 closeout. A bespoke original UI-art replacement is a separate art
  production task, not a hardening prerequisite.

## Alternatives considered

### Rely on flex wrapping only

Rejected. It does not solve the current fixed minimum heights, preserve action priority, or ensure
that keyboard/gamepad focus remains visible at `640x360` and UI scale `1.4`.

### Raise the minimum supported window size

Rejected. The client already validates and documents `640x360`; the product shell can remain
usable there with a compact scrollable composition. Removing support would hide the defect instead
of hardening the shell.

### Add a generic responsive UI framework

Rejected. Only the Dashboard has the demonstrated fixed-composition problem. Two private layout
classes and focused marker components are sufficient.

### Replace product-flow focus with Bevy automatic directional navigation

Rejected for M03. Settings already uses Bevy navigation, but replacing the accepted product-flow
action and overlay gate would couple a visual hardening milestone to a broad input rewrite. A small
Dashboard-specific neighbor function can add spatial behavior while retaining the single existing
flow action pipeline.

### Add new generated Dashboard art or UI sounds

Rejected for V5 closeout. The accepted licensed assets and procedural field already establish the
style. New art/audio would require selection, licensing, fallback, and acceptance work without
addressing the observed operability defect.

## Accepted technical specification

### 1. Supported layout matrix

M03 retains the current validated window range `640x360..=3840x2160` and UI scale range
`0.8..=1.4`. Layout selection uses effective UI space:

```text
effective_width  = primary_window.logical_width  / ui_scale
effective_height = primary_window.logical_height / ui_scale

Compact when effective_width < 1000 or effective_height < 640
Wide otherwise
```

The boundary deliberately puts `960x540` and `1280x720` at UI scale `1.4` into Compact while
preserving the accepted `1280x720` scale-1.0 composition as Wide. This is a presentation-only
classification and is recalculated only when the primary window size or `UiScale` changes.

The manual matrix is:

| Logical window | UI scales | Required checks |
|---|---|---|
| `640x360` | `0.8`, `1.0`, `1.4` | compact scrolling, utility access, focus visibility, no clipping |
| `960x540` | `1.0`, `1.4` | compact breakpoint and live resize |
| `1280x720` | `0.8`, `1.0`, `1.4` | accepted wide hierarchy and scale-triggered compact layout |
| `1920x1080` | `1.0`, `1.4` | wide hierarchy, restrained growth, readable text |
| `3840x2160` | `1.0`, `1.4` | bounded content width/preview target and no excessive scaling |

Child screens and overlays retain their current scrollable bounded panels, but are exercised at the
same minimum, standard, and maximum representative sizes.

### 2. Dashboard composition

#### Wide

Wide preserves the accepted M01 composition:

- wordmark, authenticated player/server identity, Settings, and Menu in one header;
- actual selected brawler/build as the visual center;
- brawler/build card immediately below the preview;
- game-type, Practice, and dominant Play actions in one bottom row;
- existing maximum widths/heights keep content bounded on ultrawide/high-resolution windows.

M03 may tune gaps and bounded heights where captures show unnecessary dead space, but it does not
change the information hierarchy, card facts, palette, typeface, icons, or dominant orange Play
action.

#### Compact

Compact uses the same entities and information in a vertically scrollable composition:

- header stays one compact row: smaller wordmark, condensed identity card, and icon-led Settings and
  Menu controls; visible text remains where effective width permits and explicit accessible labels
  always remain;
- preview width is bounded by the available content width and uses a `180px` minimum logical height
  instead of Wide's `280px` minimum;
- build card becomes full-width below the preview;
- game-type, Practice, and Play become full-width stacked controls in their existing semantic DOM
  order;
- the root uses `Overflow::scroll_y()` plus `ScrollPosition`; mouse wheel and focus-following scroll
  use the already proven shell/build-editor math;
- initial logical focus remains Play. On very short windows, the focus-following system may scroll
  the primary action into view on entry; the fighter remains available above it rather than being
  hidden or replaced.

The compact layout is desktop fallback, not a touch/mobile redesign.

### 3. Layout ownership and ECS lifecycle

All new types remain private to `src/client/flow.rs` unless a separate lifecycle owner requires a
small `pub(super)` marker:

- `DashboardLayoutClass::{Wide, Compact}` — a private component on the Dashboard root holding the
  applied class;
- `DashboardLayoutRole` — private marker data for the root, header, wordmark, identity card,
  preview, center column, build card, action row, game-type card, and utility controls/labels;
- `DashboardRoot` remains the small `pub(super)` lifecycle marker consumed by render diagnostics.

`spawn_dashboard` creates one hierarchy that supports both classes. `apply_dashboard_layout`
mutates only marked `Node`, `TextFont`, and label visibility values when the class changes; it does
not despawn/rebuild the screen, recreate the preview camera, or reset selection/session state.

System ownership and order:

```text
OnEnter(Dashboard): spawn Dashboard hierarchy
Update / PresentFlow:
  observe window + UiScale changes
  -> classify Dashboard layout
  -> apply marked Node/text presentation changes
  -> present live facts and focus/disabled styling
PostUpdate after UiSystems::Layout:
  keep the logically focused Dashboard action inside the scroll viewport
OnExit(Dashboard):
  DespawnOnExit removes UI/preview entities
  -> explicit Image and UiMaterial asset removal
  -> remove private layout metrics/resource state
```

Classification reads effective `Window`/`UiScale` and exits without mutation while the root's
applied class is unchanged. Focus-following work runs only on Dashboard and reacts only when the
root/class/focus key changes. No every-frame hierarchy rebuild or broad query is added.

The existing `ViewportNode` remains attached to the same preview host. Bevy continues to own target
resizing; the client continues to own camera and asset cleanup.

### 4. Focus and input contract

All actions still resolve through `PendingFlowActions`; presentation never changes flow state
directly.

Non-Dashboard product screens retain their existing ordered Up/Down behavior. Dashboard navigation
adds Left/Right/A/D and gamepad D-pad spatial movement through a pure
`dashboard_focus_neighbor(class, current, direction, available)` function.

Wide neighbor contract:

```text
Settings <-> Menu
     \       /
      Brawler
         |
Game Type <-> Practice <-> Play
```

- Down from either utility enters Brawler.
- Up from Brawler returns to the last utility when known, otherwise Settings.
- Down from Brawler enters Game Type; Up from any bottom action returns to Brawler.
- Left/Right moves across utilities or bottom actions without wrapping.

Compact neighbor contract is the visible order `Settings ↔ Menu`, then vertical
`Brawler → Game Type → Practice → Play`. Disabled actions are skipped without wrapping or trapping
focus. The preview and build card remain one `Brawler` logical target, so both may show the same
focus outline but only one action is dispatched.

Pointer hover never paints an opaque rectangle over the fighter preview. Pointer press sets the
logical focus before dispatch. Enter/Space and gamepad South activate once. Escape/gamepad East
continues to close the active overlay or return through the current screen-specific path; it does
nothing destructive on the Dashboard itself.

### 5. Focus, hover, busy, and disabled presentation

- Every enabled action has visibly distinct rest, hover, pressed, and keyboard/gamepad focus states.
- Focus uses the strongest white/cyan border and remains visible independently of hover.
- The fighter preview retains its transparent background at rest, hover, press, and focus; only its
  outline/glow changes.
- Disabled controls use lower-contrast fill/text, no hover/press response, accessible disabled state,
  and cannot retain logical focus.
- Admission pending keeps Build and Game Type disabled while Play reads `JOINING...`; practice start
  keeps the same controls disabled while Practice reads `STARTING...`; occupied capacity keeps Play
  disabled with `MATCH IN PROGRESS`.
- If a focused action becomes disabled, focus moves deterministically to the closest enabled action
  in the current layout, preferring Play, then Practice, Game Type, Brawler, Settings, Menu.

### 6. Accessibility contract

Every product-flow `Button` receives an explicit `AccessibleLabel` based on its actual current
action and facts. At minimum:

- preview/build: `Change brawler: <build>, <weapon>, <points>`;
- game card: `Change game type: <advertised name>, <rules>, <map pool>, <population/status>`;
- Play/Practice: current action or busy/disabled reason;
- identity is a label containing authenticated player name, logical server name, and online state;
- Settings, Menu, confirmation, retry, cancel, back, and results actions have concise action labels.

Labels update in the same presentation phase as their visible facts. Icon `ImageNode` children are
decorative within labeled buttons and must not create duplicate actionable meaning. Hidden/despawned
overlays do not remain in the accessibility tree. Existing `InteractionDisabled` remains the single
disabled-state component.

M03 verifies keyboard-only and gamepad-only operation and inspects the macOS accessibility tree for
roles, names, and disabled states. Native screen-reader activation is recorded as a follow-up if the
current Bevy/AccessKit platform bridge does not dispatch the existing pointer/keyboard action path;
it is not silently claimed.

### 7. Motion, effects, audio, and fallback

- The accepted slow background shader and fighter glow remain the only continuous Dashboard effect.
- Reduced Motion freezes shader time and suppresses shell/card entrance transforms.
- Reduced Effects uses the current static background fallback and removes any nonessential shadow or
  animated emphasis added by M03; it never removes focus or busy feedback.
- A short entrance transition may be applied to the Dashboard card/action group using the existing
  shell transform pattern. It must be presentation-only, complete within `180ms`, and be skipped on
  reduced motion, repeated rapid entry, or recovery error return.
- Existing ready/error session cues remain the complete M03 shell-audio set. No hover sound, repeated
  navigation tick, reused weapon sound, or audio-only state is added.
- Missing Lilita One, icons, imported model, custom shader, or optional audio retains the documented
  text/default-font, primitive/static, and silence fallbacks.

### 8. Recovery contract

The following transitions are regression-tested as explicit destination/overlay pairs:

| Trigger | Destination | Required overlay |
|---|---|---|
| startup failure or Connecting cancel | Server Select | error for failure; none for cancel |
| Server Select Connect success | Dashboard | none |
| Dashboard Menu → Change Server → confirm | Server Select | none |
| Dashboard unexpected lobby loss | Server Select | classified error |
| Dashboard retryable stale game type | Game Type Select or Dashboard after refresh | actionable error/notice only |
| queue cancel | Dashboard | none |
| match-start cancel success | Dashboard | none |
| confirmed match leave with valid lobby | Dashboard | none |
| results Dashboard | Dashboard | none |
| retry succeeds | owning destination | none |

Any committed flow change despawns modal roots not owned by the destination. An error's action set is
derived from its failure kind and return flow; stale buttons cannot survive behind or above the new
screen. Settings remains locally reachable from Server Select after connection failure.

### 9. Network and authority behavior

M03 changes no wire type, protocol registration, transport, server authority, build legality,
admission, formation, map choice, gameplay, or results ownership. Responsive layout, focus,
accessibility labels, transitions, and preview diagnostics are client-only presentation.

The server and headless feature graphs must not acquire windowing, UI, rendering, assets, audio, or
device-input dependencies. Headless automation continues to bypass presentation while using the
same routed session and gameplay protocols.

### 10. Original-art disposition

`V5-ORIGINAL-DASHBOARD-ART` is closed for V5 as **deferred outside this version**. The accepted
PewPew Blitz branding, Lilita One, Kenney CC0 icons, actual gameplay model, and procedural field ship
as the V5 visual set. A later original-art milestone may replace licensed functional art as one
coherent pack after defining an art-production budget and acceptance target; M03 will not mix a few
new generated elements into the accepted set.

## Implementation plan

### Phase 1 — Responsive Dashboard

- [x] Add the private root layout-class component and role marker components.
- [x] Restructure `spawn_dashboard` only as needed to support one mutable Wide/Compact hierarchy.
- [x] Implement effective-space classification and change-driven node/text mutation.
- [x] Add Compact vertical scroll, mouse-wheel handling, and post-layout focus visibility.
- [x] Preserve the transparent fighter-preview interaction surface and ViewportNode attachment.
- [x] Add pure class-boundary and ECS node-mutation tests.

### Phase 2 — Input, focus, and accessibility

- [x] Add Dashboard-only spatial neighbors for keyboard/gamepad Left/Right/Up/Down.
- [x] Deduplicate logical targets, skip disabled actions, and recover focus deterministically.
- [x] Add explicit dynamic `AccessibleLabel` values and inspect icon/label tree semantics.
- [x] Verify pointer, keyboard, and gamepad dispatch through the existing action queue.
- [x] Add navigation-table, disabled-skip, label, and focus-following regression tests.

### Phase 3 — State and recovery hardening

- [x] Audit every busy/disabled action and align visible and accessible reasons.
- [x] Encode the destination/overlay recovery matrix in focused tests.
- [x] Verify Settings remains available during connection recovery and stale overlays are removed.
- [x] Re-run M02 selection/admission/return-loop tests after presentation changes.

### Phase 4 — Restrained polish and lifecycle

- [x] Keep the existing immediate Dashboard entry after native review found no clarity benefit from
  adding another transition.
- [x] Verify Reduced Motion, Reduced Effects, missing-asset, primitive-model, and silence fallbacks.
- [x] Add repeated Dashboard entry/exit coverage for UI, camera, lights, model/fallback, Image, and
  UiMaterial ownership.
- [x] Remove any duplicate or stale Dashboard resource cleanup encountered by that test.
- [x] Generalize the existing opt-in render report with an explicit Dashboard/gameplay context and
  authenticated-Dashboard readiness, then record bounded native frame/entity/asset evidence.

### Phase 5 — Closeout verification and documentation

- [x] Run `just fmt`, `just check`, `just lint`, and `just test`.
- [x] Run `just e2e 2`, `just e2e 4`, and `just e2e 6`.
- [x] Resolve the native resolution/UI-scale/reduced-motion/effects matrix. Automated endpoint
  classification and native `640x360`/`1280x720` captures pass; the user accepted the remaining
  subjective maximum-size/scale observations as an unevidenced closeout limitation.
- [x] Resolve the manual input matrix. Pointer, keyboard, and synthetic controller paths are covered
  by focused tests; the user accepted the physical-controller observation as an unevidenced
  closeout limitation rather than a claimed test pass.
- [x] Reconcile `README.md`, `docs/13-player-ux.md`, `docs/screen-flow-map.md`, and the V5 roadmap.
- [x] Deliver the playtest path and requested observations.
- [x] Triage every user feedback item and re-run affected verification where behavior changed.
- [x] Record the learn-from-errors review and close M03/V5 only after acceptance.

## Automated verification plan

### Pure and focused ECS tests

- layout class at exact threshold boundaries and all matrix endpoints/scales;
- no layout mutation when size/scale and class are unchanged;
- Wide/Compact marker nodes receive the specified dimensions, flex direction, visibility, overflow,
  and bounds;
- spatial neighbor table, no wrap, disabled skip, duplicate Brawler target, and deterministic focus
  recovery;
- focus-following scroll moves only when the focused action leaves the computed viewport;
- every actionable Dashboard control has a non-empty factually current accessible label;
- busy/disabled visible copy, accessible copy, `InteractionDisabled`, and focus eligibility agree;
- reduced motion freezes the Dashboard material and skips entrance motion;
- no imported asset leaves the primitive/static/text/silence fallbacks unusable.

### Lifecycle tests

Repeat Dashboard → Build Editor → Dashboard, Dashboard → Game Type → Dashboard, queue cancel →
Dashboard, and Dashboard → Server Select → reconnect → Dashboard for at least 25 cycles in a bounded
test harness. At each stable Dashboard there must be exactly one preview camera, preview root, host,
render-target resource, background material resource, and owned light set. After Dashboard exit
there must be no Dashboard-owned entity or resource, and the owned Image/UiMaterial asset counts
must return to baseline.

Late `WorldInstanceReady` observation from an exited generation must not attach a weapon, animation,
or render layer to a new/unrelated hierarchy.

### Network and process tests

- preserve separate-App selection, admission, queue cancel, match-start cancel, match return, Results,
  and disconnect/recovery coverage;
- run canonical role-specific build/test/lint gates;
- run routed 2-, 4-, and 6-client product E2E;
- confirm the dedicated-server feature graph remains presentation-free.

No new network message or compatibility fixture is expected.

## Native visual, input, and performance checks

For every representative layout, capture the settled Dashboard plus Build Editor, Game Type Select,
Settings, Dashboard Menu, queue, loading/cancel, Results, and one recovery error where applicable.
Check:

- no clipped or unreachable action;
- no overlapping header/card text;
- readable actual facts at all UI scales;
- Play remains visually dominant and fighter hover stays transparent;
- focus is visible, scroll follows focus, and pointer hover does not steal keyboard/gamepad focus;
- shader/glow remains quiet and freezes/reduces as configured;
- imported and primitive preview fallbacks both fit their viewport;
- no flash of stale overlay or obsolete screen during return/retry.

The native performance record uses a release client at `1280x720`, imported assets, default effects,
10-second warmup, and 30-second settled Dashboard sample on the same machine used for V3 evidence.
Record commit, OS/CPU/GPU, render profile, sample count, p50/p95/p99/max frame time, frames over
25/50/100ms, explicit `measurement_context=dashboard`, Dashboard-owned entity counts, and
Image/material counts. The existing opt-in `--render-report` flag and bounded exit behavior are
reused; its schema advances once to distinguish Dashboard from gameplay evidence. Acceptance uses
the existing V3 thresholds: at least 1,200 samples, p95 at most `18.5ms`, p99 at most `25ms`, no
frame above `100ms`, and no more than 1% above `25ms`. A second run with Reduced Effects confirms
that the fallback is not slower. Repeated entry/exit must not produce increasing terminal counts.

The manual input matrix covers:

- pointer hover/press/wheel and window resize;
- keyboard arrows/WASD, Enter/Space, Escape, and text-field isolation;
- synthetic controller smoke for the engine input path;
- one physical controller pass for D-pad, South confirm, East back, Settings, Menu, selection,
  queue cancel, and Results return.

## User playtest handoff

After automated and native verification, hand off:

```bash
just run 2
```

Requested playtest path:

1. launch into Connecting and Dashboard;
2. resize from `1280x720` down to `640x360`, then raise UI scale to `1.4`;
3. reach Settings, Menu, Brawler, Game Type, Practice, and Play using only keyboard;
4. repeat using controller, including cancel/back;
5. change brawler and game type, cancel queue, start/cancel loading where timing permits, complete or
   leave a match, and return from Results;
6. change server, reconnect, and exercise one failed connection/retry;
7. toggle Reduced Motion and Reduced Effects and revisit the Dashboard.

Ask the user to report only concrete problems: clipped/unreachable controls, surprising focus move,
unclear busy/disabled state, stale overlay, visual distraction, unreadable fact, controller mismatch,
or lifecycle/performance degradation.

## Implementation and verification evidence

Implementation completed on 2026-08-22 and M03 entered `User playtest` after the automated,
routed-process, lifecycle, and native-render gates passed.

### Automated and routed evidence

- `just check` passed before the final role-local refactor; the subsequent `just lint` rebuilt and
  checked the client, server, routing package, feature isolation, and V3 presentation guard.
- `just lint` passed after formatting and Clippy cleanup.
- `just test` passed: 406 client tests, 82 serialized network tests, 14 performance gates, and the
  routing/server/unit targets all completed with zero failures.
- `just e2e 2`, `just e2e 4`, and `just e2e 6` passed with exact 1v1, 2v2, and 3v3 rosters reaching
  `Active` and all routed workers stopping cleanly.
- The focused Dashboard suite covers the layout thresholds, wide/compact spatial navigation,
  disabled-target skip and focus repair, fact-derived accessible labels, reduced-motion/effects
  shader freeze, and 25 preview entry/exit cycles with Image and UI-material counts returning to
  baseline.

### Native visual and render evidence

- A settled `640x360` release Dashboard uses Compact, scrolls the initial Play focus into view, and
  keeps game type, Practice, and Play fully reachable without clipping. The header, fighter, and
  build card remain reachable above through scroll.
- A settled `1280x720` release Dashboard preserves the accepted Wide composition and transparent
  fighter interaction surface.
- Dashboard report: macOS 26.5.1, Apple M3/Metal, native profile, schema 3,
  `measurement_context=dashboard`, 1,801 samples, p50 `16.669ms`, p95 `16.839ms`, p99 `16.942ms`,
  max `28.509ms`, one frame over 25ms, none over 50/100ms, eight Dashboard-owned root entities,
  and `result=pass`.
- Gameplay report: the canonical two-client routed release run wrote
  `target/v5-m03-gameplay-render-evidence.txt`; schema 3 retained
  `measurement_context=gameplay`, 1,801 samples, p95 `17.091ms`, p99 `17.259ms`, max `18.281ms`,
  no frame over 25/50/100ms, zero Dashboard-owned entities, and `result=pass`.
- A workspace-wide `just clean` began concurrently with the first evidence retry and removed the
  release directory during compilation. After it completed, the clean rebuild and evidence run
  passed. Only regenerable Cargo artifacts were removed; no authored file or player setting was
  touched.

### Accepted closeout limitations

The user requested V5 closeout on 2026-08-22 without an additional defect report. That accepts the
implemented product shell and existing automated/native evidence. A physical-controller pass and
subjective observations at maximum window size/UI scale were not separately reported; they remain
unevidenced observations and are not retroactively claimed as successful tests. No behavior change
was requested during closeout, so only documentation integrity checks were affected and re-run.

## Feedback review

| Feedback item | Decision |
|---|---|
| Close M03 and V5 | Accepted on 2026-08-22; the user accepted the implemented responsive Dashboard, connected loop, and recorded evidence |
| Additional visual, navigation, recovery, or performance defect | None reported during the final playtest handoff |
| Physical-controller and maximum-scale observations | Accepted closeout limitation; retain as future regression observations if a concrete defect is reported |
| Bespoke original Dashboard art | Remains deferred outside V5; the accepted PewPew Blitz branding, Lilita One, Kenney icons, actual model, and procedural field are the closed V5 set |

There was no accepted behavior change after the canonical verification matrix, so the affected
closeout rerun was `cargo fmt --all -- --check` plus `git diff --check` over the documentation update.

## Learn-from-errors review

### What went wrong

1. The first gameplay evidence retry encountered a full data volume while Rust was stripping the
   release client. The failed strip left a small invalid output that Cargo subsequently considered
   current.
2. A separate workspace-wide `just clean` began while the clean release evidence rebuild was
   running and removed `target/release` during compilation, causing a second non-code failure.
3. The accepted Wide Dashboard could not simply shrink to the documented `640x360` minimum; fixed
   heights and widths made the main actions unreachable before Compact was introduced.

### Causes

- Native evidence began without first checking free space and active Cargo/Just processes.
- Generated-output cleanup and evidence compilation did not have an explicit coordination check.
- The original Dashboard implementation optimized for the accepted `1280x720` composition before
  exercising the complete validated window/UI-scale contract.

### Prevention and reusable lessons

- Before a long release evidence run, check disk headroom, validate any existing output binary, and
  inspect active Cargo/Just processes. If cleanup is required, remove only validated regenerable
  targets and wait for every cleanup owner to finish before rebuilding.
- Treat logical navigation targets separately from render surfaces. Preview and build-card entities
  can share one semantic Brawler action without duplicating focus dispatch.
- Classify responsive UI from effective logical space (`window / UiScale`) and mutate marked nodes
  only when the applied class changes; do not rebuild stateful screen hierarchies on resize.
- Extend native diagnostics through explicit measurement context and lifecycle ownership. Dashboard
  evidence must not pretend gameplay/map readiness, and gameplay evidence must report zero
  Dashboard-owned roots.
- Accessibility claims must stay bounded to evidence: factual labels and disabled states are tested;
  unperformed native screen-reader or physical-controller passes are recorded, not inferred.

These lessons are project-specific extensions of the existing Bevy and repository guidance. The
build/evidence interruption was a single observed incident, so it does not yet justify creating a
new reusable Codex skill.

## Closeout decision

M03 and V5 completed on 2026-08-22. The accepted version delivers auto-connect startup, a factual
responsive Player Dashboard, Dashboard-owned brawler/game-type selection, connected queue/practice,
match/Results convergence, recovery/menu cleanup, input/accessibility hardening, bounded preview
lifecycle, and passing routed/native evidence without changing server authority.

## Risks and mitigations

1. **Resize churn:** mutate marked nodes only when effective metrics/class change; never rebuild every
   frame.
2. **Preview loss during layout change:** retain the host/camera/target and mutate its `Node`; do not
   despawn the hierarchy on breakpoint changes.
3. **Focus hidden by compact scroll:** run a Dashboard-specific visibility system after
   `UiSystems::Layout` and only when focus/layout changes.
4. **Two focus architectures:** keep M03 changes local to Dashboard logic; do not partially migrate
   accepted non-Dashboard flow screens.
5. **Disabled focus trap:** calculate neighbors from enabled logical targets and deterministically
   repair focus when state changes.
6. **Accessibility overclaim:** verify roles/names/states in the native tree and explicitly record
   screen-reader activation as tested or deferred.
7. **Late scene event after exit:** validate generation/ownership before attaching imported
   descendants and cover it with a lifecycle regression.
8. **Performance evidence drift:** record exact hardware/build/settings and use the established V3
   sample size and thresholds.
9. **Polish scope growth:** no new generated art, audio pack, generic widget framework, localization,
   or protocol work enters M03 without updating this specification and returning to review.

## Exit criteria

M03 can enter `User playtest` only when:

- every supported matrix endpoint is readable and every action is reachable;
- pointer, keyboard, gamepad, hover, focus, disabled, and busy behavior match this contract;
- explicit accessibility labels/states match visible real facts;
- startup, retry, change-server, disconnect, queue/loading cancellation, match exit, and Results
  transitions have no trap or stale overlay;
- Reduced Motion/Effects and all optional-asset fallbacks remain functional;
- repeated Dashboard lifecycle counts return to baseline and native performance meets its locked
  thresholds;
- `just fmt`, `just check`, `just lint`, `just test`, and routed E2E 2/4/6 pass;
- user-facing documentation matches the product.

Satisfied on 2026-08-22: the user accepted the playtest handoff and requested V5 closeout, feedback
was triaged, documentation checks were re-run, and the learning review was recorded.

## Specification validation

Validated by the user on 2026-08-22. Production implementation, verification, feedback review,
learning review, and closeout are complete.
