# Context

Found by BRL-wpr8. Credits now lists original PewPew Blitz work and its tests require each entry to
point at existing provenance. Weapons, environment kits, floor decals, icons, and original audio
have notes in `assets/licenses/`; two shipped original families do not:

- Riff (`assets/brawler/models/original/characters/riff/chr_riff.glb`): provenance lives only in
  `asset_src/characters/riff/` (README, builder, versions).
- Shell branding and menu art (`assets/brawler/ui/branding/`, `assets/brawler/ui/shell/`):
  provenance lives in `asset_src/player-shell/README.md`.

BRL-wpr8's `ORIGINAL` table in `packages/brawler-client/src/shell/credits.rs` therefore points
those two entries at their `asset_src/` directories.

# Scope

- Add `pewpew-blitz-original-characters.txt` and `pewpew-blitz-original-shell-art.txt` (names
  open) that state files, builder/source, and provenance factually, matching the existing notes.
- Point the two Credits entries at the new notes.

# Acceptance criteria

- Every shipped original asset family has a note; the BRL-wpr8 Credits tests pass.

# Verification

`cargo test -p brawler-client --lib shell::credits`, `just lint`.
