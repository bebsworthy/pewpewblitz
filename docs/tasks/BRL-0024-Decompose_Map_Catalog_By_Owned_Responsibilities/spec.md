# Context

Repowise identifies `src/map/catalog.rs` as a high-risk oversized hotspot. The review confirmed that the file is 3,579 physical lines and combines several independently changing responsibilities:

- authored map/catalog definitions, embedded RON loading, catalog validation, lookup, and fingerprinting;
- shared replicated snapshot and dynamic-state wire shapes;
- canonical recipe expansion, validation, ordering, fingerprinting, and resolved-map construction;
- collision geometry, dynamic effective-state queries, navigation validation, and Heist access validation;
- approximately 1,230 lines of tests.

The strongest findings are the 269-line `resolve_grid_recipe`, the mixed-output `derive_runtime_facts`, and compound rule matrices in `validate_replacement_assets` and `validate_asset_profile`. Repowise's duplication, churn, presentation co-change, missing-ADR, and unwrap findings are context rather than independent reasons to change behavior.

# Outcome

Create clear responsibility and lifecycle boundaries inside `src/map/` while retaining the existing `crate::map` API and producing no player-visible, authority, content, persistence, protocol, or rendering change.

# Scope

## Module ownership

Decompose the current implementation along these demonstrated responsibilities. Exact private filenames may be adjusted if source inspection shows a smaller cohesive arrangement, but any material deviation must be recorded in this spec before implementation.

1. `catalog.rs`
   - Own authored catalog/profile/asset/recipe definitions, schema constants, embedded source parsing, catalog resource/plugin installation, catalog lookup, catalog-level validation, and canonical catalog fingerprint material.
   - Keep catalog validation headless-safe and independent of client rendering/assets.

2. `state.rs`
   - Own the shared serialized/replicated resolved snapshot, dynamic generation/state, mutation, reset, recovery, and placement-transition shapes.
   - Remain available to both client and server roles without importing rendering, audio, device input, or server-only mutation systems.

3. `resolution.rs`
   - Own `ResolvedMap` and its derived result types plus canonical recipe-to-runtime resolution.
   - Retain a short coordinating resolution function with explicit phases rather than moving the existing giant function unchanged.
   - Extract focused helpers/results for, at minimum: recipe identity/default-surface validation; filled-rectangle expansion; placement identity/slot/parameter/surface validation; canonical ordering and mode-anchor resolution; capacity/navigation/topology validation; fingerprint/snapshot construction and byte ceilings; and derived runtime fact construction.
   - Preserve deterministic ordering and the exact material included in recipe fingerprints.

4. `geometry.rs`
   - Own pure placement/world geometry, collider-shape derivation, effective dynamic blocker/projectile queries, circle overlap/relaxation, cell-to-rectangle merging, and reusable navigation/access geometry.
   - Keep client use read-only. Authoritative collision installation and mutation remain in the existing server-gated runtime module.

5. Focused tests
   - Move tests beside their owning module or into focused `tests.rs` submodules so production responsibilities are not obscured.
   - Preserve behavioral coverage for catalog validation, recipe rejection, canonical ordering/fingerprints, map topology, collision/navigation, and wire-size ceilings.

`src/map/mod.rs` remains the composition and public re-export surface. Existing consumers should not need import-path changes outside private `src/map/` implementation imports.

## Function-level improvements

- Replace the `clippy::too_many_lines` allowance on `resolve_grid_recipe` with a readable phase coordinator and focused helpers. Preserve error propagation and transaction semantics.
- Split `derive_runtime_facts` into independently named collider and spawn derivation operations, returning a named result structure instead of an opaque tuple where that improves clarity.
- Remove or correct the ineffective `cells.remove(...)` work performed after `std::mem::take(&mut cells)`. Avoid repeatedly scanning every placement for every merged cell; build a stable placement-origin lookup when deriving collider identities.
- Express asset/profile, replacement, and Heist-anchor compatibility through named predicates or focused validators. Prefer specific invariant-oriented error messages where doing so does not alter a tested external contract.
- Keep schedule ordering, deferred-command boundaries, protocol registration, stable IDs, and feature gates unchanged.
- Do not add a framework, crate, public abstraction, compatibility decoder, per-message schema version, or alternative runtime path.

# Compatibility constraints

- Preserve all current public `brawler::map::*` exports and their visibility unless a consumer audit proves an item is private and the spec is updated before changing it.
- Preserve serde field order, enum variant order, discriminants, component/resource derives, protocol registrations, schema-version constants, admission revisions, and snapshot/event byte contracts.
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

# Verification

Before or during extraction, retain or add characterization coverage sufficient to prove unchanged behavior for representative Wipeout, Hot Zone, and Heist presets, including:

- resolved snapshots and deterministic placement/anchor ordering;
- canonical catalog and recipe fingerprints;
- dynamic mutation/recovery serialization and byte ceilings;
- fighter and projectile collision geometry;
- spawn connectivity and Heist attack/defence access validation;
- invalid asset/profile/replacement/anchor/placement rejection.

Run and record:

1. `just fmt`
2. focused map/catalog unit tests during implementation;
3. `just check` for independently buildable client and headless server roles;
4. `just lint` for formatting, Clippy, and dedicated-server isolation;
5. `just test` for the deterministic Rust and performance suites;
6. `repowise health --file src/map/catalog.rs --format md` and equivalent checks for the extracted modules, with remaining findings explicitly dispositioned rather than optimized blindly.

A native playtest is not required for a behavior-preserving organization-only change unless automated verification exposes a presentation or gameplay uncertainty.

# Acceptance criteria

- [ ] Catalog loading/definitions, replicated state, canonical resolution, geometry/topology rules, and tests have explicit cohesive module owners under `src/map/`.
- [ ] `src/map/mod.rs` visibly composes the new modules and preserves the established public `crate::map` API.
- [ ] `resolve_grid_recipe` is a concise phase coordinator; the existing `clippy::too_many_lines` allowance is removed and the original body was not merely relocated intact.
- [ ] Runtime collider derivation and spawn indexing are separate named operations, use a named result where appropriate, and no longer perform ineffective removal or repeated full placement scans per merged cell.
- [ ] Compound catalog/profile/replacement and mode-topology rules are decomposed enough that each invariant is reviewable and focused tests identify the rejected contract.
- [ ] Existing serialized shapes, protocol registration, schema constants, stable IDs, field/variant ordering, canonical fingerprint material, and wire-size ceilings are unchanged.
- [ ] Server authority and feature isolation remain intact; no client rendering/audio/input dependency enters the headless graph and no client-side authority path is introduced.
- [ ] All built-in maps resolve with unchanged deterministic snapshots, runtime facts, objectives/safes, collision behavior, and navigation/access guarantees.
- [ ] Existing and added focused tests pass, followed by successful `just check`, `just lint`, and `just test` runs.
- [ ] Repowise is rerun on the resulting module family; resolved findings and justified residual findings are recorded in the ticket without using a hard line-count or score target.
- [ ] Durable documentation is updated only if the refactor reveals or changes an enduring ownership contract; otherwise the ticket records that no documentation change was necessary.
- [ ] A proportional learn-from-errors review records mistakes, causes, prevention, and reusable lessons, or explicitly records that no material errors occurred.
- [ ] `ticket sync` completes without conflicts before closeout.
