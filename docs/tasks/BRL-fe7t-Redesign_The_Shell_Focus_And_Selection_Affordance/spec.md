Reviewed 2026-11-15 against HEAD 3437c0d8: valid as written (design-first, needs owner choice); refs refreshed.

# Outcome
Focused and selected shell controls read clearly and look intentional on the BRL-11zt blue arcade surfaces.

# Feedback (2026-11-13)
User playtest of BRL-drny: "the focus/selection ring on the buttons doesn't look great. We may want to think of a better affordance design."

# Current state (HEAD 3437c0d8, unchanged since BRL-drny f0ee11c5 apart from the BRL-51zd burst flash)
- `packages/brawler-client/src/ui/surface.rs`: `SurfaceMaterial` uniforms `accent` (focus frame + pointer, :94-96) and `selection` (persistent rim, :106-108); `present` (:595-650) writes `ACCENT` / `PLAY_HIGHLIGHT` and `flags = (focused, quiet, selected)`; `fill` (:552) raises selected non-primary surfaces to `PANEL_RAISED`.
- `assets/brawler/shaders/shell_surface.wgsl:66-77`: selection = yellow inner rim (4 px, 9 px when also focused); focus = cyan band at the border, ink gap, then the face, plus the cyan `marker` pointer.
- Focus source: `flow/screens/shared.rs:221` sets `focused` whenever `navigation.selected == button.index`, with no input-modality gate, so touch users also see a persistent focus frame.
- Reduced motion: `SurfaceMotionPolicy.reduced` (surface.rs:126-135) already gates idle/burst motion.
- Docs: `docs/design/player-shell/motion.md`, `docs/design/player-shell/ui-toolkit.md`. Gallery: `just ui-gallery` (`ui/gallery.rs`).

# To explore
Design options first (mockups in the UI gallery), e.g. lift/scale + glow, inner bevel highlight, animated chevron or bracket corners, outline thickness/contrast; distinct focus vs selection vs pressed; reduced-motion variant; touch (no persistent focus) vs gamepad/keyboard (must be visible). Get user choice before implementing.

# Plan
1. Add 2–3 affordance variants as gallery states (focus, selected, focused+selected, pressed, disabled) on Primary/Secondary/Card/Row roles; capture native screenshots; ask the owner via `ticket task BRL-fe7t question add`.
2. Implement the chosen variant in `shell_surface.wgsl` + `surface.rs::present` (new uniforms if needed); add a "focus visible" input-modality rule (hide focus after touch input, show after key/gamepad) if the owner chooses it.
3. Update motion.md / ui-toolkit.md.

# Acceptance criteria
- User-approved affordance applied across shell surfaces; focus visible for keyboard/gamepad; selection distinct from focus; reduced motion respected; gallery evidence (native captures).
- Existing surface tests (`surface.rs` `selected_focus_keeps_choice_and_inactive_ignores_hover`, reduced-motion tests) updated/passing; `just lint`, `just test`.
- Pixel 8a device check: touch navigation shows no stray focus frame (if that rule is chosen); gamepad focus visible.


# Phase 1 (variants), 2026-11-18
- Review seam, all in automation builds only; player shell unchanged: `SurfaceAffordance` resource in `ui/surface.rs` (`Current` default; `Frame`, `Reticle`, `Spotlight`) selected by `BRAWLER_SURFACE_AFFORDANCE`, fed to `shell_surface.wgsl` as `flags.w`. The focus pulse is `motion.w`, constant under reduced motion. The Frame lift is `SurfaceMotion::ease_lift`. `ReducedMotionPreview` lets one capture compare reduced and normal motion. Whichever variant is chosen becomes the real implementation in this seam; the others get deleted.
- Gallery board: `BRAWLER_UI_GALLERY_AFFORDANCE_BOARD=1 just ui-gallery` (`ui/gallery/affordance_board.rs`; V cycles variants; `BRAWLER_UI_GALLERY_SIZE=WxH`). It shows rest/focus/selected/focus+selected/pressed/disabled on Primary, Secondary, Card and Row, plus a reduced-motion row and a touch row (focus hidden).
- Variants: **Frame**: white outline, face lit in its own hue, narrow inner glow, 3.5% lift, gold inner trim. **Reticle**: breathing white corner brackets, gold bottom bar. **Spotlight**: lit face (cyan, or gold on Play) under a white bevel, gold outline. Iteration: a white-mix glow greyed the navy roles, so Frame now scales the face in hue. Spotlight on Play mixed toward white and looked washed out, so it now uses `play-highlight`. Reticle's underline was thickened for the phone viewport.
- Evidence: sheets `target/fe7t/fe7t-{frame,reticle,spotlight}-sheet.png` (assets a-000019..21). Each compares current vs variant on the gallery board at 1280x720 and 914x411, plus live `just shell-capture` dashboard, settings and game-select under each variant (scratch supervisor 127.0.0.1:5031, data under target/). Raw captures: `target/fe7t/raw/`, `target/fe7t/shell/`.
- Questions: q-y21kgwz (variant; Frame recommended), q-4yrvg6g (focus-visible modality rule; hide after touch, show after key/gamepad recommended).
- Gates: `just lint` pass (target/fe7t/lint.log), `just test` pass, 2565 tests (target/fe7t/test.log). New test: `frame_affordance_lifts_focus_except_under_reduced_motion_or_preview`.
- Not verified: animated pulse/lift timing only by code and tests (captures are stills); no device check (phase 2).


