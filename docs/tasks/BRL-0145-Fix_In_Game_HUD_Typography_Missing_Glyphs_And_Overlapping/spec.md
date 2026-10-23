# Outcome

In-match text uses the game's font, never shows missing-glyph boxes, and stays legible in close fights.

# Context

- `src/client/hud.rs` (4 uses) and `src/client/presentation.rs` (3 uses) create text with `TextFont::from_font_size`, Bevy's default monospace font, instead of `theme::font` (Lilita One, `src/client/ui/theme.rs:5`).
- Missing glyphs render as boxes. The live 2026-10-23 capture shows `T1 1 ▯ 3 T2` (em dash at `hud.rs:607`, `:621`; also `:853`) and `READY ▯`.
- The live capture also shows overhead labels for two nearby fighters ("Bot 1" and "Brawler-0001") overlapping until neither is readable.
- `hud.rs:528-550` assigns `format!(...)` to text every frame without a change check, which forces relayout.

# Scope

- Route all in-match HUD and overhead text through the theme font, with a consistent stroke/shadow treatment.
- Replace unsupported characters. Add a test that every HUD/overhead string template uses only glyphs present in the shipped font.
- Overhead label collision handling: offset or stack nearby labels, and prioritize the local fighter's plate.
- Write HUD text only when the value changes.

# Out of scope

HUD layout redesign and new HUD elements (touch-first HUD ticket). BRL-0133 recorded an instruction not to touch the in-game HUD for that delivery; confirm with the owner before starting this ticket.

# Acceptance criteria

- No missing-glyph boxes in any HUD state for Wipeout, Hot Zone, Heist, and Zombie.
- With two fighters at minimum separation, both overhead labels remain readable.
- HUD text components are marked changed only when their content changes (ECS test).

# Verification

- The glyph-coverage test.
- An ECS change-detection test.
- `just test`.
- Native captures of all modes.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
