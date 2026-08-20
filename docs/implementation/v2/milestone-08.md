# Milestone 08 — Server-hosted bot practice

## Tracking

| Field | Value |
|---|---|
| Status | User playtest |
| Prepared | 2026-08-20; researched during M07 implementation by explicit user direction; corrected to server-hosted practice by user direction |
| Objective | Let one connected player select any supported server game type and immediately request an authoritative match with bots filling every other roster position, without entering or changing multiplayer queues |
| Entry dependency | Satisfied 2026-08-20: M07 is complete. Confirm its final UI, Results, participant, and protocol seams against the implementation starting commit before editing them |
| Research | Complete for the current product contracts, live M01–M07 worktree, pinned Bevy 0.19.1/Lightyear 0.29.0/Avian 0.7.0 material, and current primary documentation |
| Implementation | Complete 2026-08-20; automated verification passed and the milestone is ready for interactive playtest |
| Scope authority | User chose selectable server-hosted practice for any game type, unchanged queues, names as the only bot label, and inert bots with AI deferred to a later version |

## Player-visible outcome

From Title, the player chooses **Practice**. Brawler uses the normal server selection/direct-connect
path, then presents that server's compatible advertised game types. The player selects a game type
and build, then chooses **Start Practice**.

The server validates the request and immediately allocates an ordinary authoritative match worker.
The player occupies team 0's first slot; authoritative bots fill every other slot required by the
selected 1v1, 2v2, or 3v3 topology. Practice does not create, consume, or wait on a queue ticket.

Bots use ordinary participant names—`Bot 1`, `Bot 2`, and so on—everywhere names already appear.
M08 adds no separate HUD badge or bot-specific presentation. Wipeout and Hot Zone use their ordinary
maps, rules, HUD, scoring, objective, combat, respawn, and completion systems.

Practice Results reuse M07's product surface:

- **Play Again** requests a fresh server match with the same game type and build;
- **Change Game** returns to that server's practice game selection;
- **Exit Practice** returns to Title through the normal server disconnect path.

Server rejection, capacity exhaustion, or worker failure uses the normal recoverable server error
surface. The client never hosts or simulates authority and never launches server processes.

## KISS scope contract

1. Reuse the connected server's validated game-type advertisements. Every current Wipeout and Hot
   Zone game type is practice-compatible, independent of team size.
2. Reuse the existing build resolver. The player's build is validated normally; bots rotate through
   the four existing embedded presets in deterministic roster order.
3. Reuse the routed supervisor/lobby/match-worker topology and existing packet, IPC, manifest,
   worker admission, replication, and match lifecycle.
4. Add one lobby practice request that bypasses queue admission and asks the supervisor for ordinary
   match-worker capacity.
5. Materialize bots as ordinary authoritative fighters with neutral input. They stand still and do
   not aim, move, fire, use abilities, or pursue objectives in M08.
6. Add no queue policy or bot ticket. Multiplayer pools remain exact-human FIFO pools.
7. Add no bot AI, AI framework, navigation, difficulty, tactics, client host mode, or second
   gameplay path. Bot behavior belongs to the planned later version.

M08 excludes:

- bots in multiplayer queues, delayed filling, human backfill, join-in-progress, or replacing a
  disconnected player with a bot;
- any bot AI, including targeting, movement, firing, abilities, objective behavior, navigation,
  difficulty selection, tactics, personality, or balance work;
- client-launched supervisors/workers, packaged server helper binaries, offline practice, split
  screen, or LAN host-client mode;
- changing game rules, maps, builds, queue counts, or queue admission semantics;
- accounts, progression, rewards, or persisted practice results;
- a new crate, third-party dependency, or general request framework.

## Research findings

### Existing product and authority seams

- M01–M06 deliver the routed supervisor/lobby/match-worker topology, server-advertised game types,
  build resolution, worker allocation, handoff, Results, and cleanup.
- M07 owns the HUD, scoreboard, participant names, menu, settings, and Results presentation that M08
  extends rather than duplicates.
- `src/client/shell.rs` contains the Practice entry and `src/client/flow.rs` owns connection,
  GameSelect, BuildEditor, MatchLoading, Match, and Results transitions.
- `src/server/lobby/catalog.rs` resolves current 1v1, 2v2, and 3v3 Wipeout/Hot Zone game types. No
  separate client or bundled practice catalog is needed.
- `src/server/lobby/queue.rs` owns queue tickets and build validation. Practice reuses only pure
  build-candidate validation and never enters `QueueState`.
- Routing allocation/manifest code assumes participants have sessions, routes, and capabilities.
  Bots therefore need separate bounded roster rows, not fake clients.
- Match workers currently materialize fighters after human Match hellos. A focused shared
  materialization helper is justified for bot installation; a participant framework is not.
