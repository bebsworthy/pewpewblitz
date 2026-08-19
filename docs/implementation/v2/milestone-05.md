# V2 milestone 05 — Exact formation and match handoff

## Tracking

| Field | Value |
|---|---|
| Status | User playtest |
| Prepared | 2026-08-19; simplified during implementation review on the same date |
| Objective | Turn an exact lobby queue roster into one isolated authoritative match and reach the existing server-owned Countdown |
| Entry dependency | M04 complete |
| Scope authority | User validated implementation, then explicitly directed the implementation to be reduced to the smallest lobby-queue vertical slice |

## Player-visible outcome

Players choose an advertised game and join its queue. When the oldest exact roster exists, the
lobby reserves it, starts a compatible match worker, and moves those players to Match Loading. Each
player receives only their own route grant and creates a fresh match connection through the same
public UDP endpoint.

Match Loading reports connecting, synchronizing, and waiting-for-players progress. Once every
manifest participant is connected and checked in, the match worker marks that roster ready and the
existing authoritative match lifecycle starts Countdown. The client observes that Countdown; it
does not invent another one.

If anything fails before Active—cancel, disconnect, timeout, rejected allocation, failed worker, or
Countdown departure—the attempted reservation ends. Affected clients return through a fresh lobby
connection to Game Select and may join again. M05 does not preserve an old queue position or resume
a partially started match.

M05 deliberately supports one product match per logical server. Once it becomes Active, new queue
admission is unavailable until the process is restarted. Reusable match completion, Results,
Queue Again, Change Game, and concurrent product matches belong to M06.

## Scope

M05 includes:

- exact oldest-roster formation for advertised 1v1, 2v2, and 3v3 games;
- deterministic balanced team assignment and compatible-map selection;
- one supervisor allocation request containing the selected game topology and roster;
- bounded application-owned build snapshots in the immutable worker manifest;
- one private route grant per participant and direct `BeginMatchConnect` delivery;
- a fresh lobby-to-match Lightyear connection and manifest-only match admission;
- client readiness based on accepted admission, map, terrain, assets, and controlled state;
- worker-owned full-roster check-in and authoritative Countdown start;
- a small Match Loading UI and minimal Match presentation;
- clean pre-Active failure and cancel behavior;
- 1v1, 2v2, and 3v3 routed process smoke coverage; and
- preservation of the named direct-UDP development baseline.

M05 excludes:

- parties, skill matching, backfill, join-in-progress, or partial rosters;
- reservation offers or a roster-wide offer acknowledgement barrier;
- detached tickets, reconnect leases, or reservation reconciliation;
- automatic requeue, FIFO restoration, participant blame, or failure cooldown policy;
- supervisor prepare/commit activation arbitration;
- multiple simultaneous product matches;
- post-Active leave/forfeit, result reconciliation, rematch, or return-to-queue lifecycle; and
- release-ready Match presentation.

## Technical contract

### Formation

The lobby queue remains the only matchmaking authority. A game forms when its pool contains at
least `team_count * players_per_team` application-acknowledged tickets. It removes exactly the
oldest required tickets, ordered by `(admission_order, ticket_id)`, and leaves overflow queued.

V2 game types have exactly two teams and one, two, or three players per team. Sorted participants
receive `team = index % 2`, producing a stable balanced roster. Compatible maps rotate in catalog
order after a complete worker grant set is accepted.

The reservation owns one unpredictable nonzero reservation ID, one allocation request ID, its
selected game/map/rules topology, and the immutable participant rows. A ticket is either queued or
reserved; there is no third recovery state.

### Allocation and manifest

The lobby sends one semantic allocation request. Transport may retry the identical request, but no
second queue or roster decision exists in the supervisor. The supervisor validates bounds and host
capacity, starts one isolated match worker, and returns one complete grant set after manifest/Ready
validation.

The request and manifest carry stable game, map, topology, rules, player, team, routed peer, and
Netcode identities. Each participant also carries a maximum-256-byte opaque
`MatchBuildSnapshotV1`. The match worker decodes and revalidates that snapshot against embedded
content before it reports Ready. The rule payload is the resolved objective target plus match,
countdown, and respawn ticks from the selected flat catalog entry. Wipeout resolves
`kills_to_win`; Hot Zone resolves `capture_seconds`. Routing code never imports gameplay
definitions or interprets those values.

The single current routing/control/manifest schema and the global application protocol version are
updated together. M05 adds no compatibility decoder or per-message version.

### Handoff

When all grants are present, the lobby sends each participant one correlated
`BeginMatchConnect` containing only that participant's grant and public match summary. There is no
pre-offer phase. The client intentionally tears down the lobby session, waits for the deferred
entity boundary, and creates one fresh match session.

