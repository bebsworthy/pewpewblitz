# Context

V7 removed the shared 12-point budget from the player-facing saved-brawler workflow, and BRL-0007 later removed the superseded full-build editor, persistence, selection transaction, named presets, and product copy. Saved-brawler resolution explicitly bypasses budget enforcement and reports zero total points.

Private legacy machinery still remains:

- `BUILD_POINT_BUDGET`, `build_point_total`, `BuildResolutionError::OverBudget`, weapon/ultimate/passive `point_cost` fields, and Custom Pulse cost increments;
- the legacy `resolve_build_recipe` path that enforces the retired budget and infers fighter profiles from frame passives;
- `total_points` in resolved/accepted loadout summaries, queue validation and rejection shapes, match telemetry, verification output, and client loading/queue copy;
- catalog validation and tests that continue to assign balance meaning to costs no longer used by the product.

This residue caused BRL-0032 planning to incorrectly describe a new ultimate as having a 4-point product cost.

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

Playtest exposed live Queue and Match Loading point-cost copy. BRL-0003 removes that misleading copy immediately because it obscures loading diagnosis. This ticket still owns full deletion of `total_points`, the 12-point budget, over-budget outcomes, catalog costs, and related protocol and fingerprint machinery.
