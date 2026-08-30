# BRL-0073 specification

## Outcome

Direct diagnostic admission resolves its fallback fighters from a validated authored policy in `content/catalogs/builds.ron`. The current behavior is preserved exactly: fighter profile 1; weapon bases 1, 2, 3, and 4 selected cyclically by stable allocated player ID; ultimate 1; and passives 3 and 4. No production admission code owns those balance/content identities.

## Scope

1. Add a typed `DirectDiagnosticLoadoutPolicy` to `BuildCatalog` with one fighter-profile ID, a bounded non-empty ordered list of weapon-base IDs, one ultimate definition ID, and two passive definition IDs.
2. Author the accepted current values explicitly in `content/catalogs/builds.ron`. The weapon rotation order is policy data and must not be inferred from `weapon_costs`, weapon preset vector position, or enum discriminants.
3. Bump the build catalog schema and build fingerprint format for the additive serialized field. Keep the global content envelope shape/version unchanged because the existing build-material contribution already covers the new bytes.
4. Extend shared build validation to reject zero/unsupported fighter IDs, empty/excessive weapon rotations, zero or duplicate weapon IDs, missing ultimate/passive IDs, duplicate passives, and frame passives that are not legal saved-brawler choices.
5. Extend build/weapon cross-catalog validation to reject any authored diagnostic weapon ID absent from the active weapon catalog and prove every authored rotated recipe resolves through the ordinary saved-brawler resolver.
6. Add `resolve_direct_diagnostic_loadout(builds, weapons, player_id)` as the focused stable selection/resolution API. It must use `(player_id - 1) mod authored_rotation_len`, preserve deterministic cycling, and fail closed through `BuildResolutionError`.
7. Replace the hardcoded construction inside `process_client_hellos` with that API. Do not change routed manifest loadouts, persistent profiles, Practice bot identities, team/spawn selection, allocation order, or admission rejection precedence.
8. Update durable build/content documentation only where it clarifies ownership.

## Constraints

- Server authority, fixed scheduling, protocol types, saved-brawler resolution, selected-build fingerprints, and replication remain unchanged.
- This ticket must not preserve or expand retired point-budget machinery owned by BRL-0043.
- Do not add a second catalog/resource solely for one demonstrated policy; the policy belongs to the existing build catalog and is automatically included in its fingerprint material.
- The authored rotation is bounded by an engine ceiling; the ceiling remains code-owned.
- The policy is not a player-facing default build, Practice-bot build, or profile migration. Those existing paths remain out of scope.
- Preserve unrelated worktree changes.

## Verification

- Focused build validation, fingerprint, selection-cycle, and resolution tests.
- Direct diagnostic network loadout tests.
- Server/client role checks.
- `cargo fmt --all` and `git diff --check`.
- `just check`.

No native evidence is required because the accepted diagnostic loadouts remain unchanged.

## Acceptance criteria

- [x] Direct diagnostic admission contains no fighter/weapon/ultimate/passive ID literals or hardcoded rotation length.
- [x] The build catalog contains the exact accepted policy and its fingerprint changes when any policy field changes.
- [x] Invalid local and cross-catalog references fail validation.
- [x] Player IDs cycle deterministically through authored weapon IDs 1, 2, 3, 4.
- [x] Routed and profile-owned loadout paths are unchanged.
- [x] Focused tests and the canonical repository gate pass.
- [x] Verification and learning are recorded before closeout.


## Design amendment — preserve accepted build identities

Only `BUILD_CATALOG_SCHEMA_VERSION` advances for the additive serialized policy. `BUILD_FINGERPRINT_FORMAT_VERSION` remains unchanged because it also governs persisted/admitted recipe identities, and this system-only diagnostic policy must not churn player `BuildRecipeFingerprint` values. Resolution now uses a dedicated private recipe-schema compatibility constant retaining the historical value previously supplied by `catalog.schema_version`; therefore the fingerprint tuple bytes for every unchanged player recipe remain identical. The global gameplay-content fingerprint still changes because the existing canonical build-catalog material serializes the new schema and policy fields.


## Implementation and verification record — 2026-08-30

Implemented the policy in `BuildCatalog` and authored the accepted values in `content/catalogs/builds.ron` schema 17. Direct diagnostic admission now calls the focused server-only `resolve_direct_diagnostic_loadout` API; production admission contains no fighter, weapon, ultimate, passive, or rotation-length literals. Validation is bounded and fail-closed across build-local IDs and weapon-catalog references, while each authored recipe is proved through the existing saved-brawler resolver.

Compatibility remained explicit: the global gameplay-content fingerprint changes with any diagnostic policy field, but unchanged persisted/admitted player recipes retain their historical `BuildRecipeFingerprint` bytes through the private recipe-schema compatibility constant. Routed manifests, saved profiles, Practice bots, allocation, teams, spawns, protocol shapes, and admission precedence were not changed.

Verification passed:

- `cargo test --locked --no-default-features --features server --lib builds` — 15 passed.
- `cargo test --locked --no-default-features --features client --lib content::tests` — 3 passed.
- `cargo test --locked --no-default-features --features network-test --test network loadouts::` — 1 passed; two direct clients retained weapon bases 1/2 and expected delivery behavior.
- `just check` — routing, client, server, network-test, Balance Lab Rust targets, Balance Lab web tests, and web build passed.
- `cargo fmt --all` and `git diff --check` — passed.

No native evidence was required because authored values and player-visible behavior are unchanged.

## Learn-from-errors review

- The first schema plan coupled catalog compatibility and player recipe identity. Cause: `catalog.schema_version` previously served two responsibilities inside fingerprint material. Prevention: classify every schema/version consumer before advancing it; keep system-policy content compatibility separate from persisted identity compatibility. The implementation now locks the historical recipe tuple with a regression assertion.
- The first RON draft used list syntax for a fixed-size passive array. Cause: treating `Vec` and array serialization syntax as interchangeable. Prevention: run the embedded-catalog parse test immediately after schema edits and use tuple syntax for fixed arrays.
- The initial resolver test compiled in client all-target checks although the API is server-only. Cause: the test's feature ownership was broader than the symbol's. Prevention: feature-gate server-only behavior tests and retain client-shared coverage at the content-fingerprint boundary.
