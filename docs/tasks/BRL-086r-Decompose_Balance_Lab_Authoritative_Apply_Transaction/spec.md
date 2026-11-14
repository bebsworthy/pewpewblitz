# Technical specification

## Outcome and scope

Refactor only the development-only authoritative Balance Lab Apply/Restore transaction currently implemented by `server::balance_lab::apply_balance_lab_transaction`. Move the transaction implementation into a private `server::balance_lab::transaction` module and make its internal lifecycle explicit:

1. dequeue and normalize one request;
2. prepare and validate all fallible gameplay state from immutable inputs;
3. persist the accepted candidate or clear persisted defaults;
4. commit prevalidated authoritative catalogs/map/rules/fighter runtime atomically;
5. stage the existing same-tick match restart and publish the result.

This is an organization, rollback-characterization, and testability phase. It must not change Balance Lab snapshot or persistence schemas, HTTP/API shapes, validation policy, tuning values, gameplay authority, fixed schedule placement, restart timing, protocol, or player-visible behavior.

## Current problem

The approximately 240-line fixed-tick system currently owns command dequeue, Apply/Restore normalization, schema/revision/role/root/restart preconditions, catalog and mode validation, manifest decoding, roster/runtime completeness, revised loadout resolution, map re-resolution, revision allocation, filesystem persistence, authoritative resource and fighter mutation, restart staging, and result publication in one function with broad complexity suppressions.

The order is safety-critical: all ordinary fallible validation must finish before persistence; persistence must succeed before ECS mutation; the staged restart must be consumed by mode/environment/common restart phases in the same FixedUpdate. Those contracts are implicit in one control flow and lack focused rollback tests.

## Module and schedule ownership

- Add private `src/server/balance_lab/transaction.rs`.
- Keep `server::balance_lab::mod.rs` as the feature composition, runtime/HTTP/startup/schema surface.
- Keep one schedule-facing `apply_balance_lab_transaction` system registered in `FixedUpdate`, `MatchRestartSet::Prepare`, before `matchplay::prepare_match_restart`.
- Do not add systems, schedules, events/messages, observers, deferred-command boundaries, transaction resources, background work, or plugins.
- Keep persistence helpers in `persistence.rs`; keep editor metadata and HTTP concerns out of this phase.
- Default new transaction shapes/helpers to private. Re-export only the system to the parent module if required for schedule registration.

## Transaction architecture

### Request normalization

Represent Apply and Restore as one private requested-transaction shape containing transaction ID, expected revision, owned candidate snapshot, and an explicit Save-or-Clear persistence action.

Preserve:

- one non-blocking command dequeue per fixed tick;
- Apply schema validation and exact rejection text;
- Restore using the immutable startup baseline;
- stale revision rejection before gameplay preparation or persistence.

### Preparation

Introduce an owned `PreparedBalanceLabTransaction` produced by a preparation helper from immutable resources/queries. It must contain every value needed after persistence:

- transaction ID and accepted candidate;
- next applied revision;
- rebuilt Build, Weapon, and Map catalogs;
- re-resolved authoritative Practice map;
- stable `(Entity, ResolvedMatchLoadout)` roster plan;
- condition rules and optional mode tuning input already validated;
- previous match ID and restart tick;
- Save-or-Clear persistence action.

Use a private focused mutable `QueryData` view for the exact fighter runtime component contract when it improves clarity; preparation must use its read-only view and commit its mutable view. Do not introduce a broad gameplay aggregate component or second runtime model.

Preparation owns all ordinary failure paths and preserves exact precedence/messages for:

- role manifest and match root availability;
- existing pending restart;
- snapshot/catalog/mode-specific validation;
- manifest build decoding and duplicate detection;
- complete human+bot manifest versus instantiated fighter cardinality;
- exact fighter runtime completeness;
- revised loadout resolution;
- selected-map source preset and map re-resolution;
- applied revision exhaustion.

