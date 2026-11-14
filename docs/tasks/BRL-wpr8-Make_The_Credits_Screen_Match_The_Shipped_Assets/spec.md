# Outcome

The Credits overlay (`CREDITS` in `src/client/shell.rs`) attributes only assets that actually ship.

# Context

Found during BRL-1tdg (retired the Kenney/KayKit environment packs). The Credits text still says
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


# Mismatches found by BRL-da56 (2026-11-14)

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

# Implementation (2026-11-14)

- Credits moved from the `CREDITS` literal in `packages/brawler-client/src/shell.rs` into a new
  `shell/credits.rs` submodule that builds the text from two tables:
  - `THIRD_PARTY`: one entry per row of the `THIRD_PARTY.md` "Shipped packs" table (Mini
    Characters, Sci-Fi Sounds, Impact Sounds, Interface Sounds, Lilita One, Fira Mono (subset)),
    keyed by the exact index pack name, author and SPDX license, with what each is used for.
  - `ORIGINAL`: Riff fighter, fighter weapons, environment kits, floor decals, icons, branding and
    menu art, and all other sound effects and music, each pointing at its provenance (a
    `pewpew-blitz-original-*.txt` note, or the `asset_src/` directory for Riff and shell art,
    which have no note yet: BRL-ztpa).
- Removed: "Fighters and sounds: Kenney" and the never-shipped "Facility tiles: Murphy's Dad /
  HaywardMorihara". Bevy line and "Full license texts ship in assets/licenses/." kept.
- `THIRD_PARTY.md` "Adding a new third-party file" now names the Credits list and its tests.
- No protocol, gameplay, server or manifest change.

# Alignment tests (shell::credits::tests)

- `third_party_credits_match_the_license_index`: parses the "Shipped packs" table and requires
  set equality of (pack, author, license) with `THIRD_PARTY`.
- `every_license_file_is_credited`: every table license file exists; every `ORIGINAL` provenance
  path exists; every file in `assets/licenses/` (except the index) is a shipped-pack license or an
  original-work note named by `ORIGINAL`. A new pack, note, or stale license text fails it.
- `credits_text_names_only_shipped_work`: rendered text names every entry and none of Blaster
  Kit, Murphy, HaywardMorihara, KayKit.
- `shell::tests::credits_back_is_reachable_on_phone_and_desktop`: layout_harness at 868x411 and
  1280x720; Credits text spawns and BACK is reachable (the panel scrolls).

# Verification

Scratch `CARGO_TARGET_DIR`, no native window, no server touched.

- Focused: `cargo test -p brawler-client --lib -- shell::` 25 passed (4 new).
- `just check`: pass. `just lint`: pass (after fixing rustfmt, `doc_markdown`, and
  `format_push_string` findings). `just test`: pass (run before the final lint-only refactor of
  `credits.rs`; focused shell tests and `just lint`/`just check` re-ran afterward).
- The native Credits look was replaced by the layout_harness check per the handoff instruction;
  wording is plain text so no visual risk beyond fit, which the harness covers.

# Playtest

Not required: text-only change whose fit is covered by the layout_harness test. The user may
glance at Dashboard Menu > CREDITS on the next client run.

# Learn from errors

- The first draft kept a display `name` beside the index `pack` key; `pack` was then read only by
  tests and `just test` warned `dead_code`. Prevention: when a field exists only for a test
  cross-check, display it too or derive the check from displayed data.
- The first `just lint` run failed on rustfmt and two Clippy pedantic lints (`doc_markdown` on
  "PewPew", `format_push_string`). Prevention: run `rustfmt --check` and the client Clippy recipe
  on new files before the long `just test`.
- The original ticket listed "the blaster held by Riff (Blaster Kit)"; BRL-q29d retired it before
  this work started. Deriving Credits from the index rather than from the ticket text avoided
  re-crediting it.


# Closeout (2026-11-14)

Closed without a playtest (text-only; layout test covers fit). docs/13-player-ux.md now says Credits mirrors THIRD_PARTY.md. The full `just test` was not re-run after the final Clippy-only edit to credits.rs; focused shell tests, `just check` and `just lint` passed after it.
