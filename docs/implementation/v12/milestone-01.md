# V12 Milestone 01 — proper 3v3 maps

## Status

`User playtest`

The user selected the map-first sequence, supplied one image reference for each supported mode,
directed manual conversion into existing Brawler assets, and approved server-configured map limits
of 20 through 512 cells per axis on 2026-08-26.

## Player-visible outcome

The advertised 3v3 Wipeout, Hot Zone, and Heist paths each use one deliberate mode-owned map rather
than the shared Feature Yard integration geometry. The maps preserve the recognizable lane, cover,
concealment, spawn, and objective structure of the supplied references while using Brawler's
authoritative assets, collision profiles, mode anchors, presentation, and original product naming.

## Reference inputs

- Wipeout: `external_assets/map_images/bounty/think_ahead.png`
- Hot Zone: `external_assets/map_images/hot-zone/back_shuffle.webp`
- Heist: `external_assets/map_images/heist/hot-potato.png`

The references are reviewed manually. No importer output or runtime image path enters production.

## Validated viewport and padding contract

The three references each encode a 23×35 logical grid. Production adds one symmetric empty cell on
every side, yielding a 25×37 recipe without scaling or rotating the supplied layout. Image rows are
flipped into Brawler's positive-Y map coordinates and translated by the one-cell inset.

The user identified the original mobile combat framing as approximately 14 visible tiles vertically.
At the existing 27-degree vertical field of view and 55-degree elevation, a camera distance of 743
world units yields approximately 23.82×14.00 visible cells at 16:9. This nearly fills the padded map
width while retaining vertical follow, so wider authoritative padding must not be used to compensate
for presentation framing.

## Validated dimension contract

`config/server/game-types.ron` owns one required `map_dimension_limits` record. The checked-in
operator policy is:

```text
minimum_width  = 20
minimum_height = 20
maximum_width  = 512
maximum_height = 512
```

Shared parsing retains a positive hard ceiling of 512 cells on either axis. Server startup rejects
invalid or inverted limits, a configured maximum above the engine ceiling, and every advertised
map outside the configured envelope. Spawn, reachability, mode-anchor, placement, concealment,
damageable-object, recipe-byte, snapshot-byte, collider, recovery, and bot-work bounds remain
independent gates.

The user rejected a style-specific concealment ceiling on 2026-08-26 and approved rounding the
maximum dimension to the power-of-two 512. Placement capacity now follows the four per-cell asset
slots, concealment may cover every cell, and the bounded resolved snapshot ceiling is 32 MiB. Thus
a 512×512 recipe may contain 262,144 concealing cells; rendering and lookup optimization remain
evidence-driven work rather than an authoring restriction.

## Validated objective-anchor precision

The padded 25×37 Hot Zone reference has its objective at the exact center of the middle cell and a
radius of approximately 3.5 cells. Recipe schema 5 therefore represents Hot Zone centers and radii
as unsigned integer half-cell units. `(2x + 1, 2y + 1)` is the center of cell `(x, y)`, so Back
Shuffle uses center `(25, 37)` and radius `7`; resolution converts those values to authoritative
world coordinates without floating-point authoring ambiguity.

This precision is reusable for geometric mode anchors, where sub-cell geometry has a demonstrated
need. It is deliberately not generalized to walls, grass, cover, spawns, safes, decorations, or
filled rectangles: those assets retain integer-cell placement, footprint, collision, canonical
ordering, and overlap rules. The existing Feature Yard Hot Zone recipe is migrated losslessly from
center `(32, 20)`, radius `5` cells to half-cell values `(64, 40)`, radius `10`.

Because `ResolvedMapSnapshot` replicates the anchor enum and the same three integers now carry
half-cell semantics, this is an incompatible meaning change even though its binary width is
unchanged. The one global application protocol therefore advances from 28 to 29; recipe schema and
canonical fingerprint formats advance from 4/7 to 5/8. There is no legacy decoder.

## Map-authoring contract

- Manually transcribe the references into ordinary sparse `MapRecipe` RON documents.
- Use existing sand/ground, garden/arena wall, tall-grass, water, destructible-cover, barrier,
  spawn-marker, decoration, damageable-object, and objective-anchor capabilities only where the
  reference has a clear gameplay equivalent.
- Omit reference-mode objects with no Wipeout meaning rather than inventing rules.
- Record any visually distinct object that lacks a Brawler equivalent before adding or
  approximating it.
- Preserve exactly three safe spawns per team, mode compatibility, fighter-clearance navigation,
  and Heist safe attack sectors.
- Update the map index and only the 3v3 advertised game types; 1v1/2v2 remain unchanged unless the
  user later expands scope.

## Implemented map conversion

- Wipeout advertises **Verdant Crossfire** (`MapPresetId(10)`), derived from Think Ahead.
- Hot Zone advertises **Switchback Basin** (`MapPresetId(11)`), derived from Back Shuffle, with the
  exact half-cell objective center `(25, 37)` and radius `7`.
- Heist advertises **Powderline Vault** (`MapPresetId(12)`), derived from Hot Potato, with mirrored
  three-by-two safe anchors centered at world Y `-400` and `400`.

Each recipe uses one empty-cell perimeter around the transcribed 23×35 reference. Existing garden
walls carry the collision role of themed blocks, metal barriers, and rope fences; existing
destructible cover carries the orange/green box role; and tall grass preserves every reviewed
concealment group. Powderline Vault's two blocking cacti use a dedicated destructible one-cell
asset with the Kenney Graveyard Kit `trunk.glb` visual, green tint, and the same removal behavior as
destructible cover. The Wipeout reference's central Bounty star is omitted because Wipeout has no
corresponding pickup/scoring rule. Exact rope-fence, skull-block, and themed-barrel models are the
remaining visual asset gaps; no missing gameplay primitive blocks the playtest.

