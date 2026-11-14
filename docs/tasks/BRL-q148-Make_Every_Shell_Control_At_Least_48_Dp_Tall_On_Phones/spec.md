# Context (found in BRL-rs10, 2026-11-14, Pixel 8a)
BRL-rs10 raised only Settings **slider** rows to 76 UI px (48.8 dp at the 411/640 phone fit with the
default 100% UI scale). Other shared `spawn_control` rows keep `min_height: 56` UI px, which is about
36 dp on the phone (toggles, binding chips, action rows, category tabs). Slider rows also fall below
48 dp when the player chooses a UI scale under 100% (80% gives ~39 dp).

# Scope
- Decide one phone touch-target policy for shell rows (for example a minimum height derived from
  the effective `UiScale` on touch platforms, or taller rows everywhere) and apply it through the
  shared UI kit (`src/client/ui/controls.rs`, `surface.rs`) rather than per screen.
- Keep desktop layouts readable; re-run the BRL-30za phone reachability tests.

# Acceptance criteria
- A layout-harness test asserts every visible shell `Button` is >= 48 dp tall at 868x411 with the
  default UI scale, and states the policy for reduced UI scale.
- Pixel 8a screenshots of Settings, Server Select and Dashboard.
