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
