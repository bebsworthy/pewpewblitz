# Third-party assets

This is the human-readable index of every third-party asset PewPew Blitz ships. The per-file
authority is `assets/manifest.ron` (checked by the client asset tests); each row there names the
pack, original file, license and source URL. Full license texts live beside this file.

Everything else under `assets/` is original PewPew Blitz work, provenanced by its `asset_src/`
builder and the `pewpew-blitz-original-*.txt` notes in this directory.

## Shipped packs

| Pack | Version | Author | License | Source | License text |
|---|---|---|---|---|---|
| Mini Characters | 1.0 | Kenney | CC0-1.0 | https://kenney.nl/assets/mini-characters | `kenney-mini-characters.txt` |
| Sci-Fi Sounds | 1.0 | Kenney | CC0-1.0 | https://kenney.nl/assets/sci-fi-sounds | `kenney-sci-fi-sounds.txt` |
| Impact Sounds | 1.0 | Kenney | CC0-1.0 | https://kenney.nl/assets/impact-sounds | `kenney-impact-sounds.txt` |
| Interface Sounds | 1.0 | Kenney | CC0-1.0 | https://kenney.nl/assets/interface-sounds | `kenney-interface-sounds.txt` |
| Lilita One | Google Fonts release | Juan Montoreano | OFL-1.1 | https://fonts.google.com/specimen/Lilita+One | `LilitaOne-OFL-1.1.txt` |
| Fira Mono (subset) | as embedded by `bevy_text` 0.19.1 | Mozilla Foundation / Telefonica | OFL-1.1 | Bevy `default_font` feature | `FiraMono-LICENSE` |
| Fira Sans (Bold) | as bundled in Bevy's `bevy_feathers` assets | Mozilla Foundation / Telefonica S.A. | OFL-1.1 | https://github.com/mozilla/Fira | `FiraSans-OFL-1.1.txt` |
| Twemoji Mozilla | 0.7.0 | Twitter, Inc and other contributors / Mozilla | CC-BY-4.0 | https://github.com/mozilla/twemoji-colr/releases/tag/v0.7.0 | `twemoji-mozilla-CC-BY-4.0.txt` |

## Files used

Paths are relative to `assets/`; originals are relative to the unpacked pack.

| Shipped path | Pack | Original | Used for |
|---|---|---|---|
| `brawler/models/kenney/mini-characters/character-male-d.glb` | Mini Characters | `Models/GLB format/character-male-d.glb` | Zombie-mode NPC |
| `brawler/models/kenney/mini-characters/Textures/colormap.png` | Mini Characters | `Models/GLB format/Textures/colormap.png` | Zombie-mode NPC texture |
| `brawler/audio/fire.ogg` | Sci-Fi Sounds | `Audio/laserSmall_000.ogg` | Fire cue |
| `brawler/audio/defeat.ogg` | Sci-Fi Sounds | `Audio/explosionCrunch_000.ogg` | Defeat cue |
| `brawler/audio/impact.ogg` | Impact Sounds | `Audio/impactMetal_light_000.ogg` | Impact cue |
| `brawler/audio/ready.ogg` | Interface Sounds | `Audio/confirmation_001.ogg` | Ready cue |
| `brawler/audio/error.ogg` | Interface Sounds | `Audio/error_001.ogg` | Error cue |
| `brawler/ui/fonts/lilita-one-regular.ttf` | Lilita One | `LilitaOne-Regular.ttf` | UI display font |
| none (compiled into the client binary) | Fira Mono | `bevy_text/src/FiraMono-subset.ttf` | Bevy fallback font |
| `brawler/ui/fonts/fira-sans-bold.ttf` | Fira Sans | `FiraSans-Bold.ttf` (unmodified) | Map Maker body text (BRL-2xpc) |
| `brawler/ui/fonts/twemoji-mozilla.ttf` | Twemoji Mozilla | `Twemoji.Mozilla.ttf` (release asset, unmodified) | Colour emoji fallback, loaded on Android only |

## Retired or never shipped

These packs were used as local source material only. They are not under `assets/` and their
license texts were removed when the last shipped file was retired.

- Kenney Mini Arena, Mini Dungeon, Mini Forest 1.0, Graveyard Kit 5.0 and KayKit Block Bits 1.0:
  retired environment packs, replaced by the original kits in BRL-0153, BRL-0187 and BRL-0188.
- Kenney Game Icons: retired by the original icon set in BRL-0157.
- Kenney Blaster Kit 2.1: the held blaster, retired by the original per-family fighter weapons in
  BRL-q29d.
- Kenney Emotes Pack, Mini Arcade, Pirate Kit and Furniture Kit: evaluated, never shipped.

## Re-acquiring a source pack

Source packs are kept out of git. The local working copy lives in the ignored `external_assets/`
directory, which nothing in the build, tests, scripts or `asset_src/` reads. There is no fetch
script, because the pack pages above are not stable direct-download URLs. To re-acquire one:

1. Open the pack's source URL and download it (Kenney packs are free; donating is optional).
2. Unzip it into `external_assets/` (for example `external_assets/kenney_mini-characters_1.0/`).
3. Check that the pack's `License.txt` version matches the table above. If the version changed,
   compare the original file from the table byte-for-byte with the shipped copy before replacing
   anything, and update `assets/manifest.ron` and this file together.

Twemoji Mozilla is a single release file rather than a pack: download `Twemoji.Mozilla.ttf` from
the v0.7.0 release and check it against the SHA-256 in `twemoji-mozilla-CC-BY-4.0.txt`. Its CC BY
4.0 attribution (title, authors, licence URI, changes) lives in that notice; the licence's full
legal code is linked there, not copied. Every client ships the file under `assets/`, but only the
Android build loads it (`packages/brawler-client/src/ui/fonts.rs`); desktop keeps its system emoji.

Adding a new third-party file requires a `assets/manifest.ron` row, the pack's license text in
this directory, a row in the tables above, and a matching entry in the Credits list
(`packages/brawler-client/src/shell/credits.rs`), whose tests fail until Credits, this table and
the license files in this directory agree. A new `pewpew-blitz-original-*.txt` note likewise needs
an original-work Credits entry.
