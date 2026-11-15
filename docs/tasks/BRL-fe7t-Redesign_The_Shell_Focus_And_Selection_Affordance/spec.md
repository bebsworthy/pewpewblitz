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
