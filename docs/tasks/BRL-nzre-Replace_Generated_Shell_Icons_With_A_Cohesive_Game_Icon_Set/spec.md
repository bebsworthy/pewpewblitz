# Outcome

Icons read as polished game UI and share one style.

# Context

`asset_src/player-shell/details/build_art.py` hand-codes polygons (for example, `details/fire.png` is a five-vertex blob in a hexagon). The 96 px map tool icons are single-color silhouettes.

# Scope

- Choose a purchased or licensed set, or redraw in vector with an outline, two tones, and a highlight.
- Cover shell, part-picker, ability, and Map Maker icon needs.
- Record provenance.

# Out of scope

Character portraits.

# Acceptance criteria

- All shell and Map Maker icons come from the new set.
- Licenses are recorded.
- Owner visual acceptance.

# Verification

- `just ui-gallery`.
- Native captures.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.


# Design (2026-11-13)

Research findings:
- Shipped icon needs (by load site): Dashboard (`assets.rs` `dashboard_icons`: lightning/Play, crosshair/Practice, menu, player, arena, skull/View Brawlers, chevron), brawler details/creation/choice cards (`details/{back,chevron,edit,trash,empty}` plus 16 semantic ability/passive badges selected by `details/art.rs`), confirmations (trash), part picker and details slots (`shell/part-{magazine,ammunition,barrel,chamber,element}`), and Map Maker (15 `details/map-*` pictograms). They come from three unrelated styles: Pillow polygons (`asset_src/player-shell/details/build_art.py`), Sharp SVG exports (`generate-chrome.cjs`, `generate-parts.cjs`), and six Kenney Game Icons (`ui/icons/*`, CC0) that are loaded and readiness-reported but never displayed.
- `scan` and `target` are currently the same pictogram; `dash` is a different raster style.
- Every icon load site tints with white (no-op) except the header arena icon (tinted `ACCENT`). Full-color icons can therefore replace them without per-screen color logic; the arena tint becomes white.
- Tooling: no Pillow, NumPy, Sharp, resvg, rsvg-convert or Inkscape; Bevy has no SVG loader and new crates are blocked. Python 3 stdlib is available everywhere the repo builds.
- Sourcing: original procedural artwork only (no paid or third-party icon set), per the owner's direction for this plan.

