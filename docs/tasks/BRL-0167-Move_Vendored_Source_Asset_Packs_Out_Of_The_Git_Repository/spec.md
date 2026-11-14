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


# Inventory and decisions (2026-11-14)

## Finding: `external_assets/` was never tracked

- `git ls-files external_assets` is empty and `git log --all -- 'external_assets/*'` has no
  commits. `/external_assets/` has been in `.gitignore` since `d084938b` (2026-08-20, V3 cutover).
  The ~100 MB on disk (Kenney/KayKit packs, Lilita One, map drawings, screenshots) never entered
  history, so there is nothing to `git rm --cached` and nothing to rewrite. The "untrack" and
  "add to .gitignore" acceptance criteria are already satisfied; this ticket's remaining work is
  the human-readable manifest and confirming nothing reads `external_assets/`.

## Shipped third-party assets (all under `assets/`)

| Pack | Version | Source | License | Shipped files |
|---|---|---|---|---|
| Kenney Mini Characters | 1.0 | https://kenney.nl/assets/mini-characters | CC0-1.0 | `brawler/models/kenney/mini-characters/character-male-d.glb`, `.../Textures/colormap.png` (Zombie NPC) |
| Kenney Blaster Kit | 2.1 | https://kenney.nl/assets/blaster-kit | CC0-1.0 | `brawler/models/kenney/blaster-kit/blaster-a.glb`, `.../Textures/colormap.png` (blaster on Riff) |
| Kenney Sci-fi Sounds | 1.0 | https://kenney.nl/assets/sci-fi-sounds | CC0-1.0 | `brawler/audio/fire.ogg`, `defeat.ogg` |
| Kenney Impact Sounds | 1.0 | https://www.kenney.nl/assets/impact-sounds | CC0-1.0 | `brawler/audio/impact.ogg` |
| Kenney Interface Sounds | 1.0 | https://kenney.nl/assets/interface-sounds | CC0-1.0 | `brawler/audio/ready.ogg`, `error.ogg` |
| Lilita One (Juan Montoreano) | Google Fonts release | https://fonts.google.com/specimen/Lilita+One | OFL-1.1 | `brawler/ui/fonts/lilita-one-regular.ttf` |
| Fira Mono subset (Mozilla / Telefonica) | bevy_text 0.19.1 | embedded by the `bevy/default_font` feature | OFL-1.1 | none under `assets/` (compiled into the binary); license `assets/licenses/FiraMono-LICENSE` |

Each matches an `assets/manifest.ron` row and an `assets/licenses/` text. The four model/texture
files and the font are byte-identical (`cmp`) to the copies in `external_assets/`. The three
Kenney sound packs are not present in `external_assets/` at all.

## Not shipped (local source only)

KayKit Block Bits 1.0, Kenney Graveyard Kit 5.0, Mini Arena, Mini Dungeon, Mini Forest 1.0
(retired by BRL-0187/0188), Kenney Game Icons (retired by BRL-0157), Kenney Emotes Pack, Mini
Arcade, Pirate Kit, Furniture Kit (never shipped), plus `map_images/` and screenshots (design
references). Everything else under `assets/` is original project work provenanced by
`asset_src/` and the `pewpew-blitz-original-*.txt` notes (environment kits, Riff, icons,
original audio, shell art, branding, shaders).

## Readers of `external_assets/`

None in `src/`, `tests/`, `build.rs`, `justfile`, `scripts/`, or `asset_src/`. Only prose
references remain in `docs/11-art-and-presentation-direction.md` and
`docs/16-grid-map-asset-system.md` (design-reference drawings), which are accurate.

## Decisions

- Manifest location: `assets/licenses/THIRD_PARTY.md`, beside the license texts it indexes.
  `assets/manifest.ron` stays the machine-checked per-file inventory; the Markdown file is the
  human-readable pack summary with re-acquisition steps, and points at `manifest.ron` as the
  authority. Add a row for it to the provenance table in docs/11.
- No fetch script: Kenney pack pages and the Google Fonts specimen are not stable direct-download
  URLs (Kenney downloads are hashed media links behind the page). Manual steps are documented.
- Credits mismatches go to BRL-0189, not here.


# Implementation and verification (2026-11-14)

## Changes

- Added `assets/licenses/THIRD_PARTY.md`: shipped packs (version, author, license, source URL,
  license text), every shipped third-party file mapped to its `assets/` path and pack original,
  retired/never-shipped packs, and manual re-acquisition steps.
- `docs/11-art-and-presentation-direction.md`: added the file to the asset-provenance table.
- No `.gitignore` or index change: `external_assets/` was already ignored and never tracked.
  `git ls-files external_assets` = 0, `git check-ignore` matches `.gitignore:3`, and the
  directory is still on disk (100 MB).
- Credits mismatches recorded in the BRL-0189 spec.

## Verification

- `just lint` (VS Code ripgrep first on PATH): pass.
- `just check`: pass.
- `just test`: pass, 2305 passed, 0 failed.
- Manifest review: each shipped row matches `assets/manifest.ron` and an `assets/licenses/` text;
  the model, texture, and font files are byte-identical to the `external_assets/` sources.

## Deferred / notes for the owner

- History rewrite: not needed, since the packs never entered history.
- `assets/brawler/ui/shell/logo.png` is recorded in `asset_src/player-shell/README.md` as an
  AI-generated (OpenAI) refinement. It is project artwork, not a third-party pack, so it is out of
  scope here, but the owner may want to review it against the "no AI-generated" provenance used
  for the newer original sets.
- The three Kenney sound packs are not present in `external_assets/`; they would need to be
  re-downloaded to rebuild those five clips.

## Learnings

- The ticket premise (100 MB tracked) came from a review that looked at the working tree, not the
  index. Check `git ls-files` and `git check-ignore` before scoping "remove from the repository"
  work.
