# Milestone 10 — Quantized destructible terrain

## Tracking

| Field | Value |
|---|---|
| Version | v1 — gameplay MVP |
| Roadmap | [roadmap.md](./roadmap.md) |
| Status | Complete (2026-08-17) — all three implementation-review rounds and the user-playtest round triaged and remediated; learn-from-errors review done; reserved follow-ups live in `GAP-DESIGN-TERRAIN-RESERVATION` and the v1 backlog |
| Research | Complete for specification review; product and network contracts, the live M09 codebase, pinned Bevy 0.19.1/Lightyear 0.29/Avian 0.7 sources and examples, installed exact-version crate sources, and current primary exact-version documentation inspected through 2026-08-16 |
| Review findings | Checked against the live source and pinned dependencies on 2026-08-16 (round 1), 2026-08-17 (round 2), and 2026-08-17 (round 3); dispositions recorded below |
| Specification validation | User validated by requesting implementation of this specification on 2026-08-16 |
| Implementation | Complete from starting commit `7a7a2baa0dc6aaa8bcacf61155e4d75727f397db`, delivered as `5cb219f` and remediated from the 2026-08-16 and 2026-08-17 implementation reviews as follow-up fix commits; all six slices delivered with per-slice evidence below |
| Verification | Complete 2026-08-16 and re-run after all three review remediations: canonical gate, both-mode terrain process profiles under local/typical/adverse conditions, and windowed visual capture green (see Slice 6 evidence); pre-existing `network-combat-profiles` failure recorded in the backlog |

At M10's start, Milestone 09 was complete while Milestones 01–03, 05, and 08 retained recorded user
or hardware gates. M10 therefore built on the accepted M09 codebase without rewriting those
historical statuses. M11 later reconciled and closed those owning milestones from the final basic
v1 acceptance while explicitly deferring release-quality polish.

### Slice 0 evidence (2026-08-16)

- Starting commit `7a7a2baa0dc6aaa8bcacf61155e4d75727f397db` with uncommitted M09-closeout/M10-spec
  documentation changes preserved in the worktree.
- No new overlapping M01–M03/M05/M08 feedback arrived since the M09 closeout; nothing to reconcile.
- Full automated baseline rerun and recorded:
  `just fmt-check` green; `just clippy-client` green after fixing one pre-existing Clippy 1.95
  `float_cmp` drift in a `src/client/presentation.rs` test assertion; `just clippy-server` green;
  `just server-features` green; `just check` green for client/server/network-test;
  `just test-client` 132 passed; `just test-server` 119 passed; `just test-network` 68 passed;
  `just test-performance` 10 passed (100-fighter/200-projectile p95 ≈ 1.94 ms);
  `just network-smoke` green after clearing one stale server process that held the UDP port;
  `git diff --check` clean.
- Schedule-trace, protocol/content fingerprint, and `ResolvedMatchCapacity` expectations are added
  with the terrain slices below rather than as a separate pre-commit, because they can only assert
  behavior that does not exist yet; each slice's tests lock the ordering/registry invariants as
  they are introduced.

## Outcome

Brawler gains server-authoritative, monotonically destructible terrain whose gameplay solidity is an
8-world-unit occupancy grid. The grid is divided into sparse 32×32-cell chunks covering 256×256
world units. Map recipes continue to describe immutable destructible regions; the terrain subsystem
derives initial occupied cells, owns mutable match state, rebuilds only affected Avian voxel
colliders, publishes compact ordered brush events, and supplies bounded full-state chunk recovery.

The implementation supports every legal playable-map size, currently 1024–4096 units wide and
720–3072 units high. The existing 192×192 Crossroads region remains a playtest scenario, not a
capacity target. Minimum/maximum map sizes, arbitrary legal world offsets, near-maximum terrain,
multiple regions, chunk seams, the 24-active-fighter terrain capacity profile, late recovery, and
restart reset all receive explicit automated or process evidence.

No visible map tile becomes authoritative. Permanent geometry remains unchanged. Clients keep only
presentation/recovery occupancy and never generate gameplay collision. V1 destruction changes solid
cells to empty cells only; terrain construction, repair, materials, collapse, fluids, persistence,
and smooth sub-cell carving remain deferred.

## Decisions requiring specification validation

1. Use fixed 8-unit square cells. This gives a standard 48-unit-diameter fighter six cells across
   and the existing 8–12-unit projectile diameters one to one-and-a-half cells while retaining a
   compact upper bound.
2. Use globally aligned 32×32-cell chunks (256×256 world units), allocated only where an authored
   destructible region initially occupies at least one cell. A legal map can intersect at most 221
   such chunks under the current size and coordinate limits; actual occupancy is bounded to 196,608
   cells (24 KiB of raw bits).
3. Use one `[u64; 16]` occupancy value per chunk. Keep initial and current occupancy distinct so
   restart is exact and does not reread content or replay history.
4. Use Avian 0.7's existing `Collider::voxels`/Parry voxel shape for server collision. Rebuild every
   occupancy-changed chunk, and rebuild an allocated orthogonal neighbor only when a changed boundary
   cell can alter cross-chunk voxel topology. Do not add a terrain, polygon, or bit-vector dependency.
5. Use a global world grid with Euclidean floor division for negative coordinates. Quantize brush
   centers to 4-unit half-cell coordinates and brush radii to 4-unit increments; network peers never
   rerasterize unconstrained floating-point circles.
6. Add a world-effect list to `WeaponRecipe`, separate from target-recipient payload effects. The Arc
   Launcher gets one `DestroyTerrain { radius: 48 }` effect per landed delivery. World effects are
   emitted once per delivery, never once per fighter target.
7. Apply occupancy changes and replace colliders after the current fixed-post damage transaction.
   The changed collider is authoritative for subsequent queries; Avian refreshes its AABB in the
   next physics prepare phase. Destruction caused by an impact does not retroactively alter hits or
   line of sight already resolved in that tick.
8. Use an ordered-reliable bidirectional terrain channel. Live server-to-client events carry a
   generation-tagged integer brush, revision, and at most four affected chunk IDs. Recovery sends a
   bounded full set of current allocated chunk bitsets, not a texture and not the complete map.
9. Gate client playability on a matching map snapshot and terrain recovery snapshot. Buffer bounded
   live events while recovery is outstanding; discard stale generations, ignore duplicates, and
   request recovery on a revision gap.
10. Reset terrain inside the existing match restart transaction by adding a common environment-reset
    phase between mode reset and commit. Reset does not reinstall the immutable map.
11. Keep the current central 192×192 region solid in both built-in presets for the first playtest.
    In Hot Zone it remains independent of the objective and intentionally makes the initial center
    constrained until players open it. Automated layout checks must prove reachable legal fighter
    positions remain; the playtest may move or resize it as tuning feedback without changing the
    terrain architecture.
12. Treat the numeric budgets below as v1 engine ceilings. A recipe that cannot produce terrain
    within them is rejected during map resolution rather than partially instantiated.
13. Do not derive terrain concurrency from the temporary M07 two-team/two-player validation guard.
    The selected game-mode and map profiles jointly determine team topology and participant capacity;
    M10 terrain consumes their resolved maximum-active-fighter count and supports up to 24 active
    fighters, independently of how they are divided among teams.

## Product and scope boundaries

### In scope

- one engine-owned quantized terrain format and deterministic world/cell/chunk coordinate helpers;
- rectangular and circular authored destructible reservations, including rotated rectangles, turned
  into initial occupancy by cell-center containment;
- exact validation against playable bounds, permanent geometry, duplicate occupied cells, spawn
  safety, aggregate cells/chunks, and recovery size;
- sparse authoritative chunk entities with stable wire IDs and process-local entity indexing;
- monotonic circular erasure from Arc Launcher world effects;
- Avian voxel collider creation, changed-collider replacement, cross-chunk neighbor state, and empty
  chunk collider removal;
- server/client revisions, ordered live events, bounded recovery requests/snapshots, duplicate/gap/
  stale-generation handling, reconnect/late-state convergence, and restart reset;
- one nearest-filtered 32×32 RGBA image and sprite per allocated client chunk, dirty texture updates,
  readable exposed-edge coloring, and bounded cosmetic debris derived from terrain events;
- terrain readiness in the existing client playable gate;
- pure, App/World, network, UDP-process, performance, visual, controller, and playtest evidence;
- telemetry for requests, applied/no-op/deferred brushes, cells erased, dirty/collision/visual chunks,
  recovery, serialized sizes, rebuild time, and bounded-record drops;
- verification of every remaining Gameplay MVP acceptance criterion at the end of M10.

### Out of scope

- continuous-resolution masks, marching squares, polygon simplification, arbitrary smooth cutouts,
  signed-distance fields, GPU-authored gameplay masks, or one physics entity per cell;
- solid-cell creation, terrain healing, construction, moving terrain, structural support/collapse,
  falling gameplay debris, fluids, material layers, hardness, health, elemental reactions, or fire;
- persistent terrain saves, cross-match deformation, replays, spectator catch-up, internet-scale
  compression/delta snapshots, interest management, or CDN/map distribution;
- client collision authority, terrain prediction, rollback, lag compensation, or client-authored
  brush/occupancy/revision messages;
- player-facing terrain/map editing, arbitrary initial bitmap upload, procedural terrain, or new
  user-exposed weapon-builder controls for terrain permissions;
- changing permanent walls, objectives, pickups, hazards, props, fighter bodies, or map visuals into
  terrain cells;
- new production art/audio assets, advanced crater animation, or unrestricted particle debris.

## Current architecture findings

The existing map architecture is the correct immutable boundary. `MapRecipe` and
`ResolvedMapSnapshot` already carry playable bounds, stable region IDs/profiles, region shapes, map
identity, fingerprints, and a one-time replicated snapshot. `ResolvedMap` indexes regions for the
authoritative role. `AuthoritativeMapPlugin` installs permanent Avian geometry separately, and the
client reconstructs presentation without loading gameplay state onto the server.

The existing `RegionProfileId(1)` reservation is currently inert. M10 activates that profile as an
initial-terrain declaration but does not put mutable bits or revisions into `ResolvedMapSnapshot`.
That snapshot is immutable and `replicate_once`; live terrain has a different match/recovery
lifecycle and remains separate.

Movement, straight/lobbed/melee combat, dashes, sentries, placement, and line-of-sight queries already
include `DESTRUCTIBLE_TERRAIN_LAYER`. The missing owner is the collider producer. This makes the
combat/movement impact deliberately small once chunk colliders use the reserved layer.

The current combat recipe has only target-recipient `Damage`, `Knockback`, and `Slow`. Terrain is not
a recipient and must not be applied once per `PendingPayload`. A delivery-level world-effect fact is
the clean seam: combat resolves geometry and emits a fact once; terrain owns whether cells change.

The current client playable gate combines accepted join, immutable map readiness, and asset
readiness. Terrain recovery must become a fourth observation. The client must never enter gameplay
showing initial terrain while the server owns a later crater state.

## Research questions and conclusions

### Is a smooth mask or contour pipeline justified for v1?

No. Brawler needs readable tactical openings, deterministic authority, bounded recovery, and support
for varied map sizes. It does not currently need Worms-like sub-pixel silhouettes. A fixed occupancy
grid makes the gameplay resolution explicit, bounds storage independently of the demonstration map,
and turns recovery into compact bitsets. Smooth mask carving remains an upgrade path only if the
playtest rejects the chosen cell resolution.

### What cell and chunk size fits current gameplay?

Use 8-unit cells and 32×32-cell chunks. The 24-unit fighter radius becomes three cells; the complete
fighter diameter spans six. Pulse and Scatter projectile diameters are 12 and 8 units, so terrain
edges remain no coarser than one projectile diameter. A 48-unit Arc Launcher brush has a six-cell
radius and opens a passage materially wider than a fighter after one or two edge hits.

A chunk covers 256×256 units and stores 1,024 bits (128 bytes). The maximum 4096×3072 map spans
16×12 chunks when aligned and at most 17×13 = 221 when arbitrarily offset against the global grid.
The full aligned map contains 512×384 = 196,608 cells, or 24 KiB of raw solidity. Chunk metadata,
physics shapes, and client images cost more, but all have fixed count ceilings and are measured.

### Should collision use rectangles, contours, or Avian voxels?

Use Avian voxels. Pinned Avian 0.7 exposes `Collider::voxels(voxel_size, grid_coordinates)`, backed by
the already-enabled Parry 2D feature. Parry's voxel shape tracks neighbor topology specifically to
avoid internal-edge snagging and uses an internal BVH. It directly represents the selected gameplay
grid and removes a custom polygon/rectangle decomposition algorithm.

Brawler chunks remain necessary for dirty work and recovery. Separate voxel shapes need neighbor
topology across chunk boundaries. Build each changed chunk from its current occupied cells, combine
its Parry voxel neighborhood state with each orthogonal neighbor using the relative 32-cell origin
shift, and rebuild direct neighbors when a boundary cell changes. Seam movement and projectile tests
are mandatory. If exact-version evidence exposes an Avian integration defect, the approved fallback
is a deterministic row-run compound rectangle collider behind the same occupancy API—not smooth
contours and not a new framework.

### Can changed colliders be replaced safely with the current schedule?

Yes, with explicit ordering and tests. Avian 0.7's default physics schedule runs in
`FixedPostUpdate` through `PhysicsSystems::{Prepare, StepSimulation, Writeback}`. Its collider-tree
update checks the `Collider` change tick and recomputes AABBs. Terrain brushes arise from delivery
outcomes after the current physics step and damage resolution, then replace only changed static
colliders. The new shape is used on subsequent gameplay queries; the old AABB is conservative because
v1 only removes cells, and Avian refreshes it at the next prepare phase.

Empty chunks retain their stable terrain entity/state but remove `Collider`, `RigidBody`, and
collision-layer components through an explicit deferred flush. Reoccupying chunks is out of scope,
so no collider grows from empty during a match. Restart reconstructs the initial collider before the
new match becomes active.

