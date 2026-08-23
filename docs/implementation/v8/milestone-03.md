# V8 Milestone 03 — Complete built-in and catalog conversion

## Status

`Complete`

Research and planning started on 2026-08-23 after the user accepted Tidal Garden and authorized M02
closeout. The user approved implementation on 2026-08-23.

## Outcome

M03 converts the two remaining legacy built-ins—Crossroads Facility Hot Zone and Ashen Court—into
the accepted V8 sparse-grid/map-asset pipeline. At the end of the milestone, every advertised game
type selects a V8 recipe, every selected environment visual resolves through the V8 client catalog,
and routing, admission, authoritative collision, Hot Zone scoring, dynamic destruction, recovery,
and native presentation use one V8 map pool.

M03 does not delete the dormant V4 implementation. M04 remains the explicit deletion owner for
old source types, loaders, generated inputs, compatibility snapshot/projection, legacy-only tests,
and the old catalog files. This boundary keeps M03 focused on proving the converted product before
removing the comparison path.

## Research questions and answers

### Can Crossroads Hot Zone be converted without changing its arena?

Yes. It has the same `56 x 36` bounds, six permanent wall rectangles, four decorations, eight
spawns, and central `6 x 6` destructible field as the accepted Crossroads Wipeout conversion. M03
reuses those exact V8 cells and adds one typed Hot Zone objective. No second Crossroads geometry
document or map-identity branch is needed.

The old objective is a circle centered at world origin with radius `160`. World origin is grid
vertex `(28, 18)` in a `56 x 36` map, and `160` is exactly five 32-unit cells. The V8 anchor can
therefore preserve its authoritative center and radius exactly.

### How should an area objective fit the map-asset model?

It should not pretend to be an environment asset. An objective spans cells, is required and
interpreted by the selected mode, and owns scoring semantics. M03 adds a small typed
`GridModeAnchorPlacement` array to the recipe. The only shipped value is a Hot Zone circle with a
stable anchor ID, a grid-vertex center, and an integer radius in cells.

This is deliberately narrower than a generic marker/property framework. `PLAYER_SPAWN` remains a
placed `MapAssetId` because it is a point-like reusable placement. The Hot Zone objective remains a
mode-owned anchor because Hot Zone validates and scores it.

### Can Ashen Court preserve its round trees and rocks on a square authoring grid?

Yes, if authoring footprint and collision shape remain distinct responsibilities. Each tree or rock
owns a `2 x 2` feature footprint centered exactly on the old position, while its gameplay profile
owns the existing circular collider with radius `28`. The footprint reserves cells and prevents a
wall or another feature from coexisting there; the profile determines how players and projectiles
actually collide. The grid does not force the circle to become a `64 x 64` square blocker.

This is an actual catalog property, not a terrain kind. M03 adds a bounded collider shape to the
gameplay profile rather than introducing identities such as `CircularObstacle`.

### What cannot remain pixel-identical after quantization?

Ashen Court's walls, tree/rock centers and radii, bounds, and spawn vertical positions convert
exactly. Its spawn x positions move 16 units inward, matching the already accepted Crossroads spawn
quantization. Each `96 x 96` destructible reservation becomes one symmetric `3 x 3` field shifted
16 units outward on each axis; the footprint size and rotational symmetry remain exact, while
destruction uses the accepted whole-cell behavior.

Inert decorations are placed at the nearest symmetric cell centers. The two coffins formerly used
45-degree angles; V8 supports the reviewed quarter-turn contract only, so they snap to a symmetric
cardinal pair. M03 does not add sub-cell offsets or eighth-turn authoring solely for decoration
parity. The native review must judge the resulting Ashen composition before closeout.

### Which old paths remain after M03?

No advertised or production-selected map uses V4 after M03. The V4 parser, compatibility runtime
snapshot, old environment catalog, and legacy test override remain physically present only so M04
can delete them after the converted suite and user playtest are green. M03 may extend the existing
M04-owned compatibility projection enough for current neutral consumers, but it must not add a
decoder, facade, alias, or new old-schema authoring path.

## Research sources

### Current Brawler content and consumers

- `content/v4/maps/builtin/crossroads-facility-hot-zone.ron` and
  `content/v4/maps/builtin/ashen-court.ron` are the conversion baselines.
