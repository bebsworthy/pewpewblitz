# Outcome

Players can read health and ammo at a glance and see exactly where their weapon will reach.

# Context

- Overhead health bars are 11 px (`presentation_3d/combat/fighter_ui.rs:257`). The ammo row is 7 px. Names fall back to placeholders such as "Brawler-9A8C".
- The aim preview (`presentation_3d/combat/aim_preview.rs`) shows a thin 1 px line in captures.

# Scope

- About 2x thicker health plates with a dark outline, stroked numbers, and team-colored fill.
- Ammo segments beneath the plate for the local fighter.
- An aim lane/cone derived from the resolved loadout (range, projectile width, spread) for primary fire, plus a distinct ultimate preview.
- Distinct styling for local, ally, and enemy plates.

# Out of scope

The HUD layout redesign and font fixes (separate tickets).

# Acceptance criteria

- Aim lane geometry comes from resolved definitions. A test covers the width/range/spread mapping for each delivery family.
- Plates stay readable at the chosen camera framing in 3v3.
- Concealment rules are respected for plates.

# Verification

- Focused tests.
- `just test`.
- Native captures.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
