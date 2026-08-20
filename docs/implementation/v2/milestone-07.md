# V2 milestone 07 — Minimal combat HUD, menus, readability, and accessibility

## Tracking

| Field | Value |
|---|---|
| Status | Specification review |
| Prepared | 2026-08-19, by explicit user direction while M06 remains under development |
| Objective | Replace development-heavy match presentation with the smallest readable gameplay HUD, mode score display, scoreboard, non-pausing menu, polished Results, and bounded accessibility/audio/display settings |
| Entry dependency | Satisfied 2026-08-20: M06 is complete and its accepted Match/Results/Leave seams are recorded; implementation still requires user validation of this specification |
| Scope authority | Research and specification only; production implementation requires user validation |

## MVP outcome

During a match, the player sees only information that changes a gameplay decision: health and
harmful status, weapon ammo/reload state, ultimate/deployable readiness, authoritative match time
and phase, and the active mode's score/objective. The top-right corner is the single mode-owned
score slot. Wipeout and Hot Zone render different compact content there without owning the rest of
the HUD.

Holding Scoreboard shows the current teams and bounded participant state. Opening the in-match menu
sends neutral local input but never pauses the server. Results keeps M06's authoritative result and
fresh-lobby behavior while presenting the final mode score cleanly. Team identity remains readable
without color, and the existing UI-scale/reduced-motion settings grow only enough to cover current
audio, display, and reduced-effects needs.

Connection phase, input device, raw actions, player/network IDs, ticks, entity counts, controls help,
asset fallback detail, and similar troubleshooting facts are absent from the product HUD. They
remain available through the existing F3/environment-controlled diagnostics mode.

## Scope discipline

### Included

- one minimal product HUD shown during Countdown and Active;
- one fixed top-right mode score slot with Wipeout and Hot Zone view models;
- large, bounded countdown and important state alerts;
- a held, non-blocking team scoreboard;
- a non-pausing in-match menu with Resume, Settings, Scoreboard, and confirmed Leave Match;
- M06 Results restyling plus captured final mode score;
- non-color-only team labels in the world, HUD, scoreboard, and Results;
- existing UI scale applied consistently to shell, flows, match UI, and diagnostics;
- master volume, mute on focus loss, windowed/fullscreen, vsync, reduced motion, and reduced combat
  effects;
- direct migration from settings schema v1 to v2;
- supported-layout, keyboard/mouse, controller, contrast, and reduced-effects verification.

### Deferred or explicitly absent

- damage meters, per-player kill/death statistics, medals, streaks, match history, rewards, rank,
  spectators, replay, chat, minimap, compass, latency, FPS, or network quality in the product HUD;
- a generic HUD/widget framework, runtime widget registry, trait-object mode UI, data-driven layout
  language, theme editor, animation graph, or separate plugin for every mode;
- user-authored mode UI; authored maps remain non-executable and cannot provide HUD code;
- per-audio-bus mixing before music or another real bus exists; M07 has one master volume;
- a resolution catalog, monitor selector, frame-limit menu, HDR, render-scale, or graphics-quality
  preset; the OS-resizable window and fullscreen toggle are sufficient for the v2 MVP;
- selectable controller-brand glyph packs, localization, screen-reader conformance claims, or a
  selectable family of color-blind palettes;
- new combat rules, scoreboard statistics, authority state, or result protocol in the supervisor or
  lobby.

The default team palette must be color-vision-friendly, but correctness never depends on that
palette: visible `T1`/`T2` labels, `YOU`, names, values, and status words carry the meaning. A palette
selector is not justified for this milestone.

## Research findings

### Current implementation

- `src/client/presentation.rs` spawns always-visible controls help, connection/input/action text, a
  text pause surface, and a placeholder scoreboard. Those are development surfaces, not a product
  HUD.
- `src/combat/client/hud.rs` already reads the correct replicated local fighter state, including
  health, ammo, reload/cooldown, ultimate phase/charge, deployable lifetime, passive state, slow,
  and defeat. It currently compresses everything into one debug string.
- `src/client/hud.rs` already dispatches Wipeout versus Hot Zone using the stable
  `ModeDefinitionId`, derives deadlines from matching `MatchState`/`MatchClock` generations, and
  retains a bounded roster across participant disappearance. The authority and late-arrival seams
  should be preserved while presentation is replaced.
