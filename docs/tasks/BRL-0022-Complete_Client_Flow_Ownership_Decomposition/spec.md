# Outcome

Complete the remaining `client::flow` decomposition as one behavior-preserving refactor. `flow.rs` remains the single visible schedule/composition root, while screen state, presentation, session observation, and tests live with explicit owners and communicate through narrow module interfaces.

# Context

BRL-0006 reduced `src/client/flow.rs` from 7,925 lines to approximately 3,368 lines and established `actions`, `connection`, `input`, `model`, `persistence`, `reducer`, and focused screen modules. The follow-up review found that the structural move is incomplete:

- the root still defines screen-specific drafts, markers, layout types, constants, and presentation systems;
- `input.rs`, `reducer.rs`, `screens/dashboard.rs`, and `screens/brawlers.rs` depend on broad `super::*` imports;
- `present_flow` updates unrelated server-select, connection, queue, dashboard, brawler, and shared button concerns;
- `observe_session` is a 123-NLOC, CCN-35 priority coordinator;
- roughly 1,594 lines of focused tests remain inline in `flow.rs`.

Repowise's churn and duplication scores are advisory context, not numerical acceptance gates. Recent behavior-neutral moves themselves contribute heavily to the churn score, and repeated Bevy UI declarations do not by themselves justify a generic UI framework.

# Scope

## Root composition

- Keep `ClientFlowPlugin`, `ClientFlowSet`, state/model re-exports, system registration, explicit ordering, and the `ApplyDeferred` boundary visible at the `flow.rs` composition point.
- Retain a primitive in the root only when it has demonstrated cross-screen ownership and a narrow stable purpose.
- Remove screen-specific drafts, components, constants, and rendering logic from the root.

## Owned screen concerns

- Move dashboard-owned resources, markers, layout roles, styles, and constants into `screens/dashboard.rs`.
- Move saved-brawler creation, list, details, editing, deletion, and weapon-equipment state and markers into `screens/brawlers.rs`.
- Move game-selection state, constants, and markers into `screens/game_select.rs`.
- Move server-selection editor state and labels into `screens/server_select.rs` or another clearly named focused owner.
- Give connecting, queue, match-loading, error-overlay, and confirmation-overlay presentation focused owners under `flow/screens/`; combine only concerns with the same lifecycle and state owner.
- Keep screen-specific layout, focus, scroll, copy, and live-fact updates local to the owning screen.

## Session observation

- Preserve one visibly ordered session-observation coordinator and the current first-match priority semantics.
- Extract named helpers for resolver completion, practice/queue/loading outcomes, lobby return/loss, match readiness/failure, and connection acceptance/deadlines where this reduces branching without hiding ordering.
- Preserve the consuming behavior of `take_*` observations and do not convert the priority chain into unordered or competing Bevy systems.

## Presentation updates

- Replace the cross-screen `present_flow` system with focused update systems.
- Keep generic button interaction/focus/selection chrome in one shared system only if it remains genuinely common.
- Move queue copy, server editor text, connecting copy, game selection, and brawler selection updates to their owning screen systems.
- Keep query filters disjoint and explicit; do not introduce a general-purpose widget, styling, or screen framework.

## Interfaces and tests

- Replace production `use super::*` and `use super::super::*` imports within `client::flow` with explicit imports.
- Default moved items to private and expose only the smallest `pub(in crate::client::flow)` or `pub(super)` surface required by sibling modules and composition.
- Move the root inline test module to `flow/tests.rs`; keep narrowly owned pure tests beside their owner when that makes the tested contract clearer.
- Add or retain characterization tests for session-observation priority, deferred action arbitration, state-scoped root replacement, overlay blocking, and focused live-presentation updates.

# Invariants

