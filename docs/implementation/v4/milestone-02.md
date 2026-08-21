# V4 Milestone 02 — Scalable map documents and reusable object definitions

| Field | Value |
|---|---|
| Status | Complete |
| Depends on | V4 M01 complete |
| Outcome | Built-in maps load from independently validated documents through a deterministic index, while reusable semantic object definitions, theme defaults, and compatible visual overrides remain stable and server-safe |

## Research question

What is the smallest map-content storage and resolution change that allows hundreds of built-in
maps to be added without editing Rust source or a monolithic recipe array, while preserving stable
identities, deterministic fingerprints, routed admission, planar authority, and the M01 reusable
object/visual taxonomy?

## Current implementation findings

The runtime architecture is already close to the required boundary, but authored storage and a few
selection rules are not:

- `MapContentCatalog::embedded()` parses all shared definitions and both complete recipes from
  `content/v1/maps.ron`, then injects the separately embedded V4 object catalog.
- `MapContentCatalog::validate()` requires exactly two presets with IDs 1 and 2 in ascending order.
  Adding a third map therefore requires editing the monolith and Rust validation.
- `MapRecipe` repeats low-level collision, presentation, and entity profile IDs in every geometry or
  decoration placement even though M01 introduced semantic object definitions that can own those
  stable defaults.
- M01 object definitions, compatible visual variants, and themes currently share
  `content/v4/map_objects.ron`. They have different growth and validation ownership and the V4
  roadmap already calls for separate catalogs.
- the resolver correctly normalizes placement order, equivalent rotation, and signed zero before
  computing a recipe fingerprint. The global gameplay fingerprint includes canonical map, weapon,
  and build material and gates client/server compatibility.
- lobby operator configuration already refers to maps by stable key, validates them against the
  embedded catalog, and rotates reservations across a game type's advertised map IDs.
- the routed match manifest carries `map_preset` and `map_revision`, but match-worker admission and
  server map installation still derive one hard-coded map from `GameMode`. A second map for the same
  mode would be advertised and reserved by the lobby but rejected by the worker.
- clients need local map metadata to display map names, but authoritative recipes are resolved and
  installed only by the server. Clients receive `ResolvedMapSnapshot`; no client-authored geometry
  becomes authoritative.
- built-in content is deliberately compiled into both roles today. Runtime filesystem or
  `AssetServer` loading would introduce packaging, asynchronous readiness, and missing-file failure
  modes into server startup without helping M02's built-in-map goal.

## Alternatives considered

| Option | Result | Decision |
|---|---|---|
| Keep one `maps.ron` array | Simple but every map edits one conflict-prone file, validation remains order-coupled, and hundreds of recipes become difficult to review | Reject |
| Runtime filesystem discovery | Easy author iteration, but packaged clients/workers can disagree about available files and startup now depends on arbitrary disk state | Defer to custom-map work |
| Load recipes through Bevy `AssetServer` | Useful for render assets and hot reload, but map authority needs synchronous validated content in headless roles; it would also couple content admission to Bevy asset lifecycle | Reject for built-ins |
| Add `include_dir`/`rust-embed` | Provides recursive embedding but adds a dependency for a small, project-owned need | Reject unless the standard-library generator becomes costly |
| Hand-maintained Rust `include_str!` list | Keeps embedding deterministic but violates the no-Rust-change addition gate | Reject |
| Authored index plus a standard-library `build.rs` source table | Keeps exact embedded bytes, requires no new dependency, detects directory changes, and lets runtime validation compare the authored index with the complete embedded set | **Selected** |

Cargo explicitly supports build scripts generating Rust source in `OUT_DIR`; a directory passed to
`cargo::rerun-if-changed` is rescanned for entry changes. The generator will only enumerate safe
checked-in `.ron` files and will not parse or rewrite authored content.

## Proposed authored layout

