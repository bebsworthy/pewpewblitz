# Context

V7 removed the shared 12-point budget from the player-facing saved-brawler workflow, and BRL-5pxn later removed the superseded full-build editor, persistence, selection transaction, named presets, and product copy. Saved-brawler resolution explicitly bypasses budget enforcement and reports zero total points.

Private legacy machinery still remains:

- `BUILD_POINT_BUDGET`, `build_point_total`, `BuildResolutionError::OverBudget`, weapon/ultimate/passive `point_cost` fields, and Custom Pulse cost increments;
- the legacy `resolve_build_recipe` path that enforces the retired budget and infers fighter profiles from frame passives;
- `total_points` in resolved/accepted loadout summaries, queue validation and rejection shapes, match telemetry, verification output, and client loading/queue copy;
- catalog validation and tests that continue to assign balance meaning to costs no longer used by the product.

This residue caused BRL-1t80 planning to incorrectly describe a new ultimate as having a 4-point product cost.

# Outcome

Saved brawlers have one obvious authoritative resolution model with no point costs, total-point budget, over-budget rejection, or legacy full-build resolution path. Weapon base, ultimate, two passive slots, fighter profile, and four weapon-part slots remain the actual bounded build choices.

# Scope

- Audit all live uses of `point_cost`, `BUILD_POINT_BUDGET`, `build_point_total`, `total_points`, `OverBudget`, Custom Pulse point increments, and `resolve_build_recipe`.
- Remove cost fields from authored build definitions and resolved ultimate/passive/loadout shapes where no current non-budget behavior needs them.
- Remove the legacy budget-enforcing resolver and retain `resolve_saved_brawler_recipe` as the single product authority path, renaming it only if the resulting API becomes clearer.
- Remove frame-passive stat inference and legacy Custom Pulse budget calculations that are unreachable from saved-brawler selection. Preserve canonical weapon bases and any independently supported custom-weapon behavior only when a current product or diagnostic owner is demonstrated.
- Remove total-point values and over-budget outcomes from lobby queue decisions, accepted-build summaries, routed admission, client copy, match telemetry, diagnostics, evidence, and tests.
- Simplify build catalog validation to stable IDs, metadata, slot/conflict rules, parameter bounds, byte ceilings, and saved-brawler-selectable inventory.
- Advance the global protocol/content/build/closeout compatibility floors required by removed serialized fields. Fail stale peers and artifacts closed; add no compatibility decoder or per-message version.
- Update durable product, weapon/build, UX, network, Balance Lab, and operator documentation so no current contract implies a point budget.
- Preserve existing saved profiles and their selected fighter profile, weapon base, ultimate, passives, and weapon parts.

# Constraints

- This is behavior removal and model cleanup, not a balance pass or saved-brawler redesign.
- Do not remove slot limits, mutually exclusive choices, weapon-part inventory/equipment rules, authored parameter bounds, stable IDs, server authority, or catalog byte ceilings.
- Do not change weapon, ultimate, passive, fighter-profile, or weapon-part gameplay values except for deleting inert cost metadata.
- Existing saved profiles must either remain readable unchanged or receive one explicit exact-version migration decision recorded before implementation; do not silently discard player data.
- Preserve the routed product, Practice, Balance Lab, recovery, and headless server paths.
- Do not retain dead aliases or compatibility shims solely to keep the old budget vocabulary compiling.

# Implementation plan

1. Characterize current saved-brawler resolution, serialized shapes, queue admission, fingerprints, persisted profiles, Balance Lab, telemetry, and client presentation.
2. Prove which legacy resolver/Custom Pulse paths have no live product owner; create a separate ticket for any independently supported behavior that cannot be removed safely.
3. Remove the point/cost/budget model from authored and resolved build types, then simplify the sole saved-brawler resolver and catalog validation.
4. Remove downstream queue, protocol, telemetry, diagnostics, client-copy, Balance Lab, fixture, and test assumptions.
5. Advance exact-version compatibility floors and verify old peers fail closed while current saved profiles preserve their choices.
6. Run focused and canonical verification, reconcile durable docs, record learning, and sync Ticket.

# Verification

- Focused build tests prove every valid saved-brawler combination resolves without cost calculation and invalid IDs, duplicate/conflicting passives, bounds, and byte ceilings still reject.
- Profile/storage tests prove existing saved profiles preserve fighter profile, weapon base, ultimate, passives, parts, names, and selected brawler.
- Queue/admission/network tests prove no over-budget decision exists and authoritative loadout installation remains exact.
- Protocol and evidence tests prove removed fields are absent, compatibility floors advance, and stale peers/artifacts fail closed.
- Balance Lab tests prove catalog tuning and apply/reset/persistence no longer expose or depend on point costs.
- Client tests prove Dashboard, brawler creation/editing, queue, loading, match HUD, and result flows contain no point totals or budget copy.
- Run and record `just fmt`, focused tests, `just check`, `just lint`, `just test`, representative `just e2e`, representative `just practice-e2e`, and native saved-brawler/queue smoke.

