# BRL-0060 remediation specification

## Outcome

Close every actionable finding from the BRL-0052 implementation review so that Balance Lab edits are truthful at runtime, routed worker behavior is resilient and mode-complete, content-driven bot/audio/VFX behavior is safe, the audited extension seams are real, the projectile coordinator has characterized and focused responsibilities, and current native evidence supports the delivered implementation.

## Constraints and decisions

- Preserve dedicated-server authority, client-intent-only networking, stable wire identities, current protocol compatibility, and headless server feature isolation.
- Do not add a new game mode, weapon, ability, UI flow, protocol variant, or generalized framework.
- Treat catalog application as one validated transaction. A successful Balance Lab map edit must refresh the authoritative runtime representation consumed by effect-tile systems; a failed refresh must leave the prior catalog/runtime state intact.
- Treat `WouldBlock` on routed control output as recoverable backpressure. Heartbeats are considered sent only after successful enqueue and are retried without busy-spinning or terminating the worker.
- Use one immediate frame-local audio reservation owner so deferred entity creation cannot exceed the applicable configured cap.
- Reject or deterministically fall back from VFX lifetime/renderer combinations that cannot execute correctly for the cue family. Renderer mechanics own renderer-specific orientation and placement offsets; cue producers own only semantic anchors.
- Characterize sticky projectile behavior through the production sweep before further decomposition. Preserve fixed-tick order, deferred-command boundaries, deterministic ordering, outcome publication, and telemetry.
- Extend `ModeDescriptor` with the smallest concrete topology/anchor policy needed by current modes and synthetic extension tests; map resolution delegates to that policy rather than switching on built-in mode IDs.
- Derive combat time conversions from the canonical simulation tick definition. Bot content validation must reject values that exceed engine/map-safe domains or overflow squared-distance use.
- Native evidence must be captured from the final implementation commit/state in both normal and reduced-effects configurations and must include an explicit audible-cue observation.

## Work plan

### Stage 1 — authoritative and routed correctness

1. Add a failing Balance Lab regression that applies non-default effect-tile tuning and observes the active authoritative `ResolvedMap`/fighter effect behavior after the transaction. Refresh the runtime map state atomically from the validated revised catalog.
2. Add a Heist automatic-transition parser/startup regression and remove the supervisor/worker mode-token divergence by using the canonical registered mode mapping.
3. Add a full-control-queue plus due-heartbeat regression. Make heartbeat `WouldBlock` retryable and keep fatal handling for malformed/disconnected control streams.

### Stage 2 — bounded allocation and content safety

4. Add a same-frame multi-producer audio-cap regression and introduce a shared reservation owner reset once per frame.
5. Add VFX catalog validation/resolution tests for deadline-less cue families, renderer-family transform tests, and reduced-effect anchor parity; implement rejection/fallback and renderer-owned mechanics.
6. Add bot-profile extreme-value tests and enforce documented upper bounds plus overflow-safe squared-distance invariants.
7. Replace remaining authoritative/evidence `60.0` conversions with the canonical simulation tick and add a consistency test.

### Stage 3 — characterization and audited extension seams

8. Add production-sweep unit coverage for sticky expiry, attachment, per-carrier/global caps, and same-carrier chain detonation, plus one routed Sticky Blomb scenario.
9. Extract focused straight/lob/splash/sticky planning/execution helpers from `sweep_composed_projectiles` until the schedule-facing coordinator visibly owns only phase orchestration, deterministic merge/order, and publication.
10. Add descriptor-owned topology policy, migrate Wipeout/Hot Zone/Heist descriptors, and prove a synthetic registered mode can reuse an existing topology without editing map resolution.

### Stage 4 — verification and closeout

11. Run focused role-specific tests after each change, then `just check`, `just test`, `just lint`, and `git diff --check`.
12. Capture native current-state evidence for normal and reduced VFX plus audible cues. Record any subjective limitations and requested observations.
13. Reconcile durable documentation where behavior changed, record the learn-from-errors review, run `ticket sync`, and keep the ticket in `doing` until all evidence is satisfied.

## Acceptance criteria

- A successful live Balance Lab effect-tile edit changes authoritative behavior in the running Practice worker without a process restart; invalid edits remain atomic and non-mutating.
- `--automatic-transition-driver --mode heist` resolves to the canonical Heist definition and starts successfully.
- A due heartbeat against a full nonblocking control queue does not terminate the worker; enqueue is retried and later succeeds when capacity returns.
- Simultaneous audio producers cannot exceed the configured allocation cap in one frame.
- Every catalog-valid VFX profile either produces a visible, correctly oriented effect for its supported cue family or follows a deterministic validated fallback. Reduced effects preserve semantic anchor placement.
- Unsafe bot geometry/timing values, including squared-distance overflow cases, fail content loading with actionable validation errors.
- No audited authoritative combat/evidence conversion uses a literal 60 Hz assumption.
- Sticky production-sweep and routed behavior are characterized before and after decomposition with unchanged deterministic outcomes.
- The projectile schedule system no longer mixes family-specific planning/execution algorithms with phase orchestration.
- A synthetic fourth mode can reuse a supported topology through descriptor/registry data without another built-in-ID branch in map resolution.
- Native evidence identifies the final implementation state, covers normal and reduced effects, and records audible cue verification.
- Full canonical check, test, lint, network, performance, feature-isolation, and diff checks pass.

