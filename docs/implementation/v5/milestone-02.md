# V5 Milestone 02 — Dashboard-owned selection and connected-loop convergence

| Field | Value |
|---|---|
| Status | Complete |
| Depends on | V5 M01 accepted and completed on 2026-08-21 |
| Outcome | Dashboard-owned brawler and game-type selection plus queue, loading, match, and Results paths whose ordinary connected exits return to Dashboard |

## Research question

What is the smallest revision of the existing connected screen flow that makes build and game-type
selection true Dashboard children, preserves accepted queue/build authority, and returns every
ordinary connected exit to Dashboard without introducing a second navigation framework?

## Accepted product boundary

- Dashboard remains the authenticated connected home established by M01.
- Build and game-type selection need explicit confirm/back semantics and deterministic focus
  restoration; they must not become a carousel or a new client authority path.
- A queued build remains the server-validated frozen candidate. Editing a draft must not silently
  change an accepted queue request.
- Queue cancellation, loading/match-start cancellation, confirmed match leave, successful match
  completion, and Results exit should return to Dashboard while the lobby session remains valid.
- Unexpected connection loss and explicit disconnect continue to use the recovery paths established
  by M01.
- Results must retain authoritative outcomes. Play Again may reuse a valid selection, but Change
  Game and Disconnect should not remain duplicated there when Dashboard owns those choices.
- No screen may claim a selected map while the server advertises a map pool and formation owns the
  actual map choice.

## Research starting points

- `src/client/flow.rs`, `src/client/shell.rs`, `src/client/dashboard.rs`, and existing flow tests for
  screen ownership, overlays, focus restoration, and authenticated-session transitions;
- the existing build editor, game selection, queue/loading, pause/leave, and Results systems for
  their current state and action ownership;
- `src/client/session.rs` and routed lifecycle tests for lobby validity, admission cancellation,
  match handoff, result return, and unexpected-loss behavior;
- `docs/13-player-ux.md`, `docs/14-multiplayer-server-architecture.md`, and completed V2 milestone
  evidence for enduring navigation and authority decisions;
- checked-in Bevy 0.19 state/UI examples and local Lightyear lifecycle material where exact APIs or
  ordering need confirmation.

## Research checklist

- inventory every current transition into and out of Build Select, Game Select, queue, loading,
  match, pause/leave confirmation, and Results;
- identify which state owns the accepted build, editable draft, selected advertised game type,
  lobby validity, and focus-return target at each transition;
- determine the smallest action/state changes needed to make Dashboard the sole connected home;
- specify loss, cancellation, stale-advertisement, rejected-build, and worker-handoff recovery;
- define focused state/ECS tests and representative routed lifecycle cases;
- record the native playtest scenarios needed to validate confirm/back and input parity.

## Research log — screen and navigation inventory

Audited on 2026-08-21 against:

- `src/client/flow.rs` for the `ClientFlow`/`ClientOverlay` definitions, action arbitration,
  state-scoped roots, and all primary transitions;
- `src/client/shell.rs` for Settings/Credits, focus layers, return targets, and legacy Title code;
- `src/client/presentation.rs` for the non-pausing in-match menu and held/latched Scoreboard;
- `src/client/session.rs` for authoritative result capture and routed match-to-lobby return;
- `src/client/queue.rs` and `src/client/build_editor.rs` for admission, cancellation, frozen queue
  membership, editable drafts, and accepted build ownership;
- `README.md` and `docs/13-player-ux.md` for documentation that still described the superseded V2
  title-first flow;
- `references/bevy/examples/state/states.rs` and `state/sub_states.rs`, plus the local
  `bevy-game-engine` state reference, to confirm that primary destinations remain appropriate
  `States` while modal child concerns remain independently scoped overlays. The current two-axis
  design is sound; M02 does not need a new navigation framework or a state hierarchy rewrite.

The as-built diagrams, surface inventory, remnant register, and proposed destination map below form
this milestone's historical audit. The accepted durable flow is maintained in
[`../../13-player-ux.md`](../../13-player-ux.md).