No authored catalog, resolved map, rules, fighter runtime, restart state, next match identity, applied snapshot, or revision may mutate during preparation.

### Persistence

Persist the fully prepared transaction before authoritative mutation:

- Apply atomically saves the candidate with its next revision.
- Restore idempotently clears the persisted file.
- A persistence error publishes the existing Rejected result and leaves every authoritative resource/component, restart slot/policy, next match identity, applied snapshot, and revision unchanged.
- Do not change persistence schema 13, snapshot schema 19, paths, durability behavior, or error text.

### Commit, restart, and publication

After successful persistence, authoritative application is an infallible transaction over prevalidated values:

- install rebuilt catalogs and resolved map;
- install condition rules and optional Heist safe-health tuning;
- update each fighter's SelectedBuild, ResolvedMatchLoadout, ResolvedFighterStats, FighterBody, ResolvedWeapon, ResolvedUltimate, ResolvedPassives, CurrentHealth, ready WeaponState, and HealthRecoveryState;
- set `RestartBuildPolicy::Retain`;
- allocate the next match ID at the preserved point;
- stage the exact `PendingMatchRestartSlot { previous_id, next_id, restart_tick }`;
- update runtime applied snapshot/revision;
- publish the exact Applied result and clear pending state.

The existing precondition plus single-system exclusive resource ownership makes post-persistence restart staging failure an internal invariant breach, not a recoverable user rejection. Express that invariant explicitly so disk cannot silently advance while ECS reports a normal rejection. Do not broaden this ticket into match-ID exhaustion policy.

## Preserved contracts

- Dedicated-server gameplay authority and Practice-only Balance Lab gating remain unchanged.
- Snapshot schema 19, persistence envelope 13, JSON casing, HTTP endpoints, transaction status shapes, validation bounds, and catalog fingerprints remain unchanged.
- Apply and Restore preserve exact revision semantics; Restore increments the current revision rather than resetting it.
- Every rejection preserves exact status publication, pending clearing, revision/applied state, catalogs/map/rules/fighters, restart state, and persisted data according to its pre-persistence point.
- Human and bot roster resolution remains stable by PlayerId and must be complete before persistence.
- FixedUpdate schedule ordering and same-tick Prepare -> ModeReset -> EnvironmentReset -> Commit restart visibility remain exact.
- No native UI, protocol, presentation, audio, VFX, balance, editor schema, or content changes.

## Tests and verification

Add or retain:

1. success characterization proving one fixed-tick apply re-resolves the complete Practice roster atomically;
2. persistence-failure rollback test covering catalogs, map, rules, fighter runtime, restart slot/policy, next match ID, applied snapshot/revision, pending state, and Rejected publication;
3. already-pending-restart rejection before persistence with disk and authoritative state unchanged;
4. representative stale revision or invalid/incomplete roster rejection with no mutation;
5. Restore success proving persisted file removal, baseline restoration, revision increment, restart staging, and Applied publication;
6. schedule contract proving Balance Lab staging remains before ordinary match restart preparation and is consumed in the same fixed tick;
7. existing snapshot validation, persistence migration/roundtrip/oversize, HTTP queue, and revised-catalog network tests remain green.

Run:

- `cargo fmt --all -- --check`
- `git diff --check`
- focused Balance Lab tests
- strict Balance Lab Clippy with `-D warnings`
- combined Balance Lab/network revised-catalog smoke
- `just check`
- `just lint`
- `just test`

Native evidence is not required if schemas, UI, timing, tuning values, and player-visible behavior remain unchanged.

## Exclusions

Balance Lab editor descriptor co-location, schema changes, new tuning fields, HTTP/UI changes, persistence migration changes, general transaction frameworks, match restart redesign, NextMatchId exhaustion policy, background persistence, protocol changes, client presentation, and unrelated lobby/client-flow/presentation decomposition are excluded.

## Implementation record — 2026-08-31