## Implementation and verification

- [x] Move map minimum/maximum dimensions into validated server operator configuration.
- [x] Set the checked-in policy to 20×20 through 512×512 and extend hard shared safety to 512×512.
- [x] Keep Practice navigation representable at the configured maximum and add focused limit tests.
- [x] Set and test the accepted 14-cell vertical gameplay-camera target.
- [x] Replace fixed placement/concealment counts with 512×512 dimension-derived structural bounds.
- [x] Add bounded half-cell precision for Hot Zone centers and radii, migrate schema 5 recipes, and
  retain cell alignment for ordinary placements.
- [x] Author, index, resolve, and advertise the Wipeout 3v3 map.
- [x] Author, index, resolve, and advertise the Hot Zone 3v3 map with one typed capture anchor.
- [x] Author, index, resolve, and advertise the Heist 3v3 map with two typed team safes.
- [ ] Run formatting, role checks, focused catalog/navigation/admission tests, canonical tests,
  routed 3v3 and Practice evidence, and native rendering/playtest.
- [ ] Triage gameplay feedback, rerun affected verification, reconcile durable documentation, and
  complete the learning review before closeout.

### Dimension-policy verification evidence — 2026-08-26

- `cargo fmt --all -- --check` passed.
- Server, client, network-test, and Balance Lab role checks passed.
- Server and client Clippy passed with warnings denied.
- The focused dimension-policy, operator-catalog, and 512×512 Practice-navigation tests passed.
- The complete server library suite passed serially: 319 passed, 0 failed. Its earlier parallel run
  exposed the existing global diagnostics/logger test interaction in
  `exit_frame_report_observes_terminal_counts_after_the_shutdown_chain`; that test passed both in
  isolation and in the serial suite.
- The server feature-isolation and V8 canonical-map-cleanup scripts passed.
- `git diff --check` passed.

The serial 319-test server suite above preceded the later power-of-two/capacity revision. After that
revision, the focused 512×512 dimension-policy, invalid-envelope, 1,048,576-slot capacity,
262,144-cell concealment, operator-catalog, and maximum-size Practice-navigation tests passed.
Server and client all-target role checks, formatting, and `git diff --check` also passed. The
14-cell camera-footprint test passed separately under the client role. Full closeout verification
remains pending until the three recipes are authored.

### Objective-anchor precision verification — 2026-08-26

- All 24 map-catalog unit tests passed under the client role, including exact odd-grid half-cell
  conversion, a `(25, 37)` center with radius `7`, lossless Feature Yard migration, embedded recipe
  parsing, canonical fingerprinting, and invalid-anchor rejection.
- All four Hot Zone-filtered authoritative, HUD, and 3D-presentation tests passed.
- All 14 protocol unit tests passed under the combined network-test role after advancing the global
  compatibility version to 29.
- Server and client all-target role checks, formatting, and `git diff --check` passed.

### Three-map implementation verification — 2026-08-26

- All 25 map-catalog unit tests passed under the client role. The new exact-topology test resolves
  all three 25×37 maps, checks three spawns per team, requires concealment, rejects mode-anchor
  drift, and proves the mirrored Heist safe centers at world Y `-400` and `400`.
- All 12 server admission tests passed, including exact preset/revision admission for the three new
  maps and real-schedule objective-bot coverage. The four operator-catalog tests passed with the
  revised golden advertisement and revision digest.
- The focused client 3D test passed for Switchback Basin's exact `3.5`-cell visual radius. The
  focused lobby Practice test proves Hot Zone 3v3 allocates Switchback Basin and five named bots.
- Server, client, and combined network-test all-target checks passed. Server and client Clippy
  passed with warnings denied. Formatting, canonical V8 map-cleanup, and `git diff --check` passed.
- Production-routed headless Practice reached Active with one human and five bots for
  `wipeout-3v3`, `hot-zone-3v3`, and `heist-3v3`; each supervisor/lobby/match-worker process tree
  shut down cleanly.
- Native visual/gameplay acceptance remains the active user-playtest gate.

### Feedback review — cactus visual variety — 2026-08-26

Disposition: **implemented now**. The user clarified that each cactus is a destructible wall used
for visual variety and selected Kenney Graveyard Kit `trunk.glb` as an acceptable source. The shared
catalog now owns stable cactus asset `28`, which is a one-cell blocking feature on sand and reuses
the exact destructible-cover gameplay profile. Client visual profile `43` uses the promoted trunk
model with a green tint and contained one-cell fitting. Powderline Vault recipe revision `2`
replaces the two former garden-wall approximations at cells `(9, 20)` and `(15, 17)`; its admission
revision advances to `2`, and the Heist 3v3 game-type revision advances to `5`.

Focused catalog tests prove both exact cells and gameplay-profile equality with destructible cover.
Client tests prove visual-catalog resolution, promoted-file presence, manifest provenance, and GLB
dependency coverage. Server/client all-target checks, exact manifest admission, the revised
operator-catalog golden, and production-routed Heist 3v3 Practice all passed. Native appearance and
scale remain part of the active user playtest.

## Exit criteria

- all three maps resolve through the ordinary server-owned catalog and exact 3v3 admission path;
- their collision, concealment, spawn, objective, restart, recovery, and bot-navigation behavior
  is valid and readable;
- the configured dimension envelope fails closed and does not become a client-authored rule;
- the user can play each map and every feedback item receives an explicit disposition; and
- M02 remains unstarted and unspecified until the user defines the balancing-tool changes.