### Primary-state findings

`ClientFlow` currently contains exactly eight primary states: Connecting, Server Select, Dashboard,
Game Select, Queue, Match Loading, Match, and Results. Title has already been removed from this
enum. Match Complete is a transient cover over Match while a fresh lobby session is established;
it is not a ninth destination.

The connected flow has only partially converged on Dashboard. Success from Connecting correctly
enters Dashboard, and direct Dashboard Play/Practice already reuse the authoritative admission
paths. Four ordinary lobby returns still target Game Select: queue cancellation, match-start
cancellation, confirmed match leave, and match failure/no-result return. Results additionally owns
Change Game and Disconnect, and Escape on Queue, Match Loading, and Results performs a full
disconnect instead of the nearest local return action.

### Child-surface findings

- Game Select is currently both a child selector and a remnant connected hub. Activating a game row
  updates `SelectedGameType` immediately and returns to Dashboard, but the old Build & Join/Start,
  favorite-server, and disconnect actions remain below the list.
- Build Editor is one overlay with two contracts selected from its parent flow. From Dashboard it
  has select/back semantics; from Game Select it owns queue/practice admission and disconnect.
  M02 should retain the proven draft/validation UI but make it a Dashboard child only.
- Build Editor already keeps `loaded_selection` distinct from its editable choice and only accepts
  on Save or authoritative Join acceptance. Game selection lacks the equivalent draft/confirmed
  distinction and needs one.
- A timed-out Dashboard-originated queue Join can reopen Build Editor through
  `queue_recovery_overlay`, mixing an editable selector with a frozen pending admission command.
  Retry should remain with the command-owning Dashboard/Queue error context.
- Settings and Credits are valid modal surfaces. Their production behavior returns to the
  underlying flow, but their internal neutral layer and return target are still called `Title`.
- The in-match menu and Scoreboard are deliberately separate from `ClientOverlay`; this correctly
  models continuing authoritative gameplay and should remain unchanged.

### Recovery and lifecycle findings

- Unexpected lobby loss is handled for Dashboard, Game Select, and Queue, but Results is omitted
  even though Results depends on the fresh authenticated lobby for replay. M02 must cover this.
- `SessionPurpose` currently drives alternate Game Select, Build Editor, Results, and disconnect
  behavior. Once Play and Practice are Dashboard actions, it should be scoped to the active
  admission/result and normalized on return to Dashboard rather than representing a parallel
  navigation journey.
- The authoritative boundaries are already suitable: queue membership carries the frozen accepted
  build; match completion is copied from replicated match authority before unlink; a fresh lobby
  authentication gates Results/re-entry. Navigation changes do not require protocol changes.

### Remnants to remove or rename during M02

1. All ordinary `GameSelect` return destinations outside the game-type child itself.
2. Game Select's Build & Join/Start, favorite, and disconnect controls.
3. Build Editor's parent-dependent Join/Start/Disconnect mode.
4. Results Change Game and Disconnect/Exit Practice duplication; add an explicit Dashboard exit.
5. Escape-to-disconnect behavior on Queue, Match Loading, and Results.
6. The dead `spawn_title` implementation, Title marker/control variants, and title-only regression
   fixtures in `shell.rs`.
7. `ShellLayer::Title`, `ErrorReturn::Title`, and `SettingsReturnTarget::Title` terminology.
8. README and current UX/map documentation statements that still claim title-first startup or
   Game Select as the connected home.
9. Historical screen-flow test expectations and stale milestone labels where they obscure current
   ownership.

The replicated in-match `SelectingBuild` overlay is also a product-shell remnant candidate. It is
not entered by the routed V5 admission path, but it is globally installed by the windowed
presentation composition. M02 should either prove a supported non-product owner and gate it there,
or retire it; it must not become a second player build-selection path.

## Initial design conclusion

Keep the existing flat `ClientFlow` plus independent `ClientOverlay` resource. Rename Game Select
to a game-type-selection child in code only if that improves auditability; a new generic router,
navigation stack, or hierarchical state model is not justified. The smallest coherent transition
contract is:

- connected child Confirm/Back always returns Dashboard with deterministic focus restoration;
- Play and Practice are the only ordinary admission initiators;
- queue/loading cancellation and match leave/failure return Dashboard while the lobby is valid;
- Results exposes replay plus Dashboard, with stale replay disabling explicitly rather than
  silently changing selection;
- explicit Change Server and unexpected lobby loss are the only connected paths to Server Select.

The research recommendation for Results is to derive replay availability from the exact previous
game-type ID and the fresh authenticated lobby catalog. If absent, preserve the result, disable
replay with a factual reason, and keep Dashboard available; do not fall through the chain of cached
IDs or silently select a different game. Recoverable rate/capacity rejection should remain on
Results, while catalog/protocol incompatibility uses the established reconnect recovery path. This
recommendation and the one-time Dashboard selection reconciliation message are carried into the
technical specification below for user validation.

## Technical specification

Status: accepted by the user on 2026-08-21 and in implementation.

### Scope and non-goals

M02 changes navigation ownership and the minimum presentation necessary to make its child screens
coherent. It does not perform M03's final responsive/art/audio/accessibility pass, change gameplay
or lobby protocols, introduce a generic navigation stack, replace the accepted Dashboard, or add a
client-selected map. Existing authenticated lobby, queue, reservation, routed handoff, match, and
result authority remain unchanged.

### State and overlay model

Retain one flat `ClientFlow` for mutually exclusive primary destinations and one independent
`ClientOverlay` for modal surfaces. Rename `ClientFlow::GameSelect` to
`ClientFlow::GameTypeSelect` so its child-only role is explicit. The primary state set remains:

```text
Connecting · ServerSelect · Dashboard · GameTypeSelect · Queue · MatchLoading · Match · Results
```

Build Editor remains an overlay because it is a modal Dashboard child. Settings, Credits,
Dashboard Menu, errors, and confirmations remain overlays. The in-match menu remains
`ClientInputContext::Menu`; Scoreboard remains a held/latched match presentation. Match Complete
remains a transient cover over Match while routed return establishes a fresh lobby.

Continue using the ordered `ClientFlowSet` phases. Actions and session observations resolve into
one `FlowCommit`; `NextState` changes occur only in the commit phase. Primary roots remain spawned
by `OnEnter` and scoped with `DespawnOnExit`. No presentation entity or transition becomes gameplay
or network authority.

### Owned client state

| State | Owner and rule |
|---|---|
| Accepted game type | `SelectedGameType`; always an exact entry from the current authenticated lobby catalog, including current catalog/configuration revisions |
| Game-type draft | New private `GameTypeSelectionDraft`; opened from the accepted selection, changed locally by row activation, committed only by Confirm, discarded by Back/loss |
| Accepted local build | Existing `BuildEditorState::loaded_selection`; changed only by successful Dashboard-child Save and persisted through the existing validated file contract |
| Build draft | Existing Build Editor choice/custom fields; discarded by Back and never used to mutate an already submitted command |
| Submitted admission | Existing queue/practice model; captures the selected game/build at submission and is single-flight |
| Accepted queued build | Existing server-authored `QueueMembership::accepted_build`; remains frozen until cancellation/formation |
| Match result | Existing `ClientMatchResultContext`, copied from authoritative replicated completion before unlink |
| Active purpose | Existing `SessionPurpose`, narrowed to the current admission/result; reset to Multiplayer on entry to Dashboard |
| Return focus | New small private `DashboardReturnFocus` value consumed by Dashboard entry; no general history stack |
| Reconciliation notice | One bounded client-local Dashboard notice used only when a previously selected game disappears from a fresh catalog |

### Exact transition contract