# Acceptance criteria

- [ ] No active source, authored content, wire shape, UI copy, telemetry, evidence, test, or durable current documentation contains a shared build-point budget or player-facing point cost.
- [ ] `BUILD_POINT_BUDGET`, `build_point_total`, budget-only cost fields/calculations, `BuildResolutionError::OverBudget`, over-budget queue rejection, and total-point reporting are removed.
- [ ] The legacy full-build resolver and frame-passive stat-inference path are removed; saved-brawler resolution is the single authoritative loadout path.
- [ ] Fighter profile, weapon base, ultimate, two passive slots, four weapon-part slots, stable identities, conflicts, parameter bounds, and server authority remain intact.
- [ ] Existing saved profiles preserve all player choices and remain usable through the routed product flow.
- [ ] Required build/content/protocol/diagnostic compatibility floors advance together without decoders or parallel legacy paths.
- [ ] Balance Lab, Practice bots, automation, routed admission, recovery, telemetry, and native client flows pass proportional verification.
- [ ] Durable documentation is reconciled, substantial-work learning is recorded, and `ticket sync` completes without conflicts before closeout.

## 2026-08-28 immediate presentation correction

Playtest exposed live Queue and Match Loading point-cost copy. BRL-k66v removes that misleading copy immediately because it obscures loading diagnosis. This ticket still owns full deletion of `total_points`, the 12-point budget, over-budget outcomes, catalog costs, and related protocol and fingerprint machinery.

# Scope note (2026-11-19)

BRL-egn6 removes saved brawlers and the player part inventory. The point-cost fields, the budget and total-point reporting in `content/catalogs/builds.ron` and the build model are still internal machinery, so this ticket stays valid. Do it after BRL-egn6, or fold it in, and drop any acceptance item about preserving saved-brawler choices.

# Note from BRL-egn6 (2026-11-20)

BRL-egn6 replaced saved brawlers with roster character picks and removed the parts of this machinery its change made unreachable: `resolve_build_recipe`, `BrawlerBuildRecipe`, the custom Pulse `WeaponChoice`, `build_point_total`, `BuildResolutionError::OverBudget` and `MAX_BUILD_CANDIDATE_BYTES`. Still here for this ticket: `BUILD_POINT_BUDGET`, every catalog `point_cost` field, `ResolvedMatchLoadout.total_points` (now always 0) and the unreachable protocol variant `QueueRejection::OverBudget` (removing it is a protocol change; follow docs/08). The description's references to saved brawlers are stale: the product loadout is now a character pick.

# Plan after BRL-egn6 (2026-11-20)

Saved brawlers no longer exist, so the original scope items about the legacy resolver, Custom Pulse, frame-passive inference, `build_point_total`, `BuildResolutionError::OverBudget`, and preserving saved-brawler profiles are already done (BRL-egn6) or void. The acceptance items about saved-profile preservation are void. The product loadout is a roster character pick resolved by `builds::resolve_loadout`; it never computed or enforced points.

## Removal list (grep of the tree at 5db2bbf8, excluding `references/`, `docs/implementation/`, `docs/tasks/`)

Build model and content:
- `src/builds/definitions.rs`: `BUILD_POINT_BUDGET`; `WeaponPointCost` and `BuildCatalog.weapon_costs` (exists only to carry costs; its "exactly covers the weapon catalog" cross-check goes with it, the per-preset `resolve_preset` check stays); `point_cost` on `UltimateDefinition` and `PassiveDefinition`; the "invalid authored point cost" validation; `point_cost`/`total_points: 0` in `resolve_loadout`. `BUILD_CATALOG_SCHEMA_VERSION` 18 -> 19 because the authored file shape changes.
- `src/builds/mod.rs` re-exports of `BUILD_POINT_BUDGET` and `WeaponPointCost`.
- `src/builds/model.rs`: `ResolvedUltimate.point_cost`, `ResolvedPassive.point_cost`, `ResolvedMatchLoadout.total_points` (replicated; wire shape change).
- `content/catalogs/builds.ron`: `schema_version`, the `weapon_costs` table, every ultimate/passive `point_cost`.
- `src/builds/tests.rs`: cost-coverage and cost-bound tests; inventory assertion on `weapon_costs`.
- `src/combat/attack.rs` test fixture `point_cost`.