```text
content/v4/
  map_definitions.ron                 shared policy and stable non-object definitions
  map_objects.ron                     semantic game-object definitions
  map_visual_variants.ron             render-neutral compatibility and fitting definitions
  map_themes.ron                      theme defaults and dressing choices
  maps/
    index.ron                         ordered built-in metadata and document paths
    builtin/
      crossroads-facility.ron         one complete Wipeout recipe
      crossroads-facility-hot-zone.ron
```

The current in-memory `MapContentCatalog` and `MapObjectCatalog` remain the public runtime
aggregates. File separation is an authoring and loading concern, not a new ECS layer or wire model.

### Shared definition files

`map_definitions.ron` owns the current recipe policy plus presentation, collision, region, entity,
mode, and anchor definition lists. The three object-library files parse into focused source structs
and are assembled into the existing `MapObjectCatalog` before reciprocal compatibility validation.

Splitting these files is justified by current ownership:

- adding a gameplay object should not edit themes or asset variants;
- adding a compatible art variant should not edit map recipes;
- a theme may change defaults without restricting compatible explicit overrides;
- client-only paths, transforms, material tints, and GLB handles remain exclusively in
  `assets/catalogs/environment_visuals.ron`.

### Built-in index

The index has one entry per complete built-in map:

```ron
(
  schema_version: 1,
  maps: [
    (
      id: MapPresetId(1),
      key: "crossroads-facility",
      display_name: "Crossroads Facility",
      admission_revision: 1,
      document: "builtin/crossroads-facility.ron",
    ),
  ],
)
```

Rules:

- entries are sorted by nonzero `MapPresetId` and have unique IDs, keys, recipe IDs, and paths;
- `key` retains the existing lowercase-hyphen grammar and the document must be exactly
  `builtin/<key>.ron`; arbitrary paths, absolute paths, backslashes, `..`, empty segments, and
  symlinks fail;
- `admission_revision` is the nonzero routing revision already carried by match manifests. It is
  distinct from a recipe's author revision and schema version;
- index metadata is the only owner of preset ID, public key, display name, admission revision, and
  source path. A recipe document owns gameplay content only;
- every indexed source must exist and every embedded source must be indexed. Unindexed leftovers
  and duplicate source files fail closed;
- recipes retain the existing 96 KiB per-recipe and 64 KiB per-resolved-snapshot runtime limits.

There is no additional built-in map-count, index-size, or aggregate source-size policy. Built-ins
are trusted reviewed content compiled with the game; the `u16` stable ID space and normal build
constraints are sufficient. Lobby advertisements remain separately bounded to eight maps per game
type, so the full library is not sent to every client in one message.

## Compile-time embedding and startup assembly

A small standard-library-only `build.rs` will:

1. emit `cargo::rerun-if-changed=content/v4/maps/builtin`;
2. inspect only direct regular `.ron` files in that directory and reject symlinks, non-UTF-8 names,
   nested directories, or unsupported extensions;
3. sort relative paths by byte order;
4. write `embedded_builtin_maps.rs` into `OUT_DIR` containing a static `(path, include_str!(...))`
   table.

It will never modify `content/`, parse RON, choose IDs, or infer index metadata. Runtime assembly
remains the single semantic validator and can be exercised with injected byte fixtures.

`MapContentCatalog::embedded()` will synchronously:

1. parse and size-check shared definition and object-library files;
2. parse and validate the index before resolving any document path;
3. compare the index against the generated embedded source table;
4. parse each recipe independently with unknown fields denied;
5. assemble presets in stable ID order;
6. validate all cross-catalog references and fully resolve every built-in against its mode layout;
7. expose the same `MapCatalogResource` used by lobby, server, client metadata, and fingerprinting.

A pure `assemble_map_catalog(shared, objects, variants, themes, index, sources)` boundary will own
this transaction. Tests can reverse the supplied source table, omit files, inject extras, and parse
non-preset fixtures without depending on global filesystem state.

This is build-embedded gameplay content, not a Bevy render asset. `FromWorld` resource
initialization stays synchronous and deterministic. GLBs and materials continue through M01's
asynchronous client-only environment asset lifecycle.

