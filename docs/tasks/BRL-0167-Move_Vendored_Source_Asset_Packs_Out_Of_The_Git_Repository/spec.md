# Outcome

The repository stays lean, and asset provenance is still reproducible.

# Context

`external_assets/` holds about 100 MB of unpacked Kenney/KayKit packs and screenshots. Shipped assets already live under `assets/`, with licenses in `assets/licenses/`.

# Scope

- A manifest (pack, version, URL, license, files used).
- Remove `external_assets/` from tracking and add it to `.gitignore`.
- Optionally provide a fetch script.

# Out of scope

Rewriting git history (needs an explicit owner decision).

# Acceptance criteria

- The manifest covers every shipped third-party asset.
- `external_assets/` is untracked.
- Builds and tests are unaffected.

# Verification

`just check`, plus a review of the license manifest.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
