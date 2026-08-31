# Technical specification

## Outcome

Complete BRL-0070 Stage 6.2 by extracting the demonstrated queue/Practice/match-loading/Results ownership from `src/client/flow/reducer.rs` into private `src/client/flow/reducer/match_flow.rs`. The root remains the schedule-facing precedence coordinator plus connection/server-selection shell, teardown, and commit owner. This is an organization and characterization phase; protocol, UI, copy, authority, schedules, resources, and player-visible transitions remain unchanged.

## Exact ownership boundary

Move to `match_flow.rs`:

- `MatchFailureNotice`;
- matchmaking-side session observation reduction;
- queue outcome and queue rejection reduction;
- Practice rejection copy;
- game-selection acceptance and draft actions;
- queue, Practice, match-loading, cancel-confirmation, and Results navigation/replay actions;
- selected-brawler lookup;
- `accept_game_type_draft`.

Decompose the current large match-navigation action function into focused plain reducers for game selection, matchmaking/loading, and Results/replay. Do not move it unchanged. Keep `MatchFailureNotice` and `accept_game_type_draft` narrowly re-exported through `reducer.rs` so existing `flow.rs` composition and tests retain their paths.

Keep in `reducer.rs`:

- the single `resolve_flow_action` system and category/precedence dispatch;
- explicit cancel/disconnect/change-server handling;
- connection-side session observation handling, including `UnexpectedLoss`;
- server-selection, favorite, persistence retry, and connection-start action rules;
- profile/equipment delegation;
- teardown and `FlowCommit` application.

Keep the membership query alias in the root with the narrow visibility required by the child. Shared fail-to-server-select behavior may be exposed only to the child because connection and match content failures intentionally converge there. Add no plugin, system, resource, message, event, trait, command bus, context framework, or public API.

## Preserved behavioral contracts

- Profile decision resolution runs before action precedence.
- Explicit action preempts session and ordinary action; at most one session observation is reduced and then the system returns; otherwise at most one ordinary action is reduced.
- Session observation routing remains connection versus match-flow with `UnexpectedLoss` connection-owned.
- Queue and Practice remain mutually exclusive and require the exact selected brawler.
- Reservation, countdown, cancellation, queue acknowledgement, and error transitions retain exact `FlowCommit`, overlay, focus, purpose, and copy outcomes.
- `MatchFailed` clears result context, sets `MatchFailureNotice`, and requests return to lobby; a later fresh-lobby return chooses Results or Dashboard and reports the existing failure exactly once.
- QueueAgain retains exact advertised game lookup, catalog/configuration revision refresh, lobby generation binding, selected-brawler revision, Practice versus multiplayer behavior, and disappearing-game failure.
- One existing `FlowCommit` remains the sole mutation publication path.
- The existing Resolve -> Teardown -> ApplyDeferred -> Commit schedule and all session observation ordering remain unchanged.

## Tests

Retain existing owned-ambiguity, profile-first, explicit > session > ordinary, queue/Practice exclusion, selected-brawler, game-draft, fresh-lobby replay, completed-match return, and disappearing-game behavior tests.

Add focused injected-action/observation characterization where current coverage is not exact for:

1. ReservationStarted and CountdownObserved transitions;
2. QueueOutcome Joined/Cancelled plus incompatible/stale content rejection;
3. stale-content versus ordinary PracticeRejected errors;
4. MatchFailed followed by FreshLobbyReturn with and without result context, including exact focus/purpose/error behavior;
5. Practice QueueAgain exact game, generation, and selected-brawler path.

Prefer pure reducer tests when the helper interface permits; otherwise use the existing small client-flow App harness. Do not add a production abstraction only for tests.

## Verification

Run and record:

- `cargo fmt --all -- --check`
- `git diff --check`
- focused `client::flow::tests`
- client all-target check
- strict client all-target Clippy
- `just check`
- `just lint`
- `just test`

No native evidence is required if copy, UI, actions, transitions, schedule relations, and routed behavior remain exact. Independent review must confirm the ownership boundary and precedence/order parity before closeout.

## Exclusions

Connection-attempt mechanics in `flow/connection.rs`, connection/server-selection shell extraction, action/message redesign, screen presentation, network protocol, queue or Practice policy, session observation production, schedule changes, and player-facing polish are excluded. After this phase the remaining root is considered cohesive; further splitting solely by line count is not BRL-0070 work.

## Implementation evidence — 2026-08-31

Extracted the complete game-selection through queue/Practice/loading/Results/replay reduction lifecycle into private `client::flow::reducer::match_flow`. The root remains the sole schedule-facing precedence coordinator and retains connection/server-selection shell rules, explicit-action handling, profile/equipment delegation, teardown, and commit publication. The moved action path was decomposed into focused game-selection, matchmaking/loading, and Results/replay reducers rather than relocated unchanged. No systems, resources, schedules, messages, protocol shapes, UI, copy, or player-visible transitions changed.

Five grouped characterization tests now cover loading/countdown commits, queue join/cancel/content rejection, stale versus ordinary Practice rejection, MatchFailed/FreshLobbyReturn branches, and Practice replay. Independent review found no P0/P1 behavioral or schedule drift. Its two P2 evidence gaps were corrected: FreshLobbyReturn now asserts exact purpose, overlay, one-shot failure message/actions/return flow and both context branches; Practice replay now competes with a stale-generation lobby and asserts the emitted current-generation request's exact game and brawler identity/revision through one narrow cfg(test) accessor. The membership query alias remains parent-private.

Verification passed:

- `cargo fmt --all -- --check`
- `git diff --check`
- 52 focused `client::flow::tests` (47 retained plus five grouped characterizations)
- client all-target check and strict all-target Clippy
- `just check`
- `just lint`
- `just test`: 550 client, 493 server, 522 Balance Lab, the exact cross-feature network smoke, all 97 routed network tests, and all 12 performance gates

No native rerun was required because copy, UI, input meaning, schedule relations, routed behavior, and transition outcomes are unchanged. No feedback item remains open.

## Learn-from-errors review

The first characterization pass asserted broad destinations but not the distinguishing state that makes the extraction safe. Cause: treating a green moved-code diff as sufficient evidence for generation-sensitive replay and one-shot error behavior. Prevention: tests for reducers that select among multiple ECS candidates must include a competing stale candidate and assert the exact emitted request identity; tests for multi-step session recovery must assert purpose, overlay, exact error actions/copy, and consumption on the following observation. The reusable lesson is to characterize discriminators and consumption semantics, not only final screen states.