# Owner feedback 2026-11-18 (phase 1 review): scope change
- **Focus and "selected" were presented as two states of every button.** That is wrong for action buttons (Play, Connect, Back): they only have focus, which the mouse (hover), keyboard and gamepad all move. The review board must not show a selected/current state on action buttons at all.
- **"Selected" means the current choice in a set of options.** It is only real where the shell keeps a choice separate from focus: the game type draft on Game Select, the current brawler, the active loadout slot, the equipped part, and the active Map Maker palette tool (see `flow/screens/shared.rs`). The design sample for that state must use those real components, not buttons.
- **Rename:** the chosen-option state becomes **current** (`ControlState`/`SurfaceState` field and its call sites). The navigation field `navigation.selected`, which is really focus, becomes `focused` (or similar). Code then never uses "selected" for two concepts.
- **New review board:** (1) action buttons (Primary, Secondary, with icon): rest / focus / pressed / disabled per candidate focus look; (2) choice components as they appear in the real screens (game type card, brawler row, loadout slot, part row, palette tool): rest / current / focus / current + focus / disabled, with the "current" mark designed for each candidate; (3) the touch row, focus hidden after a tap. Re-render the sheets for Frame, Reticle and Spotlight and replace the ticket assets. Then ask the owner again, with the questions rewritten to match.


# Phase 1 rework after owner review, 2026-11-18
Feedback handling: the scope change (rename, new board, new sheets, rewritten questions) is **implemented** below. Nothing is deferred to another ticket yet; the phase 2 notes list what the chosen look must clean up.

## Rename (behaviour unchanged)
- `SurfaceState::selected` and `ControlState::selected` became `current`: the option the player has chosen in a set. All call sites were renamed with them, including the screen helpers' `selected: bool` parameters, `Choice::current` (game_select), and `MapMakerRuntime::action_selected` → `action_is_current`. The shader's `selection` uniform became `current_mark`, and `flags.z` is now documented as current.
- `FlowNavigation::selected` (navigation focus) became `focused` everywhere in flow and its tests. The match menu's `MatchMenuState::selected` is also focus, so it became `focused` too (a small local rename).
- Left as they are: these are domain choices or placement selection, a separate concept: `selected_brawler_id`, `selected_slot`, `selected_index`, `selected_map`, Map Maker placement `selection`, and `PaletteSelectionMark`. No wire or protocol types changed. `docs/design/player-shell/ui-toolkit.md` interaction-state table: "Selected" became "Current", and focus is now defined as navigation (`FlowNavigation::focused`).

## Action buttons never get a current mark
- Production does set current on action roles: Map Maker's `MapMakerToggleValidation` is `Surface::Primary` and is current while the validation panel is open. `ConfirmGameType` is spawned with current = true, though the chrome system overwrites it every frame.
- New `SurfaceAffordance::marks_current(role)`: the Frame, Reticle and Spotlight candidates never draw the current mark on `Primary`, `Secondary` or `Back`. The shipped `Current` look keeps its rim on every role, so the player shell is unchanged.
- Test: `review_candidates_never_mark_action_buttons_as_current`.

## Current mark per candidate (decision)
- All three candidates add the same "this is your choice" sign: a gold disc with an ink check in the top-right corner of the face (`current_badge` in `shell_surface.wgsl`). It is drawn only on faces at least 56 px tall.
- **Why 56 px:** at 34 px the badge covered the "2v2" label on the Game Select team-size chips. Small chips, tabs and settings categories now keep only the candidate's trim, bar or outline.
- Each candidate keeps its own trim beside the badge:
  - Frame: gold inner trim.
  - Reticle: gold bottom bar.
  - Spotlight: gold outline.
