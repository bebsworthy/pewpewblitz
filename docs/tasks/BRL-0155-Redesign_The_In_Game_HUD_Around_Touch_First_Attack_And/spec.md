# Outcome

The in-match HUD is graphical, readable at a glance, and ready for mobile touch controls.

# Context

- `src/client/presentation.rs:16-52` spawns "HEALTH --/--" and "WEAPON --/--\nULT --" text boxes. `hud.rs` builds the timer, objective, and scoreboard as formatted strings.
- The HEALTH box duplicates the overhead plate. "ULT 69% charging" is text.

# Scope

- Bottom-right attack button and ultimate button with a radial charge fill and a ready pulse. Desktop shows key/controller hints on them.
- A bottom-left reserved zone for the touch joystick.
- Remove the HEALTH box. Ammo moves under the overhead plate.
- A top-center plate for timer, team scores, and mode objective state with team colors, for Wipeout, Hot Zone, Heist, and Zombie.
- A kill feed.
- Safe-area-aware layout that scales with resolution.
- Uses the shell UI toolkit and theme font, and follows the art direction decision.

# Out of scope

The touch input implementation itself (touch controls ticket); this ticket provides the layout and visual controls.

# Acceptance criteria

- All four modes show complete state without any text-only HUD boxes.
- Layout holds at 16:9, 2.21:1, and 19.5:9 with safe-area insets.
- The ultimate charge is readable at a glance.
- The kill feed respects concealment and relevance rules.
- Owner playtest acceptance.

# Verification

- Layout tests where feasible.
- `just test`.
- Native captures of every mode and aspect.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
