# Outcome

The Credits overlay (`CREDITS` in `src/client/shell.rs`) attributes only assets that actually ship.

# Context

Found during BRL-0188 (retired the Kenney/KayKit environment packs). The Credits text still says
"Fighters, icons, and sounds: Kenney" (fighters are now the original Riff model; Kenney ships only
the zombie NPC, the blaster, icons, and sounds) and "Facility tiles: Murphy's Dad /
HaywardMorihara, CC0 1.0", for which no asset or license file exists in the repository. Original
environment kits are not mentioned.

# Scope

- Update the Credits string to match `assets/manifest.ron` and `assets/licenses/`.
- Mention the original environment kits and fighter as PewPew Blitz original work.

# Acceptance criteria

- Every credited third-party pack has a shipped file and license text; every shipped third-party
  pack is credited. A focused test or check keeps them aligned if cheap.

# Verification

Focused test, `just lint`, a native look at the Credits overlay.


# Mismatches found by BRL-0167 (2026-11-14)

Cross-checked against the new pack index `assets/licenses/THIRD_PARTY.md`:

- "Fighters and sounds: Kenney, CC0 1.0." Fighters are the original Riff model. Kenney now ships
  only the Zombie NPC (Mini Characters), the blaster held by Riff (Blaster Kit), and five clips
  (fire/defeat from Sci-Fi Sounds, impact from Impact Sounds, ready/error from Interface Sounds).
  Most cues are original audio (`pewpew-blitz-original-audio.txt`), which Credits does not mention.
- "Facility tiles: Murphy's Dad / HaywardMorihara, CC0 1.0." No such asset, manifest row, or
  license text exists; it must go.
- Original environment kits, the Riff fighter, and original audio are not credited as
  PewPew Blitz original work.
- Lilita One, Fira Mono, Bevy, and "Icons: original" are correct.
- Consider pointing the focused alignment check at `THIRD_PARTY.md` / `assets/manifest.ron` pack
  names.