- Focus never uses gold, so "chosen" (gold plus check) stays separate from "where I am" (white or lit) on the same card.

## Review board (rebuilt)
`ui/gallery/affordance_board.rs` now has two pages. `BRAWLER_UI_GALLERY_AFFORDANCE_BOARD=1 just ui-gallery` opens the actions page; the value `choices` opens the other page. B switches pages and V cycles the looks.
- **Actions page:** Play (Primary), Practice (Secondary), and the Map Maker FIT icon button (Dark + `MAP_ACTION`), shown at rest / focus / pressed / disabled. It also has a reduced-motion Practice row and a touch row (Play after a tap, focus hidden).
- **Choices page:** game type card, brawler card, loadout slot, part card and Map Maker PLACE tool, shown at rest / current / focus / current + focus / disabled. Disabled shows "never disabled" for game type cards and tools, which the real screens never disable. The page also has a touch row (brawler card with focus hidden).
- **Why the board doesn't reuse the screen helpers:** they need `FlowButton`/`FlowUiAction` (private to flow) plus profile, catalog and 3D preview data. Each cell mirrors the real role (`Surface::Card`, as `surface_role` and `tool_button` give), the style (`SurfaceStyle::MAP_TOOL` for the tool), the content order and the real icons (mode pennant, part illustrations, `place`). Sizes are scaled to fit 6 rows on the 914x411 viewport, and the brawler card leaves out its 3D weapon preview.

## Evidence
- Board captures: `target/fe7t/capture-board.sh` → `target/fe7t/board/{current,frame,reticle,spotlight}-{actions,choices}-{1280x720,914x411}.png`.
- Live shell captures re-run for all four looks with `target/fe7t/shell.sh` (scratch supervisor 127.0.0.1:5031, data under target/; 12/12 passed): dashboard, settings and game-select in `target/fe7t/shell/`.
- Sheets were rebuilt with `uv run --with pillow python target/fe7t/sheet.py target/fe7t assets/brawler/ui/fonts/lilita-one-regular.ttf` → `target/fe7t/fe7t-{frame,reticle,spotlight}-sheet.png`. Each sheet shows the shipped look on the left and the candidate on the right: both board pages at 1280x720 and 914x411, then the live screens.
- Ticket assets a-000019..21 removed; new assets a-000022 (frame), a-000023 (reticle), a-000024 (spotlight).
- I opened the sheets and checked them:
  - The labels are legible at both sizes.
  - The touch rows show no focus.
  - Action buttons show no current mark in any candidate.
  - On the live Game Select screen, the chosen game card carries the badge and the 2v2 chip shows only its trim.

## Questions
- q-y21kgwz superseded: answered as agent bookkeeping (not an owner choice) and resolved.
- New q-rc8skq9 asks which focus look to use, with each candidate's current mark.
- q-4yrvg6g (touch hides focus) is kept; its wording now uses focus/current and `navigation.focused`.

## Phase 2 notes (for whichever look is chosen)
- Map Maker `palette_arrow_button` sets `current: !disabled` to mean "enabled". That is a misuse, and it would put the current mark on the palette arrows. Replace it with plain enabled styling when implementing.
- Map Maker Clear (`Surface::Danger`) uses current for "armed". Results use current for "your team" banners and for your own row. These are not choices; decide their treatment (likely a distinct armed state, or no mark) when implementing.
- The validation toggle (Primary) loses its open-state rim under the candidates; the open panel itself shows the state.

## Mistakes
- The first badge threshold (34 px) collided with short chip labels. I found it only on the live Game Select capture, because the board has no chip-sized choice. Prevention: check the live screens, not just the board, whenever a shader mark is added.

## Gates
- `just lint`: pass (`target/fe7t/lint.log`).
- `just test`: pass, 2575 tests, 0 failed (`target/fe7t/test.log`). The new test is `review_candidates_never_mark_action_buttons_as_current`. Renamed tests: `current_focus_keeps_choice_and_inactive_ignores_hover`, `focused_category_still_shows_it_is_current`, and `current_and_focus_have_distinct_treatments`.
- Not verified: the animated pulse and lift (the captures are stills), and any check on a device (that is phase 2).
