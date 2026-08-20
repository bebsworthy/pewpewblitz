# V3 presentation asset inventory

## Status and filename

This file keeps its historical `12-sprite-inventory.md` path so existing links remain valid, but
the sprite inventory itself is retired. V3 ships no gameplay-world sprite, sprite atlas, tileset,
`Mesh2d`, or `ColorMaterial` asset. The current inventory is organized by runtime ownership and
asset type.

## Imported runtime files

| Family | Runtime files | Owner | Required behavior |
|---|---|---|---|
| Fighter | Mini Characters `character-male-a.glb` and pack-local colormap | Imported fighter scene | Rigged model, named clips, bind-pose recovery, sphere fallback |
| Weapon | Blaster Kit `blaster-a.glb` and pack-local colormap | Character attachment | Corrected grip/orientation, cuboid fallback |
| Cover | Mini Arena `block.glb` and pack-local colormap | Map presentation profile | Validated 64×64 footprint, cuboid fallback |
| Audio | `ready`, `fire`, `impact`, `defeat`, and `error` OGG files | Client audio presentation | Optional/degraded readiness and bounded playback |

`assets/manifest.ron` is the canonical per-file inventory and provenance record. This design
document summarizes roles; it must not duplicate paths or licenses as a second source of truth.

## Cached primitive and generated mesh families

| Visual family | Current mesh strategy | Authority relationship |
|---|---|---|
| Floor | Cached cuboid tile | Covers resolved visual placement; no collision ownership |
| Rectangular cover/perimeter | Cached cuboid or imported validated block | Footprint derives from resolved map geometry |
| Circular geometry | Cylinder/generated mesh | Footprint derives from authoritative circle |
| Destructible terrain | Generation/chunk-owned generated mesh | Occupied cells derive from replicated terrain state |
| Fighter fallback | Sphere | Radius matches authoritative fighter body |
| Fighter ground identity | Annulus plus generated ring-integrated arrowhead | Relation/facing presentation only |
| Sentry | Cached cylinders and direction cuboid | Pose/team derive from replicated sentry state |
| Straight projectile | Cached cylinder | Position/rotation/radius derive from authoritative flight |
| Lobbed projectile | Cached sphere | Ground position is authoritative; vertical arc is visual |
| Hot Zone | Procedural fill and annulus | Center and boundary derive from resolved mode anchor |
| Previews/telegraphs | Scaled cuboids and procedural segments | Geometry derives from the same preview rules as authority |
| Status/dash/effects | Shared ring, cuboid, and sphere meshes | Bounded presentation of replicated state/cues |
| Terrain debris | Shared cuboid mesh | Bounded transient feedback; never terrain state |

Meshes and materials are retained in shared client resources. Dynamic map meshes have explicit
generation ownership and removal. Terrain chunks update an owned mesh in place. Per-entity asset
allocation is not an accepted content strategy.

## Bevy UI presentation families

The following are rendered UI rather than world meshes or sprites:

- product title/server-select/game-select/build/queue/match/results/error flow;
- combat, match, roster, readiness, score, timer, result, and ability HUD;
- pause/settings/rebinding/accessibility overlays;
- scoreboard and diagnostic overlays;
- camera-projected fighter name, health amount, rounded health bar, and local-only segmented
  ammunition bar.

World-attached UI is positioned from the propagated `Camera3d` projection but rendered by Bevy UI.
It remains client-only and reads replicated state. `Camera2d` is retained solely as the UI camera;
its presence does not represent a 2D gameplay-world renderer.

## Material/color families

The shared material palette includes calm floor, wall/perimeter, blue/red team, green/blue/red
relation markers, neutral, objective fill/boundary, terrain, allowed/blocked preview, slow,
knockback, muzzle, impact, damage, and dash roles. Transparent/unlit readability geometry and UI do
not cast shadows. Opaque actors, cover, and terrain may use restrained shadows.

Relation colors are invariant presentation semantics:

- green: controlled player;
- blue: ally;
- red: enemy.

Health is green for the controlled player/allies and red for enemies. These signals stay separate
from imported model materials and future skins.

## Source-only material

`external_assets/` may contain complete Kenney distributions: GLB, FBX, OBJ, textures, preview PNGs,
isometric renders, overview files, and licenses. Only a selected, validated subset may be promoted
to `assets/brawler/`. Source-only files are not loaded, packaged, scanned for readiness, or exposed
through authored gameplay definitions.

The old 2D PNG packs, fighter sprites, tileset, atlas manifests, Aseprite masters, paper-doll parts,
pixel-density rules, and y-sort bands described by the pre-V3 proposal are not runtime requirements.
If 2D images return later, they need a concrete UI/catalog use and must not recreate a parallel
gameplay-world renderer.

## Addition checklist

Before adding a new presentation asset family:

1. identify the current gameplay/UI owner and fallback;
2. validate license, provenance, format, dependency paths, orientation, scale, and footprint;
3. decide whether GLB, shared primitive, generated mesh, or Bevy UI is the smallest correct form;
4. keep handles and loading client-owned;
5. add lifecycle/readiness/degradation checks proportional to risk;
6. verify the server feature graph and V3 source audit remain clean.