The router checks the capability. The match worker admits only the immutable manifest's Netcode
client ID and routed peer pair and rejects duplicates or extra players.

### Loading and Countdown

The client sends one idempotent Ready action after all of these are true:

- Match Hello was accepted;
- the authoritative map snapshot is ready;
- matching terrain convergence is ready;
- required assets are ready or have a declared degraded fallback; and
- the controlled fighter/build state matches manifest-backed admission.

The match worker records each manifest participant once. When the exact roster is connected and all
participants have checked in, it directly marks those authoritative participants ready. The
existing fixed-tick match lifecycle then performs `Waiting -> Countdown -> Active`.

This is the authoritative countdown: the deadline stored in replicated `MatchState` and advanced
by the server's fixed-tick lifecycle. Match Loading status is presentation only and cannot start,
delay, or replace it.

### Failure and cancellation

Before Active, any terminal loading failure completes the reservation and cancels the allocation.
The client receives a bounded terminal outcome where possible, disconnects, opens a fresh lobby,
and returns to Game Select. Joining again is a new queue request with a new position.

A player may cancel while the match is Waiting and readiness has not committed. The worker accepts
the cancellation, sends the cleanup fact to the supervisor, and terminates loading. After readiness
commits or Countdown begins, cancellation is too late; a subsequent departure still makes the
incomplete start terminal rather than returning the immutable roster to Waiting.

The lobby does not identify an innocent roster, retain old tickets, or reconcile a failed handoff.
This intentionally favors a small, obvious failure path over speculative fairness machinery.

### Temporary one-match boundary

M05 admits at most one reservation or Active product match. While a reservation is forming or
loading, other queue pools may remain visible. When the match reaches Active, queued overflow is
removed with an honest capacity-unavailable outcome and new joins are rejected. M06 replaces this
temporary boundary with reusable concurrent worker lifecycle.

## Ownership and schedule

| Concern | Owner |
|---|---|
| Queue membership, exact formation, reservation, map choice | Lobby worker |
| Host capacity, process spawn, routes, grants, revocation | Supervisor |
| Immutable roster and build validation | Match worker startup |
| Match admission, check-in, readiness, Countdown | Match worker |
| Connection replacement and loading presentation | Client flow/session |
| Combat, score, terrain, and match phase | Existing authoritative gameplay systems |

Lobby command/disconnect handling runs before formation so a same-update queue Cancel or lobby
disconnect wins. Client lobby teardown is applied before the replacement match entity is spawned.
On the worker, loading messages are processed before readiness commit; readiness commit occurs
before the fixed-tick lifecycle can enter Countdown. Product departure detection prevents a
Countdown failure from silently restoring Waiting.

## Implementation checklist

- [x] Advertise and validate exact 1v1, 2v2, and 3v3 game types.
- [x] Form the oldest exact acknowledged roster with deterministic teams and map selection.
- [x] Add reservation ownership without adding a second matchmaking authority.
- [x] Carry selected topology and bounded build snapshots through allocation and manifest.
- [x] Generalize routing allocation and worker admission to two, four, or six participants.
- [x] Deliver complete grants directly as per-participant Begin messages.
- [x] Integrate fresh routed lobby-to-match connection replacement into product flow.
- [x] Add Match Loading progress, cancel confirmation, and minimal Match presentation.
- [x] Add exact client readiness and server check-in.
- [x] Let the match worker directly unlock the existing authoritative Countdown.
- [x] End failed pre-Active attempts cleanly without requeue/reconciliation machinery.
- [x] Block additional product admission after the one M05 match reaches Active.
- [x] Put routed 1v1/2v2/3v3 coverage behind the shared `run` and `e2e` commands.
- [x] Complete the final canonical verification matrix after simplification.
- [ ] Run the user playtest and record feedback decisions.
- [x] Complete the learn-from-errors review.
- [ ] Close the milestone after playtest feedback triage.

## Verification plan

Automated gates:

1. `just lint`
2. `just test`
3. `just e2e 2`
4. `just e2e 4`
5. `just e2e 6`

Focused assertions cover exact formation, balanced topology, map capacity, build snapshot
revalidation, recipient-correct grants, manifest-only admission, the complete readiness predicate,
full-roster Countdown activation, clean pre-Active failure, and the one-match admission boundary.

Manual playtest:

1. Run `just run 2` and join First Blood from both windows.
2. Confirm both clients reach Active and the first defeat completes the match.
3. Run `just run 4` and join one advertised 2v2 game from all four windows.
4. Confirm Queue changes to Match Loading only for the exact roster.
5. Confirm all clients show the same game/map/topology and only their own build.
6. Confirm all clients reach the server-authored Countdown and then Active.
7. Run `just run 6` and repeat with the 3v3 game and all six windows.
8. Repeat once with a pre-Countdown cancel or disconnected client; confirm the attempted start ends
   and clients can return to Game Select and queue afresh.