- `src/diagnostics/overlay.rs` already supplies the requested hidden troubleshooting mode. F3
  toggles it, and `BRAWLER_DIAGNOSTICS_OVERLAY=0|1` can pin it for verification. M07 should move
  development facts there rather than create another debug surface.
- `ClientOverlay` and the M06 Leave confirmation already provide product flow arbitration. The
  legacy `ClientInputContext::Paused` name is misleading because the server never pauses.
- Explicit direct-UDP/windowed auto-connect does not install the product `ClientFlow`. HUD and
  basic Menu/Scoreboard visibility must therefore continue to key from live match/input state;
  product-only Results/Leave routing stays gated on the product flow. Do not install the full shell
  merely to preserve the comparison baseline.
- M06 Results owns only bounded client-local result context and a fresh lobby connection. Final
  mode score can be copied into that same context before match unlink; it does not require lobby or
  supervisor result forwarding.
- Accepted display names currently stop at lobby presentation. A readable match scoreboard needs
  the already-normalized bounded name carried once in the match manifest and replicated as
  presentation metadata. No account identity or mutable rename path is required.

### Bevy 0.19.1 fit

- The exact installed `bevy_ui-0.19.1` exposes `UiScale` as one client resource. Continue applying
  it centrally instead of scaling individual roots or fonts.
- The exact installed `bevy_audio-0.19.1` exposes `GlobalVolume`, but changing it affects newly
  started audio only. M07 can use it for future one-shots and also mute/unmute current `AudioSink`
  and `SpatialAudioSink` components on focus changes.
- The exact installed `bevy_window-0.19.1` exposes runtime `Window.mode` and `present_mode` fields.
  Use only `Windowed`/borderless fullscreen and `AutoVsync`/`AutoNoVsync`; do not enumerate platform
  display modes.
- The checked-in Bevy UI scaling and navigation examples are 0.20-dev architecture references, not
  exact API authority. Existing Brawler 0.19 focus/navigation code remains the implementation
  baseline.

### Accessibility baseline

M07 uses WCAG guidance as a practical visual baseline, not as a web-conformance claim. Gameplay
meaning must have text/shape in addition to hue. Normal HUD/menu text targets at least 4.5:1 against
its backing panel, large text and essential non-text indicators at least 3:1, and non-essential
motion can be disabled. Important telegraphs and authoritative state are never removed by reduced
effects.

## Product layout

The layout uses a few fixed semantic slots with safe edge margins. It is intentionally not a grid
framework:

```text
+------------------------------------------------------------------+
|                         01:42              [ MODE SCORE / GOAL ] |
|                                                                  |
|                 countdown / short critical alert                 |
|                                                                  |
|                         arena view                               |
|                                                                  |
| [ HEALTH + STATUS ]                    [ AMMO | ULT | DEPLOYABLE ]|
+------------------------------------------------------------------+
```

| Slot | Active content | Explicitly not shown |
|---|---|---|
| Top center | countdown, authoritative remaining time, short `SYNCING` fallback | match/player IDs, ticks |
| Top right | mode name only when needed for clarity, team labels, score/progress, target/control state | roster, queue or network facts |
| Bottom left | health bar/value; current slow, defeated, respawn, or protection state | full build sheet, passive internals |
| Bottom right | weapon ammo plus reload/cooldown; ultimate charge/ready/deployed; owned sentry health/lifetime when present | raw recipe points and definition IDs |
| Center alert | countdown, `CONTESTED`, defeat/respawn, victory/defeat transition; one bounded message at a time | scrolling event feed |

Weapon/build names may appear as small labels where they disambiguate the displayed ammo or
ability. The HUD does not repeat the player's name, numeric identity, full build, controls, or
always-ready passive state. A datum with no current decision value is hidden.

## Small mode-score design

Use one presentation-only enum, built by pure functions from matching replicated generations:

```rust
enum ModeScoreView {
    Wipeout {
        scores: [u16; 2],
        target: u16,
    },
    HotZone {
        progress_percent: [u8; 2],
        status: HotZoneStatus,
    },
    Syncing,
}
```

`ModeScoreView` is neither a component on authoritative entities nor a wire type. The product HUD
system matches the stable mode definition ID, reads only that mode's replicated state, and writes
the shared top-right node hierarchy:

