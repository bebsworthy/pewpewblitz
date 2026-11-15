# Outcome

Mode pennant badges share the BRL-nzre construction: ink outline and offset lip, two tones plus a highlight, BRL-11zt palette.

# Context

BRL-nzre replaced every shell, part-picker, ability and Map Maker icon with the generated set in `asset_src/player-shell/icons/`. The pennant badges (`assets/brawler/ui/shell/{wipeout,hot-zone,heist,zombie}.png` and `game-badge-*.png`, sources `generate-chrome.cjs`, `generate-game-selection.cjs`, `export-badges.cjs`) were kept out of scope as mode emblems; they still use the earlier cream/orange/teal flat style and need Sharp to re-export.

# Scope

- Add the pennants and their emblems to `build_icons.py` (or a sibling using the same renderer), keeping the hanging-pennant silhouette that distinguishes modes.
- Retire the Sharp badge exports they replace.

# Acceptance criteria

- All mode badges come from the generated set with SVG sources and README provenance.
- Owner visual acceptance on Dashboard, game selection and match loading.

# Verification

- `just ui-gallery` capture, native Dashboard capture, focused asset tests.



# Implementation (2026-11-14)

- Generator: `asset_src/player-shell/icons/build_icons.py` gains a `PENNANT` class (100x124 units -> 250x310 px, close to the 0.8 aspect used by all three load sites), a shared `pennant()` construction and four icons: `mode-wipeout` (skull and crossbones), `mode-hot-zone` (flag on a capture-zone disk), `mode-heist` (safe with dial), `mode-zombie` (lime X-eyed head). Construction: orange hanging pennant on a steel rod, inset navy emblem field (the hexagon badges' navy inset), ink outline, offset lip, shade and highlight bands. One silhouette for all modes; only the emblem names the mode. The full rebuild is deterministic (all other icons byte-identical).
- Runtime: `assets/brawler/ui/icons/mode-*.png` + `svg/mode-*.svg`. `ui::icons::MODE_PENNANTS` (names `mode-<mode key>`) feeds `ClientAssetHandles::dashboard_badges` (Dashboard); game selection now always uses those retained handles; match loading loads `icons::path("mode-<key>")`. The Dashboard and game selection previously used two different badge families (orange `shell/<mode>.png` vs teal/ink `shell/game-badge-*.png`); both now show the same pennant.
- Retired: `assets/brawler/ui/shell/{wipeout,hot-zone,heist,zombie}.png`, `shell/game-badge-{hot-zone,heist,zombie}.png`, their SVGs in `asset_src/player-shell/`, `export-badges.cjs`, the Wipeout write in `generate-chrome.cjs` and the badge loop in `generate-game-selection.cjs`.
- Gallery: ICON SET adds the pennants at Dashboard size (64x80 on Panel) and game-selection size (140x174 on Card).
- Provenance/docs: `asset_src/player-shell/icons/README.md` (canvas, pennant style rule, inventory, retirements), `asset_src/player-shell/README.md`, `assets/licenses/pewpew-blitz-original-icons.txt` (now names the mode pennants), Credits original-work label "icons and mode pennants" (same note path), `docs/11` provenance paragraph, `docs/design/player-shell/README.md` iconography line.
- Tests: `icons::tests` covers pennant size/SVG/generator definition and directory contents, asserts the seven retired shell badge PNGs are gone, and a new test checks every registered mode key resolves to `mode-<key>` in Dashboard index order.

# Verification (2026-11-14)

Scratch `CARGO_TARGET_DIR`; user's server on UDP 5000 untouched; `data/` and `config/` unchanged.
- Focused: `cargo test -p brawler-client --lib -- ui::icons shell::credits assets::`: 34 passed (includes the three Credits drift tests).
- `just check`: pass. `just lint` (VS Code ripgrep first on PATH): pass. `just test` after the last edit: pass, 2389 passed, 0 failed.
- Before: `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl1wms/before/contact-sheet` sources (`/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl1wms/before/*.png`, old PNGs), `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl1wms/before/gallery-wide-bottom.png` and `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl1wms/before/gallery-phone-bottom.png` (HEAD gallery, identical to the BRL-51zd after-captures; no pennants shown), `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl1wms/before/dashboard-native-brl-nzre.png` (native Dashboard with the old orange Wipeout pennant).
- After: `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl1wms/contact-sheet.png` (old vs new on Dashboard PANEL 64x80, game-card PANEL_RAISED 140x174, match-loading 130x160; plus Chrome-rendered SVG source for parity and 40x50 on ink), `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl1wms/after/gallery-wide-bottom.png`, `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl1wms/after/gallery-phone-bottom.png` (914x411), `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl1wms/after/dashboard/brawler-000600.png` (native Dashboard against a scratch supervisor on 127.0.0.1:5077 with a seeded scratch profile).
- Not captured natively: game selection and match loading (no OS input automation; BRL-pw9k). Their pennants are the same files shown at those sizes in the contact sheet and the gallery Card strip.

# Playtest / visual sign-off request

Run `just ui-gallery` (scroll to the bottom of ICON SET) and `just run 1`: Dashboard mode button, Play > game selection (all four cards, selected and focused), and match loading for at least two modes. Observe: does the pennant family read as part of the icon set; is each mode recognizable at 64x80 on the Dashboard (Hot Zone flag and Heist safe are the smallest details); does the orange rim hold against the indigo cards and the selected card state. Known limitation: all modes share one orange pennant; mode identity is the emblem only.

# Learn from errors

- Mistake: the first draft gave the emblem field nearly the whole pennant, so at 64x80 on the navy Dashboard panel the badge read as a dark block. Caught by comparing on the real host colors in a contact sheet before wiring code. Prevention: review new UI art on its actual host surfaces and smallest shipped size first.
- Mistake: macOS has no `timeout`; the first gallery capture run silently produced nothing. Prevention: use a backgrounded kill guard (or the gallery's own exit) instead of `timeout`.
- Reusable: a mode-key -> asset-name test through `ModeRegistry` replaces index-based path tables, so a fifth mode fails fast without a pennant.

# Follow-ups

- BRL-4jdx (backlog): retire the unreferenced Sharp-exported shell chrome PNGs (panel-*, game-card*, practice-card*) and move surviving exports off Sharp.


# Owner sign-off (2026-11-14)

Owner reviewed the pennants in `just ui-gallery` and approved them ("nice. approved"). Single orange body with per-mode emblems kept; per-mode body tones not requested.

# Device check 2026-11-15 (Pixel 8a)
Setup: Pixel 8a (45171JEKB03025, Android 17), release APK built from HEAD 6a6d6599. A scratch protocol-48 supervisor ran on 0.0.0.0:5001. Evidence is under /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/dc/.
- Pennant badges render on the Dashboard game tile (Wipeout skull, Zombie) and on all four Choose Game cards (Wipeout, Hot Zone, Heist, Zombie). They show the outline, the two tones and the highlight, with no missing or white textures (`h09-dashboard.jpg`, `s-h10.jpg`, `z01.jpg`). **Pass.**