## Expanded semantic object placement

M02 recipe schema version 3 replaces the authored `geometry`, `visuals`, and `entities` arrays with
one bounded `objects` array. Regions, spawn areas, spawn points, and mode anchors remain specialized
because they have distinct authoritative rules.

```ron
objects: [
  (
    placement_id: MapPlacementId(1),
    object_definition_id: MapObjectDefinitionId(1),
    visual_variant_id: Some(MapVisualVariantId(2)),
    position: Vec2(0.0, -256.0),
    rotation: 0.0,
    footprint_override: Some(Rectangle(half_extents: Vec2(160.0, 32.0))),
  ),
  (
    placement_id: MapPlacementId(120),
    object_definition_id: MapObjectDefinitionId(100),
    visual_variant_id: None,
    position: Vec2(-288.0, -480.0),
    rotation: 0.0,
    footprint_override: None,
  ),
]
```

Each semantic object definition gains one server-safe placement binding:

- indestructible obstacle: default footprint, collision profile, and optional presentation profile;
- decoration: entity definition and presentation profile;
- surface and boundary: theme-generated presentation, not an explicit inside-map placement;
- destructible obstacle: catalogued but rejected until a later milestone owns its runtime health,
  removal, replication, restart, and recovery behavior;
- destructible terrain and markers: continue through their existing region/anchor forms.

The binding contains only stable gameplay/presentation IDs—never asset paths or Bevy handles. Its
shape must agree with the semantic role, and every referenced profile must be allowed by shared
policy.

Resolution performs these steps for every object placement:

1. resolve the semantic definition;
2. validate finite position and rotation quantized to the object's rotation step;
3. select an explicit compatible visual variant or the theme default;
4. use the object's default footprint unless a legal obstacle override is provided;
5. validate `Exact`, `Contained`, or `Modular` fit against the authoritative footprint;
6. lower an indestructible obstacle into the existing `GeometryPlacement`, or a decoration into
   the existing `MapEntityPlacement`.

The resolved `ResolvedMapSnapshot` structure remains unchanged. Server collision, replication,
client reconstruction, map-generation cleanup, terrain, and presentation therefore keep their
existing runtime contracts. The obsolete tiled-floor visual is removed from authored recipes;
M01's theme-driven generated ground already owns that presentation, so built-in snapshots no longer
expand 504 unused floor instances.

No compatibility decoder for recipe schema 2 will ship. Both current built-ins migrate atomically,
and the global content handshake rejects binaries with different embedded generations.

## Canonical identity and fingerprints

The following identities remain stable through migration:

- `MapPresetId(1)` / `crossroads-facility` and `MapPresetId(2)` /
  `crossroads-facility-hot-zone`;
- existing `MapRecipeId` values;
- stable mode, object, visual-variant, theme, collision, region, entity, and anchor IDs.

Recipe fingerprints may intentionally change because the authored schema and canonical recipe
version change. Recipe revisions will be incremented. `MAP_CATALOG_SCHEMA_VERSION`,
`MAP_RECIPE_SCHEMA_VERSION`, `MAP_FINGERPRINT_FORMAT_VERSION`, and the global gameplay content
envelope version will be advanced together.

Canonical map fingerprint material includes:

- engine limits and layout schema versions;
- assembled shared/object/variant/theme definitions;
- preset ID, key, display name, admission revision, and normalized recipe for every map sorted by
  preset ID.

It excludes source filenames, generated table order, and the physical split among catalog files.
Renaming internal source storage without changing the index's semantic catalog must not alter
gameplay identity; changing a recipe, stable definition, public map metadata, or admission revision
must alter it.

## Routed selection and network behavior

M02 closes the existing multi-map admission gap without changing routing wire shapes:

1. lobby operator configuration resolves map keys through the assembled catalog as today;
2. queue reservation continues deterministic round-robin selection across the advertised IDs;
3. manifest construction obtains `map_preset` and `map_revision` from the selected indexed preset,
   removing literal revision `1` writes;
