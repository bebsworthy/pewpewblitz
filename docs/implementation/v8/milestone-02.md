# V8 Milestone 02 — Surface, vegetation, adjacency, and replacement proof

## Status

`Complete`

Research and planning started on 2026-08-23 after the user accepted M01, including its whole-cell
destruction granularity. The user accepted the decision to keep concealing bush behavior for a
future dedicated milestone and authorized M02 implementation on 2026-08-23.

## Player/developer-visible outcome

M02 adds one small original map, **Tidal Garden**, as a dedicated Wipeout 2v2 game type. It proves
that the V8 grammar can express the supplied map references without tracing or importing them:

- sand as the default walkable surface;
- irregular water cells that block players and deployable placement but let projectiles pass;
- walkable, projectile-pass-through `TALL_GRASS` that occupies the feature slot but does not claim
  concealment;
- adjacency-derived water shores, vegetation edges, and wall joins;
- ordinary blocking walls and corners;
- a rotatable two-cell destructible barrier that atomically becomes nonblocking rubble;
- inert decorations and eight explicit spawn markers;
- restart, late join, reconnect, recovery, replacement, and primitive fallback through the same
  V8 dynamic-state owner introduced in M01.

Tidal Garden is deliberately a proof map, not another conversion. Crossroads remains unchanged;
M03 still owns conversion of Crossroads Hot Zone and Ashen Court.

## Research sources

### Supplied visual references

The five ignored drawings under `external_assets/map_images/` were inspected directly:

| Reference | Grammar demonstrated | M02 use |
|---|---|---|
| `acid_lakes.webp` | irregular water, vegetation borders, wall islands, default floor, decorations | primary grammar reference, not copied |
| `core_of_orbit.webp` | sparse wall bends, grass/wall adjacency, small water pockets | supports bounded adjacency masks |
| `dark_passage.webp` | long shaped water boundary, alternate vegetation palette, open lanes | supports projectile-pass-through water and readable shore edges |
| `double_trouble.webp` | symmetric competitive lanes, small water/grass clusters, corners | informs original left/right proof-map symmetry |
| `hyacinth_house.webp` | compact vegetation corridors and explicit objective landmarks | confirms that objectives are mode-owned; M02 chooses Wipeout and adds no fake generic marker |

The drawings remain design references only. No pixels, coordinates, names, layouts, or art are
shipped.

### Local engine/network sources

- `Cargo.toml` pins Bevy `0.19.1`, Lightyear `0.29.0`, and Avian 2D `0.7.0`.
- `references/bevy/examples/3d/transparency_3d.rs` demonstrates current `StandardMaterial` alpha
  modes. M02 may use opaque/masked vegetation, but does not use transparency as gameplay hiding.
- `references/bevy/examples/math/custom_primitives.rs` and
  `references/bevy/examples/3d/generate_custom_mesh.rs` demonstrate retained `Mesh` assets,
  explicit positions/normals/UVs/indices, and ordinary `Mesh3d` ownership.
- `references/bevy/examples/2d/tilemap_chunk.rs` demonstrates bounded chunk ownership and
  deterministic source data. Brawler keeps its established 3D renderer and uses the ownership idea,
  not the 2D tilemap component.
- `references/lightyear/book/src/concepts/bevy_integration/system_order.md` confirms replication is
  received in `PreUpdate`, authoritative fixed work runs in the fixed schedule, and sends flush in
  `PostUpdate`.
- `references/lightyear/book/src/concepts/reliability/channels.md` confirms the existing ordered
  reliable map channel remains the right carrier for dynamic transitions.
- The exact pinned implementation at
  `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lightyear_replication-0.29.0/src/visibility/immediate.rs`
  provides per-link `gain_visibility` and `lose_visibility`; losing visibility despawns the remote
  entity, while retained modes intentionally preserve stale state and are unsuitable for secret
  occupants.
- Lightyear's official 0.29 replication documentation describes the same per-client visibility
  surface: <https://docs.rs/lightyear_replication/0.29.0/lightyear_replication/>.
