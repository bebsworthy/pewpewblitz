# Version 11 implementation roadmap

## Purpose and scope

V11 promotes the existing inert Practice roster fillers into playable, readable, server-hosted
opponents. The player continues to choose an ordinary advertised game type and saved brawler from
the Player Dashboard, then starts Practice through the existing routed lobby transaction. The
allocated match worker gives every manifest bot a bounded deterministic controller that perceives
only permitted authoritative facts and produces ordinary validated `FighterInput`.

The durable capability contract is [Bots](../../10-bots.md). V11 implements that contract as one
complete player-visible milestone because useful practice opponents must navigate, fight, respect
concealment, and pursue the selected mode objective together. It does not create a bot framework,
external bot host, matchmaking fill, difficulty system, learned policy, or alternate gameplay
authority.

## Version status

| Field | Value |
|---|---|
| Status | Complete |
| Current milestone | Complete — M01 and V11 accepted on 2026-08-26 |
| Entry gate | Satisfied: V10 completed and was accepted on 2026-08-25, including the Feature Yard Wipeout/Hot Zone/Heist family, damageable objects, pickups, routed/capacity/native evidence, feedback triage, documentation reconciliation, and learning review |
| Completion gate | Start Practice creates useful deterministic Pulse/Dash opponents for every advertised Feature Yard topology; their perception, navigation, team/objective behavior, input production, lifecycle, boundedness, concealment fairness, performance, routed operation, native playtest, feedback, and learning gates all pass without adding another gameplay-authority path |

Allowed statuses are `Not started`, `Researching`, `Specification review`, `Implementing`,
`Verifying`, `User playtest`, `Feedback review`, `Complete`, and `Blocked`.

V11 planning was explicitly started by the user on 2026-08-25. The user approved M01 production
implementation on 2026-08-25. V11 completed and was accepted on 2026-08-26 after the playable
server-hosted slice, fair delayed perception, bounded resumable navigation, objective behavior,
ordinary-input authority, automated/routed/native evidence, two feedback rounds, affected
verification, durable-document reconciliation, and the learn-from-errors review passed closeout.
All V11 completion gates were satisfied and accepted on 2026-08-26.

## Implemented product decisions

1. The first playable bots are server-hosted controllers installed only on existing manifest bot
   fighters in Practice workers.
2. A bot observes a bounded owned snapshot assembled from an explicit server allowlist. The pure
   policy never queries or mutates Bevy ECS state.
3. Concealment fairness uses the same pure observer-specific visibility rule as a human observer,
   plus a fixed reaction delay and bounded last-permitted contact memory. Hidden current motion
   never enters the observation or memory.
4. A project-owned utility policy and deterministic bounded resumable path planner are the only
   V11 decision technologies. Measured maximum-topology and p95 evidence did not justify the
   proposed sector/portal hierarchy. No behavior-tree, GOAP, navmesh, ML, LLM, or remote policy
   dependency is added.
5. Bot decisions become ordinary `FighterInput`, pass the complete shared decoded-input validity
   rule, and atomically update the existing `ActionState<FighterInput>` plus local
   `InputFreshness`. Movement, attacks, abilities, damage, pickups, map mutation, scores, respawns,
   and outcomes remain owned by their current authoritative systems.
6. The first profile is one code-owned, validated behavior profile. V11 adds no player-facing
   difficulty, skill rank, adaptive tuning, or authored bot content format.
7. Every manifest bot uses one explicit saved-brawler-native recipe: default fighter profile,
   Pulse Sidearm weapon base, Dash ultimate, Adrenal Response, Close Quarters, and no equipped
   weapon parts. The recipe is resolved through the same profile/build catalogs as player brawlers
   and does not depend on a legacy full-build preset ID.
8. Wipeout, Hot Zone, and Heist provide focused goal candidates to one common policy. Team planning
   assigns only bot roles and reservations; it may account for a human teammate but never controls
   or reserves actions on the human's behalf.