- Wipeout: `T1 3  —  2 T2` with `FIRST TO 5` in smaller text;
- Hot Zone: labeled T1/T2 progress bars and values, plus `EMPTY`, `CONTESTED`, or `Tn CONTROL`;
- missing or generation-mismatched state: `SYNCING OBJECTIVE`, never stale mixed-generation data;
- unknown mode: hide score values and show one bounded unsupported-mode label in diagnostics; do
  not fabricate a generic score.

With two concrete modes, an enum and one `match` are clearer than traits, callbacks, registered
renderers, per-mode plugins, or a general objective schema. A future third mode adds one enum branch
and its focused builder. Only repeated complexity demonstrated then may justify extraction.

## ECS ownership and schedule

| Owner | Responsibility |
|---|---|
| Match worker | unchanged authority over phase, clock, scores, objectives, fighters, effects, result, respawn, and protection |
| Match manifest/admission | carry the lobby-accepted bounded display name with each participant |
| Replicated fighter presentation metadata | stable accepted display name used by the match scoreboard; never authority or account identity |
| Client combat presentation | read the controlled fighter and build a small combat-status view |
| Client match HUD | build phase/time, mode-score, roster, and alert views from matching replicated state |
| Client flow/overlay | in-match menu, Settings return destination, Leave confirmation, Results actions |
| Diagnostics overlay | all troubleshooting/development facts |

Keep the existing client-only plugin boundary. Start by reorganizing the existing
`src/client/hud.rs`, `src/combat/client/hud.rs`, and `src/client/presentation.rs`; do not pre-create a
HUD crate or folder tree. Extract `match_menu.rs` or another focused file only if implementation
shows distinct lifecycle ownership that materially improves tests.

Use two named client presentation phases if ordering is otherwise unclear:

1. collect immutable presentation views after replicated state and combat HUD/status readers are
   available;
2. write the retained UI nodes and visibility.

The presentation schedule must not write `MatchState`, concrete mode state, fighter runtime state,
queue state, or network messages. Menu input is resolved before the next gameplay intent is emitted
so opening the menu produces neutral movement/aim/actions without a one-frame leak. Deferred
despawn/spawn boundaries remain explicit when changing Match/Results or modal roots.

Do not force change-detection complexity across all HUD sources. The current match has at most one
root, one controlled fighter, and a bounded roster; rebuilding the small view every frame is
acceptable until profiling demonstrates a problem. Retain nodes and mutate text/bar values rather
than despawning the HUD every frame.

## Scoreboard and team identity

Holding Tab/View during Active shows a centered non-modal scoreboard and does not suppress movement
or actions. Releasing the control hides it. The in-match menu may open the same scoreboard in a
latched form; Cancel returns to the menu.

Each row shows only:

- `YOU` when local;
- accepted display name;
- team label (`T1`, `T2`) and stable team grouping;
- compact build/weapon name;
- alive, respawning, defeated, or disconnected state.

There are no per-player kills, damage, ping, or other columns because the server does not currently
own those scoreboard facts. Cached disconnected rows remain bounded to the current match and clear
on match generation change or Match exit.

World fighters keep team color but also gain a short overhead `T1`/`T2` label; the controlled
fighter retains a distinct `YOU` marker. HUD, scoreboard, Hot Zone bars, and Results repeat team
labels and numeric values. Color is reinforcement, never the only carrier.

The accepted display name is added to the bounded match-manifest participant row, included in its
canonical encoding/digest, validated again by the match worker, and installed as one replicated
presentation component. This is a deliberate application-protocol change: bump the global protocol
version/registry fingerprint as required and update both directions atomically. Do not add a
per-message version or compatibility decoder.

## In-match menu and input

Rename the presentation concept from `Paused` to `InMatchMenu` (or `Menu`) while preserving the
three useful contexts: Gameplay, Menu, and Shell. Product actions use a modal `ClientOverlay` only
while `ClientFlow::Match` is active. The direct-UDP windowed comparison path may show the same basic
Resume/Settings/Scoreboard surface from live match state, but it does not fabricate routed Leave or
Results transitions and does not gain the product shell.

- Pause/Menu opens: **Resume**, **Settings**, **Scoreboard**, **Leave Match**.
- Resume or Cancel closes immediately and restores gameplay input.
- Settings reuses the existing draft/Apply/Cancel surface. A single explicit return destination
  distinguishes Title from InMatchMenu; do not add an arbitrary overlay stack.