- `content/v4/{map_definitions,map_objects,map_visual_variants,map_themes}.ron` and
  `assets/catalogs/environment_visuals.ron` identify the retained wall, tree, rock, gravestone,
  coffin, lantern, theme, scene, transform, tint, and fallback facts.
- `content/v8/{map_assets,map_gameplay_profiles,map_presentation_themes}.ron`,
  `content/v8/maps/**`, and `assets/catalogs/{map_asset_visuals,map_presentation_themes}.ron` are the
  one catalogs and recipe set that M03 evolves.
- `src/map/grid.rs` owns V8 coordinates, slots, profiles, canonical resolution, validation,
  fingerprints, resolved snapshots, and the temporary compatibility projection.
- `src/map/grid_server.rs` owns authoritative collider installation, fixed-post destruction,
  reset, ordered mutation publication, and recovery.
- `src/matchplay/hot_zone.rs` currently consumes exactly one
  `HOT_ZONE_OBJECTIVE_ANCHOR_DEFINITION` area and already accepts a resolved objective resource.
- `src/server/{admission.rs,lobby/catalog.rs,lobby/mod.rs}` contains the temporary V4/V8 lookup
  branches that M03 must remove from production selection.
- `config/server/game-types.ron` selects Crossroads Hot Zone for 2v2/3v3 and Ashen Court for First
  Blood. Stable preset keys and IDs remain unchanged.
- `src/client/presentation_3d/{environment_assets.rs,mod.rs,border.rs}`, `src/map/client.rs`, and
  `src/terrain/client/recovery.rs` own current client loading, presentation, readiness, and
  convergence seams.
- `tests/network/**`, `tests/performance.rs`, `scripts/e2e.sh`,
  `scripts/v3-render-evidence.sh`, and the canonical `justfile` commands provide the existing
  separate-App, routed-process, performance, imported, and primitive evidence paths.

The source audit also found old model consumers in diagnostics, movement/prediction, HUD, combat
tests, and terrain fixtures. M03 converts selected-map fixtures and metadata; M04 removes the old
types from consumers that still use the compatibility snapshot independent of authored content.

### Local engine and network references

- `references/bevy/examples/app/plugin.rs` confirms that the collider/profile additions remain in
  the existing focused map plugin composition rather than creating an application-wide framework.
- `references/lightyear/book/src/concepts/bevy_integration/system_order.md` confirms authoritative
  fixed work runs after received replication and before PostUpdate sends. M03 retains M02's
  fixed-post destruction and ordered publication boundaries.
- `references/lightyear/book/src/concepts/advanced_replication/avian.md` distinguishes canonical
  Avian `Position`/`Rotation` state from presentation `Transform`; static map collider installation
  continues to author both consistently, while presentation never feeds authority.
- `references/avian/crates/avian2d/examples/collision_layers.rs` demonstrates ordinary
  `Collider::circle`, `Collider::rectangle`, and explicit layer membership. M03 uses those existing
  primitives for profile-owned shapes and does not introduce custom collision geometry.

The pinned local sources were sufficient. No new internet research or dependency is required.

## Technical specification

### Shared gameplay profiles own bounded collider shape

`MapGameplayProfile` gains one server-known collision-shape field:

```text
MapColliderShape
  None
  FootprintRectangle
  Circle { radius_world_units: u16 }
```

Rules:

- profiles with `Pass/Pass` use `None`;
- a profile that blocks either players or projectiles must use a non-`None` shape;
- `FootprintRectangle` derives its dimensions and center from the rotated placement footprint;
- `Circle` is allowed only on a feature, has a positive bounded radius, and must fit within the
  declared footprint around its derived center;
- collision response remains separately controlled by `player_collision` and
  `projectile_collision`; shape cannot change layer behavior;
- decoration and marker assets always use a `None` collider;
- replacement validation rejects an outcome that expands collision behavior or no longer fits its
  source footprint;
- themes and visual profiles cannot override collider shape.

The existing ground, decoration, grass, rubble, and spawn profiles use `None`; wall, destructible
cover, and barrier profiles use `FootprintRectangle`; water retains its footprint rectangle with
player-block/projectile-pass layers. A new shared round-obstacle profile uses
`Circle(radius_world_units: 28)` for Ashen pine and rock assets.

