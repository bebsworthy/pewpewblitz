# Outcome

Fighters are clearly readable characters rather than specks, and the framing is chosen by playtest evidence.

# Context

- `presentation_3d/camera.rs:5-9`: 15-degree vertical FOV, 62-degree elevation, 1,495-unit distance. This is nearly orthographic and shows about 14 cells vertically. `docs/11-art-and-presentation-direction.md:52-58` locks these values.
- Fighters occupy roughly 5% of screen height in captures, and their faces are not visible.

# Scope

- Prototype at least three framings, for example FOV 25-35 degrees at 50-55 degrees elevation with 11-14 visible cells.
- Evaluate at the native 2.21:1 aspect and a phone aspect.
- Criteria: fighter readability, threat visibility at the longest authored weapon range, objective visibility, and motion comfort.
- Choose one. Update the framing constants, the framing tests, and `docs/11`.

# Out of scope

Camera smoothing and shake (camera rig ticket) and map size changes.

# Acceptance criteria

- The chosen framing is documented with comparison captures and rationale.
- The longest-range authored weapon's full range from the fighter is on screen in the primary aim directions, or an explicit decision explains the exception.
- Framing tests are updated and passing.
- Native playtest acceptance.

# Verification

- `just test`.
- Native captures and a playtest.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
