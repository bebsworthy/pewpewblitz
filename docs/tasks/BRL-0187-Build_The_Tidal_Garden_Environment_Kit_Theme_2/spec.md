# Outcome

Every built-in theme-2 map (Tidal Garden, Powderline Vault) draws its imported environment pieces
only from one original Tidal Garden kit.

# Context

BRL-0153 delivered the theme-kit mechanism (`theme_kits` in `assets/catalogs/map_asset_visuals.ron`,
schema 6, resolved per map theme), foliage sway, the theme `water_edge` colour, and the theme-1
Crossroads Facility kit built by `asset_src/environment/crossroads-facility/build_kit.py`
(`just facility-kit`). Theme 2 still draws Kenney mini-arena/graveyard and KayKit BlockBits pieces
with per-asset tints.

# Scope

- A `build_kit.py` builder under `asset_src/environment/tidal-garden/` with the same kit contract
  (shared bevel, top-face highlight, height AO in a vertex palette, one material, byte-identical
  rebuilds, self-validation, review sheet) and a `just` recipe.
- Theme-2 kit visuals for every visual a theme-2 built-in map draws: garden wall (32), red brick
  wall (45), metal wall (46), wood wall (47), yellow striped cover (48), cactus (43), seashell (36,
  generated today), concealment bush (33, sway), breakable barrier (34), Heist safe (39), pickup (42).
- Readability: walls vs destructible cover vs bushes vs water stay distinct at gameplay scale.
- Extend the kit coverage test to theme 2; provenance in the kit README and
  `assets/licenses/pewpew-blitz-original-environment.txt`.

# Out of scope

Gameplay geometry, map content, content fingerprint; removing the default third-party profiles
(BRL-0188).

# Acceptance criteria

- Tidal Garden and Powderline Vault render only theme-2 kit imports; footprint/visual-bounds tests
  pass; Map Maker palette previews show the kit for theme-2 documents.

# Verification

Focused tests, `just lint`, `just test`, `just e2e 2`, native captures of both maps (practice,
combat) plus primitive world.