4. match-worker admission resolves the manifest preset from the embedded catalog, verifies its
   admission revision, verifies its mode against the manifest/configuration, and fully resolves it;
5. before `Startup`, the worker overwrites `ServerMapSelection` with the admitted preset;
6. direct-development mode keeps its existing Wipeout and Hot Zone default preset constants.

The global content fingerprint still proves that supervisor-facing processes, lobby workers,
match workers, and clients contain the same canonical definitions and recipes. Unknown IDs,
revision mismatch, mode mismatch, or content mismatch fail before match authority starts.

`MapRecipe` remains build-embedded authored data, not a new network upload format. Clients still
send selection intent only and receive the resolved server snapshot. Custom-map transfer,
persistence, signatures, moderation, and arbitrary filesystem loading remain outside V4.

## ECS ownership and lifecycle

No new gameplay ECS state is required:

```text
build.rs embedded source table
            +
shared catalogs + authored index
            |
            v
MapContentCatalog assembly/validation (startup, both roles)
            |
     +------+------+
     |             |
     v             v
lobby metadata   admitted ServerMapSelection
                       |
                       v
existing resolve/install/replicate/present lifecycle
```

- `MapContentPlugin` owns the immutable assembled catalogs.
- lobby catalog resolution owns public map availability and per-game-type compatibility checks.
- match-worker admission owns selected preset/revision/mode verification.
- `AuthoritativeMapPlugin` owns resolution, generation identity, collision, and teardown exactly as
  before.
- client presentation owns only replicated snapshot visualization and client-only assets.
- no content parsing, filesystem access, or fingerprint work runs in `FixedUpdate`.

## Implementation plan

### Phase 1 — Source bundle and catalog split

- [x] Add the standard-library build script and generated embedded source table.
- [x] Split shared definitions, objects, variants, themes, and built-in index into their accepted
      V4 files.
- [x] Add deny-unknown-field source structs and pure transactional catalog assembly.
- [x] Validate exact index/source-set equality, safe paths, sorted IDs, unique
      keys/recipe IDs, and reciprocal object compatibility.
- [x] Preserve the existing in-memory catalog resource/API where it remains useful.

### Phase 2 — Recipe schema and resolver

- [x] Add the semantic object placement and server-safe per-role placement bindings.
- [x] Lower semantic obstacle/decoration placements into the existing resolved snapshot shapes.
- [x] Enforce rotation steps, theme defaults, explicit compatibility, fitting policies, footprint
      bounds, placement limits, and unsupported-role rejection.
- [x] Migrate the two current built-ins into independent documents and remove their unused tiled
      floor visuals.
- [x] Advance schema/fingerprint versions and update intentional golden identities.

### Phase 3 — Routed map admission

- [x] Carry indexed admission revision through lobby reservation/manifest construction.
- [x] Replace hard-coded match-worker preset checks with catalog-backed preset, revision, mode, and
      recipe validation.
- [x] Install the admitted preset into `ServerMapSelection` before startup.
- [x] Preserve direct Wipeout/Hot Zone defaults and all existing authority boundaries.

### Phase 4 — Documentation and verification

- [x] Document the exact two-file addition workflow: create `builtin/<key>.ron`, then add the sorted
      index entry; edit operator game types only when the map should be advertised.
- [x] Update root/content architecture documentation and remove references to the monolithic map
      catalog.
- [x] Run focused unit tests, role checks, full integration/network/performance gates, and a native
      visual regression pass in both modes.

## Verification evidence

Automated verification passed on 2026-08-21:

- `just fmt` and `just lint` passed, including routing, client-only, server-only, feature-isolation,
  and retired-renderer checks;
- `just check` passed for routing plus all client, server, and network-test targets;
- `just test` passed 99 routing/process tests, 387 client-role tests, 309 server-role tests, 82
  serialized network tests, and 14 performance tests;
