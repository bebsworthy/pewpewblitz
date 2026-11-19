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


# Owner decision 2026-11-19
- **Look:** keep the current cyan focus frame + pointer and the gold current rim. Frame, Reticle and Spotlight are rejected: remove their code, the `BRAWLER_SURFACE_AFFORDANCE` switch and the review board. Keep the `current`/`focused` rename.
- **Touch (q-4yrvg6g):** focus visible by modality, the recommended option. Focus hides after a touch press and shows again on the next keyboard or gamepad navigation event. Mouse hover/press keeps today's behaviour. Navigation state is unchanged.
- **New scope, spacing bug the owner reported (screenshots in the ticket assets):** the focus frame and current rim are drawn inside the face, but content padding doesn't reserve their width. As a result:
  - "PRACTICE" sits low (it is centered on the whole box including the bottom lip, not on the visible face);
  - the frame/pointer crosses the FIT icon;
  - on the loadout slot the rim has no padding and runs under the "1" slot number.
- **Fix in the shared surface component:**
  - a fixed ring band (frame width + gap), reserved in every state so content never shifts;
  - content centered on the visible face above the lip;
  - pointer space mirrored on both sides, so centered content stays centered.
- **Checks:**
  - a test that content never overlaps the ring band;
  - before/after captures of Dashboard, Game Select, loadout/equipment, Map Maker, Settings, Results at 1280x720 and 914x411;
  - a Pixel 8a check, including focus hiding after a tap.

# Phase 2 implementation, 2026-11-19

Feedback handling for "Owner decision 2026-11-19":
- Keep the current look, remove Frame/Reticle/Spotlight: **implemented**.
- Touch hides focus (q-4yrvg6g option 1): **implemented**.
- Spacing bug (ring band, centring on the visible face, mirrored pointer space): **implemented** in the shared surface component.
- Phase 2 notes: `palette_arrow_button` misuse **implemented** (plain enabled styling). Map Maker Clear "armed" and Results team banners / own row: **left as they are**. The rename didn't change their behaviour, and under the kept current look they still show the gold current rim, as before phase 1.

## Removed (rejected candidates)
- `SurfaceAffordance`, the `BRAWLER_SURFACE_AFFORDANCE` switch, `marks_current`, `ReducedMotionPreview`, the focus lift (`ease_lift`, `FOCUS_LIFT*`) and the focus pulse (`motion.w`).
- The candidate shader branches in `assets/brawler/shaders/shell_surface.wgsl`: `corner`, `band`, `segment`, `current_badge` and the variant switch. `flags.w` is now unused.
- `ui/gallery/affordance_board.rs`, its `BRAWLER_UI_GALLERY_AFFORDANCE_BOARD` switch and the `GalleryBoard` single-shot capture.
- The two candidate tests, and the review-board paragraph in `docs/design/player-shell/ui-toolkit.md`.
- `BRAWLER_UI_GALLERY_SIZE=WxH` is kept: it is generic gallery sizing.
- The `current`/`focused` rename is kept. The shader uniform `selection` is now `current_mark`.

## Ring band (spacing fix), `ui/surface.rs`
- `ContentReserve` gives the minimum content padding, in UI px from the node edge, that keeps content off everything the shader draws inside the face in any state. It mirrors the shader; a comment on both sides says to change them together. It reserves:
  - top: 1 px inset + border + 9 (the focused current rim, the deepest ring) + `RING_GAP` 2. That is 17 px for the default 5 px border.
  - bottom: the top value + lip + 1, so content centres on the visible face, not the box (the PRACTICE fix).
  - left: the larger of
    - the ring band plus the slant over the content's height (`slope * (face_h - band)`), and
    - the pointer's right tip at its widest row (`slope*(mid+pointer) + border + 10 + pointer`) + `POINTER_GAP` 4.
  - right: the left value, mirrored, + lip x + 1, so centred content stays centred.
- The pointer now starts at `border + 10` (unchanged for 5 px borders) and is `clamp(face_mid - border - 7, 6, 12)` wide, so it stays inside the frame on short faces. The look is otherwise unchanged.
- `reserve_surface_content` (PostUpdate, chained after `bind`, before `UiSystems::Layout`) raises each surface's padding to the reserve. `SurfacePadding` keeps the owner's padding as the base:
  - Owner padding counts from the face and only adds: `max(owner, reserve)`, plus the lip on the bottom and right.
  - If the owner later writes a new padding, it becomes the new base.
  - The slant term is sized from the previous layout, so a new surface settles on its second frame. The entrance animation covers that frame.