| From | Trigger | Destination/behavior |
|---|---|---|
| Connecting | authenticated lobby accepted | Dashboard |
| Connecting | cancel, bounded failure, rejection | Server Select with existing recovery copy |
| Server Select | validated Connect/saved target | Connecting |
| Dashboard | Change Brawler | Build Editor overlay; draft begins from accepted local build |
| Build Editor | Confirm | validate, persist accepted build, close to Dashboard, restore build-card focus |
| Build Editor | Back/Escape | discard draft, close to Dashboard, restore build-card focus |
| Dashboard | Change Game | Game Type Select; draft begins from accepted advertised game |
| Game Type Select | row activation | change draft only and remain on screen |
| Game Type Select | Confirm | commit exact current catalog entry, return Dashboard, restore game-card focus |
| Game Type Select | Back/Escape | discard draft, return Dashboard, restore game-card focus |
| Dashboard | Play | submit one multiplayer Join from accepted game/build; remain Dashboard until outcome |
| Dashboard | Practice | submit one practice request from accepted game/build; remain Dashboard until outcome |
| Dashboard | multiplayer Join accepted | Queue |
| Dashboard | practice reservation accepted | Match Loading |
| Queue | Cancel/Escape then cancellation accepted | Dashboard; reset purpose; restore Play focus |
| Queue | reservation started | Match Loading |
| Match Loading | Cancel/Escape | open Cancel Match Start confirmation |
| Match Loading | cancellation accepted and fresh lobby ready | Dashboard; reset purpose; restore Play focus |
| Match Loading | cancellation too late | clear confirmation and continue loading into Match |
| Match Loading | authoritative countdown | Match |
| Match | confirmed Leave and fresh lobby ready | Dashboard with no fabricated result |
| Match | worker failure and fresh lobby ready | Dashboard plus factual recoverable error |
| Match | authoritative completion and fresh lobby ready | Results with preserved outcome |
| Results | Dashboard/Back/Escape | Dashboard; clear result on exit and reset purpose |
| Results | valid multiplayer replay | submit exact previous advertised game/current revision; accepted outcome enters Queue |
| Results | valid practice replay | submit exact previous advertised game/current revision; accepted reservation enters Match Loading |
| Any connected state | current lobby session lost | Server Select with loss error; clear child draft/result/admission presentation as appropriate |
| Dashboard | confirmed Change Server | Server Select after deliberate teardown |

Queue and Match Loading no longer display ordinary Disconnect buttons. Their local Cancel action is
the normal exit. A timeout or incompatible state may still offer an explicit destructive recovery
action when the client cannot safely claim that the server removed the ticket/reservation.

### Game-type child behavior

Game Type Select shows the same real advertised facts already used by Dashboard: display name,
mode/topology, rules summary, complete map pool, fresh population/availability, and practice bot
copy only where relevant. It owns only selectable rows, Confirm, and Back. Remove Build & Join/Start,
favorite/unfavorite, and Disconnect. Favorite and server controls remain Dashboard/Menu concerns.

Entering creates a bounded draft from the current accepted ID. If that ID is no longer advertised,
the draft selects the first current advertised game and shows why; the accepted selection is not
changed until Confirm. Confirm is disabled if there is no current valid candidate. Back never
changes `SelectedGameType`.

### Build child behavior

Build Editor always uses the current Dashboard-child contract: preset/custom choices, real budget
and comparison details, Confirm/Select Brawler, and Back. Remove its Game Select parent mode,
Join Queue, Start Practice, game-name dependency, and Disconnect. It cannot open while a queue or
practice admission command is pending. Queue retry never opens Build Editor; it retries the frozen
command from the owning error context.

### Dashboard admission behavior

Play and Practice are the only ordinary admission initiators. While either command is pending,
disable Play, Practice, Change Game, and Change Brawler, show factual pending copy on the initiating
action, and retain Settings/Menu. An accepted multiplayer outcome persists the exact accepted build
through the existing path and enters Queue. Practice and queue rejection return to an error over
Dashboard unless the rejection proves catalog/protocol incompatibility, which uses explicit
reconnect recovery.

Dashboard entry reconciles `SelectedGameType` against the current authenticated catalog. An exact
ID match refreshes its revisions. If a previously accepted ID disappeared, select the first current
advertised game and display one bounded notice naming that the previous game is unavailable and the
real replacement now selected. Initial connection may continue to select the first advertised game
without presenting it as a recovery event.