Navigation and spawn-clearance validation must test the actual derived shape, not conservatively
mark every footprint cell as fully blocked. This preserves truthful resolver validation for the
round obstacles. Rectangle merging remains valid only for compatible footprint-rectangle
placements; circles remain individual static colliders.

### Typed grid-owned Hot Zone anchor

`GridMapRecipe` gains:

```text
mode_anchors: [GridModeAnchorPlacement]

GridModeAnchorPlacement {
  placement_id: MapPlacementId,
  anchor_id: ModeAnchorId,
  kind: HotZoneCircle {
    center_vertex: MapGridVertex,
    radius_cells: u16,
  },
}
```

`MapGridVertex` uses integer coordinates in `0..=width` and `0..=height`. It represents a grid-line
intersection rather than a cell center, which preserves the current origin-centered objective
without floating authoring. Radius is a positive integer count of `MAP_CELL_SIZE_WORLD` and the
resolved circle must fit inside playable bounds.

Validation is mode-specific:

- Wipeout recipes contain no mode anchors;
- Hot Zone recipes contain exactly one `HotZoneCircle` with unique nonzero placement and anchor
  IDs;
- both teams retain at least three spawn markers and every spawn remains fighter-clear;
- all spawn markers are connected to cells that overlap or border the objective's navigable area;
- the resolver derives exact world center/radius and `ResolvedObjectiveZone` from this typed value;
- the authoritative install inserts `ResolvedObjectiveZone` before match initialization, so Hot
  Zone scoring does not depend on reverse-reading a presentation object;
- the replicated V8 snapshot includes the typed anchor for client validation and presentation;
- the temporary compatibility projection emits the equivalent old neutral anchor only until M04.

There is no generic anchor definition catalog in V8 M03 because only Hot Zone area semantics have a
current production owner.

### Converted map documents

#### Crossroads Facility Hot Zone

Stable identity remains preset `MapPresetId(2)`, recipe `MapRecipeId(2)`, key
`crossroads-facility-hot-zone`, and Hot Zone mode definition `3`. Admission revision increases from
`1` to `2` because the accepted source and fingerprint change.

The document uses:

- dimensions `56 x 36`, default ground, and V8 Crossroads theme `1`;
- the exact wall rectangles, decorations, eight spawn markers, and central `6 x 6` destructible
  cover cells already exercised by Crossroads Wipeout;
- one Hot Zone anchor at grid vertex `(28, 18)` with radius `5` cells;
- no old object, region, spawn-area, floating spawn-point, or mode-anchor shapes.

This preserves the old arena topology while ensuring both Crossroads modes are visibly and
authoritatively the same map family.

#### Ashen Court

Stable identity remains preset `MapPresetId(3)`, recipe `MapRecipeId(3)`, key `ashen-court`, and
Wipeout mode definition `2`. Admission revision increases from `1` to `2`.

The document uses dimensions `48 x 32`, default ground, and new V8 Ashen theme `3`. Theme ID `2`
remains Tidal Garden; stable IDs are not silently repurposed.

Permanent gameplay placements are:

| Old placement | V8 placement | Parity |
|---|---|---|
| four `192 x 64` stone walls | four `6 x 2` filled rectangles | exact bounds |
| two `64 x 192` stone walls | two `2 x 6` filled rectangles | exact bounds |
| pines at `(-128, 0)` and `(128, 0)`, radius `28` | two `2 x 2` pine assets with circle profile | exact center/radius |
| rocks at `(0, -320)` and `(0, 320)`, radius `28` | two `2 x 2` rock assets with circle profile | exact center/radius |
| two `96 x 96` destructible reservations | two rotationally symmetric `3 x 3` cover fields | exact size; 16-unit outward quantization |
| eight spawn points at x `±640` | eight marker assets at x `±624` | 16-unit inward quantization; y exact |

The gravestone, coffin, and lantern pairs become inert V8 decoration assets at the nearest cell
centers chosen as 180-degree symmetric pairs. Coffin yaw snaps from the old 45/225-degree pair to a
cardinal pair. No gameplay collider or mode rule depends on these decorations.

Ashen Court keeps the accepted coarse rule: each explicit destructible cover cell clears as a whole
when overlapped by the authoritative destruction effect. It does not reconstruct the old hidden
8-unit terrain raster.

