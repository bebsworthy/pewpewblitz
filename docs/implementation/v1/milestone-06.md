# Milestone 06 — First map-recipe arena and presentation baseline

## Tracking

| Field | Value |
|---|---|
| Version | v1 — gameplay MVP |
| Roadmap | [roadmap.md](./roadmap.md) |
| Status | Specification review |
| Research | Post-M05 coherence and architecture review complete |
| Specification validation | Awaiting user validation |
| Implementation | Not started; blocked until Milestone 05 reaches its user-playtest and feedback gates |
| Verification | Not started |
| User validation/playtest | Not started |

This specification has been realigned with the completed Milestone 05 architecture remediation and
automated verification baseline. Update this table and the roadmap together whenever the milestone
changes phase. Production implementation must not begin until the user validates this revision and
Milestone 05 reaches the user-playtest and feedback gates required by the roadmap. Immediately before
implementation, rerun the accepted M05 automated baseline against the actual starting commit.

## Outcome

One symmetrical, controller-readable arena is loaded from a typed built-in `MapPreset`, validated and
resolved by the authoritative server, instantiated as server-owned collision/spawn/region state, and
reconstructed as client-only presentation from one bounded replicated `ResolvedMapSnapshot`. The
same resolver accepts a legal non-preset fixture without branching on the built-in preset ID.

The milestone replaces `GreyboxArenaDefinition` and the client's independently constructed greybox
with an explicit content/recipe/resolved/runtime boundary. It also establishes replaceable visual and
audio presentation, asset provenance, map-aware camera bounds, a controller-oriented HUD shell, and
visible loading/connection/error states. It does not add a player map editor, formal match lifecycle,
score, respawn rules, objectives, or destructible-terrain behavior.

## Decisions requiring specification validation

This revision recommends these choices:

1. Embed and synchronously validate `content/v1/maps.ron`, as Milestone 05 does for weapon content;
   do not add a Bevy custom asset or hot reload for authoritative map data in v1.
2. Replicate one immutable, bounded `ResolvedMapSnapshot` component once on each server-created
   map-root instance. A replacement creates a new monotonic `MapInstanceId`; it never mutates the
   snapshot in place. Server colliders and client presentation entities remain local to their roles.
3. Support only axis-aligned rectangular playable/camera bounds and rectangle/circle authored shapes
   in this milestone. Polygonal geometry, slopes, and arbitrary paths wait for demonstrated need.
4. Use a 64-world-unit visual grid (16 source pixels at 4 world units per source pixel) with nearest
   filtering. Begin with the CC0 Sci-Fi Facility and Kenney Shape Characters packs, retaining
   primitives as explicit per-profile fallbacks.
5. Add client-only Bevy audio with Vorbis decoding and a small CC0 Kenney sound subset. Audio remains
   non-spatial in this baseline and is driven only by already-authoritative replicated state/cues.
6. Use readiness resources and run conditions rather than introducing a global game `States` enum
   before Milestone 07 defines the match lifecycle.
7. Use provisional arena bounds `x = [-896, 896]`, `y = [-576, 576]`, preserving the current 720-unit
   vertical camera span while adding central, north, and south routes for all four weapons.
8. Define only sandbox/base layout requirements in M06, including a stable practice-dummy anchor;
   M07 owns the first concrete Wipeout layout schema and executable rules.
9. Move the global gameplay-content fingerprint envelope to neutral `src/content.rs`; weapon and map
   definitions contribute canonical material without either domain owning the other.
10. Treat the 64 KiB snapshot ceiling as a candidate maximum that must pass real Lightyear
    fragmentation tests under typical and adverse impairment before it becomes an accepted bound.

Changing one of these choices during implementation returns the milestone to specification review if
it alters authority, wire format, content grammar, dependencies, or milestone scope. Numeric layout or
presentation tuning can be recorded as an implementation/playtest adjustment when the contract stays
unchanged.

## Source requirements

- [Product direction](../../00-product-direction.md): combat readability, replaceable content,
  user-authorable map recipes, and network-first simulation.
- [Maps and game modes](../../04-maps-and-game-modes.md): map catalog/recipe/preset/resolved/runtime
  separation, map-builder boundary, first-map grammar, and developer-owned mode rules.
- [Gameplay MVP](../../05-gameplay-mvp.md): one map recipe, four weapon range profiles, controller
  readability, future legal layout changes, and headless authority.
- [MVP asset shortlist](../../07-mvp-asset-shortlist.md): provisional visual packs, pixel filtering,
  collision/presentation separation, and provenance requirements.
- [Network architecture](../../08-network-architecture.md): server map resolution, replicated stable
  identity/data, recovery, and no client-authored live map edits.
- [Environment, surface, and tile ideas](../../09-environment-and-tile-ideas.md): visual/gameplay
  separation and the inert Milestone 10 terrain reservation.
- [Version 1 roadmap](./roadmap.md): Milestone 06 deliverable, scope, verification, and exit criteria.
- [Milestones 03–05](./milestone-05.md): current bounds/camera/spawn behavior, Avian layers and
  movement queries, combat terrain queries, content fingerprint, selected weapon state, cues, and HUD.

## Scope boundaries

### In scope

- one embedded versioned RON map catalog containing one built-in preset and stable numeric IDs;
- typed `MapContentCatalog`, `MapRecipe`, `MapPreset`, immutable `ResolvedMap`, bounded public
  `ResolvedMapSnapshot`, and runtime ECS state with explicit ownership/lifetimes;
- stable recipe, preset, presentation, collision-profile, region-profile, entity-definition, spawn,
  placement, mode-definition, and mode-anchor IDs;
- deterministic validation, normalization, canonical recipe fingerprinting, resolution, and
  snapshot-size enforcement independent of preset identity;
- rectangular playable/camera bounds, synthesized perimeter collision, permanent rectangle/circle
  geometry, visual placements/fills, inert decoration placements, inert typed regions, team spawn
  areas/points, and typed mode anchors;
- the sandbox/base layout requirement schema plus a stable generic seam through which Milestones 07
  and 09 add their concrete mode requirements;
- server-local Avian static colliders plus immutable spawn/region indexes derived from the resolved
  map; spawn or region ECS entities exist only where a current query requires entity identity;
- one replicated map-root snapshot used for client reconstruction, late join, reconnect, and map
  replacement recovery;
- one symmetrical built-in layout with an open center, north/south routes, permanent cover, spawn
  shielding, chokepoints, and one visibly reserved but inert destructible-terrain region;
- client-only asset loading, nearest filtering, fighter/map sprites or explicit primitive fallbacks,
  presentation-driven projectile/combat feedback, and minimal one-shot audio;
- stable team palette plus non-color team/owner markers, health/ammo/weapon state, map/session state,
  aiming/range feedback, and a reserved match-information HUD area for Milestone 07;
- loading, connecting, handshaking, waiting-for-map, ready, rejected, disconnected, and asset-failure
  presentation with neutral gameplay input until locally ready;
- asset provenance records and client/server feature-isolation verification.

### Out of scope

- a player-facing map editor, drag/drop authoring, undo/redo, save UI, or map preview tool;
- user-map persistence, account ownership, revisions from a service, publishing, discovery,
  moderation, migration, ratings, remote download, or runtime content distribution;
- arbitrary file/URL references, uploaded assets, scripts, Rust, behavior graphs, custom component
  blobs, or executable mode rules inside a recipe;
- production art direction, animation set, music, voiceover, accessibility menu, audio mixer UI,
  camera shake, vibration, post-processing, or spatial audio;
- lobby, ready check, formal team selection, countdown, score, timer, victory, respawn, restart,
  results, or bots; those belong to Milestone 07;
- active objective rules or Hot Zone regions; Milestone 09 owns the first objective region;
- Wipeout-specific layout requirements or executable Wipeout rules; Milestone 07 owns both;
- terrain masks, destruction brushes, generated colliders, terrain revisions, or recovery; the M06
  terrain reservation is data and presentation only, and Milestone 10 owns behavior;
- concealment, speedways, slow/slippery surfaces, hazards, pickups, traversal, doors, moving cover,
  interactive props, deployables, or visibility/interest management;