- the maximum semantic-map snapshot converged over both typical and adverse real-UDP impairment;
- native routed render evidence passed for Wipeout and Hot Zone with imported assets and with
  `BRAWLER_FORCE_PRIMITIVE_WORLD=1`. Each run used two clients, a 10-second warm-up, and a
  30-second measurement. Reports are under `target/v4-m02-*.txt` and are intentionally untracked.

## User playtest handoff

Run `just run 1`, select both Wipeout and Hot Zone from the product flow, and play one short match
of each. Please check that the arena bounds, wall/decoration placement, floor, border, Hot Zone
objective, terrain, camera, and HUD look unchanged from the accepted M01 presentation. The expected
intentional difference is internal only: authored recipes are now independent semantic-object
documents and no longer contain the unused 504-instance tiled-floor expansion.

Known limitation: M02 does not add a new map or product-facing map selector; that proof belongs to
M03. Destructible discrete objects remain catalogued but deliberately unplaceable until their
authoritative lifecycle is implemented.

## Feedback review and closeout — 2026-08-21

The user completed the visual playtest and reported that the migrated maps looked unchanged. This
is the intended result: M02 replaces authored storage and resolution paths while preserving the
accepted M01 presentation. No feedback change or additional reverification was required.

The final verification remains the evidence recorded above: all canonical format, role, lint,
unit, network, performance, and four native render paths passed after the implementation changes.

### Learn-from-errors review

- two network tests still encoded the removed 504-instance tiled-floor representation even though
  runtime presentation no longer used it. Tests for a storage migration should assert enduring
  resolved behavior, not obsolete intermediate expansion counts;
- the maximum-policy fixture initially appended the full entity allowance without subtracting the
  four semantic decorations already present. Capacity fixtures must derive every existing lowered
  category before filling to a limit;
- keeping the resolved snapshot and map lifecycle unchanged made the migration substantially safer:
  semantic authoring can evolve independently while server collision, replication, client
  reconstruction, and rendering continue to consume their established contract;
- exact index/source-set validation provides scalable discovery without runtime filesystem state or
  a hand-maintained Rust source list.

No new general-purpose skill is justified. These are project-specific map-content and regression
testing lessons and are recorded here for M03.

## Verification plan

### Parsing and index tests

- independently parse and RON-round-trip each built-in and a non-preset fixture;
- reject malformed UTF-8, unknown fields, unsupported schema, empty sources, oversize recipes,
  zero IDs/revisions, invalid metadata, unsafe paths, symlinks at generation time, duplicate
  IDs/keys/recipe IDs/paths, missing indexed files, and unindexed embedded files;
- assemble the same catalog from forward and reversed embedded-source order and assert identical
  canonical material/fingerprint;
- prove adding a fixture document and index row requires no Rust source change.

### Object-resolution tests

- theme default and explicit compatible variants lower to identical authority geometry;
- mixed wall styles remain legal in one map;
- incompatible variants, invalid rotation steps, illegal footprint overrides, out-of-bounds
  placements, unsupported destructible-object placements, and profile-policy mismatches fail;
- decorations produce no colliders; obstacles preserve exact authoritative shapes;
- recipe normalization remains independent of placement order, equivalent rotation, and signed
  zero;
- resolved snapshots round-trip exactly and remain below their byte limit;
- migrated maps preserve bounds, geometry, entities, spawns, regions, anchors, and terrain layout,
  while legacy floor instances disappear.

### Routing and authority tests

- operator catalog resolves multiple same-mode maps by key and rejects unknown/incompatible maps;
- deterministic queue rotation emits each advertised preset and its indexed admission revision;
- a match worker accepts the second same-mode fixture, installs that exact preset, and rejects
  unknown preset, wrong revision, wrong mode, or changed content fingerprint before startup;
- separate-App and routed-process tests prove the selected resolved fingerprint reaches match
  summary/diagnostics and converges on clients;
- clients cannot submit recipes, positions, collision, or map identity.

### Regression and performance