### How should initial shapes become cells?

Use one global grid anchored at world origin. Cell `(x, y)` covers
`[x*8, (x+1)*8] × [y*8, (y+1)*8]` and has center `((x+0.5)*8, (y+0.5)*8)`. Euclidean floor division,
not truncation, maps negative coordinates. Rectangles use inverse rotation and inclusive point-in-
shape at the cell center; circles use squared-distance containment.

The resolver then validates every selected cell AABB against playable bounds and permanent geometry.
Two destructible reservations may not select the same cell. A non-empty authored reservation that
selects no cell is invalid. Spawn reachability treats initial occupied cells as blocking; destruction
only increases reachability. Objective regions may overlap destructible terrain because they are
independent rules, but the mode's required legal fighter positions and paths must still validate.

### How are brush centers deterministic without losing too much placement detail?

Quantize authoritative impact positions to half-cell units (4 world units) using a single checked
rounding helper. Store the center as signed `i16` half-cell coordinates and the radius as `u16`
half-cell units. An occupied cell center is an odd half-cell coordinate. Erase it when the integer
squared distance to the brush center is at most the squared radius. Current world bounds need only
approximately ±1,024 half-cell units, well inside `i16`.

Authored terrain brush radii must be finite multiples of 4 from 8 through 64 units. Arc Launcher uses
48. The maximum brush diameter is smaller than one 256-unit chunk, so one brush affects at most four
Brawler chunks; collider neighbor refresh may rebuild up to twelve.

The 48-unit crater radius is intentionally smaller than Arc Launcher's existing 150-unit area-payload
radius. Damage, knockback, slow, blast visuals, and the existing landed-impact audio communicate the
full combat blast; terrain erasure communicates a tighter high-energy core so one shot does not remove
an entire 300-unit-wide area of cover. The crater edge/burst must make that smaller boundary readable,
and the playtest explicitly decides whether the relationship feels intentional or misleading.

### How does terrain capacity relate to teams and participants?

Terrain does not own or assume team topology. The selected game-mode rules define legal team-count
and participants-per-team ranges; the selected map defines compatible team slots, spawn capacity, and
layout support. Their validated composition produces a resolved maximum-active-fighter count that the
terrain subsystem consumes only for admission and performance validation.

The existing `MatchLifecycleRules::validate` restriction to two teams and at most two participants per
team is a real M07 implementation limit, but it is not a product or terrain-engine ceiling. The M10
terrain contract supports up to 24 simultaneously active fighters so it covers the planned large-group
arrangements, including `1v1 × 12`, `2v5 × 2`, and `3v3 × 3`, without encoding any of those arrangements
inside terrain. Generalizing Wipeout/Hot Zone scoring, roster arrays, HUDs, and admission beyond their
current profiles is separate work; synthetic terrain authority/performance fixtures still prove the
24-fighter destruction bound in M10.

### Where does terrain destruction enter combat composition?

Add `world_effects: Vec<WorldEffectDefinition>` to `WeaponRecipe`, with a policy/engine ceiling of
one world effect per delivery in v1. `WorldEffectDefinition::DestroyTerrain { radius }` is valid only
for single-fire lobbed delivery during this milestone. All existing presets carry an explicit empty
list except Arc Launcher. The weapon schema and affected fingerprints/revisions are bumped.

When a delivery outcome is transactionally committed, combat emits one bounded
`CombatWorldEffectFact` per authored world effect, keyed by attack/delivery/effect order. It does not
inspect terrain or send network messages. The terrain authority consumes the sorted facts once after
`CombatSet::Damage`, clips brushes to allocated occupied cells, and increments revision only when at
least one cell changes. No-op brushes remain telemetry facts but create no terrain network event.

### What is the recovery contract?

Use a dedicated `TerrainChannel` configured `OrderedReliable` and bidirectional. Lightyear 0.29
supports ordered reliable channels and packet fragmentation for messages above roughly 1,200 bytes.
Live events are still kept below 96 serialized bytes. A recovery snapshot may be fragmented but is
bounded to 48 KiB and rate-limited.

The client derives the expected stable chunk-ID set from the matching map snapshot, then sends one
`TerrainRecoveryRequest`. The server replies on the same channel with a full current bitset for every
allocated chunk, sorted by stable ID, plus map instance, match ID, terrain-format fingerprint, and
global revision. The client validates exact chunk-set equality, uniqueness, bits, byte size, and
generation before committing the snapshot.

Live events arriving while recovery is pending are buffered to a maximum of 64. After snapshot
revision `R`, buffered events at or below `R` are discarded and contiguous later events apply.
Duplicate revisions are ignored. A revision greater than `local + 1`, buffer overflow, invalid chunk
ID, or generation mismatch clears unsafe pending state and requests recovery. Stale map/match events
are discarded. One accepted client may request at most one recovery response per 30 simulation
ticks; invalid or excessive requests are counted and ignored.

### What happens on match restart?

Terrain is match-scoped runtime state even though the immutable map remains installed. Extend
`MatchRestartSet` to the chain `Prepare -> ModeReset -> EnvironmentReset -> Commit`. Terrain reset
copies initial bits to current bits, rebuilds changed colliders, sets revision zero for the allocated
next match ID, clears queues/telemetry epoch state, and stages one reset event/snapshot marker. Common
commit then publishes the new match ID. No downstream system can observe a new match with old terrain.

Clients accept the terrain reset only with the matching new `MatchState.match_id`; other arrival
orders show `syncing terrain`. A reconnect or late accepted connection always uses recovery and never
requires the reset event or historical brushes.

### Can destruction embed a fighter?

Valid v1 destruction cannot create a new overlap because it only changes solid cells to empty.
Initial map validation prevents spawn overlap, and exact voxel rebuilding does not expand occupied
cells. The normal path therefore asserts that no previously valid fighter is moved by a brush.

Retain a defensive server-only repair for malformed/recovered/injected states to satisfy the original
unstuck contract. It checks a fighter circle against authoritative occupancy using pure circle/AABB
math, then searches candidate cell centers by `(squared distance, y, x)` within playable bounds,
rejecting occupied terrain and permanent geometry. If none exists, it uses that fighter's stable
spawn state. Record every invocation; any invocation in a valid automated or playtest scenario is a
failure requiring root-cause correction, not expected gameplay.

## Research log