- navmesh generation, automated competitive-fairness scoring, procedural generation, polygonal
  editing, or arbitrary concave collider input;
- map selection requests or mid-sandbox map switching; the server starts with the one configured
  built-in preset.

## Research questions and conclusions

### Version, dependencies, and role isolation

- [x] Retain Rust 1.95, Bevy `=0.19.1`, Lightyear `=0.29.0`, Avian 2D `=0.7.0`, RON 0.12, the
  existing one-package topology, and the 60 Hz server-authoritative simulation.
- [x] Use Bevy's existing `AssetServer` and retained handles. The version-pinned Bevy 0.19 loading
  example confirms asynchronous recursive dependency state; no `bevy_asset_loader` dependency is
  justified for this bounded asset set.
- [x] Configure `ImagePlugin::default_nearest()` in the windowed client. The Bevy 0.19 sprite-sheet
  example confirms the exact filtering and atlas surface. Do not copy the checked-in 0.20-dev
  tilemap-chunk API into the 0.19 project.
- [x] Add only `bevy/bevy_audio` and `bevy/vorbis` to `bevy-client`. `bevy_audio`, decoder features,
  asset handles, images, fonts, and runtime asset paths remain absent from the isolated server graph.
- [x] Keep map RON compiled into both roles with `include_str!`. The server validates gameplay and
  stable presentation IDs but never loads PNG, font, or OGG files.

### Map representation and resolver boundary

- [x] Do not serialize an ECS `World`, Bevy scene, Avian collider, asset handle, or arbitrary
  component. A recipe is a bounded arrangement of typed primitives and stable IDs.
- [x] Keep `MapPreset` as named developer content containing an ordinary `MapRecipe`; resolver and
  instantiation systems never branch on `MapPresetId`.
- [x] Resolve through a pure function accepting the recipe, code-owned engine limits, catalog
  policy, and developer-owned `MapLayoutRequirements`. This lets Milestones 07 and 09 supply mode
  requirements without making a recipe executable or requiring a generic `GameMode` trait.
- [x] Canonicalize ID-keyed collections, rotations, and signed zero before fingerprinting with the
  same postcard plus fixed FNV-1a compatibility approach established by Milestone 05. RON text
  whitespace/order is not identity.
- [x] Keep the resolved public snapshot bounded and directly serializable. Server-only lookup
  indexes are derived resources and never become a second mutable simulation model.

### Networking and recovery

- [x] Replicate one map root with the full bounded `ResolvedMapSnapshot`, rather than replicating
  every wall/region/spawn entity or assuming all clients possess the built-in recipe forever.
- [x] Register the snapshot and map identity as replicate-once components, not interpolation,
  prediction, or a transient message. A late/reconnecting client receives current state through
  Lightyear entity replication; replacing the map despawns the old root and creates a new instance.
- [x] No client-to-server map message exists in M06. The server selects, validates, resolves, and
  instantiates the map before it accepts gameplay sessions.
- [x] Extend the shared gameplay-content fingerprint with normalized map catalog/preset data,
  stable presentation IDs, and the sandbox layout-schema version. Move global fingerprint-envelope
  ownership to a neutral shared module, retaining a compatibility re-export if needed; map content
  must not depend on combat as the owner of global compatibility. This rejects incompatible clients
  before fighter spawn, while the replicated snapshot remains authoritative for the selected layout.
- [x] Client map presentation validates snapshot bounds and known presentation IDs before use, but
  this defensive check never grants gameplay authority or creates client colliders.

### Collision, schedules, and lifecycle

- [x] Reuse Avian static rectangle/circle colliders and existing collision layers. Map geometry is
  converted to `RigidBody::Static`, `Collider`, `CollisionLayers`, `Position`, and `Rotation`; map
  data never stores an Avian component blob.
- [x] Replace `GreyboxArenaDefinition` with `ResolvedMap`, `PlayableBounds`, and stable spawn lookup
  derived from the resolved snapshot. Movement and combat continue reading authoritative bounds and
  `ArenaWall` collider entities.
- [x] Instantiate the authoritative map before binding the server endpoint. Map entities are
  map-instance-owned, not connection-owned, and disconnect cleanup cannot remove them.
- [x] The map is immutable during M06 fixed simulation. It adds no per-tick server map system and no
  new ordering inside the accepted M05 combat pipeline.
- [x] Client asset readiness and snapshot arrival are orthogonal to the existing connection and
  selection lifecycle. Derive one playable gate from those existing facts rather than duplicating
  them in another mutable state machine. Local gameplay intent is neutral until that gate is ready.

### Presentation and asset choice

- [x] Use Sci-Fi Facility for floor/wall/decorative context and Shape Characters for fighter
  silhouettes, both under CC0. Inspect the downloaded archives and commit only the runtime subset;
  the font shown in the Sci-Fi Facility preview is explicitly not part of that pack.
- [x] Use small CC0 subsets from Kenney Sci-fi Sounds, Impact Sounds, and Interface Sounds for fire,
  impact/hit, defeat/reset, reload, and session feedback. Exact source filenames are selected and
  recorded during implementation after archive inspection.
- [x] Stable presentation IDs map to client-local paths and sizing metadata. Recipes/snapshots never
  contain a path, URL, `Handle`, raw audio, or arbitrary color string.
- [x] Keep primitive fallbacks for every gameplay-critical visual profile and show an explicit
  degraded/failure overlay. Optional decorations may fall back or be omitted. A corrupt required
  visual mapping with no declared fallback fails readiness rather than starting invisibly. Audio
  loading or output-device failure is presentation-only and never blocks gameplay readiness.
- [x] Keep audio non-spatial for the first baseline, coalesce multi-delivery attacks, and bound
  simultaneous one-shots. Positional mixing is a later playtest-driven choice.

## Research log