- Leave Match reuses M06's confirmation and intentional fresh-lobby transition. **Keep Playing**
  remains the initial focus.
- While Menu, Settings, or Leave confirmation owns input, send neutral gameplay intent every tick.
  The match, timers, opponents, and vulnerability continue normally.
- The held gameplay scoreboard is non-modal. The menu-opened scoreboard is modal only because the
  menu already owns input.

Remove the word `PAUSED` from product copy. Use `MATCH MENU — MATCH CONTINUES` so behavior is honest.

## Results

Preserve M06's one client-local result context, fresh lobby connection, Queue Again, Change Game,
and Disconnect behavior. Before match unlink, copy one additional bounded `ModeScoreView` final
snapshot into that context. Results shows:

- `VICTORY`, `DEFEAT`, `DRAW`, or the existing forfeit variant;
- local team and non-color team labels;
- final Wipeout score or Hot Zone percentages;
- game name;
- the three existing actions.

Do not retain the match roster, replicate terminal history to the lobby, or add rewards/statistics.
If final objective state is absent or generation-mismatched, show the authoritative result without
a fabricated score.

## Settings and persistence

Extend the current draft/Apply/Cancel/Reset behavior with the following bounded fields:

| Field | MVP values | Application |
|---|---|---|
| UI scale | existing `0.8..=1.4` in `0.1` steps | central Bevy `UiScale`; preview allowed |
| Reduced motion | on/off | existing shell transitions become immediate; no non-essential match UI movement is added |
| Reduced combat effects | on/off | smaller/shorter transient flashes and dash trails; durable status markers, hit confirmation, crosshair/range, objective state, and telegraphs remain |
| Master volume | `0..=100` in steps of 10 | future sounds use adjusted `GlobalVolume`; current sinks update when Apply commits |
| Mute when unfocused | on/off, default on | mute current sinks while the primary window lacks focus and restore the configured volume on focus |
| Fullscreen | windowed / borderless fullscreen | primary `Window.mode`; Cancel restores the prior draft baseline |
| Vsync | on/off, default on | primary `Window.present_mode` uses automatic supported modes |

No screen shake exists today, so M07 does not add a shake system merely to expose a setting. Reduced
combat effects adjusts only current transient presentation. Essential gameplay feedback remains
visible in both modes and is tested as such.

Create `SettingsFileV2` only because new persisted fields now exist. Read a tiny
`schema_version` probe, decode v1 or v2 directly, map v1 fields plus safe new defaults into v2, and
write only v2 after Apply. Do not create a migration registry. Unknown, malformed, invalid, or
oversized files retain the existing safe-default/error behavior and are not overwritten silently.

## Responsive and visual rules

- Use native Bevy UI, percent/px constraints, existing safe margins, retained nodes, and the current
  theme helpers. Do not introduce another UI dependency.
- The HUD must remain inside safe margins at 640x360, 1280x720, 1440x900, 1920x1080, one 4:3 size,
  and one 21:9 size at default and maximum UI scale. The smallest window may compact labels but may
  not clip gameplay-critical values.
- Back panels behind small text must provide stable contrast over bright arena content. Target
  4.5:1 for ordinary text and 3:1 for large text/essential bars and focus outlines.
- Alerts replace in place; they do not accumulate, scroll, or cover the reticle longer than their
  bounded state requires.
- UI scaling must not change world-space aiming, camera span, input, or authority.
- Results and modal panels may scroll only if the existing Settings content requires it; the combat
  HUD itself never scrolls.

## Implementation plan

M06's final seams are reconciled. Implementation begins only after the user validates this
specification.

### Slice 1 — Product HUD and diagnostics separation

- [ ] Replace controls, connection/input/action, local numeric identity, and monolithic match text
  with retained product HUD slots.
- [ ] Build focused pure combat, phase/time, alert, and `ModeScoreView` functions.
- [ ] Implement the top-right Wipeout and Hot Zone presentations with generation-safe syncing.
- [ ] Preserve crosshair/range/landing, world health bars, durable status cues, and authoritative
  countdown behavior.
- [ ] Keep all development facts in the existing F3/environment diagnostics mode.

### Slice 2 — Team-readable scoreboard and menus

- [ ] Carry accepted display names through the bounded match manifest and one replicated
  presentation component; update the global protocol contract atomically.