### Shared and client catalog conversion

The shared V8 asset catalog adds only identities used by the conversion:

- Ashen stone wall;
- Ashen pine feature;
- Ashen rock feature;
- gravestone decoration;
- coffin decoration;
- lantern decoration.

Ashen stone wall uses the ordinary blocking/indestructible rectangle profile. Pine and rock share
the round-obstacle profile but remain separate `MapAssetId`s because their author-facing meaning and
visuals differ. Decorations share the inert profile. Existing destructible cover supplies both
Ashen destructible fields; there is no `DestructibleTerrain` kind.

`assets/catalogs/map_asset_visuals.ron` promotes the active graveyard scene paths, scales, offsets,
tints, fitting, and fallbacks from the old environment catalog under stable `MapVisualProfileId`s.
The V8 theme catalogs add Ashen as theme `3`, preserving its ground, outer-world, perimeter,
ambient, directional, cover, and fallback palette. Every shared V8 visual reference must have
exactly one client profile; unused legacy visual rows are not copied.

`assets/manifest.ron` changes only if the promoted V8 catalog references a shipped file not already
covered. Catalog migration alone is not asset provenance and does not duplicate GLBs.

### Resolution, authority, and lifecycle

The resolver accepts both shipped mode definitions and dispatches only validation rules, never a
separate map parser:

```text
parse one GridMapRecipe
  -> validate common cells/assets/profiles
  -> validate Wipeout or Hot Zone layout contract
  -> derive actual collider descriptors and spawn index
  -> derive optional typed objective resource
  -> fingerprint canonical recipe
  -> build one ResolvedGridMapSnapshot
```

Authoritative installation:

- installs perimeter, merged rectangle blockers, individual circular blockers, player-only water,
  and dynamic destructible/replacement colliders from V8 profiles;
- inserts bounds, spawn catalog, and an objective resource when the typed recipe contains one;
- retains the existing destruction ordering before physics/mode rules;
- resets and recovers only V8 terminal placement states;
- tears down every static/dynamic collider and objective resource with the owning map instance;
- preserves exact deterministic ordering by placement ID and derived collider key.

Circular Ashen obstacles are indestructible, so M03 does not add dynamic circle reconstruction.
Dynamic cover remains rectangular and follows the accepted M01/M02 transaction.

### Production map-pool cutover

Once both recipes resolve:

- lobby operator-catalog resolution looks up map keys only in `GridMapContentCatalog`;
- match-worker admission validates preset, revision, mode, and recipe through only the V8 catalog;
- the default direct lobby allocation obtains both Wipeout and Hot Zone revisions from V8;
- authoritative production startup resolves every selected preset directly through V8;
- `config/server/game-types.ron` keeps the same game type and map keys;
- the preset dispatcher and lobby/admission V4 fallback branches marked `V8-MIGRATION(M03)` are
  removed;
- the test-only legacy override may remain isolated for M04-owned old-terrain tests and cannot be
  inserted by production composition;
- selected map identity, admission revision, fingerprints, readiness, diagnostics, Balance Lab,
  and routed manifests all report V8 facts.

Because `ResolvedGridMapSnapshot` and `MapGameplayProfile` change incompatibly, M03 bumps the V8
catalog/recipe/fingerprint versions, the global gameplay-content envelope, and the one global
protocol compatibility version. It does not add per-message versions or compatibility decoders.

### Presentation and readiness

The existing V8 client owner loads Ashen imported scenes and theme `3`, derives grid positions and
quarter-turn yaw, and provides generated/primitive fallbacks for every promoted asset. Imported
scene readiness remains a presentation concern; authoritative map acceptance depends only on the
validated V8 snapshot and shared catalog fingerprint.

Hot Zone objective presentation derives from the typed V8 anchor and retains the existing
mode-owned circle styling. Ashen pine/rock visuals center on their `2 x 2` footprint while collision
uses the profile-owned circle. Primitive fallback must communicate the same wall/circle/cover
topology even if it does not resemble the imported graveyard art.

Client acceptance rejects wrong schema, fingerprint, preset, mode, missing/duplicate Hot Zone
anchor, invalid circle, unknown asset/profile, or incompatible dynamic state. Late join, recovery,
restart, and requeue must not leave stale objective, Ashen visuals, or colliders.