- A fighter without `ControlledBy` is already treated as server-local for match participation.
  Installing bots with neutral input is sufficient; M08 needs no decision schedule or AI seam.

### Pinned engine and networking material

| Source | Finding | Decision |
|---|---|---|
| `Cargo.lock` and root `Cargo.toml` | Versions are Bevy 0.19.1, Lightyear 0.29.0, Avian 2D 0.7.0 | Transfer APIs only from these versions |
| `references/lightyear/examples/simple_box/{README.md,Cargo.toml,src/{protocol,server}.rs}` | `ControlledBy` denotes a network owner | Bots have no `ControlledBy`, input buffer, connection, or route |
| `src/matchplay/server.rs::participant_is_connected` | Fighters without `ControlledBy` are treated as server-local participants | Reuse the current lifecycle instead of faking client connections |

Process-launch and AI research from superseded proposals are not applicable to this design.

## Accepted design

### Practice is a server session purpose, not a game type

Keep a small client presentation context:

```text
SessionPurpose = Multiplayer | Practice
```

It changes labels and available UI actions only. It does not choose authority, launch a process, or
cross the gameplay wire. The Title action selects this purpose before the normal connection begins;
after connection, Practice uses that server's current validated catalog advertisement.

Practice shows advertised Wipeout and Hot Zone entries. A future unsupported mode is unavailable
for Practice with a clear reason; M08 adds no per-game bot configuration or silent catalog revision.

### Practice flow

```text
Title
  -> Practice
  -> normal server selection / connection
  -> Practice GameSelect
  -> BuildEditor / Start Practice
  -> MatchLoading -> Match -> Results
       -> Play Again -> fresh server practice allocation
       -> Change Game -> Practice GameSelect
       -> Exit Practice -> normal disconnect -> Title
```

Reuse existing screen roots, focus, build draft, error overlay, handoff, and Results snapshot.
Disconnect follows the existing lost-server path. There is no practice startup screen, local
process state, or special app-exit cleanup.

### Lobby practice command

Register one bounded ordered-reliable request/outcome pair on `SessionChannel`:

```text
PracticeStartRequest
  request_id
  catalog_revision
  game_type_id
  game_type_configuration_revision
  complete BuildCandidate

MatchmakingServerPhase
  PracticeRejected { request_id, reason }
  ReservationStarted { ticket_id: None, ... }
  BeginMatchConnect { human MatchRouteGrant, ... }
```

Any authenticated lobby session may request Practice. The lobby allows one pending request per
session, replays byte-equivalent duplicate request IDs, rejects conflicting/stale requests, and
clears pending state on disconnect or terminal allocation response. Ordered-reliable transport
handles delivery; M08 adds no queue retry bucket or parallel command bus.

Validation reuses exact catalog identity and a pure build-candidate resolver extracted from queue
admission. It creates no ticket, admission order, pool revision, or population change. Rejections
cover stale catalog/build data, unsupported mode, an existing pending request, and unavailable
supervisor worker capacity.

The lobby assigns the human to team 0 slot 0 and creates bots in stable `(team, slot)` order. Bot
builds rotate through presets 1–4. Names are `Bot 1` through `Bot 5`. No behavior profile exists in
M08.

### Routing and manifest contract

Keep current participant rows for human network participants. Add separate bounded bot rows:

```text
AllocateBot / MatchManifestBot
  player_id
  team
  display_name
  source_build_preset
  recipe_fingerprint
  build_revision
  build_snapshot
```

Allocation and manifest gain a bounded `bots` vector. Validation requires:

- exactly one human and at least one bot for practice;
- zero bots for every multiplayer queue allocation;
- `humans + bots == team_count * players_per_team` and exact team sizes;
- unique stable IDs and valid display/build data across both vectors;
- no bot session, Netcode ID, peer, route, capability, or grant.

The supervisor uses its existing worker-capacity allocator. It routes/grants only the human, copies
bots into the match manifest, and sizes the worker from the complete roster. If capacity is full,
the request is rejected immediately; it is not held in a separate wait list.

The match worker validates both row types before Ready. Network admission matches only humans.
MatchLoading connection/check-in counts humans; roster readiness counts humans plus installed bots.

### Bot fighter identity and inert behavior

Bots receive normal stable identity, display name, team, loadout, match membership, spawn, combat,
collider, replication, and interpolation components. They do not receive `ControlledBy`, an input
buffer, connection/session state, or client prediction ownership.

Bot installation occurs after map/spawn and match-root availability. Give bots the minimum neutral
input state required by existing fighter systems, but add no bot controller, intent system,
targeting state, or behavior. Invalid data fails worker startup rather than shrinking the roster.
Implementation kept this in one focused server-owned plugin instead of refactoring the stable human
admission path for a single second use.