9. Navigation is derived from resolved authoritative bounds and collision geometry into a private,
   revisioned graph. The policy does not depend on grid dimensions or today's map-size validation,
   and Avian remains the final collision authority.
10. Deterministic entropy is split into versioned independent streams. The same seed, profile,
    match/life identity, observations, navigation revision, and work budgets reproduce the same
    plan and decision trace.
11. Bot components, observations, plans, traces, and algorithm versions remain server-private.
    V11 changes no routing envelope, manifest wire shape, lobby admission/check-in, Lightyear input
    protocol, replication registration, or global application compatibility scheme.
12. The first slice must be useful and readable in native play before V11 expands to more weapons,
    abilities, avoidance behaviors, squad tactics, hosting adapters, or policy technologies.

## Milestone overview

| Milestone | Status | Player-visible deliverable | Plan |
|---|---|---|---|
| 01 | Complete | Start Practice provides active Pulse/Dash opponents that navigate Feature Yard, fight with imperfect delayed reactions, coordinate bounded roles, pursue Wipeout/Hot Zone/Heist goals, respect concealment, reason about V10 objects/pickups, respawn correctly, and remain deterministic and bounded in 1v1/2v2/3v3 | [milestone-01.md](./milestone-01.md); closed with accepted objective-priority and perimeter-recovery feedback, affected verification, documentation reconciliation, and learning review on 2026-08-26 |

## Ordering rationale

### M01 — Playable server-hosted Practice bots

There is no separate V11 architecture milestone. The current routed Practice path already owns bot
identity, team, build snapshot, spawn, fighter runtime, match worker, lifecycle, and replication.
M01 adds the smallest new responsibility that creates player value: fair observations become
validated local input for those existing fighters.

Splitting navigation, perception, or objective adapters into earlier infrastructure-only
milestones would not produce an opponent the player can meaningfully practice against. Splitting
the three existing modes would leave Start Practice behavior dependent on which ordinary game type
the player happened to select. M01 therefore implements the complete first behavior profile while
keeping build and hosting breadth deliberately narrow.

Gate:

- Start Practice, routed allocation, manifest validation, match-worker startup, results, Practice
  Again, requeue, reconnect, restart, and shutdown retain their current transactions and capacity
  ownership;
- only manifest bot fighters receive controller state, while connected humans and multiplayer
  matches cannot acquire it;
- canonical delayed observations contain only stable identities and permitted bounded facts, and
  concealment tests prove no current hidden pose or motion reaches policy or contact memory;
- one deterministic team planner and per-bot utility policy produce readable pressure, survival,
  range, retreat, contest, defend, safe-attack, lane-pressure, chest, pickup, and barrel behavior;
- one revisioned bounded resumable planner navigates direct and multi-turn routes, avoids diagonal
  corner cutting, handles delayed dynamic blockers, fails safely under budget exhaustion, and does
  not encode `MapDimensions` maxima as product invariants;
- the Pulse/Dash capability executor derives range, speed, ammo, cooldown, charge, and activation
  facts from the resolved match loadout, holds bounded aim error, and never invents a weapon- or
  ability-specific authority path;
- every emitted input passes the same complete finite/radial/button validation used after network
  decode, and input plus local freshness commit exactly once before authoritative simulation/fire;
- defeat, respawn, restart, map generation, navigation revision, match completion, and invalid
  decision paths clear or preserve the correct private state and fail closed to neutral input;
- deterministic trace, insertion-order permutation, synthetic large-topology, maximum 3v3 roster,
  repeated lifecycle, fixed-tick, memory, and bounded-work evidence pass;
- native keyboard/mouse and controller playtests confirm useful opposition, readable imperfection,
  fair concealment, goal competence, route reliability, and acceptable match pacing; and
- every feedback item is implemented, deferred, rejected with rationale, or marked as needing
  evidence, and the learn-from-errors review is complete before V11 is accepted.

