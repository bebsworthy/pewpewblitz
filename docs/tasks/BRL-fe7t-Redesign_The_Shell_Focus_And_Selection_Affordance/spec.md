# Outcome
Focused and selected shell controls read clearly and look intentional on the BRL-11zt blue arcade surfaces.

# Feedback (2026-11-13)
User playtest of BRL-drny: "the focus/selection ring on the buttons doesn't look great. We may want to think of a better affordance design."

# Current state
BRL-drny (f0ee11c5): focus = cyan accent frame + cyan pointer; selection = lighter blue raised fill + yellow play-highlight rim (`src/client/ui/surface.rs`, `assets/brawler/shaders/shell_surface.wgsl`, docs/design/player-shell/motion.md and ui-toolkit.md).

# To explore
Design options first (mockups in the UI gallery), e.g. lift/scale + glow, inner bevel highlight, animated chevron or bracket corners, outline thickness/contrast; distinct focus vs selection vs pressed; reduced-motion variant; touch (no persistent focus) vs gamepad/keyboard (must be visible). Get user choice before implementing.

# Acceptance criteria
User-approved affordance applied across shell surfaces; focus visible for keyboard/gamepad; selection distinct from focus; reduced motion respected; gallery evidence.
