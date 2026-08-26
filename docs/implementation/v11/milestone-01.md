# V11 Milestone 01 — Playable server-hosted Practice bots

## Status

`Complete`

V11 M01 planning was explicitly started by the user on 2026-08-25. The user approved production
implementation on 2026-08-25. The first playable production slice was implemented on 2026-08-25;
bounded diagnostics, resumable planning, and automated routed verification are now implemented;
the user accepted the final objective and perimeter-recovery corrections and directed V11 closeout
on 2026-08-26. M01 and V11 are complete.

## Player-visible outcome

The existing **Practice** action starts an ordinary routed match in which every manifest `Bot N`
fighter actively plays. Bots navigate Feature Yard, pressure or evade opponents, maintain imperfect
Pulse range and aim, use Dash for bounded engage/escape/traversal decisions, pursue the selected
Wipeout/Hot Zone/Heist goal, coordinate simple bot roles, respect all concealment and reveal rules,
and use or avoid barrels, chests, pickups, and safes through their existing authoritative behavior.

The target is useful build practice, not optimal or human-indistinguishable play. A player should be
able to read a bot's intent, exploit its reaction delay and aim error, see it recover from ordinary
route failures, and trust that it never knows a currently concealed position that a human observer
would not know.

## Scope decisions

### Specified for M01

- Existing routed Start Practice, Practice Again, and selected-game-type flows.
- Server-hosted controllers on manifest bots only; ordinary authoritative fighter entities remain
  the controlled bodies.
- One code-owned behavior profile and one canonical saved-brawler-native Pulse/Dash recipe.
- Canonical bounded observations, observer-specific visibility, reaction delay, and bounded contact
  memory.
- Deterministic independent entropy streams and bounded diagnostic traces.
- Pure team goal/role assignment and per-bot utility decisions.
- Derived revisioned navigation, deterministic bounded resumable search, route smoothing,
  range/retreat/strafe steering, separation, and stuck recovery.
- Wipeout, Hot Zone, and Heist goals for every currently advertised Feature Yard 1v1/2v2/3v3
  topology.
- Typed reasoning for hostile fighters, Heist safes, oil barrels, treasure chests, and useful
  restoration pickups. Pickup collection and every damage outcome remain existing authority.
- Pulse predictive aim with held bounded error, ammo/cooldown awareness, line-of-fire checks, and
  Dash engage/escape/traversal rules derived from the resolved loadout.
- Shared decoded-input validation and atomic `ActionState<FighterInput>`/local
  `InputFreshness` installation before existing movement/fire.
- Defeat, respawn, restart, map replacement, match completion, invalid-output, budget-exhaustion,
  requeue, reconnect, and shutdown lifecycle behavior.
- Focused, separate-App, routed, capacity, performance, deterministic-trace, and native playtest
  evidence followed by feedback triage and a learn-from-errors review.

### Explicitly not in M01

- External bot clients, bot connections, input packets, admission/check-in exceptions, sidecars,
  supervisor orchestration, or bot-specific IPC.
- Multiplayer fill/backfill, absent-player replacement, matchmaking, parties, ranks, or adaptive
  difficulty.
- Player-facing difficulty or bot setup UI.
- Bot-controlled non-Pulse weapons, active items, Sentry, Self Cloak, Reveal Scan, Concealment
  Field, or arbitrary build selection.
- General projectile avoidance, perfect projectile awareness, advanced formations, or a general
  squad-strategy layer.
- Learned policies, training data, inference runtimes, remote services, LLM decisions, or replay
  corpus production.
- A generic AI framework, behavior tree, GOAP system, navmesh dependency, public bot/navigation
  API, map waypoint authoring, or second gameplay simulation.
- Changes to map-size limits, map recipes, mode rules, routing, wire schemas, combat balance, or
  current content solely to simplify bot behavior.

## Research record

### Local product and architecture sources

Research used the current tree and pinned snapshots before selecting exact seams:

- [bot contract](../../10-bots.md), [product direction](../../00-product-direction.md),
  [gameplay loops](../../05-gameplay-loops.md), [network architecture](../../08-network-architecture.md),
  [concealment](../../17-concealment.md), [player UX](../../13-player-ux.md), and the
  [V10 closeout](../v10/roadmap.md);
- `src/server/practice.rs`: `InertPracticeBotPlugin` installs manifest bots at startup as full
  replicated fighters with resolved `MatchBuildSnapshotV3`, `ActionState<FighterInput>`, and
  `InputFreshness` but no controller;
- `src/server/lobby/mod.rs`: `practice_bot_rows` already creates stable IDs/teams/names and currently
  rotates four legacy preset-derived snapshots; `build_identity` demonstrates the existing saved-
  brawler-native snapshot path to replace that rotation;
- `src/gameplay.rs`: `GameplaySet::{Lifecycle, Input, Simulation, Fire, Finalize}` is a chained
  `FixedUpdate` contract with `ApplyDeferred` between lifecycle and input;
- `src/movement/input.rs`, `src/movement/authority.rs`, and `src/protocol.rs`:
  `decoded_input_is_valid` is the complete post-decode finite/radial/button rule, movement reads
  ordinary `ActionState<FighterInput>`, and freshness is local component state;
- `src/concealment/model.rs`: `observer_can_see` is already pure; `src/concealment/mod.rs` assembles
  the inputs but limits its cache to connection-owning observers, so bots need a pure adapter path
  rather than a fake connection;
- `src/map/catalog.rs` and `src/map/runtime.rs`: `ResolvedMap` owns bounds, static colliders, dynamic
  placements, objectives, and safe anchors; `MapDynamicState` owns generation, revision, and stable
  terminal placement transitions;
- `src/map/objects.rs`, `src/map/pickups.rs`, and `src/matchplay/heist.rs`: current object, pickup,
  and safe state already uses bounded stable identities and server-owned outcomes;
- `src/matchplay/wipeout.rs`, `hot_zone.rs`, `heist.rs`, and lifecycle/server modules: all three
  modes expose authoritative state without requiring a second objective model;
- `src/builds/`, `src/profiles/`, `content/catalogs/builds.ron`, and
  `content/catalogs/weapons.ron`: the required default fighter, Pulse Sidearm, Dash, passives, and
  immutable resolved capability facts already exist; and
