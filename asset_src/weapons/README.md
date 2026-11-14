# Fighter weapons

One original held weapon per animation family of the primary weapon's delivery (BRL-q29d). The
weapons are original project work built entirely by the checked-in procedural Blender script; no
third-party, commissioned, or hand-edited model data is involved. They replace the Kenney
`blaster-a` that every family used to share.

## Files

- `build_weapons.py` — palette, weapon geometry, anchors, export, and self-validation.
- `weapons.blend` — the generated source master: the `EXPORT__weapons` collection with one
  `weapon_root` empty per weapon, its `GEO_weapon` mesh, and its anchor empties.
- `previews/weapon-sheet.png` — generated review sheet. It re-imports every exported GLB (a
  round-trip check) and renders it with vertex colours.
- Runtime derivatives (not in this directory):
  `assets/brawler/models/original/weapons/<weapon>.glb`.

## Rebuild

From the repository root, weapons first, then Riff (whose two-handed clips read the anchors):

```sh
just weapon-assets
just riff-asset
# equivalent to
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  -P asset_src/weapons/build_weapons.py
```

Built and verified with Blender 5.2.3 LTS. Pass `-- --no-renders` after the script to skip the
review sheet. The run prints a `WEAPON_BUILD_REPORT` JSON line (bytes, triangles, bounds, anchors)
and fails if an export breaks the contract below. Two consecutive builds produce byte-identical
GLBs. Never hand-edit a GLB: change the builder and rebuild, then update the matching
`preview_center` in `assets/catalogs/fighter_weapons.ron` if a bound changed.

## Weapon contract

Follows `asset_src/03-characters_modeling_spec.md` sections 10 to 12, in Riff's source units.

- `weapon_root` is the only scene root and the identity attachment origin. The client attaches it
  to Riff's `socket_weapon_r` at unit scale. It sits on the primary grip, the palm of the weapon
  hand.
- Blender `-Y` is forward (the business end) and `+Z` up, so the glTF faces `+Z` with `+Y` up.
- Anchor empties under `weapon_root`: `grip_primary` (on the origin), `muzzle`,
  `projectile_spawn`, and on two-handed weapons `grip_secondary`, the support-hand target. Riff's
  builder bakes his left hand onto `grip_secondary` with a per-frame two-bone IK solve and fails if
  a hand misses its anchor by more than 0.02 units. No IK runs at runtime. Gameplay never reads the
  anchors; projectiles still leave from the server-authoritative muzzle offset.
- One mesh, one material, flat per-face palette in `COLOR_0` (Riff's flat look, no baked
  shading), no normals (Bevy rebuilds flat normals), UVs, or textures. The client's toon material
  and outline shade it like the fighter.
- Budget: at most 1,500 triangles and 48 KB per weapon.

## Weapons

| GLB | Family | Weapons | Look | Hands |
|---|---|---|---|---|
| `wpn_blaster.glb` | `1h` | Pulse Sidearm, Scatter Cannon, Sticky Blomb | white toy blaster, cyan emitter | one |
| `wpn_sprayer.glb` | `heavy` | Spray | gunmetal body, yellow tank, flared green nozzle | two (fore grip) |
| `wpn_lobber.glb` | `throwable` | Arc Launcher, Splash | fat pink tube with a loaded lime blob | two (fore grip) |
| `wpn_blade.glb` | `melee` | Impact Blade | wide steel blade, cyan edges, gold guard | two (long hilt) |

The family comes from the primary weapon's delivery: Straight and StickyStraight are `1h`,
ConeSpray is `heavy`, Lobbed and Splash are `throwable`, MeleeArc is `melee`. The client picks
the model per family from `assets/catalogs/fighter_weapons.ron`.

Riff's arms are short (shoulder to wrist is about 0.48 units), so each fore grip sits within
about 0.2 units of the primary grip and two-handed holds keep the weapon centred in front of his
belly with the body turned into the weapon side.

## Provenance

Original work of the PewPew Blitz project, so it is not a row in the third-party
`assets/manifest.ron`. The source of truth is this directory: the builder reproduces the runtime
GLBs with the command above. `assets/licenses/pewpew-blitz-original-weapons.txt` points here.
