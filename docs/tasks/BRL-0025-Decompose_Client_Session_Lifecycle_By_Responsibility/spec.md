# Context

`src/client/session.rs` is 1,831 physical lines (about 1,704 NLOC) and currently owns several independently changing client lifecycles. Repowise reports health 1.85, CCN 31, nesting 8, and recurring defects. The concrete concern is mixed ownership, not a numeric line limit: match loading/commands, connection construction, lobby identity persistence, compatibility outcomes, routed handoff/recovery, controller automation, timeouts, roster observation, and shutdown all live behind one broad import surface.

This work is separate from BRL-0022. BRL-0022 owns `client::flow` screen/session observation and presentation decomposition; this ticket owns the network-session implementation beneath that flow. Coordinate interfaces so neither ticket duplicates or silently absorbs the other.

# Target ownership

Keep `ClientNetworkPlugin`, `ClientSessionSet`, and the visible schedule composition in a small `client::session` root. Organize focused private submodules by demonstrated lifecycle:

- connection materialization: client entity construction, deferred `Connect`, routed UDP attachment, and product-lobby connection setup;
- handshake and admission: hello emission, lobby identity binding/persistence, join outcomes, rejection classification, and failure recording;
- routed transition: route grants, lobby/match handoff, return-to-lobby recovery, transition deadlines, and routed timeout enforcement;
- match session commands: match-loading ready/cancel exchange, ready/restart commands, and command outcomes;
- automation: product/headless smoke completion and controller-demo behavior, kept separate from production admission rules;
- observation and termination: replicated lifecycle/roster observations, general session timeout, disconnect, AppExit forwarding, and shutdown completion.

Use fewer modules if two concerns genuinely share the same state owner. Do not create a plugin per helper or a generic session framework.

# Function-level improvements

- Turn `process_join_outcome` into a visibly ordered coordinator with named helpers for accepted lobby, accepted match, and rejection outcomes. Reduce its current 194-NLOC/CCN-31 nested transaction without changing consuming message behavior.
- Decompose `process_lobby_server_identity` into validation, account resolution/persistence, and commit/reject steps while keeping the account binding transaction atomic from the caller's perspective.
- Decompose `drive_match_loading_check_in` into correlation/status consumption, cancellation, and readiness policy helpers while preserving retry timing and message order.
- Keep `send_match_command` policy explicit; separate command eligibility/selection from emission where this improves testability.
- Move settings-UI scheduling and controller-demo implementation to their existing focused owners when practical; if the session root must retain a scheduling edge, record why.
- Replace production wildcard imports introduced or retained by the decomposition with explicit imports. Do not add module-wide complexity suppressions.

# Compatibility constraints

- Preserve the public `crate::client::ClientNetworkPlugin` path and all currently used `pub(super)`/crate-visible session entry points unless an internal visibility reduction is proven safe.
- Preserve the `ClientSessionSet` phase chain, strict transactional subchains, observer placement, `ApplyDeferred` visibility, and terminal ordering relative to diagnostics.
- Preserve every protocol type, channel, message correlation field, resend interval, timeout, failure category, log/evidence marker, and AppExit classification.
- Preserve product-shell, headless simulation, render measurement, controller demo, requeue smoke, routed UDP, and direct diagnostic behavior.
- Preserve client/server feature isolation; server-only builds must not acquire client rendering, input, assets, or settings dependencies.
- Do not change flow UI, matchmaking semantics, protocol/wire contracts, server authority, or gameplay.

# Acceptance criteria

- `client::session` is an intentional composition/API root and each production item has one clear lifecycle owner.
- Connection, handshake/admission, routed transition, match-command/loading, automation, and terminal-observation responsibilities are separated along the boundaries above or an equivalent documented ownership split.
- `process_join_outcome` no longer remains a 194-line nested branch tree and its accepted/rejected role paths are covered by focused characterization tests.
- Lobby identity persistence is expressed as named validate/resolve/commit behavior with failure paths covered and no partial identity installation.
- Match-loading cancellation, status consumption, ready retry, and correlation filtering remain ordered and independently testable.
- Session schedule ordering and deferred connection materialization remain explicit and pass ambiguity/order tests.
- Existing public paths and all protocol, timing, recovery, failure, automation, and shutdown behavior remain unchanged.
- Production session modules use explicit imports and introduce no module-wide Clippy suppression or generic framework.
- BRL-0022 overlap is reconciled through stable narrow interfaces, with no duplicate ownership of flow presentation/state.
- Focused session tests plus `just fmt`, `just check`, `just lint`, and `just test` pass; client and server feature graphs are checked independently.
- A native/routed smoke covers lobby connect, admission, match handoff, return to lobby, timeout/rejection recovery, and clean shutdown.
- Repowise health is rerun and remaining findings are either improved or dispositioned based on ownership rather than a score target.
- The ticket records verification, feedback disposition, learn-from-errors review, and a conflict-free `ticket sync` before completion.

