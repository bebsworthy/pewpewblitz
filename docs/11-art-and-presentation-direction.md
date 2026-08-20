# Art and presentation direction — superseded 2D proposal

## Status

This document preserves the 2D pixel-art research and the decisions validated on 2026-08-16, but
it is no longer Brawler's active renderer or production-art direction. On 2026-08-20 the user
reopened the decision after comparing the intended dimensional arena result with the complexity of
simulating depth through sprites, cliff tiles, Y sorting, directional body art, and authored
shadows. V3 now plans a complete 3D gameplay-world presentation over the unchanged 2D
server-authoritative simulation.

The active migration scope is [Version 3 implementation roadmap](./implementation/v3/roadmap.md).
The [3D presentation feasibility foundation](./implementation/v3/milestone-01.md) is complete, and
[the default 3D arena/map/terrain cutover](./implementation/v3/milestone-02.md) is now active.
The enduring boundaries in this document—combat readability, client-only presentation, stable
presentation IDs, bounded effects, licensing/originality, and no render dependency on the
dedicated server—still apply. Its pixel density, blob tileset, paper-doll rig, Y-sort, sprite
particle, and 2D material decisions are historical inputs, not implementation requirements.

## Purpose and scope

This document records the formerly validated 2D visual direction and presentation architecture
for moving Brawler from greybox placeholder visuals to production-quality presentation:
tileset-based terrain, skinned animated characters, weapon/effect visuals, and readable area
effects. It is a direction and research catalog in the spirit of
[environment and tile ideas](./09-environment-and-tile-ideas.md); the V3 roadmap and its milestone
specifications now supersede it for scheduled scope.

The direction was validated with the user on 2026-08-16 across style references, terrain
authoring strategy, character animation architecture, and art pixel density. It governed the V1
greybox-to-art proposal until the user reopened the renderer decision for V3 on 2026-08-20.

In scope for this document:

- the style specification and its reference materials;
- the owned blob-template terrain contract and per-environment themes;
- the character skeleton rig and skin model;
- weapon, projectile, explosion, and area-effect presentation;
- rendering architecture decisions these imply (tile meshes, y-sorting, sampling, batching);
- server, protocol, and asset-boundary consequences;
- a sequencing proposal and the decisions still open.

Out of scope:

- scheduling, milestone content, or acceptance criteria (owned by a future version roadmap);
- production art assets, commissioned art contracts, or final palettes;
- changes to authority, replication, or gameplay rules. Presentation derives from replicated
  state and cues; it never becomes an input to gameplay.

## Style direction

Brawler's visual target is **pixel-art nature environments with chunky readable silhouettes**:
organic terrain built from blob tilesets, elevated blocking terrain rendered as cliff faces,
cartoon-proportioned fighters with outlines and accent team colors, and flipbook effects. Each
reference owns one aspect of the target rather than the whole look:

| Reference | Contributes | Does not contribute |
|---|---|---|
| [zedpxl pixel-art forest pack](https://zedpxl.itch.io/pixelart-forest-asset-pack) | terrain pixel density, palette discipline, forest-theme reference | layout (single 16px sample sheet, no elevation or units) |
| [Pixelfrog Tiny Swords](https://pixelfrog-assets.itch.io/tiny-swords) | composition: flat/elevation/shadow sheet separation, cliff-face elevation illusion, unit proportions, accent-based team color, deco props | its 64px painted scale (does not fit the camera; see density below); frame-animated units (superseded by the skeleton rig) |
| Nuclear Throne / Enter the Gungeon (commercial games; style reference only) | character motion model: quantized body facing with a freely rotating weapon layer | any assets |

Free samples of both packs were reviewed on 2026-08-16 (developer-local, outside the repository:
`/Users/boyd/wip/Tiny_sword_010`, `/Users/boyd/wip/pixel_16_wood`). Both are CC0; they serve as
style references and temporary greybox stand-ins, not shipping content — see
[licensing and originality](#server-protocol-and-asset-boundaries).

### Readability guardrails

These rules bind every future theme and skin; they exist because combat readability is a core
product constraint:

- **Team color is a dedicated accent layer, never the body.** Skins change body shape, palette,
  and accessories; team identity lives in a fixed trim/plume/outline layer skins cannot override.
- **An effect color language.** Damage reads warm, control/slow reads cold, beneficial reads
  green, objectives read faction-neutral gold, destruction reads distinct from all of these.
- **Calm floors, loud edges.** Floor art stays low-contrast so fighters, projectiles, and effects
  pop; walls and terrain edges get strong contrast and edge lighting.
- **Blocking terrain has one universal metaphor** — a raised plateau with a cliff face — because
  "you cannot walk up a cliff" is a universally readable affordance. Themes vary the material
  (grass over dirt, sand over sandstone, snow over ice), never the metaphor.

### Art pixel density

The camera presents a fixed 720 world units vertically (`CAMERA_VERTICAL_SPAN`,
`src/movement/arena.rs`). Screen pixels per art pixel equal
`(window physical height / 720) / pixels-per-world-unit`, and pixel art requires at least one
screen pixel per art pixel:

| Art density (px per 16-unit visual tile) | 1080p | 1440p | Retina laptop (2880 physical) | 4K |
|---|---|---|---|---|
| **16px, P=1 (selected)** | 1.5× | 2× | 4× | 3× |
| 32px, P=2 ("hi-bit" upgrade path) | 0.75× minified | 1× | 2× | 1.5× |
| 64px, P=4 (Tiny Swords' native scale) | 0.375× | 0.5× | 1× | 0.75× |

Decision: **author at 16px per 16-unit visual tile (P=1)** with the camera span unchanged. This
renders natively at or above one screen pixel on every display 1080p and larger, matches the
forest-pack density, and leaves gameplay framing untouched. Tiny Swords' 64px scale only works
under linear-filtered minification and is therefore rejected for pixel art at this camera span.

The template contract specifies slot layout in tile units, not pixels, so a future "hi-bit"
32px re-authoring is an asset change plus (on sub-1440p displays) snapping the camera span to
about 540 units — presentation-only, no renderer rework.

## Terrain: owned blob template

Brawler authors its own terrain template rather than adopting a third-party tileset, so that
environments (forest, desert, snow, …) are parallel themes of one contract and the future player
map builder selects a theme ID instead of code selecting art.

### Slot contract

The renderer is written once against slot names; a theme is an atlas that fills every slot. Slot
layout is defined in tile units so pixel density can change without touching the contract.

| Family | Slots | Purpose |
|---|---|---|
| Floor base | 3–4 variants + breakup | walkable ground |
| Elevated top | 3–4 variants | blocking-terrain cap surface |
| Edge blob (16-blob) | 4 edges, 4 outer corners, 4 inner corners, lip trim | top-surface edges against air |
| Cliff faces | N/E/S/W + 4 corner faces, ×2–3 erosion stages | the 2.5D elevation illusion; erosion keyed to terrain dirty history |
| Face-base rubble | 2–3 transition strips | seam where faces meet floor |
| Biome transition | one 16-blob strip A↔B | shipped in the contract even if v1-style maps stay single-theme |
| Liquid + shoreline | surface ×4 frames, shore 16-blob, foam ×4 | water/tar/lava chosen per theme |
| Deco props | 8–12 multi-tile props + 4–6 ground scatter | theme flavor; no collision |
| Shadows/AO | cliff-top AO strip, prop blob shadows | the separate shadow-pass role Tiny Swords demonstrates |

### Theme model

A theme fills the slot contract plus a **palette manifest** (roughly 16–24 declared colors per
theme; 16px pixel art lives or dies on palette discipline). Themes are validated like all
authored content: every theme atlas fills every slot with identical layout, and each theme gets
a fingerprint. A grayscale skeleton fill of the contract doubles as the greybox fallback for
unfinished themes.

Maps bind themes through a validated, fingerprinted `environment_theme_id` on the map recipe,
replicated with the map snapshot. The renderer is theme-agnostic: same mesh builder, atlas and
palette resolved from the theme ID. Multiple biomes per map reuse the transition family later.

### Authoring pipeline

One Aseprite master per theme with identical canvas layout and slice names; an export script
produces the atlas PNG plus a RON slot manifest; the provenance manifest and validation tests
(see below) accept the result. This matches the Tiny Swords sample, which ships `.aseprite`
sources for its effects.

### Integration with the terrain subsystem

Visual tiles derive from authoritative data and never live alongside it, per
[the environment/tile vocabulary](./09-environment-and-tile-ideas.md):

- **destructible terrain**: the M10 occupancy grid is the tile data source. A client tile
  renderer replaces the M10 per-chunk CPU-image placeholder with chunked tile meshes selected by
  autotiling over the same replicated occupancy bitsets, rebuilt on the same dirty-chunk and
  neighbor-seam tracking;
- **permanent walls**: static autotiled mesh from permanent geometry;
- **floors, liquid, deco**: presentation layers resolved from map recipe regions and
  deterministic placement seeded by the map fingerprint (props never collide and never
  replicate);
- an occupied cell with an empty southern neighbor draws a cliff-face quad; edge/erosion variant
  selection is the same 4-neighbor logic M10 already computes for crater-edge coloring.

## Characters: skeleton paper-doll rig and skins

Characters use a **skeleton of part sprites** (child entities with transforms), not frame
flipbooks. Bones are plain ECS children; a small animator (~150 lines) drives transforms from
procedural state and keyed flourishes. No Spine dependency, no mesh skinning.

```text
FighterRoot            replicated position; body facing quantized to 8 directions
├─ ShadowSlot          blob shadow at feet
├─ BodySlot            torso+legs sprite per facing (static or 2-frame); procedural
│                      bob/tilt/squash; never rotated to arbitrary angles
├─ HeadSlot            head sprite, slight aim-lag offset
├─ TeamTrimSlot        plume/trim/outline accents, team palette; skins cannot override
├─ AccessorySlot(s)    skin-defined (hat, backpack, antenna, …)
└─ WeaponSlot          anchored at a shoulder pivot; rotates freely to the replicated aim
   ├─ HandSprite       recoil translation on fire
   └─ WeaponSprite     per-weapon art + muzzle-flash anchor
```

The motion model resolves the pixel-art-versus-rotation tension: **the body never rotates
freely** (quantized facings keep body art on the pixel grid; life comes from procedural bob,
lean, squash on dash, recoil kick), while only the weapon-and-hands layer rotates continuously
to the true aim angle. Rotated-pixel aliasing on the small weapon layer is invisible in motion
under a 2–3 frame muzzle flash; if playtests disagree, weapon sprites move to a 2×-finer grid —
an asset-only change the layered rig absorbs.

Why this model won over directional flipbooks:

- a skin is a handful of part sprites (torso ×8 facings, head, accessories) plus a palette, not
  directions × animations × frames — cheap skins mean many skins;
- free twin-stick aim with no animation blending problem; the weapon layer tracks the
  already-replicated aim angle;
- one rig renders future deployables (the sentry is a natural parts rig);
- network and authority are untouched: facing and aim replicate today; the rig is pure client
  presentation.

Skins are definitions like weapons: `SkinDefinition { part sprites by slot, pivots, palette,
accessories, flourish overrides }`, validated, fingerprinted, selected through the build-selection
flow (client requests, server validates the stable skin ID, the ID replicates so all clients
render the same skin). Defeat/respawn/ultimate flourishes may use 2–3 frame flipbooks where the
rig cannot sell the moment.

## Weapons, projectiles, and explosions

Weapon definitions gain a client-resolvable **presentation profile** keyed by the weapon's stable
definition ID (implicit per-weapon defaults first, authorable later):

```text
WeaponPresentationProfile
  muzzle_flash frames, recoil curve
  projectile: core sprite, trail kind (ghost sprites | particles), glow
  impact/explosion: flipbook sheet, shockwave params, debris palette, shake amplitude
  layer/blend hints
```

The VFX runtime has three tiers, cheapest first:

1. **Flipbook spritesheets** for explosions and impacts — the largest visual win per effort; a
   landing is flipbook + expanding ring + bounded debris + bounded camera shake, all spawned
   from the existing cue path (one landed delivery still produces one impact one-shot, per the
   M10 audio contract).
2. **Bounded sprite particles** for sparks, trails, and casings: pooled entities with lifetime,
   velocity, and fade under a fixed live cap (generalizing the M10 debris cap of 64; e.g. 512
   live particles). No GPU particle dependency at first; one is the evaluated upgrade if profiles
   outgrow sprites.
3. **Glow**: alpha-falloff textures over dark floors first; one additive-blended custom material
   quad layer for premium glow later.

Presentation animation runs on render time in `Update`, never fixed-tick; simulation stays 60 Hz
authoritative and visuals stay smooth under network jitter.

## Area effects

Flat translucent rectangles are replaced by per-definition procedural shader visuals on quads
with custom 2D materials (stock sprites expose no blend modes):

- objective zones: an SDF ring pulse plus animated fill driven by a time uniform — the shader
  animates itself at no per-frame CPU cost;
- telegraphs: lob-landing and area-telegraph decals as expanding-ring shaders synced to flight
  time;
- status areas: tinted animated fields keyed by the effect definition's presentation ID.

Readability rule: every area effect presents a boundary that reads **before** entry — a ring and
a floor tint, never fill-only. Presentation profiles live in a client presentation catalog with
the same fingerprint discipline as gameplay content; they carry no gameplay meaning.

## Rendering architecture

- **Chunked tile meshes** over occupancy and geometry, per visual chunk, rebuilt only when dirty
  — reusing M10's chunk/dirty machinery. `bevy_ecs_tilemap` was considered and rejected: it wants
  to own tile storage, and Brawler's data flow (replicated occupancy → derived visuals) would
  fight it. The custom builder is small because the chunking already exists.
- **Y-sorting for the 3/4 illusion.** A fighter north of a plateau is occluded by it; a fighter
  south of the cliff face draws in front. Because blocking terrain is never walkable, fighters
  are always on the floor layer, and classic painter's order suffices: terrain-mesh quads and
  entity z both derive from world y (a small epsilon per unit of y, sized so the full coordinate
  span fits the existing camera-clip z budget). Floors, liquid, shadows, and effects keep fixed
  layers; the existing camera clip-range test is updated with the final layer plan.
- **Pixel-crisp sampling.** Nearest sampling on every atlas; a small pixel-perfect system snaps
  the projection scale to integer screen-pixel multiples where close (1080p's 1.5× either
  tolerates slightly uneven pixel widths, as shipped retro games did, or snaps to 1×/2×). This is
  a foundation task, not polish.
- **Batching.** One atlas per theme/pack; no texture interleaving within a z band.
- **Bounds and determinism.** VFX entity counts stay bounded (the established pattern); VFX
  randomness seeds from the cue's tick/attack ID now so future replays or spectating do not
  retrofit determinism.

## Server, protocol, and asset boundaries

- Everything visual stays in client-gated modules; the server feature graph must not gain
  render, asset, or image dependencies (existing rule, existing checks). The dedicated server
  never creates images, atlases, or VFX entities.
- **Protocol changes are additive only:** a stable skin ID in the build selection, and content
  fingerprint changes for `environment_theme_id` / presentation IDs. No geometry, collision, or
  authority semantics cross the wire.
- The CC0 provenance manifest (`src/client/assets.rs` validation) extends to tilesets, character
  parts, and effect sheets, tracking origin, author, license, and fallback for every production
  asset.
- **Licensing and originality:** both reference packs are CC0, so prototyping with them is legal,
  but shipping a look indistinguishable from Tiny Swords reads as an asset flip and the product
  demands original art. The samples serve as style references and temporary greybox stand-ins;
  production terrain, characters, and effects come from the owned template and rig. Characters'
  motion references (Nuclear Throne, Enter the Gungeon) are commercial games and are referenced
  for style only — no assets from them ever enter the repository.

## Sequencing proposal

Ordered by visual payoff per risk; a future version roadmap owns actual scheduling:

1. **P1 — Tile and floor foundation.** Atlas pipeline, chunked tile renderer with autotiling and
   cliff faces over M10 occupancy, pixel-crisp sampling. Largest single jump from greybox; also
   de-risks the player map builder.
2. **P2 — Characters and skins.** Skeleton rig, procedural locomotion, animator, skin definitions
   and selection replication.
3. **P3 — Weapons and combat VFX.** Presentation profiles, flipbooks, particles, trails, shake.
4. **P4 — Area effects and polish.** Zone/telegraph/status shaders, glow pass, HUD art.

Each phase is client-only and independently shippable; none blocks the v1 closeout milestone.

## Open decisions

- First theme list and order (forest has the strongest reference material; desert/snow follow).
- Body facings: 8 selected provisionally; 4 is cheaper but reads coarse against free aim.
- 1080p fractional scale: tolerate 1.5× pixel unevenness or snap the span (540/1080 units).
- Character art sourcing: CC0/palette-swap skins prove the pipeline; commissioned parts slot into
  the same definitions later.
- Weapon-sprite pixel density: same grid as characters first, 2×-finer grid only if rotated-pixel
  aliasing bothers in playtest.
- Particle cap value (512 provisional) and whether effects sheets gain theme accents (leaf bursts,
  dust, snow puffs) in P3 or P4.

## Relationship to version scope

V1 kept its greybox presentation through Milestone 11 closeout, including M10's placeholder
per-chunk terrain images. V2 deliberately deferred the complete production-art replacement. V3
supersedes this document's sprite/tileset sequencing with a staged 3D gameplay-world migration.
That migration uses primitives for exact dynamic geometry and fallbacks plus a curated subset of
the user-supplied CC0 Kenney GLBs for first-release environments, characters, weapons, and supplied
animation. Original production art remains later work. Each V3 milestone still re-verifies exact
Bevy 0.19.1 APIs against pinned sources—the checked-in Bevy reference is 0.20-dev and must not be
trusted for exact APIs.

## Research references

- [zedpxl pixel-art forest asset pack](https://zedpxl.itch.io/pixelart-forest-asset-pack) — terrain density and palette reference.
- [Pixelfrog Tiny Swords](https://pixelfrog-assets.itch.io/tiny-swords) — composition, elevation, and unit-proportion reference.
- [Nuclear Throne](https://store.steampowered.com/app/242680/Nuclear_Throne/) and
  [Enter the Gungeon](https://store.steampowered.com/app/311690/Enter_the_Gungeon/) — character
  motion model references (style only; no assets).
- [Environment, surface, and tile ideas](./09-environment-and-tile-ideas.md) — visual-tile
  vocabulary and the authoritative/presentation separation this direction builds on.
- [Maps and game modes](./04-maps-and-game-modes.md) — map recipe model the theme ID extends.
- [Network architecture](./08-network-architecture.md) — authority boundaries the presentation
  layer respects.
- [Milestone 10](./implementation/v1/milestone-10.md) — occupancy grid, chunk dirty tracking, and
  placeholder visuals the tile renderer replaces.