- The padding is the same in every state, so focus and current never shift content.
- `Geometry::of(role, style)` is the single source of slope, border, cut and lip. Both `present` and the reserve use it.
- Back's `min_width` now adds the reserve (`BACK_NOMINAL_SIZE`) instead of 48 px.

## Size adjustments for the reserve
Every adjustment was found by the new layout check or in the captures:
- Equipment header Back, Brawlers Back, Practice Maps Back and Start Practice: `height` changed to `min_height`, so the label never overflows.
  - Equipment Back also gets `align_self: Center` and its own small vertical padding, because the card padding from `button()` doubled its height.
- Brawler list card: the preview frame may shrink (`min_height: 0`, `flex_shrink: 1`), so a name that wraps never pushes the label out. "CREATE\nBRAWLER" is now 28 px.
- Dashboard VIEW BRAWLERS label: no wrap. The column widens instead of the label breaking into two lines.
- Map Maker palette:
  - cards `min_width: 0`, so they shrink with the tray as before;
  - the preview is `width: 100%`, `max_width: 168` at the 168:124 aspect, so it fits inside the band;
  - the "1–6 OF 17" counter is `flex_shrink: 0` (it was pushed off screen in one capture).
- `palette_arrow_button`: `current: !disabled` removed.

## Touch hides focus, `ui/focus_visibility.rs`
- `FocusVisibility { visible }` is a resource. `track_focus_visibility` runs in PreUpdate after `InputSystems` and is installed by `SurfacePlugin`.
  - A `TouchInput` `Started` hides focus. A touch wins any frame it occurs in.
  - A key press of an arrow or Tab, any gamepad button press, or a mouse button press shows it again.
  - Text keys and Enter don't count, because the Android on-screen keyboard sends them while the player types after a tap.
- `present` draws the frame and pointer only when `shows_focus(state, visibility)`. `FlowNavigation::focused`, `SurfaceState::focused`, the current rim and press feedback are untouched, so a gamepad resumes from the same control.
- Docs: `docs/design/player-shell/ui-toolkit.md` now describes the Focused state by modality and adds a paragraph on the ring band.

## Tests
- `ui::focus_visibility::tests::touch_hides_focus_until_keyboard_gamepad_or_mouse_navigation`: an App test covering touch, typing, arrow, gamepad and mouse.
- `flow::tests::touch_hides_focus_and_navigation_resumes_where_it_was`: on the phone layout flow app, a touch hides focus while `FlowNavigation::focused` and the focused `SurfaceState` stay on the same control; the next arrow shows focus and moves on from it.
- `ui::surface::tests::content_reserve_clears_frame_rim_pointer_slant_and_lip`: for every role and map style at three sizes, the reserve covers the rim + gap, the pointer tip + gap and the slant, and mirrors the lip.
- `ui::surface::tests::ring_band_padding_is_the_same_in_every_state_and_keeps_owner_padding`: the padding is identical at rest, focused, current, focused + current and disabled; owner padding below the band is raised, larger owner padding is kept and the lip is added.
- `ui::surface::tests::hidden_focus_keeps_the_focused_state_and_current_choice`.
- Layout check `ui::layout_harness::surface_ring_intrusions`: every visible in-flow child of a laid-out surface must lie inside its content box. Tolerance is 1.5 px for per-node pixel rounding. It is used by:
  - `flow::tests::shell_surface_content_stays_clear_of_the_ring_band`: Dashboard, Weapon equipment, Brawlers, Choose Game and Practice maps at 868x411, 914x411 and 1280x720;
  - `shell::tests::settings_and_controls_fit_a_landscape_phone`: all four Settings categories at 868x411.