## Version-wide architecture boundary

```text
existing routed Start Practice transaction
        |
        v
manifest bot -> ordinary authoritative fighter + private controller marker
        |
        v
server allowlist -> canonical observation history -> delayed permitted observation
        |
        v
pure team plan -> pure utility intent -> bounded deterministic route/capability decision
        |
        v
shared decoded-input validation
        |
        v
existing ActionState<FighterInput> + local InputFreshness
        |
        v
existing authoritative movement / combat / ability / map / pickup / mode / lifecycle
        |
        v
existing replication, client presentation, HUD, audio, results, and recovery
```

The controller may observe authority but may mutate gameplay only through validated input and its
controller-produced freshness marker. Pure behavior owns plans and returned private state. The
server adapter owns ECS collection, lifecycle, scheduling, input installation, diagnostics, and
failure handling. Existing gameplay plugins remain the sole owners of outcomes.

## Fixed-tick and lifecycle contract

M01 refines the established `FixedUpdate` ordering without adding another simulation schedule:

```text
GameplaySet::Lifecycle
  -> existing ApplyDeferred boundary
  -> bot lifecycle reconciliation and canonical observation capture
  -> GameplaySet::Input
       delayed-observation selection
       slower team-plan/tactic cadences and material interrupts
       bounded stable route work
       route following + Pulse/Dash capability execution
       decoded-input validation
       ActionState + InputFreshness commit exactly once
  -> GameplaySet::Simulation
  -> GameplaySet::Fire
  -> GameplaySet::Finalize
```

The exact bot-owned system sets and any extra deferred boundary must remain visible in the server
composition and be proven with schedule traces. No policy work depends on wall clock, asynchronous
task completion, system interleaving, or thread race. A fighter that is inactive, defeated,
respawning, incomplete, over budget, or invalid receives neutral input.

A private controller life generation advances once when authoritative lifecycle returns the bot to
`ActiveCombatant`. It resets observations, contacts, intent, target, aim, route, stuck, and
capability commitments. Match or map-generation change clears all bot/team/navigation state;
navigation revision invalidates only affected search and route state.

## Navigation and bounded-work plan

The server builds private navigation state from the installed `ResolvedMap` playable bounds and
resolved collision shapes. Current sparse-grid data lowers into stable cells and edges consumed by
the pure planner. Line of travel and line of fire remain distinct; path smoothing and route
following use fighter-clearance geometry, while Avian's existing move-and-slide result remains
authoritative.

M01 validates measured topology and runtime safety ceilings rather than a width/height promise. The
implemented planner retains one request per live bot, shares at most 512 expansions per tick across
the stable controller order, and fails safely when a request exhausts its ceiling. These are
safety/work limits, not difficulty or map-format limits. Synthetic 128x96 topology and maximum-
roster p95 evidence showed that adding the proposed sector/portal hierarchy would be unused
infrastructure in M01.

Public dynamic objects and terminal placement changes enter each bot's route decision only through
its delayed permitted observation. A global map-generation or collision-topology replacement
invalidates immediately; a newly destroyed barrel or opened chest does not become traversable
knowledge before the profile delay. Unexpected authoritative collision may trigger ordinary stuck
recovery without revealing the hidden cause.

## Content and tuning plan

M01 adds no gameplay catalog family. The canonical bot brawler uses existing stable fighter,
weapon-base, ultimate, and passive definitions and is encoded as the existing
`MatchBuildSnapshotV3` in the unchanged manifest row. Every bot receives the same recipe; stable
player identity still differentiates its entropy streams and display name.

One typed `BotProfile` owns reaction, contact, cadence, commitment, range, aim, retreat, navigation,
stuck, and capability values. The profile is code-owned and versioned for deterministic traces. It
is intentionally not a Balance Lab persistence/UI surface in M01 because it is neither authored
combat content nor a player-facing difficulty choice; implementation and feedback may overturn
that disposition only by returning to specification review. V11 must still record the review in
the Balance Lab guide before closeout.