- `just check`, including client-only and dedicated-server feature isolation;
- `just lint`;
- `just test`, including serialized network and routed-process suites;
- existing maximum recipe/snapshot/terrain performance gates;
- native Wipeout and Hot Zone visual pass on imported and forced-primitive world paths to confirm
  the storage migration does not alter camera, border, floor, cover, objective, terrain, or UI.

## Risks and containment

- **Build-script portability:** use only `std`, write only to `OUT_DIR`, reject symlinks/non-UTF-8
  paths, sort input, and test the generated table contract.
- **Fingerprint churn:** advance the explicit format/envelope versions once and update goldens only
  after semantic migration tests pass.
- **Object abstraction becoming too general:** support only current indestructible obstacles and
  decorations. Keep destructible objects catalogued but unplaceable until their real lifecycle is
  specified.
- **Hidden routing default:** test a second same-mode fixture through lobby reservation and worker
  startup so no map-ID branch remains.
- **Presentation regression:** keep resolved snapshot shapes and client visual catalog unchanged;
  compare both current maps before accepting intentional removal of unused floor instances.

## Deferred work

- player-facing map editor and custom-map launch/save/load flows;
- runtime directory watching or hot reload of authoritative recipes;
- remote/user map transfer, signatures, publishing, moderation, and persistence;
- destructible discrete obstacle runtime;
- new gameplay modes or arbitrary user-defined rules;
- the second visual theme and materially different layout, owned by M03.

## Exit criteria

M02 enters user playtest when:

1. shared definitions and both current maps load from the accepted V4 file layout;
2. each built-in is independently parseable, validated, and round-trippable;
3. one indexed document plus one index row adds a built-in without Rust edits;
4. semantic object placements resolve through object bindings and theme/explicit variants into the
   unchanged authoritative snapshot;
5. canonical fingerprints are independent of file enumeration and reject semantic changes;
6. lobby rotation and match-worker admission install an indexed same-mode fixture without hard-coded
   map identity;
7. both current modes pass authority, routing, network, lifecycle, terrain, performance, and visual
   regression checks.

Complete only after user feedback classification, affected reverification, documentation updates,
and a learn-from-errors review.

## Research references

Local project:

- `src/map/definitions/{mod.rs,resolver.rs,tests.rs}` — aggregate parsing, validation,
  normalization, resolution, and fingerprints;
- `src/map/{model.rs,objects.rs,server.rs,client.rs}` — authored/resolved shapes, M01 taxonomy,
  authority installation, and client reconstruction;
- `src/content.rs` and `src/protocol.rs` — global content compatibility envelope;
- `src/server/lobby/{catalog.rs,queue.rs}` and `src/server/admission.rs` — map-key discovery,
  reservation rotation, and the hard-coded worker-admission gap;
- `packages/brawler-routing/src/{allocation.rs,manifest.rs,runtime.rs}` — existing preset/revision
  routing fields;
- `content/v1/maps.ron`, `content/v4/map_objects.ron`, and
  `config/server/game-types.ron` — current authored sources;
- `references/bevy/examples/asset/embedded_asset.rs` and
  `references/bevy/crates/bevy_asset/src/io/embedded/mod.rs` — Bevy's build-embedded render-asset
  mechanism uses literal `include_bytes!` paths and an `AssetServer` source; useful comparison, but
  not the selected authority-content loader.

Primary external:

- [Cargo Book — Build Scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html) —
  generated source belongs in `OUT_DIR`, and directory `rerun-if-changed` supports source-set
  changes;
- [Serde — Container attributes](https://serde.rs/container-attrs.html) — source structs can reject
  unknown fields with `deny_unknown_fields`;
- [RON project and format](https://github.com/ron-rs/ron) — typed Serde-based RON remains the
  checked-in authoring format.

The local Bevy snapshot is 0.20-dev while Brawler pins 0.19.1. M02 deliberately avoids a new exact
Bevy asset API: gameplay documents remain synchronous embedded Rust data in both roles, and the
existing compiling 0.19.1 client asset path remains unchanged.