- No player-visible flow, copy, layout, focus order, controls, persistence behavior, connection behavior, queue behavior, match transition, or error recovery changes.
- No protocol, wire type, compatibility, server authority, or routed-process topology changes.
- Preserve the `ClientFlowSet` chain, `ApplyDeferred` placement, state hooks, and `FlowCommit` mutation boundary.
- Preserve state-scoped root cleanup and the existing input handoff between shell and gameplay.
- The server-only feature graph must not acquire rendering, windowing, audio, device input, or client assets.
- Perform reviewable moves before simplification; do not combine this work with visual redesign or feature additions.

# Acceptance criteria

- `flow.rs` is an intentional composition/API surface: it contains module declarations, public re-exports, schedule sets, plugin wiring, and only demonstrated cross-screen primitives—not screen-owned models or presentation bodies.
- Every remaining production item in `flow.rs` has an explicit composition or cross-screen ownership rationale recorded in the ticket.
- Dashboard, brawler, game-selection, server-selection, connecting, queue, match-loading, and overlay state/presentation each have a clear focused owner.
- No production module under `src/client/flow/` uses `super::*` or `super::super::*`.
- The former `present_flow` responsibilities are split into shared button chrome and focused screen update systems with explicit query ownership.
- Session-observation priority remains visible and is covered by tests for competing observations and consuming `take_*` inputs.
- Flow tests no longer occupy the production body of `flow.rs`; root integration tests live in `flow/tests.rs` and focused unit tests live with their owners.
- Existing public and `pub(crate)` flow paths used outside `client::flow` remain unchanged unless the ticket records and verifies a strictly internal visibility reduction.
- No new module-wide Clippy suppressions, generic UI framework, protocol layer, or plugin-per-screen architecture is introduced.
- Required automated and native verification passes, and the ticket records results and any feedback disposition before closeout.

# Verification

Run the repository's canonical commands rather than inventing substitutes:

- `just fmt`
- `just check`
- `just lint`
- `just test`

Also run focused client-flow tests during implementation and verify the client and server feature graphs independently after ownership/import moves. Because this refactor touches product-flow schedules and UI presentation, complete a native smoke pass covering server selection/connection, dashboard, saved-brawler screens, game selection, queue/cancel, match loading, results, error/confirmation overlays, and return to the connected lobby. Record the exact commands, results, screenshots or evidence paths, and observed limitations in the ticket.

# Closeout

- Reconcile any durable ownership guidance in repository documentation if the final boundary differs materially from the current source-layout description.
- Record a learn-from-errors review covering any ordering, visibility, test, or extraction mistakes and how recurrence will be prevented.
- Run `ticket sync`; leave the ticket in `doing` while required corrections or native evidence remain, and move it to `done` only after every acceptance criterion is satisfied.

# Implementation record (2026-08-28)

## Final ownership and composition rationale

- `src/client/flow.rs` is now a 235-line composition/API root. Its remaining production items are all composition-owned: module declarations and public model/preview re-exports preserve existing paths; `ClientFlowSet` and `ClientFlowPlugin` expose schedule ownership; plugin wiring keeps the ordered state hooks, `ClientFlowSet` chain, and `ApplyDeferred` boundary reviewable; `enter_match_input` and `exit_match_input` are the two cross-screen shell/gameplay input handoff hooks.
- Screen-specific resources, markers, constants, spawning, presentation, and live-copy updates now live in focused owners: `screens/dashboard.rs`, `screens/brawlers.rs`, `screens/game_select.rs`, `screens/server_select.rs`, `screens/connecting.rs`, `screens/queue.rs`, `screens/match_loading.rs`, and `screens/overlays.rs`. Common navigation and button chrome live in `screens/shared.rs`.
- `observation.rs` owns the visibly ordered consuming session-observation coordinator and named resolver, global-priority, match-scope, lobby-scope, and connection helpers. `connection.rs` owns initial connection startup; `actions.rs` owns frame arbitration reset; reducer-owned pending mutations/notices remain with the commit boundary.
- The former cross-screen `present_flow` body was removed. Focused update systems now have explicit screen-owned query types, while `update_flow_button_chrome` retains only genuinely common interaction/focus/selection chrome.
- Root integration tests moved unchanged to `flow/tests.rs`; focused observation priority tests live beside `observation.rs`. No production file under `src/client/flow/` uses wildcard parent imports. Existing public and crate-visible paths were preserved through narrow re-exports.
- No protocol, authority, routed topology, feature-gate, player-facing copy/layout/control, persistence, or product behavior change was made. No generic UI framework, plugin-per-screen structure, or new module-wide suppression was introduced. The existing source-layout documentation remains accurate, so no durable documentation change was required.