### Explicit M03/M04 boundary

M03 removes production selection of the old content. M04 deletes it. The following remain assigned
to M04 after M03 only if no product path selects them:

- `content/v4/**` and `assets/catalogs/environment_visuals.ron`;
- the V4 generated build input and old catalog/fingerprint material;
- `MapContentCatalog`, old object/variant/profile/region/source types and resolver;
- `ResolvedMapSnapshot` and the visibly marked compatibility projection/wire component;
- legacy terrain runtime/recovery and explicitly named legacy-only fixtures;
- old environment loader/presenter code made unreachable by V8 selected maps.

M03 must update the migration inventory so every surviving seam has exactly one M04 marker. A new
M03 implementation may not introduce another old-system reference.

## ECS and schedule ownership

- `MapContentPlugin` continues to install immutable shared V8 catalog state in every role.
- `AuthoritativeMapPlugin` resolves and installs the selected V8 map at startup.
- `GridMapRoot`, `ResolvedGridMapSnapshot`, and `GridMapDynamicState` remain the replicated map
  identity/state owner.
- `ResolvedObjectiveZone` is a server resource derived during map install and read by Hot Zone mode
  initialization; it is removed during teardown.
- static rectangle/circle and dynamic collider entities carry `MapInstanceMember` for exact
  teardown; their Avian `Position`/`Rotation` is authority state and `Transform` is presentation
  synchronization only.
- `GridMapRuntimeSet::ApplyDestruction -> Publish` remains in `FixedPostUpdate`, after accepted
  combat outcomes and before mode rules/physics observation as already composed.
- client snapshot acceptance/recovery remains renderer-neutral; mesh/scene realization remains in
  the windowed presentation plugin.

No new global plugin, command bus, generic map interaction framework, or alternate runtime map is
introduced.

## Implementation plan

### Schema, resolver, and authority

- [x] Add bounded collider shape to shared gameplay profiles and catalog validation.
- [x] Derive rectangle/circle collider descriptors, actual-shape navigation, and spawn-clearance
  checks.
- [x] Add typed Hot Zone circle anchors to recipe, canonical ordering, fingerprinting, snapshot,
  and layout validation.
- [x] Derive/install/teardown `ResolvedObjectiveZone` directly from V8 resolution.
- [x] Install merged static rectangles and individual circular blockers with correct player and
  projectile layers.
- [x] Extend only the existing M04-owned compatibility projection needed by neutral consumers.

### Content and presentation

- [x] Add Crossroads Facility Hot Zone as V8 preset `2`, admission revision `2`.
- [x] Add Ashen Court as V8 preset `3`, admission revision `2`, with reviewed quantization.
- [x] Add the six retained Ashen map assets, round-obstacle gameplay profile, visual profiles, and
  theme `3`.
- [x] Render typed Hot Zone objective and Ashen rectangle/circle/decorative placements in imported
  and primitive paths.
- [x] Verify the asset manifest needs no path/provenance change, or update it only for a real new
  shipped reference.

### Product cutover and consumers

- [x] Switch lobby catalog, direct allocation, match admission, and production authoritative map
  startup to V8-only preset lookup.
- [x] Remove every `V8-MIGRATION(M03)` dispatcher/fallback and retain only enumerated M04 seams.
- [x] Update selected-map diagnostics, client readiness, Balance Lab, performance fixtures,
  integration harnesses, and native automation for presets `2` and `3`.
- [x] Bump the V8 schema/recipe/fingerprint versions, gameplay-content envelope, protocol
  compatibility version, and locked admission expectations together.
- [x] Update current README/spec/art/map documentation where it still describes the selected V4
  content path; historical milestone documents remain unchanged.

## Verification plan

### Pure/catalog tests

- parse and resolve all four V8 presets in stable order with unique keys, IDs, recipe IDs, and
  admission revisions;
- reject unknown/duplicate/mismatched profiles, illegal collider shapes, zero/oversized circles,
  circle/footprint disagreement, and colliding decoration/marker profiles;
- reject Wipeout anchors and missing, duplicate, out-of-bounds, or wrong-shaped Hot Zone anchors;
- prove Hot Zone center `(28,18)`/radius `5` resolves exactly to `(0,0)`/`160`;
- prove Ashen wall rectangles, circle centers/radii, symmetric `3 x 3` fields, spawns, and
  decoration pairs resolve to the reviewed coordinates;
