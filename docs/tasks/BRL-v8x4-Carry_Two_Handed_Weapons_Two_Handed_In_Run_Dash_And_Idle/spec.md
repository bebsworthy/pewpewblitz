Reviewed 2026-11-15 against HEAD 3437c0d8: valid; spec refreshed with builder/clip refs and current GLB headroom (~74 KB).

# Context (verified)

- BRL-q29d baked two-handed holds only into `combat_idle_<family>` and `attack_<family>` (`asset_src/characters/riff/build_riff.py:362-388`, per-frame IK via `Poser.hold_weapon` l.259 + `sampled` l.319). `run` (l.411-428, one-handed `carry`), `dash`, `idle`, `hit_front`, `victory`, `ultimate` are shared and hold every weapon one-handed by the grip.
- Client: `CLIP_NAMES` (15 clips, `packages/brawler-client/src/presentation_3d/fighter_character.rs:53`), `FighterClips::from_gltf` (l.136) and `node()` (l.171): `Run`, `Dash`, `Idle` are family-independent. `select_motion` (l.267) uses `Hold(family)` when stationary in a match, so in-match standing is already two-handed; the gap is Run, Dash, and the Dashboard/brawler preview which plays plain `idle` with the family weapon attached (`dashboard.rs:569`, `:605`).
- Budget: `GLB_BYTE_BUDGET = 700_000` (`build_riff.py:62`, checked l.674); current `chr_riff.glb` is 625,732 B.

# Plan

1. Builder: add `run_heavy/throwable/melee` (and `dash_*`, `idle_*` if budget allows) by reusing the run/dash/idle leg+body keys merged with the family's `weapon` placement and per-frame IK support hand; 1h keeps the shared clips. Measure GLB size per added clip; if over budget, drop idle variants first (Dashboard can use `combat_idle_<family>` instead).
2. Client: extend `CLIP_NAMES`/`FighterClips` with per-family run (and dash) nodes; `CharacterMotion::Run`/`Dash` carry the family; Dashboard preview picks family idle (or `combat_idle_<family>`) for two-handed families.
3. Update `asset_src/characters/riff/README.md` clip list and docs/11 animation notes.

# Acceptance

- Two-handed weapons keep both hands on them while running, dashing and in the Dashboard preview (builder grip error under the existing threshold for new clips).
- GLB ≤ 700 KB; `just test` (clip-contract tests) and `just lint` green.
- Native captures and owner visual acceptance.