### Results and replay behavior

Results keeps the authoritative outcome, final score, local team, and resolved game name. Its only
actions are Replay (`PLAY AGAIN` for multiplayer, `PRACTICE AGAIN` for practice) and Dashboard.
Remove Change Game and Disconnect/Exit Practice.

Replay eligibility uses only `ClientMatchResultContext.game_type_id` against the fresh current
lobby catalog. Remove the current fallback chain through cached accepted IDs and current Dashboard
selection. When the exact ID is absent, disable Replay and show a factual unavailable reason;
Dashboard remains enabled. A current entry may use its fresh catalog/configuration revisions—the
client does not reuse stale revisions. A recoverable rate/capacity error remains over Results and
preserves the outcome. Catalog/protocol incompatibility follows Server Select recovery.

Unexpected-loss observation must identify the current lobby session/generation rather than treating
an old disconnected match entity as lobby loss. Apply that precise check to Dashboard, Game Type
Select, Queue, and Results.

### Input and focus

- pointer press, keyboard Enter/Space, and gamepad South activate the same focused action;
- keyboard Escape and gamepad East mean Back for Game Type Select, Build Editor, and Results;
- the same inputs request queue cancellation in Queue and open cancellation confirmation in Match
  Loading; they never silently disconnect there;
- Dashboard return focus is Build Card after Build Editor, Game Card after Game Type Select, and
  Play after queue/loading/match/Results return;
- Settings entered from a match restores the continuing match menu; Settings/Credits entered from
  product flow restore their underlying flow without any Title concept;
- disabled/pending controls are skipped by focus and cannot activate by pointer.

### Legacy and documentation cleanup

- remove dead `spawn_title`, `TitleRoot`, title-only Play/Practice/Quit shell actions and controls,
  and their obsolete fixtures;
- rename shell `Title` layers/return values to underlying-product-flow terminology and retain tests
  around real Settings/Credits behavior;
- retain the legacy direct-UDP in-match `SelectingBuild` path only for configurations that do not
  present the product shell; prevent its overlay from becoming a second V5 build selector;
- replace stale `GameSelect` transition tests and historical milestone names where they obscure
  ownership;
- reconcile README and `docs/13-player-ux.md` with the accepted V5 flow,
  leaving versioned V2 documents as historical evidence.

### Network and authority behavior

No protocol, channel, wire type, server admission rule, supervisor allocation, or match-worker rule
changes. The client still sends build/game intent; the lobby validates the current catalog revision,
game configuration, build, queue command, and practice request. Formation still chooses the map.
The worker still owns readiness, gameplay, leave/forfeit consequences, and result. Routed return
still requires a fresh authenticated lobby before Dashboard or Results becomes usable.

## Implementation checklist

1. [x] Add the game-type draft, Dashboard return-focus/notice state, and exact lobby-session helper.
2. [x] Rename `GameSelect` to `GameTypeSelect`; rebuild it as rows plus Confirm/Back and remove hub
   actions.
3. [x] Simplify Build Editor to one Dashboard-child contract and block child entry during admission.
4. [x] Change queue, loading, leave, failure, and no-result lobby returns to Dashboard; normalize
   purpose and focus.
5. [x] Replace destructive Escape mappings and remove ordinary Disconnect controls from Queue,
   Match Loading, and Results.
6. [x] Reduce Results to Replay/Dashboard, implement exact fresh-catalog eligibility, and preserve
   result context across recoverable replay errors.
7. [x] Make lobby-loss observation generation/kind aware and include Results.
8. [x] Remove dead Title shell code/names/tests and gate the legacy match build selector away from the
   product shell.
9. [x] Update affected focused tests, routed lifecycle tests, README, player UX, and the screen map.
10. [x] Run canonical automated verification and the two-client routed E2E. The native
    keyboard/gamepad/pointer scenario is the current user-playtest gate.