- prove canonical reorderings keep fingerprints stable while gameplay/profile/anchor changes alter
  them.

### ECS/authority tests

- static circle and rectangle collider counts, shapes, positions, layers, and teardown are exact;
- fighter and projectile contacts distinguish walls, round obstacles, water, cover, and open
  footprint corners correctly;
- Ashen cover destruction clears whole cells, repairs collision, resets, and recovers without old
  terrain state;
- Hot Zone initializes only from the typed V8 objective and produces correct empty/contested/team
  occupancy, progress, result, restart, and deadline behavior;
- every 1v1/2v2/3v3 spawn is safe and all teams can navigate to the objective or opposing side;
- install/restart/late join/reconnect/requeue cycles leave one map root, one objective where
  required, bounded colliders, and no stale resources.

### Network/product tests

- separate-App replication/recovery for Crossroads Hot Zone and Ashen Court;
- lobby operator catalog advertises presets `1..=4` from V8 only and rejects old/mismatched
  revisions;
- match-worker admission accepts revised presets `2` and `3` and rejects wrong mode/revision;
- routed First Blood 1v1 on Ashen Court;
- routed Hot Zone 2v2 and 3v3 on Crossroads Hot Zone, including scoring, completion, and fresh-lobby
  requeue;
- canonical `just check`, `just lint`, `just test`, and `just e2e 2/4/6` pass;
- server-only checks prove collider/catalog additions introduce no render, window, audio, input, or
  client-asset dependency.

### Capacity/performance evidence

- lock recipe/snapshot byte sizes after all four presets load;
- retain dynamic event/recovery ceilings from M02 and measure Ashen's worst-case `3 x 3` field
  transition;
- assert static collider, map entity, mesh/material, and memory high-water marks for the largest
  converted map;
- repeated Hot Zone/Ashen lifecycle cycles remain bounded and do not grow caches or queues.

### Native review

Run imported and forced-primitive evidence for:

1. Crossroads Hot Zone, confirming objective readability, shared Crossroads geometry, cover
   destruction, and 2v2/3v3 routes;
2. Ashen Court, confirming stone-wall identity, round pine/rock collision readability, symmetric
   cover fields, quantified decoration snapping, spawn framing, and First Blood flow.

The user is specifically asked whether Ashen's 16-unit cover/spawn quantization and cardinal coffin
pair preserve the map's identity. Rejection returns those authored choices to specification review;
it does not silently add floating offsets or a second rotation grammar.

## Implementation and verification evidence

Implementation and automated verification completed on 2026-08-23. Production selection,
operator-catalog lookup, admission, authoritative startup, replicated V8 snapshots, and client
visual/theme resolution now use the V8 catalog for all four advertised presets. The six remaining
coexistence seams are dormant and assigned only to M04 deletion; no `V8-MIGRATION(M03)` marker
remains.

The canonical gates passed:

- `just check` and `just lint`, including server feature isolation;
- 436 client tests, 348 server tests, 357 Balance Lab tests plus its separate-App catalog test;
- 85 serial separate-App network tests, including converted Hot Zone/Ashen replication and Ashen
  late join;
- all 14 fixed-tick/performance gates;
- routed `just e2e 2`, `just e2e 4`, and `just e2e 6`;
- explicit routed First Blood 1v1 on Ashen Court and Hot Zone 2v2/3v3 on Crossroads Hot Zone.

Converted wire measurements are:

| Preset | Recipe | Snapshot | Maximum event | Full recovery |
|---|---:|---:|---:|---:|
| Crossroads Hot Zone | 3,430 bytes | 940 bytes | 84 bytes | 84 bytes |
| Ashen Court | 4,194 bytes | 728 bytes | 58 bytes | 58 bytes |

Native release evidence passed for both presentation paths:

| Map/path | Frame p95 | Entity high-water | Mesh-entity high-water | Mesh/material assets |
|---|---:|---:|---:|---:|
| Ashen imported | 17.006 ms | 1,022 | 248 | 52 / 74 |
| Ashen primitive | 17.005 ms | 731 | 238 | 52 / 68 |
| Hot Zone imported | 17.062 ms | 1,188 | 309 | 52 / 72 |
| Hot Zone primitive | 17.080 ms | 805 | 292 | 52 / 68 |

