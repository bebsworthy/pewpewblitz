# Context

`packages/brawler-routing/src/runtime.rs` is 3,591 physical lines (about 3,301 NLOC) and is the repository's lowest-health file at 1.40. `SupervisorRuntime` spans about 2,180 lines and 59 methods with LCOM4 5. It owns allocation policy execution, worker spawning and supervision, Mio registration, public UDP ingress/egress, worker packet/control IPC, route activation, backpressure queues, expiry, shutdown, and reporting. `handle_control` alone is about 277 NLOC/CCN 50. These are distinct runtime responsibilities sharing one event loop, so the goal is to expose their ownership while retaining one deterministic owner loop.

# Target ownership

Keep `SupervisorRuntime`, `RuntimeConfig`, `RuntimeError`, `RuntimePollReport`, `RuntimeTimingEvent`, `StopHandle`, construction, public facade methods, and top-level poll/run ordering in a small `runtime` composition module.

Extract focused private modules or cohesive helper state for:

- allocation transactions: admission, participant/capability construction, rejection records, finalization, response queuing, and terminal reclamation;
- worker lifecycle: listener/channel attachment, process spawning/polling, readiness, stop/failure handling, packet-drain completion, and cleanup;
- routing I/O: public datagram receive/validation, worker packet/control dispatch, route activation, queue dispatch, interest updates, and public/worker flushes;
- runtime expiry and capacity: limiter expiry, lobby capacity refresh, result-drain deadlines, and bounded queue/capacity enforcement where not already owned above;
- reporting: timing/lifecycle event formatting and stable runtime observation markers;
- focused tests grouped by allocation, public routing, worker isolation/lifecycle, queue bounds, and shutdown.

The event loop remains one owner. Do not introduce threads, async runtimes, traits, callbacks, or a general reactor abstraction.

# Function-level improvements

- Replace the monolithic `handle_control` match body with a small decode/sequence/dispatch coordinator and typed handlers for distinct control bodies. Preserve validation order, sequence disposition, failure isolation, and response timing.
- Split `accept_allocation_request` into validation, deterministic participant/route construction, worker launch/registration, and transaction commit/rejection helpers. A failure must leave no partial route, worker, allocation, or capability state.
- Decompose `handle_packet` and `receive_public` into validation/routing/enqueue steps while keeping hot-path allocations and bounds visible.
- Keep `poll_once` as a readable ordered phase coordinator for process polling, expiry, readiness, public receive, worker handling, queue dispatch, flushing, and reporting.
- Encapsulate coherent field groups only when it reduces invalid intermediate states; do not hide the core's bounded state behind generic repositories or services.
- Move the inline test module to focused runtime test modules without weakening private-state characterization.

# Compatibility and safety constraints

- Preserve every public export from `brawler-routing`, including method signatures and error types.
- Preserve exact control, packet, public-envelope, manifest, capability, digest, route, and ID byte contracts; this ticket does not change protocol versions or compatibility.
- Preserve Mio token registration, readiness handling, burst limits, queue/drop policy, source ingress limiting, allocation caps, rejection codes, sequence tracking, generation rules, route activation/revocation, and deterministic cleanup.
- Preserve worker failure isolation: malformed or terminal traffic from one worker must not corrupt or stop unrelated lobby/match workers.
- Preserve process supervisor ownership, private runtime-directory security, capability secrecy, log redaction, and bounded memory behavior.
- Preserve graceful result packet draining and bounded shutdown semantics.
- Keep the routing crate engine-independent with no Bevy, Lightyear, gameplay, serialization-framework, or async-runtime dependency.

# Acceptance criteria

- `runtime` presents a concise public facade and explicit poll-loop composition; allocation, lifecycle, routing I/O, bounded queues/expiry, reporting, and tests have clear focused owners.
- `SupervisorRuntime` is no longer one 2,180-line implementation block whose methods form disconnected cohesion groups.
- `handle_control` is a dispatch coordinator with typed handlers and no longer remains a 277-line/CCN-50 branch tree.
- Allocation admission is a staged transaction with tested rollback/rejection behavior and no partial state on failure.
- Public and worker packet paths expose validation, route lookup, enqueue/drop, and flush decisions through named helpers while retaining bounds and ordering.
- The public API and every byte-level protocol fixture remain unchanged.
- Existing allocation, malformed-input, source-limiter, queue-bound, worker-isolation, capacity, result-drain, and shutdown tests remain green and are reorganized by owned behavior.
- No new thread, async runtime, generic reactor, trait hierarchy, or dependency is introduced.
- `cargo test -p brawler-routing` and the repository's `just fmt`, `just check`, `just lint`, and `just test` pass.
- Relevant routed process/E2E smoke tests pass for lobby startup, allocation, match activation/result, worker failure isolation, and clean shutdown.
- Repowise health is rerun; remaining duplication/co-change signals are dispositioned as wire-fixture or cross-boundary context where appropriate, not treated as numeric gates.
- Verification evidence, learn-from-errors review, and conflict-free `ticket sync` are recorded before completion.

# Non-goals

- No routing feature, capacity, timeout, protocol, process-topology, or performance-policy change.
- No speculative public abstraction or crate split.
- No hard file-size or health-score acceptance target.


# Implementation evidence (2026-08-28)

## Ownership decomposition

