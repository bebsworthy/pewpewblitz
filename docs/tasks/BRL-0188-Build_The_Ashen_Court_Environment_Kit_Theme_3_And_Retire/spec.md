# Outcome

Every built-in theme-3 map (Ashen Court, Switchback Basin, Verdant Crossfire) draws its imported
environment pieces only from one original Ashen Court kit, and no shipped map uses a third-party
environment pack or a per-asset tint multiplier.

# Context

BRL-0153 delivered the theme-kit mechanism and the theme-1 Crossroads Facility kit; BRL-0187 does
theme 2. Theme 3 still draws Kenney graveyard pieces (stone wall 16, pine 19, rocks 20, gravestone
21, coffin 22, lantern 23) and KayKit BlockBits (44, 45, 48, 49) with tints.

# Scope

- `asset_src/environment/ashen-court/build_kit.py` with the shared kit contract and a `just` recipe.
- Theme-3 kit visuals for every visual a theme-3 map draws, including destructible cover (24),
  concealment bush (33, sway), and decorations.
- After all three kits exist: make kit coverage mandatory for every theme, retire the default
  third-party profiles in favour of kit visuals, remove unused files under
  `assets/brawler/models/kenney/` and `kaykit/`, their `assets/manifest.ron` rows and
  `assets/licenses/` texts, and the `tint` field from `map_asset_visuals.ron` and code.

# Out of scope

Gameplay geometry, map content, content fingerprint.

# Acceptance criteria

- All built-in maps render only their theme's kit; footprint tests pass; no environment tint field;
  manifest and licenses list only shipped files; Map Maker previews show each theme's kit.

# Verification

Focused tests, `just lint`, `just test`, `just e2e 2`, native captures of every built-in map plus
primitive world.