## Implementation result

Implemented on 2026-08-21:

- game-type selection now owns a private draft; rows do not mutate the accepted selection, Confirm
  commits an exact current advertisement, and Back discards with game-card focus restoration;
- Build Editor has one Dashboard-child selection contract and no queue, practice, game-name, or
  disconnect mode;
- Dashboard Play/Practice are the only ordinary admission actions and block both selectors while a
  command is pending;
- queue/loading cancellation, leave, match failure, and no-result fresh-lobby return converge on
  Dashboard; local Back/Escape no longer disconnects from Queue, Match Loading, or Results;
- Results contains only exact fresh-catalog replay and Dashboard; missing replay is disabled with a
  factual reason and recoverable replay errors preserve the result;
- current-generation lobby observation ignores stale disconnected match entities and covers
  Dashboard, Game Type Select, Queue, and Results;
- favorite/unfavorite moved from the retired Game Select hub to Dashboard Menu;
- dead Title UI/actions/fixtures and Title return terminology were removed; the legacy replicated
  build selector is hidden and input-gated in product-shell composition;
- README, the V2 UX supersession note, and the current screen map now describe the V5 loop.

No protocol, admission, worker, gameplay, map-selection, or server-authority contract changed.

## Verification evidence

Passed on 2026-08-21:

- `just fmt`;
- `just lint`, including client/server Clippy with warnings denied and server feature isolation;
- `just check`, including routing, client, server, and network-test targets;
- `just test`: 83 routing unit tests, routing process/isolation suites, 400 client tests, 311 server
  tests, 82 separate-App network tests, and 14 performance tests;
- `just e2e 2`: one real routed exact 1v1 roster reached Active; lobby and match workers stopped,
  exited, reaped, and cleaned normally.

Focused coverage includes accepted-versus-draft game selection, stale replay disablement, exact
fresh-catalog replay, Dashboard return after fresh lobby, generation-safe stale-match handling,
Build Editor focus/scroll lifecycle, real Settings overlay ownership, and absence of Title-layer
fixtures.

## Verification plan

### Focused automated coverage

- Game Type Select row activation changes only its draft; Confirm commits; Back/Escape discards;
- Build Editor Confirm/Back owns accepted-versus-draft state and contains no admission/disconnect
  controls;
- Dashboard blocks selectors and duplicate admission while Join/Practice is pending;
- queue cancellation, match-start cancellation, leave, match failure, and no-result fresh-lobby
  return all reach Dashboard with the expected focus and purpose;
- Escape behavior is local on Game Type Select, Build Editor, Queue, Match Loading, and Results;
- Results replay uses only its exact prior ID and fresh revisions, disables cleanly when missing,
  preserves results on recoverable failure, and never falls through cached IDs;
- current-lobby loss reaches Server Select from Dashboard, Game Type Select, Queue, and Results,
  while an old disconnected match entity beside a fresh lobby does not trigger loss;
- correctable build/queue errors retain frozen submissions and never reopen an editable selector;
- all state/overlay roots remain singular and clean up across repeated entry;
- product-shell composition contains no Title UI and never displays the legacy replicated match
  build selector; the supported legacy diagnostic composition remains intact;
- client/server feature isolation and protocol registration are unchanged.

### Integration and process coverage

- update existing separate-App queue cancellation, loading cancellation, completion/requeue, and
  routed fresh-lobby-return cases to assert Dashboard convergence;
- add representative unexpected-loss-on-Results and stale-replay cases without multiplying every
  error/input combination;
- run `just fmt`, `just lint`, `just check`, and `just test`;
- run `just e2e 2` to prove the unchanged authoritative admission, match, result, and return path.
  The 4/6-client closeout matrix remains M03 unless M02 exposes a routing regression.

### Native interaction matrix

Using `just server` and `just client`, verify one multiplayer and one practice loop with pointer,
keyboard, and gamepad: child Confirm/Back and focus restoration; queue cancellation; loading cancel
confirmation; match leave; completed Results replay; Results Dashboard; missing/stale replay copy;
Settings return from Dashboard and match menu; Change Server; and unexpected lobby loss. Confirm
that no path exposes Title, hybrid Build & Join, duplicated Results controls, or an unrequested
disconnect.