## Feedback and closeout

Every review item is closed as implemented, explicitly deferred to a separately linked ticket, or rejected with evidence. Required native feedback remains open in this ticket until observed. Completion requires verification evidence, durable documentation reconciliation, a learn-from-errors note, and a conflict-free `ticket sync`.


## Implementation progress

### Stage 1 checkpoint — 2026-08-30

- Implemented truthful Balance Lab map tuning: persisted tuning is loaded before authoritative map instantiation, and successful fixed-tick applies re-resolve the selected preset from the validated revised catalog and atomically replace both `MapCatalogResource` and the active `ResolvedMap` before success is published. The regression asserts resolved effect-tile behaviors.
- Replaced the automatic-transition worker's closed mode parser with the canonical mode descriptor registry, including Heist.
- Unified routed control-output backpressure classification. A due heartbeat retains its deadline and sequence under a full queue, does not terminate the worker, and succeeds after capacity returns.

Verification at this checkpoint:

- `cargo test --locked --no-default-features --features server --lib server::worker::tests` — 18 passed.
- `cargo test --locked --no-default-features --features server --lib` — 416 passed.
- `cargo test --locked --no-default-features --features balance-lab --lib` — 438 passed.
- `just check` — routing, client, server, network-test, Balance Lab, and Balance Lab web checks passed.
- `cargo fmt --all` and `git diff --check` — passed.

Stages 2–4 remain open. No native evidence has been claimed.

### Stage 2 checkpoint — 2026-08-30

- Added one frame-local audio reservation resource shared by all seven producers, reset before production. Deferred one-shot spawns now reserve capacity immediately, and the multi-producer regression proves the configured cap cannot be exceeded in one frame.
- Tightened VFX catalog validation so deadline profiles are valid only for Reveal Scan, validated their fixed-lifetime fallback, and made runtime resolution use that fallback when authoritative deadline metadata is absent. Renderer families now own ground-ring orientation and placement mechanics; semantic anchors remain cue-owned, and reduced sphere scale preserves center height.
- Bounded bot world distances from map dimensions, rejected squared-distance overflow, and constrained waypoint/route thresholds below one cell plus perimeter inset to a map-derived maximum.
- Replaced authoritative projectile, validation, cone-fill, maximum-step, and evidence conversions with `SIMULATION_TICK_HZ`. Remaining `60.0` occurrences under `src/combat` are test geometry or authored numeric fixtures, not time conversions.

### Stage 3 checkpoint — 2026-08-30

- Added `ModeTopologyPolicy` to the canonical mode descriptor. Map resolution now delegates anchor validation and Heist access validation to the descriptor policy rather than branching on built-in IDs. Synthetic descriptor and anchorless resolver regressions demonstrate reuse without another ID branch.
- Added production-scheduled sticky sweep regressions for expiry arming, impact attachment, per-owner and process-global caps, and same-carrier primary chain detonation.
- Added a routed network Sticky Blomb scenario covering authoritative attachment, client replication of the exact fuse state, and server-owned detonation damage.
- Extracted sticky sweep allocation, arming, attachment, and chain decisions into `StickySweepState`; the fixed-schedule coordinator retains ordering and publication. Straight/lob/splash extraction remains open and Stage 3 is not complete.

Verification at these checkpoints:

- `cargo test --locked --no-default-features --features server --lib` — 421 passed.
- `cargo test --locked --no-default-features --features client --lib` — 481 passed.
- `cargo test --locked --no-default-features --features network-test --test network combat_sticky::sticky_blomb_attaches_replicates_and_detonates_from_the_authoritative_fuse -- --exact --nocapture --test-threads=1` — 1 passed.
- Focused sticky production-sweep coverage — 3 passed; focused audio, VFX catalog/effects, bot, mode, map, combat definition/delivery/evidence tests passed.
- `just check`, `cargo fmt --all`, and `git diff --check` — passed.

Open work: complete straight/lob/splash coordinator decomposition, run full canonical test/lint/network/performance gates, capture current-state native normal/reduced/audible evidence, reconcile durable documentation, and record closeout learning. Ticket remains `doing`.
