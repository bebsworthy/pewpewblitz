# Outcome

`process_client_hellos` coordinates ordered messages, invokes independently testable planning, commits one shared fighter assembly path, sends the outcome, and transitions the session. Human and Practice fighters reuse the same invariant authoritative runtime construction without weakening role-specific ownership.

# Scope and decisions

1. Characterize and preserve current rejection precedence: protocol version, build version, registry fingerprint, content fingerprint, routed manifest identity, match phase, process capacity, direct team capacity, loadout resolution, and identifier exhaustion.
2. Extract focused `validate_match_hello`, `resolve_join_loadout`, and `plan_match_join` helpers with explicit bounded inputs/results. Planning owns deterministic player/team/loadout/display/spawn decisions but performs no ECS mutation.
3. Introduce `AuthoritativeFighterSpawnSpec` and one `spawn_authoritative_fighter` helper for the common identity, health/economy, ability/passive/effect state, transform/physics, match/map membership, display, replication/interpolation, and spawn assignment bundle.
4. Keep control ownership explicit at the assembly boundary: connected fighters receive session-scoped `ControlledBy`; Practice fighters receive local `ActionState`, input freshness, and `PracticeBotController`. No client gains authority.
5. Preserve allocation order, stable IDs, routed participant identity, direct diagnostic loadout rotation, team counts, spawn clearance, idempotent replays, diagnostics, deferred-command behavior, and exact public/wire types.
6. Keep this ticket organization-only: no balance, schema, routing, protocol, presentation, or player-visible behavior changes.

# Acceptance criteria

- `process_client_hellos` no longer contains compatibility/loadout/team/spawn algorithms or fighter bundle construction inline.
- Planning helpers have focused tests for rejection precedence, routed/direct selection, loadout failure, allocation exhaustion, team/spawn determinism, and plan output.
- Human and Practice creation call the same authoritative fighter assembly helper for their overlapping contract; focused ECS tests prove common and controller-specific component sets.
- Existing direct/routed admission, reconnect, Practice all-mode, replication, loadout, map spawn, lifecycle, network, and performance tests remain green.
- `cargo fmt --all`, `git diff --check`, role-specific checks, `just check`, and `just lint` pass.

# Verification

- Focused server admission and fighter assembly unit tests.
- Existing `server::admission` and production startup tests.
- Relevant separate-App/network admission, lifecycle, loadout, map, and Practice cases.
- `cargo check --no-default-features --features server --lib`
- `cargo check --no-default-features --features client --lib`
- `just check`
- `just lint`

## Implementation evidence (2026-08-31)

Implemented pure hello validation, routed/direct loadout resolution, deterministic join planning, batch-state collection, and explicit join commit. Rejection precedence and allocation ordering are preserved. Connected and Practice creation now share controller-neutral authoritative fighter assembly; their callers add only session ownership or local bot input/controller state.

Focused verification passed: 17 server admission tests and the fighter assembly ECS contract test. Role-specific client/server checks, `just check`, `just lint`, and `git diff --check` passed. Canonical `just test` passed routing, client (500), server (461), Balance Lab (483), the Balance Lab network case, and all 95 standard network cases. Its first performance leg had one timing outlier at 24.403 ms p95 for the 100-fighter/200-projectile case; an immediate isolated rerun passed all 12 gates with that case at 8.831 ms p95.

## Learn-from-errors review

The first draft selected direct-team capacity before match-phase and process-capacity checks. A precedence audit caught that behavioral drift before integration; validation now defers direct team selection until the established point. The first shared assembly draft also represented controller ownership as a closed enum; it was corrected to a controller-neutral common bundle so future controller plugins remain additive. Finally, the large routed participant value triggered Clippy large-enum feedback; the validated plan now boxes that bounded snapshot and loadout resolution borrows it.
