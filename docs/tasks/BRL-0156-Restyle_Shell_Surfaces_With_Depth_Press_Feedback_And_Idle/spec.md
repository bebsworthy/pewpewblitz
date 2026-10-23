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

# Note from BRL-0140

When the palette swaps, also update the legacy color wording in `docs/13-player-ux.md` (cream/orange/teal action descriptions near the pause menu, confirmation screens and the ability picker) to the token roles in `docs/design/player-shell/README.md`. Reproducible token and surface reference: `asset_src/art-direction/compose.py` and the BRL-0140 board.