- `tests/network/harness.rs`, routed process tests, `tests/performance.rs`, and current `justfile`
  commands provide reusable authority, lifecycle, real-process, and fixed-tick evidence paths.

Pinned reference research:

- `references/bevy/examples/app/plugin.rs`, `ecs/fixed_timestep.rs`, and `ecs/ecs_guide.rs` confirm
  focused plugin composition, fixed-tick ownership, chained system sets, and explicit ordering;
- `references/lightyear/book/src/concepts/bevy_integration/system_order.md` and
  `concepts/advanced_replication/inputs.md` confirm that network input buffering and server
  `ActionState` updates are transport/timeline concerns that a server-hosted controller must not
  fabricate;
- `references/lightyear/examples/network_visibility/src/server.rs` confirms connection-owned
  replication visibility is the wrong abstraction for a connectionless bot observation; and
- `references/avian/crates/avian2d/examples/ray_caster.rs` and `move_and_slide_2d.rs` confirm pinned
  spatial-query filters and Avian's final move-and-slide collision role.

### Current primary sources

The exact pinned APIs were cross-checked against primary versioned documentation:

- [Bevy 0.19 `FixedUpdate`](https://docs.rs/bevy/0.19.1/bevy/app/struct.FixedUpdate.html) identifies
  AI, networking, physics, and game rules as fixed-rate work;
- [Bevy 0.19 `ApplyDeferred`](https://docs.rs/bevy/0.19.1/bevy/ecs/schedule/struct.ApplyDeferred.html)
  documents deferred-buffer visibility and why the existing lifecycle/input boundary must remain
  explicit;
- [Lightyear 0.29 native `ActionState`](https://docs.rs/lightyear/0.29.0/lightyear/input/native/prelude/struct.ActionState.html)
  distinguishes neutral/default input from missing network input;
- [Lightyear 0.29 server input sets](https://docs.rs/lightyear/0.29.0/lightyear/prelude/server/input/enum.InputSystems.html)
  confirm that receive/validation/action-state updates belong to client-originated input and are not
  required for a server-local controller; and
- [Avian2D 0.7 spatial queries](https://docs.rs/avian2d/0.7.0/avian2d/spatial_query/index.html)
  provide ray/shape tests for authoritative geometry checks while the derived pure graph remains
  project-owned.

The local tree answered the feature questions. Current sources confirmed compatibility; no
unpinned API, new crate, or external service is needed.

### Findings and risks

1. Practice bots already have every gameplay component required by movement, combat, abilities,
   mode rules, replication, results, and lifecycle. Adding another bot entity or simulation would
   duplicate authority.
2. `decoded_input_is_valid` is currently `pub(super)` to movement; M01 needs one deliberate
   `pub(crate)` reuse boundary and no broader input API.
3. The concealment decision is pure, but its ECS assembly and cache are connection-centric. The
   bot adapter must share or extract canonical visibility-input construction without teaching the
   policy about `Entity`, `ControlledBy`, or Lightyear visibility.
4. Current resolved map facts are sufficient to derive navigation, but current dimension validation
   is not a stable complexity guarantee. The graph builder must validate its own measured counts.
5. Current Feature Yard is 64×40 and current product topology is at most 3v3, but manifest capacity
   is eight. Bot storage and ordering should use resolved/declared ceilings rather than the number
   five or a 64×40 allocation assumption.
6. Exact deterministic decision replay requires stable sorting, versioned split entropy streams,
   fixed work budgets, explicit floating-point comparison/tie rules, and no async completion order.
7. Server truth creates an accidental-cheating risk. Reaction delay must cover target changes,
   dynamic object changes, aim correction, and tactic changes—not only target acquisition.
8. A human teammate cannot be assigned a bot reservation. Team plans may account for the human's
   permitted current/delayed facts but only allocate bot-owned roles.
9. Navigation rebuilds and dynamic blocker knowledge have different fairness clocks. Match/map
   generation invalidation is immediate; public object state becomes bot knowledge only through the
   delayed observation.
10. The entire maximum-roster planner runs inside a 16.67 ms fixed step. Work limits must be hard
    safety properties, with component timing reported separately so policy quality cannot hide a
    scheduling regression.

### Alternatives considered

- **External headless client first:** rejected because it adds process orchestration, network
  admission, replication-clock, input-buffer, and shutdown work without improving the initial
  Practice loop. It remains a later adapter over the same pure boundary.
- **Directly mutate movement/combat/ability state:** rejected because it creates bot-only authority,
  bypasses validation, and makes bot outcomes incomparable to player outcomes.
- **Reuse the Lightyear input buffer for local bots:** rejected because it fabricates remote ticks,
  packets, sequence/freshness evidence, and connection ownership that do not exist.
- **Let policy query ECS:** rejected because it makes fairness unauditable, leaks hidden state, ties
  behavior to process-local entities, and obstructs deterministic pure tests.
- **Behavior tree, GOAP, or third-party utility-AI crate:** rejected because current intelligence is
  a small set of project-specific goals, scores, commitments, and capability rules; another
  framework would not solve perception, navigation, concealment, or authority.
- **Learned policy:** rejected because there is no representative trace corpus, reproducibility and
  diagnosis are more valuable now, and current gameplay remains subject to change.
- **Recast/navmesh:** rejected because Brawler remains planar and current resolved collision shapes
  can lower directly into the one graph/search contract.
- **Whole-map A* every tactic tick:** rejected because it couples cost to map size, causes avoidable
  fixed-tick spikes, and discards still-valid route commitments.
- **Async pathfinding:** rejected because wall-clock completion and task scheduling would affect
  decisions and complicate authoritative lifecycle cleanup. M01 uses resumable stable-order work
  under tick budgets.
- **Perfect current-state aim and instant replanning:** rejected because it makes server hosting an
  unfair advantage and produces unreadable opponent behavior.

## Technical specification

### Ownership and module boundary

Replace the single-file inert plugin with focused server-only composition:

```text
src/
  bots/
    mod.rs          private composition, system sets, and narrow internal re-exports
    model.rs        owned observations, contacts, goals, plans, state, intent, decisions, traces
    observation.rs  pure canonicalization helpers and bounded collection rules
    policy.rs       pure utility scoring and committed per-bot transition
    team.rs         pure stable bot-role/reservation assignment
    navigation.rs   derived geometry, bounded resumable search, route following and recovery
    capability.rs   resolved Pulse/Dash intent-to-input execution
    profile.rs      validated first profile and safety ceilings
    entropy.rs      pinned split deterministic sampler
  server/
    practice/
      mod.rs        manifest materialization plus focused plugin composition
      controller.rs ECS allowlist, lifecycle, scheduling, input commit, diagnostics
```

The navigation implementation remains cohesive in one focused module. `bots` compiles
only with `server` and is not public from the crate root. Pure policy/search code may use shared
stable gameplay model types and `Vec2`, but not `World`, `Query`, `Commands`, `Entity`, `ControlledBy`,
Lightyear connection types, mutable authority resources, or presentation state.

`server::practice` owns only the adapter and controller lifecycle. Movement, combat, abilities,
map, pickups, matchplay, concealment, routing, and replication retain their existing plugins and
public APIs. `movement::decoded_input_is_valid` becomes `pub(crate)` as the one required cross-
module seam.

### Canonical bot brawler

Replace `practice_bot_rows`' four-preset rotation with a helper that constructs one ordinary
`SavedBrawler` and resolves it through `MatchBuildSnapshotV3::from_brawler`:

| Slot | Existing stable choice |
|---|---|
| Fighter profile | `FighterProfileId(1)` — default |
| Weapon base | `WeaponBaseId(1)` — Pulse Sidearm |
| Ultimate | `UltimateDefinitionId(1)` — Dash |
| Passives | `PassiveDefinitionId(3)` Adrenal Response; `PassiveDefinitionId(4)` Close Quarters |
| Weapon parts | four `None` slots |
| Saved-brawler identity | code-owned nonzero stable ID reserved for the canonical bot recipe |

The stable player ID, team, display name, recipe fingerprint, build revision, and encoded snapshot
remain fields of the current `AllocateBot` row. The controller reads only the resolved match
loadout. It never branches on a preset ID, assumes canonical numeric balance values, queries an
inventory, or reconstructs a recipe in combat.

### Private controller state

Manifest bot materialization adds private server components/resources conceptually equivalent to:

```rust
struct PracticeBotController {
    seed: u64,
    life_generation: u64,
    last_observation_tick: Option<u64>,
    last_decision_tick: Option<u64>,
}

struct BotObservationHistory { /* bounded tick-indexed ring */ }
struct BotState { /* contact, target, tactic, aim, route, stuck, capability commitments */ }
struct BotRouteState { /* revision, request/search/route/progress */ }
struct BotTeamPlanState { /* match/team generation, cadence, roles, reservations */ }
struct BotNavigationRuntime { /* immutable snapshot + stable bounded request/search work */ }
struct BotDiagnostics { /* bounded counters/timings and optional trace ring */ }
```

Only the private controller marker selects bot systems. Display names and high numeric `PlayerId`
values are not behavioral classification. Connected fighters never receive controller state.
Resources and components store stable gameplay IDs across decision boundaries; any adapter-local
`Entity` lookup ends before the pure call.

### Observation model and canonicalization

At most one owned `BotObservation` is captured per active manifest bot and simulation tick. It
contains:

- match ID, tick, phase, mode kind, map generation, navigation revision, completeness, and life
  generation;
- controlled stable fighter ID/team, pose, current/maximum health, active/defeated/protected state,
  immutable resolved Pulse/Dash capabilities, ammo/cooldown/charge/ability phase, and relevant
  active movement restrictions;
- permitted fighters with stable ID/team, delayed pose and velocity derived only from permitted
  samples, public combat/lifecycle facts, and source observation tick;
- permitted supported deployable/projectile facts only if a first-slice behavior consumes them;
  otherwise they are omitted rather than collected speculatively;
- public damageable-object state keyed by stable target/placement identity, Heist safe state keyed
  by objective identity, and restoration pickup state keyed by pickup identity;
- bounded mode-specific facts: Wipeout score/target, Hot Zone bounds/status/progress/occupants, or
  Heist safe identities/health/completion; and
- delayed dynamic-blocker facts plus the shared immutable navigation identity.

Every collection sorts by its stable identity before truncation and policy evaluation. A capacity
overflow marks the observation incomplete, increments a bounded diagnostic, and causes neutral or
still-safe committed behavior; it never leaks an arbitrary insertion-order subset.

Static navigation geometry is shared, not copied into the history ring. History stores the minimum
owned dynamic facts needed by the first behavior. Proposed safety ceilings are:

| Concern | Ceiling |
|---|---:|
| Manifest controllers | `MAX_PARTICIPANTS - 1` |
| Observed fighters | `MAX_PARTICIPANTS` |
| Observed damageable objects | existing `MAX_DAMAGEABLE_MAP_OBJECTS` |
| Observed restoration pickups | existing `MAX_LIVE_RESTORATION_PICKUPS` |
| History ticks | 64 |
| Contacts per bot | `MAX_PARTICIPANTS - 1` |
| Trace records per bot | 256 when trace capture is enabled; counters only otherwise |

The 64-tick history ceiling safely exceeds the proposed 9-tick reaction delay without becoming a
replay system. Projectile/deployable ceilings are added only if the implemented first behavior
proves a consumer.

### Concealment, delay, and contact memory

Extract or reuse one canonical adapter helper that assembles `ObserverVisibilityInput` for an
observer/subject pair from authoritative source components, then call the existing
`observer_can_see` rule. The human connection cache continues to drive Lightyear visibility; the
bot path produces no cache entry or replication command.

At tick `T`, tactic and aim evaluation select the complete observation at
`T - reaction_delay_ticks`. The initial proposed profile is:

| Behavior value | Initial value |
|---|---:|
| Reaction delay | 9 ticks (150 ms) |
| Contact memory | 120 ticks (2 s) |
| Tactic evaluation cadence | 6 ticks (100 ms) |
| Team-plan cadence | 15 ticks (250 ms) |
| Minimum tactic commitment | 18 ticks (300 ms) |
| Aim-error hold | 24 ticks (400 ms) |
| Maximum absolute aim error | 5 degrees |
| Preferred Pulse range | 70% of resolved maximum range |
| Retreat health threshold | 35% of resolved maximum health |
| Stuck window | 30 ticks (500 ms) |
| Minimum replan interval | 12 ticks (200 ms), except invalidation |

These are proposed playtest starting points, not final balance claims. Profile validation allows
only finite/representable ratios and bounded tick values that fit the history, deadline, route, and
work ceilings.

When an enemy becomes impermissible, the policy may retain only stable ID, last permitted delayed
pose, its source tick, and an expiry/confidence state. It cannot retain or update hidden velocity,
current pose, aim, status, or object relationship. Investigation movement may target the stale
position, but attack and Dash engage require a currently permitted delayed target. Team sharing, if
implemented, shares only these same source-stamped contacts and never lengthens their expiry.

### Deterministic entropy and traces

Implement a small pinned integer sampler with explicit `BOT_ENTROPY_ALGORITHM_VERSION` and
`BOT_PROFILE_VERSION`. Derive independent fixed sample slots from:

```text
bot seed + match ID + controller life generation + simulation tick + stream ID + sample slot
```

Streams are reserved for team assignment, target ties, tactic ties, strafe direction, aim error,
and cadence jitter. Construct the complete sample bundle before branch evaluation. Adding or
skipping one behavior cannot perturb another stream.

Default seed derives from stable manifest `PlayerId`; focused tests may override it through a
server-private fixture. A trace record contains only bounded canonical IDs, tick, source
observation tick, profile/algorithm version, effective seed, life generation, navigation revision,
work budgets, chosen role/goal/tactic/target/capability, route result, and one enum reason. It stores
no raw `Entity`, mutable resource, unbounded debug string, or secret current pose beyond the
permitted observation.

### Team planning and utility policy

One pure batch transition runs per team in stable bot ID order. It consumes a bounded
`BotTeamObservation`, previous plan, fixed entropy, and profile, then returns roles/reservations for
bot members only. The human teammate contributes permitted pressure/objective context but receives
no assigned role.

Mode adapters provide common goal candidates:

| Mode | Initial goal candidates |
|---|---|
| Wipeout | survive, pressure visible enemy, support pressure lane, recover useful pickup |
| Hot Zone | contest zone, hold/defend controlled zone, pressure approach lane, recover useful pickup |
| Heist | attack hostile safe, defend friendly safe, pressure lane, intercept visible attacker, recover useful pickup |

Reservations cover a target, lane/approach sector, safe role, zone role, or pickup. They expire and
are recomputed on the slower cadence or a material mode change. Stable total tie-breaking prevents
every bot from collapsing on one equal-cost choice.

The per-bot utility transition scores only the first tactics: `approach`, `hold_range`, `retreat`,
`reposition`, `contest`, `defend`, `attack_objective`, and `recover_pickup`. Inputs include health
ratio, permitted target distance/age, line of travel/fire, resolved capability readiness,
objective pressure, travel estimate, reservation, and commitment state. Validated score curves and
stable tie rules are pure. Minimum commitment prevents dithering; target loss, invalid route,
objective terminal change, defeat, or capability completion may interrupt it.

Barrel and chest behavior remains typed. A bot may target a live hostile-opportunity barrel when
the delayed blast estimate favors its team, avoid a delayed dangerous barrel region, attack a live
chest when recovery utility justifies it, and route to a useful pickup. It never receives a generic
interaction callback. Safes are hostile objective targets; pickups are collected only by ordinary
authoritative overlap.

### Navigation snapshot and search

At selected-map installation, build one immutable `BotNavigationSnapshot` from playable bounds,
resolved player-blocking shapes, fighter radius, and skin width. The snapshot owns:

- map instance/generation and a private monotonic navigation revision;
- stable cell and directed-edge IDs;
- node positions and clearance-valid travel edges;
- deterministic geometry used for direct travel, line of fire, smoothing, and goal clamping; and
- measured counts checked against code-owned safety ceilings.

The current grid recipe may generate clearance-safe cell-center nodes with cardinal/diagonal edges,
but diagonal edges are rejected when either adjacent cardinal passage is blocked. Stable IDs and
edge costs use integer or explicitly quantized values for ordering. The policy sees graph facts,
not `MapDimensions`, cells, meshes, scenes, or renderer state.

Planning order:

1. clamp the tactical goal to legal playable clearance;
2. use direct steering when a clearance shape cast/derived geometry test is unobstructed;
3. otherwise resume stable exact shortest-path work for the route;
4. retain unfinished open/cost/parent state for the next fixed tick;
5. simplify with clearance-valid line-of-travel tests;
6. follow the committed route with arrival, desired-range, retreat, strafe, and bounded local bot
   separation; and
7. replan only on material goal change, invalid revision/overlay, exhausted route, or the bounded
   stuck window.

Proposed initial ceilings:

| Navigation concern | Ceiling |
|---|---:|
| Nodes | 32,768 |
| Directed edges | 262,144 |
| Stored route points per bot | 1,024 |
| Live/queued requests | one per live controller |
| Total node expansions per tick | 512, shared in stable bot/goal order |
| Total expansions per request | 16,384 |
| Retained route-cache entries | 64, keyed by revision and stable endpoints |

Unfinished work resumes on the next tick. Request exhaustion returns an explicit failure and clears
its retained search state. The bot continues a still-valid route, uses a bounded direct/local
fallback, or emits neutral movement. It never performs an unbounded same-tick search, panics, or
waits on an async task.

Map generation or a rebuilt authoritative collision topology replaces the snapshot and invalidates
all search/routes immediately. V10 barrel/chest/pickup and other public dynamic state is a delayed
overlay per bot; terminal change does not grant immediate traversability knowledge. Avian's
authoritative collision can still reject a chosen step, which feeds only bounded progress/stuck
state.

### Navigation specification refinement — 2026-08-26

The reviewed design allowed a sector/portal hierarchy, but implementation evidence did not
demonstrate a need for it. The stable resumable search completes the synthetic 128x96 topology and
the maximum five-controller roster remains below the fixed-tick p95 gate while sharing the hard
512-expansion budget. M01 therefore keeps the smaller flat derived topology and records pending,
completed, exhausted, and expansion diagnostics. This is a deliberate evidence-based scope
refinement, not a deferred correctness gap; a hierarchy should be reconsidered only if a real map
or measured timing failure demonstrates the need.

### Pulse and Dash capability execution

The capability executor consumes `BotIntent`, committed route progress, delayed observation,
current self capability state, and the immutable resolved match loadout. It returns a complete
`FighterInput` and next private state.

Pulse rules:

- derive projectile speed/range, muzzle offset, fire cooldown, ammo/refill, damage, and delivery
  semantics from the resolved Pulse recipe;
- use delayed permitted target position and permitted consecutive samples for bounded intercept;
- sample aim error only when its hold interval begins, then retain it for the interval;
- require current delayed line of fire and supported target eligibility before setting
  `PRIMARY_FIRE`;
- use objective/object stable identity for safe/barrel/chest shots; and
- emit no target-dependent fire when there is no permitted target, even if movement toward a valid
  goal continues.

Dash rules:

- set `ULTIMATE` only when resolved Dash is ready and charged;
- allow bounded engage, escape, and route-traversal cases with explicit health, distance,
  destination-clearance, and post-dash danger checks;
- never Dash into a known blocked/out-of-bounds destination or use current hidden target facts; and
- let the existing Dash ability system validate, move, interrupt, and consume charge.

`ACTIVE_ITEM` and `INTERACT` remain clear in M01. Movement axes are normalized/clamped before
quantization; aim distance is omitted for Pulse unless an existing rule requires it. The final
quantized value must pass `decoded_input_is_valid`; rejection produces neutral input and a bounded
diagnostic.

### ECS schedule and atomic input commit

Add explicit private bot sets under the existing schedule contract, with final names chosen during
implementation after a schedule trace confirms all owning systems:

```text
FixedUpdate
  GameplaySet::Lifecycle
  ApplyDeferred
  BotSet::ReconcileLifecycle
  BotSet::CaptureObservation
  GameplaySet::Input
    BotSet::PlanTeams
    BotSet::AdvanceRoutes
    BotSet::DecideAndCommit
  GameplaySet::Simulation
  GameplaySet::Fire
```

If Bevy ambiguity rules do not guarantee the nested order, chain the bot sets explicitly. Capture
reads lifecycle state only after deferred fighter markers are visible. Each controller records its
last captured and decided tick; duplicate execution for one `SimulationTick` fails closed without
consuming entropy or advancing commitments.

Decision commit validates first, then writes `ActionState(FighterInput)` and
`InputFreshness { last_fresh_tick: Some(tick) }` in the same system. No `Commands`-deferred split is
allowed between those writes. Before every early return for inactive/incomplete/invalid state, the
system installs neutral input and current controller-produced freshness only when the bot is an
active controller for that tick; defeated/respawning bots remain neutral and do not pretend active
decision progress.

### Lifecycle, reset, and failure behavior

Use `(match_id, controller_life_generation)` as the policy-life key. The controller observes
authoritative transitions among `ActiveCombatant`, `Defeated`, and `RespawnState` after lifecycle
commands apply:

- entering defeat/wait immediately neutralizes input and clears target-dependent commitments;
- the single transition back to active increments life generation and replaces history, contacts,
  bot state, route/search state, and entropy context;
- restart/match change clears all controller/team state and rebuilds or rekeys navigation;
- navigation revision alone clears affected route/search state without incrementing fighter life;
- match completion keeps inputs neutral while existing results/teardown proceed; and
- missing snapshot, invalid profile, observation overflow, graph failure, search exhaustion, invalid
  decision, or stable-ID lookup failure records a bounded reason and yields neutral/local fallback.

No bot error may panic, spin, stall shutdown, retain stale match entities, or change a match outcome
outside existing input-driven rules. Canonical embedded content and every advertised Feature Yard
game type must pass startup validation, so fail-closed runtime paths are resilience evidence rather
than an accepted normal player experience.

### Diagnostics, telemetry, and tuning disposition

Add bounded server-private counters and sampled timings for observation capture, visibility pairs,
team-plan runs, tactic runs, route submissions, expansions, route success/failure, stuck/replan,
invalid decisions, neutral fallbacks, and total bot work. Optional trace capture is test/operator
diagnostics and does not register a protocol message or expose secret state to clients.

Match summaries may add aggregate bot participation only if a concrete closeout consumer requires
it; no per-tick bot facts enter ordinary player telemetry by default. Existing combat/mode/object
telemetry already records the outcomes caused by bot input.

M01 intentionally keeps `BotProfile` code-owned. Before closeout, update
`docs/15-balance-lab.md` to record why the first behavior profile is not yet a persisted operator
surface. If implementation playtesting demonstrates that rapid behavior iteration is impractical,
return to specification review before extending the Balance Lab snapshot/UI.

### Network and process behavior

M01 adds no `ProtocolPlugin` registration. Controller marker/state, navigation, observations,
contacts, plans, entropy, and traces never replicate. Ordinary fighter inputs are not sent as bot
messages; clients observe only resulting replicated gameplay.

Routed Practice allocation, match manifest encoding, worker process creation, public UDP routing,
lobby/match handoff, participant check-in, reconnect, result return, and worker shutdown remain
unchanged. Bot materialization still occurs inside the selected match worker from validated
manifest rows.

Separate-App/routed tests must prove that bot-caused movement, projectiles, damage, abilities,
object transitions, pickups, objectives, scores, results, and lifecycle converge through existing
state. They must not claim that server-hosted bots exercise remote input validation, impairment, or
connection ownership.

## Implementation checklist

- [x] Replace rotating practice bot snapshots with the validated canonical saved-brawler recipe.
- [x] Convert `server/practice.rs` into focused composition without changing manifest or spawn
  ownership.
- [x] Add private bot model/profile/entropy boundaries and validation.
- [x] Expose the complete decoded-input validity helper as narrow `pub(crate)` API.
- [x] Reuse canonical observer decisions and build bounded delayed observations.
- [x] Implement controller life generation, history, contacts, neutral failure behavior, bounded
  counters/timings, and opt-in traces.
- [x] Implement pure stable controller-roster team planning and Wipeout/Hot Zone/Heist goal
  adapters without assigning the human participant.
- [x] Complete bounded resumable planning and diagnostics: derived geometry, stable A*, delayed
  dynamic blockers, route following, separation, stuck recovery, and search-state counters are
  implemented. The unneeded sector/portal hierarchy was removed by the evidence-based refinement
  above.
- [x] Implement pure utility policy and committed state.
- [x] Implement resolved Pulse/Dash capability-to-input rules.
- [x] Install explicit fixed-tick ordering and atomic input/freshness commit.
- [x] Add focused, schedule, lifecycle, network, routed, determinism, capacity, and performance
  tests.
- [x] Run canonical role-specific build/test/lint commands and all affected routed E2E.
- [ ] Complete native controller and keyboard/mouse playtest handoff across all three modes.
- [ ] Triage feedback, rerun affected verification, reconcile durable docs/commands, and record the
  learn-from-errors review.

### Implementation evidence — 2026-08-25

- `just check` passed for routing, client, server, network-test, and Balance Lab configurations.
- Server all-target Clippy passed with `-D warnings` after the bot integration.
- Focused bot tests pass for profile validation, split deterministic entropy, exact reaction-delay
  selection, stable team roles, bounded trace eviction, stable routing, diagonal corner-cut
  prevention, synthetic maximum dimensions, and search exhaustion.
- The production match-worker composition test proves manifest bots remain connectionless ordinary
  fighters, receive private controllers, and emit non-neutral ordinary input after the reaction
  window. The lobby test proves a 3v3 Practice allocation uses five copies of the canonical recipe.
- After stable team planning and bounded diagnostics were added, all 306 server library tests and
  server all-target Clippy with `-D warnings` passed. The focused bot suite now contains six passing
  tests.
- `just test` passed the routing, client, and server suites. The user requested `cargo clean` for
  disk pressure while the command was rebuilding the Balance Lab suite, so the remaining Balance
  Lab, network, and performance portions need a fresh later run.
- A selected Practice network regression rebuild was initially stopped when the freshly cleaned
  cache began compiling the full client rendering graph; it was resumed and passed on 2026-08-26.
- `cargo clean` completed after the interrupted full gate; the workspace volume then reported 186
  GiB available.

### Implementation evidence — 2026-08-26

- The profile now enforces one aggregate 512-expansion navigation budget per fixed tick and divides
  it deterministically across the active controller roster. Stable-ID cadence staggering prevents
  routine replans from aligning; direct clear travel consumes no A* work. Goals clamp into playable
  bounds and blocked routes compress collinear grid steps before following.
- Nine focused bot tests pass, adding exact contact-memory expiry, complete private life reset,
  maximum-roster budget arithmetic, deterministic roles under input permutation, synthetic maximum
  dimensions, bounded search exhaustion, and a 200-sample five-controller pure-decision p95 gate
  against the 16.67 ms fixed tick.
- The complete 306-test server library suite and server all-target Clippy with `-D warnings` pass.
- The separate-App Practice regression
  `practice_request_bypasses_queue_and_starts_one_human_three_v_three_reservation` passes with one
  human and the five-bot 3v3 reservation.
- Rebuilding the network-test graph after the requested clean grew `target/` to 21 GiB while the
  workspace retained 172 GiB free. After the selected regression and final focused checks passed,
  `cargo clean` removed 25.2 GiB and restored 193 GiB free.
- Subsequent user direction is to retain Cargo build artifacts and stop cleaning. No further
  `cargo clean` is part of M01 implementation or verification unless the user explicitly changes
  that direction.
- Navigation search now persists its deterministic open set, costs, parents, delayed dynamic
  blocker snapshot, and expansion count in private controller state. Each fixed tick advances it
  only by the controller's share of the 512-expansion roster budget; pending work resumes on the
  next tick, terminal exhaustion falls back to neutral movement with a bounded retry cadence, and life
  or context reset discards the search.
- Ten focused bot tests pass, including a search that is deliberately split across multiple tiny
  expansion budgets and converges without restarting. The complete server library suite now has
  310 passing tests, and server all-target Clippy passes with `-D warnings`.
- The production match-worker composition test now builds Feature Yard Wipeout, Hot Zone, and
  Heist workers from mode-valid manifests, advances the real fixed schedules beyond the reaction
  delay, and proves each mode's connectionless manifest fighters receive controllers and produce
  ordinary non-neutral input.
- Navigation diagnostics now report search starts, pending/completed/exhausted outcomes, and exact
  expansions without exposing private controller state or creating another gameplay path.
- Post-change `just lint` passes formatting, the Balance Lab web build, routing/client/server/
  Balance Lab Clippy with `-D warnings`, server feature isolation, the sole-world-renderer check,
  and canonical-map cleanup.
- Post-change `just test` passes 83 routing library tests plus routed process suites, 389 client
  tests, 310 server tests, 320 Balance Lab tests, the combined Balance Lab/network regression, all
  88 network tests, and all 12 performance gates. The combined existing worst-case fixed tick
  reports 5.203125 ms p95 against the 16.67 ms gate.
- The production 2/4/6-client routed E2E matrix passes exact 1v1/2v2/3v3 allocation, Active entry,
  and bounded worker shutdown.
- A new `just practice-e2e <game-type>` production-process seam drives the real Dashboard Practice
  transaction with one human. Its complete nine-game matrix passes Wipeout, Hot Zone, and Heist
  1v1/2v2/3v3, proving every advertised exact type reaches Active with the manifest bot roster and
  shuts down cleanly.
- A native release `wipeout-3v3` Practice run through the retained render-evidence path passes its
  locked report over 1,800 gameplay samples: six fighters, projectile/effect activity, 17.510 ms
  frame p95, no frames over 50 ms, and clean routed worker shutdown. Evidence is retained at
  `target/v11-m01-practice-render-evidence-30s.txt`.
- User feedback corrections add weapon-derived stand-off goals for safes/chests/barrels, a full-cell
  playable-boundary inset, and a deterministic short clearance-valid escape after the stuck
  window. Objective tactics now precede generic visible-enemy pressure: Hot Zone controllers retain
  stable in-zone hold points while fighting, Heist attackers retain hostile-safe aim/approach, and
  defenders anchor on the friendly safe's exposed side.
- A second corner report with screenshot evidence isolated a separate low-health loop: `Retreat`
  continually chose a far away-from-enemy goal, goal clamping placed it at a corner, and the next
  tactic decision undid the short stuck escape. Retreat now stops increasing separation at the
  weapon-derived preferred range. Entering the outer two-cell perimeter also latches a routed
  recovery toward a five-cell inset until that safer interior is reached.
- Fifteen focused bot tests pass, including the objective, stand-off, obstruction-stall, and
  repeated low-health perimeter regressions.
  The production worker regression proves delayed Hot Zone and Heist controllers emit input toward
  their selected zone/hostile safe in the real fixed schedule. All 316 server tests and server
  all-target Clippy with `-D warnings` pass.
- Affected routed Practice reruns pass after the second correction for Hot Zone, Heist, and Wipeout
  3v3 with one human, exact manifest bot fill, and Active entry.

## Verification plan

### Pure policy and invariants

- canonical sorting/truncation is invariant to ECS insertion and collection order;
- observer truth tables cover self/ally/enemy, alive/defeated, terrain, Self Cloak, Concealment
  Field, proximity, attack/damage locks, Reveal Scan, exact boundaries, and non-finite failure;
- delayed observations prove target movement, aim correction, tactic changes, dynamic-object
  state, and contact expiry use the source tick and never current hidden state;
- entropy streams are reproducible and independent; aim error remains held for its interval;
- team roles/reservations are stable across bot ordering and never assign the human participant;
- Wipeout, Hot Zone, and Heist candidate/scoring rules select readable survival, contest/defend,
  safe-attack/defend, lane, and recovery behavior;
- Pulse intercept/range/fire and Dash engage/escape/traversal tests use resolved capabilities;
- emitted axes, aim, aim distance, and buttons are finite, quantized, allowed, and accepted by the
  shared validity rule;
- no-target and hidden-target cases clear attack/ultimate while legal committed movement may
  continue; and
- profile/observation/navigation capacity failures return explicit neutral/local fallbacks.

### Navigation

- direct travel, blocked direct travel, multiple-turn route, stable equal-cost selection, no
  diagonal corner cutting, goal clamping, clearance, smoothing, range/retreat/strafe, arrival,
  separation, and stuck recovery;
- stable results under controller and candidate insertion permutation;
- incremental shared-budget work resumes in stable bot/goal order and cannot exceed per-tick or
  per-request ceilings;
- route/search/cache state is bounded and cleared on exhaustion, generation, revision, life, and
  teardown;
- delayed live/destroyed barrel/chest/cover overlays do not reveal traversability early;
- synthetic topology larger than current built-ins succeeds without encoding current grid maxima;
  a maximum-ceiling topology fails or completes within the declared fixed-tick budget; and
- final movement collisions remain ordinary Avian outcomes rather than planner authority.

### Focused ECS and schedule

- only manifest bot fighters receive controller state;
- lifecycle/deferred application precedes observation; one observation and one decision/commit
  occur per tick; simulation and fire consume the committed input afterward;
- incomplete history, countdown, completed match, defeat, respawn, invalid output, and duplicate
  execution install or preserve neutral state correctly;
- `ActionState` and `InputFreshness` commit atomically and post-selection activation barriers work;
- authoritative movement, firing, Dash, damage, charge, pickups, score, objective, defeat, respawn,
  and reset occur only in existing owning systems; and
- repeated lives/restarts leave bounded history, contacts, plan, search, route, trace, and entities.

### Network, routed, and lifecycle

- separate server/client Apps converge on bot movement, firing, Dash, health, defeat, respawn,
  object/pickup/objective state, score, and result without any bot protocol type;
- Start Practice creates one human plus the exact manifest bot count for every Feature Yard
  1v1/2v2/3v3 game type;
- representative routed Wipeout, Hot Zone, and Heist matches complete from bot input and return
  ordinary results;
- reconnect, restart, Practice Again, fresh-lobby requeue, concurrent heterogeneous workers, and
  shutdown retain no stale controller or navigation state; and
- normal human input validation/impairment tests remain unchanged and no test labels local bot
  freshness as a received packet.

### Capacity and performance

- maximum 3v3 Practice runs one human and five controllers with current maximum relevant fighter,
  object, pickup, safe, and dynamic-state facts;
- report p50/p95/max observation, team-plan, tactic, navigation, total bot, and whole fixed-tick
  timings separately;
- the combined maximum-roster fixed tick remains below the existing 16.67 ms p95 gate on the
  repository's benchmark host, with no individual route request causing an unbounded spike;
- repeated restart/requeue and maximum trace-disabled operation remain bounded in memory/entity
  high-water marks; and
- trace-enabled diagnostics remain bounded and are excluded from normal production timing claims.

### Native playtest matrix

Run the canonical routed server/client path with both keyboard/mouse and controller:

1. Wipeout 1v1: read reaction delay, imperfect aim, preferred range, retreat, Dash, concealment
   fairness, barrel/chest/pickup behavior, defeat, and respawn.
2. Wipeout 3v3: observe target/role spread, lane choice, separation, team pressure, and route
   recovery without all bots collapsing into one point.
3. Hot Zone 1v1 and 3v3: observe contest, hold/defend, approach pressure, pickup diversion, score
   progress, and objective readability.
4. Heist 1v1 and 3v3: observe attack/defend/lane roles, hostile-safe targeting, friendly-safe
   protection, timeout/threshold outcomes, and no chest/safe confusion.
5. Conceal/reveal scenarios: Self Cloak, terrain, field, proximity, attack/damage reveal, and Reveal
   Scan must produce behavior consistent with what the human can infer.
6. Primitive fallback and reduced effects: intent, targets, Dash, objectives, pickups, and results
   remain readable without presentation becoming authority.
7. Practice Again/requeue: no stale target, route, contact, aim, role, or life state appears in the
   next match.

Requested observations:

- Is Practice useful for testing a brawler's range, movement, and survival tradeoffs?
- Do bots look fallible without looking inert or randomly jittery?
- Can the player understand why a bot approached, retreated, contested, defended, or changed lane?
- Do routes look intentional, and do failures recover without oscillation or long stalls?
- Does any bot appear to track a concealed player unfairly?
- Do role coordination and object/pickup choices help rather than distract from combat?
- Does the match remain fun and readable in all three modes/topologies?

## User playtest handoff

Automated verification is complete. For the acceptance playtest, start the routed product in one
terminal and the native client in another:

```sh
just server
```

```sh
just client
```

Create or select a saved brawler, select a game type on the Dashboard, and choose **Practice**. Use
WASD plus mouse aim/left-click/E for keyboard and mouse; use the left stick, right stick, right
trigger, and right bumper for controller. Cover at least `wipeout-1v1`, `wipeout-3v3`,
`hot-zone-1v1`, `hot-zone-3v3`, `heist-1v1`, and `heist-3v3`, then use Practice Again once.

The intentional M01 limitations are one code-owned bot profile, one canonical Pulse/Dash recipe,
no player-facing difficulty or bot setup, no arbitrary bot loadouts, and no general projectile
avoidance. Please report the requested observations above, especially usefulness, readable intent,
route stalls/oscillation, concealment fairness, and whether any accepted correction is required.

## Feedback review

Feedback received 2026-08-26:

- **Bots favor map corners and appear stuck — implemented and accepted.** The first correction
  addressed blocked object centers, boundary clamping, and
  true stationary stalls, but the follow-up playtest still showed a 16-health bot holding a corner.
  The screenshot exposed an independent policy loop: low-health retreat continually selected a far
  outward goal, so each decision reselected the corner after a short escape. Retreat now increases
  separation only to preferred weapon range, and perimeter entry latches a routed inward recovery
  until the bot reaches a five-cell release inset. A repeated-decision regression reproduces the
  low-health bottom-right case and proves that recovery cannot be immediately reversed.
- **Hot Zone and Heist bots appear indifferent to objectives — implemented and accepted.** Generic
  visible-enemy combat returned before objective intent was evaluated.
  Objective tactics now have movement priority while still allowing opportunistic aim/fire. Hot
  Zone objective bots select stable hold points inside the real zone radius; Heist attackers move
  to and aim at the hostile safe from stand-off; defenders anchor between the friendly and hostile
  safes.

Focused, full-server, Clippy, worker-schedule, and affected routed reruns pass. On 2026-08-26 the
user confirmed the final behavior was good enough for the accepted first profile and directed the
version to close. No feedback item is deferred, rejected, or awaiting evidence.

## Acceptance and closeout

The user accepted M01 and directed V11 closeout on 2026-08-26 after confirming the second corner
correction was good enough for the intentionally bounded first bot profile. This acceptance is not
a claim of optimal AI or release balance: additional builds, difficulty choices, projectile
avoidance, advanced tactics, external hosting, and learned policies retain their recorded deferred
or trigger-bound dispositions. All M01 exit criteria are satisfied; M01 and V11 are complete.

## Learn-from-errors review

Pre-playtest review completed on 2026-08-26:

- The first navigation pass spent a per-tick budget on a one-shot search and discarded unfinished
  work. The cause was treating the work ceiling only as a rejection bound. Search frontier, cost,
  parent, blocker snapshot, and expansion state now persist across ticks; future bounded algorithms
  must test progress under deliberately tiny budgets, not only terminal exhaustion.
- The reviewed design named a sector/portal hierarchy before a real topology demonstrated that
  boundary. Synthetic 128x96 and five-controller p95 evidence showed the flat stable resumable
  search is sufficient, so the hierarchy was removed from M01. Future planning layers require a
  measured failure or second real use before implementation.
- The initial production-worker test covered only Wipeout, allowing mode adapters to remain less
  evidenced than the product claim. It now constructs valid Wipeout, Hot Zone, and Heist workers
  through one production helper and advances each beyond reaction delay.
- Existing E2E automation exercised multiplayer Play but could not issue the Dashboard Practice
  transaction. A narrowly scoped `--product-practice-smoke` path and `just practice-e2e` command now
  cover that exact transaction without introducing a bot protocol or alternative authority path.
- The first native Practice evidence attempt used a 10-second measurement even though the locked
  report requires the canonical sample floor, and the wrapper printed a success line before
  validating the report. The report correctly failed `sample_count`; the wrapper now validates
  before reporting success, and the retained release build reran the canonical 30-second window to
  a passing report. Evidence scripts must preserve their locked duration/sample contract when a
  new scenario reuses them.
- Repeated cleaning during a low-disk investigation caused costly full role-graph rebuilds. The
  user later directed that artifacts be retained; no subsequent `cargo clean` was run. Cleanup is
  not a routine verification step and must follow the user's current storage direction.
- Objective roles were assigned correctly, but `choose_intent` returned generic combat as soon as
  any enemy was visible, making the role semantically inert in normal play. Pure tests originally
  checked role assignment rather than the resulting intent under competing facts. Each product
  priority now has a regression with the competing condition present (visible enemy plus objective).
- Stuck handling originally treated repeated replanning as recovery and object goals used target
  centers despite those centers being authoritative colliders. Future navigation tests must assert
  an executable stand-off/escape output, not merely that a route was recomputed.
- The first corner diagnosis covered only lack of movement. The follow-up screenshot showed that a
  bot can be policy-stable and still look stuck because its selected tactic intentionally holds the
  perimeter. Future navigation feedback must be replayed with health, tactic, goal, and position
  together; recovery tests must span repeated decisions so a higher-level policy cannot immediately
  undo a locally correct escape.

These lessons reinforce existing repository rules, so no new project or Codex skill is warranted.
The final playtest-specific policy-loop learning is included above.

## Exit criteria

- The user has validated this specification before production implementation begins.
- Every implementation checklist item is complete or explicitly re-reviewed with the user.
- Start Practice produces active, useful, readable Pulse/Dash bots in every advertised Feature Yard
  Wipeout/Hot Zone/Heist 1v1/2v2/3v3 game type.
- Bots perceive only bounded permitted delayed facts, and concealment/contact tests prove no current
  hidden spatial leak.
- Policy, navigation, entropy, input, lifecycle, and failure behavior are deterministic, bounded,
  and independent of ECS insertion order and wall-clock scheduling.
- Bot actions affect gameplay only through validated `FighterInput` plus local freshness, with all
  outcomes owned by existing authoritative systems.
- Route derivation/search/following passes direct, obstacle, dynamic, synthetic-large-topology,
  exhaustion, revision, stuck, and fixed-tick gates without encoding current dimension maxima.
- Focused, full, role-specific, separate-App, routed, lifecycle, capacity, and performance commands
  pass using canonical repository entry points.
- Native controller/keyboard, all-mode/topology, concealment, object/pickup, primitive/reduced, and
  Practice Again observations are completed and accepted.
- Feedback dispositions, affected reruns, durable documentation reconciliation, and the learn-from-
  errors review are complete.
- The roadmap and milestone are marked `Complete` only after explicit user acceptance.

All exit criteria were satisfied and accepted on 2026-08-26.