## Exit criteria

- Exact 1v1, 2v2, and 3v3 rosters reach an isolated match worker's authoritative Countdown.
- No client can start Countdown, join outside the manifest, or receive another player's grant.
- Pre-Active failure leaves no reservation or invisible queue membership.
- The direct-UDP baseline remains available.
- Canonical checks and the playtest pass, feedback is triaged, and closeout learning is recorded.

## Verification evidence — 2026-08-19

- `just lint`: passed for routing, client, and server with warnings denied.
- `just test`: passed — routing unit/process suites, 344 client tests, 296 server tests, 81
  separate-app network tests, and 14 performance gates.
- `just server-features`: passed; the server graph excludes client presentation capabilities.
- `just network-product-match-smoke`: exact 2v2 reached authoritative Active.
- `just network-product-match-3v3-smoke`: exact 3v3 reached authoritative Active. One earlier run
  timed out during initial lobby connection retries; the bounded rerun passed, and the final
  post-simplification run also passed.
- `just network-direct-smoke`: both direct-UDP baseline clients exited successfully.
- Focused post-audit cancellation checks passed: completing a reservation removes the exact four
  reserved tickets while leaving overflow queued, and an accepted client cancellation clears the
  loading reservation and produces exactly one return-to-Game-Select observation.
- After correcting the windowed launcher mode, `sh -n scripts/network-product-match.sh`,
  `git diff --check`, and a fresh `just network-product-match-smoke` passed; the 2v2 roster again
  reached authoritative Active.
- After correcting product Match input ownership, the focused flow transition regression passed,
  all 346 client library tests passed, and client Clippy completed with warnings denied.
- After correcting completed-match return presentation, both focused completion/lobby-generation
  regressions passed, all 348 client library tests passed, and client Clippy completed with warnings
  denied.
- After allowing explicit rematches for returning identities, the focused second-allocation
  regression passed, the complete server library suite passed, and server Clippy completed with
  warnings denied.
- After making formation require live lobby sessions, the exact stale-ticket regression passed,
  all 298 server library tests passed, and server Clippy completed with warnings denied.
- `just network-first-blood-smoke`: the advertised exact 1v1 formed, both clients completed routed
  handoff, and the worker reached authoritative Active using the catalog's one-kill objective.
- First Blood follow-up verification passed: 299 server library tests, 348 client library tests,
  routing's exact-1v1 validation regression, and client/server Clippy with warnings denied.
- After replacing authored profiles with flat per-game parameters, 298 server tests, 348 client
  tests, and every routing target passed; routing/client/server Clippy passed with warnings denied.
  Fresh First Blood 1v1 and Hot Zone 3v3 process smokes both reached authoritative Active.
- The follow-up consistency pass updated the player UX contract, M01 manifest field list, M03
  operator-catalog specification, M05 contract, roadmap, README, and owned code comments. `just
  docs`, formatting checks, and whitespace validation passed.
- The command-surface cleanup reduced the advertised `just` recipes from 70 to 11. `just lint`,
  `just test`, and the two-client `just e2e` path passed; the new routed server launcher also
  started and stopped cleanly on an alternate local port while an existing port-5000 session was
  left untouched.

The redundant match-loading acknowledgement removal was followed by the full lint/test/smoke
matrix. The subsequent removal of the unused successful check-in response was followed by
formatting, warning-free lint, both complete role checks, and fresh 2v2, 3v3, and direct smokes.

## Feedback review

| Feedback | Decision | Result |
|---|---|---|
| “Review the complexity around the core objective; this is getting overengineered.” | Implement now | Replaced the durable-transaction-style recovery design with a disposable reservation and clean fresh-join failure path |
| “What is Authoritative Countdown?” | Clarify now | The contract now names the replicated server `MatchState` deadline and fixed-tick transition as the only Countdown authority |
| “Simplify this as much as possible. It’s a lobby queue, not a space rocket.” | Implement now | Removed offer ACKs, leases, reconciliation, requeue restoration, blame/cooldowns, activation prepare/commit, and redundant loading ACK/success messages |
| `just network-product-match` loops between Connecting and Preparing Sandbox | Implement now | Removed the automation-only `--auto-connect` mode from the windowed launcher so it opens the product shell; headless match smokes retain automation mode |
| Match reaches Active, but WASD and mouse input do nothing | Implement now | Entering product Match now transfers the existing input context from Shell to Gameplay; leaving Match returns it to Shell, while pause remains stateful |
| Match completion exposes “Preparing sandbox” and “waiting for match state” | Implement now | A retained Match Complete overlay now covers the routed return, and Game Select resumes only after the fresh lobby authenticates; stale loading state is cleared with the lobby generation |
| The same four players queue again, a second worker starts, but nobody connects | Implement now | Removed the M01 one-allocation-per-client rejection from explicit product allocations; returning identities reuse their existing bounded identity slots and receive fresh grants |
| Three players queue, one disconnects, and one replacement causes a three-player match attempt | Implement now | Formation now cross-checks every selected ticket against a current non-disconnected lobby entity, so a stale ticket can remain visible briefly but can never count toward an exact roster |
| Add a short 1v1 match where one kill wins, named First Blood | Implement now | Added an advertised First Blood game, an authoritative one-kill objective, exact 1v1 admission, and two-client windowed/smoke commands |
| Replace inconvenient game-type rule profiles with basic parameters and no shared defaults block | Implement now | Every game now directly declares `kills_to_win` or `capture_seconds`, plus its own match duration, countdown, and respawn seconds; resolved values travel in the worker manifest |
| Replace the growing flat list of `just` recipes with development basics plus `server`, `client`, and `run <n>` | Implement now | Reduced the advertised surface from 70 recipes to 11; deterministic checks live under `test`, real-process matches live under `e2e [2|4|6]`, and specialized evidence remains in focused scripts |