Reports are under `target/v8-m03-{ashen,hot-zone}-{imported,primitive*}.txt`, with peer reports.
The first Ashen primitive attempt connected and was admitted but hit a transient transport timeout
before report creation; its logs are retained, and the fresh-port retry passed. Hot Zone emitted
the already-known non-fatal duplicate-despawn warning when auxiliary clients exited; both locked
reports passed with bounded terminal ownership.

Verification also exposed a stale test harness dependency: after production grid presentation
stopped borrowing legacy theme profiles, the renderer test harness still installed only the legacy
material catalog. The harness now composes V8 themes exactly like the real client, and the affected
Tidal Garden presentation/replacement test and full canonical suite pass.

## Native playtest handoff

Run `just run 2`, select **First Blood**, and inspect Ashen Court. Then run `just run 4`, select
**Hot Zone 2v2**, and inspect Crossroads Hot Zone. For fallback comparison, launch the same command
with `BRAWLER_FORCE_PRIMITIVE_WORLD=1`.

Please check:

1. whether Ashen's stone walls, round pine/rock obstacles, and two coarse `3 x 3` cover fields are
   readable and preserve the map's identity;
2. whether the 16-unit inward spawn shift and outward cover shift feel acceptable;
3. whether the cardinal coffin pair is visually acceptable;
4. whether the Crossroads objective circle is clear and the arena matches Crossroads Wipeout;
5. whether imported and primitive paths communicate the same collision and destruction topology.

## Exit criteria

- the user approves this specification before implementation;
- all advertised map keys resolve from the one V8 catalog and no production selector falls back to
  V4;
- Crossroads Hot Zone preserves exact arena/objective topology and Ashen preserves reviewed
  gameplay/presentation parity;
- Wipeout and Hot Zone 1v1/2v2/3v3 authority, scoring, results, recovery, reconnect, and requeue
  pass;
- imported and primitive native evidence passes and the user accepts both converted maps;
- every retained old-system seam is dormant, enumerated, and assigned to M04 deletion;
- feedback is triaged, affected verification rerun, and the learn-from-errors review is complete
  before M03 becomes `Complete`.

## Feedback review

The user accepted the M03 handoff on 2026-08-23 by directing work to proceed to the cleanup
milestone. No map, collision, quantization, objective, imported-asset, or primitive-fallback change
was requested. The accepted coarse whole-cell destruction rule remains the V8 contract, and
concealing bushes remain deferred to `V8-CONCEALMENT` rather than being pulled into cleanup.

Disposition:

- Ashen Court conversion and its 16-unit spawn/cover quantization: accepted as implemented;
- Crossroads Hot Zone conversion and objective presentation: accepted as implemented;
- imported and forced-primitive presentation parity: accepted as implemented;
- no M03 feedback item requires a code change or affected verification rerun;
- legacy deletion proceeds in M04 against the already-green converted product.

## Learn-from-errors review

1. The first production visual cutover still selected the legacy theme catalog before reaching the
   grid branch. The cause was that the branch point sat too late in one large materialization
   system. It was corrected before canonical verification. M04 will make the V8 snapshot the sole
   input and delete the alternate branch so a dormant catalog can no longer gate current maps.
2. The renderer test harness installed only legacy theme resources after production stopped doing
   so. `just test` exposed the mismatch. The fix made the harness compose the same V8 visual/theme
   dependencies as the real client. Reusable lesson: test helpers must install production-owned
   dependencies, not a convenient historical subset; M04 replacement tests will use one canonical
   V8 fixture/composition helper.
3. The first Ashen forced-primitive native run connected and was admitted but ended in a transient
   transport timeout before writing reports. A fresh-port retry passed. Reusable lesson: retain the
   failed logs, distinguish transport setup failure from render evidence, and require a clean retry
   rather than treating a missing report as visual failure.
4. The bounded M01-M03 coexistence markers made the remaining six seams auditable, but several
   neutral consumers still depend on the compatibility projection. M04 therefore cuts consumers
   over before deleting sources and finishes with an independent zero-match audit; deleting data
   files first would only hide architectural dependencies.
