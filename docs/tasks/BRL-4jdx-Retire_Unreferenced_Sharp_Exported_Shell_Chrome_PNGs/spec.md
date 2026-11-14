# Outcome

`assets/brawler/ui/shell/` holds only artwork a screen loads; the remaining shell exporters no longer need Sharp.

# Context

Found during BRL-1wms. After the toolkit migration (BRL-gaec/BRL-drny) no Rust code loads `shell/panel-{mode,play,practice,build,menu}{,-focus}.png`, `shell/game-card*.png` or `shell/practice-card*.png` (checked with `rg` over `packages/` and `src/` on 2026-11-14; only `panel-identity.png`, `underline.png`, `logo.png`, `game-selection-background.png` and the map overviews are loaded). Their exporters `generate-chrome.cjs`, `generate-game-selection.cjs` and `generate-practice-cards.cjs` still require Sharp, which is not installed here.

# Scope

- Prove each file unreferenced (code, manifests, docs that claim runtime use), then delete the PNGs, their SVGs and the exporter sections.
- Move the surviving exports (`panel-identity`, `underline`, `game-selection-background`) to a stdlib-Python builder, or document why Sharp stays.
- Update `asset_src/player-shell/README.md`.

# Acceptance criteria

- No unreferenced PNG remains in `assets/brawler/ui/shell/`; a test guards it.
- Credits drift tests still pass.

# Verification

- Focused asset test, `just check`, `just test`, `just ui-gallery` and a native Dashboard capture showing no visual change.