- Bevy's official pinned material documentation confirms `StandardMaterial` owns `alpha_mode`, but
  visual alpha has no authority or privacy semantics:
  <https://docs.rs/bevy/0.19.1/bevy/prelude/struct.StandardMaterial.html>.

### Current Brawler seams

- `src/map/grid.rs` currently resolves only one effective default surface, one-cell assets, and a
  removed-placement dynamic state. M02 must generalize these directly rather than add a second
  resolver.
- `src/map/grid_server.rs` already owns atomic fixed-post destruction, per-placement colliders,
  ordered events, reset, and recovery. M02 evolves its event/state vocabulary from removal-only to
  explicit terminal transitions.
- `src/client/presentation_3d/mod.rs` currently creates one full-map ground plane, static V8
  placement visuals, and per-cell live cover visuals. M02 must keep generated meshes bounded and
  rebuild only the accepted map/dynamic groups that changed.
- `src/movement/arena.rs` currently has collision layers that treat every terrain-shaped collider
  as both player and projectile blocking. Water requires one map-owned player/deployable-only
  membership layer; merely changing Avian collision filters is insufficient because combat shape
  casts also filter by membership masks.
- Fighter entities, projectiles, sentries, and combat cues currently target every client. In
  particular, `src/combat/authority.rs::send_combat_cues` sends position- and identity-bearing cues
  to every link, and projectile entities replicate `ReplicatedAttackSource` to all clients.

## Research decisions

### 1. Concealment does not ship in M02

M02 adds `TALL_GRASS`, not `BUSH`, and does not add a `HideOccupants` enum value to the production
catalog. Tall grass is a walkable feature so it cannot coexist with a wall in the same cell, but its
gameplay profile is pass/pass, indestructible, non-interactive, and non-concealing.

Lightyear's per-link entity visibility is necessary but not sufficient for Brawler concealment. A
complete implementation would also need recipient-aware fighter interpolation/control, projectile
and deployable visibility, every combat cue, allied visibility, proximity reveal, attack/damage
reveal timing, defeat/reset, Hot Zone state, spectators, audio, reconnect, late join, and leakage
tests. Building that privacy subsystem inside a map-grammar proof would violate M02's smallest
vertical-slice goal and risk shipping false security.

`BUSH/HideOccupants` is deferred as `V8-CONCEALMENT`. It can be promoted only as a dedicated
specification with observer/subject policy and wire-leakage evidence. A future visual-only bush must
also use an honest non-concealing name.

The user accepted this concealment disposition on 2026-08-23.

### 2. No interaction framework or teleporter

Spawn markers already prove typed placement parameters. Teleport, chest, healing, launcher, and
hazard behavior are not needed to prove surfaces, adjacency, or replacement and remain in
`V8-INTERACTIONS`.

### 3. Wipeout proof; no generic mode anchor

Tidal Garden uses Wipeout because M02's new authority is environmental. Adding a Hot Zone anchor
would duplicate work already owned by M03 and would not strengthen the surface/replacement proof.
The map contains no mode anchor; M03 converts the real Hot Zone layouts using grid-owned typed
anchors.

### 4. Derived adjacency, not an authored autotile language

Client presentation computes a four-neighbor `N/E/S/W` mask from canonical occupied cells and a
bounded visual adjacency group. Code maps the 16 masks to isolated, end, straight, corner, tee, and
filled variants or generated edges. Recipes cannot provide adjacency variants or override masks.
Adjacency never changes collision, replacement, fingerprints, or server state.

### 5. Replacement is explicit dynamic state

M01's `removed_placements` is sufficient only while every outcome is removal. M02 replaces it with
sorted terminal placement states:

```text
MapPlacementOutcome::Removed
MapPlacementOutcome::ReplacedWith(MapAssetId)
MapPlacementTransition { placement_id, outcome }
```

The source placement remains immutable in the snapshot. The catalog determines the legal terminal
outcome; live events and recovery carry the committed outcome explicitly. This supports removal and
replacement without mutating recipes or maintaining two dynamic representations.

## Technical specification

### Catalog/schema additions

The one shared V8 catalog evolves in place and bumps its schema, recipe fingerprint format,
protocol, and gameplay-content identity. There is no schema compatibility decoder.