| Date | Source | Finding | Decision |
|---|---|---|---|
| 2026-08-14 | `docs/{00-product-direction,04-maps-and-game-modes,05-gameplay-mvp,07-mvp-asset-shortlist,08-network-architecture,09-environment-and-tile-ideas}.md` | M06 must prove future map-authoring boundaries without importing editor/platform services or mode execution. | One built-in preset and one non-preset fixture use the same bounded resolver; mode logic remains developer-owned. |
| 2026-08-14 | `docs/implementation/v1/{roadmap,milestone-03,milestone-04,milestone-05}.md`, `AGENTS.md`, and current `src/{movement,client,server,combat}/`, `src/protocol.rs`, and `tests/network/` | M05 remediation split the former large modules, established embedded RON, stable presentation IDs, Avian terrain queries, authoritative cues, one server verification owner, and a shared network harness. Its automated baseline is green (73 client, 60 server, 38 network, 7 performance tests and all 12 process profiles); hardware/specification/user gates remain. | M06 extends those focused modules and verification seams, removes the greybox as the one remaining duplicate arena truth, and does not recreate role-level god modules. |
| 2026-08-14 | `docs/04-maps-and-game-modes.md` | M06 owns only sandbox/base requirements and the generic mode-layout seam; M07 owns concrete Wipeout requirements and rules. | Use `SandboxLayoutRequirements` plus a typed practice-dummy anchor. Defer all Wipeout-specific schema and execution to M07. |
| 2026-08-14 | `references/lightyear/book/src/concepts/transport/{serialization,packet}.md` and `references/lightyear/crates/transport/transport/src/packet/` | Messages above the roughly 1200-byte packet limit are fragmented; fragmentation support does not by itself prove a 64 KiB replicated component is reliable under the project impairment profiles. | Keep one immutable snapshot consistency unit, but make the 64 KiB ceiling conditional on maximum-size real-transport evidence. |
| 2026-08-14 | `references/bevy/examples/README.md`, `asset/asset_loading.rs`, `showcase/loading_screen.rs`, `2d/{sprite_sheet,tilemap_chunk}.rs`, and `audio/{audio,play_sound_effect}.rs` | Assets load asynchronously and must retain handles; nearest filtering is an app-level image-plugin choice; audio is entity/component driven; the local tilemap example is 0.20-dev. | Use retained handles and readiness polling, nearest filtering, one-shot `AudioPlayer`, and simple M06 sprite instances confirmed against Bevy 0.19. |
| 2026-08-14 | [Bevy 0.19 loading-screen example](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/showcase/loading_screen.rs), [sprite-sheet example](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/2d/sprite_sheet.rs), and [audio example](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/audio/audio.rs) | Version-pinned primary sources confirm `get_recursive_dependency_load_state`, `ImagePlugin::default_nearest`, texture atlases, `AudioPlayer`, and retained asset handles. | Exact implementation APIs must come from 0.19, not the checked-in Bevy main snapshot. |
| 2026-08-14 | `references/lightyear/examples/README.md`, `simple_box/src/{protocol,server}.rs`, and book `concepts/replication/{protocol,replicate}.md` plus `advanced_replication/replication_logic.md` | Registered replicated components on a `Replicate` entity provide durable spawn/component state; entity actions are ordered/reliable and groups preserve consistency. | Replicate one immutable map snapshot component on one map root; do not create a custom snapshot message channel. |
| 2026-08-14 | [Lightyear 0.29 simple-box protocol](https://github.com/cBournhonesque/lightyear/blob/0.29.0/examples/simple_box/src/protocol.rs) | The pinned protocol API registers serializable components directly and adds direction/sync behavior through the app. | Register the snapshot as an ordinary server-to-client component with no prediction/interpolation. |
| 2026-08-14 | `references/avian/README.md`, `crates/avian2d/Cargo.toml`, and examples `collision_layers.rs` and `move_and_slide_2d.rs`; [Avian 0.7 examples](https://github.com/avianphysics/avian/tree/v0.7.0/crates/avian2d/examples) | Avian 0.7 targets Bevy 0.19 and directly represents static bodies, rectangle/circle colliders, and collision filters as ECS components. | Resolve typed shapes into existing static collider bundles; no new physics dependency or rigid-body simulation. |
| 2026-08-14 | [Kenney Shape Characters](https://kenney.nl/assets/shape-characters) and [OpenGameArt Sci-Fi Facility](https://opengameart.org/content/sci-fi-facility-asset-pack) | Both pages identify CC0 licensing; Sci-Fi Facility supplies 16×16 facility tiles and explicitly excludes its preview font. | Use a small recorded runtime subset and the Bevy default font for M06 unless a separately licensed font is approved. |
| 2026-08-14 | [Kenney Sci-fi Sounds](https://kenney.nl/assets/sci-fi-sounds), [Impact Sounds](https://kenney.nl/assets/impact-sounds), and [Interface Sounds](https://kenney.nl/assets/interface-sounds) | Each source is CC0 and provides a bounded provisional audio pool. | Select only the cues required by this milestone and record every imported file in the manifest. |

## Post-M05 review resolution

The coherence review found and resolves these specification-level problems:

- stale flat-file module paths and permissive “keep it in `client.rs`” language are replaced by the
  focused module layout and migration table below;
- concrete Wipeout layout requirements are removed from M06 and returned to M07;
- the M05 sandbox team adapter and dummy lifecycle are preserved but obtain authored map poses,
  eliminating both greybox and environment-dependent coordinate sources;
- immutable snapshot semantics, monotonic instance identity, replacement ordering, and duplicate-root
  handling now form one recovery model;
- global content compatibility moves out of combat ownership and has one deterministic initialization
  order before either client or server handshake;
- server startup has one composition owner and a tested fail-before-bind boundary;
- client playability derives from existing connection/selection facts plus map/visual readiness,
  while headless automation remains independent of renderer/assets/audio;
- only physics-queryable map objects become server ECS entities; inert spawn/region data remains
  immutable indexed content until a real system needs entity identity;
- M05's process checkpoints, exit gate, impairment matrix, performance cases, and shared network
  harness are extended rather than duplicated;
- required visual fallback behavior, optional audio degradation, and the provisional fragmented-
  snapshot transport gate no longer contradict readiness or failure semantics.

## Technical specification

### Application and module composition

Keep one package and the existing `client`, `server`, and `network-test` configurations. Add a
cohesive `src/map/` module family only when implementation begins:

```text
src/map/
  mod.rs                 composition root, shared schedule sets, and intentional re-exports
  model.rs               stable map IDs, shapes, bounds, resolved data, and public snapshot
  definitions/
    mod.rs               catalog/policy parsing, validation, and canonicalization
    resolver.rs          pure preset-independent resolution
    tests.rs             definition and resolver tests
  server.rs              authoritative resources, instantiation, and lifecycle
  client.rs              snapshot validation/reconciliation and map-only presentation
  tests.rs               shared map small-App tests
src/client/
  assets.rs              retained asset catalog, readiness, fallbacks, and provenance mapping
  audio.rs               bounded cue-to-audio presentation
  hud.rs                 HUD plus readiness/error presentation shell
src/content.rs            neutral gameplay-content fingerprint envelope
src/protocol.rs           all wire registration, including map components
tests/network/map.rs      map authority, lifecycle, and recovery scenarios using the shared harness
```

This is a module boundary inside the current package, not a new crate or public service layer.
Do not create every listed file pre-emptively: add a leaf only when its named responsibility exists.
`src/movement/` retains movement/input and physics-integration rules but stops owning authored arena
data or spawning arena geometry. `src/combat/` retains weapon/damage rules and consumes map bounds and
semantic wall queries through the map API. `src/server/mod.rs` remains the application composition
root and startup owner. `src/client/presentation.rs` remains general presentation composition; it
does not absorb map reconstruction, all HUD code, assets, and audio into another giant module.
Network registration remains in root `src/protocol.rs`; there is no `src/map/protocol.rs`.

Plugin responsibilities:

| Plugin | Installed in | Responsibility |
|---|---|---|
| `MapContentPlugin` | client, server, tests | Parse/validate embedded catalog and register code-owned limits plus sandbox layout requirements. Expose deterministic fingerprint material to the neutral content envelope. No filesystem assets. |
| `AuthoritativeMapPlugin` | server and authoritative tests | Resolve the configured preset before endpoint startup; insert immutable `ResolvedMap`/`PlayableBounds`/spawn and region indexes; spawn the map root and static colliders; clean exact map-instance state. |
| existing `ProtocolPlugin` extension | client, server, tests | Register map-root marker, instance/source identity, bounded `ResolvedMapSnapshot`, and any exposed spawn assignment; bump protocol identifiers. |
| `ClientAssetPlugin` | windowed client only | Load and retain client asset handles, validate stable-ID mappings/provenance, expose visual readiness/fallback/degraded facts, and never define gameplay geometry. |
| `MapPresentationPlugin` | windowed client only | Reconcile the replicated snapshot into map visuals, expose camera bounds/map readiness, render inert regions/decorations, and clean replacement/despawn state. |
| `CombatPresentationPlugin` extension | windowed client only | Preserve M05 world-space weapon/projectile/effect feedback while consuming the new stable presentation mappings. |
| `AudioPresentationPlugin` | windowed client only | Convert existing authoritative session/combat facts into bounded, coalesced local one-shots. |
| `HudPresentationPlugin` | windowed client only | Compose controller-readable health/weapon/session/map/match shell and connection/error overlays in `src/client/hud.rs`. Combat keeps world-space combat feedback rather than owning the whole application HUD. |

Do not install map presentation, asset, audio, UI, camera, or device-input code in headless clients or
the dedicated server. Do not move server authority into `GameplayPlugin`, which is shared by both roles.

Migration ownership is explicit:

| Current M05 location/contract | M06 owner after migration |
|---|---|
| `movement/arena.rs::GreyboxArenaDefinition` and authored wall/spawn construction | `map::model` definitions plus `map::server` instantiation; delete the greybox definition after all consumers migrate |
| movement collision integration and general geometry/query helpers | remain in `src/movement/`; consume `PlayableBounds` and map-spawned semantic `ArenaWall` colliders |
| fighter creation in `src/server/mod.rs` | remains session-owned; obtains `SpawnAssignment`/`SpawnState` from `SpawnPointCatalog` |
| sandbox team mapping and test-dummy lifecycle in combat | remain M05 compatibility behavior; consume map team slots and `PracticeDummySpawn` rather than coordinates |
| combat lob clamp/repair and terrain queries | remain in focused `src/combat/` modules; consume role-local authoritative/client bounds and semantic wall data |
| general camera/sprite composition in `src/client/presentation.rs` | stays general; map reconstruction moves to `map/client.rs`, application HUD to `client/hud.rs`, assets/audio to their focused modules |
| `combat::GameplayContentFingerprint` | neutral `src/content.rs` envelope; weapon/map definitions only contribute canonical material |
| protocol registration/fingerprints in `src/protocol.rs` | remain in `src/protocol.rs`, extended with map wire types |
| process checks in `src/server/verification.rs` and `combat/evidence.rs` | extend the same evidence contract with map facts; do not create parallel clocks or exit gates |
| monolithic integration entry points | add focused `tests/network/map.rs` scenarios using `tests/network/harness.rs` and existing lifecycle helpers |

No compatibility wrapper may leave both `GreyboxArenaDefinition` and `ResolvedMap` as live sources of
truth. A short-lived re-export is allowed only while a vertical migration slice remains green and is
removed before the milestone exits implementation.

### Authored content files and provenance

Implementation adds:

```text
content/v1/maps.ron                 shared typed catalog plus built-in preset
assets/brawler/...                  curated runtime PNG/OGG subset
assets/manifest.ron                 provenance for every imported runtime file
third_party/...                     optional original archives/license text, never runtime-loaded
```

Do not create `third_party/` unless source archives/license text are actually retained. Every
`assets/manifest.ron` entry records:

```text
stable presentation/audio ID
runtime relative path
pack and original filename
author
license identifier and license URL
source page URL
import date
required versus optional/fallback status
```

The shared catalog contains allowed stable presentation IDs and semantic sizing/layer categories,
but no runtime path. The client-only presentation catalog is the only place that maps an ID to an
`AssetServer` path and handle type. Tests parse the manifest and prove every referenced runtime path
has one provenance entry and no unreferenced imported file is silently shipped.

### Typed authored model

Exact field visibility may follow local style, but the semantic contract is:

```text
MapContentCatalog
  schema_version: u16
  policy: MapRecipePolicy
  allowed presentation/collision/region/entity definition records
  presets: Vec<MapPreset>

MapPreset
  id: MapPresetId(u16)
  key/display name
  recipe: MapRecipe

MapRecipe
  recipe_id: MapRecipeId(u64)
  revision: u32
  recipe_version: u16
  mode_definition_id: ModeDefinitionId(u16)
  presentation_theme_id: MapPresentationThemeId(u16)
  playable_bounds: AxisAlignedMapRect
  camera_bounds: AxisAlignedMapRect
  geometry: Vec<GeometryPlacement>
  visuals: Vec<VisualPlacement>
  entities: Vec<MapEntityPlacement>
  regions: Vec<MapRegionPlacement>
  spawn_areas: Vec<TeamSpawnArea>
  spawn_points: Vec<TeamSpawnPoint>
  mode_anchors: Vec<ModeAnchorPlacement>
```

All placement collections use a single globally unique `MapPlacementId(u32)` namespace. Semantic
objects additionally use `SpawnPointId`, `RegionId`, or `ModeAnchorId`; these are stable recipe IDs,
not Bevy entities. Every network-visible identity uses these IDs plus `MapInstanceId`, never a
process-local entity.

M06 authored shapes are only:

```text
MapShape::Rectangle { half_extents: Vec2 }
MapShape::Circle { radius: f32 }
```

Each placement has a finite position and rotation. Rectangles may rotate for cover/regions, although
the playable and camera bounds remain axis-aligned. Geometry explicitly references a collision
profile and may independently reference a presentation profile. A visual profile never creates a
collider. A decoration/entity profile creates only the developer-authored behavior associated with
that known ID; M06's allowed entity profiles are inert presentation props.

`VisualPlacement` supports one sprite instance or a bounded tiled-rectangle fill. Resolution expands
tiled fills into row-major, stable `ResolvedVisualInstance` records so the network snapshot and client
do not rely on a Bevy tilemap or serialized render component. The built-in floor uses 64-unit cells.

`MapRegionPlacement` supports only the inert `DestructibleReservation` profile in M06. It is visible
to clients and queryable/identifiable on the server but applies no movement, damage, terrain, or
collision rule. A region profile cannot smuggle executable parameters beyond its catalog-defined
shape/metadata fields.

### Code-owned limits and catalog policy

`EngineMapLimits` is non-authorable and cannot be widened by RON policy:

| Limit | First value |
|---|---:|
| Maximum absolute world coordinate | 4096 units |
| Playable width | 1024–4096 units |
| Playable height | 720–3072 units |
| Minimum authored shape diameter/side | 8 units |
| Maximum authored shape diameter/side | 2048 units |
| Permanent geometry/collider placements | 256 |
| Expanded visual instances | 1024 |
| Entity placements | 128 |
| Regions | 32 |
| Spawn areas | 8 |
| Spawn points | 32 |
| Mode anchors | 32 |
| Destructible reservations | 4 |
| Serialized candidate recipe | 96 KiB |
| Candidate serialized resolved public snapshot | 64 KiB, accepted only after the transport gate |
| Key/display string | 32/64 bytes |

Catalog policy may narrow counts, dimensions, supported definition IDs, or shapes. Validation rejects
non-finite values; negative/zero sizes; over-wide policy; unknown/duplicate IDs; duplicate global
placement IDs; control characters; unsupported combinations; objects outside bounds; camera bounds
outside playable bounds; geometry overlapping the invalid exterior; expansion over count/byte limits;
and any snapshot that cannot round-trip through the registered serialization.

Normalize `-0.0` to `0.0`, rotations to `[-pi, pi)`, every ID-keyed collection into ascending order,
and tiled fills into row-major order. Canonical postcard bytes include explicit fingerprint-format,
catalog-schema, recipe-schema, and layout-schema versions. `MapRecipeFingerprint(u64)` uses the fixed
FNV-1a compatibility hash already used by M05 and is not a security signature.

### Sandbox layout validation and the future mode seam

The resolver accepts a developer-owned value rather than asking the recipe for executable behavior:

```text
MapLayoutRequirements
  mode_definition_id
  schema_version
  allowed team slots
  minimum/maximum spawn areas and points per team
  required anchor definitions and shape/count bounds
  allowed region/entity profiles for this layout
```

M06 provides only `SandboxLayoutRequirements`: exactly team slots 0 and 1, one spawn area per team,
at least three and at most eight spawn points per team, and one typed `PracticeDummySpawn` point
anchor. The built-in preset supplies four points per team. The existing M05 `sandbox_team(player_id)`
mapping remains a temporary server-session adapter: the session layer maps that team to a stable map
spawn; the map module does not own team allocation. The combat sandbox dummy receives its
`SpawnState` from the practice anchor. This removes the final hard-coded dummy/fighter poses without
claiming formal team, respawn, score, timer, or victory rules.

Milestone 07 supplies and versions concrete Wipeout requirements through the same
`MapLayoutRequirements` value, then revalidates this preset. Milestone 09 does likewise for Hot Zone.
Neither mode's concrete schema is pre-authored here.

The built-in sandbox requirements exercise missing, duplicate, wrong-shape, and unsupported anchor
validation directly. A second legal non-preset sandbox fixture proves the resolver does not branch
on the built-in preset ID.

### Spawn-safety validation

For every spawn point, the pure resolver verifies:

- it belongs to a declared team and lies inside that team's spawn area with a 32-unit inset;
- a fighter circle of radius 24 plus 8 units of safety does not overlap permanent geometry or bounds;
- same-team points are at least 64 units apart and opposing points are at least 600 units apart;
- facing is finite and points generally into the playable area (dot product toward arena center > 0);
- a 96-unit, fighter-width egress segment along facing is clear of permanent geometry;
- each team has the required count and stable IDs, and no spawn area overlaps the opposing area.

These rules catch malformed/blocked spawns but do not claim automated competitive fairness. A small
32-unit-cell clearance grid, with permanent geometry inflated by the fighter radius, must also prove
that every spawn reaches the central combat probe. Visual and multiplayer playtests judge route
quality and spawn trapping; no navmesh becomes runtime gameplay state.

### Provisional built-in arena

`MapPresetId(1)` uses a 28×18 grid of 64-unit cells:

| Element | Provisional value |
|---|---|
| Playable/camera bounds | `x = [-896, 896]`, `y = [-576, 576]` |
| Camera vertical span | existing 720 world units |
| Team 0/1 spawn areas | left/right rectangles centered at `x = -768/+768`, size `192 × 768` |
| Spawn points | `x = -768/+768`, `y = {-288, -96, 96, 288}`, facing inward |
| Practice dummy anchor | one stable point near the south-center approach lane, selected once for all process profiles |
| Center | open rectangle approximately `x = [-320, 320]`, `y = [-192, 192]` |
| Center side cover | horizontal rectangles at `(0, +/-256)`, size `320 × 64` |
| Center entry cover | vertical rectangles at `(+/-384, 0)`, size `64 × 256` |
| Spawn shields | vertical rectangles at `(+/-576, 0)`, size `64 × 192` |
| Side routes | north/south lanes outside the center-side cover |
| Chokepoints | gaps between center-entry and center-side cover, mirrored on both axes |
| Destruction reservation | inert, marked `192 × 192` square centered at `(0, 0)` |

Perimeter colliders are deterministically synthesized outside the playable rectangle from the bounds
and existing 48-unit wall thickness. All other geometry is explicit recipe data. The layout is
symmetric across both axes and under 180-degree rotation. Tests assert the coordinate symmetry and
stable resolved ordering; weapon and spawn-flow suitability remains a visual/playtest gate.

The intended weapon reads are: pulse can contest center and portions of side lanes without crossing
the complete arena; scatter and blade can use gate/side cover to close distance; launcher can lob
over permanent cover into exits but its 150-unit area does not cover an entire route; every spawn has
two immediate exits around its shield. Playtest tuning may move/resize cover without changing the
grammar.

The practice anchor replaces both hard-coded M05 dummy coordinates, including the environment-
dependent process-evidence branch. The chosen authored pose must let the unchanged 12-profile M05
combat evidence scenario reach the dummy with every delivery family; process mode may not resolve a
different map or silently move the target.

### Resolved map and public snapshot

The pure resolver produces normalized immutable data:

```text
ResolvedMap
  identity/source preset/recipe fingerprint/revision/schema versions
  playable and camera bounds
  sorted resolved permanent geometry
  sorted expanded visual instances
  sorted entity and inert region instances
  sorted spawn areas/points
  sorted validated mode anchors
  derived server lookup indexes (not serialized)

ResolvedMapSnapshot
  the bounded serializable public fields above, excluding server-only indexes
```

`ResolvedMap` and `ResolvedMapSnapshot` must agree exactly on all public gameplay/presentation data.
`MapRecipeFingerprint` identifies canonical content. `MapInstanceId` is a distinct monotonically
increasing, nonzero server-issued generation ID and is never derived from the recipe hash. The
snapshot carries both identities plus source preset identity. A legal non-preset fixture sets
`source_preset_id = None` and still resolves/instantiates without adding an enum branch or system.

The client never re-resolves a local preset as the selected truth. It uses the server snapshot and
checks that schema versions, size bounds, fingerprint shape, and all presentation IDs are understood.
This is the seam required for a later server-approved custom recipe whose exact arrangement is not
compiled into the client.

### ECS ownership and lifecycle

#### Authoritative server

One replicated map-root entity owns:

- `MapRoot`, `MapInstanceId`, `ResolvedMapIdentity`, and `ResolvedMapSnapshot`;
- `Replicate::to_clients(NetworkTarget::All)` with no owner, interpolation, or prediction target;
- app/match scope, never `ControlledBy` and never session-based cleanup.

Derived server runtime entities contain `MapInstanceMember { map_instance_id, placement_id }` and:

- permanent geometry: existing `ArenaWall`, `RigidBody::Static`, typed `Collider`, terrain collision
  layers, `Position`, and `Rotation`;
- inert entity placements only when a current server query demonstrably needs entity identity.

Spawn points and inert regions otherwise remain sorted immutable indexes inside `ResolvedMap`; they
are not speculative ECS entities. Permanent colliders are entities because Avian queries consume
them. Pure decorations remain only in the resolved snapshot for client presentation.

`ResolvedMap` is an immutable server resource. `PlayableBounds` and `SpawnPointCatalog` are small
derived resources for hot queries. Movement reads `PlayableBounds`; session fighter creation selects
a team point deterministically from `SpawnPointCatalog`; lob clamp/repair and terrain queries use the
same bounds and `ArenaWall` set. The current code-authored `GreyboxArenaDefinition`, cover helpers,
and separately initialized spawn arrays are removed after migration.

Fighters gain stable `SpawnAssignment { map_instance_id, spawn_point_id }`. Server session code owns
assignment using the existing sandbox team adapter plus `SpawnPointCatalog`; map code only exposes a
deterministic selection helper. Sandbox reset continues using the assigned server-owned `SpawnState`.
The neutral dummy uses the sandbox practice anchor. Milestone 07 replaces this adapter with formal
match/team/respawn selection without changing the map resolver.

Map replacement or app shutdown removes every member of that exact `MapInstanceId`, then its root and
derived resources. It never uses a broad `despawn` query without instance filtering. Disconnect only
removes session-owned fighters/deliveries and cannot affect map entities.

#### Client

The replicated root/snapshot is durable network state. Client-local resources/components include:

- `PresentationAssetCatalog`: retained image/audio handles keyed by stable IDs;
- `PresentationAssetReadiness`: loading/ready/failed with exact missing/failed IDs;
- `PresentedMap`: current source root, instance ID, recipe fingerprint, and generation;
- `MapPresentationMember`: local instance ID, stable placement ID, and visual category;
- `ClientPlayableBounds` and `ClientCameraBounds`, derived from the current snapshot;
- `MapPresentationReadiness`: waiting for snapshot, loading visuals, ready, degraded, or fatal
  presentation error;
- `ClientPlayableGate`: a derived fact combining the existing `ClientJoinPhase`, accepted weapon
  selection, valid map snapshot, required visual readiness, and pause/fatal state;
- local HUD, map/fighter sprites, region markings, effects, and audio entities.

Baseline interpolated clients do not create map colliders. M06 does not silently resolve the earlier
prediction decision. If an explicitly accepted prediction design later needs local terrain, that
work returns to specification review and creates separately tagged nonauthoritative colliders from
the server snapshot; client collision outcomes still cannot become authoritative.

Headless clients do not load renderer, asset, UI, or audio state: their playable/evidence gate uses
only connection, selection, and valid snapshot facts. Windowed clients build map visuals in stable
placement order. Reprocessing an unchanged instance/fingerprint is a no-op. A changed/replaced root
first despawns only the previous `MapPresentationMember` generation, then rebuilds and atomically
publishes the new bounds/readiness after `ApplyDeferred`. Removal/disconnect cleans local map visuals
and returns to waiting/disconnected presentation; it does not fabricate a map from the local preset.

The server maintains exactly one current root. During defensive client reconciliation, a higher
`MapInstanceId` supersedes an older root; duplicate roots with the same generation but different
identity are fatal protocol state. Cleanup is instance-scoped and tested across the transient
despawn/spawn ordering of replacement.

### Network protocol and recovery

Register these serializable components through the existing protocol plugin:

```text
MapRoot                         replicate once marker
MapInstanceId                  replicate once
ResolvedMapIdentity            replicate once
ResolvedMapSnapshot            replicate once
SpawnAssignment                replicate once in the M06 sandbox
```

`ResolvedMapSnapshot` is not interpolated, predicted, or client-authored. The server constructs the
root before accepting sessions and targets all clients, so new links receive it through normal
replication. The snapshot is one consistency unit on one entity; no cross-entity mapping is needed.

Move the global `GameplayContentFingerprint` envelope/type from combat ownership to neutral
`src/content.rs` (with a temporary compatibility re-export if existing call sites require it).
Weapon and map definitions each expose canonical, versioned fingerprint material; root
`ProtocolPlugin` composes the envelope after both catalogs exist. Include map catalog/presets,
allowed stable presentation IDs, engine/policy bounds, and sandbox layout-schema version. Bump
`NETWORK_PROTOCOL_ID` and `SUPPORTED_PROTOCOL_VERSION` for the registered component/wire change.
Registry and content mismatch keep using the existing controlled join rejection and cleanup path.

No map-selection, map-edit, map-delta, or asset message/channel is added. A client cannot submit a
recipe, geometry, spawn, region, anchor, resolved snapshot, asset path, or runtime map change.

The snapshot remains one bounded immutable document because the map is one consistency unit, but the
64 KiB ceiling is provisional rather than assumed transport-safe. Lightyear fragments messages above
its roughly 1200-byte packet limit; implementation must serialize the maximum legal snapshot and
prove delivery under the existing typical/adverse UDP profiles before accepting that ceiling. Reduce
the limit or design a separately reviewed chunk transfer if the evidence is not reliable.

Late join/reconnect receives the current root and reconstructs current presentation without history.
A synthetic test replacement despawns the old root and spawns a new `MapInstanceId`; Lightyear's
ordered/reliable entity actions remove stale network state. If a snapshot fails defensive client
validation or references a missing required asset, gameplay input remains neutral and the UI shows a
specific fatal presentation error; the client never installs partial colliders or continues invisibly.

### Startup, schedules, and deferred commands

Server startup ordering is explicit:

```text
MapStartupSet::Content
  parse/validate both catalogs and compose the shared content fingerprint
MapStartupSet::Resolve
  resolve configured built-in preset and insert immutable resources
MapStartupSet::Instantiate
  spawn root and perimeter/cover colliders; publish spawn/region indexes
MapStartupSet::Network
  bind endpoint and begin accepting links
```

`src/server/mod.rs` owns this composition. Use a chained/exclusive initialization pipeline with an
explicit deferred-command visibility boundary: catalog resources must exist before resolution;
resolved resources and local colliders must be observable before the existing endpoint is spawned
and `Start` is triggered. The combat sandbox dummy starts after map instantiation so it can consume
the practice anchor. An invalid embedded catalog/preset is a fail-closed developer startup error with
a precise diagnostic and non-successful app exit before socket bind or fighter spawn. Pure parser/
resolver tests report errors without panicking; a focused small-App test proves the no-bind invariant.
The client composition likewise creates both catalogs and the shared fingerprint before its endpoint
can send `ClientHello`; this is explicit schedule ordering, not incidental plugin insertion order.

Map geometry is immutable in M06, so no server map system runs in `FixedUpdate` or `FixedPostUpdate`.
Existing movement -> Avian refresh -> delivery/targeting -> damage/effects ordering remains
unchanged. Static colliders exist before the first fixed tick.

Client variable-rate ordering is:

```text
PreUpdate
  Lightyear receives/installs replicated snapshot
Update MapPresentationSet::Assets
  poll recursive asset dependency readiness and failures
Update MapPresentationSet::Reconcile
  validate new snapshot; queue old-generation cleanup/new visuals
Update ApplyDeferred
Update MapPresentationSet::ReadinessAndHud
  publish map readiness, derive the playable gate, and render status/error/HUD
Update MapPresentationSet::Feedback
  existing deduplicated combat visuals plus bounded audio
PostUpdate
  write interpolated fighter pose, follow/clamp camera, propagate transforms
```

Asset and snapshot changes are variable-rate presentation concerns. Gameplay outcomes never wait on
asset readiness on the server. Local input sampling may continue for menu/navigation, but
the final input writer emits neutral gameplay axes/buttons until `ClientPlayableGate` is true; it
must not create a second mutable connection/selection state machine. Pausing still neutralizes
through the existing context.

### Client visual baseline

Use `ImagePlugin::default_nearest()` and a 64-unit authored visual cell. Sci-Fi Facility's 16×16
pixels therefore render at four world units per source pixel. Larger/smaller source images receive an
explicit presentation-profile size; native pixel dimensions never define collision.

Z layers are stable and tested:

```text
-10 floor fills
-5  inert reservation/decal marks
0   low decorations
2   permanent cover/walls
10 fighters and projectiles
20 aim/range/impact world feedback
100 camera (existing transform convention)
UI  Bevy UI overlay, independent of world Z
```

Team presentation reads replicated `TeamId`, not `PlayerId`. Team 0 begins cyan/blue and Team 1
orange; each fighter also has a high-contrast outline/ground ring and a small stable team glyph so
recognition is not color-only. The locally controlled fighter has a white owner ring. The neutral
dummy remains visually distinct. Health bars, hit flash, aim/range previews, and status feedback are
separate children/components and do not depend on the fighter sprite image.

Every geometry presentation profile has a colored primitive fallback sized from the resolved shape.
Optional decoration failure uses its fallback or omission as declared by the manifest. Required
floor, wall, fighter, HUD, and feedback profiles must either load or have their declared primitive
fallback available before `ClientPlayableGate` opens. Optional audio/decor failures produce a visible
degraded state and diagnostics, not invisible gameplay or a gameplay block.

### HUD and readiness presentation

Replace the current debug-text scatter with a small layout shell that survives common aspect ratios:

- top-left: team glyph/color, health value/bar, and connection quality/status text;
- bottom-center: selected weapon name/icon, resource pips/count, cooldown/reload/recharge state, and
  primary-fire control hint;
- center/world: preserved M05 facing, scatter cone, lob landing/area, blade sector, hit/damage, and
  incoming-direction feedback;
- top-center: current arena/preset label plus an intentionally empty match-information slot that
  Milestone 07 will populate with score/timer/phase;
- context overlay: loading assets, connecting, handshaking, waiting for map, weapon selection,
  rejected, disconnected, and exact asset/snapshot error;
- scoreboard/help overlays remain controller accessible but do not display invented score/match data.

Use anchored/flex UI with 16 logical-pixel safe margins and bounded text, not percentage rectangles
that can overlap at 4:3. Test at 1280×720, 1440×900, 1024×768, and the smallest supported test window
960×540. World camera keeps the 720-unit vertical span and clamps to replicated camera bounds; when a
viewport is wider than the map axis, center that axis as the current helper does.
`camera_bounds` denotes the maximum visible world rectangle; the existing clamp helper derives legal
camera-center limits from that rectangle and the current viewport rather than treating it as a second
fighter-playable area.

### Audio and combat feedback

Enable Bevy audio and Vorbis only in the windowed client. Retain typed handles in
`PresentationAssetCatalog`; spawn one-shot `(AudioPlayer, PlaybackSettings::DESPAWN)` entities from
deduplicated authoritative facts.

Initial mappings use the M05 cue vocabulary exactly:

- `AttackAccepted`: one fire/swing/launch sound selected by weapon presentation profile;
- `DeliveryImpact`/`LobLanded`/`MeleeContact`: one bounded impact sound per attack/frame group;
- `DamageApplied`: local hit-confirm or incoming-damage cue according to source/target ownership;
- `FighterDefeated` and `FighterReset`: distinct authoritative lifecycle cues; the shorter `Defeat`
  and `Reset` presentation/evidence cues must not cause duplicate playback;
- replicated `WeaponState` phase transition: reload/recharge completion cue;
- join accepted/rejected/disconnected: restrained interface cues.

Scatter pellets and multi-target melee may create many visual contacts but play at most one contact
sound for the same `(AttackId, cue family)` in one rendered frame. Keep at most 32 live one-shots,
prioritize local damage/defeat over remote impacts, count suppressed cues, and never let audio
throttling affect visuals, telemetry, or gameplay. No sound is authoritative and no client audio
state is replicated.

### Diagnostics and performance

Add bounded presentation diagnostics for map recipe/snapshot byte size, resolved counts, asset load
failure IDs, map reconstruction generation/count, live visual entity count, live one-shot count, and
suppressed audio cues. These are local evidence and never drive server rules.

The server adds no steady-state map system. Re-run M05's fixed-step performance cases inside the new
arena and retain the existing p95 `< 16.67 ms` target on the recorded machine. Resolver/instantiation
bench evidence records counts and duration but does not create a premature production loading budget.
Client visual checks record frame profile and entity count; optimization such as a tilemap draw call is
adopted only if the simple bounded sprite approach is measured inadequate on target hardware.

## Trackable implementation plan

Implement as four green vertical slices: (1) pure content/model/resolver, (2) authoritative greybox
migration plus startup failure behavior, (3) protocol/late-join/reconnect and maximum-snapshot
transport evidence, and (4) windowed assets/map presentation/HUD/audio. Each slice ends with its
affected role-specific tests and M05 regression subset. Do not begin asset acquisition or broaden the
client shell while the authoritative map and network recovery slice is red.

### Prerequisite and content foundation

- [ ] Wait for Milestone 05 to reach its roadmap-required user-playtest and feedback gates. Then
  re-run its accepted format, role-specific Clippy/tests/builds, 38-case network suite, 12-profile
  impairment/process matrix, and performance baseline against the exact M06 starting commit; retain
  its still-required windowed/controller evidence rather than treating automated success as playtest.
- [ ] Add map IDs, definitions, embedded RON catalog/preset, code-owned limits, policy validation,
  normalization, canonical fingerprint, resolver, round-trip helpers, snapshot-size checks, and a
  legal non-preset fixture.
- [ ] Add sandbox/base layout requirements including the typed practice-dummy anchor; implement
  bounds, counts, reference, shape, spawn-safety, reachability, and layout-compatibility validation.
- [ ] Add the authoritative map module/plugin and migrate `GreyboxArenaDefinition`, movement bounds,
  combat lob bounds, arena-wall spawning, stable spawn lookup, reset spawn assignment, and tests to
  resolved map resources without weakening M03–M05 behavior.

### Protocol and authoritative runtime

- [ ] Extend the shared content fingerprint, register map root/identity/snapshot and spawn assignment,
  bump protocol identifiers, and preserve registry equality in every supported role.
- [ ] Spawn the server map root plus synthesized perimeter and recipe geometry colliders before
  endpoint bind; keep spawn/region data in immutable indexes unless a current ECS query requires an
  entity; add exact instance cleanup and replacement tests.
- [ ] Prove the snapshot equals public resolved data and arrives for initial, late, and reconnecting
  clients. Exercise the maximum legal serialized snapshot under typical/adverse UDP fragmentation;
  accept 64 KiB only if that evidence is reliable, otherwise reduce the bound before implementation.
- [ ] Extend real UDP/network automation to assert map identity/fingerprint, current snapshot,
  collision/layout behavior, rejection on content mismatch, and no client map-authority path.

### Assets, map presentation, HUD, and audio

- [ ] Download/inspect the approved CC0 packs; select a minimal PNG/OGG subset; add license/source
  records and `assets/manifest.ron`; do not import the Sci-Fi preview font or unused archive contents.
- [ ] Add client-only asset/audio/Vorbis features, nearest filtering, retained typed handles,
  recursive readiness/failure polling, stable ID mapping, required/optional fallbacks, and server
  feature-isolation assertions.
- [ ] Reconstruct floor, geometry, decorations, spawn/team cues, and terrain reservation from the
  replicated snapshot; implement idempotence, generation-scoped cleanup, replacement, and bounds.
- [ ] Replace fighter `PlayerId` coloring with team/owner presentation and preserve all M05 weapon,
  projectile, effect, health, selection, and aim/range feedback.
- [ ] Build the controller-first HUD/readiness/error shell and neutral gameplay input until ready;
  retain keyboard/mouse parity and pause behavior.
- [ ] Add bounded cue-to-audio mapping, coalescing, priority, one-shot cleanup, and diagnostics with
  no effect on gameplay/cue deduplication.

### Verification and handoff

- [ ] Run format, role-specific Clippy/tests/builds, feature graphs, headless server, network tests,
  performance cases, real UDP/process automation, fixed-port cleanup, and asset-manifest validation.
- [ ] Run keyboard/mouse and physical Xbox-like controller checks for every weapon on center/side
  routes, connection/readiness/error states, HUD, team recognition, audio, camera, and spawn flow.
- [ ] Capture the four required aspect ratios at 30/60/high render profiles and record entity counts,
  clipping/overlap/readability observations, and tuning changes.
- [ ] Set `User playtest` only after automated/network/visual gates pass; provide commands, controls,
  route/weapon scenarios, known limitations, and requested observations.

## Test plan

### Pure parser/resolver tests

- [ ] RON parses the exact built-in catalog/preset and round-trips `MapRecipe` equality. Reordered
  collections, whitespace/comments, equivalent rotations, and signed zero resolve/fingerprint
  identically; one semantic value change changes the fingerprint.
- [ ] Reject unsupported schema/revision, unknown IDs, duplicate catalog IDs, duplicate global
  placement IDs, invalid keys/names, non-finite values, invalid sizes/rotations, outside-bounds
  placements, over-wide policy, excessive counts, excessive tiled expansion, and recipe/snapshot
  byte overflow.
- [ ] Reject missing/invalid camera bounds, geometry crossing invalid exterior, unsupported shapes or
  collision/profile combinations, arbitrary path/URL-like data where only stable IDs are allowed,
  and inert region/entity parameters outside their catalog schema.
- [ ] Reject missing/extra team slots, unsafe/blocked/too-close/wrong-facing spawns, overlapping team
  areas, missing egress, disconnected center reachability, and missing/duplicate/wrong-shape sandbox
  practice anchors.
- [ ] The built-in preset and legal non-preset fixture produce sorted bounded snapshots through the
  same resolver; no resolver/instantiator/presentation system branches on `MapPresetId(1)`.
- [ ] Built-in geometry, spawn points, regions, and visual placements are symmetric under x/y mirror
  and 180-degree rotation; every spawn reaches the center clearance grid.

### Small-App/ECS and schedule tests

- [ ] `AuthoritativeMapPlugin` creates exactly one map root, four perimeter colliders, every resolved
  permanent collider, `PlayableBounds`, and immutable spawn/region lookup before the
  first fixed tick; repeat initialization does not duplicate them.
- [ ] Collider shape/pose/layers equal the resolved primitive, and movement/projectile/melee/area/
  lob terrain queries keep the M03–M05 wall semantics in the new layout.
- [ ] Deterministic team spawn selection uses only stable resolved points; reset retains its assigned
  pose; disconnect does not remove map members; exact instance teardown removes no unrelated entity.
- [ ] Startup ordering resolves/instantiates before endpoint spawn; invalid embedded content cannot
  bind or spawn a fighter in the focused failure composition.
- [ ] Client asset polling distinguishes loading/ready/failure, retains handles, lists exact failed
  stable IDs, and requires no renderer/audio in headless test composition.
- [ ] Snapshot reconciliation is idempotent; changed instance cleans the old generation before
  publishing new bounds/readiness; removal/disconnect cleans local presentation; missing required
  presentation mapping with no fallback fails visibly with neutral gameplay input. Optional audio
  or decoration failure yields a visible degraded state without closing the playable gate.
- [ ] Camera clamp and HUD anchors pass all four window sizes, including a viewport wider than one
  map axis. Team/owner visuals derive from replicated `TeamId`/`Controlled`, not `PlayerId`.
- [ ] Audio mapping consumes deduplicated cues, coalesces one scatter/melee attack correctly, obeys
  priority/live caps, despawns one-shots, and never changes combat components or telemetry.

### Deterministic network tests

- [ ] Client/server protocol fingerprints agree with map components; map content mismatch rejects
  through the existing join outcome before fighter spawn and cleans the link.
- [ ] Two clients receive the same `MapInstanceId`, source identity, recipe fingerprint, bounds,
  geometry, regions, spawns, anchors, and presentation IDs while only the server owns colliders and
  authoritative spawn/region indexes.
- [ ] A client cannot send or insert a recipe/snapshot/map edit that affects the server, select a
  different preset, alter bounds/collision/spawns, or use presentation data as gameplay state.
- [ ] Initial join, late join, reconnect, and a synthetic map-root replacement converge without a
  historical map event stream, duplicate visuals, stale bounds, orphan colliders, or local Bevy
  entity identity comparisons.
- [ ] Two fighters move/collide and all four weapons resolve terrain/area/melee/launcher behavior
  identically under local, typical, and adverse impairment profiles in the resolved arena.
- [ ] Replacing fighter/map PNG references in the client presentation catalog changes no server
  snapshot, collider, position, health, damage, or recipe fingerprint; changing a shared stable
  presentation ID changes content compatibility as specified.

### Process, feature, performance, and visual verification

- [ ] Isolated `server` features contain no `bevy_asset`, renderer, window, sprite, text, UI, audio,
  Vorbis, device input, PNG handles, or runtime `assets/` dependency; client/test feature unification
  is not accepted as server evidence.
- [ ] The dedicated server starts with assets absent, resolves/instantiates the map, accepts two
  clients, and shuts down/cleans fixed ports predictably.
- [ ] Real windowed clients with assets present and one deliberately broken required visual mapping
  with no fallback show ready and explicit fatal paths respectively; optional audio/decor failure
  shows degraded-but-playable presentation and never produces invisible active gameplay.
- [ ] M05 fixed-step worst cases retain p95 `< 16.67 ms`; server map code adds no steady-state system
  or connection-proportional arena entities.
- [ ] Record client visual entity count and 30/60/high render profiles at 1280×720, 1440×900,
  1024×768, and 960×540; no critical HUD/control/map information clips or overlaps.
- [ ] Physical controller and keyboard/mouse both navigate selection/pause/help, retain aim previews,
  fire every weapon, and understand health/ammo/reload/effect/team/incoming-damage cues.
- [ ] Audio checks identify fire, impact/hit, defeat/reset, reload, and connection outcomes without
  scatter spam masking local damage/defeat; running without an audio output device fails gracefully.

### Evidence rules

- Map authority assertions inspect the server `ResolvedMap`, static colliders, bounds, and stable
  placement IDs; client sprites/screenshots never prove collision authority.
- Network assertions compare stable map/recipe/placement/spawn IDs and canonical snapshot data,
  never local Bevy entity IDs or asset handles.
- The non-preset fixture must enter through the public pure resolver and normal instantiation path;
  directly constructing `ResolvedMap` does not prove recipe independence.
- Spawn safety uses pure geometry plus the deterministic clearance grid; visual inspection alone is
  insufficient, and the grid does not replace playtest fairness evidence.
- Asset licensing evidence comes from source pages/license text and the checked manifest, not a file
  name or assumption that every catalog item shares one license.
- Visual/controller/audio checks complement automated authority, lifecycle, and failure tests; they
  cannot replace them.
- Extend M05's existing `server/verification.rs`, combat evidence checkpoints, client automation exit
  gate, and `tests/network/` harness. Add map checkpoints to that single process contract; do not add
  an independent completion timer or a second process harness. A successful process run requires
  both the existing exact combat evidence and the expected map instance/fingerprint/readiness facts.

## Visual and user smoke-test plan

The playtest handoff will provide one supervised server/two-client command and explicit weapon demos.
Requested scenario:

1. Start while disconnected, connect, pass handshake, receive the map, select a weapon, and identify
   each readiness state. Repeat once with a deliberately missing required client asset and verify the
   visible non-playable error.
2. Traverse every boundary, center entry, north/south route, spawn shield, cover edge, chokepoint,
   and the marked inert destruction reservation. Report ambiguous walkable/collidable art.
3. Compare team/owner/neutral-dummy recognition at rest, movement, overlap, hit flash, defeat, and
   reset without relying only on hue.
4. Exercise pulse from spawn-to-center and across side lanes; scatter/blade while closing through
   cover; launcher over center-entry/side cover and near bounds. Report dominant sightlines, useless
   routes, unreadable landing areas, or cover that invalidates a weapon's preferred distance.
5. Have both clients exit spawn through different routes, then pressure the other spawn from center
   and both side lanes. Report unavoidable sightlines, single-exit traps, or immediate re-defeats.
6. Verify health, weapon, ammo/charges, reload/recharge, aim preview, incoming damage, connection/map
   state, and the reserved match HUD slot at all four window sizes.
7. Compare fire/impact/hit/defeat/reset/reload sounds during simultaneous scatter and launcher
   attacks. Report masking, repetition, excessive volume, latency, or ambiguous ownership.
8. Disconnect/reconnect one client during projectiles/effects and confirm the current arena returns
   once with no duplicate visuals, stale camera bounds, orphan effects, or changed collision.

Known limitations must state: no formal match/score/respawn/restart; no map selection/editor/custom
maps; no active objective or terrain destruction; the marked terrain region is inert; no hot reload,
runtime asset distribution, music, production art/audio, spatial audio, animation set, camera shake,
vibration, concealment, hazards, pickups, or advanced environment behavior; owner/projectile
prediction and lag compensation remain governed by the open earlier decision.

## Risks and follow-up decisions

- **M05 boundary regression:** M05's remediated module split, combat schedule, content validation,
  process evidence, session lifecycle, and shared network harness are the starting contract. M06 must
  extend those seams rather than recombining them into map/client/server god modules or independent
  readiness/evidence clocks.
- **Resolved snapshot growth:** future user maps may exceed the provisional 64 KiB component bound.
  M06 proves the bounded v1 representation; chunked map transfer/distribution is a future network
  design, not a reason to add it now.
- **Simple sprite rendering:** up to 1024 resolved visual instances are bounded but may cost more
  draw calls than a tilemap/batched mesh. Measure before adopting a 0.19-specific batching API.
- **Asset-pack coherence:** Shape Characters and Sci-Fi Facility differ stylistically. Team markers,
  consistent scale, and primitive fallbacks protect readability; playtest may choose primitives or a
  different approved CC0 subset without changing simulation.
- **Audio device/codec variance:** Vorbis and an absent output device need target-macOS checks. A
  failure must remain presentation-only and visible; it cannot stop the server or corrupt gameplay.
- **Mode schema evolution:** M07 introduces Wipeout's first concrete mode requirements through the
  generic seam. It must revalidate this preset and update schema/fingerprint versions instead of
  silently changing resolver meaning.
- **Terrain reservation semantics:** M10 may need a different chunk size/material representation.
  The M06 region supplies stable placement/bounds/presentation only and deliberately promises no
  mask/collider format.

## Exit checklist

- [x] Post-M05 coherence and architecture research questions are resolved or explicitly deferred.
- [ ] Technical specification is validated by the user.
- [ ] Milestone 05's accepted baseline is green before production implementation begins.
- [ ] Catalog, recipe, preset, resolved snapshot, runtime state, and mode requirements remain typed,
  separate, bounded, and free of arbitrary code/assets/components.
- [ ] Built-in and non-preset recipes use the same canonical resolver and instantiation path with no
  preset-ID behavior branch.
- [ ] Validation rejects every bounds/count/reference/spawn/mode/snapshot invariant and code-owned
  ceilings cannot be widened by authored policy.
- [ ] The server owns one map root, immutable resolved map, colliders, bounds, spawn/region indexes,
  and cleanup; clients own reconstructed presentation and input intent and create no map colliders.
- [ ] Initial/late/reconnecting clients converge on the same bounded map identity/snapshot, and map
  replacement removes stale network/local state.
- [ ] The dedicated server loads no client asset/audio data and passes isolated feature evidence.
- [ ] A legal recipe data change can alter layout, presentation IDs, geometry, regions, entities,
  spawns, or anchors without a new map/mode system.
- [ ] The arena visibly supports all four weapon ranges, center/side choices, clear collision, and
  at least two practical exits from each spawn without obvious spawn trapping.
- [ ] Team, owner, walkable space, cover, health, weapon economy, aim/range, incoming damage,
  connection/readiness/error state, and inert destruction reservation are readable.
- [ ] Imported visual/audio assets have exact provenance; missing critical visuals fail visibly or
  use a declared fallback; optional audio/decor failures degrade visibly; replacing presentation
  changes no simulation outcome.
- [ ] HUD/camera/audio pass required aspect-ratio, keyboard/mouse, controller, two-client, and
  simultaneous-combat checks.
- [ ] M03–M05 authority, collision, combat, impairment, lifecycle, performance, and cleanup contracts
  remain proven in the resolved arena.
- [ ] Editor, persistence, publishing, asset upload/distribution, executable map logic, formal match
  rules, objective behavior, and destructible-terrain behavior remain deferred.
- [ ] User feedback is triaged, affected verification reruns, learn-from-errors is recorded, and
  roadmap/current milestone status is updated before completion.
