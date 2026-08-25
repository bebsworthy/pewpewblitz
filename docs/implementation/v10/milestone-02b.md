# V10 Milestone 02b — Feature Yard map-family consolidation

## Status

`Complete`

The user accepted the consolidation direction on 2026-08-25 while M02 entered feedback review. On
2026-08-25, the user accepted the corrected M02 playtest, approved this prepared specification, and
explicitly started M02b production implementation.

## Player-visible outcome

The Dashboard advertises one **Feature Yard** test-map family instead of a collection of maps that
each demonstrate one mechanic. Feature Yard is available for Wipeout, Hot Zone, and Heist in exact
symmetric 1v1, 2v2, and 3v3 game types. Every variant visibly uses the same arena geometry and
contains representative permanent cover, water, concealing tall grass, destructible barriers,
damageable oil barrels, movement lanes, and safe spawns. Hot Zone adds its capture area; Heist adds
its two team safes; Wipeout adds no inactive objective.

This is deliberately an integration/test map. M02b does not claim that it is fun, competitively
balanced, release-ready, or the eventual visual identity of the product. Proper player-facing maps
remain later content with their own design and playtest goals.

## Scope decisions

### Specified for M02b

- three stable Feature Yard recipes for Wipeout, Hot Zone, and Heist;
- identical normalized geometry across those recipes, with only identity, compatible mode, and
  required typed anchors allowed to differ;
- exact symmetric 1v1, 2v2, and 3v3 advertised game types for each supported mode;
- one representative placement of every completed map-owned gameplay capability that can legally
  coexist: permanent blockers, player-blocking/projectile-passable water, concealing tall grass,
  destructible cover that removes or becomes rubble, breakable barriers, and oil barrels;
- safe spawn, objective approach, attack/defence sector, reachability, placement-capacity, collider,
  concealment, and terminal-state validation at the maximum 3v3 topology;
- migration of product configuration, automation, tests, fixtures, diagnostics, documentation, and
  native evidence away from obsolete focused maps;
- removal of obsolete recipes after equivalent coverage passes, with a documented exception only
  for a non-advertised recipe that still proves a unique contract; and
- a clear handoff that makes Feature Yard the placement target for M03 chests and pickups.

### Explicitly not in M02b

- treasure chests, pickups, healing, inventory, or other M03 behavior;
- new map gameplay primitives, game modes, topology models, or asymmetric teams;
- a map-template language, inheritance/include mechanism, procedural generation, second resolver,
  runtime mode override, Bevy scene migration, or player-facing map editor;
- Dashboard grouping/redesign beyond truthful names and the nine-entry advertised catalog;
- tuning Feature Yard into a fun or competitively balanced map; or
- authoring a second player-facing arena.

## Research record

### Local product and architecture sources

Research inspected:

- `docs/00-product-direction.md` for combat-first, content-through-composition, creator, and 3v3
  direction;
- `docs/04-maps-and-game-modes.md` for the rule that each recipe targets one mode while a visual
  arrangement may be reused across independently validated recipes;
- `docs/16-grid-map-asset-system.md` and `docs/17-concealment.md` for the one sparse recipe/resolver
  path, whole-placement destruction, water, tall grass, and observer-specific concealment;
- V8 M01–M04 and V9 M01–M03 evidence for the unique contracts currently owned by Crossroads
  Facility, Ashen Court, Tidal Garden, and Crossroads Hot Zone;
- V10 M01/M02 evidence for Barrel Yard oil barrels and Twin Vaults Heist-safe access;
- all six `content/maps/builtin/*.ron` recipes, `content/maps/index.ron`, the ten-entry
  `config/server/game-types.ron`, and their exact preset/game-type consumers;
- `src/map/catalog.rs` for build-embedded source discovery, schema validation, canonical expansion,
  fingerprints, concealment ceilings, anchor validation, and current per-map regressions;
- `src/server/lobby/catalog.rs`, `src/lobby.rs`, queue/admission/worker code, render automation, the
  network harness, and performance fixtures for product-visible identity and topology ownership;
  and
- the checked-in Bevy `references/bevy/examples/README.md` and plugin examples to confirm that this
  content-only migration needs no new ECS schedule or plugin boundary.

### Current primary source