Queue/protocol:
- `src/lobby/queue.rs`: `QueueRejection::OverBudget { used, budget }`, its wire-validation arm and tests.
- `packages/brawler-client/src/flow/reducer/match_flow.rs`: the over-budget copy ("The selected build uses N of M points").
- `src/server/lobby/queue.rs`: test `saved_brawler_admission_does_not_apply_the_legacy_point_budget` (nothing left to prove).
- `src/protocol.rs`: `SUPPORTED_PROTOCOL_VERSION` 54 -> 55 and its test.

Telemetry/evidence:
- `src/matchplay/telemetry.rs` `MatchParticipantSummary.total_points`, `src/matchplay/server.rs`, `src/matchplay/tests.rs`.
- `src/server/verification.rs` `build_total_points` report key; `scripts/network-match.sh` required-key list.
- `tests/network/movement.rs` forged-loadout `total_points`.

Docs:
- `docs/03-weapons-and-abilities.md` (resolution note naming BRL-01c8; balance checklist "build budget"), `docs/08-network-architecture.md` protocol 55 entry.

Not affected: Balance Lab tuning snapshot/persistence carry no costs (tuning is applied onto the embedded catalog), so its snapshot and persistence schemas stay. `MatchCharacterSnapshot` holds only the choice and `SelectedBuild`, whose recipe fingerprint never included costs, so manifest snapshot schema 8 stays. `BUILD_FINGERPRINT_FORMAT_VERSION` stays; the catalog fingerprint material changes on its own because the serialized catalog loses fields and its schema version moves. Protocol-registry fingerprint is unchanged (type paths unchanged); the global protocol version guards the new encoding.

Expected: gameplay content fingerprint changes; protocol 55; no gameplay values change.

# Implementation and verification (2026-11-20)

Commits (worktree branch `worktree-agent-a9be139af117bfa24`, on main 5db2bbf8):
- 255d50e6 Drop build point costs from the catalog, resolved loadout, telemetry and evidence
- e60a2cf3 Remove the unreachable over-budget queue rejection; protocol 55
- e4ac54a1 Document protocol 55 and point-free build catalogs

Done as planned in the removal list above. Decisions: the `weapon_costs` table existed only for costs, so it and its "exactly covers the weapon catalog" cross-check are gone; the replacement test proves an additive weapon preset needs no build-catalog entry. The client test that asserts Match Loading copy contains no "points" stays as a regression guard. `BuildRevision(13)`, `BUILD_FINGERPRINT_FORMAT_VERSION` 17 and recipe fingerprints are unchanged; no gameplay value changed.

Compatibility: protocol 54 -> 55 (`ResolvedMatchLoadout`, `ResolvedUltimate`, `ResolvedPassive`, `QueueRejection` encodings; registry fingerprint unchanged). Build catalog schema 18 -> 19, so the gameplay content fingerprint changes. Match character snapshot 8, manifest v5, control IPC v7, route envelope, Balance Lab snapshot 21 / persistence 15 and closeout 5 unchanged (none carried costs). `build_total_points` leaves the verification report and `scripts/network-match.sh`.

Verification:
- `just lint`: pass.
- `just test`: first run had 8 network failures (`lobby_takeover::*`, `map_publishing::*`, `queue::a_handed_off_lobby_session_closing_first...`), all a bare `Main::run_main` panic under host load (load avg ~9 from parallel agents); each passed in isolation, the full network suite then passed 136/136, and a second full `just test` passed (exit 0, including the performance gate).
- `BRAWLER_ROUTED_BIND=127.0.0.1:5170 BRAWLER_SERVER_DATA_DIR=target/01c8/practice just practice-e2e`: pass (Practice wipeout-1v1 reached Active).
- `just rejoin-e2e` on 127.0.0.1:5171: pass (rejoined the same running match).
- `just e2e 2` on 127.0.0.1:5172: pass (1 exact 1v1 roster reached Active).
- Not run: `scripts/network-match.sh` (legacy direct-UDP baseline; only its required-key list changed). No native/manual playtest needed: no player-visible change.

Learning: the first `just test` network failures were host-load flakes, not this change; rerun focused suites before investigating when every failure is a lobby-hosting test with a bare `run_main` panic.

## Merge verification (main, 2026-11-20)

Cherry-picked the three commits onto main. On the combined tree: `just lint`, `just test` (including network and performance gates), `just practice-e2e`, `just rejoin-e2e` and `just e2e 2` all pass on the first run. Protocol is 55.