`MapAssetDefinition` gains:

```text
footprint_cells: (width, height)       # each axis 1..=8
```

Quarter-turn rotation swaps footprint axes. The authored cell is always the minimum occupied cell
after rotation. Every occupied cell participates in bounds, slot, effective-surface, collision,
destruction-overlap, spawn-clearance, and replacement validation.

`MapGameplayProfile` retains explicit player/projectile properties and evolves destruction to:

```text
Indestructible
OnMapDestruction(Remove)
OnMapDestruction(Replace(MapAssetId))
```

M02 does not add durability or direct weapon damage to map assets: the only implemented acceptance
fact remains the bounded authoritative `DestroyMap` world effect. A replacement must:

- exist and use the same slot;
- have the same rotated footprint;
- be legal on every effective surface under the source placement;
- be terminal and indestructible in M02;
- not require parameters;
- not introduce stronger player/projectile collision than the source;
- reference a complete normal visual profile.

The shared catalog adds exactly the assets needed by the proof:

| Asset | Slot | Footprint | Player | Projectile | Destruction/state |
|---|---|---:|---|---|---|
| `SAND_FLOOR` | Surface | 1x1 | Pass | Pass | Indestructible; default surface |
| `WATER` | Surface | 1x1 | Block | Pass | Indestructible |
| `GARDEN_WALL` | Feature | 1x1 | Block | BlockAndConsume | Indestructible |
| `TALL_GRASS` | Feature | 1x1 | Pass | Pass | Indestructible; no concealment |
| `BREAKABLE_BARRIER` | Feature | 2x1 | Block | BlockAndConsume | Replace with `RUBBLE` |
| `RUBBLE` | Feature | 2x1 | Pass | Pass | Indestructible terminal state |
| `DECOR_SEASHELL` | Decoration | 1x1 | Pass | Pass | Inert |
| existing `PLAYER_SPAWN` | Marker | 1x1 | Pass | Pass | Existing typed parameters |

Crossroads assets gain explicit `1x1` footprints with no behavioral change. `RUBBLE` is not authored
directly in recipes; validation rejects it as a source placement in M02.

### Effective surfaces and occupancy

Resolution becomes a deterministic two-pass transaction:

1. Start every cell with the default surface, then apply at most one explicit `Surface` placement
   per cell and derive its surface tag and collision profile.
2. Resolve feature, decoration, and marker footprints against those effective surfaces and the
   existing slot rules.

`WATER + GARDEN_WALL`, `WATER + TALL_GRASS`, `WATER + BREAKABLE_BARRIER`, decorations on water, and
spawns on water are rejected. `SAND_FLOOR + TALL_GRASS` and `SAND_FLOOR + wall/barrier` are valid.
Filled rectangles remain source convenience and expand before both passes.

The resolver derives:

- canonical effective-surface cells and four-neighbor adjacency masks;
- merged player/deployable-only water collider rectangles;
- merged indestructible wall collider rectangles where profiles and adjacency allow;
- one collider descriptor per live destructible multi-cell placement;
- rotated occupied-cell sets and AABBs for multi-cell assets;
- spawn clearance and navigation reachability using fighter radius, not one-cell BFS alone;
- dynamic source/outcome indexes and worst-case event/recovery sizes.

### Tidal Garden proof recipe

Tidal Garden uses preset ID `4`, a new recipe/key `tidal-garden`, a `40 x 28` grid, and a new
presentation theme. It is horizontally mirrored for left/right teams but not rotationally copied
from any supplied drawing.

The reviewed implementation target is:

- sand default surface;
- 48 explicit irregular water cells arranged as mirrored pools with no one-cell navigable choke;
- 40 tall-grass cells in several irregular islands;
- 36 one-cell wall modules forming bends, ends, straights, and corners;
- four `2x1` breakable barriers, with two authored at quarter-turn rotation;
- six inert seashell decorations;
- eight spawn markers, four per team, with at least three legal positions per side after fighter
  clearance;
- at least three fighter-diameter routes between team sides before and after every barrier is
  destroyed;
- no mode anchor or unsupported interaction.