Decisions:
- One generator, `asset_src/player-shell/icons/build_icons.py` (Python 3 stdlib only), owns the complete set. Each icon is a small list of filled parts built from polygons, circles, rings, round-capped strokes and rounded rects. From that single model it writes an editable SVG per icon (`asset_src/player-shell/icons/svg/<name>.svg`, explicit viewBox, real paths, no filters or external links) and rasterizes the runtime PNG with an exact signed-distance renderer (antialiased, deterministic, zlib PNG writer). The SVG and PNG are the same geometry; the SVG is the editable source, the PNG the runtime export.
- Style (BRL-11zt palette): every part gets the navy `ink` (#0E1638) outline, a 3-unit ink offset lip below the silhouette (the surfaces' pressable lip), a base tone, a darker bottom-lip shade band and a light top highlight band — outline, two tones and a highlight. Tone families: white, orange/yellow (`play`), cyan (`accent`), indigo (`panel-raised`), coral (danger/warning), green (restore), lime (poison), violet (concealment), ice (cold). Ability/passive badges share an indigo hexagon emblem with a raised top band so the 16 mechanics read as one family; navigation and Map Maker pictograms are white with tone accents; part illustrations use the same construction at 6:5.
- Runtime layout: all icons move to `assets/brawler/ui/icons/<name>.png` (square 128 px; badges 240×200; parts 288×240, preserving current aspect ratios). A tiny `src/client/ui/icons.rs` owns the name list and `path()` used by screens, the gallery and tests. Names become semantic (`chevron-left/right/up/down`, `place`, `select`, `close`, `check`, `warning`, ...). `scan` becomes a radar sweep distinct from `target`.
- Retired: `asset_src/player-shell/details/` (Pillow builder and README), the icon and part sections of `generate-chrome.cjs`/`generate-parts.cjs` and their SVGs, `assets/brawler/ui/details/`, `shell/icon-*.png`, `shell/part-*.png`, the six unused Kenney Game Icons with their `ui.icon_*` manifest rows and `assets/licenses/kenney-game-icons.txt` (proven unreferenced first). Credits stop attributing icons to Kenney.
- Provenance: original project artwork. `asset_src/player-shell/icons/README.md` records the command, style contract and inventory; `assets/licenses/pewpew-blitz-original-icons.txt` points at it (the manifest only admits third-party files).
- Evidence: the UI gallery gains an `ICON SET` section showing every icon at real sizes on primary, secondary and dark surfaces, so `BRAWLER_UI_GALLERY_CAPTURE` gives native evidence for screens agents cannot navigate to; plus Dashboard client captures.
- Out of scope: the hanging-pennant mode badges (emblems, not icons; `docs/design/player-shell/README.md`), the logo/branding, panel chrome and underline, and in-match HUD icons (none are image icons today; touch HUD is BRL-3zas).

Tests: the icon module test checks every listed icon exists as an RGBA PNG of its class size with an SVG source, that the directory holds no unlisted file, that every ultimate/passive icon mapping resolves to a listed icon, and that the retired paths are gone.


# Implementation (2026-11-13)

- Generator: `asset_src/player-shell/icons/build_icons.py` (stdlib Python, ~5 s, deterministic: re-running reproduced identical SHA-1s for all PNGs and SVGs), README in the same folder, 47 editable SVGs in `svg/`.
- Runtime: 47 PNGs in `assets/brawler/ui/icons/` (26 pictograms 128 px, 16 badges 240x200, 5 parts 288x240; 293 KB total). `menu.png`/`play.png` reuse the old Kenney file names with new content.
- Code: `src/client/ui/icons.rs` (`PICTOGRAMS`, `BADGES`, `PARTS`, `path()`); load sites in Dashboard (via `ClientAssetHandles`, underline split into its own handle and readiness id), brawler details/choice cards, confirmations, equipment and Map Maker use `icons::path`. Header arena icon tint changed from `ACCENT` to white. Removed six never-displayed Kenney handles and their `ui.icon_*` readiness entries. Credits now list icons as original artwork.
- Gallery: new `ICON SET` section (all pictograms on Primary 36 px, Secondary 28 px, Dark 22 px; badges and parts on a Card).
- Retired (proven unreferenced by `rg` outside generators/history): `assets/brawler/ui/details/` (37 files), `shell/icon-*.png` (7), `shell/part-*.png` (5), Kenney `ui/icons/{settings,build,mode,practice}.png`, `assets/licenses/kenney-game-icons.txt`, the 6 `ui.icon_*` manifest rows, `asset_src/player-shell/details/`, `generate-parts.cjs`, `icon-*.svg`/`part-*.svg`, and the icon writes in `generate-chrome.cjs`.
- Provenance/docs: `assets/licenses/pewpew-blitz-original-icons.txt`, `asset_src/player-shell/README.md`, `docs/11` (index adopted, provenance paragraph), `docs/design/player-shell/ui-toolkit.md`, `src/client/ui/README.md`.

# Verification (2026-11-13)

- Focused: `icons::tests` (every icon is an RGBA PNG of its class size, defined by the generator, with an SVG source; directory equals the set; retired paths gone; license pointer present), `details::art::tests` (all 11 ultimate and 9 passive kinds resolve to shipped badges; Reveal Scan differs from Close Quarters), `assets::tests` updated: pass.
- `just lint` (real ripgrep first on PATH): pass.
- `just test`: pass, 2199 passed, 0 failed.
- `just e2e 2`: pass (exact 1v1 roster reached Active).
- Native (scratch data dirs; `data/` and `config/` unchanged; no client errors):
  - Before: `/private/tmp/claude-501/-Users-boyd--claude-spares-spare-3o59cT/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/evidence-0157/before-sheet.png` (old icons on shell blue), `/private/tmp/claude-501/-Users-boyd--claude-spares-spare-3o59cT/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/evidence/after/gallery-wide.png`, `/private/tmp/claude-501/-Users-boyd--claude-spares-spare-3o59cT/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/evidence/after/dashboard/brawler-000600.png` (BRL-drny state).
  - After: `/private/tmp/claude-501/-Users-boyd--claude-spares-spare-3o59cT/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/evidence-0157/after/gallery-wide-bottom.png` (ICON SET at real sizes), `/private/tmp/claude-501/-Users-boyd--claude-spares-spare-3o59cT/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/evidence-0157/after/gallery-compact-bottom.png`, `/private/tmp/claude-501/-Users-boyd--claude-spares-spare-3o59cT/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/evidence-0157/after/dashboard/brawler-000600.png` (Dashboard icons), `/private/tmp/claude-501/-Users-boyd--claude-spares-spare-3o59cT/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/evidence-0157/after-sheet.png` (PNG beside Chrome-rendered SVG source for parity, plus 18-24 px on orange/ink/indigo).
- Not captured natively (no OS input automation): brawler details, creation, part picker, confirmations and Map Maker screens; all their icons appear at shipped sizes in the gallery capture and their load paths are covered by the directory test.

# Playtest request

Run `just ui-gallery` (scroll to ICON SET) and `just run 1`. Visit Dashboard, View Brawlers > details (ultimate/passive badges, part slots, edit/back/chevron), creation, weapon parts, delete confirmation, and Map Maker (tool rail, grid/symmetry/fit, selection toolbar, search, validation). Requested observations: owner visual acceptance of the style; any icon that reads poorly at its size or on its surface (notably badges on indigo cards, yellow lightning on orange Play); any meaning that is unclear (scan radar, field dome, cycle, symmetry).
Known limitations: mode pennant badges keep the earlier style (BRL-1wms).

# Learn from errors

- Mistake: the generator's stale-file cleanup in `assets/brawler/ui/icons/` silently deleted and overwrote the Kenney icons that shared the directory and names (`menu`, `play`), before their retirement had been proven. Harmless here because retirement was planned, but a generator that owns a directory must only be pointed at one it exclusively owns. Prevention: give a generator a fresh output directory or check for foreign files before cleaning.
- Mistake: `xargs`-less `$files` expansion failed under zsh (no word splitting). Prevention: pipe file lists through `xargs`.
- Reusable: Chrome headless (`--screenshot`) renders SVG/PNG contact sheets for quick parity and small-size checks without image libraries; a signed-distance rasterizer in stdlib Python is fast enough (multiprocessing) for a full UI icon set.

- Deferred: BRL-1wms (mode pennant badges in the icon style).