- [ ] Add non-color team/local labels to fighters, HUD, scoreboard, and Results.
- [ ] Replace the placeholder held scoreboard with bounded team rows and current participant state.
- [ ] Replace the legacy pause surface with the non-pausing menu, same-frame neutral input, Settings
  return destination, latched scoreboard, and existing Leave confirmation.

### Slice 3 — Results, settings, and accessibility

- [ ] Capture final `ModeScoreView` into M06's local result context and restyle Results without new
  lobby/supervisor state.
- [ ] Add master volume, focus mute, fullscreen, vsync, and reduced combat effects to the existing
  settings draft.
- [ ] Add direct settings v1-to-v2 migration and preserve atomic save/error behavior.
- [ ] Apply UI scale and contrast/safe-margin rules consistently across shell, product flow, match
  UI, scoreboard, menus, Results, and diagnostics.

### Slice 4 — Verify and hand off

- [ ] Run canonical formatting, lint, client/server role checks, unit, network, process, and
  performance commands.
- [ ] Run automated HUD/model/layout snapshots or node-bound assertions for both modes, all match
  states, supported representative layouts, default/max UI scale, and reduced effects.
- [ ] Run supervised keyboard/mouse and physical-controller matches for Wipeout and Hot Zone.
- [ ] Record visual, controller, contrast, audio, and non-pausing-menu observations; triage user
  feedback and complete the learn-from-errors review.

## Verification contract

### Pure and ECS tests

- combat HUD models show only current gameplay facts and use authoritative ticks for deadlines;
- Wipeout and Hot Zone builders reject mismatched match generations and produce `Syncing`;
- the mode score slot never displays stale values from the previous match;
- scoreboard ordering is deterministic by team then stable player ID, while displaying accepted
  names and a separate `YOU` marker;
- disconnected roster cache and every match UI root clear on match generation/flow exit;
- opening Menu/Settings/Leave emits neutral intent without pausing or mutating authority;
- held scoreboard does not suppress gameplay input; menu scoreboard returns to the menu;
- reduced effects retains durable statuses, hit feedback, reticle/range, objective state, and
  telegraphs;
- v1 settings migrate to safe v2 defaults; v2 round-trips; invalid/unknown files fail safely;
- UI scale changes presentation only; audio/display settings affect only client resources/window.

### Protocol, role, and network tests

- accepted names remain bounded/normalized through lobby, manifest encode/decode/digest, match
  admission, replication, disconnect, and cleanup;
- the one global compatibility handshake rejects the old protocol; there is no per-message version;
- clients cannot author names, scores, objective progress, result, roster status, or match time after
  admission;
- Wipeout and Hot Zone matches reach Countdown/Active/Results through routed workers with the right
  mode HUD and final score;
- Leave and Results retain M06's fresh-lobby and Queue Again behavior;
- direct UDP remains a named comparison baseline;
- server features gain no window, renderer, UI, text, audio, device-input, or client-asset
  dependency; the routing package remains Bevy-free.

### Layout and human checks

- inspect 640x360, 1280x720, 1440x900, 1920x1080, representative 4:3 and 21:9 at UI scale 1.0 and
  1.4; resize one live window across the minimum boundary;
- confirm health, ammo/reload, ultimate, timer, score/objective, and critical alerts remain legible
  without clipping or reticle obstruction;
- inspect grayscale/non-color reading: fighters, scoreboard, objective bars, and Results remain
  identifiable from labels/values alone;
- confirm ordinary text, important bars, and focus outlines meet the recorded contrast targets;
- with reduced effects enabled, confirm combat remains understandable and no non-essential motion or
  intense transient dominates play;
- confirm master volume and focus mute perceptually affect existing cues without changing cue
  emission/deduplication;
- confirm keyboard/mouse and a physical controller can open/close the scoreboard, use every menu and
  Results action, edit/apply/cancel settings, and leave safely;
- while the menu is open, confirm opponents, match clock, scoring, damage, defeat, and forfeit
  continue under server authority.

## Performance and bounds

- HUD work remains O(1) for the local fighter/match root plus O(current match participants) for a
  scoreboard that exists only while visible.
- No history, event feed, queue roster, retained result roster, or unbounded strings are stored.
- Retained product nodes are spawned once per owning flow and mutated in place.
- Keep existing fixed-tick, frame-time, entity, audio one-shot, and diagnostics bounds. Record the
  incremental client UI entity count and windowed frame-time result; do not invent a new universal
  FPS promise without measurement.