# Non-goals

- No player-visible flow or settings redesign.
- No protocol, compatibility, routing-topology, gameplay, or authority changes.
- No hard line-count target and no abstractions created solely to improve a metric.


# Implementation evidence (2026-08-28)

## Ownership decomposition

- Replaced `src/client/session.rs` with a `src/client/session/` module tree.
- `mod.rs` now owns only the public plugin, session-set chain, resource/plugin installation, input/automation registration, network-session phase registration, terminal ordering, and cross-phase marker components.
- `connection.rs` owns direct/routed client entity construction, deferred Connect materialization, and product-lobby connection setup.
- `admission.rs` owns hello exchange, lobby identity validation/persistence, match/lobby outcomes, rejection classification, and failure recording.
- `routing.rs` owns grant acceptance, match completion observation, routed disconnect/unlink/despawn/recreate transitions, transition deadlines, and rejected-client disconnect.
- `match_commands.rs` owns match-loading status/cancel/ready correlation, retry policy, match commands, and product match smoke completion.
- `automation.rs` owns the controller-demo gamepad.
- `observation.rs` owns lifecycle/roster observation, direct timeout enforcement, AppExit-to-disconnect forwarding, and shutdown completion.
- `tests.rs` owns schedule/deferred-materialization characterization; admission and match-loading pure policy tests live with their owners.
- Updated the repository source-layout guidance to describe the session directory.

## Function decomposition

- `process_join_outcome` is now a short ordered coordinator. Match acceptance/rejection and lobby batch processing have focused helpers; lobby welcome consistency is expressed through named validation facts.
- `process_lobby_server_identity` now delegates announcement validation and account resolution/persistence before committing identity state.
- `drive_match_loading_check_in` now delegates correlated status/outcome consumption, action emission, retry timing, and readiness policy.
- The plugin `build` implementation delegates focused registration phases and no longer needs `clippy::too_many_lines`.
- All production `super::*` imports and the module-wide wildcard suppression were removed.

## Automated verification

- `just fmt`: pass.
- Focused `cargo test --features client session --lib`: 13 tests pass after adding identity, routed stale-rejection, readiness-conjunction, and retry-interval characterization.
- `just check`: pass for routing, client, server, network-test, Balance Lab, and Balance Lab web.
- `just lint`: pass for formatting, all Clippy role/all-target lanes, server feature isolation, renderer isolation, and map cleanup.
- `just test`: pass. Reported lanes include 83 routing tests, 428 client tests, 336 server tests, 353 Balance Lab tests, 88 serialized network scenarios, and 12 performance gates; no failures.
- Repowise health rerun: the former 1.85 monolith is gone. Module files score 7.41–9.85; admission max CCN is 10, routing max CCN 10, the root max CCN 2, and the former CCN-31 join coordinator no longer appears.
- `git diff --check`: pass.

## Routed process evidence

- `BRAWLER_ROUTED_BIND=127.0.0.1:5002 BRAWLER_PRODUCT_PLAYERS_PER_TEAM=1 BRAWLER_NETWORK_HEADLESS=1 BRAWLER_ROUTED_TIMEOUT_SECONDS=90 RUST_LOG=brawler=info ./scripts/network-product-match.sh`: pass. Two native client processes authenticated the lobby, received one allocation/grant, completed the deferred lobby-to-match handoff, authenticated the match, converged on the authoritative map/roster, checked in, reached Active, and shut down cleanly.
- The broader `scripts/network-routed.sh` terminal-result/fresh-lobby smoke fails after Active with supervisor `WorkerExitMismatch` and client timeouts. The identical command fails the same way on clean detached `HEAD` c34d339, so it is not a BRL-0025 regression. BRL-0031 records the actionable routing/result defect and is related to BRL-0025/BRL-0026.
- Timeout, rejection, routed recovery, and shutdown behavior remain covered by the passing 88-scenario network suite and focused client lifecycle tests.

# Learn-from-errors review

- The first mechanical extraction split attributes/doc comments from their functions at file boundaries. Cause: slicing at the `fn` token rather than the first attached attribute/comment. Prevention: split Rust items at complete item boundaries and compile after each ownership move.
- The original nested test-module wrapper was retained when moving tests into `tests.rs`, making `super` resolve one level too shallow. Prevention: unwrap inline test modules when converting them to file modules and run `cargo test --lib`, not only `cargo check`.
- A clean-HEAD comparison reused the current Cargo target, and generated absolute `include_str!` paths then referenced the removed temporary worktree. The scoped `cargo clean -p brawler` removed only rebuildable artifacts and restored a clean current-worktree build. Prevention: use an isolated target directory for detached-worktree comparisons even when compilation is slower.
- The baseline comparison was still valuable: it prevented an unrelated existing routed-result defect from being misattributed to this refactor and produced the focused BRL-0031 action ticket.
