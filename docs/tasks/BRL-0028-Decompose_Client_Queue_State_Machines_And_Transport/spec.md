# Context

`src/client/queue.rs` is 1,422 physical lines (about 1,339 NLOC) and combines three different client state machines with Bevy transport orchestration and headless smoke automation. `ClientPracticeModel`, `ClientQueueModel`, and `ClientMatchLoadingModel` have different transitions and protocol messages; production receive/send systems and automation then interleave all three. Repowise reports health 2.49, with complex message observation and automation functions. The issue is lifecycle ownership, not repeated client/server wire handling or file size alone.

This ticket owns queue/practice/loading state and networking. BRL-0022 owns flow screen presentation and consumes these models; coordinate narrow interfaces without moving queue authority or UI scope between tickets.

# Target ownership

Keep `ClientQueuePlugin` and its explicit schedule registration in a small `client::queue` composition module. Separate:

- Practice start: `ClientPracticeModel`, generation binding, single-flight requests, started/rejected outcomes, and Practice message send/receive;
- matchmaking: `ClientQueueModel`, snapshots, memberships, join/cancel commands, freshness, retry/rate-limit/timeout state, and message send/receive;
- match loading: `ClientMatchLoadingModel`, reservation status, cancellation request/outcome, lobby return observation, and the narrow interface used by session handoff;
- automation: headless queue/requeue smoke stages, automatic game selection, product request initiation, and exit behavior;
- focused tests for each pure state machine plus plugin/transport composition tests.

Combine send/receive helpers only where they share the same channel, correlation, and lifecycle. Do not introduce a generic state-machine framework.

# Function-level improvements

- Decompose `observe_matchmaking_messages` into snapshot, membership, and command-outcome consumption helpers, keeping per-receiver ordering and generation correlation explicit.
- Keep `observe_queue_messages` as an intentional lifecycle bridge or split its Practice/matchmaking/loading branches into named owners without changing consuming `take_*` semantics.
- Extract eligibility/transition helpers from `drive_headless_queue_smoke` and `drive_headless_requeue_smoke`; automation must call the same production model commands as interactive flow.
- Preserve `ClientQueueModel::accept_snapshot` and `accept_outcome` as explicit revision/correlation transactions; clarify named predicates rather than weakening validation.
- Move inline tests to focused modules beside the state machines and retain behavioral characterization.
- Use explicit imports and the smallest visibility needed by `client::flow` and `client::session`.

# Compatibility constraints

- Preserve public exports from `crate::client`: `ClientPracticeModel`, `ClientQueueModel`, `ClientMatchLoadingModel`, and `PendingQueueCommand`.
- Preserve all existing method behavior used by flow/session, especially consuming `take_started`, `take_outcome`, `take_returned`, timeout notices, and cancellation observations.
- Preserve lobby-generation reset behavior, request-ID monotonicity, exact command correlation, frozen join targets, snapshot revision/freshness rules, cancellation hiding, late authoritative outcomes, retries, rate limits, and timeouts.
- Preserve protocol types, channels, message order, transport behavior, product-shell copy/state inputs, and server authority.
- Preserve headless queue, requeue, Practice, render, and product smoke behavior.
- Do not add client-to-server authority, shared execution with lobby server rules, or a generic UI/state framework.

# Acceptance criteria

- Practice, matchmaking, match loading, transport orchestration, and automation have explicit focused owners under one visible `ClientQueuePlugin` composition.
- Each of the three client models is independently readable/testable and exposes only the narrow interface required by flow/session.
- Matchmaking receive logic clearly separates snapshot, membership, and command-outcome handling while preserving receiver order and correlations.
- Headless automation is isolated from production state rules and exercises the same model commands as player-driven flow.
- Inline tests are organized by owned state machine and continue to cover single flight, generation reset, snapshot conflict/freshness, timeout/retry/rate-limit, frozen target, late outcome, pending cancel, match cancellation, and requeue behavior.
- Existing public paths, protocol traffic, queue semantics, flow observations, and automation outcomes remain unchanged.
- No generic state-machine/UI framework, duplicated server authority, or module-wide Clippy suppression is introduced.
- BRL-0022 integration remains narrow and conflict-free; queue screen presentation stays owned by flow.
- Focused queue tests plus `just fmt`, `just check`, `just lint`, and `just test` pass with client/server features checked independently.
- Routed product smoke covers Practice start, queue join/cancel, match loading, cancellation/return, and requeue.
- Repowise health is rerun and remaining cross-client/server co-change is dispositioned as protocol behavior where appropriate.
- Verification evidence, feedback disposition, learn-from-errors review, and conflict-free `ticket sync` are recorded before completion.

