# Outcome

Fighters look like appealing, readable characters with expressive motion.

# Context

- Every fighter uses Kenney `character-male-a/d` (`src/client/assets.rs:52-54`, `presentation_3d/mod.rs` character scene). The Zombie mode reuses it.
- The animation graph has five clips (idle, walk, holding, shoot, defeated).
- Riff exists as a rigged procedural model in `asset_src/characters/riff/` (with a run action).

# Scope

- **Proportions:** about 45% head, a weapon about 1.5x oversized, and visual height about 1.6 cells, while the authoritative collision radius is unchanged.
- **Animations:** idle, run, per-family hold/shoot with recoil, hit react, dash, defeat, and victory.
- A procedural squash/stretch layer on footfalls, fire, and landing.
- glTF export pipeline documented and reproducible from `asset_src`.
- Riff becomes the default fighter presentation.

# Out of scope

Per-brawler model selection (needs a profile/data decision; follow-up ticket), other roster characters, and cosmetics.

# Acceptance criteria

- Riff renders in gameplay, the dashboard, and the brawler screens with all listed animations driven by existing state/cues.
- Footprint and readability tests pass.
- Visual height is independent of collision.
- Owner visual acceptance.

# Verification

- Asset preparation tests.
- `just test`.
- Native captures and a playtest.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