Exact cells are reviewed through a checked-in ASCII planning fixture or focused test table before
the RON recipe is finalized. The source of truth remains the RON recipe; no image or ASCII runtime
loader is added.

A dedicated `tidal-garden-2v2` game type exposes the proof without changing existing game-type map
pools. It selects only preset `4`, uses ordinary Wipeout rules, and is included in lobby/admission,
routed identity, automation, and playtest documentation.

### Authoritative collision and schedule ownership

M02 adds a map-owned player/deployable-only collision membership layer. Fighter movement and
deployable placement query it; projectile casts, muzzle contact, projectile line of sight, and area
delivery do not. Existing blockers continue to block both player and projectile paths.

The fixed-post order remains one visible chain:

```text
combat outcomes finalized
  -> collect accepted DestroyMap facts
  -> resolve unique source placements and legal terminal outcomes
  -> capacity-check the entire batch
  -> commit sorted transitions and dynamic revision atomically
  -> remove source colliders / insert replacement colliders
  -> repair fighters and deployables if a topology transition requires it
  -> mode rules
  -> publish ordered transition events
```

For the M02 barrier, replacement is nonblocking, so collision removal and visible replacement are
the same committed fact. Whole-placement destruction preserves the M01 readability invariant: a
destroyed multi-cell barrier cannot leave a partial blocking cell or collision speck.

Restart restores every authored source placement under a new generation. Map replacement and
teardown remove all water, feature, dynamic, adjacency, mesh, recovery, and index ownership.

### Dynamic network contract

The ordered reliable `MapDynamicChannel` remains. Its messages evolve with the global protocol:

```text
MapMutationEvent {
  generation,
  revision,
  transitions: Vec<MapPlacementTransition>,
}

MapDynamicState {
  map_instance_id,
  generation,
  revision,
  terminal_states: Vec<MapPlacementTransition>,
}
```

Transitions and recovery states are sorted by placement ID and contain no duplicate placement.
Clients derive visuals and collision-independent presentation from the shared catalog only after
validating the transition matches the immutable source placement's legal outcome. Unknown,
contradictory, stale, duplicate, or out-of-order transitions trigger the existing gap-recovery path
or fail readiness; clients never guess.

Snapshot/bootstrap, late join, reset, reconnect, and recovery keep M01's generation/revision
semantics. The server is the only producer of transitions and recovery snapshots.

### Client presentation

The client-only visual catalog gains bounded fitting and adjacency data rather than asset-specific
branches in the map recipe:

```text
fitting: Exact | Tiled | Contained
adjacency_group: None | Water | Vegetation | Wall
kind: Imported | GeneratedSurface | GeneratedVegetation | GeneratedFeature | HiddenMarker
```

The client builds bounded generated groups:

- one sand ground mesh/plane for the default surface;
- one generated water mesh group containing cell tops and exposed shore edges from water adjacency;
- one generated tall-grass mesh group with deterministic per-cell tufts and exposed-edge shaping;
- wall modules whose orientation/join presentation derives from the 4-bit wall mask;
- one dynamic visual per breakable barrier source or rubble replacement;
- inert decoration scenes or exact primitive fallbacks;
- the existing generated perimeter.

Generated geometry is deterministic from recipe fingerprint, placement/cell, and adjacency mask.
It does not use runtime randomness. Water remains visually low and readable; tall grass cannot fully
occlude fighters or imply concealment. Primitive fallback preserves water boundaries, wall blocking,
barrier/rubble state, and grass identity.

Static map materialization may rebuild once when an accepted immutable snapshot changes. Dynamic
replacement reconciliation updates only transition-owned entities and removes old `Mesh` assets
when necessary. Repeated restart/reconnect/replacement must keep entity and mesh counts bounded.

### Module and ownership plan

M02 keeps `src/map/` as the owner and splits only where M01 now has demonstrated independent work:

```text
src/map/grid.rs                 shared IDs, recipe/catalog models and compatibility projection
src/map/grid_resolution.rs      effective surfaces, footprints, occupancy, adjacency, indexes
src/map/grid_server.rs          install, dynamic transaction, reset/recovery publication
src/map/grid_collision.rs       typed collider descriptors, merging, spawn/navigation queries
src/map/client.rs               snapshot/dynamic convergence and readiness
src/client/presentation_3d/
  grid_map.rs                   V8 static/dynamic generated/imported materialization
  environment_assets.rs        client visual/theme parsing and retained handles
```

The exact extraction is approved only if it leaves fixed ordering visible at the plugin composition
point and does not move client catalog code into shared/server modules. It is not a requirement to
create one file per type.

## Alternatives considered

- **Ship hiding as client opacity:** rejected; it leaks authoritative state and is cosmetic only.
- **Hide only fighter entities with Lightyear visibility:** rejected for M02; universal projectile,
  deployable, cue, audio, spectator, and reconnect paths still disclose hidden subjects.
- **Add a generic property map:** rejected; bounded enums keep implemented behavior auditable.
- **Use water as a blocking feature over sand:** rejected; water is an effective surface and must
  exercise the surface slot/default override contract.
- **Keep removal-only state and infer every replacement client-side:** rejected; recovery must name
  the committed terminal outcome and validate it against the catalog.
- **Author corner/shore variants in the recipe:** rejected; presentation adjacency is derived and
  cannot affect authority.
- **Use the supplied bitmap as a tile source:** rejected; drawings are references, not production
  geometry or content.
- **Add teleports or Hot Zone now:** rejected; neither is needed for this environmental slice.

## Implementation checklist

- [x] Bump V8 catalog/recipe/fingerprint/protocol/content identities with no compatibility decoder.
- [x] Add bounded footprints, rotated occupied cells, effective surfaces, replacement outcomes, and
  complete catalog contradiction/reference validation.
- [x] Re-resolve Crossroads through explicit `1x1` footprints with byte-for-byte equivalent
  gameplay bounds, spawns, colliders, destruction, and presentation.
- [x] Add sand, water, garden wall, tall grass, barrier, rubble, seashell, and their exact client
  visual/theme profiles; add only any genuinely used asset provenance entries.
- [x] Author and validate the original `40 x 28` Tidal Garden recipe and dedicated 2v2 game type.
- [x] Add effective-surface and multi-cell occupancy indexes, collider merging, fighter-radius
  navigation validation, and adjacency-mask helpers.
- [x] Add the player/deployable-only map collision layer and verify every movement, placement,
  projectile, muzzle, line-of-sight, lob, melee, and area-delivery query.
- [x] Generalize dynamic state/events/recovery from removals to explicit terminal transitions.
- [x] Commit replacement collider/state changes atomically; reset, recover, reconnect, replace, and
  teardown without stale state.
- [x] Keep the cohesive V8 presenter in its existing owner and add bounded water,
  vegetation, wall-adjacency, barrier/rubble, decoration, and primitive-fallback presentation.
- [x] Update lobby/admission/routing/automation/diagnostics/current docs and add exact payload,
  collider, entity, mesh, fixed-tick, readiness, restart, and reconnect evidence.
- [x] Add `V8-CONCEALMENT` to the version backlog and ensure no production `BUSH`,
  `HideOccupants`, or client-only hiding claim enters M02.

## Verification plan

### Pure/catalog tests

- default and overridden effective surfaces, surface conflicts, legal/illegal feature combinations;
- `1x1`, `2x1`, rotated `1x2`, bounds, occupied-cell ordering, slot conflicts, and replacement
  footprint compatibility;
- all 16 adjacency masks, mirror symmetry, source-order-independent fingerprints, and no authored
  variant override;
- exact Tidal Garden counts, water/grass irregularity, spawn capacity, fighter-radius clearance,
  three route requirement, and before/after-destruction reachability;
- unknown replacement, chain/cycle, stronger replacement collision, authored terminal asset,
  unsupported concealment, excessive footprint/bytes/colliders/transitions, and schema rejection;
- Crossroads canonical behavior remains unchanged except expected global identity bumps.

### ECS/authority tests

- water blocks fighter movement and deployable placement while straight/lobbed projectiles and
  projectile line-of-sight pass through;