# Non-goals

- No matchmaking, Practice, queue timing, product-flow UI/copy, protocol, or lobby-server behavior change.
- No hard line-count or health-score target.

# Implementation evidence (2026-08-28)

- Replaced `src/client/queue.rs` with a private `client::queue` module family. `mod.rs` is a 37-NLOC composition facade; `practice.rs`, `matchmaking.rs`, `loading.rs`, and `automation.rs` own their respective models, messages, and lifecycle rules; `tests.rs` retains focused state-machine characterization.
- Preserved crate-level exports for `ClientPracticeModel`, `ClientQueueModel`, `ClientMatchLoadingModel`, and `PendingQueueCommand`, plus the `client::queue::observe_queue_messages` schedule anchor used by flow.
- Preserved the original chained Update order while making each owner explicit at the plugin composition point.
- Split queue transport observation into ordered snapshot consumption, command-outcome consumption, and deferred-snapshot application. Split matchmaking server messages into sequence admission and named phase dispatch while retaining exact generation/sequence/grant validation.
- Automation is isolated from production model rules and continues to call `start`, `start_join`, `start_cancel`, and `start_requeue_join` rather than mutating model state directly.

# Verification evidence (2026-08-28)

- Ten focused `client::queue::tests` passed, covering exact automation selection, Practice single flight/rejection, loading cancellation/return, generation reset, snapshot freshness/conflict, timeout/retry/rate-limit, cancellation revision, late outcome, frozen join target, and pending-cancel rejection.
- Independent client and server feature checks passed; strict client Clippy passed with all targets and `-D warnings`.
- `just fmt`, `just check`, `just lint`, and `just test` passed. The full run included 83 routing tests plus process suites, 428 client tests, 337 server tests, 354 Balance Lab tests, 88 serialized network tests, and 12 performance tests.
- `scripts/network-product-queue.sh` passed real routed initial snapshot, Join, fresh admitted snapshot, Cancel, fresh cancelled snapshot, and cleanup.
- `scripts/e2e-practice.sh wipeout-1v1` passed real routed Practice request, allocation, handoff, check-in, and authoritative Active with one human.
- The Results-to-requeue smoke passed with `BRAWLER_ROUTED_TIMEOUT_SECONDS=220`, observing the terminal worker result at about 187 seconds, worker cleanup, fresh lobby return, and a new Joined outcome for both clients.
- The same requeue smoke's legacy 60-second default and a 90-second diagnostic run timed out after clean match handoff because current `wipeout-1v1` permits a 180-second match. This is harness-bound drift rather than queue behavior; actionable fix BRL-0042 owns aligning the default watchdog or deterministic terminal trigger. No timeout or gameplay behavior was changed in this refactor.
- Repowise health for `src/client/queue` improved from the flat-file finding to 8.6/10 average, with a 10.0/10 composition facade, no alert files, and no static performance findings. Remaining complexity is confined to explicit state transactions (`accept_snapshot`, `accept_outcome`, loading phase dispatch) and bounded automation orchestration.
- Scoped `git diff --check` for queue/ticket paths passed.

# Feedback disposition

- No subjective player feedback was required because this is behavior-preserving state/transport organization. The failed default requeue watchdog was deferred explicitly to actionable ticket BRL-0042; the underlying requeue lifecycle itself passed.

# Learn-from-errors review

- Moving a flat client module one level deeper changes `pub(super)` from client-visible to queue-parent-visible. Compile errors exposed the two interfaces consumed by `client::flow`; they now use `pub(in crate::client)` while all other internals remain narrower.
- Extracted tests had relied on flat-module imports and private fields. Explicit test imports plus `pub(super)` visibility only inside the queue family keep characterization access narrow without widening the crate API.
- A file split alone would have left complex receivers intact. Extracting snapshot, outcome, deferred snapshot, and match-phase handlers made ordering and correlation reviewable at the transport boundary.
- Smoke timeouts must be compared with authoritative scenario bounds before being classified as product regressions. The longer bounded rerun proved requeue behavior and produced BRL-0042 for the stale harness default.