## Verification
- `just lint` (ripgrep on PATH): pass, `target/fe7t/phase2/lint.log`.
- `just test`: exit 0, `target/fe7t/phase2/test.log`. The final run overlapped a second `just test` writing the same log, so the log is interleaved; both runs exited 0, with 0 failed in the log.
- Captures: `target/fe7t/phase2/capture-retry.sh` is a copy of `scripts/shell-capture.sh`, including the BRL-g7ny background-window launcher for the final runs. It takes a comma-separated route list and retries a route whose PNG comes out black, up to 4 times; native captures here often come out black, at HEAD too. Scratch supervisor on 127.0.0.1:5032, data under target/.
  - Final after runs: 1280x720 all routes, then 914x411 for first-brawler, dashboard, settings, brawler-list, weapon-equipment, weapon-parts, game-select, practice-maps, map-library, map-editor and map-editor-palette.
  - before = HEAD cb0a5aaf, built from a detached worktree `target/fe7t/head-src`: `target/fe7t/phase2/before/<size>/<route>.png`.
  - after = working tree: `target/fe7t/phase2/after/<size>/<route>.png`.
  - Side-by-side pairs (before left, after right): `target/fe7t/phase2/pairs/<route>-<size>.png`. Made by `uv run --with pillow python target/fe7t/phase2/pairs.py target/fe7t/phase2`.
  - Key pairs: `dashboard-1280x720` (PRACTICE centred on the face), `weapon-equipment-914x411` / `-1280x720` (slot "1" clear of the rims), `map-editor-*` (tool row), `settings-*`, `game-select-*`.
  - Missing before images: HEAD's `dashboard` at 914x411 came out black on every attempt. The owner's screenshot 1 covers that state.
- Not captured: Results. There is no `--shell-capture` route for it, and reaching it on the device was cut short when adb lost its daemon. The Results banner and row treatment is unchanged code (gold current rim on the local team and own row).
- Not directly captured on the desktop: focus on the Map Maker FIT button. The owner's FIT case is covered by the ring reserve test for `MAP_ACTION`, and the tool row in the Map Maker captures shows no crossing.
- Pixel 8a (45171JEKB03025): installed with `just android-install 192.168.31.47:5000` against the existing port-5000 server. Screenshots in `target/fe7t/phase2/device/`:
  - `01-launch`: focus visible on CONNECT before any touch.
  - `02-after-connect`: after the tap, no cyan frame on the Dashboard.
  - `03-brawlers`: gold current rim, no focus.
  - `05`/`14-equipment`: loadout. Slot 1 has the gold rim, "1" sits clear of it, and there is no frame after the tap.
  - `06-tap-slot2`: the part picker after a tap, with no frame.
  - `07-dpad-right`: `adb input keyevent DPAD_RIGHT` brings back the cyan frame and pointer on the next part, clear of its text.
  - `11`: Connecting, then Cancel with no frame.
  - `15-dash`: Android Back keeps focus hidden.
  - `16-practice-maps` and `19-menu` (match menu): no frame after taps.

## Known limits and observations
- Buttons with a 22–24 px label are now at least about 66–70 logical px tall: the 41 px ring band plus the line. Examples are the equipment header Back, which is visibly larger than before (it used to clip its label inside a fixed 66·factor height), and the Brawlers Back. If the owner wants these smaller, the next step is a smaller label or a slimmer band for Back. Narrow cards wrap long names sooner, for example "BRAWLER 1" on the 868-wide phone list.
- On the device, Brawler details' DELETE BRAWLER (touch-target floor) sits flush with the bottom edge. I didn't compare this with the previous APK.
- Map Maker Clear (armed) wasn't exercised in a capture.

## Mistakes
- I built the HEAD "before" binaries from a git worktree into the shared `CARGO_TARGET_DIR`. The workspace crates got the same unit hashes, so the next main-tree build reported "fresh" and reused the HEAD binary. One whole "after" set showed no change. Cause: path-only differences don't change cargo's metadata for workspace members. Prevention: build a comparison tree into its own target dir. Recovered with `cargo clean -p brawler-client -p brawler -p brawler-routing` (release and dev), then a rebuild.
- A parent-session capture briefly shared port 5032, and two of my own capture runs overlapped on that port and log; one rewrote the retry script while bash was still reading it. All affected captures were re-run. Prevention: one capture run at a time, each with its own log.
- Early capture runs launched clients in the foreground and took macOS focus. Since BRL-g7ny, captures go through `scripts/lib/background-window.sh`.

# Phase 2 rework after parent review, 2026-11-19