The development-only Apply/Restore path now lives in private `server::balance_lab::transaction` while the parent module retains runtime, HTTP, startup, schema, validation, and visible schedule composition.

One unchanged FixedUpdate system now coordinates explicit private phases:

1. normalize one dequeued Apply or Restore request;
2. prepare a fully owned transaction from immutable catalog/map/runtime/query views;
3. save the accepted snapshot/revision or clear persisted defaults;
4. apply only prevalidated catalogs, map, rules, and fighter projections;
5. allocate/stage the restart, update applied state/revision, and publish Applied.

A focused mutable `QueryData` names the exact ten-component fighter contract. Roster facts are sorted by PlayerId, validated with the original per-fighter rejection precedence, and then checked for exact manifest coverage. Persistence remains before NextMatchId allocation or any authoritative mutation. A post-persistence restart-stage failure is now an asserted internal invariant rather than a normal rejection that could leave disk ahead of ECS.

No schema, HTTP/API, tuning, protocol, plugin, schedule, ApplyDeferred, presentation, or player-visible contract changed.

## Acceptance and review evidence

New transaction tests prove:

- save failure leaves Build/Weapon/Map catalogs, ResolvedMap, condition and optional Heist rules, all ten fighter runtime fields (including absent-component state), restart slot/policy, runtime and published applied/revision state, NextMatchId, and disk sentinel unchanged while publishing the exact Rejected result;
- an already-staged restart and a stale revision reject before persistence or identity allocation;
- duplicate/missing PlayerId coverage rejects deterministically at the final roster-completeness boundary;
- unknown complete and incomplete extra fighters preserve the original no-snapshot versus runtime-incomplete rejection precedence;
- Restore clears persisted tuning, rebuilds the validated Balance Lab baseline catalogs, increments revision from 3 to 4, stages restart, and publishes Applied;
- Prepare-installed state is visible to ModeReset and EnvironmentReset and the staged slot is consumed by Commit in the same FixedUpdate.

Independent review initially found two consequential test/logic issues: an upfront exact-roster check changed legacy rejection precedence, and the incomplete-runtime test snapshot unwrapped the intentionally missing component. Both were corrected. Final review found no P0, P1, or P2 issue and confirmed immutable preparation, persistence/ID/mutation order, optional Heist parity, exact roster coverage, rollback completeness, and same-tick restart visibility.

Native evidence was not repeated because this is a headless organization/rollback phase with unchanged schemas, values, timing, UI, and player-visible behavior.

## Verification evidence

Passed:

- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo check --locked --no-default-features --features balance-lab --all-targets`
- transaction tests 7/7
- full Balance Lab module tests 29/29
- strict Balance Lab all-target Clippy with `-D warnings`
- revised-catalog combined Balance Lab/network smoke 1/1
- `just check`
- `just lint`
- `just test`
  - routing library 83/83 plus routing process suites
  - client 543/543
  - server/Balance Lab all-target suite 520/520
  - combined Balance Lab/network smoke 1/1
  - separate-App network 97/97
  - performance 12/12

## Learn-from-errors review

The first exact-roster hardening compared PlayerId sets before running the historical per-fighter validation loop. Although valid behavior was unchanged, invalid-state rejection precedence changed. Cause: treating deterministic set equality as a preliminary invariant instead of locating it at the existing cardinality boundary. Prevention: when stabilizing query order, first sort the same facts, then preserve every existing validation stage, and add exact-coverage checks only at the original completeness point.

The first incomplete-runtime rollback test reused a snapshot helper that assumed all ten components existed, so it could panic before exercising the system. Prevention: rollback snapshots for negative ECS tests must represent intentionally absent components with Option rather than encoding the success-path invariant.

Restore test expectations also initially compared rebuilt Balance Lab catalogs to embedded shipping catalogs, overlooking the existing development-safe 128-unit world-effect ceiling. Prevention: compare transaction output against the canonical validation/resolution function, not a related source catalog whose policy projection differs.
