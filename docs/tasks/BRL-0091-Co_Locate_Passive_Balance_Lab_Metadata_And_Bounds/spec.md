# Technical specification

## Outcome

Deliver the smallest independently reviewable BRL-0070 Stage 6.3 slice: a passive-family pilot that removes duplicated authoritative numeric bounds from the Balance Lab editor, makes passive metadata evolution compile-visible, and adds bidirectional schema-to-editor coverage. Shared gameplay/build modules own only passive rule constants; Balance Lab retains labels, grouping, units, controls, paths, ordering, and serialized editor shapes.

This phase must preserve byte-for-byte editor manifest behavior, snapshot/persistence formats, schema versions, balance values, validation results, UI behavior, protocol, and public module paths.

## Shared passive rule ownership

Co-locate crate-private, family-specific numeric range constants or one small passive bounds descriptor beside `PassiveParameters` in `src/builds/model.rs`. Cover:

- Adrenal duration, rearm, and movement bonus;
- Close Quarters near/far distance and near/far damage;
- Quick Cycle refill duration;
- Tenacity slow duration;
- the three elemental-resistance passive values.

Use those shared bounds in `src/builds/definitions.rs::validate_passive_definitions` and in Balance Lab NumberSpec construction. Cross-field invariants remain validator-owned: Adrenal rearm >= duration, Close Quarters near distance < far distance, and near damage > far damage. Do not move editor types, copy, paths, or scaling into shared gameplay modules. Do not introduce reflection, a generic schema framework, trait-per-passive, proc macro, or public API.

## Balance Lab editor boundary

Create private `src/server/balance_lab/editor/passives.rs` and move only passive manifest projection there. The child owns the exhaustive `PassiveParameters` projection to external variant/path names and adapter-owned descriptor labels/groups/units/controls. `editor.rs` retains `BalanceLabEditorManifest`, serialized descriptor/wire types, shared path/field helpers, top-level manifest order, and tests/composition.

Expose only the minimum editor helper/types to the child with `pub(super)`. Match every `PassiveParameters` variant with exhaustive field destructuring and no `{ .. }` wildcard for numeric variants, so adding a field to an existing variant fails compilation until metadata is addressed. Parameterless LightweightFrame and ReinforcedFrame remain uneditable.

Preserve exact observable output:

- passive vector/index ordering;
- externally tagged variant path segments and snake_case field tails;
- the existing `EditorSection::Ultimates` placement;
- subject key/display name, group, label, NumberSpec unit/scale/min/max/step/control/help;
- the total field count and relative manifest order.

Do not bump snapshot, persistence, editor, content, or protocol versions because no serialized output changes.

## Coverage

Add bidirectional passive coverage:

1. For every passive in the embedded snapshot, serialize `parameters`, recursively collect every numeric leaf path, and assert exact equality and uniqueness against manifest descriptor paths under that passive root. The current inventory has 12 editable numeric leaves across the parameterized passive definitions; the two unit variants yield zero.
2. Assert exact ordered passive descriptor parity, either through a focused serialized fixture or explicit expected path/descriptor data, so path casing, tags, section, subjects, grouping, labels, scaling, bounds, steps, controls, and help remain unchanged.
3. Retain the existing whole-manifest path-resolution and total-field-count tests.
4. Add or retain boundary tests proving shared constants drive accepted/rejected values plus Adrenal and Close Quarters cross-field invariants.

The production implementation must benefit from the shared rule seam; do not add a general abstraction solely for testing.

## Verification

Run and record:

- `cargo fmt --all -- --check`
- `git diff --check`
- focused build/passive validation tests
- focused `server::balance_lab::editor` tests
- Balance Lab all-target test/check and strict Clippy
- client and server role checks because the shared build model changes
- `just check`
- `just lint`
- `just test`

Independent review must confirm no manifest/validation drift, exhaustive compile-time coverage, minimal visibility, and no client/server role contamination. No native evidence is required if output and validation are exact.

## Follow-up decision