## Verification evidence

All commands ran from `/Users/boyd/wip/brawler` and passed unless explicitly noted:

- `cargo fmt --all`
- `cargo check --locked --no-default-features --features client`
- `cargo check --locked --no-default-features --features server`
- focused client-flow tests: 39 passed
- full client library tests: 424 passed
- `cargo clippy --locked --no-default-features --features client --lib -- -D warnings`
- `repowise distill just check`
- `repowise distill just lint`
- `repowise distill just test`, including 88 serialized network scenarios and performance gates
- `repowise distill just practice-e2e wipeout-1v1`; reached Active with one human and the manifest bot
- `git diff --check`
- production wildcard audit: `rg -n 'use super::\\*|use super::super::\\*|wildcard_imports' src/client/flow --glob '*.rs'`; matches exist only in test modules

## Native smoke, evidence, and feedback disposition

- Used an isolated profile/server data directory and the native macOS app bundle. Screenshots are under `target/brl-0022-native-smoke/`: create brawler, brawler details, brawler list, Dashboard, game selection, multiplayer queue, queue cancellation back to Dashboard, Practice match loading, cancel-match confirmation, and connection-error overlay.
- Canonical routed native Practice evidence passed with `BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_GAME_TYPE=wipeout-1v1 BRAWLER_RENDER_TIMEOUT_SECONDS=75 repowise distill just v3-render-evidence target/brl-0022-native-smoke/practice-wipeout-1v1-canonical.txt`. The threshold-checked report records `result=pass`, 1,799 samples, match gameplay, and terminal fighter/map visual counts of zero after the result/cleanup path.
- The user observed that the stationary player was likely being killed. This matches the supervisor's match-result timing: the Practice bot completed the match while the automated player remained stationary. It is expected smoke behavior and exercised result, cleanup, and lobby-return handling; no correction is required.
- One deliberately shortened 5-second warmup/10-second measurement report failed only `sample_count` (497 samples). It was not accepted as evidence and was replaced by the canonical 10-second warmup/30-second measurement pass above.
- The first ad-hoc 2v2 visual attempt ended while cancelling; exact 1v1 canonical Practice E2E and native render evidence both passed afterward. This limitation is confined to the ad-hoc setup and does not change the accepted canonical evidence.
- Automated characterization additionally covers state-scoped root replacement, deferred action arbitration, overlay blocking, results acknowledgement, error actions, queue cancellation, return-to-lobby, and focused live-presentation updates.

## Learn-from-errors review

- Mistake: the first render-evidence run shortened the measurement below the locked minimum sample contract. Cause: optimizing runtime without first checking the report validator's sample requirement. Prevention: use canonical evidence durations for closeout; use shorter windows only for exploratory diagnostics and never record them as acceptance evidence.
- Risk caught during extraction: resolver completion is a fallback observation, while later global/match/lobby/connection observations can override it, and `take_*` calls consume state. Prevention: keep one ordered coordinator, name helpers by scope rather than splitting them into competing Bevy systems, and retain characterization tests for competing observations and consumption.
- Visibility/import friction: moving screen types exposed broad implicit dependencies from wildcard imports. Prevention: move owner state and its systems together, default visibility to private, then add only the narrow `pub(in crate::client::flow)` surface demanded by the composition root or sibling owners.
- The user correctly identified that player death explained the native match ending. Prevention for future smoke interpretation: correlate UI behavior with supervisor result timing and terminal render counters before classifying an automated match exit as a lifecycle failure.