## Playtest handoff

Provide one canonical routed launch path covering Wipeout and Hot Zone, controls for Scoreboard,
Menu, diagnostics mode, and Settings, and a short checklist:

1. Can the player decide health, ammo/reload, ultimate readiness, time, and objective state at a
   glance without reading debug text?
2. Does the top-right score slot feel consistent while still making the two modes distinct?
3. Can teams and the local fighter be identified with color ignored?
4. Does the held scoreboard provide enough information without becoming a stats screen?
5. Is it obvious that the in-match menu does not pause?
6. Are default/max UI scale, reduced effects, volume, fullscreen, and vsync understandable and
   comfortable?
7. Are Results and Queue Again/Change Game/Disconnect clear after both modes?

## Exit criteria

- [ ] The user validates this specification before implementation.
- [x] M06's final Match, Results, Leave, and fresh-lobby seams are reconciled without duplicating
  lifecycle ownership.
- [ ] Product combat HUD contains only gameplay-relevant information; development facts are hidden
  in diagnostics mode.
- [ ] One top-right slot cleanly presents Wipeout and Hot Zone score/objective state through the
  small enum dispatch.
- [ ] Scoreboard, menu, Settings return, Leave, and Results work with keyboard/mouse and controller
  while the server continues normally.
- [ ] Team meaning is never color-only, and representative contrast/layout/reduced-effects checks
  pass.
- [ ] Settings v1 migrates to v2 and audio/display/reduced-effects choices persist and fail safely.
- [ ] Required automated, routed/direct, role-isolation, performance, visual, controller, and audio
  evidence is recorded.
- [ ] User feedback is triaged and learn-from-errors review is complete before M07 becomes
  `Complete`.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| HUD grows back into a debug dashboard | Every persistent field must answer a current gameplay decision; everything else uses diagnostics mode |
| Mode customization becomes a framework | Keep one `ModeScoreView` enum and one stable-ID match for the two real modes |
| M06 result/leave lifecycle is duplicated | Restyle existing roots/context and copy only a final presentation snapshot before unlink |
| Display names expand identity/security scope | Carry only the lobby-accepted bounded presentation name; stable player ID remains authority |
| Reduced effects hides counterplay | Maintain an explicit never-remove list and test both settings against the same authoritative cues |
| Menu appears to pause or leaks one input frame | Honest copy plus schedule test proving neutral intent before the next network input write |
| Settings migration becomes infrastructure | One schema probe and direct v1/v2 conversion; no registry until a third real schema exists |
| UI scale clips the smallest layout | Fixed semantic slots, safe margins, compact labels, and node-bound checks at scale 1.4 |

## Research sources

### Local exact-version and project sources

- `docs/00-product-direction.md`, `docs/13-player-ux.md`, and
  `docs/implementation/v2/{roadmap,milestone-02,milestone-06}.md`;
- `src/client/{presentation,hud,flow,input,shell,audio}.rs`,
  `src/client/settings/{mod,persistence,ui}.rs`, `src/combat/client/{hud,effects,cues,world}.rs`,
  `src/diagnostics/overlay.rs`, `src/matchplay/model.rs`, `src/protocol.rs`, and
  `packages/brawler-routing/src/manifest.rs`;
- exact installed sources:
  `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ui-0.19.1/src/lib.rs`,
  `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_audio-0.19.1/src/{volume,sinks,audio_output}.rs`,
  and
  `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_window-0.19.1/src/window.rs`;
- architecture-only checked-in Bevy 0.20-dev references:
  `references/bevy/examples/README.md`, `references/bevy/examples/ui/ui_scaling.rs`,
  `references/bevy/examples/ui/navigation/directional_navigation.rs`, and
  `references/bevy/Cargo.toml`.

### Current primary external guidance

- [W3C WCAG 2.2 — Use of Color](https://www.w3.org/WAI/WCAG22/Understanding/use-of-color)
- [W3C WCAG 2.2 — Contrast Minimum](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html)
- [W3C WCAG 2.2 — Non-text Contrast](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html)
- [W3C WCAG 2.2 — Animation from Interactions](https://www.w3.org/WAI/WCAG22/Understanding/animation-from-interactions.html)

The local registry source is authoritative for exact Bevy 0.19.1 APIs. Internet research was used
only for current accessibility guidance because the checked-in engine snapshot does not define the
product's readability criteria.