- Replaced `packages/brawler-routing/src/runtime.rs` with a directory-based `runtime/` composition.
- `mod.rs` retains the public `RuntimeConfig`, `RuntimeError`, `RuntimePollReport`, `RuntimeTimingEvent`, `StopHandle`, `SupervisorRuntime` state, construction/inspection APIs, and the visibly ordered `run`/`poll_once` owner loop.
- `allocation.rs` owns admission, launch prerequisites and entropy identities, manifest/participant preparation, immutable record commit, match-worker spawn rollback, grants, rejections, response queuing, and terminal reclamation.
- `worker_lifecycle.rs` owns listener/channel attachment, process spawning/polling, readiness, result drain and teardown, stop/failure handling, and manifest delivery.
- `routing_io.rs` owns bounded public UDP receive/validation, worker packet reads, queue dispatch, public/worker flushes, and Mio interest updates.
- `control_io.rs` owns worker-control decode, lifecycle/sequence observation, typed body dispatch, activation forwarding, authentication promotion, peer close, and worker-scoped failure isolation.
- `capacity.rs` owns external lifecycle control intake, lobby capacity refresh, source/route expiry, and expiry-driven peer-close delivery.
- `reporting.rs` owns redacted lifecycle/timing markers; `tests.rs` owns the former inline private-state characterization suite.
- No thread, async runtime, trait hierarchy, reactor abstraction, dependency, public API, protocol, capacity, or topology change was introduced.

## Function decomposition and preserved transactions

- `handle_control` is now a bounded read/decode/sequence/typed-dispatch/flush coordinator. Typed handlers own allocation requests, results, peer closes, authentication, cancel activation, activation, and start failure. Worker-scoped semantic errors retain isolation; supervisor invariants still escape the poll turn as hard errors.
- Allocation acceptance is staged as request admission, launch-context validation, entropy identity generation, participant/manifest preparation, immutable allocation commit, then spawn with exact cleanup/rejection rollback. No route or capability is installed until validated worker readiness and finalization.
- `handle_packet` separates readable packet validation/routing/enqueue from EOF, writable flush, cleanup, and interest-update coordination.
- `poll_once` remains the explicit single-owner ordering point for process polling, result-drain expiry, route activation, external controls, capacity, expiry, readiness events, public ingress, worker I/O, queue dispatch, flushes, allocation finalization/responses, and cleanup.
- Strict Clippy passes without new complexity suppressions; the former `handle_control` CCN 50 branch tree and monolithic 2,180-line implementation are gone.

## Verification

- `cargo test -p brawler-routing`: pass (83 library tests, 4 supervisor CLI tests, 5 process lifecycle tests, 5 runtime-process tests, and 3 two-worker isolation tests).
- `cargo clippy -p brawler-routing --all-targets -- -D warnings`: pass.
- `just fmt`, `just check`, and `just lint`: pass, including client/server/network/Balance Lab feature isolation checks.
- `just test`: pass: 83 routing tests, 428 client tests, 336 server tests, 353 Balance Lab tests, the combined Balance Lab/network catalog case, 88 serialized network scenarios, and 12 performance gates.
- Native product-process smoke with `BRAWLER_ROUTED_BIND=127.0.0.1:5003 BRAWLER_PRODUCT_PLAYERS_PER_TEAM=1 BRAWLER_NETWORK_HEADLESS=1 BRAWLER_ROUTED_TIMEOUT_SECONDS=90 RUST_LOG=brawler=info ./scripts/network-product-match.sh`: pass. Two client processes authenticated the lobby, one allocation was accepted, a match worker spawned and became Ready, both clients completed routed handoff, authoritative map/roster state converged, the match reached Active, and supervisor shutdown remained bounded.
- Worker failure isolation and cleanup are covered by the passing real-process and two-worker tests, including crash cleanup restricted to one match and a stalled sibling remaining routable.
- The broader fresh-lobby terminal-result smoke remains the clean-HEAD defect already recorded in related action ticket BRL-0031; it was not broadened into this organization-only change.
- `git diff --check`: pass.

## Repowise disposition

- The former single-file score was 1.40 with CCN 50 and a 2,180-line implementation block. The new composition has no alert files; scores range from 5.4 for the deliberately bounded hot-path routing owner to 10.0 for reporting, and allocation's maximum CCN is 8.
- Remaining routing-I/O nesting is the explicit bounded datagram/frame loop and fail-closed validation path. Remaining `poll_once` complexity is the intentional readiness-phase coordinator. Remaining allocation size reflects wire manifest construction and exact grant binding; these stay together because their byte/transaction ownership is shared, not to meet a numeric score.

# Learn-from-errors review

- A first attempted `include!` extraction inside an inherent `impl` is not accepted in this item position. The implementation was corrected to conventional private child modules with partial inherent impls. Prevention: use directory modules for partial impl ownership from the start.
- Mechanical region extraction carried a preceding doc comment and a detached item-level Clippy attribute into the lifecycle owner. Strict Clippy caught both. Prevention: treat attached comments/attributes as part of complete Rust items and run strict lint immediately after each extraction.
- Moving tests out of the inline module exposed imports that had been supplied accidentally through the production root. The tests now import their fixtures explicitly, reducing production import coupling.
- Splitting packet reads initially retained an initialization that every match arm overwrote; `-D warnings` caught it. Prevention: let phase helpers return their disposition facts directly instead of preinitializing coordinator state.
