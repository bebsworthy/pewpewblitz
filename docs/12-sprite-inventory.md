# Sprite inventory

## Status

This is a historical inventory for the superseded 2D production-art proposal. V3 is the active
version and replaces gameplay-world sprites and `Mesh2d` presentation with a fixed-camera 3D scene.
The active scope is [V3 M04](./implementation/v3/milestone-04.md) within the
[V3 roadmap](./implementation/v3/roadmap.md). Screen-space Bevy UI remains outside that world-renderer
replacement.

## Purpose and scope

This document is a complete list of objects that require sprite images, synthesized from the
visual direction ([Art and presentation direction](./11-art-and-presentation-direction.md)), the
map/mode model ([Maps and game modes](./04-maps-and-game-modes.md)), the combat model
([Weapons and abilities](./03-weapons-and-abilities.md)), the environment catalog
([Environment, surface, and tile ideas](./09-environment-and-tile-ideas.md)), and the prototype
shortlist ([MVP asset shortlist](./07-mvp-asset-shortlist.md)).

It is an inventory, not a scheduling document. It described the sprite requirements considered
before V3 superseded that direction; the active roadmap and milestone remain authoritative.

Status triage:

- **v1 stand-in** — required now; met by CC0/greybox placeholders, never shipping art.
- **Production** — required for the doc-11 production look; owned template/rig art, scheduled by a
  future version roadmap.
- **Future content** — named in a research catalog; no committed scope.
- **Procedural, no sprite** — shader or render-time art; listed for completeness so it is not
  accidentally authored as a sprite.

Sprites are always replaceable client presentation. No sprite participates in authority,
collision, or gameplay rules. Team color, health bars, selection ring, and hit flash are rendered
signals layered on top of sprites, never painted into them.

## Characters (skeleton paper-doll rig)

