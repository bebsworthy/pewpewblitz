# Outcome

The out-of-game UI feels like an arcade game rather than a flat poster.

# Context

- `assets/brawler/shaders/shell_surface.wgsl` draws a well-antialiased slanted SDF panel with one flat fill. Hover and press only change the tint.
- `docs/design/player-shell/motion.md` allows only a 0.4% background drift.
- Menu buttons act on `Interaction::Pressed` (`src/client/flow/input.rs:205`), which conflicts with touch drag-scroll.

# Scope

- Surface shader: vertical gradient, 2-3 px top highlight band, darker bottom lip, and optional inner shadow.
- Pressed state: 0.95 squash and the label drops about 3 px.
- Activate on release, with drag-cancel.
- Primary Play button: breathing scale and a periodic shine sweep.
- Panel entrance transitions.
- Palette tokens per the art direction decision.
- Reduced-motion is honored.
- Update `motion.md` and `ui-toolkit.md`.

# Out of scope

Icon replacement (separate ticket) and screen information architecture changes.

# Acceptance criteria

- All shell screens use the updated surfaces.
- Buttons activate on release and cancel on drag (test).
- Reduced motion disables idle motion.
- Owner visual acceptance.

# Verification

- `just ui-gallery` review.
- `just test`.
- Native captures.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.

# Note from BRL-11zt

When the palette swaps, also update the legacy color wording in `docs/13-player-ux.md` (cream/orange/teal action descriptions near the pause menu, confirmation screens and the ability picker) to the token roles in `docs/design/player-shell/README.md`. Reproducible token and surface reference: `asset_src/art-direction/compose.py` and the BRL-11zt board.


# Design (2026-11-13, before implementation)

Research: `shell_surface.wgsl` draws one flat fill plus an offset shadow; `fill()` in `src/client/ui/surface.rs` is the single role/state color contract; every screen root paints `BackgroundColor(CREAM)` and text uses `INK`; the Dashboard/brawler overlays draw `dashboard_background.wgsl` cream/teal/orange planes with a reduced-motion-aware drift. Pointer activation reads `Interaction::Pressed` in five places: flow buttons (`flow/input.rs`), Practice map carousel, Settings/shell buttons (`shell.rs`, binding-capture Cancel), the in-match menu, and the map maker (press latch). Bevy 0.19 `ui_focus_system` turns a released mouse press into `Hovered` over the node and `None` off it, but a released touch always becomes `None` (no position after release), so release-over-target cannot be inferred from `Interaction` alone. Bevy 0.19 provides `UiTransform` (scale/translation without relayout), `BackgroundGradient`/`LinearGradient`, and `TextShadow` (no text stroke).

Decisions:

- Tokens: `theme.rs` replaces the legacy cream/teal/cyan/orange constants with the BRL-11zt roles `SHELL_BG_TOP`, `SHELL_BG_BOTTOM`, `PANEL`, `PANEL_RAISED`, `INK` (#0E1638, absorbs `TYPE_STROKE`), `PLAY`, `PLAY_HIGHLIGHT`, `ACCENT`, `TEXT`. Removing the legacy names makes the compiler enumerate every consumer; screens migrate by role (cream surface/background → panel or shell background, navy text → `TEXT`, teal selected/secondary → `PANEL_RAISED`, orange → `PLAY`, cyan → `ACCENT`). Relationship tokens are not added until a shell consumer needs them.
- Background: `theme::shell_background()` returns the `shell-bg-top`→`shell-bg-bottom` `BackgroundGradient` for every full-screen root. The Dashboard/brawler background shader keeps its two composition planes but as low-contrast blue tonal planes over the same gradient; its existing drift still freezes under reduced motion/effects. No motif layer in this slice.
- Type: white `TEXT` with an `ink` `TextShadow` sized from the font size. `SurfacePlugin` adds that shadow to any `Text` whose color is the `TEXT` token and which has no shadow, so screens choose the token and never assemble stroke styling.
- Surface shader: vertical gradient (`fill_top`→`fill`), 2.5 px light top band, solid `ink` bottom lip (the old offset shadow, now opaque) that collapses while pressed, optional diagonal shine sweep. Roles: Primary = `play` with a `play-highlight` top; Back/Secondary/selected = `panel-raised`; other roles = `panel`. Focus = `accent` frame with `ink` keyline and `accent` pointer; selection = `panel-raised` fill plus a `play-highlight` rim, so focus and choice stay distinguishable on every fill.
- Motion (`SurfaceMotion`, presentation only, PostUpdate before layout): press squashes to 0.95 and drops ~3 px over ~60 ms via `UiTransform`; release overshoots back over ~120 ms (ease-out-back). Hit areas and layout are untouched. Dashboard Play carries `PrimaryIdleMotion`: ±2% breathing on a 1.6 s period and a shine sweep every 4 s, suppressed while pressed/disabled/inactive. Entrance: surfaces spawned within a short window after the active flow screen or overlay changes scale in from 0.92 over ~200 ms with ease-out-back; ordinary rebuilds (Settings toggles, typing, map editor) do not replay it, and a new navigation simply replaces the old entities. A `SurfaceMotionPolicy` resource carries reduced motion; the client syncs it from `reduced_motion || reduced_combat_effects`. Reduced motion disables breathing, shine, entrance and scale/translation; press remains immediate color + lip collapse.
- Release activation: `src/client/ui/press.rs` adds a `ReleaseActivation` component (required by `FlowButton`, `ShellButton`, `MatchMenuButton`, Practice map controls and gallery items) and a PreUpdate tracker after `UiSystems::Focus`. A press arms it with the pointer origin; moving more than 12 logical px cancels; it fires once on the frame the press ends, if not cancelled and either the mouse release ended over the target (`Hovered`) or the press was a touch. Consumers act on `activated()` instead of `Interaction::Pressed`; focus still moves on press. Keyboard/gamepad activation is unchanged. Slider dragging keeps using held `Pressed`. The map maker's press latch is replaced by release activation, which also prevents a rebuilt button under a held pointer from firing.
- Gallery: a `BRAWLER_UI_GALLERY_CAPTURE=<png>` development hook saves one settled frame and exits (native evidence without OS screen capture); a forced pressed example and the existing Reduced motion demo toggle drive `SurfaceMotionPolicy`.
- Docs: update `motion.md`, `ui-toolkit.md`, `src/client/ui/README.md`, `docs/design/player-shell/README.md` migration note, and the legacy color wording in `docs/13-player-ux.md`.


# Implementation (2026-11-13)

- Tokens: `src/client/ui/theme.rs` holds the BRL-11zt roles, `shell_background()` and `type_stroke()`; legacy CREAM/TEAL/ORANGE/CYAN/TYPE_STROKE are removed. Every shell screen (flow screens, Settings/Controls, shell overlays, match menu, map maker, gallery) migrated by role; screen literals were replaced with tokens. The legacy `game-selection-background.png` used by ten screens and the Dashboard background shader now draw the `shell-bg` gradient with two low-contrast tonal planes (source updated in `asset_src/player-shell/generate-game-selection.cjs` and `.svg`; PNG rasterized by an equivalent pure-Python writer because Sharp/PIL are unavailable, noted in `asset_src/player-shell/README.md`).
- Surfaces: `shell_surface.wgsl` gradient, top band, solid ink lip that collapses on press, selection rim, accent focus frame/pointer, and shine sweep; `surface.rs` role fills, `SurfaceMotion` (press 0.95/3 px over 60 ms, 120 ms ease-out-back release, 0.92→1 entrance over 200 ms, ±2%/1.6 s breathing and 4 s shine on `PrimaryIdleMotion`), `SurfaceMotionPolicy` (synced from reduced motion or reduced combat effects in `dashboard.rs`), `SurfaceEntrance` (opened by `flow.rs` on flow/overlay change), and automatic ink stroke for `TEXT` labels. Dashboard Menu became `Secondary` so Play is the single `play` action.
- Release activation: `src/client/ui/press.rs`; consumers in `flow/input.rs`, `practice_maps.rs`, `shell.rs`, `shell/controls/capture.rs`, `presentation.rs` (match menu), `map_maker.rs` (press latch removed) and the gallery.
- Gallery: `BRAWLER_UI_GALLERY_CAPTURE` evidence hook (top + scrolled bottom), held-press reference, Play idle example, Reduced motion demo drives the policy.
- Docs: `motion.md` (implementation table, entrance rule), `ui-toolkit.md`, `player-shell/README.md` (migration mapping), `docs/13-player-ux.md` legacy color wording, `docs/11` index, `src/client/ui/README.md`.

# Verification (2026-11-13)

- Focused tests: `press.rs` (release-only activation, drag-cancel for mouse and touch, jitter tolerance, mouse release off target, schedule tracker fires once); `surface.rs` (role fills and gradient top, press squash/drop and overshoot with `TimeUpdateStrategy::ManualDuration`, reduced-motion immediate lip without scale, idle breathing/shine only without reduced motion, inactive Play does not breathe, entrance settles); flow test asserts a held press only focuses and the release navigates; match-menu test likewise. Client lib tests 625/625 at first pass.
- `just lint` (real ripgrep first on PATH): pass after fixing new Clippy findings.
- `just test`: pass, 2196 tests, 0 failed.
- `just e2e 2`: pass (exact 1v1 roster reached Active).
- Native evidence (1280x720 at 2x; gallery 1440x900 and 900x600): client logs have no errors or warnings beyond Bevy's window-destroyed notice. Native runs used scratch data dirs; `data/` and `config/` unchanged.
  - Before: `/private/tmp/claude-501/-Users-boyd--claude-spares-spare-3o59cT/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/evidence/before/gallery-wide.png`, `/private/tmp/claude-501/-Users-boyd--claude-spares-spare-3o59cT/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/evidence/before/gallery-compact.png`, `/private/tmp/claude-501/-Users-boyd--claude-spares-spare-3o59cT/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/evidence/before/client/brawler-000600.png` (Dashboard).
  - After: `/private/tmp/claude-501/-Users-boyd--claude-spares-spare-3o59cT/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/evidence/after/gallery-wide.png`, `/private/tmp/claude-501/-Users-boyd--claude-spares-spare-3o59cT/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/evidence/after/gallery-wide-bottom.png` (held press, disabled, selection), `/private/tmp/claude-501/-Users-boyd--claude-spares-spare-3o59cT/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/evidence/after/gallery-compact.png`, `/private/tmp/claude-501/-Users-boyd--claude-spares-spare-3o59cT/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/evidence/after/dashboard/brawler-000600.png`, `/private/tmp/claude-501/-Users-boyd--claude-spares-spare-3o59cT/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/evidence/after/creation/brawler-000600.png`, `/private/tmp/claude-501/-Users-boyd--claude-spares-spare-3o59cT/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/evidence/after/recovery/brawler-000060.png` (Connecting), `/private/tmp/claude-501/-Users-boyd--claude-spares-spare-3o59cT/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/evidence/after/recovery/brawler-001260.png` (Connection Lost).
- Not captured natively (no OS input automation available to this agent): Settings, game selection, Practice maps, brawler list/details, weapon/parts, abilities, confirmations, results, match menu, map maker. Their code paths are covered by tests; native review is requested below and tracked in BRL-51zd.

# Playtest request

Run `just ui-gallery` and `just run 1` (or `just server` + `just client`).
- Gallery: click and hold a button (squash, label drop, lip collapse), release over it (overshoot, action), press then drag more than ~12 px before releasing (no action). Toggle Reduced motion: Play stops breathing/shining and presses no longer scale.
- Client: Dashboard Play breathes and shines; each destination (game select, brawlers, details, weapon/parts, abilities, Practice maps, Settings, confirmations, results, match menu, map maker) scales in once and does not re-pop on Settings toggles, typing, or map-editor edits. Check focus (cyan frame + pointer) versus selection (raised fill + yellow rim) on game/map/part cards. Turn on Reduced motion in Settings and confirm idle and entrance motion stop. On a touch device or trackpad, drag-scroll a list starting on a card: nothing activates.
- Known limitations: icons still use the legacy set (BRL-nzre); reward bursts and motifs are BRL-51zd; Connecting briefly shows a fallback font before the logo loads (pre-existing, BRL-51zd).
- Requested observations: owner visual acceptance of palette and surfaces, motion strength, any screen with low-contrast text or a missing stroke, and any button that fails to fire on release.

# Learn from errors

- Mistake: the first gallery "held press" example set `Interaction::Pressed` once and showed no press, because Bevy's focus system resets a press when there is no cursor. Prevention: drive reference-only states from a system that re-asserts them after `UiSystems::Focus`, and look at the captured image before trusting a state matrix.
- Mistake: the bulk import rewrite picked up tokens named inside string literals (for example "PLAY"), producing unused imports caught only by Clippy. Prevention: strip string literals before scanning for identifiers, or let the compiler drive import edits.
- Mistake: first-pass new code hit pedantic Clippy rules (bool-heavy struct, 9-argument system, float equality in tests, u32→u8 casts). Prevention: run client Clippy with `-D warnings` before the full `just lint`, and model gesture state as an enum/option rather than flags.
- Reusable: OS screen capture is unavailable to agents here; in-app `Screenshot` hooks (`--screenshot-dir`, `BRAWLER_UI_GALLERY_CAPTURE`) plus scratch server/client data dirs (seeding a brawler row in the scratch SQLite) give repeatable native shell evidence without touching `data/`.

# Playtest feedback (2026-11-13)
- Buttons/press feel accepted ("Buttons feel ok").
- Focus/selection ring look: deferred to BRL-fe7t for a better affordance design.