## Playtest handoff and exit criteria

M02 can advance to User playtest after implementation and automated/native verification. The
handoff will list exact run commands and ask whether selection feels like a child of Dashboard,
whether cancellation feels local and safe, whether replay versus Dashboard is unambiguous, and
whether focus returns to the expected card.

M02 is complete only when the user accepts the connected return loop, every finding in the remnant
register is implemented/deferred/rejected with rationale, affected verification is rerun, and the
learning review is recorded. M03 retains exhaustive resolution/UI-scale, accessibility, transition
audio, repeated lifecycle/performance, and full 2/4/6-client closeout work.

## Feedback review — 2026-08-21

The first M02 playtest found dead-looking Dashboard Menu items and an unusable Server Select after
Change Server: Connect appeared to do nothing. Accepted for immediate correction.

Root cause: confirming Change Server performed the deliberate lobby teardown and primary-state
transition but left `ClientOverlay::ChangeServerConfirmation` active. The confirmation entity was
state-scoped and disappeared on leaving Dashboard, while the overlay resource continued to block
every ordinary Server Select control. Existing tests invoked flow actions directly and therefore
did not cover the complete rendered-button/overlay transition.

Correction scope:

- clear the active overlay on every explicit disconnect/cancel transition to Server Select;
- render Favorite Server only when the current client has a real `RuntimeLobbyTarget`, avoiding a
  menu action that can silently have no target;
- add rendered-button regression coverage for Connect, Menu/Credits, the complete Change Server →
  Server Select → Connect path, and absence of Favorite Server without a real target;
- rerun affected client tests and canonical verification before returning M02 to playtest.

Correction verification passed on 2026-08-22: focused flow tests (32/32), `just fmt`, `just lint`,
`just check`, the complete `just test` matrix (including 400 client, 311 server, 82 network, and 14
performance tests), and `just e2e 2`. At that point, M02 remained in Feedback review pending native
user confirmation.

The user confirmed the corrected native flow on 2026-08-22. The feedback item is accepted and
resolved; no M02 feedback remains deferred, rejected, or awaiting evidence.

## Learn-from-errors review

### What went wrong

The first implementation correctly despawned the Change Server confirmation entity when Dashboard
exited, but did not clear the independent `ClientOverlay::ChangeServerConfirmation` resource. The
new Server Select rendered normally while its controls were still rejected by overlay input
filtering. Direct action tests proved the transition logic but skipped the rendered-button and
overlay lifecycle that failed in the product.

### Causes

- Entity cleanup and resource cleanup were treated as if state-scoped despawning covered both.
- Tests asserted internal actions and destination states separately instead of traversing the full
  player-visible Menu → confirmation → Server Select → Connect sequence.
- Favorite Server was rendered without first proving that a real runtime lobby target existed,
  leaving a valid-looking action with no possible effect in one state.

### Prevention and reusable lessons

- Every transition out of a modal surface must explicitly commit its destination overlay state;
  state-scoped entity cleanup is not a substitute for resource lifecycle ownership.
- Navigation regressions should press the rendered `FlowButton` entities and cover at least one
  complete cross-screen path, not only inject internal action values.
- Do not render an enabled action unless its real target and required facts exist. Omit the action
  or present a factual disabled state.
- Keep overlay permission tests, rendered-button dispatch tests, and destination-state tests
  together around destructive navigation boundaries.

These lessons are now encoded in focused regression tests and the M03 hardening scope. No new
general-purpose skill or framework is warranted; the failure was specific to the existing product
flow ownership and is prevented most directly at that boundary.

## Closeout

M02 completed on 2026-08-22 after implementation, canonical verification, routed E2E evidence,
native user acceptance of the corrected menu/change-server/connect path, feedback disposition, and
this learning review. M03 is now the next milestone and remains `Not started`.