Review feedback: the first ring band (41 px of padding, sized from the node's own computed size) made buttons bigger and made layout circular. The rework below supersedes the "Ring band (spacing fix)" and "Size adjustments" sections above.

## Rings fit the existing space
- `shell_surface.wgsl`: every ring now stays within `border + 2` px of the edge.
  - Focus turns the border cyan behind a 1.5 px ink edge, with an ink keyline just inside it.
  - The current rim is a 3 px gold band straddling the inner edge of the border. It replaces the keyline when the surface is also focused.
  - The pointer starts at `border + 3` and is `clamp(face_mid - border - 4, 4, 8)` wide (was 12).
  - The look stays cyan frame + pointer and gold rim, only thinner and closer to the edge.
- `ContentReserve` is now a small fixed inset that never depends on the laid-out size, so it can't feed back into layout:
  - every side: `1 + border + 2 + 1 gap`, which is 9 px for 5 px borders;
  - left and right, on focusable surfaces only (`Has<Interaction>`): the pointer reserve `1 + border + 3 + 8 + 2 gap`, 19 px; mirrored on the right; no slope term;
  - bottom and right: plus the lip.
- `reserve_surface_content` keeps the owner's padding total where it already clears the rings: half the lip moves from the top padding to the bottom, so content centres on the visible face. Otherwise it raises each side to the reserve.
- Screen changes:
  - Reverted all the height-to-min_height and label-size changes from the first pass.
  - Kept: the brawler-list preview may shrink when a name wraps; Map Maker palette preview is `width: 100%`, `max_width: 168` at its aspect, so the 180 px card keeps its width; `palette_arrow_button` no longer sets current.
  - New: the Dashboard PLAY label uses `LineHeight::RelativeToFont(1.0)`. The glyphs keep their size and centre, and the button keeps its height.
- Docs: the `ui-toolkit.md` paragraph describes the new fixed inset.

## Tests (updated and new)
- `content_reserve_clears_rings_pointer_and_lip_with_a_small_fixed_inset`: for every role and style, the band clears the rings plus gap; buttons also clear the pointer tip plus gap; the inset is ≤ 10 px above content and ≤ 20 px beside a pointer.
- `ring_band_padding_is_the_same_in_every_state_and_keeps_the_owner_total`.
- `shell_surface_content_stays_clear_of_the_ring_band`: unchanged intent, now on the shared `walk_ring_band_screens` walk.
- New `ring_band_keeps_button_and_card_outer_sizes`: Dashboard, Weapon equipment, Brawlers, Choose Game and Practice maps at 868x411, 914x411 and 1280x720.
  - Each screen is laid out with and without the reserve. Without it, the layout matches pre-fe7t.
  - Every visible flow button's outer size must stay within 3.5 px. The largest delta is Dashboard PLAY at the phone sizes, +3.1 px; everything else is ≤ 1.6 px.
  - The loadout header Back must not move at all (±0.5). It is 53 px at 1280x720, a fixed `66 × factor` bar in both.
  - Back widths are excluded, because `bind` floors them from the label in the client.
- The touch-hides-focus tests are unchanged.

## Verification
- `just lint`: pass. `just test`: exit 0, 2580 passed, 0 failed (`target/fe7t/phase2/test.log`).
- Captures: `target/fe7t/phase2/capture-retry.sh`, a copy of `scripts/shell-capture.sh` with the BRL-g7ny background-window launcher and route lists. Scratch supervisor 127.0.0.1:5032.
  - Routes: first-brawler, dashboard, settings, brawler-list, weapon-equipment, game-select, map-library, map-editor, map-editor-palette, at 1280x720 and 914x411. No black frames.
  - Pairs, before (HEAD cb0a5aaf) left and after right: `target/fe7t/phase2/pairs/<route>-<size>.png`.
  - I checked them. Button and card sizes match before. PRACTICE sits centred on the face. The loadout "1" is clear of the thin rims. Settings rows and tabs are unchanged in size. The Map Maker palette row now fits the tray at 1280x720, where HEAD overflowed and cut the left arrow.
- Pixel 8a: see the device section below.

## Device (rework build)
- Pixel 8a: `just android-install 192.168.31.47:5000`. Screenshots in `target/fe7t/phase2/device/rework/`:
  - `02b-dashboard`: after tapping Connect, no focus frame; PLAY, WIPEOUT and PRACTICE at their old sizes.
  - `05b-equipment`: the loadout header Back is its old slim bar; slot 1 has the thin gold rim with "1" clear of it; no frame after the tap.
  - `06-dpad`: `adb input keyevent DPAD_RIGHT` shows the cyan frame and the small pointer on slot 2, clear of its content.

## Mistakes (rework)
- The first pass sized the band for the deepest ring the old shader drew, plus a slant term taken from the node's own size. That made buttons grow, made layout circular, and I covered it with per-screen min_height changes. Cause: I fixed content to fit the rings instead of fitting the rings to the space. Prevention: when a visual affordance can't fit, change the affordance first, and pin outer sizes with a before/after layout test (now `ring_band_keeps_button_and_card_outer_sizes`).