- [Bevy 0.19 release notes](https://bevy.org/news/bevy-0-19/) confirm the exact engine baseline and
  note that 0.19 does not ship a first-party `.bsn` asset loader. M02b therefore keeps Brawler's
  established headless-safe RON recipe path instead of adopting a scene format for map sharing.

No current Bevy, Lightyear, or Avian API is required. Consolidation changes authored content and
catalog consumers; authoritative ECS/runtime ownership remains unchanged.

### Current inventory and unique coverage

| Current recipe | Advertised use | Distinct evidence to migrate or retain |
|---|---|---|
| Crossroads Facility | Wipeout 2v2 | permanent lanes and remove-to-empty destructible cover |
| Crossroads Facility Hot Zone | Hot Zone 1v1/2v2/3v3 | central Hot Zone plus terrain concealment |
| Ashen Court | First Blood 1v1 | alternate theme, circular obstacle profiles, decorations, primitive/imported presentation |
| Tidal Garden | Wipeout 2v2 | irregular water, concealing vegetation, breakable barriers/rubble, multi-cell features |
| Barrel Yard | Wipeout 1v1 | damageable oil barrels, health, explosion chains, terminal debris |
| Twin Vaults | Heist 1v1/2v2/3v3 | mirrored safe anchors and legal attack/defence sectors |

Feature Yard can absorb the gameplay contracts currently proved by Crossroads, Crossroads Hot Zone,
Tidal Garden, Barrel Yard, and Twin Vaults. Ashen Court's alternate theme and rounded imported
obstacles are presentation/catalog coverage rather than another gameplay mechanic. It may remain
temporarily as a non-advertised regression recipe until equivalent focused catalog/native evidence
exists; that exception does not make it a product choice.

### Alternatives considered

1. **Put every anchor in one recipe and activate it by mode.** Rejected. It weakens the current
   mode-specific validation contract, makes inactive objectives ambiguous, and lets a recipe claim
   compatibility it has not independently proved.
2. **Add a shared layout/template/include schema.** Rejected for M02b. There is one concrete family
   of three recipes, while built-in and future authored maps currently share one complete recipe
   representation. Three ordinary documents plus a normalized-equivalence regression are smaller
   and preserve the creator contract. A shared source abstraction can be reconsidered only after
   another real family demonstrates recurring authoring cost.
3. **Keep every map advertised and merely add Feature Yard.** Rejected because it increases the
   selection noise that motivated this milestone.
4. **Delete every old recipe immediately.** Rejected. Coverage must move first, and Ashen Court may
   temporarily remain hidden for its distinct presentation/theme regression.
5. **Put consolidation inside M03.** Rejected. Catalog identity, nine routed game types, map
   retirement, and broad regression migration are an independently reviewable content slice. M03
   remains the chest/pickup behavior and V10 closeout milestone.

## Technical specification

### Recipe family and identities

Add three new, never-reused preset/recipe identities:

| Recipe | Preset / recipe ID | Revision / admission | Mode | Anchors |
|---|---|---|---|---|
| Feature Yard Wipeout | `MapPresetId(7)` / `MapRecipeId(7)` | `1` / `1` | Wipeout | none |
| Feature Yard Hot Zone | `MapPresetId(8)` / `MapRecipeId(8)` | `1` / `1` | Hot Zone | one central `HotZoneCircle` |
| Feature Yard Heist | `MapPresetId(9)` / `MapRecipeId(9)` | `1` / `1` | Heist | exactly one `HeistSafe` for team 0 and team 1 |

Existing preset/recipe IDs remain retired or retained; none is reassigned to Feature Yard. Map
catalog schema `5`, recipe schema `4`, fingerprint format `6`, operator-catalog schema `2`, and
application protocol `27` do not advance because their shapes and hash formats do not change. The
global content envelope advances once from `14` to `15` because the accepted embedded content set
changes. The application protocol advances only if implementation discovers an actual wire-shape
change and returns that variance for specification review.

Each recipe is a complete `MapRecipe`. Thinness is a validated semantic relationship, not a new
source format. A focused projection helper used by catalog tests compares dimensions, theme,
default surface, placements, filled rectangles, and spawn markers across the three recipes. The
test fails if geometry drifts. It deliberately excludes recipe identity/revision,
`mode_definition_id`, and `mode_anchors`, then separately asserts the exact allowed anchor set.

### Geometry contract

Use one symmetric arena sized within the existing 32-unit grid bounds and expanded-placement
ceiling. It must provide:

- at least three readable approaches between teams plus cross-connections so one blocker or safe
  does not reduce the map to a single choke;
- at least three safe, clear, stable spawn markers per team, ordered consistently across variants;
- permanent player/projectile-blocking walls and representative cover;
- bounded irregular water cells that block fighters but pass projectiles without sealing an
  approach;
- mirrored tall-grass groups large enough to exercise entry, occupancy, proximity reveal, attack
  reveal, Reveal Scan, and concealment-field overlap near contested space;
- both remove-on-destruction cover and replace-with-rubble barriers, placed so every terminal
  combination preserves objective access and spawn escape;
- mirrored or equivalently fair oil-barrel groups with enough separation to exercise isolated and
  chained explosions without placing unavoidable damage at spawn;
- a central legal Hot Zone area with readable surrounding contest space; and
- two Heist-safe reservations with at least two legal attack sectors per safe and paths from every
  advertised spawn.

Feature representation is representative, not exhaustive: the yard need not contain every visual
profile, decoration, theme, quantity extreme, or weapon interaction combination. Synthetic maximum
fixtures continue to own capacity ceilings.

### Product catalog

Advertise exactly nine entries:

- `wipeout-1v1`, existing `wipeout-2v2`, and `wipeout-3v3` on Feature Yard Wipeout;
- existing `hot-zone-1v1`, `hot-zone-2v2`, and `hot-zone-3v3` on Feature Yard Hot Zone; and
- existing `heist-1v1`, `heist-2v2`, and `heist-3v3` on Feature Yard Heist.

Existing IDs are retained where their mode/topology meaning is unchanged and receive revised
configuration revisions: `wipeout-2v2`, every `hot-zone-*`, and every `heist-*` advance to revision
`2`. New `wipeout-1v1` and `wipeout-3v3` entries start at revision `1`. New Wipeout topology IDs use
the same bounded naming convention.
`first-blood`, `tidal-garden-2v2`, and `barrel-yard-1v1` leave the advertised catalog. The
`MAX_GAME_TYPES` ceiling remains ten; M02b does not tighten a wire bound merely because the current
catalog contains nine entries.

Display names identify mode and topology while making the common Feature Yard family clear. M02b
does not add a second map picker or group rows in the Dashboard.

### Retirement and regression migration

Implementation proceeds in this order:

1. add and validate the three Feature Yard recipes plus their geometry-equivalence test;
2. route all nine advertised game types to the matching variant and pass focused/routed evidence;
3. migrate behavioral assertions and automation away from each old preset identity;
4. remove Crossroads Wipeout/Hot Zone, Tidal Garden, Barrel Yard, and Twin Vaults from the embedded
   map index and source directory once their named contracts pass on Feature Yard;
5. retain Ashen Court only if its alternate-theme/rounded-obstacle regression still lacks an
   equivalent focused fixture, keep it absent from the operator catalog, and record the exact
   removal condition; otherwise remove it too; and
6. search production code, tests, scripts, commands, and current docs for stale preset constants,
   game-type IDs, display names, recipe paths, admission revisions, and golden catalog hashes.

Historical completed milestone documents remain unchanged except for explicit later supersession
notes. Current durable docs and commands teach Feature Yard as the product-visible test family.

### Runtime, network, and presentation ownership

No new ECS system, plugin, component, message, replication path, collider rule, concealment rule,
mode evaluator, or presentation renderer is introduced. Every variant follows:

```text
embedded complete MapRecipe
  -> existing catalog validation and canonical resolution
  -> existing routed admission/content fingerprint
  -> existing authoritative map/mode installation
  -> existing replication, recovery, and 3D presentation
```

Mode systems consume only their validated anchors. Wipeout never receives an objective entity, Hot
Zone never receives safes, and Heist never receives a capture zone. Neutral barrels and later M03
chests are ordinary map features and may appear in every compatible variant.

## Implementation checklist

- [x] Approve this specification and set M02b plus the roadmap to `Implementing`.
- [x] Author three complete Feature Yard recipes and exact normalized-equivalence/anchor tests.
- [x] Validate geometry, spawns, reachability, objectives, terminal placement combinations, and
  supported content/collider/concealment ceilings.
- [x] Replace the operator catalog with the nine exact mode/topology entries and update all routing,
  admission, queue, practice, diagnostics, automation, and golden fixtures.
- [x] Migrate focused map, destruction, concealment, barrel, Hot Zone, Heist, recovery, performance,
  and presentation regressions to Feature Yard.
- [x] Remove superseded recipes/constants after coverage passes; record any temporary hidden Ashen
  Court exception and its removal condition.
- [x] Reconcile current durable map/mode/content docs, README/just commands, and the M03 placement
  contract without rewriting historical evidence.
- [x] Run focused, canonical, routed, impairment, lifecycle, capacity, imported, primitive, and
  native verification; record exact evidence.
- [x] Deliver the Feature Yard playtest, triage feedback, rerun affected checks, and complete the
  learning review before closeout.

## Implementation progress

The first implementation tranche completed on 2026-08-25:

- added stable presets/recipes `7`, `8`, and `9` with one complete shared 64-by-40 geometry and only
  the legal Wipeout, Hot Zone, or Heist anchors differing;
- replaced the product catalog with the nine exact Feature Yard 1v1/2v2/3v3 entries and revised
  existing configuration identities as specified;
- changed direct-server defaults, lobby mode selection, supervisor allocation, admission checks,
  render automation, and primary map/concealment/barrel/Heist/performance fixtures to Feature Yard;
- migrated exact-once barrel chains, map destruction, rubble replacement, recovery readiness, and
  connected/late-join barrier convergence onto the shared family; and
- retired Crossroads Hot Zone and Twin Vaults after their objective, safe-access, and negative
  validation coverage moved to Feature Yard. Their stable IDs remain unused. Four older recipes
  remain embedded but absent from the product catalog as focused fixtures:

| Hidden fixture | Unique retained contract | Removal condition |
|---|---|---|
| Crossroads Facility | Exact V8 rectangle coalescing, 32-unit remove-to-empty conversion, and compact wire golden | Replace those historical conversion assertions with an equally focused synthetic fixture |
| Ashen Court | Alternate theme plus circular imported-obstacle quantization and primitive/imported presentation | Add equivalent focused theme/circular-profile presentation evidence independent of a built-in recipe |
| Tidal Garden | Dense mirrored irregular water/grass, rotated two-cell barriers, and near-ceiling wire payload | Add a synthetic density/mirroring/multi-cell capacity fixture with the same bounds |
| Barrel Yard | A permanent wall immediately adjacent to a damageable barrel for the accepted occlusion/readability regression | Add that adjacency to a focused synthetic occlusion fixture |

First-tranche evidence: the normalized geometry/mode-anchor regression passed, the focused six-test
map-runtime suite passed, the separate-App barrier late-join convergence test passed, and the full
server-feature library suite passed `527 passed; 0 failed`.

The verification tranche completed the following on 2026-08-25:

- `just lint` passed formatting, every Clippy role, Balance Lab web compilation, server feature
  isolation, sole-renderer enforcement, and V8 map-cleanup checks;
- the canonical role suites passed: routing `83 + 4 + 5 + 5 + 3`, client `387`, server `296`,
  Balance Lab `306`, combined Balance Lab/network `1`, network `86`, and performance `12` tests;
- the network suite includes impairment, late join, recovery, Feature Yard barrier/barrel
  convergence, all three mode rule sets, and 25-restart Hot Zone/Wipeout soaks;
- every advertised Wipeout, Hot Zone, and Heist 1v1/2v2/3v3 ID formed its exact two-, four-, or
  six-client routed roster, reached authoritative `Active`, and shut down cleanly; and
- native investigation found that the first 56-wall-cell draft exceeded the locked frame threshold.
  Reducing repeated wall, water, and grass cells while preserving every representative capability
  lowered primitive mesh high-water from `271` to `219`;
- controlled native imported and primitive reports passed at the actual reported `1280x720`
  resolution for all three modes: Wipeout reported `227`/`219` map meshes and `16.914`/`17.019` ms
  p95 frame time, Hot Zone reported `229`/`221` and `16.936`/`17.059` ms, and Heist reported
  `233`/`227` and `16.965`/`17.236` ms; and
- the native harness retains its canonical two-native-client default while allowing the measured
  primary client to use a paced live routed peer. This prevents desktop compositor contention from
  being mistaken for map-render cost and records which report was performance-validated.

## User playtest handoff

Run `just run 2`, then select and ready the same entry in both clients for these three short checks:

1. `Feature Yard Wipeout 1v1`;
2. `Feature Yard Hot Zone 1v1`; and
3. `Feature Yard Heist 1v1`.

Confirm that the Dashboard list scrolls and confirms reliably, the arena is recognizably the same
in all three matches, each match shows only its own mode objective, and walls, water, grass,
destructible barriers, barrels, the Hot Zone, and the two Heist idols remain readable and
reachable. Automated routed evidence covers the corresponding 2v2 and 3v3 rosters, so the manual
pass is about visual and functional coherence rather than repeating every topology or judging fun
and competitive balance.

## Feedback review

User acceptance and closeout on 2026-08-25: the Feature Yard family passed the requested Wipeout,
Hot Zone, and Heist playtest without a new correction. The user reported “all good” and explicitly
requested milestone closeout. No M02b feedback item remains open, deferred, rejected, or awaiting
evidence. All exit criteria are satisfied and M02b is `Complete`.

## Learn-from-errors review

1. **Catalog identity and ordering are behavior.** Early fixtures implicitly selected the first
   embedded recipe. Product tests now resolve semantic Feature Yard identities, while coordinate-
   specific generic fixtures explicitly name their retained hidden contract.
2. **A default-map migration exposes geometry-coupled tests.** Behavioral tests now derive authored
   Feature Yard cells where the map capability is under test. Tests that intentionally exercise
   historical conversion geometry remain attached to the documented hidden fixture instead of
   pretending to validate the product map.
3. **Repeated imported cells have visible native cost.** The first draft proved all features but
   repeated them excessively. The final map uses bounded representative groups and keeps synthetic
   fixtures responsible for density ceilings; native evidence is required before declaring a test
   yard suitably integrated.
4. **Desktop contention can contaminate render evidence.** Two full-rate Retina windows can measure
   compositor pressure rather than one client's map presentation. The harness still defaults to
   the canonical dual-client check, but controlled diagnosis can pace the live peer, validate only
   the primary report, and verify the physical resolution reported by the client.
5. **A content migration must verify the envelope constant, not only the resulting fingerprint.**
   The closeout audit found the specification and canonical catalog had advanced while
   `GAMEPLAY_CONTENT_ENVELOPE_VERSION` still read `14`. It was corrected to `15`; future catalog
   migrations must assert the expected envelope version explicitly alongside fingerprint goldens.

## Verification plan

### Focused content and authority

- parse and resolve all three recipes through `MapContentCatalog::embedded`;
- assert normalized geometry equality and exact mode-specific anchors;
- assert exact 1v1/2v2/3v3 spawn capacity, clearance, reachability, Hot Zone access, and Heist attack/
  defence sectors;
- cover water collision/projectile policy, terrain concealment membership/privacy, both map-
  destruction outcomes, barrel health/chains/debris, and all-terminal reachability;
- assert no system branches on a Feature Yard preset identity; and
- prove removed recipes/constants have no current production consumer or advertised catalog row.

### Routed and lifecycle

- run all nine exact advertised game types through practice and representative routed product E2E;
- run the canonical 1v1/2v2/3v3 matrix plus concurrent heterogeneous Wipeout/Hot Zone/Heist workers;
- cover restart, late join, reconnect, recovery, fresh-lobby requeue, map replacement, and shutdown;
- retain observer-specific concealment and public-object/objective convergence under impairment; and
- remeasure welcome, manifest, recovery, entity, collider, map bytes, bandwidth, fixed tick, and
  repeated-lifecycle bounds at 3v3.

### Native and user playtest

Run imported and forced-primitive evidence for one representative topology in each mode, then
playtest the family across Wipeout, Hot Zone, and Heist. Confirm:

1. geometry is recognizably the same arena across modes;
2. each mode shows only its relevant objective;
3. water, grass, permanent cover, destructible barriers, barrels, Hot Zone, and safes remain
   visually and behaviorally distinct;
4. all three spawn lanes and objectives are reachable at 1v1 through 3v3; and
5. the Dashboard reads as one test-map family rather than unrelated feature maps.

Feedback asks about functional readability and coverage, not fun or competitive balance.

## Exit criteria

M02b may enter `Complete` only when:

1. the user approves this specification before production implementation;
2. the three variants share validated normalized geometry and exact legal mode anchors;
3. the nine advertised exact topologies pass catalog, practice, routed, and lifecycle evidence;
4. current map-feature regressions pass on Feature Yard before obsolete recipes are removed;
5. every remaining non-advertised legacy recipe has one named unique contract and removal condition;
6. imported/primitive presentation and user playtest confirm one coherent functional test family;
7. affected canonical, concealment, destruction, barrel, Hot Zone, Heist, recovery, capacity, and
   performance checks pass;
8. M03 explicitly targets Feature Yard for chest/pickup placement; and
9. feedback triage, affected reruns, roadmap/durable-doc reconciliation, and the learn-from-errors
   review are complete.