Names `Bot 1` through `Bot 5` flow through the existing replicated display-name field into every
surface that already shows participant names. M08 adds no `ParticipantKind` protocol component,
`BOT` badge, bot HUD branch, or bot-specific scoreboard/Results rule.

Bots remain stationary and take no actions. Existing combat, defeat, respawn, scoring, attribution,
and Results rules still apply when a human interacts with them. Implementing bot decisions is
explicitly deferred to the planned later bot-AI version.

## Plugin and schedule composition

| Owner | Change |
|---|---|
| `src/client/{shell,flow,queue}.rs` | Offer Practice after connection, reuse GameSelect/BuildEditor, route request and Results actions |
| `src/lobby.rs` / `src/protocol.rs` | Practice request/outcome; update protocol fingerprint atomically |
| `src/server/lobby/mod.rs` | Authenticated validation, build reuse, bot roster, one pending allocation; no queue ownership |
| `src/server/practice.rs` | Install validated bot rows as ordinary inert authoritative fighters |
| `src/server/admission.rs` | Validate complete manifest while authenticating only humans |
| `packages/brawler-routing` allocation/manifest/runtime | Separate bot rows; route humans only; ordinary server capacity allocation |

Do not add a bot-AI module or crate, client launcher, supervisor mode, packaging artifact, or public
AI API.

## Implementation slices

1. **Practice request without queues:** reconcile M07, add Practice UI, extract build resolution, add
   the authenticated request/outcome, and prove queue state unchanged.
2. **Inert bot roster transport:** add bot allocation/manifest rows, human-only routing, worker bot
   installation, stable names, and neutral input.
3. **All modes and topologies:** verify inert bot rosters in every current game type and normal
   human-driven match/Results lifecycle.
4. **Verification and playtest:** run automated/process/performance checks, keyboard/controller
   playtest, feedback triage, and learning review.

Do not begin slice 1 until the user validates this corrected specification. Slice 1 records the
final M07 seam check against the implementation starting commit.

## Verification plan

### Automated behavior

- all current Wipeout/Hot Zone types are accepted; an unsupported mode is rejected;
- build validation matches queue admission but creates no queue state;
- exact 1v1/2v2/3v3 rosters have one human, deterministic bots, and unique IDs;
- allocation/manifest codecs round-trip separate human/bot vectors and reject invalid bounds/IDs;
- routes, grants, and capabilities equal human count, never total roster count;
- bot install creates ordinary named fighters with no network ownership or `ParticipantKind`;
- across repeated fixed ticks bots remain stationary, never aim/fire/use abilities, and do not
  affect Hot Zone occupancy except when their spawn already overlaps it;
- humans can damage, defeat, and trigger normal respawn/scoring against bots;
- complete rosters activate while network connection/check-in remains one human.

### Network and real-process behavior

- routed First Blood practice reaches Active with one human route and one replicated bot;
- representative Wipeout and Hot Zone matches reach authoritative Results through human action or
  normal match time/objective rules without bot decisions;
- a catalog-driven matrix starts every current game type with exact human/bot/team counts;
- Play Again allocates afresh; Change Game returns to practice selection; Exit Practice follows the
  normal disconnect path to Title;
- stale, malformed, conflicting, duplicate, and capacity-rejected requests never mutate queues;
- disconnect, lobby/match failure, leave, and supervisor shutdown clean ordinary server state;
- multiplayer allocation tests continue to produce zero bot rows.

### Performance and bounds

- measure one 3v3 worker with five inert bots in both modes and current terrain/combat;
- fixed-tick p95 remains below 16.67 ms and shows no unexplained material regression from installing
  five additional inert fighter entities;
- bot state is O(roster) and adds no AI scans, spatial casts, or decision systems;
- request-to-controllable time remains under one minute and worker Ready under five seconds when
  capacity is available;
- repeat 25 request→complete/leave→lobby cycles with zero retained routes, workers, queue tickets,
  pending practice requests, or bot entities.

Canonical checks remain `just lint`, `just check`, `just test`, `just e2e 2`,
`scripts/check-server-features.sh`, `git diff --check`, plus one focused server-hosted practice
scenario through the existing test/CI surface.

## Implementation and verification evidence

Implemented on 2026-08-20:

- Practice is an enabled Title action and reuses Server Select, advertised Game Select, Build
  Editor, MatchLoading, Match, and Results.
- `PracticeStartRequest` is a single-flight client/lobby command. Acceptance reuses the existing
  `ReservationStarted` and `BeginMatchConnect` phases with `ticket_id: None`; rejection has one
  bounded practice-specific phase.