## Network and compatibility plan

Server-hosted bot input does not traverse Lightyear and is not evidence for client input sequence,
freshness, rate, ownership, packet impairment, or hostile-message validation. M01 reuses only the
post-decode `FighterInput` validity rule and writes the local freshness tick explicitly. Ordinary
human input validation and `NativeBuffer` ownership do not change.

No bot state is registered or replicated. Clients learn bot actions only from ordinary replicated
fighter, projectile, ability, object, pickup, objective, score, cue, and lifecycle state. The
existing manifest snapshot and gameplay catalogs are sufficient, so M01 proposes no protocol,
routing, control-IPC, content-schema, recovery-message, or per-message version change.

## Verification strategy

M01 uses the smallest relevant layers:

- pure canonical-observation, visibility, contact-memory, utility, team assignment, capability,
  entropy, graph, search, smoothing, route-following, and budget tests;
- focused `App`/`World` fixed-tick tests for materialization, schedule ordering, input/freshness
  installation, authoritative consumption, lifecycle reset, restart, teardown, and failure paths;
- separate-App and routed tests proving ordinary replication/results and no new client authority;
- Wipeout, Hot Zone, and Heist 1v1/2v2/3v3 behavior scenarios over Feature Yard, including
  concealment, barrels, safes, chests, restoration pickups, defeat, respawn, and completion;
- deterministic permutation and trace-replay evidence independent of ECS insertion order;
- synthetic larger-topology and maximum declared navigation-capacity evidence;
- maximum practice-roster performance with observation, team, tactic, navigation, total bot, and
  whole fixed-tick timing reported separately; and
- native normal/primitive/reduced-effects, controller, keyboard/mouse, HUD/audio/results, and
  repeated Practice Again playtests.

Visual evidence can establish readability and usefulness. It cannot prove concealment privacy,
input-only authority, deterministic replay, bounded work, lifecycle cleanup, or exact schedule
ordering.

## Cross-version dependency decisions

- V2 routed Practice remains the only player entry and process topology. V11 replaces inert
  controller behavior, not practice allocation, routing, admission, or worker ownership.
- V5 Dashboard/results and Practice Again remain the complete player flow. V11 adds no bot setup or
  difficulty screen.
- V7 saved-brawler snapshots and resolved immutable loadouts are the only bot build source. V11
  removes the four-preset rotation from practice allocation but does not reopen the superseded full-
  build product flow.
- V8 resolved map recipes and collision geometry are the only navigation source. V11 creates no
  parallel authored waypoint map or client render-derived navigation.
- V9 observer-specific concealment is the only visibility rule. Bot contact memory stores only the
  last permitted delayed fact and never refreshes from hidden authority.
- V10 damageable objects, Heist safes, chests, pickups, and Feature Yard state remain existing
  authority. Bots interact through ordinary movement and attacks.
- V6 Balance Lab is reviewed because bot behavior introduces tuneable code values; M01's proposed
  disposition is to keep the single profile code-owned until a second profile or operator workflow
  demonstrates a persistence/UI need.

## Explicitly deferred beyond V11

- External headless bot clients, bot network admission/check-in, supervisor-managed bot processes,
  subprocess shutdown, or UDP impairment evidence for bot-hosted input.
- Multiplayer queue fill, absent-player substitution, join-in-progress backfill, parties, skill
  matching, rankings, or adaptive difficulty.
- Player-facing difficulty profiles, bot selection, per-bot build selection, or saved bot profiles.
- Additional straight/lobbed/melee recipes, active items, Sentry, Self Cloak, Reveal Scan,
  Concealment Field, or arbitrary build capability dispatch.
- Perfect projectile tracking, broad projectile avoidance, advanced coordinated tactics, formation
  systems, or map-control strategy beyond the bounded first roles/reservations.
- Replay corpus production, learned-policy training/inference, remote services, LLM calls, or an
  alternative policy competition harness.