Source: [Art and presentation direction](./11-art-and-presentation-direction.md#characters-skeleton-paper-doll-rig-and-skins).

| Object | Status | Notes |
|---|---|---|
| Shadow sprite | Production | Blob shadow at feet. |
| Body sprite | Production | Torso+legs per facing, 8 directions, static or 2-frame; procedural bob/tilt/squash. |
| Head sprite | Production | Slight aim-lag offset. |
| Team-trim sprite | Production | Plume/trim/outline accent in team palette; skins cannot override. |
| Accessory sprites | Production | Skin-defined: hat, backpack, antenna, and similar. |
| Weapon sprite | Production | Per-weapon art; anchors at the shoulder pivot and rotates freely. |
| Hand sprite | Production | Recoil translation on fire. |
| Flipbook flourishes | Production | Defeat / respawn / ultimate, 2–3 frames each. |

## Terrain blob-template slots

Source: [Art and presentation direction](./11-art-and-presentation-direction.md#terrain-owned-blob-template).

One atlas per theme fills every slot; the renderer is written once against slot names.

| Object | Status | Notes |
|---|---|---|
| Floor base variants | Production | 3–4 variants + breakup; walkable ground. |
| Elevated-top variants | Production | 3–4 variants; blocking-terrain cap surface. |
| Edge blob (16-blob) | Production | 4 edges, 4 outer corners, 4 inner corners, lip trim. |
| Cliff-face variants | Production | N/E/S/W + 4 corner faces, ×2–3 erosion stages. |
| Face-base rubble | Production | 2–3 transition strips at the floor seam. |
| Biome transition strip | Production | 16-blob A↔B strip; shipped even if v1 maps stay single-theme. |
| Liquid surface frames | Production | 4 frames; water/tar/lava chosen per theme. |
| Shoreline blob | Production | 16-blob shore. |
| Foam frames | Production | 4 frames. |
| Deco props | Production | 8–12 multi-tile props + 4–6 ground scatter; no collision. |
| Cliff-top AO strip | Production | Shadow/AO pass. |
| Prop blob shadows | Production | Separate shadow-pass role. |
| Grayscale skeleton fill | Production | Doubles as the greybox fallback for unfinished themes. |

## Weapons, projectiles, and combat VFX

Source: [Weapons and abilities](./03-weapons-and-abilities.md#presentation-effects) and
[Art and presentation direction](./11-art-and-presentation-direction.md#weapons-projectiles-and-explosions).

| Object | Status | Notes |
|---|---|---|
| Muzzle-flash frames | v1 stand-in → Production | 2–3 frames per weapon. |
| Projectile core sprites | v1 stand-in → Production | Pulse, pellet, lobbed shell, blade, charge rifle. |
| Projectile trail | Production | Ghost sprites or sprite particles, per profile. |
| Explosion/impact flipbooks | v1 stand-in → Production | Flipbook sheet + expanding ring + shockwave params. |
| Debris palette | Production | Bounded sprite particles; generalizes the M10 cap of 64 toward 512 live. |
| Shell casings | Production | Bounded sprite particles. |
| Sparks | Production | Bounded sprite particles. |
| Glow textures | Production | Alpha-falloff over dark floors first; additive quad layer later. |
| Hit marker | v1 stand-in | Rendered HUD signal; independent of sprites. |
| Terrain crater edge | v1 stand-in → Production | Client presents/softens the quantized occupancy edge. |
| Screen shake | Procedural, no sprite | Camera transform, not an image. |

## Map and environment objects

Source: [Maps and game modes](./04-maps-and-game-modes.md), [MVP asset shortlist](./07-mvp-asset-shortlist.md), and
[Environment, surface, and tile ideas](./09-environment-and-tile-ideas.md).

| Object | Status | Notes |
|---|---|---|
| Permanent walls and cover | v1 stand-in → Production | Visual only; collision is separate geometry. |
| Destructible-terrain visuals | v1 stand-in | M10 per-chunk placeholder images → tile meshes over occupancy bits. |
| Spawn-point markers | v1 stand-in | Rendered markers; separate from collision. |
| Generic pickups | v1 stand-in → Production | Stand-ins: Versatile 255-Tile Pack markers. |
| Gem Grab collectibles | Future content | Periodic spawn; carrier indicator. |
| Gem Grab loot drops | Future content | Dropped on defeat. |
| Showdown loot pickups | Future content | Loot progression. |
| Hot Zone capture volume | Procedural, no sprite | SDF ring pulse + animated fill shader. |
| Heist durable objective | Future content | Two objective locations; objective health separate from fighter damage. |
| Showdown boundary | Future content | Late-game shrinking playable area. |
| Hazard sprites | Future content | Fire, acid, lava, electricity, danger boundary. |
| Water compositions | Future content | Deep blocking, shallow slowing, damaging, visual-only puddle. |
| Mobility surfaces | Future content | Speedway, conveyor, wind current. |
| Hindering surfaces | Future content | Mud, snow, webs, shallow water. |
| Slippery surfaces | Future content | Ice, oil. |
| Concealment visuals | Future content | Tall grass, bushes, smoke, darkness, invisibility field. |
| Traversal devices | Future content | Jump pads, teleporters, one-way gates. |
| Interactive geometry | Future content | Doors, switches, moving cover, retractable walls. |
| Ability-created areas | Future content | Smoke, temporary walls, speed fields; server-owned runtime entities. |
| Cosmetic decals | Production | Grass, puddles, trim, scatter; presentation only. |

## Area effects

Source: [Art and presentation direction](./11-art-and-presentation-direction.md#area-effects).

| Object | Status | Notes |
|---|---|---|
| Objective-zone ring + fill | Procedural, no sprite | SDF ring pulse, animated fill, time uniform. |
| Telegraph decals | Procedural, no sprite | Lob-landing + area-telegraph expanding rings. |
| Status-area fields | Procedural, no sprite | Tinted animated quads keyed by presentation ID. |
| Floor tint / glow base textures | Production | Base texture for shader quads. |

## HUD and UI

Source: [MVP asset shortlist](./07-mvp-asset-shortlist.md) and
[Gameplay MVP](./05-gameplay-mvp.md#presentation-acceptance).

| Object | Status | Notes |
|---|---|---|
| Ability/weapon/status/objective icons | v1 stand-in | Game-icons.net CC BY 3.0 with attribution. |
| Health bars | v1 stand-in | Rendered; independent of the sprite. |
| Ammo / cooldown indicators | v1 stand-in | Rendered; controller-friendly. |
| Selection ring | v1 stand-in | Rendered; separate from sprite. |
| Hit flash | v1 stand-in | Rendered; separate from sprite. |
| App icon | Backlog | GAP-BUILD-NOTARIZE packaging. |

## Historical proposed file structure

Source: the atlas/batching, theme, and skin models in
[Art and presentation direction](./11-art-and-presentation-direction.md). The renderer is
theme-agnostic; a directory name is the stable theme or definition ID that code and the map recipe
resolve to paths.

```text
assets/
  manifest.ron                  # one provenance manifest covering every shipped asset
  licenses/                     # CC0/source license texts
  brawler/
    themes/
      forest/                   # one dir per theme; identical slot layout across themes
        forest.atlas.ron        #   slot manifest: slot_name, frame_rect, pivot, palette
        forest.png              #   single atlas filling every blob-template slot
      desert/
      snow/
    skins/
      default/                  # skeleton paper-doll parts for one skin
        skin.ron                #   SkinDefinition: pivots, palette, accessories, flourish overrides
        body/                   #   body_<facing>.png (8 facings x 1-2 frames)
        head.png
        accessories/            #   hat.png, backpack.png, antenna.png ...
        flourish_*.png          #   defeat/respawn/ultimate flipbooks
    fighters/
      team_trim.png             # shared team palette layer (skins cannot override)
      shadow.png                # shared blob shadow
    weapons/
      pulse_sidearm/            # keyed by stable weapon definition ID
        weapon.png
        muzzle_flash.png        # 2-3 frames
        projectile.png
        trail.png
      scatter_cannon/
      arc_launcher/
      impact_blade/
    vfx/                        # shared, weapon-agnostic
      explosion.png             # flipbook sheet
      impact.png
      shockwave.png
      debris.png
      particles.png             # sparks, casings, dust (pooled sheet)
      glow.png
    hud/
      icons.png                 # ability/weapon/status/objective icon atlas
```

Authoring sources stay outside the runtime asset tree so masters never ship or load; one Aseprite
master per theme with identical canvas layout and slice names feeds the export script:

```text
art-sources/
  themes/forest/forest.aseprite     # one master per theme, identical canvas/slices
  themes/forest/export.sh           # produces assets/brawler/themes/forest/*
  skins/...
  weapons/...
```

Structure rules:

- **One atlas per theme, never per-sprite.** Doc 11 requires batching without texture
  interleaving within a z band. The `.atlas.ron` slot manifest is what the renderer reads; a
  pixel-density change (P=1 to P=2) re-authors the PNG and manifest, not the renderer.
- **Theme ID is the directory name**, mirroring the validated, fingerprinted
  `environment_theme_id` in the map recipe.
- **Skins live under `skins/<id>/`** and match `SkinDefinition`; team trim and shadow live outside
  skins so skins cannot override them.
- **Weapons live under `weapons/<id>/`**, keyed directly into the weapon presentation profile by
  stable definition ID.
- **VFX and HUD are shared atlases**, one texture per z band.
- **`art-sources/` sits outside `assets/`** so Aseprite masters and export scripts never enter the
  shipped/loaded tree; every shipped PNG stays CC0-tracked in the manifest.

## Notes and boundaries

- **Authored, not sprites:** slot layout (tile units), palette manifests (16–24 colors/theme),
  presentation profiles (muzzle-flash frames, recoil curve, trail kind, debris palette), and skin
  definitions (part sprites by slot, pivots, palette, accessories, flourish overrides) are data
  that reference sprites; the sprites above are the images those data point to.
- **Licensing:** CC0 packs are stand-ins and style references, not shipping content. Production art
  comes from the owned blob template and rig; commercial games are referenced for motion style only.
  The CC0 provenance manifest extends to tilesets, character parts, and effect sheets.
- **Server boundary:** no sprite is needed by the dedicated server; everything above stays in
  client-gated modules.