- Practice allocation contains one human and deterministic `Bot N` rows. Supervisor grants and
  routes remain human-only; bot rows pass into match-manifest v3.
- The private control contract is version 3 because allocation and manifest canonical bytes
  changed. Mixed worker binaries therefore reject each other instead of mis-decoding.
- Match workers validate bot identities/builds and install replicated fighters with neutral input,
  no `ControlledBy`, no client/session/route identity, and no AI system.
- Results labels are Play Again, Change Game, and Exit Practice; multiplayer labels and queue
  behavior remain unchanged.

Evidence:

| Command or test | Result |
|---|---|
| `just check` | Passed routing, client, server, and network-test build graphs |
| `just lint` | Passed formatting, all-target Clippy with warnings denied, and server feature isolation |
| `just test` | Passed 83 routing, 353 client, 301 server, 81 existing network, and 14 performance tests |
| `just e2e 2` | Passed real supervisor/lobby/match-worker First Blood activation and clean process teardown |
| `queue::practice_request_bypasses_queue_and_starts_one_human_three_v_three_reservation` | Added afterward as network test 82; focused run passed one-human 3v3 over Crossbeam with zero queue tickets |
| `server::lobby::tests::practice_uses_one_human_and_fills_the_selected_roster_with_named_bots` | Passed exact five-bot names and 3v3 team counts |
| `server::admission::tests::match_worker_materializes_manifest_bots_as_inert_ordinary_fighters` | Passed ordinary fighter install, neutral input presence, and absence of network ownership |
| `git diff --check` | Passed |

The existing fixed-tick performance suite remains well below 16.67 ms; its largest reported p95 in
this run was 4.956 ms. Interactive Practice navigation, each advertised mode, normal combat against
inert bots, and Results actions remain the user-playtest gate.

## Manual playtest

Start the normal server, then connect through the ordinary product flow.

1. Choose Practice, select First Blood and a build, and confirm `Bot 1` appears anywhere the normal
   participant name appears, with no extra bot HUD badge.
2. Play Again; then Change Game and run Wipeout 2v2 with one ally and two enemy bots.
3. Run Hot Zone 2v2 and 3v3 and confirm every bot stays inert while ordinary rules still run.
4. Exit Practice and confirm the normal disconnect path returns to Title.
5. Exhaust worker capacity and confirm Practice is rejected clearly without creating a queue.
6. Disconnect during MatchLoading, Active, and Results and confirm normal server cleanup.

Requested observations: Is server-hosted-but-not-matchmade Practice clear? Are stationary named
targets sufficient for testing builds and modes at this stage? Are Results and errors clear?

## Risks and explicit defenses

| Risk | Defense |
|---|---|
| Bots become fake clients | Separate bot rows; no session, route, capability, buffer, or `ControlledBy` |
| Practice mutates queues | Dedicated request path and unchanged-queue assertions |
| Practice bypasses capacity | Existing supervisor allocation gate; immediate full rejection |
| “Any game type” becomes a framework | Reuse current Wipeout/Hot Zone advertisements |
| MatchLoading waits for bots | Connection counts humans; installed roster includes bots |
| Bots affect PvP | Queue allocations require zero bot rows |
| M08 duplicates planned bot AI | Bots have neutral input only; no decision system or AI abstraction |
| Bot-specific presentation expands | Reuse `Bot N` display names; add no replicated kind or HUD branch |
| M07/M08 UI overlap | Reconcile M07, then extend its models and roots |

## Exit criteria

- [x] M07 is complete and this file is reconciled against its delivered seams.
- [x] The user validated this corrected server-hosted specification and directed implementation on
  2026-08-20.
- [x] A connected client can select every compatible advertised game type and request Practice
  without entering a queue.
- [x] The server allocates one human plus a complete bot roster subject to ordinary capacity.
- [ ] Bots remain inert while ordinary human-driven Wipeout/Hot Zone rules reach Results.
- [x] Existing name surfaces reuse `Bot N`; there is no extra bot labelling or protocol kind.
- [x] Bots never receive network-client identity or ownership, and M08 adds no AI behavior.
- [ ] Results actions, rejection, leave, disconnect, and failures have bounded flows.
- [x] Multiplayer queue, routed PvP, role isolation, protocol, network, and performance gates pass.
- [ ] Practice reaches play within one minute when capacity exists; five-bot 3v3 stays in budget.
- [ ] Keyboard/mouse and controller playtests cover every current game type; feedback is triaged.
- [ ] Learn-from-errors review is recorded before M08 becomes Complete.

## Specification-review questions

The user accepted the flow choices, selected `Bot N` names as the only bot labelling, deferred all
bot AI to a later version, and directed implementation. No product choice remains open; interactive
playtest feedback is now required before closeout.
