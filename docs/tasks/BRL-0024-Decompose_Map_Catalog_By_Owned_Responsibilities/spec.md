# Context

Repowise identifies `src/map/catalog.rs` as a high-risk oversized hotspot. The file combines several independently changing responsibilities and continues to grow as map capabilities are added:

- authored map/catalog definitions, embedded RON loading, catalog validation, lookup, and fingerprinting;
- shared replicated snapshot and dynamic-state wire shapes;
- canonical recipe expansion, validation, ordering, fingerprinting, and resolved-map construction;
- collision geometry, dynamic effective-state queries, navigation validation, and Heist access validation;
- a large inline test module spanning all of those concerns.

The strongest findings are the oversized `resolve_grid_recipe`, the mixed-output `derive_runtime_facts`, and compound rule matrices in `validate_replacement_assets` and `validate_asset_profile`. Repowise's duplication, churn, presentation co-change, missing-ADR, and unwrap findings are context rather than independent reasons to change behavior.

BRL-0036 is actively adding authored effect tiles and currently changes `src/map/catalog.rs`, `src/map/mod.rs`, resolved-map data, schema/fingerprint constants, validation, and focused tests. BRL-0024 must therefore begin only after BRL-0036 is accepted and its changes form the clean implementation baseline. Reinspect the resulting map module family and update this spec through Ticket before implementation if that completed work materially changes the ownership split below.

# Outcome

Create clear responsibility and lifecycle boundaries inside `src/map/` while retaining the established `crate::map` API and producing no player-visible, authority, content, persistence, protocol, or rendering change relative to the accepted post-BRL-0036 baseline.

# Execution preconditions

- BRL-0036 is complete and its accepted changes are present in the implementation baseline.
- The worktree is clean for files in the BRL-0024 change set, or any unrelated user changes have been identified and can be preserved without overlap.
- Record the baseline revision in a BRL-0024 ticket comment before extraction begins.
- Add and run the exact compatibility characterization described below before moving production definitions or algorithms. The same assertions must pass after extraction.

# Scope

## Module ownership

Decompose the current implementation along these demonstrated responsibilities. Exact private filenames may be adjusted if source inspection shows a smaller cohesive arrangement, but any material deviation must be recorded in this spec before implementation.

1. `catalog.rs`
   - Own authored catalog/profile/asset/recipe definitions, schema constants, embedded source parsing, catalog resource/plugin installation, catalog lookup, catalog-level validation, and canonical catalog fingerprint material.
   - Incorporate the accepted post-BRL-0036 authored effect-tile definitions and validation without changing their behavior or wire representation.
   - Keep catalog validation headless-safe and independent of client rendering/assets.

2. `state.rs`
   - Own the shared serialized/replicated resolved snapshot, dynamic generation/state, mutation, reset, recovery, and placement-transition shapes.
   - Remain available to both client and server roles without importing rendering, audio, device input, or server-only mutation systems.

3. `resolution.rs`
   - Own `ResolvedMap` and its derived result types plus canonical recipe-to-runtime resolution, including accepted post-BRL-0036 resolved facts.
   - Retain a short coordinating resolution function with explicit phases rather than moving the existing giant function unchanged.
   - Extract focused helpers/results for, at minimum: recipe identity/default-surface validation; filled-rectangle expansion; placement identity/slot/parameter/surface validation; canonical ordering and mode-anchor resolution; capacity/navigation/topology validation; fingerprint/snapshot construction and byte ceilings; and derived runtime fact construction.
   - Preserve deterministic ordering and the exact material included in recipe and catalog fingerprints.

4. `geometry.rs`
   - Own pure placement/world geometry, collider-shape derivation, effective dynamic blocker/projectile queries, circle overlap/relaxation, cell-to-rectangle merging, and reusable navigation/access geometry.
   - Resolution may coordinate topology validation, but reusable spatial primitives and reachability calculations belong here so there is one clear geometry owner.
   - Keep client use read-only. Authoritative collision installation and mutation remain in the existing server-gated runtime module.

5. Focused tests
   - Move tests beside their owning module or into focused `tests.rs` submodules so production responsibilities are not obscured.
   - Preserve behavioral coverage for catalog validation, recipe rejection, canonical ordering/fingerprints, map topology, collision/navigation, post-BRL-0036 map capabilities, and wire-size ceilings.

`src/map/mod.rs` remains the composition and public re-export surface. Existing consumers should not need import-path changes outside private `src/map/` implementation imports.

## Function-level improvements

- Replace the `clippy::too_many_lines` allowance on `resolve_grid_recipe` with a readable phase coordinator and focused helpers. Preserve error propagation and transaction semantics.
- Split `derive_runtime_facts` into independently named collider and spawn derivation operations, returning a named result structure instead of an opaque tuple where that improves clarity.
- Remove or correct the ineffective `cells.remove(...)` work performed after `std::mem::take(&mut cells)`. Avoid repeatedly scanning every placement for every merged cell; build a stable placement-origin lookup when deriving collider identities.
- Express asset/profile, replacement, effect-tile, and Heist-anchor compatibility through named predicates or focused validators. Prefer specific invariant-oriented error messages where doing so does not alter a tested external contract.
- Keep schedule ordering, deferred-command boundaries, protocol registration, stable IDs, and feature gates unchanged.
- Do not add a framework, crate, public abstraction, compatibility decoder, per-message schema version, or alternative runtime path.

# Compatibility constraints

