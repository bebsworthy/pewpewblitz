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

# Implementation (2026-10-23)

Owner direction: the parent session assigned this ticket explicitly, which satisfies the BRL-0133 confirm-with-owner note. Layout was not redesigned.

- `src/client/ui/theme.rs`: added `TYPE_STROKE` (#0E1638) and `display_font_handle` (Lilita One, or Bevy's default font when a headless schedule test has no `AssetServer`). Added `hud_text(font, size)` → `(TextFont, TextShadow)`. Bevy 0.19 UI text has no outline stroke, so the stroke is approximated by a navy drop shadow scaled to the type size. Added the test-only `display_font_has_glyph`, which reads the shipped TTF cmap with `skrifa` (dev-dependency pinned to `=0.44.0`, already in the lockfile).
- `src/client/hud.rs` (readiness/timer, objective, countdown, phase overlay), `src/client/presentation.rs` (health, weapon/ult, scoreboard), and `presentation_3d/combat/fighter_ui.rs` (overhead name and health) now create text through `hud_text`. The overhead text's old black 1.5 px shadow became the navy treatment.
- Glyphs: once the HUD uses Lilita One, the em dash (`—`) and middle dot (`·`) render correctly. No template change was needed, and the coverage test now guards against regressions.
- Change detection: `update_readiness_hud` writes through `set_retained_text`, and `src/combat/client/hud.rs::update_combat_hud` writes through `set_if_neq`. Both use `set_if_neq` for `Text` and `Visibility`. The overhead name `TextColor` also uses `set_if_neq`.
- Overhead collisions: `project_fighter_overhead_ui` collects every projected plate, then `resolve_overhead_overlaps` places them. The local fighter keeps its anchor. The other plates go in a stable order (lowest on screen first, then entity) and each stacks directly above any plate it would overlap (120 px plate width, 2 px gap). The loop is bounded.

# Verification

- `cargo test --lib -- hud_ fighter_ui`: pass. New tests:
  - `hud_and_overhead_string_templates_use_only_display_font_glyphs`: scans the string literals of the four HUD/overhead sources against the Lilita One cmap, and self-checks the scanner and the missing-glyph detection (U+25AF).
  - `retained_hud_text_changes_only_when_its_content_changes`: an ECS test. A second `update` leaves the `Text` and `Visibility` change ticks of the timer, objective, and scoreboard unchanged. Advancing the clock changes only the timer text.
  - `nearby_overheads_stack_without_overlap_and_keep_the_local_anchor`: three plates at minimum separation do not overlap, the local plate keeps its anchor, and distant plates are untouched.
- `just lint`: exit 0.
- `just test`: exit 0, after one fix. The first run failed 4 presentation_3d schedule tests because `setup_3d_foundation` required `Res<AssetServer>`; the fix made it `Option<Res<AssetServer>>`.
- Native evidence: four Practice runs at 1280x720 (2x Retina captures), each with `BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_COMBAT=1`. They used a scratch copy of `scripts/v3-render-evidence.sh` with only the mode allow-list extended to `zombie`, since the canonical script rejects zombie. Runs: wipeout-3v3, hot-zone-3v3, heist-3v3, and zombie-2p, with captures under the session scratchpad `evidence/<game-type>.txt.captures/`.
  - All HUD text renders in Lilita One with no missing-glyph boxes: `T1 0 — 0 T2`, `T1 0% — 0% T2 / EMPTY`, `DEFEND T1 … / ATTACK T2 …`, `ZOMBIES 8 / BOOSTS 0 / +0%`, the roster, `READY · AMMO 30%`, and the overhead names.
  - Report results: hot-zone passed. Wipeout, heist, and zombie failed only the locked frame-time threshold, with isolated spikes under a concurrent build (p50 16.7 ms, p95 under 19.3 ms; max 29, 86, and 116 ms). That is unrelated to this text-only change.
  - The `Capture saved:` toast still uses the default monospace font. It is evidence tooling, not match HUD, so it is out of scope.

# Remaining (keeps the ticket in doing)

- The native captures never showed two fighters at minimum separation, so the overlap criterion is covered only by the unit test. A heist capture also shows an enemy plate passing under the top timer panel. That is plate-versus-HUD overlap, not plate-versus-plate, and belongs to the HUD layout redesign (BRL-0155).
- Playtest request: `just run`, then Practice → Verdant Crossfire Wipeout 3v3 (also try Heist 3v3 and Zombie 2p). Walk into a bot until the bodies touch and stand together in a cluster of 3 or more. Expected: your green-named plate stays on your fighter, the other plates stack upward without overlapping, and every name and health number stays readable. Also check the Waiting ("GET READY"), countdown, and Completed overlays for missing-glyph boxes, and judge whether the navy shadow reads acceptably as the approved stroke.
- Deferred: BRL-0168 covers player names that use scripts outside Lilita One.

# Learn from errors

- Mistake: I added `Res<AssetServer>` to `setup_3d_foundation` and broke 4 headless schedule tests that build the foundation without asset plugins. Cause: I checked only direct callers and missed tests that schedule the system indirectly. Prevention: when adding a system parameter to an existing startup or reconcile system, use `Option<Res<_>>` for resources that headless tests may omit, or run the owning module's full test set before `just test`.
- Mistake: my first raw-string guard in the source scanner (`concat!("r#", "\"")`) contained the very pattern it searched for. Lesson: a test that scans its own source file must not spell the pattern it detects.