After this pilot, audit the ultimate and weapon families against the proven pattern. Create another child only where duplicated authoritative bounds or wildcard metadata projection demonstrates the same maintenance failure. Do not expand this phase to a wholesale editor rewrite or low-value file split.

## Exclusions

Weapon, ultimate, fighter, effect-tile, map/world-object editor metadata; web UI; Balance Lab apply transaction; snapshot/persistence schema; balance changes; new fields; new passive behavior; protocol changes; native polish; and public API changes are excluded.

## Compatibility clarification: passive resistance minimum

The audit found a pre-existing observable mismatch: authoritative passive validation accepts resistance basis points only in `1..=6_000`, while the current Balance Lab manifest intentionally or accidentally advertises a display minimum of `0.0%` through `NumberSpec::resistance_basis_points()`. This organization phase must preserve both existing contracts exactly and must not silently change validation or editor output.

Share the authoritative `1..=6_000` rule for gameplay validation and source the editor maximum from it. Preserve the existing editor minimum through an explicit, documented passive-resistance compatibility adapter derived as `authoritative_min.saturating_sub(1)`. The adapter must be local to Balance Lab presentation metadata and covered by exact manifest parity. Do not describe the zero value as authoritative or reusable gameplay policy. A separate linked backlog ticket owns deciding whether to correct the displayed minimum and advance any required editor contract/version with player-visible evidence.

## Correction: passive resistance minimum

The preceding compatibility clarification is superseded and must not be implemented. A baseline-source recheck showed that passive descriptors use `NumberSpec::basis_points(1, 6_000)`, producing the same authoritative 0.01% minimum and 0.01 step; the earlier audit had accidentally inspected the distinct fighter-resistance helper. Exact manifest parity caught the mistake before closeout. BRL-0091 must use the shared authoritative `1..=6_000` bounds directly with no compatibility adapter. BRL-0092 is canceled as unnecessary.

## Implementation and evidence — 2026-08-31

Implemented the passive-family pilot without observable behavior, schema, protocol, or public-API changes:

- added crate-private typed passive bounds beside `PassiveParameters` and reused them in authoritative validation and Balance Lab projection;
- extracted exhaustive passive descriptor construction to private `server::balance_lab::editor::passives` ownership;
- preserved all 12 ordered serialized passive descriptors exactly, including the historical `EditorSection::Ultimates` placement;
- added bidirectional schema-leaf coverage, independent literal serialized descriptor parity, lower/upper boundary rejection for every numeric field, and all existing cross-field invariants.

Verification passed:

- focused passive boundary matrix: 1 passed;
- focused independent manifest parity: 1 passed;
- focused bidirectional 12-leaf coverage: 1 passed;
- `cargo fmt --all -- --check` and `git diff --check`;
- Balance Lab all-target check and strict all-target Clippy;
- `just check`;
- `just lint`;
- `just test`: routing 83 unit + 4 supervisor + 5 process + 5 runtime-process + 3 isolation; client 551 + map API 1; server 494 + map API 1; Balance Lab 525 + map API 1; revised-catalog network smoke 1; network 97; performance 12, all passing.

Independent follow-up review confirmed exhaustive compile-visible coverage, exact manifest and validation parity, minimum visibility, no role contamination, and no remaining P0/P1/P2 findings.

## Learning review

The first audit incorrectly conflated the fighter-resistance `NumberSpec` helper with passive-resistance metadata and proposed a compatibility adapter plus BRL-0092. The independent literal parity oracle exposed that the passive baseline already matched authoritative `1..=6_000`; the adapter was removed and BRL-0092 canceled before implementation. Prevention: compare the exact family-specific serialized baseline before describing drift or creating migration work.

Independent review also found that the initial parity expectation reused production formatting helpers and that the first boundary matrix did not probe both sides of every field. The tests were strengthened to use literal JSON and exhaustive lower/upper rejection cases. Reusable lesson: a parity oracle must not share the transformation under test, and shared-bound extraction needs mutation-resistant boundary coverage per field, not merely representative family cases.