- Preserve every public item currently re-exported by `src/map/mod.rs`, with the same name, namespace, and visibility, unless a consumer audit proves an item is private and this spec is updated before changing it.
- Preserve serde field order, enum variant order, discriminants, component/resource derives, protocol registrations, schema-version constants, admission revisions, and snapshot/event byte contracts from the accepted baseline.
- Preserve canonical catalog and recipe fingerprint inputs and ordering exactly.
- Preserve server-authoritative mutation and collision ownership. Geometry shared with the client must remain pure/read-only.
- Preserve build-embedded, headless-safe catalog loading. The server feature graph must not acquire client-only dependencies.
- Preserve authored RON formats and all built-in map content without migration or reformatting.
- Preserve unrelated user changes.

# Non-goals

- No gameplay, balance, map-content, presentation, protocol, persistence, or admission-policy changes.
- No new map schema or compatibility path.
- No numeric line-count target; cohesion and independently testable ownership govern the split.
- No ADR solely to satisfy Repowise's decision index. Durable behavior remains governed by the existing Brawler documentation and this ticket.
- No deduplication of legitimate protocol tests merely to improve a duplication percentage.

# Compatibility characterization

Before extraction, add focused tests that pin the accepted baseline rather than merely checking internal equivalence or upper bounds:

1. Resolve every built-in preset with a fixed nonzero `MapInstanceId` and assert its exact recipe fingerprint plus a stable digest of the serialized `ResolvedMapSnapshot`.
2. Assert the exact canonical catalog fingerprint material digest for the accepted embedded catalog.
3. For each distinct dynamic-map shape represented by the built-ins, assert stable digests of representative `MapMutationEvent`, `MapDynamicResetEvent`, `MapDynamicRecoveryRequest`, and `MapDynamicRecoverySnapshot` postcard bytes. Continue asserting the established byte ceilings separately.
4. Assert exact deterministic ordering and stable digests for derived runtime facts that are not serialized in the snapshot: team spawn indexing, static colliders, dynamic placements, player-only surface rectangles, objectives/safes, and accepted post-BRL-0036 resolved facts.
5. Retain focused behavioral tests for fighter/projectile collision geometry, spawn connectivity, Heist attack/defence access, and invalid asset/profile/replacement/anchor/placement rejection.
6. Add a compile-only public-API characterization that names every item re-exported by `src/map/mod.rs` at the baseline. It must use the external `brawler::map::*` path so an accidentally omitted or visibility-reduced re-export fails even when no production consumer uses it.

Use deterministic in-test digesting already available to the crate; do not add a dependency or general snapshot framework solely for this refactor. When an accepted baseline intentionally changes before BRL-0024 starts, regenerate and review the characterization values before extraction, record the reason in the ticket, and do not update expected values merely to make a post-refactor failure pass.

# Verification

Run the compatibility characterization before extraction and again after extraction. Then run and record:

1. `just fmt`
2. focused map catalog/state/resolution/geometry tests during implementation;
3. the public-API characterization test;
4. `just check` for independently buildable client and headless server roles;
5. `just lint` for formatting, Clippy, and dedicated-server isolation;
6. `just test` for the deterministic Rust and performance suites;
7. `repowise health --file src/map/catalog.rs --format md` and equivalent checks for the extracted modules, with remaining findings explicitly dispositioned rather than optimized blindly.

A native playtest is not required for a behavior-preserving organization-only change unless automated verification exposes a presentation or gameplay uncertainty.

# Acceptance criteria

- [ ] BRL-0036 is complete; BRL-0024 records the clean accepted baseline revision and does not overlap unresolved map work.
- [ ] Pre-extraction characterization pins exact catalog/recipe fingerprints, serialized snapshot and dynamic-message digests, derived runtime facts for every built-in preset, established byte ceilings, and the complete public `brawler::map` re-export surface.
- [ ] Catalog loading/definitions, replicated state, canonical resolution, geometry/topology rules, and tests have explicit cohesive module owners under `src/map/`.
- [ ] `src/map/mod.rs` visibly composes the new modules and preserves the established public `crate::map` API.
- [ ] `resolve_grid_recipe` is a concise phase coordinator; the existing `clippy::too_many_lines` allowance is removed and the original body was not merely relocated intact.
- [ ] Runtime collider derivation and spawn indexing are separate named operations, use a named result where appropriate, and no longer perform ineffective removal or repeated full placement scans per merged cell.
- [ ] Compound catalog/profile/replacement/effect-tile and mode-topology rules are decomposed enough that each invariant is reviewable and focused tests identify the rejected contract.
- [ ] Existing serialized shapes, protocol registration, schema constants, stable IDs, field/variant ordering, canonical fingerprint material, and exact characterized bytes/digests are unchanged from the recorded baseline.
- [ ] Server authority and feature isolation remain intact; no client rendering/audio/input dependency enters the headless graph and no client-side authority path is introduced.
- [ ] Every built-in map resolves with unchanged deterministic snapshots, runtime facts, objectives/safes, collision behavior, navigation/access guarantees, and accepted post-BRL-0036 capabilities.
- [ ] The complete baseline `brawler::map` public re-export characterization compiles unchanged.
- [ ] Existing and added focused tests pass, followed by successful `just check`, `just lint`, and `just test` runs.
- [ ] Repowise is rerun on the resulting module family; resolved findings and justified residual findings are recorded in the ticket without using a hard line-count or score target.
- [ ] Durable documentation is updated only if the refactor reveals or changes an enduring ownership contract; otherwise the ticket records that no documentation change was necessary.
- [ ] A proportional learn-from-errors review records mistakes, causes, prevention, and reusable lessons, or explicitly records that no material errors occurred.
- [ ] `ticket sync` completes without conflicts before closeout.