| Date | Source | Finding | Decision |
|---|---|---|---|
| 2026-08-16 | `docs/{00-product-direction,03-weapons-and-abilities,04-maps-and-game-modes,05-gameplay-mvp,08-network-architecture,09-environment-and-tile-ideas}.md` and [roadmap](./roadmap.md) | Terrain must remain server-owned, separate from presentation/permanent geometry, recoverable without event history, and bounded across all map sizes. The earlier smooth-mask requirement was deliberately replaced with quantized gameplay solidity. | Preserve authority/recovery contracts; specify a sparse occupancy grid and full-range fixtures. |
| 2026-08-16 | Live `src/{map,gameplay,movement,combat,abilities,matchplay,client,server,protocol}/`, `content/v1/{maps,weapons}.ron`, `tests/network/`, and `tests/performance.rs` | Stable map regions, exact map generations, reserved destructible collision layer, fixed-post outcome transaction, delivery facts, restart transaction, client readiness, bounded harnesses, and performance gates already exist. Mutable terrain, world effects, terrain recovery, and live visuals do not. | Add one focused terrain subsystem and extend existing seams; do not replace map/combat/match owners. |
| 2026-08-16 | `references/avian/src/{schedule,collider_tree,collision/collider}/`, installed `avian2d-0.7.0`, and [Avian 0.7 collider documentation](https://docs.rs/avian2d/0.7.0/avian2d/collision/collider/struct.Collider.html) | Exact Avian exposes voxel and compound colliders; changed collider shapes are noticed by collider-tree preparation. | Select `Collider::voxels`; rebuild after damage and verify next-query behavior. |
| 2026-08-16 | Installed `parry2d-0.27.0/src/shape/voxels/` and [Parry voxel source](https://docs.rs/parry2d/0.27.0/src/parry2d/shape/voxels/voxels.rs.html) | Voxel shapes keep neighbor topology to avoid internal edges, use an internal BVH, and can combine neighborhood state across distinct voxel shapes. | Reconcile orthogonal chunk neighbors and make seam tests an exit gate. |
| 2026-08-16 | `references/bevy/examples/2d/cpu_draw.rs`, installed `bevy_image-0.19.1`, and [Bevy 0.19 Image documentation](https://docs.rs/bevy/0.19.1/bevy/image/struct.Image.html) | CPU-owned images support `new_fill`, per-pixel mutation, asset change detection/upload, and nearest sampling. | Use one 32×32 RGBA image per allocated client chunk; upload whole tiny dirty images. |
| 2026-08-16 | `references/lightyear/book/src/concepts/{reliability/channels,transport/packet}.md`, examples, installed `lightyear-0.29.0`, and [Lightyear 0.29 source](https://github.com/cBournhonesque/lightyear/tree/0.29.0) | Registered typed messages use directed ordered-reliable channels; packet fragmentation supports messages above roughly 1,200 bytes. | Use compact live events and bounded fragmented full recovery snapshots on one terrain channel. |
| 2026-08-16 | Existing `EngineMapLimits`, built-in recipes, and global-grid arithmetic | The map can be arbitrarily offset; aligned dimensions undercount chunk intersections. A maximum map can intersect 17×13 global 256-unit chunks even though its aligned span is 16×12. | Set the hard chunk ceiling to 221 and test negative/off-grid extrema. |
| 2026-08-16 | `src/{config,matchplay,map,server}/`, M07, Gameplay MVP, and specification-review feedback | Current production match validation is 2v2 even though the product describes 3v3 and planned large-group arrangements. The current guard is implementation history, not a valid terrain capacity assumption. | Consume resolved map/mode capacity, support 24 active fighters in terrain, and keep broader match-mode generalization outside terrain ownership. |

## Specification review findings

| Finding | Verification | Disposition |
|---|---|---|
| Neighbor rebuild wording conflicts | Valid. The detailed collider rule was conditional while Decision 4 sounded unconditional. | Decision 4 now uses the conditional boundary-change rule. |
| `known_revision` lacks a full-snapshot purpose | Valid. No delta or not-modified response is specified. | Remove it from the request and validation contract. |
| Convergence diagram omits recovery/error transitions | Valid. The prose implied transitions that the diagram did not show. | Add `Ready -> AwaitingRecovery` and generation-scoped `Invalid` recovery semantics. |
| Radius-48 crater differs from radius-150 payload | Valid observation, not a rules defect. | State the intentional high-energy-core relationship and require distinct crater feedback/playtest judgment. |
| Four-player bound follows the current `<= 2` per-team guard | The source claim is true; using it as the terrain ceiling is invalid. It also exposes a pre-existing product/implementation tension. | Derive capacity from map/mode composition and verify terrain at 24 active fighters; do not expand multi-team mode rules inside terrain. |
| Destruction audio is unspecified | Valid. Existing `LobLanded` already produces a bounded impact sound. | Reuse that cue; add no new M10 audio asset or authoritative event. |
| `combine_voxel_states` mutates both inputs | Valid in pinned Parry 0.27. | Construct fresh prospective voxel values, reconcile them pairwise, then wrap each in a fresh Avian collider before atomic ECS replacement. |
| `AGENTS.md` source tree is stale | Valid repository-hygiene issue. | Refresh it to the current implemented tree; keep planned `src/terrain/` in this specification until it exists. |

## Technical specification

### Application and module composition

Keep one package and the existing role features. Add one focused top-level runtime concern while the
map module retains initial authored/resolved ownership:

```text
content/v1/
  maps.ron                  active destructible profile; bumped recipe revisions
  weapons.ron               world-effect policy; Arc Launcher 48-unit brush
src/map/
  definitions/
    terrain.rs              initial grid rasterization and recipe-budget validation
  model.rs                  unchanged region placement wire shapes
  server.rs                 immutable map install/teardown; terrain generation handoff
  client.rs                 immutable map reconstruction; remove inert reservation overlay
src/terrain/
  mod.rs                    shared sets/plugins and intentional public API
  model.rs                  stable grid/chunk/brush/wire/readiness shapes and limits
  grid.rs                   pure coordinate, bitset, raster, erase, overlap, fingerprint logic
  collider.rs               server-only Parry voxel construction and neighbor reconciliation
  authority.rs              server state, world-fact consumption, reset, defensive repair
  network.rs                server recovery validation/outbox and shared convergence helpers
  client.rs                 client occupancy, recovery state, images, sprites, debris
  telemetry.rs              bounded records, aggregates, snapshot/process summaries
  tests.rs                  pure and focused App/World tests
src/combat/
  definitions/              world-effect definition/policy/validation/fingerprint
  model.rs                  bounded delivery-level `CombatWorldEffectFact`
  effects.rs                transactional fact emission; no terrain mutation
src/matchplay/
  model.rs                  resolved map/mode roster-capacity contract
  mod.rs                    add common environment-reset restart phase
src/protocol.rs             register terrain messages/channel; bump protocol identifiers
tests/network/terrain.rs    authority, event convergence, recovery, restart, forgery
tests/performance.rs        scale, dirty rebuild, recovery serialization/apply benchmarks
```

Recommended plugin ownership:

| Plugin | Installed in | Responsibility |
|---|---|---|
| existing `MapContentPlugin` | client/server/tests | Parse recipes, raster-validate initial terrain, enforce aggregate limits, and fingerprint content |
| `TerrainCorePlugin` | client/server/tests | Initialize shared terrain resources and pure convergence state; no role mutation or wire registration |
| `AuthoritativeTerrainPlugin` | server/tests | Reconcile exact map generation, own occupancy/revision/colliders, consume facts, reset, repair, recovery, and telemetry |
| `ClientTerrainPlugin` | client/tests | Derive expected chunks, request/apply recovery, process live events, maintain readiness and optional visuals |
| existing `ProtocolPlugin` | client/server/tests | Register all concrete terrain messages and the terrain channel |
| existing `ServerCombatPlugin` | server/tests | Emit delivery-level world-effect facts after transactional delivery commit |

The dedicated-server feature graph must not gain `bevy_image`, sprite, mesh, render, asset, window,
audio, or client terrain presentation dependencies. `src/terrain/client.rs` is gated at the module
boundary. Protocol shapes contain integer cells/bitsets only, never image or collider types.

### Stable constants and engine ceilings

Add code-owned terrain constants and limits. They are not authorable through RON:

```text
TERRAIN_FORMAT_VERSION                 1
TERRAIN_CELL_SIZE_WORLD                8.0
TERRAIN_SUBCELL_SIZE_WORLD             4.0
TERRAIN_CHUNK_SIDE_CELLS               32
TERRAIN_CHUNK_SIDE_WORLD               256.0
TERRAIN_WORDS_PER_CHUNK                16
MAX_TERRAIN_CHUNKS                     221
MAX_TERRAIN_CELLS                      196_608
MAX_TERRAIN_BRUSH_RADIUS_WORLD         64.0
MAX_TERRAIN_BRUSH_CHUNKS               4
MAX_TERRAIN_ACTIVE_FIGHTERS             24
MAX_TERRAIN_COLLIDER_REBUILDS_PER_TICK 221
MAX_TERRAIN_BRUSHES_PER_TICK            24
MAX_PENDING_TERRAIN_BRUSHES            64
MAX_BUFFERED_TERRAIN_EVENTS             64
MAX_TERRAIN_RECOVERY_BYTES             48 * 1024
MAX_TERRAIN_EVENT_BYTES                96
TERRAIN_RECOVERY_COOLDOWN_TICKS        30
MAX_TERRAIN_DEBRIS_EFFECTS              64
MAX_TERRAIN_TELEMETRY_RECORDS        2_048
```

`EngineMapLimits` gains `max_destructible_cells`, `max_destructible_chunks`, and
`max_terrain_recovery_bytes` with values no wider than these ceilings. Catalog RON may continue to
narrow the number of regions but cannot widen grid or byte limits. `EngineWeaponLimits` and
`WeaponRecipePolicy` gain `max_world_effects_per_delivery = 1` and
`max_terrain_brush_radius = 64.0`; policy cannot widen them.

The map/mode composition must resolve no more than `MAX_TERRAIN_ACTIVE_FIGHTERS` for a terrain-enabled
match. Each active fighter can commit at most one single-fire Arc brush in a tick. One radius-48 brush
can touch four occupancy chunks and conditionally require an eight-chunk orthogonal halo, but the
deduplicated union across all requests can never exceed the 221 allocated chunks. Therefore the
24-brush/221-rebuild ceilings cover the complete supported terrain concurrency profile rather than
the current 2v2 implementation. Requests are sorted before admission. If malformed/injected work
exceeds either ceiling, defer the next whole brush to the bounded queue; never mutate bits without
rebuilding every required collider. Queue overflow is a diagnostic failure and rejects the newest
excess fact before mutation. A legal resolved profile must never defer or overflow in verification.
Weapon validation/tests must preserve the one-destructive-landing-per-fighter-per-tick premise for
every terrain-enabled recipe; widening firing multiplicity or timing requires recomputing this budget.

### Stable and runtime data model

Shared stable types:

```text
TerrainChunkId
  x: i16                         global chunk coordinate
  y: i16

TerrainBits([u64; 16])           row-major; bit index = local_y * 32 + local_x

TerrainBrush
  center_half_cells_x: i16       one unit = 4 world units
  center_half_cells_y: i16
  radius_half_cells: u16

TerrainGeneration
  map_instance_id: MapInstanceId
  match_id: MatchId
  terrain_fingerprint: u64

TerrainDestructionEvent
  generation: TerrainGeneration
  revision: u64
  source_attack_id: AttackId
  source_delivery_index: u8
  brush: TerrainBrush
  affected_chunks: Vec<TerrainChunkId>  // sorted, unique, <= 4
  erased_cells: u16

TerrainRecoveryRequest
  generation: TerrainGeneration

TerrainChunkSnapshot
  chunk_id: TerrainChunkId
  occupancy: TerrainBits

TerrainRecoverySnapshot
  generation: TerrainGeneration
  revision: u64
  chunks: Vec<TerrainChunkSnapshot>     // exact sorted allocated set, <= 221

TerrainResetEvent
  previous_generation: TerrainGeneration
  next_generation: TerrainGeneration
```

Match/map composition supplies a mode-owned value rather than a terrain-authored team model:

```text
ResolvedMatchCapacity
  team_slots: sorted Vec<TeamSlotCapacity>
  maximum_active_fighters: u8      // checked sum, <= 24 for terrain-enabled v1

TeamSlotCapacity
  team_slot: u8
  minimum_participants: u8
  maximum_participants: u8
```

Game-mode definitions own legal team topology and participant ranges; map validation proves that its
team slots and spawn capacity satisfy the selected definition. Operational
`ServerNetworkConfig::max_clients` must not redefine that gameplay property: composition rejects a
server limit below the selected profile's required connection capacity. Terrain reads only
`maximum_active_fighters`; it does not assign teams or inspect scoring rules.

`TerrainChunkId` is meaningful only with `MapInstanceId`/terrain fingerprint. It never contains a
process-local `Entity`. The fingerprint hashes terrain format version, cell/chunk constants, sorted
destructible region placement IDs/shapes/transforms, and initial chunk bits. It does not hash mutable
current occupancy or match revision.

Server runtime ECS:

```text
TerrainRoot component
  generation
  revision

TerrainChunk component
  id
  map_instance_id

TerrainChunkState component
  initial: TerrainBits
  current: TerrainBits
  last_modified_revision: u64

TerrainChunkCollision component
  occupied_cells
  collider_revision

TerrainChunkIndex resource
  BTreeMap<TerrainChunkId, Entity>      process-local lookup only

PendingTerrainBrushes resource          bounded sorted/deferred world facts
TerrainOutbox resource                  bounded events/reset/recovery responses
TerrainRecoveryCache resource           current sorted chunk snapshots by revision
TerrainTelemetry resource               bounded match-scoped records/aggregates
```

Each allocated server chunk is one stable entity tagged with `MapInstanceMember` or an equivalent
terrain-specific exact-generation marker. Occupied chunks also carry `RigidBody::Static`, a voxel
`Collider`, destructible collision layers, `Position` at the chunk's world-space minimum corner, and
identity rotation. Empty chunks retain state/identity but no physics components.

Client runtime state mirrors stable chunk IDs and current bits but does not spawn Avian colliders.
Windowed presentation adds `TerrainChunkVisual { image: Handle<Image> }` and one sprite per chunk;
headless clients run convergence/readiness without image assets.

### Grid and initial-layout algorithms

All coordinate helpers are pure, checked, and covered at zero, cell/chunk boundaries, negative
values, world extrema, and overflow inputs:

- world to cell uses Euclidean floor division by 8;
- cell to chunk uses Euclidean division by 32;
- cell to local index uses Euclidean remainder in `0..32`;
- chunk minimum world point is `(chunk * 32) * 8`;
- cell center in half-cell units is `(2*x + 1, 2*y + 1)`;
- world brush positions quantize through finite range validation and checked nearest 4-unit rounding;
- every wire coordinate round-trips to one canonical world value.

Initial layout resolution:

1. Select only regions using the named `DESTRUCTIBLE_TERRAIN_REGION_PROFILE`; remove raw
   `RegionProfileId(1)` checks from production logic.
2. Iterate candidate global cells in each region's rotated bounding AABB in stable
   `(placement_id, cell_y, cell_x)` order.
3. Select a cell when its center is inside the authored rectangle/circle under the canonical helper.
4. Reject any selected cell whose complete 8×8 AABB leaves playable bounds, intersects permanent
   geometry, or was selected by an earlier destructible region.
5. Reject reservations selecting zero cells, spawn circles intersecting occupied cells, and layouts
   whose initial occupied cells make required team-spawn reachability fail.
6. Fold cells into sorted 32×32 `TerrainBits`; omit chunks with no initial occupied cell.
7. Reject counts/serialized recovery estimates above the engine/catalog ceilings.
8. Compute the terrain fingerprint and return an immutable local `InitialTerrainLayout` used by
   authority and client recovery validation. It is derived, not added to `ResolvedMapSnapshot`.

Map resolution invokes this exact helper before accepting a recipe. Server and client may derive the
same layout from a validated snapshot, but only an authoritative recovery snapshot makes a client
ready. Increase built-in recipe revisions and the content/catalog schema because the formerly inert
region now has collision semantics. Keep the stable region/profile IDs unless content migration
evidence requires new IDs.

The existing Hot Zone objective may geometrically overlap terrain. Validation treats the objective
as a rule area, not collision, and separately proves at least one standard fighter-center position
per team is reachable inside the initial zone. Terrain destruction does not change anchor identity,
shape, progress, or occupancy rules.

### Voxel collider construction

For each admitted fixed-tick brush batch:

1. Compute the complete collision-dirty union: every occupancy-changed chunk plus an allocated
   orthogonal neighbor only where a changed boundary cell alters cross-chunk topology.
2. Convert each union chunk's prospective bits to sorted local `IVec2` coordinates `0..31` and
   construct a fresh `avian2d::parry::shape::Voxels` value with voxel size `(8, 8)`. Never borrow,
   downcast, clone, or mutate an installed collider's shared shape.
3. Visit each adjacent pair in stable chunk-ID order exactly once. Call
   `left.combine_voxel_states(&mut right, shift)` with the relative origin shift `(±32, 0)` or
   `(0, ±32)`. The pinned API mutates both fresh values, so both members of the pair already belong
   to the prospective transaction.
4. After all pair reconciliation succeeds, convert each non-empty value with
   `Collider::from(avian2d::parry::shape::SharedShape::new(voxels))`; stage physics-component removal
   for an empty chunk.
5. Replace every staged collider/removal and collision statistic only after all prospective shapes
   build successfully. Construction cannot consume installed collision or leave some chunks at old
   occupancy and others at new occupancy.
6. Update the recovery cache and emit the staged live events only after the complete transaction
   commits.

Boundary occupancy changes mark the orthogonal neighbor collision-dirty even when its bits do not
change. Diagonal neighbors are unnecessary because 2D face topology is axis-aligned. Chunks outside
the allocated initial set are empty forever and require no entity/collider.

Use `CollisionLayers::new(DESTRUCTIBLE_TERRAIN_LAYER, FIGHTER_LAYER | PROJECTILE_LAYER |
DEPLOYABLE_LAYER)` through one named movement helper. Preserve the existing indestructible helper.
All existing queries already combine both terrain membership masks.

### Combat world-effect contract

Add shared authored shapes:

```text
WorldEffectKind
  DestroyTerrain

WorldEffectDefinition
  DestroyTerrain { radius: f32 }

WeaponRecipe addition
  world_effects: Vec<WorldEffectDefinition>

CombatWorldEffectFact
  tick
  source: AttackSource
  delivery_index
  effect_index
  position: WorldPoint
  effect: WorldEffectDefinition
```

RON schema version 3 explicitly writes `world_effects: []` for Pulse, Scatter, and Blade and
`world_effects: [DestroyTerrain(radius: 48.0)]` for Arc Launcher. Keep `#[serde(default)]` only if
tests prove it is needed for an intentional compatibility boundary; embedded v1 content itself is
fully explicit.

Validation rejects:

- more than one world effect per delivery;
- unknown/disabled effects;
- non-finite, non-4-unit-multiple, below-8, or above-64 radii;
- destruction on Spread or non-Lobbed delivery in v1;
- policy ceilings wider than engine ceilings.

World facts are reserved/committed with the delivery transaction. Disconnect, invalid delivery,
event-ID exhaustion, or aborted attack produces no terrain fact. Terrain clips a valid fact to
destructible occupied cells and never inspects target recipient policies. `AttackId`, delivery index,
and effect index give stable ordering and telemetry identity; they are not trusted from clients.

### Authoritative fixed schedule and transaction

Add shared terrain sets:

```text
TerrainSet::CollectBrushes
TerrainSet::ApplyBrushes
TerrainSet::RebuildCollision
TerrainSet::ValidateFighters
TerrainSet::Publish
```

Required fixed-post order:

```text
FixedUpdate lifecycle/input/movement/fire
  -> Avian PhysicsSystems::Prepare
  -> Avian PhysicsSystems::StepSimulation
  -> CombatSet::ProjectileSweep
  -> CombatSet::Damage
       resolve delivery/payload transaction
       emit CombatWorldEffectFact
  -> AbilitySet::ObserveOutcomes
  -> TerrainSet::CollectBrushes
  -> TerrainSet::ApplyBrushes
  -> TerrainSet::RebuildCollision
  -> ApplyDeferred
  -> TerrainSet::ValidateFighters
  -> TerrainSet::Publish
  -> MatchSet::ModeRules
  -> common outcomes/lifecycle/telemetry/finalize
```

Configure the terrain chain after both `CombatSet::Damage` and `AbilitySet::ObserveOutcomes`, and
before `MatchSet::ModeRules`. Do not split terrain mutation across mode rules. A schedule trace test
records this exact order.

Brush admission is deterministic:

1. Merge deferred and new facts, deduplicate stable keys, and sort by
   `(tick, attack_id, delivery_index, effect_index)`.
2. Admit at most the resolved maximum-active-fighter count, capped at 24. Defer each complete excess
   fact before evaluating it; never split a brush.
3. Apply admitted facts sequentially to scratch occupancy, not ECS. No-op facts record telemetry and
   consume no revision. Each changed brush advances the scratch global revision once and stages its
   event against the state produced by all earlier sorted facts.
4. From the final scratch state, compute one deduplicated collision-dirty union containing every
   occupancy-changed chunk and only the orthogonal neighbors whose boundary topology changed. A
   legal profile cannot exceed the 221 allocated-chunk rebuild ceiling.
5. Construct and reconcile every prospective collider in that union. On any failure, commit none of
   the batch's occupancy, revisions, cache, colliders, or events.
6. Atomically install the final scratch occupancy and colliders, update the recovery cache, then
   publish staged events in revision order. Multiple brushes affecting one chunk cause one final
   collider rebuild in that tick, not one rebuild per event.

No `Commands` flush, replication send, combat observer, or mode rule may see current bits without the
matching final collider. Tests inject failures into pure prospective construction to prove batch
atomicity and event-order equivalence to sequential brush rasterization.

### Restart, map replacement, and cleanup lifecycle

Authoritative terrain reconciles the exact `ResolvedMapIdentity`. Installing a new map generation:

- removes every prior terrain root/chunk/index/cache/queue/readiness/telemetry state by exact old map
  instance;
- derives and validates the new initial layout;
- spawns deterministic ascending chunk IDs and initial colliders;
- waits for the current match root to supply the match generation before serving recovery.

`teardown_authoritative_map` invokes a narrow terrain cleanup handoff or a composition-owned exact-
generation cleanup system; stale colliders may not survive until an unrelated future frame. A map
install failure leaves neither old nor partially installed new terrain.

Match restart uses `MatchRestartSet::EnvironmentReset`. It restores all initial bitsets and colliders,
including chunks emptied during the previous match, before common commit. It does not recompute map
resolution or change terrain fingerprint. Restart tests prove old events/snapshots cannot alter the
new match and that both clients return to identical revision-zero occupancy.

### Network protocol, authority, and recovery

Register these concrete message directions in `ProtocolPlugin`:

| Message | Direction | Purpose |
|---|---|---|
| `TerrainDestructionEvent` | server → client | Apply one changed authoritative brush/revision |
| `TerrainResetEvent` | server → client | Announce exact generation reset on match restart |
| `TerrainRecoveryRequest` | client → server | Request current state for one matching generation |
| `TerrainRecoverySnapshot` | server → client | Replace complete allocated chunk occupancy at one revision |

`TerrainChannel` is `OrderedReliable` and bidirectional. Keep it distinct from `SessionChannel` and
combat cues so a fragmented recovery snapshot does not head-of-line block join outcomes or immediate
combat presentation. Within the server-to-client direction, snapshot and later live events retain
terrain ordering.

The server sends terrain traffic only to accepted/join-active links. It derives requester identity
from the link entity and never accepts a target player/entity ID from the request. Validation requires:

- known accepted link and matching current map/match/fingerprint;
- request serialized size within its fixed small bound;
- cooldown elapsed and no response already staged for that link.

Invalid requests cannot mutate terrain and receive no amplified response. Record rejection reason.
The server recovery cache holds sorted typed chunk snapshots for the latest committed revision and is
updated only for changed chunks. Response serialization is measured before send and must remain at or
below 48 KiB. Failure is a server invariant violation surfaced in verification, not silent truncation.

Client convergence is a pure state machine:

```text
WaitingForMap
  -> AwaitingRecovery { generation, request_sent, buffered_events }
  -> Ready { generation, revision }
  -> Invalid(reason)

Ready -- revision gap/corrupt event --> AwaitingRecovery
Invalid -- newer valid map or match generation --> AwaitingRecovery
```

`Invalid` is terminal only for the generation that produced the irrecoverable validation error;
disconnect clears it, and a newer valid map or match generation starts a fresh recovery. Recoverable
live-event gaps or disagreements transition from `Ready` to `AwaitingRecovery`, retain no guessed
revision, set terrain readiness non-playable, and request a full snapshot.

When the immutable map generation changes, discard all prior terrain state and rebuild the expected
initial chunk set. When the accepted match generation changes, reset readiness and accept only a
matching reset/recovery. A valid recovery snapshot replaces all current bits atomically, updates
images, sets the revision, and then applies contiguous buffered events. A valid live event applies
the integer brush and verifies that the locally erased cell count and affected chunk IDs match the
server message; disagreement requests recovery and never guesses.

Ordered reliable delivery makes gaps unusual, but revision checks remain mandatory for injected
loss, process interruption, stale caches, reconnect, and future transport changes. Tests call the
pure state machine with duplicate, missing, out-of-order, corrupted, oversized, stale-map, stale-
match, and wrong-fingerprint inputs in addition to real Lightyear transport.

Current match admission rejects new active-match participants and does not resume sessions. M10 does
not silently change that product rule. Recovery is nevertheless proven in a focused terrain network
harness with an accepted peer whose live event is withheld/corrupted and with a newly accepted peer
joining a server that already owns modified terrain outside an active admission phase. Thus terrain
itself never depends on complete history and is ready for later admission-policy changes.

### Client presentation and readiness

For each allocated chunk, create one 32×32 `Rgba8UnormSrgb` `Image` with nearest sampling and one
sprite scaled to 256×256 world units. Occupied pixels are opaque terrain; empty pixels are transparent.
An occupied cell with an empty four-neighbor uses a distinct crater-edge color. When a boundary cell
changes, mark the presentation chunk and orthogonal visual neighbors dirty so edge colors agree
across chunk seams. Upload the complete tiny 4 KiB RGBA chunk image rather than implementing partial
GPU texture writes.

The server never creates images. The client texture is presentation derived from local recovery
occupancy; modifying an image cannot modify bits, collision, revision, or send a request. Remove the
M06 orange planning overlay once live terrain is active. Keep the terrain below fighters/projectiles
and Hot Zone boundary/fill but visually distinct from floor and permanent walls.

Each changed terrain event may spawn a small deterministic cosmetic burst at the quantized brush
center. Cap live debris entities at 64, expire by client presentation time, and make them non-colliding,
non-replicated, and absent from headless composition. The crater itself is durable occupancy/image,
not an effect lifetime.

Do not add a separate destruction-audio asset or terrain-authored audio event in M10. Arc Launcher
already emits the deduplicated `CombatCue::LobLanded` path, which `ClientAudioPlugin` maps to the
bounded placeholder impact sound. The terrain crater/burst is synchronized presentation for that
same landing; tests prove one landed delivery still produces at most one impact one-shot even when it
erases several chunks. The playtest judges whether that existing sound communicates destruction
clearly enough, with a dedicated cue deferred unless evidence says it does not.

Add `ClientTerrainReadiness` to the readiness HUD and overall playable calculation:

```text
playable = accepted_join
        && ClientMapReadiness::Ready
        && ClientTerrainReadiness::Ready(matching map + match)
        && required_assets_not_loading
```

Headless clients are not exempt from terrain synchronization; only image/debris asset readiness is
omitted. HUD states distinguish `waiting for map`, `syncing terrain`, `recovering terrain`, and exact
invalid-state errors. Inputs remain suppressed until ready.

### Collision, movement, projectile, and objective behavior

Destructible voxel colliders use the already-reserved layer. Existing fighter `MoveAndSlide`, dash
shape casts, projectile sweeps, lob repair/landing, sentry placement/line of sight, melee/area line of
sight, and spawn queries must behave exactly as they do for permanent terrain except that changed
cells disappear.

Required semantics:

- a projectile colliding with occupied destructible terrain resolves its delivery exactly once;
- Arc Launcher landing outside/on terrain emits one brush fact, even with zero fighter targets;
- permanent geometry clips/blocks attacks but is never erased by a terrain brush;
- an opening becomes usable for fighter movement, projectile sweep, dash, placement, and line of
  sight on the next authoritative fixed tick after its brush transaction;
- collisions at Brawler chunk seams are indistinguishable from collisions inside one chunk;
- an empty terrain chunk contributes no collider or spatial-query hit;
- Hot Zone containment remains pure fighter-center/anchor geometry and is not rewritten by terrain;
- terrain changes never mutate fighter health/effects, objective state, props, spawn identities, or
  permanent `ArenaWall` components;
- client images/cues do not participate in any collision or targeting query.

The initial central terrain may constrain Hot Zone movement but cannot make both teams unable to
reach a legal point in the zone. Add resolver and runtime assertions for this built-in preset.

### Defensive fighter repair

`repair_embedded_fighters` runs after collider replacement and deferred flush, server-only. It uses
authoritative bits for destructible overlap and existing terrain-only Avian queries for permanent
geometry. Candidate enumeration is bounded by all playable cell centers, ordered by squared distance
from the current finite position, then global `y`, then `x`. Clamp/check the full fighter circle
inside playable bounds.

For a valid monotonic erasure, the pre-change position remains valid and no command is emitted. Tests
capture every fighter pose before/after ordinary destruction and require equality unless ordinary
movement independently changed it. An injected embedded fixture proves deterministic nearest
selection across repeated runs and stable-spawn fallback. Clients cannot request or select a repair.

### Telemetry and evidence

Add bounded records with simulation tick, map/match generation, revision, source attack/delivery,
brush, affected chunks, erased cells, rebuilt colliders, serialized event size, and outcome:

```text
Applied
NoOccupiedCell
DeferredRebuildBudget
RejectedQueueFull
Reset
RecoverySent { bytes, chunks }
RecoveryRejected { reason }
ClientGapObserved
ClientDuplicateIgnored
ClientSnapshotApplied
DefensiveRepair
```

Match/process aggregates include:

- requested/applied/no-op/deferred/rejected brushes;
- cells erased and remaining, unique occupancy-dirty/collision-rebuilt/visual-dirty chunks;
- maximum brushes and collider rebuilds in one tick;
- collider voxel count before/after and empty chunks;
- event count/min/max/total serialized bytes;
- recovery requests/accepts/rejections, snapshot chunks/bytes, gaps, duplicates, stale inputs;
- client/server final terrain fingerprint/revision/occupancy digest;
- collider rebuild and client image-update p50/p95/max wall durations in process/performance evidence;
- defensive repair count and bounded telemetry drops.

Wall-clock duration is diagnostic evidence only and never feeds simulation decisions. Occupancy
digests are deterministic hashes over sorted chunk IDs/current bits. Each process report includes the
map dimensions and allocated chunk/cell counts so demo-scale evidence cannot masquerade as full-range
evidence.

### Performance and capacity budgets

Hard correctness/capacity ceilings:

| Budget | Ceiling |
|---|---:|
| Cell size | 8 world units |
| Chunk side | 32 cells / 256 world units |
| Raw occupancy bits | 196,608 cells / 24 KiB |
| Allocated chunks | 221 |
| Occupancy words per chunk | 16 / 128 bytes |
| Occupied collider voxels per chunk | 1,024 |
| Changed chunks per brush | 4 |
| Terrain-enabled active fighters | 24 across resolved map/mode team topology |
| Collider rebuilds per fixed tick | 221 distinct allocated chunks |
| Brushes admitted per fixed tick | 24 |
| Deferred brush queue | 64 |
| Buffered client events | 64 |
| Serialized live event | 96 bytes |
| Serialized recovery snapshot | 48 KiB |
| Recovery response rate | one per accepted link per 30 ticks |
| Client RGBA pixel storage | at most 221 × 32 × 32 × 4 = 905,216 bytes before asset overhead |

Measured gates on the verification machine:

- steady maximum-map terrain with 24 active fighter fixtures stays below the 16.67 ms fixed-tick p95
  budget;
- 24 simultaneous radius-48 brushes arranged to maximize distinct dirty and conditionally reconciled
  neighbor chunks stay below 16.67 ms fixed-tick p95 across repeated reset/apply samples;
- the existing 100-fighter/200-projectile and M07–M09 performance cases remain below their current
  fixed-tick budget with destructible terrain installed;
- applying the dirty client chunk/image union from the 24-brush maximum stays below 16.67 ms Update
  p95 in native process evidence;
- maximum bounded recovery serialization, Lightyear transport, validation, and client application
  completes without timeout/disconnect and reports bytes/fragments/duration;
- entity, image, and handle counts return to the same bounded baseline after repeated match resets and
  exact map-generation replacement.

If voxel collision misses the gates, profile before changing representation. The 24-active-fighter
terrain capacity and legal-profile no-deferral contract may not be reduced as an implementation
tuning shortcut. Changing that capacity, 8-unit cell resolution, 32-cell chunks, recovery semantics,
or the collider family requires returning to specification review.

## Implementation plan

Implementation starts only after user validation. Keep the roadmap and this file synchronized at
each status transition. Complete slices in order; each slice ends with its focused green gate before
the next begins.

### Slice 0 — Accepted baseline and specification lock

- [x] Record user validation date, accepted decisions/changes, exact starting commit, and worktree
  state; set roadmap and milestone to `Implementing`.
- [x] Reconcile any accepted overlapping M01–M03/M05/M08 feedback without folding unrelated scope
  into M10.
- [x] Run `just fmt-check`, both role Clippy commands, server feature isolation, all tests,
  performance tests, Wipeout/Hot Zone process gates, and `git diff --check`; record exact counts and
  measurements.
- [x] Add schedule trace and protocol/content fingerprint expectations before behavior changes so
  accidental ordering or registry drift fails immediately.
- [x] Add the mode-owned `ResolvedMatchCapacity` composition contract: game-mode rules define legal
  team topology/ranges, map validation proves compatible team slots/spawns, server connection limits
  must not under-provision it, and terrain consumes only the checked maximum-active-fighter count.
  Preserve current Wipeout/Hot Zone behavior while removing their temporary 2v2 guard as a terrain
  assumption.

### Slice 1 — Pure grid, map validation, and content activation

- [x] Add `src/terrain/{mod,model,grid,tests}.rs` with stable IDs, bits, constants, limits, Euclidean
  coordinate helpers, half-cell brush quantization, circle erase, occupancy digest, and fingerprints.
- [x] Add focused tests for positive/negative/extreme coordinates, bit ordering, chunk crossings,
  inclusive brush boundaries, non-finite/overflow rejection, duplicate application, and stable hashes.
- [x] Add `src/map/definitions/terrain.rs`; rasterize rectangle/circle regions, including rotated
  rectangles, in canonical order and enforce cell AABB, permanent geometry, overlap, spawn,
  reachability, count, and byte budgets.
- [x] Name the destructible profile constant, remove production magic ID checks, and prove maps of
  minimum/maximum dimensions and arbitrary legal offsets resolve without demo-sized allocation.
- [x] Add pure maximum-grid and four-region fixtures, including the 17×13 chunk intersection case and
  rejection immediately above every aggregate ceiling.
- [x] Activate/bump both built-in map recipes/catalog semantics, replace planning labels with active
  terrain terminology, and prove the central region selects exactly 24×24 = 576 initial cells over
  four global chunks.
- [x] Prove initial Wipeout/Hot Zone spawns and Hot Zone legal objective positions remain reachable.

### Slice 2 — Authoritative chunks, Avian collision, and lifecycle

- [x] Add server-gated `terrain/{collider,authority}.rs` and `AuthoritativeTerrainPlugin`; reconcile
  exact map generations into sorted chunk entities/index/cache without role leakage.
- [x] Build Parry voxel shapes from current bits, combine orthogonal neighbor states, install the
  destructible layer, remove empty colliders, and record collision stats.
- [x] Prove reconciliation constructs fresh prospective `Voxels`, visits adjacent pairs once, mutates
  both fresh shapes safely, wraps them through `SharedShape` into new Avian colliders, and never
  mutates or consumes an installed collider before atomic replacement.
- [x] Add direct tests comparing occupancy to Avian point/shape/ray casts for full, edge, crater,
  empty, negative-coordinate, rotated-initial, and chunk-crossing cases.
- [x] Add seam tests moving a standard fighter along flat voxel edges and across Brawler chunk
  boundaries; require no snag, position loss, false normal, or projectile double hit.
- [x] Add terrain fixed-post sets, prospective brush transactions, stable ordering, no-op behavior,
  revision increments, rebuild halo, deferral, queue bounds, and atomic failure tests.
- [x] Implement exact map teardown/replacement and match `EnvironmentReset`; extend schedule/restart
  tests to prove no mixed generation or stale collider survives.
- [x] Implement defensive repair and prove zero calls in valid scenarios plus deterministic injected
  fixture/fallback behavior.

### Slice 3 — Combat world effects and playable destruction

- [x] Extend weapon definitions, RON policy, validation, serialization, fingerprints, builders, and
  tests with delivery-level world effects; bump schema/content/protocol expectations deliberately.
- [x] Add explicit empty lists to Pulse/Scatter/Blade and one radius-48 Arc Launcher terrain effect.
- [x] Emit bounded `CombatWorldEffectFact` only after delivery transaction commit; cover targetless
  landing, multiple targets, disconnect, aborted delivery, and event-ID exhaustion.
- [x] Consume facts in terrain authority without combat importing terrain internals; verify one brush
  per Arc delivery rather than per area target.
- [x] Prove projectiles/fighters/dash/sentry/placement/LOS interact with occupied/erased cells on
  the specified next-tick boundary while permanent terrain and unrelated state remain unchanged.
- [x] Add terrain-specific combat/weapon telemetry and update definition/content validation evidence.

### Slice 4 — Protocol, recovery, convergence, and forgery resistance

- [x] Add terrain channel/messages in `protocol.rs`, exact direction tests, fixed bounds, registry and
  protocol fingerprint bumps, and server/client link components.
- [x] Add `terrain/network.rs` pure convergence state and tests for valid snapshot/event/reset plus
  duplicate, missing, out-of-order, stale, wrong-fingerprint, invalid-ID, invalid-count, oversized,
  buffer-overflow, and revision-overflow cases.
- [x] Add server recovery validation, per-link cooldown, accepted-link gating, current-state cache,
  bounded serialization, and ordered response/live-event publication.
- [x] Add client request/buffer/recovery/reset flow and generation matching without terrain collision.
- [x] Extend the Crossbeam/UDP harness with terrain helpers and `tests/network/terrain.rs`; prove two
  live clients plus an impaired and a newly accepted client converge to identical revision/digest.
- [x] Prove forged recovery cannot mutate state, target another client, bypass admission/rate limits,
  request stale generations, or amplify an invalid request into a large response.
- [x] Prove restart returns server/clients to revision-zero initial occupancy and stale prior-match
  events/snapshots are ignored.

### Slice 5 — Client visuals, readiness, and feedback

- [x] Add client-gated `terrain/client.rs`; create one nearest-filtered 32×32 image/sprite per expected
  chunk and update dirty chunks plus edge-neighbor visuals from occupancy.
- [x] Remove the inert orange reservation overlay, establish explicit z ordering, and retain clear
  distinction among floor, permanent walls, destructible cells, Hot Zone, fighters, and projectiles.
- [x] Add bounded non-colliding debris feedback derived from events and exact cleanup on expiry,
  reset, disconnect, and map replacement.
- [x] Reuse the existing deduplicated Arc landed-impact audio, add no new destruction asset/event,
  and prove multi-chunk erasure does not multiply the one-shot.
- [x] Add `ClientTerrainReadiness`, HUD states, playable/input gating, and headless convergence without
  render assets; test all non-atomic map/match/terrain arrival orders.
- [x] Add screenshot/image sampling checks for intact terrain, cell-sized edge, crater, cross-chunk
  crater, empty chunk, Hot Zone overlap, reset, and multiple aspect/window sizes.
- [x] Verify controller and keyboard/mouse Arc Launcher use the same gameplay path and receive the
  same terrain feedback.

### Slice 6 — Scale, processes, MVP gate, and handoff

- [x] Add performance fixtures for aligned/off-grid maximum maps, 221 allocated chunks, near-maximum
  occupied cells, steady colliders with 24 active fighters, 24 simultaneous worst-placement brushes,
  repeated reset, recovery, and client image updates; record p50/p95/max and entity/asset counts.
- [x] Re-run existing combat/build/match/Hot Zone performance cases with active terrain and investigate
  regressions rather than weakening established budgets.
- [x] Extend process verification/reporting with map dimensions, terrain format/fingerprint, chunk/
  cell counts, revisions/digests, brush/rebuild/recovery aggregates, serialized sizes, and repair/drop
  counts.
- [x] Add bounded Wipeout and Hot Zone real-process terrain profiles under local, typical, and adverse
  network conditions; prove two current clients and recovery convergence.
- [x] Run the complete canonical verification gate and record exact command/output evidence.
- [x] Audit every criterion in [Gameplay MVP](../../05-gameplay-mvp.md); link each to a repeatable test
  or playtest observation and update product scope explicitly for any intentionally removed criterion.
- [x] Set `User playtest` only after automated/process/visual/controller gates pass; provide the
  handoff below and request the listed observations.

## Implementation evidence (2026-08-16)

### Slice 1 evidence

- `src/terrain/{mod,model,grid,tests}.rs` landed with stable `TerrainChunkId`/`TerrainGeneration`,
  `[u64; 16]` `TerrainBits`, the 8-unit cell / 4-unit half-cell / 32-cell-chunk constants, the
  `MAX_TERRAIN_CHUNKS = 221` / `MAX_TERRAIN_CELLS = 196_608` ceilings, Euclidean floor-division
  coordinate helpers, integer circular erase, occupancy digest, and region/initial-bit fingerprints.
- Grid tests cover positive/negative/extreme coordinates
  (`world_to_cell_uses_euclidean_floor_division`, `floor_and_ceil_division_handle_negative_values`),
  chunk boundary round-trips, geometry/constants agreement, brush quantization round-trips, row-major
  bit ordering, inclusive/symmetric erase, occupied-clipped and cross-seam application, digest
  stability/sensitivity, wire-shape bounds, and fingerprint sensitivity.
- `src/map/definitions/terrain.rs` rasterizes rectangle/circle/rotated-rectangle regions in canonical
  order behind `DESTRUCTIBLE_TERRAIN_REGION_PROFILE` (no production magic-ID checks remain) and
  rejects out-of-bounds cell AABBs, permanent-geometry overlap, unsafe spawns, unreachable layouts,
  duplicate selected cells, and aggregate-ceiling overflow with exact reasons.
- `maximum_grid_fixtures_resolve_and_reject_at_the_aggregate_ceilings` proves minimum/maximum and
  arbitrary-offset playable sizes resolve, the maximum off-grid footprint intersects exactly
  17×13 = 221 global chunks, and one step above every ceiling rejects.
- `content/v1/maps.ron` moved to schema 2 (recipe revisions 2→3 and 1→2) activating the destructible
  profile as data; the central 192×192 region selects exactly 576 cells split evenly across four
  global chunks, both built-in presets stay solid at the center, and initial Wipeout/Hot Zone spawns
  plus Hot Zone objective anchors remain reachable (map definition tests, 576-cell assertions for
  both recipes).

### Slice 2 evidence

- `src/terrain/{collider,authority}.rs` and server-gated `AuthoritativeTerrainPlugin` reconcile exact
  map generations into sorted chunk entities, the chunk index, and the recovery cache. Colliders are
  Parry `Voxels` built fresh from current bits, boundary states are combined across orthogonal
  neighbor chunks, empty colliders are removed, and new `SharedShape`s are installed only after the
  whole prospective batch succeeds.
- Authority tests: exact reconciled chunk install, brush transaction
  (erase/revision/collider rebuild/telemetry/outbox), no-op brushes without revision, chunk-seam
  brushes rebuilding the boundary neighbor, whole-brush admission deferral with queue-overflow
  rejection, map-replacement teardown leaving no stale terrain, restart reset restoring initial
  occupancy at revision zero, Avian point-query agreement with occupancy for full/edge/crater/empty/
  negative/rotated/cross-seam probes, and a schedule trace placing terrain between damage effects
  and mode rules.
- `one_hundred_destroy_reset_cycles_stay_fast_and_exact` runs 100 destroy/reset cycles asserting
  revision and occupancy return to (0, initial) each cycle in under 4 seconds total.
- Defensive repair helpers exist server-side with injected deterministic behavior and zero calls in
  every valid scenario across the terrain, movement, combat, and performance suites (repair counters
  asserted `0` in the M10 performance fixtures).

### Slice 3 evidence

- `content/v1/weapons.ron` moved to schema 3 adding delivery-level `world_effects` with
  `max_world_effects_per_delivery: 1`; Pulse/Scatter/Blade carry explicit empty lists and only the
  Arc Launcher carries `DestroyTerrain(radius: 48.0)`.
  `only_the_arc_launcher_carries_a_terrain_world_effect` and
  `world_effect_validation_rejects_invalid_count_radius_and_delivery` lock the policy.
- Bounded `CombatWorldEffectFact` is emitted from the delivery transaction in `combat/effects.rs`
  only after commit, one fact per landed Arc delivery regardless of target count, with targetless
  landings covered; disconnected/aborted deliveries emit none (combat recovery suites unchanged and
  green).
- Terrain authority consumes the facts without combat importing terrain internals;
  `terrain::one_arc_landing_erases_multiple_chunks_but_plays_one_landed_cue` proves one brush per
  delivery over the wire even when the crater spans a chunk seam, and existing weapon
  damage/knockback/slow/economy suites are unchanged.
- Telemetry: terrain authority records brush/collision/repair/recovery aggregates consumed by the
  M10 process verification fields and performance fixtures.

### Slice 4 evidence

- `cargo test --lib --features server`: 223 passed including 14 new pure convergence tests
  (`terrain::tests::convergence_tests`) covering valid snapshot/event/reset, duplicate, missing,
  out-of-order replay, stale map/match, wrong fingerprint, foreign chunk IDs, false reports,
  snapshot count/set violations, oversized snapshots, buffer overflow, revision exhaustion,
  generation change, and disconnect clear.
- `cargo test --test network --features network-test`: 73 passed including
  `terrain::{two_live_clients_converge_on_authoritative_terrain_events,
  impaired_and_late_joining_clients_converge_via_recovery,
  forged_recovery_requests_cannot_mutate_target_or_amplify,
  restart_returns_server_and_clients_to_revision_zero_ignoring_stale_history}`.
- Protocol: `TerrainChannel` (OrderedReliable, bidirectional) plus four messages with exact
  direction tests; `SUPPORTED_PROTOCOL_VERSION` bumped 10 → 11; registry fingerprints shift with
  the new messages and channel (covered by the existing fingerprint-change test).
- Server recovery: accepted-link gating, generation match, request byte bound, 30-tick per-link
  cooldown component, staged-response deduplication, 48 KiB measured snapshot ceiling, counted
  rejections with exact reasons.

### Slice 5 evidence

- `cargo test --lib --features client`: 176 passed including six new terrain presentation tests
  (fill/rim pixels, cross-seam rim following, transparent holes, nearest RGBA8 image, one sprite
  per expected chunk at the terrain z depth, dirty-chunk plus orthogonal-neighbor repaint with
  one bounded debris burst per committed brush).
- `terrain::one_arc_landing_erases_multiple_chunks_but_plays_one_landed_cue` proves one landed
  Arc delivery over the wire plays exactly one `LobLanded` cue while its brush splits across a
  chunk seam; no new audio asset or terrain-authored cue exists.
- Map presentation count updated 525 → 524 after removing the orange reservation overlay;
  terrain sprites sit at z −6 between the floor (−10) and spawn/objective layers (−4.8…−4),
  below permanent walls (2) and dynamic entities (10+).
- `ClientTerrainReadiness` drives four HUD states (waiting for map / syncing terrain /
  recovering terrain / exact invalid reasons) and a PostUpdate playable clamp that suppresses
  inputs until convergence reports Ready, restoring headless gates once an accepted client's
  terrain converges; recovery requests re-arm after a 60-tick silent window so a lost request
  or response on real UDP cannot wedge input.
- Keyboard/mouse and controller both fold into `PendingLocalActions` before the single
  `write_client_input` path, so both devices drive the same authoritative Arc deliveries and
  receive the same debris/crater feedback.
- Windowed visual evidence (2026-08-16, real processes over UDP): a dedicated server plus one
  windowed 1280×720 Arc Launcher combat-demo client captured ten in-process screenshots
  (`--screenshot-*`, frames 320–680). Image-model and direct pixel inspection show the central
  destructible block rendering with its beige edge at the terrain z depth behind floor/walls and
  in front of nothing gameplay-critical, progressive quantized craters eaten into its south and
  east edges growing to fighter-width openings across frames, live explosion and debris effects
  near the edges, the HUD objective/score line, and terrain convergence logged before input
  un-gating. An earlier capture attempt that showed only the connection screen was traced to a
  misconfigured server (`--max-clients 2` under-provisioning the capacity profile), not a
  rendering regression; the automated pixel-level checks above remain the repeatable visual
  gate and multi-window-size judgment is deferred to the user playtest as recorded below.

## Gameplay MVP acceptance audit (2026-08-16)

Every criterion from [Gameplay MVP](../../05-gameplay-mvp.md) with its repeatable evidence:

| Criterion | Repeatable evidence |
|---|---|
| Controls understandable without a tutorial | `just user-test`/`just run` HUD controls text; M09 playtest confirmed; unchanged by M10 (terrain adds only the syncing HUD states). |
| Every weapon has a preferred distance and counterplay | Weapon catalog distances/falloff in `content/v1/weapons.ron`; M05–M08 combat network/performance suites; Arc gains terrain control as its niche (M10 `terrain::one_arc_landing…`). |
| Fighters can reliably hit, damage, defeat, and respawn | `tests/network/combat_{pulse,projectiles,composed,recovery}.rs`, `matchplay` lifecycle suites; all green in the M10 canonical gate. |
| Players can identify why they lost a fight | Damage/effect cues, HUD health/effects and kill attribution from M05/M07 (`client_cues` assertions). |
| Presets produce visibly different match behavior | `tests/network/builds.rs` preset-specific sentry/dash/weapon behavior suites. |
| Bounded build customization accepted/rejected server-side | `tests/network/selection.rs`/`builds.rs` legal/illegal recipe outcomes. |
| Match finishes in roughly two to four minutes | Match lifecycle rules + `tests/network/match.rs` completion/restart timings; process report `active_duration_ticks`. |
| Weapon values changeable without code changes | `content/v1/weapons.ron` schema v3 (M10 adds `world_effects` there). |
| Map layouts/presentation/geometry/regions/entities/spawns changeable in data | `content/v1/maps.ron` schema v2 (M10 activates destructible regions as data); `tests/network/map.rs` + `src/map/definitions/tests.rs` (incl. maximum-grid fixtures). |
| Same fighter/weapon code under Wipeout and Hot Zone | `tests/network/{match,hot_zone}.rs`; M10 hot-zone-over-terrain validation (`hot_zone.rs` anchors + terrain reachability proof). |
| Complete combat loop with an Xbox-like controller | Controller demo path + input tests (`src/client/input.rs`, `client/tests.rs`); terrain feedback is device-independent (single `write_client_input` path). |
| Same actions playable with keyboard/mouse | Same suites; no separate implementation (shared `PendingLocalActions`). |
| Two local clients play one server-authoritative match | `tests/network/` multi-client harness; `just run` two-window script. |
| Clients cannot authoritatively alter positions, damage, status, scores, or terrain | Forged-input/forged-request suites: `movement_input.rs`, `terrain::forged_recovery_requests…`; server-only mutation ownership in `combat/movement/terrain` authority modules. |
| Terrain destruction creates readable quantized holes/passages; visible tiles never authoritative | Quantized grid + crater visuals (`terrain::client` tests: fill/rim/holes/cross-seam); occupancy truth only in server authority + recovery snapshots; client image edits cannot touch bits/collision/revision (pure presentation derivation). |
| Terrain allocation, destruction, rebuilds, recovery bounded for every map size | Engine ceilings + map validation (`maximum_grid_fixtures…`), M10 performance fixtures (aligned 192-chunk / off-grid 221-chunk ceiling maps, 24 brushes, 32,539-byte snapshots, 100 reset cycles; remediated 2026-08-16 to the exact ceilings). |
| Terrain collision updates leave props/objectives/fighters unchanged | Hot Zone anchor identity/progress independent of terrain (validation treats anchors as rule areas); collider writes target only terrain chunk entities; M10 network tests assert objectives and fighters unaffected. |

No criterion was removed or weakened; the terrain bullet added by this milestone is covered above.

### Slice 6 evidence

- Performance fixtures (`just test-performance`, 14 passed after remediation) added four M10 cases
  on top of the existing 100-fighter/200-projectile, combat, build, match, and Hot Zone cases
  re-run with active terrain:
  - `m10_aligned_and_off_grid_maximum_terrain_stay_within_fixed_tick_budget` — maximum playable
    maps (4,096×3,072) aligned and at an arbitrary off-grid offset allocate exactly 192 and 221
    chunks with 194,048 and 193,153 occupied cells (both under the 196,608 ceiling), p95
    brush-and-rebuild ≈ 0.9 ms (aligned) and ≈ 0.5 ms (off-grid), zero defensive repairs, inside
    the fixed-tick budget. Remediation note: the pre-review fixture allocated only 176 chunks; the
    post-review fixture reaches the true ceilings with four engine-legal reservations and a clear
    corner notch for spawn/fighter clearances, and the runtime re-derivation path uses default
    engine limits throughout.
  - `m10_24_fighters_and_24_simultaneous_seam_brushes_stay_within_fixed_tick_budget` — 24 active
    fighters placed on the ceiling map's clear notch with the admission ceiling raised to the
    capacity profile's maximum and 24 simultaneous radius-48 brushes at worst-placement chunk
    seams: all 24 applied, p95 burst ≈ 3.8 ms, zero repairs.
  - `m10_recovery_serialization_and_client_image_painting_stay_within_budget` — 20 recovery
    snapshot serializations over the 221-chunk off-grid layout average ≈ 22 µs / 32,539 bytes
    (≤ 48 KiB); chunk image painting averages ≈ 6.2 µs per 32×32 repaint.
  - `m10_varied_team_capacities_derive_admission_and_admit_without_deferral` — resolved capacities
    for 2×2, 3×2, 4×3, 2×12, 8×3, and 24×1 team topologies derive the expected
    `TerrainAdmissionCapacity` (clamped at the 24-brush ceiling) and admit exactly that many
    worst-placement brushes in one fixed tick with no deferral, rejection, or repair.
- `one_hundred_destroy_reset_cycles_stay_fast_and_exact` covers repeated reset growth: 100
  destroy/reset cycles with per-cycle tick-advancing detonations return to revision zero and
  initial occupancy each cycle in under 4 s with empty pending/batch/fact queues after every reset,
  stable chunk-entity and index counts, and telemetry records within their 2,048 bound.
- `one_hundred_client_destroy_reset_cycles_hold_visual_and_debris_bounds` (client feature) drives
  the same 100 cycles through the public convergence path with the visual/debris presentation
  systems installed: chunk-visual entities and `Assets<Image>` handles stay at their baseline
  counts across all cycles, debris holds the 64 ceiling, and the last feedback expires to exact
  zero one lifetime after the final reset.
- Process verification: `verify_process_match` reports 17 terrain rows (format/fingerprint, chunk
  and cell counts, revision, occupancy digest, applied brushes, erased cells, collider rebuilds,
  recovery requests/responses/rejections, snapshot bytes, event min/max bytes, defensive repairs,
  dropped records).
- Real-process terrain profiles (all six green, 2026-08-16): a new `BRAWLER_NETWORK_ASSERT_TERRAIN`
  server check requires authoritative destruction (peak revision ≥ 1 with occupancy strictly
  reduced, peak-tracked so a post-disconnect match reset cannot erase mid-run evidence) inside a
  bounded window, and writes a ready file plus a terrain report. Two Arc Launcher clients aim at a
  terrain-profile practice target placed just south of the central block
  (`BRAWLER_NETWORK_TERRAIN_TEST_DUMMY=1`), so landed brushes erase real cells from any spawn
  lane and firing phase. `just network-terrain` (Wipeout) and `just network-terrain-hot-zone`
  passed under local, typical (25 ms ± 5 ms), and adverse (50 ms ± 10 ms, 2% loss) conditions:
  every run reached revision 1 with 2–32 cells erased, served 2 recovery snapshots, logged
  `client terrain converged` on both clients before input un-gating, and exited 0.
- Client convergence diagnostics: each client logs one `client terrain converged` INFO line per
  generation when readiness first reaches Ready, so profile and playtest runs can prove
  convergence from process logs.
- Canonical gate (2026-08-16, after final content restore): `just fmt-check`, `just clippy-client`,
  `just clippy-server`, `just server-features`, `just check`, `just test-client` (176 passed),
  `just test-server` (166 passed), `just test-network` (73 passed), `just test-performance`
  (13 passed), `just network-smoke`, headless Hot Zone smoke
  (`BRAWLER_NETWORK_GAME_MODE=hot-zone`), `just network-terrain` ×3 conditions,
  `just network-terrain-hot-zone` ×3 conditions, and `git diff --check` all green.
- Pre-existing failure recorded, not introduced by M10: `just network-combat-profiles` fails
  deterministically at the pre-M10 baseline too (rebuilt baseline binaries reproduced the same
  timeout with defeat/reset evidence missing, `pending_checkpoints=0`, zero payload effects
  landing on the neutral dummy). It was re-verified failing identically before and after M10
  changes; the defeat-evidence path it exercises is unrelated to terrain, and the M10 terrain
  profiles above provide the real-process terrain convergence evidence. Fix deferred outside M10
  scope (tracked in the backlog).
- The Gameplay MVP audit table above links every criterion to repeatable evidence; no criterion
  was removed or weakened.

## Verification plan

### Pure grid and map tests

- world/cell/chunk/local conversions round-trip at zero, ±cell/chunk edges, arbitrary offsets, map
  extrema, and negative Euclidean boundaries;
- bit get/set/clear/count/iteration and postcard round-trip are stable and reject malformed sizes;
- circular brush rasterization is integer-only after quantization, symmetric where expected, bounded,
  deterministic across insertion order, and clips only current occupied cells;
- identical initial recipes and brush sequences yield identical fingerprint/revision/digest;
- rectangles, rotated rectangles, and circles select expected cells; duplicate selected cells,
  permanent overlap, out-of-bounds cell AABBs, unsafe spawns, empty reservations, unreachable layouts,
  and aggregate overflow reject with exact reasons;
- min/max/off-grid playable sizes resolve; current 192×192 fixture and near-maximum fixtures prove the
  implementation does not allocate from demo assumptions.

### Focused ECS, schedule, collision, and lifecycle tests

- exact map generation owns exact terrain roots/chunks/index/cache and teardown leaves no stale state;
- chunk entities are stable and sorted; only occupied chunks carry static voxel collision;
- Avian point/ray/shape queries match bits before and after brush/restart;
- boundary state across adjacent voxel colliders prevents fighter/projectile snag/double contact;
- only occupancy-dirty chunks plus required orthogonal collision neighbors rebuild;
- bit/collider/cache/event commit is atomic and a changed brush increments revision exactly once;
- schedule trace proves impact/damage precedes terrain, terrain precedes mode rules/finalize, and new
  openings are usable on the next authoritative tick;
- valid destruction never moves fighters; injected overlap repair is deterministic and server-only;
- restart restores every initial bit/collider and exposes no new-match/old-terrain state.

### Combat and content tests

- schema/policy/fingerprint validation covers world-effect count/kind/delivery/radius restrictions;
- only Arc Launcher carries the v1 brush; existing weapon damage/knockback/slow/economy behavior is
  otherwise unchanged;
- landing with zero/one/many targets emits exactly one terrain fact; one fact is not duplicated by
  target count, cue count, retry, or event readers;
- disconnected/invalid/aborted deliveries emit none; a no-terrain landing records no-op without a
  terrain revision;
- permanent terrain is unaffected, destructible collision changes, and combat/objective/prop/fighter
  state changes only through their existing owners.

### Deterministic network tests

- two current clients apply the same live events and converge on revision/digest;
- duplicate events are ignored; missing/out-of-order/corrupt events trigger bounded recovery;
- a full snapshot atomically replaces state, then contiguous buffered events apply;
- a newly accepted client and an impaired existing client converge without historical replay;
- reset/reconnect/map replacement discard stale generations and reach the new authoritative state;
- invalid/oversized/wrong-ID/wrong-fingerprint/rate-limited requests receive no large response and
  cannot alter authority;
- snapshot/event serialized sizes stay within ceilings and fragmented recovery completes under the
  existing local/typical/adverse profiles.

### Performance, soak, and growth tests

- 221-chunk maximum-offset layout construction and steady query workload;
- near-maximum initial occupied cells and empty-after-destruction state;
- resolved capacities covering varied team counts/team sizes and the 24-active-fighter ceiling;
- repeated 24-brush maximum-union rebuild samples with conditional collision neighbors;
- 100 fighters/200 projectiles plus active destructible collision regression;
- repeated recovery snapshots to four clients under impairment without disconnect/timeout;
- at least 100 reset/destruction cycles with stable terrain entity/image/handle/cache counts and no
  revision, queue, telemetry, or debris leak;
- fixed-tick and client-update p95 gates from the budget section.

### Canonical commands

During `Verifying`, run and record at minimum:

```text
just fmt-check
just clippy-client
just clippy-server
just server-features
just check
just test-client
just test-server
just test-network
just test-performance
just network-smoke
```

Extend the established match/process scripts with named M10 Wipeout and Hot Zone terrain profiles;
do not invent undocumented one-off process commands as the only evidence. Run `git diff --check`
after documentation closeout.

## User playtest handoff

Do not enter this phase until automated and process gates are green. Provide one canonical command
that builds/starts a dedicated server and two windowed clients in the terrain scenario, plus a second
Hot Zone command if mode selection is not exposed in the same launcher.

Canonical commands (built and verified 2026-08-16):

```text
# Wipeout: one dedicated server plus two windowed clients (select the Arc Launcher preset, preset 3)
just run

# Hot Zone terrain scenario
just network-hot-zone

# Optional automated terrain sanity before/after the session (headless, asserts destruction +
# two-client convergence under local/typical/adverse impairment)
just network-terrain
just network-terrain-hot-zone
```

Controls (from the in-client HUD help): WASD / left stick move; mouse / right stick aim; left mouse
button / right trigger primary fire; Q active item; E ultimate; Space interact (ready-up); Tab
scoreboard; Esc pause/cancel. `just run` starts in Wipeout; both windowed clients must ready up
(Space) to end the countdown.

Scenario:

1. Join with controller or keyboard/mouse and select the Arc Launcher preset/build.
2. Approach the marked central destructible block and fire at edges, center approaches, and a chunk
   seam until a fighter-width opening exists.
3. Walk and dash through openings; fire Pulse/Scatter projectiles through them; test shots grazing
   quantized edges.
4. In Hot Zone, contest around the initial block, open alternate approaches, and confirm objective
   capture remains understandable.
5. Trigger the scripted impaired/recovery client and confirm its visible terrain catches up without
   changing the server/current clients.
6. Complete/restart the match and confirm the original terrain returns with no stale crater/debris.

Requested observations:

- Are 8-unit steps visible or distracting at normal camera scale?
- Do openings communicate passable versus projectile-only width clearly?
- Does movement or aiming snag at cell or chunk seams?
- Does the Arc Launcher remove enough/too much terrain per shot, and is the changed area predictable?
- Does the 48-unit crater read as an intentional inner core of the 150-unit combat blast, or as a
  collision/visual mismatch?
- Does the existing landed-impact sound communicate terrain destruction without needing a second cue?
- Is the initial Hot Zone center tactically interesting or merely obstructive?
- Are permanent and destructible cover visually distinguishable before firing?
- Do crater edges/debris obscure fighters, projectiles, health, or objective state?
- Is any destruction, recovery, or reset hitch visible?
- Do controller and keyboard/mouse paths feel behaviorally identical?

Known v1 limitations in the handoff: grid-quantized edges, erase-only terrain, one destructive weapon
effect, no structural collapse/materials/persistence, no active-match session resumption, and local-
scale recovery rather than production compression.

## Feedback review (2026-08-16)

An implementation review of the delivered M10 change set found two lifecycle defects, two
boundedness/readiness defects, and overstated capacity/exit evidence. All five findings were
verified against the live source and the pinned Bevy 0.19.1 executor semantics, accepted, and
implemented now as the remediation commit; none changed the validated cell/chunk/collider/recovery
contracts, so the milestone stays in feedback review rather than returning to specification review.

| Feedback | Evidence | Decision | Follow-up verification |
|---|---|---|---|
| [P1] Deferred brushes survive match restart | `reset_terrain_on_match_restart` replaced the transaction/outbox but never cleared `PendingTerrainBrushes`, `TerrainBrushBatch`, or `CombatWorldEffectFacts`; `collect_terrain_brushes` drains all three in the restart tick's fixed-post chain (payload resolution runs in `CombatSet::Damage` after the FixedUpdate reset), so an old-match detonation could carve freshly restored terrain and publish it under the new generation | Implement now | Reset clears the deferred queue, the collected batch, and combat's fact buffer, and advances a new `TerrainBrushEpoch` past the restart tick so same-tick deliveries are dropped and counted; `restart_clears_queued_brushes_and_rejects_restart_tick_facts` reproduces the ordering, the 100-cycle soak now pushes undrained facts and asserts empty queues per cycle |
| [P1] Standalone map teardown leaves fixed-post systems without required resources | `teardown_authoritative_terrain` removed six resources while `collect_terrain_brushes`, `apply_terrain_brushes`, and `repair_embedded_fighters` hold them as unconditional params; Bevy 0.19.1 treats a missing `Res` as an error validation and its own `missing_resource_panics_*` executor tests confirm the schedule panics | Implement now | Teardown resets the shared resources to a valid empty generation instead of removing them; `exact_teardown_without_reinstall_keeps_fixed_post_systems_schedulable` runs fixed ticks after an exact teardown with no reinstall and then reinstalls |
| [P2] Reset accepted before the client observes the new match generation | `apply_reset` accepted the reset when the observed match id equaled the old committed id, violating the "only with the matching new `MatchState.match_id`" contract | Implement now | `apply_reset` accepts only the matching new match id; an early reset holds as `pending_reset`, leaves the syncing state, and converges via one recovery exchange while repeated pre-restart observations no longer churn to stale requests; `reset_outrunning_match_observation_syncs_through_recovery` covers hold/converge/supersede |
| [P2] Cosmetic debris could exceed its 64-entity ceiling | `spawn_terrain_debris` trimmed the existing set to 63 and then spawned every applied brush, so a 24-brush tick reached 87 live entities | Implement now | Capacity is budgeted across existing plus pending spawns, keeping the newest burst and retiring the oldest first; `debris_bursts_respect_the_ceiling_across_existing_and_new_effects` pins 63+24 and repeated bursts to exactly 64 |
| [P2] Capacity/exit claims lacked their specified evidence | The performance fixture allocated 176 chunks while the checklist marked the 221-chunk workload complete (221 existed only in resolver tests), the varied-team capacity fixture was missing, and the reset soak checked only occupancy/revision/elapsed | Implement now | The maximum-map fixture now builds the true ceilings (aligned 192 chunks, off-grid 221 chunks with a clear spawn notch from four engine-legal reservations), all three m10 fixtures assert the exact ceilings, `m10_varied_team_capacities_derive_admission_and_admit_without_deferral` covers 2x2 through 24x1 topologies, and both soaks assert entity/index/queue/telemetry/server and visual/image/debris/client stability across 100 cycles |
| Maintainability: `authority.rs`/`client.rs`/`network.rs` mix lifecycles; module-wide Clippy suppressions | `authority.rs` mixed generation lifecycle with the per-tick pipeline; item-attached `too_many_lines` allows on `commit_terrain_collision` and an inert allow above `TerrainMutationState`; the documented module-level cast/wildcard allow in `authority.rs` predates the review | Partially now, remainder deferred | Restart/teardown/reconcile/install moved to a focused `terrain/lifecycle.rs` submodule with an explicit import list, the inert allow was dropped, and the queue-clearing invariants live with the reset; further `network.rs`/`client.rs` decomposition deferred to the v1 backlog as `GAP-ORG-TERRAIN-SPLITS` to avoid invalidating recorded evidence mid-remediation |

Remediation verification (re-run 2026-08-17): `just fmt-check`, `clippy-client`, `clippy-server`,
`check`, `server-features`, `test-client` (179), `test-server` (169), `test-network` (73),
`test-performance` (14), `git diff --check`, `network-smoke` (movement assertion at tick 595),
`network-terrain` (assertion passed at revision 1, 537/576 cells, both clients converged, exit 0),
and `network-terrain-hot-zone` (assertion passed at revision 1, 574/576 cells, exit 0) all green.

## Feedback review, round 2 (2026-08-17)

A second implementation review found five P2 hardening/evidence defects and one P3 invariant gap on
top of the remediated build. All six were verified against the live source and implemented now; none
changes the validated cell/chunk/collider/recovery contracts or the wire protocol, so the milestone
remains in feedback review.

| Feedback | Evidence | Decision | Follow-up verification |
|---|---|---|---|
| [P2] A no-op terrain event silently consumes a client revision | `event_shape_is_valid` checked radius/chunk bounds but not effect size, and a self-consistent zero-erasure event (a repeat brush inside its own crater) passed the rasterization equality check, advanced the revision, and even staged cosmetic debris; the next genuine event at that revision was then ignored as stale while readiness stayed `Ready` | Implement now | Live events must report at least one erased cell and one affected chunk; violations take the existing corrupt-input recovery path. `zero_effect_events_recover_instead_of_consuming_a_revision` commits a real erase, repeats the same brush, and pins revision 1 plus recovery; the presentation burst helper now staggers brushes so every event erases fresh cells |
| [P2] Recovery accepted impossible constructed terrain | `apply_snapshot` validated generation/chunk-set/size but never the bits, so a corrupted snapshot could occupy cells outside authored terrain (or below initial at revision zero) and still make the client `Ready` | Implement now | Snapshots must be a subset of the authored initial occupancy and a revision-zero snapshot must equal it exactly; violations invalidate with exact reasons. `snapshots_may_not_construct_cells_or_rewrite_a_revision_zero_state` covers constructed cells, preseeded revision zero, and a legal erase-only subset |
| [P2] Match capacity was never resolved against the selected map | `ResolvedMatchCapacity::from_rules` consumed only lifecycle rules, and no composition step compared its team slots or per-team maxima against the selected map; the 24-fighter performance test inserted synthetic capacity directly | Implement now | `ResolvedMatchCapacity::validate_against_map` checks exact team-slot equality and spawn points per team against every simultaneous participant; a `Startup` system ordered after map instantiation panics on mismatch like the connection-capacity gate. The 24-fighter fixture now derives its 2x12 capacity through `from_rules`, validates it against the fixture map (twelve spawn points per team under a widened large-group layout policy), and lets terrain derive admission from the published capacity; matchplay unit tests cover satisfying maps, slot mismatch, under-provisioned spawn capacity, and the composition panic |
| [P2] Cosmetic debris survived terrain-generation cleanup | `TerrainDebris` carried only an expiration timer, so reset, map replacement, and disconnect left old-generation debris visible for up to 500 ms | Implement now | Debris carries its terrain generation and a sweep retires any debris whose generation differs from the convergence machine's current phase; `WaitingForMap` after disconnect matches nothing, clearing everything. Generation transitions also drop pending presentation artifacts in the state machine itself. `stale_generation_debris_is_retired_immediately` covers reset, re-convergence, and disconnect without waiting on the timer |
| [P2] Several promised telemetry paths were inert | `ClientGapObserved`, `ClientDuplicateIgnored`, and `ClientSnapshotApplied` had no producers or consumers anywhere; every `rebuilt_colliders` literal was zero; `visual_dirty_chunks` omitted the seam neighbors presentation actually repaints | Implement now (wired, not narrowed) | Applied records are staged by the brush loop and finalized by the collision commit with per-event collider-rebuild attribution, so a refused batch records deferral rather than a phantom application; the commit extends `visual_dirty_chunks` with allocated orthogonal seam neighbors (the exact repaint rule); the client terrain plugin owns a `TerrainTelemetry` resource and records duplicate revisions, observed gaps, and applied snapshots at the wire receive sites. `applied_records_and_aggregates_carry_real_rebuild_and_visual_counts` pins an interior brush to one collider rebuild but three visually dirty chunks, and `client_convergence_telemetry_records_duplicates_gaps_and_snapshots` exercises all three client outcomes |
| [P3] Revision saturation allowed mutation without revision progress | At `u64::MAX` the brush loop's `saturating_add` left the revision flat while occupancy still mutated and duplicate maximum-revision events were staged, which clients ignore forever | Implement now | `checked_add` before touching scratch occupancy: an exhausted revision space rejects the brush and records a new `RejectedRevisionExhausted` outcome under the rejected aggregate. `revision_exhaustion_rejects_brushes_without_mutation` fabricates a `MAX`-revision root and pins unchanged occupancy, unchanged revision, no staged event, and the rejection record |
| Hygiene: 11 `unused_qualifications` warnings in the network/performance targets; one adverse-UDP maximum-map timeout on the first network run | The role Clippy gates cover only client/server feature sets, and the impaired-UDP snapshot test gave the Adverse profile the same 3,600-tick bound as Typical | Fix now; gate policy deferred | All 11 qualifications removed (verified zero under `--features network-test --tests`); the Adverse profile now runs to 7,200 ticks while Typical keeps the sensitive 3,600 bound. A full `-D warnings` gate for the network-test configuration is deferred to the backlog (~30 further pre-existing cast/line-count findings across test files need a test-lint policy decision first) |

Round-2 remediation verification (2026-08-17): `fmt-check`, `clippy-client`, `clippy-server` (`-D
warnings`), `check`, `server-features`, `test-client` (183), `test-server` (178), `test-network`
(73), `test-performance` (14; m10 worst case 4.81 ms p95 on the 24-seam-brush one-tick burst),
`git diff --check`, `network-terrain` (assertion passed at tick 580, revision 1, 546/576 cells,
both clients converged, exit 0), and `network-terrain-hot-zone` (assertion passed at tick 576,
revision 1, 539/576 cells, exit 0) all green.

Cell size, chunk size, collider family, or recovery-contract changes return the milestone to
`Specification review`. Radius/color/debris/layer tuning that preserves validated bounds may remain
in feedback review with affected verification rerun.

## Feedback review, round 3 (2026-08-17)

A third review round examined the round-2 remediation itself and found both findings in code that
remediation introduced or left unwired. Both were verified against the live source, accepted, and
implemented now as one follow-up fix commit; neither changes the validated
cell/chunk/collider/recovery contracts or the wire protocol, so the milestone stays in feedback
review.

| Feedback | Evidence | Decision | Follow-up verification |
|---|---|---|---|
| [P2] Terrain telemetry was not actually match-scoped | Telemetry is documented as match-scoped and the specification requires restart to clear the telemetry epoch state and teardown to remove every prior telemetry state by exact old map, but the restart reset appended its `Reset` record to the surviving records/aggregates/dirty sets/maxima, map teardown and install reset the seven shared generation resources while leaving `TerrainTelemetry` intact, and the client's convergence telemetry survived every generation switch and disconnect | Implement now | The server clears telemetry at every generation boundary: the restart reset clears it immediately before its collision commit so the restoration rebuilds and the `Reset` record become the new epoch's first facts, and teardown/install reset it beside the other fresh-generation resources. The client clears it exactly when the convergence machine discards a generation — a derived map/match change at the wire system's start, an applied reset immediately after its commit, disconnect via the cleared phase. `restart_starts_a_fresh_telemetry_epoch_for_the_next_generation` pins one surviving `Reset` record with zeroed applied/requested/event/recovery counters whose rebuild count equals the fresh aggregate, `map_replacement_clears_the_previous_generation_telemetry` pins an exactly default resource after replacement, and `client_telemetry_clears_exactly_once_per_generation_change` covers adopt, clear, and re-record |
| [P2] Per-event collider attribution overcounted in multi-brush batches | The round-2 attribution credited every Applied record with each committed-union chunk equal or orthogonal-adjacent to the brush's affected chunks, ignoring the boundary-mask rule that actually forces neighbor rebuilds; an interior-only brush therefore inherited collider rebuilds that only another brush's boundary change caused | Implement now | The brush loop snapshots the seeded scratch bits before each brush applies, derives that brush's own changed masks (monotone erasure within a tick never cancels), and carries the per-brush dirty union — computed with the same `compute_dirty_union` boundary rule as the batch — on the staged record; the collision commit intersects it with the union it actually rebuilt. `multi_brush_batches_credit_each_brush_only_its_own_collider_dirt` batches an interior brush with the corner seam brush and pins one credited rebuild for the interior brush while the boundary brush owns the full union; the single-brush attribution test passes unchanged |

Round-3 remediation verification (2026-08-17): `fmt-check`, `clippy-client`, `clippy-server`
(`-D warnings`), `check`, `server-features`, `test-client` (184), `test-server` (181),
`test-network` (73), `test-performance` (14; m10 worst case 4.34 ms p95 on the 24-seam-brush
one-tick burst), `git diff --check`, `network-terrain` (assertion passed at tick 600, revision 1,
537/576 cells, both clients converged, exit 0), and `network-terrain-hot-zone` (assertion passed
at tick 560, revision 1, 540/576 cells, exit 0) all green.

Cell size, chunk size, collider family, or recovery-contract changes return the milestone to
`Specification review`. Radius/color/debris/layer tuning that preserves validated bounds may remain
in feedback review with affected verification rerun.

## User playtest feedback (2026-08-17)

The first windowed M10 playtest (two Arc Launcher clients on the built-in Crossroads practice map)
returned three observations. Two are fixed in the playtest remediation commit; the third and half of
the second are intentionally deferred under a product decision the playtest settled: terrain
destruction is not a normal-weapon property going forward — it is reserved for ultimate moves and
special items (for example a thrown bomb), with the Arc Launcher's `DestroyTerrain` acting as the
M10 test vehicle until that redesign. The reserved follow-ups are recorded as
`GAP-DESIGN-TERRAIN-RESERVATION` in the v1 backlog.

| Feedback | Evidence | Decision | Follow-up verification |
|---|---|---|---|
| Pulse shots passed through the destructible block and hit a player behind it | The projectile sweep's spatial-filter mask already included both terrain layers, but its acceptance predicate only passed hostile fighters and `ArenaWall` entities, so `cast_shape_predicate` ignored terrain chunk colliders entirely — every composed straight shot (pulse, scatter) crossed cover while area payloads simultaneously honored `terrain_occlusion` | Implement now | The predicate accepts destructible chunk colliders alongside arena walls; a terrain impact is an ordinary targetless `StraightImpact` (projectile despawns at the face, world effects fire at the impact point). Three network duels that unknowingly fired through the central block (reciprocal lethal hits, posthumous attribution, spawn protection) moved to the clear corridor at y=160 between the block and the north wall; `straight_shots_stop_at_destructible_cover_until_it_is_carved` pins the full loop — the shot dies on the face with the target at full health and zero applied damage, then crosses only after three delivered brushes carve the lane |
| The lob aimed at the block center detonated on the block's face, and the aim marker promised the center | The server's landing repair treats destructible occupancy as unclear for landing (`DESTRUCTIBLE_TERRAIN_LAYER` is in the clearance filter), so center-aimed lobs snap to the last clear point on the near face, while the client preview checked only permanent map geometry and showed the unrepaired point | Preview parity now; landing behavior deferred | The preview repairs against the committed destructible occupancy exactly like the server's collider clearance (`circle_overlaps_occupied` twin in `terrain/grid.rs`), so the marker tells the truth; `launcher_preview_repairs_landings_against_committed_terrain` pins both the pull-back distance and the repaired color. Changing where lobs may land — destructible cells as legal landings for `DestroyTerrain`-carrying recipes — is deferred to the ultimate/item redesign because the current carrier weapon is provisional |
| The radius-150 damage area and the radius-48 crater read as inconsistent | Authored Arc Launcher values: payload `Area(radius: 150)` versus `DestroyTerrain(radius: 48)`; the damage disc is ~10x the crater area, and the explosion feedback does not distinguish the high-energy core from the outer ring | Deferred with the product decision | Crater-size tuning (the 64-unit brush ceiling is the representable maximum) and distinct crater-edge feedback fold into `GAP-DESIGN-TERRAIN-RESERVATION`, decided together with the ultimate/item carrier rather than balanced on a provisional weapon. Melee arcs still ignore terrain — unreachable with the 192-unit block because melee reach is 120 — and joins the same backlog item |

Playtest remediation verification (2026-08-17): `fmt-check`, `clippy-client`, `clippy-server`
(`-D warnings`), `check`, `server-features`, `test-client` (185), `test-server` (181),
`test-network` (74, including the new cover scenario), `test-performance` (14; m10 worst case
5.14 ms p95 on the 24-seam-brush one-tick burst), `git diff --check`, `network-terrain`
(assertion passed at tick 635, revision 1, 540/576 cells, both clients converged, exit 0), and
`network-terrain-hot-zone` (assertion passed at tick 633, revision 1, 541/576 cells, exit 0) all
green.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Voxel colliders snag at separate Brawler chunk seams | Combine Parry neighbor states across orthogonal shapes; rebuild boundary neighbors; require slide/projectile seam tests before combat integration. |
| Pairwise reconciliation mutates installed/shared voxel shapes | Build fresh prospective `Voxels` for the whole dirty union, reconcile each pair once, wrap new `SharedShape`/`Collider` values, and install only after the batch succeeds. |
| Changed collider is not visible to pre-physics movement query at the intended tick | Exact schedule/App test against Avian 0.7; because erasure only shrinks geometry, retain conservative old AABB until prepare; move terrain chain only through specification review if behavior differs. |
| Maximum map is inferred from aligned demo coordinates | Global Euclidean coordinates, 221 off-grid chunk ceiling, min/max/off-grid fixtures, and process reports containing dimensions/counts. |
| Full recovery blocks smaller session/combat messages | Dedicated terrain channel, 48 KiB bound, per-link cooldown, cached typed snapshots, and impaired fragmentation tests. |
| Map snapshot and terrain snapshot arrive non-atomically | Separate terrain readiness/generation/fingerprint gate; never combine mismatched map/match state. |
| Arc world effect runs once per area target | Delivery-level world-effect list/fact, explicit zero/many-target tests, and no terrain code in recipient payload loop. |
| Terrain reset races match restart | Add common `EnvironmentReset` inside the chained restart transaction before commit; schedule trace and stale-generation tests. |
| Quantized collider extends into permanent terrain or outside bounds | Validate every selected cell AABB against playable bounds/permanent shapes before recipe acceptance. |
| Multiple authored regions overlap cells | Reject duplicate selected cells deterministically during initial rasterization. |
| Hot Zone starts effectively inaccessible | Validate legal reachable fighter-center positions for both teams; keep overlap as a playtest decision and resize/move during feedback if needed. |
| Dirty work exceeds fixed-tick budget | Conditional neighbor dirtiness, whole-brush prospective admission, 24-brush/221-distinct-rebuild ceilings, 24-active-fighter worst-placement evidence, telemetry, and profiling before representation changes. |
| Client presentation accidentally becomes gameplay truth | No client terrain colliders, no image types in wire/server graph, server-only mutation, forged-client tests. |
| Empty chunks or repeated resets leak entities/assets | Stable chunk entities, exact-generation cleanup, image handle ownership, and 100-cycle growth tests. |

## Exit criteria

- [x] User has validated this specification and every accepted change is recorded.
- [x] Server-authoritative 8-unit occupancy, 32-cell sparse chunks, integer brushes, Avian voxel
  collision, and exact restart reset are implemented without client authority or visible-tile truth.
- [x] Arc Launcher emits exactly one radius-48 world brush per committed landed delivery; existing
  combat effects and other weapons retain their specified behavior.
- [x] Minimum, maximum, arbitrarily offset, multi-region, near-maximum-cell, and chunk-crossing maps
  satisfy declared storage/collider/recovery budgets; the 192×192 map is not the scale proof.
- [x] Terrain consumes resolved map/mode capacity rather than assuming two teams or 2v2, and synthetic
  varied-team plus 24-active-fighter fixtures meet the no-deferral and fixed-tick gates.
- [x] Fighters, projectiles, dashes, sentries, placement, and line of sight agree with occupied/erased
  cells, including seams and next-tick activation; permanent/unrelated state is unchanged.
- [x] Two current clients, an impaired client, and a newly accepted recovery client converge to the
  authoritative generation/revision/digest without historical event dependence.
- [x] Duplicate/missing/out-of-order/stale/corrupt/oversized messages and forged recovery requests
  have safe tested outcomes.
- [x] Client readiness, visuals, crater edges, debris, reset, Hot Zone overlap, controller, and
  keyboard/mouse checks have explicit automated/process/human outcomes.
- [x] Fixed-tick, client update, event/recovery byte, entity/image growth, and repeated-reset gates
  meet the declared ceilings without weakening existing performance tests.
- [x] Defensive repair is deterministic in its injected test and never fires in a valid scenario.
- [x] Both role feature graphs, formatting, Clippy `-D warnings`, all unit/network/performance suites,
  server isolation, real-process profiles, and `git diff --check` are green with recorded evidence.
- [x] Every Gameplay MVP acceptance criterion is linked to repeatable evidence or intentionally
  revised in the product document.
- [x] User playtest feedback is triaged, affected verification rerun, and learn-from-errors review is
  complete before marking Milestone 10 `Complete`. The 2026-08-17 windowed playtest's three findings
  are triaged in the user-playtest section (one fixed, one preview-parity fix with the behavior
  deferred, one deferred to `GAP-DESIGN-TERRAIN-RESERVATION`), and the learning review below
  completed closeout the same day.

## Learn-from-errors review

Complete during feedback review:

| Mistake or surprise | Cause | Prevention/change | Reusable project lesson |
|---|---|---|---|
| Straight projectiles flew through the destructible block | The terrain layers were added to the sweep's spatial-filter mask but never to the acceptance predicate two gates later; mask-only integration compiles, sweeps, and passes every existing test because nothing asserted cover blocks | Integrations are gated by several structures in sequence (mask, predicate, documented contract, lifecycle boundary). Enumerate every gate a new entity family must pass and add a discriminating negative test — the blocker must block — not only positive-path outcomes | A wiring claim is only true at its last gate; a half-wired integration is indistinguishable from a working one until something asserts the negative |
| Three network duels (reciprocal hits, posthumous attribution, spawn protection) silently fired through the central block and passed | Fixtures placed at `(±140, 0)` before the map had terrain; when the built-in map gained its central destructible block nobody revalidated scenario placement against the new geometry | When a map or acquisition adds central geometry, audit existing fixture placements against the resolved map (terrain regions, permanent geometry, probes) instead of only adding new tests | Tests authored against an older map encode the old map's physics; a scenario passing through a fixture it should interact with is a smell, not a success |
| The round-2 "wired" per-event collider attribution was itself the round-3 defect (adjacency credited rebuilds only boundary changes force) | The fix implemented the reviewer's requirement with a new local adjacency approximation instead of reusing the canonical boundary-mask rule (`compute_dirty_union`) that already governed the batch union | When a fix must encode "the same rule as X", reuse X's function rather than approximating it, and close with a test that discriminates the real rule from the plausible one (here: one interior brush beside one boundary brush in a single batch) | Approximations of an existing domain rule inherit none of its correctness; reuse the rule or change it everywhere |
| Round 1 found exit evidence overstated: the 221-chunk workload was checked off while the fixture allocated 176 chunks, and the varied-team capacity fixture did not exist | Checklist items were not tied to the fixture and measurements that produced them | Every checked exit criterion now cites its producing test/fixture and its measured numbers; the remediation rebuilt the fixtures to the true ceilings and re-recorded evidence | An exit criterion without a named producer is an intention, not evidence |
| Fixture geometry needed repeated empirical nudging: maximum-map spawns violated facing/probe-inset constraints several times, and the playtest remediation first moved duels to y=240 — inside the north wall at y∈[224, 288] | Coordinates chosen as plausible round numbers without checking them against authored geometry | Derive fixture coordinates from the resolved map (permanent geometry, terrain regions, spawn/probe constraints) and assert placement validity in the test setup so a bad spot fails at setup, not mid-scenario | Place fixtures by derivation from the map, not by eyeball; the map already knows where walls are |

No project or Codex skill was created: every recorded lesson is at its first recorded occurrence and
is captured here (and in the review sections above) for recurrence checking at the next milestone's
learning review; none yet generalizes beyond Brawler's own workflow.