- walls and live barriers block players/projectiles; grass and rubble block neither;
- radius-48 destruction touching either occupied barrier cell commits exactly one sorted
  replacement transition and removes the entire blocker;
- duplicate facts do nothing; mixed removal/replacement batches are atomic; over-capacity batches
  never partially commit;
- collider changes precede repair/mode rules; restart restores source barriers and clears terminal
  state under a new generation;
- teardown/replacement leaves no surface, collider, transition, outbox, cache, or index owner.

### Network/routed tests

- snapshot plus transition bootstrap converges before readiness;
- ordered transitions, duplicates, gaps, reset, recovery, stale generation, late join, reconnect,
  and map replacement converge to exact barrier/rubble state;
- forged client transitions/recovery responses fail and cannot mutate authority;
- dedicated Tidal Garden 2v2 reaches active play, destroys a barrier, restarts, reconnects,
  completes, returns, and requeues with matching process identity;
- no `BUSH`, concealment component, hidden-fighter state, or recipient-private cue is registered by
  M02; tall grass remains honestly non-concealing;
- server feature isolation remains clean.

### Capacity/native evidence

Run the canonical repository commands:

```text
just check
just lint
just test
just e2e 2
just e2e 4
just e2e 6
BRAWLER_RENDER_GAME_TYPE=tidal-garden-2v2 BRAWLER_RENDER_PLAYERS_PER_TEAM=2 \
  just v3-render-evidence target/v8-m02-tidal-garden-render.txt
```

Record recipe/snapshot/event/recovery bytes; effective surface, placement, occupied-cell, collider,
entity, mesh, and material counts; fixed-post p95; load-to-readiness; and repeated
restart/reconnect/replacement stability. M02 must stay inside existing fixed-tick and native render
thresholds. New ceilings are set from the implemented proof plus measured margin and enforced by
tests, not waived as future optimization.

## Implementation and verification evidence

Implementation completed on 2026-08-23 and moved through verification. The final slice includes
the shared schema/catalog evolution, Tidal Garden content and routed game type, player-only water
collision, atomic barrier-to-rubble transitions, recovery/reset/late-join convergence, derived
adjacency presentation, and non-concealing tall grass.

The canonical checks passed:

- `just check`;
- `just lint`;
- the complete client suite: 432 passed;
- the complete server suite: 340 passed;
- the complete Balance Lab suite: 350 passed;
- the serial network suite: 84 passed;
- all 14 fixed-tick/performance gates;
- routed `just e2e 2`, `just e2e 4`, and `just e2e 6`;
- an explicit routed `tidal-garden-2v2` 2v2 roster reached authoritative `Active`;
- Tidal Garden barrier replacement converged for a connected and late-joining separate-App client.

Tidal Garden's locked wire measurements are recipe `10,707` bytes, resolved snapshot `1,014`
bytes, maximum four-barrier transition event `20` bytes, and full terminal-state recovery `20`
bytes. All remain far below the recipe, snapshot, event, and recovery ceilings.

Native release evidence passed for both presentation paths:

- imported/generated: p95 `17.276 ms`, 932 entity high-water, 354 mesh-entity high-water, 52 mesh
  assets, and 59 material assets;
- forced primitive fallback: p95 `17.068 ms`, 851 entity high-water, 338 mesh-entity high-water,
  52 mesh assets, and 59 material assets.

The evidence reports are `target/v8-m02-tidal-garden-final.txt` and
`target/v8-m02-tidal-garden-primitive.txt`, with passing peer reports alongside them. The imported
run emitted non-fatal duplicate-despawn warnings when auxiliary headless clients exited; both
locked reports passed and terminal ownership counts remained bounded.

Verification exposed and fixed two automation/readiness defects: renderer-neutral map readiness
had only been installed with windowed presentation, and render-measurement clients with empty
profiles did not create the automation brawler. Map readiness now belongs to client networking,
the headless playable gate evaluates current map/terrain/join state instead of relying on an edge,
and bounded render measurement uses the same default-profile automation as headless evidence.

## Native playtest handoff

The user tests Tidal Garden with imported/generated and forced-primitive presentation and reports:

1. whether water is immediately readable as non-walkable while shots visibly cross it;
2. whether tall grass reads as walkable vegetation without falsely promising concealment;
3. whether wall joins, corners, shore edges, and vegetation boundaries remain legible;
4. whether destroying either half of a barrier visibly replaces the whole blocker with rubble and
   leaves no collision speck;
5. whether routes, spawn framing, symmetry, and combat flow feel intentional rather than copied;
6. whether restart, reconnect, and requeue leave any stale water, barrier, rubble, grass, or meshes;
7. whether imported/generated and primitive modes convey the same gameplay.

Feedback that adds concealment, changes the accepted 32-unit grid/destruction contract, or adds an
interaction returns M02 to specification review before production scope changes.

## Exit criteria

- the user accepts this specification, including non-concealing `TALL_GRASS` and deferred
  `V8-CONCEALMENT`, before implementation starts;
- Tidal Garden proves default/override surfaces, irregular fields, feature exclusivity,
  player-only water blocking, adjacency, multi-cell placement, replacement, decorations, and
  spawns through one V8 resolver/runtime/presenter;
- Crossroads retains its accepted gameplay and whole-cell destruction readability;
- dynamic transitions, restart, recovery, late join, reconnect, replacement, and teardown pass;
- no bitmap importer, compatibility decoder, generic property map, authored autotile language,
  fake concealment, teleporter, or generic interaction framework is introduced;
- automated/native evidence passes and the user accepts the proof map;
- feedback is triaged, affected checks rerun, and the learn-from-errors review is complete before
  M02 becomes `Complete`.

## Feedback review

Completed on 2026-08-23. The user accepted Tidal Garden and authorized M02 closeout without a
follow-up gameplay or presentation change. The accepted decisions remain:

- coarse whole-cell and whole-placement destruction stays as implemented because it is readable
  and cannot leave tiny collision specks;
- `TALL_GRASS` remains walkable and honestly non-concealing;
- concealing bush behavior remains deferred to `V8-CONCEALMENT` and is not pulled into M03;
- the original Tidal Garden proof, water collision split, adjacency presentation, and atomic rubble
  replacement are accepted as the grammar baseline for the remaining conversions.

## Learn-from-errors review

Completed on 2026-08-23.

1. **Renderer-neutral readiness was composed under presentation.** Headless routed V8 clients could
   receive the map but never check in because `MapPresentationPlugin` was installed only by the
   windowed presentation composition. Cause: the plugin's name reflected its original visual use,
   while its actual responsibility had grown to canonical snapshot validation and readiness.
   Prevention: install renderer-neutral acceptance/reconciliation with client networking; keep
   only mesh/material realization in the windowed renderer.
2. **Readiness used an edge when the contract required current state.** Terrain could become ready
   before the replicated join became active, permanently missing the one transition that enabled
   headless play. Cause: a local `suppressed` optimization accidentally encoded ordering between
   independently replicated facts. Prevention: derive headless playability each frame from current
   join, map, and terrain readiness; do not use one-shot edges for conjunctive readiness gates.
3. **Native evidence depended on persisted profile state.** Windowed render automation did not
   create a default brawler for an empty profile, even though headless automation did. Cause: the
   automation helper was scoped by window mode rather than by bounded automation intent.
   Prevention: render measurement and headless evidence share the same deterministic default-profile
   bootstrap while ordinary interactive clients retain explicit player ownership.
4. **Roster-specific evidence used a 1v1-only auxiliary flag.** The native script's additional 2v2
   clients used requeue smoke, obscuring the real empty-profile issue and making the harness's
   requested topology unclear. Prevention: every auxiliary client receives the same exact
   roster-specific match flag and requested game type as the measured clients.
5. **Diagnostics should exist at rejection boundaries.** Snapshot validation previously failed
   silently, extending diagnosis even though it was not the final cause. Prevention: retain the
   bounded warning when an authoritative map snapshot is rejected; automation debug records remain
   bounded and opt-in through log level.

No new reusable Codex skill was created: these are project-specific composition and evidence-harness
lessons already captured in this milestone and enforced by the existing routed/native checks.