No feedback item is deferred or rejected. The remaining feedback gate is a rerun of the hands-on
1v1/2v2/3v3 and cancel-path playtest using the corrected canonical windowed commands.

## Learn-from-errors review

### What went wrong

- The first specification designed fair recovery for every partial handoff before proving the
  basic queue-to-match slice.
- Reliable ordered network delivery was wrapped in another application ACK/replay protocol without
  a demonstrated failure it solved.
- Countdown authorization was split between the match worker and supervisor even though the match
  worker already owned exact roster readiness and authoritative match phase.
- The specification's size obscured the product rule: a failed start can simply be discarded.
- The first windowed match launcher accidentally reused the headless automation flag, which selects
  the legacy sandbox client instead of the product shell.
- Product flow reached Match without transferring input ownership from the shell to the existing
  gameplay input writer.
- Match completion started the correct fresh-session return but left the flow in Match after its
  replicated match state disappeared, exposing legacy HUD fallback copy.
- The product allocation path reused an M01 automation tombstone that deliberately prevented the
  same Netcode identity from receiving a second allocation.
- Exact formation trusted acknowledged queue tickets without a final live-session check at the
  commit boundary.

### Causes

- Infrastructure failure cases were treated as product requirements instead of deferred policy.
- Existing routing primitives were mistaken for a reason to expose more routing lifecycle states.
- “Recoverable” was interpreted as preserving an old queue transaction rather than allowing the
  player to return safely and try again.
- Headless and interactive arguments were combined in one launcher without checking the client's
  explicit product-shell mode predicate.
- Match presentation and authoritative activation were verified, but the local input-context seam
  was not included in the first end-to-end product-flow assertion.
- The return-to-lobby test verified transport lifecycle but not continuous product presentation.
- First-match smoke coverage exited at Active and never exercised an explicit second queue request
  by the same authenticated identities.
- Disconnect cleanup was tested in isolation, but formation was not tested against a stale ticket
  during the observer/deferred-removal window.

### Prevention and reusable lessons

- State the player-visible failure policy in one sentence before designing messages or state.
- Add a protocol phase only when it unlocks a distinct current behavior or authority decision.
- Keep readiness and Countdown with the match worker; the supervisor owns capacity and routing.
- For disposable pre-game work, prefer delete-and-retry over durable reconciliation.
- During specification review, search every proposed state for a current producer, consumer, and
  player-visible consequence; remove states lacking all three.
- Keep automation-only client flags inside the headless launcher branch and verify interactive
  launchers against `presents_product_shell()`.
- Every product-flow transition into playable gameplay must assert both the authoritative match
  phase and `ClientInputContext::Gameplay`.
- Routed lifecycle tests must assert the product screen before, during, and after entity replacement,
  not only the final authenticated generation.
- Compatibility-only allocation guards must not silently govern explicit product queue behavior;
  repeat-player tests are required wherever process-lifetime identity memory is shared.
- Cached queue membership may drive presentation, but exact formation must revalidate current live
  session ownership immediately before reserving a roster.

These lessons are already represented by the repository's no-over-engineering rules, so no new
project skill or generalized framework is warranted.

## Simplification record

Implementation review found that the earlier specification treated a disposable lobby reservation
like a durable distributed transaction. The following planned mechanisms were removed before
closeout: capability-free reservation offers, roster-wide offer ACKs, detached handoff leases,
fresh-session reconciliation, automatic FIFO-preserving requeue, recovery acknowledgements,
participant attribution/cooldowns, and supervisor activation prepare/commit.

The retained design protects the boundaries that matter—server authority, exact rosters, private
grants, immutable admission, bounded state, and fresh connections—while making a failed start a
simple failed start.