- A public bot/navigation SDK, behavior tree, GOAP, general utility framework, navmesh dependency,
  or map-authored waypoint/AI language.
- Changing current map dimensions, authoring format, game-mode rules, combat balance, or content
  solely to make the bot simpler.

## Initial V11 backlog

| ID | Item | Disposition |
|---|---|---|
| V11-EXTERNAL-HOST | Run the same pure policy behind an official headless network client | Deferred until network/load evidence or third-party policy work creates a concrete host consumer |
| V11-DIFFICULTY | Multiple player-facing bot profiles or adaptive difficulty | Deferred until the first profile is accepted and a real player need is observed |
| V11-MORE-BUILDS | Additional weapon/ultimate capability executors | Deferred; promote one representative capability slice from playtest evidence |
| V11-PROJECTILE-AVOIDANCE | Bounded imperfect projectile awareness and dodging | Deferred; first prove readable aim, movement, and Dash behavior without perfect tracking |
| V11-LEARNED-POLICY | Trace corpus and learned-policy comparison | Deferred until a representative versioned corpus and explicit quality/operability gate exist |
| V11-NAVMESH | Alternative graph builder or navmesh | Trigger-bound; revisit only if a real future map representation cannot lower into the accepted navigation snapshot |
| V11-MULTIPLAYER-FILL | Queue fill or in-match replacement | Deferred; requires separate fairness, identity, admission, ranking, and lifecycle product decisions |

## Preparation sources

Pinned local sources inspected for V11 preparation:

- `src/server/practice.rs` and `src/server/lobby/mod.rs` for inert bot materialization, stable
  manifest rows, rotating legacy-preset selection, saved-brawler snapshots, and current spawn state;
- `src/gameplay.rs`, `src/movement/input.rs`, `src/movement/authority.rs`, and `src/protocol.rs` for
  fixed-set ordering, the deferred boundary, decoded input validation, local freshness, ordinary
  `ActionState<FighterInput>`, and allowed buttons;
- `src/concealment/model.rs`, `src/concealment/mod.rs`, and `src/concealment/network.rs` for the pure
  observer decision and the connection-keyed cache that a server-hosted bot must not fake;
- `src/map/catalog.rs`, `src/map/runtime.rs`, `src/map/objects.rs`, and `src/map/pickups.rs` for
  resolved bounds/colliders, dynamic generation/revision, stable placement/object/pickup facts, and
  existing capacity limits;
- `src/matchplay/wipeout.rs`, `hot_zone.rs`, `heist.rs`, `server.rs`, and `lifecycle.rs` for current
  authoritative mode facts and defeat/respawn/reset ownership;
- `src/builds/`, `src/profiles/`, `content/catalogs/builds.ron`, and
  `content/catalogs/weapons.ron` for the canonical Pulse/Dash saved-brawler recipe and resolved
  capability facts;
- `references/bevy/examples/app/plugin.rs`, `ecs/fixed_timestep.rs`, and `ecs/ecs_guide.rs` for
  focused plugin composition, fixed schedules, system sets, and explicit ordering;
- `references/lightyear/book/src/concepts/bevy_integration/system_order.md`,
  `concepts/advanced_replication/inputs.md`, and
  `references/lightyear/examples/network_visibility/src/server.rs` for input/replication timing and
  the distinction between network visibility and a bot's pure observer rule; and
- `references/avian/crates/avian2d/examples/ray_caster.rs` and `move_and_slide_2d.rs` for pinned
  spatial-query and final kinematic-collision behavior.

Pinned dependencies are Bevy `0.19.1`, Lightyear `0.29.0`, and Avian2D `0.7.0`. Current primary API
documentation was also checked for Bevy fixed schedules/`ApplyDeferred`, Lightyear native
`ActionState`/server input sets, and Avian spatial queries/move-and-slide. Those sources confirm the
local APIs and do not justify a new dependency or network input path.
