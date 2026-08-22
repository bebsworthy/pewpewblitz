# Milestone 07 — Wipeout match loop

## Tracking

| Field | Value |
|---|---|
| Version | v1 — gameplay MVP |
| Roadmap | [roadmap.md](./roadmap.md) |
| Status | Complete |
| Research | Complete; post-M06 source, architecture, pinned-reference, and primary-source review performed on 2026-08-15 |
| Specification validation | Approved by the user on 2026-08-15 |
| Implementation | Complete; initial implementation plus accepted feedback remediation implemented |
| Verification | Automated, deterministic-network, role-isolation, performance, and local four-process gates are green; unperformed supervised controller/audio/full-aspect/normal-duration observations are explicitly deferred to M11 |
| User validation/playtest | Closeout approved by the user on 2026-08-15; this approval does not claim the deferred supervised observations were performed |

Update this table and the roadmap together whenever the milestone changes phase. Milestone 06 is the
accepted starting baseline: its automated, process-network, performance, visual, controller, and
audio checks are green, and its user playtest was approved on 2026-08-15.

## Outcome

One dedicated server runs a complete, repeatable Wipeout match for two teams with one or two human
participants per team. The server owns team assignment, ready state, countdown, active time,
defeat attribution, score, respawn, spawn protection, completion, result, and restart. Clients send
only weapon selection, fixed-tick fighter intent, and idempotent ready/restart requests.

The match uses the Milestone 06 arena after resolving it against a concrete Wipeout layout schema.
It starts, plays, completes, presents identical results to every client, and restarts with a new
stable `MatchId` without restarting the server, clients, or map. Match-scoped telemetry provides the
first comparable weapon/arena evidence for the combat vertical-slice gate review.

## Decisions requiring specification validation

This specification recommends these choices:

1. Represent authoritative match lifecycle as fixed-tick ECS data on a replicated match-root entity,
   not as a Bevy `States` value mirrored independently in server and client worlds. Bevy states remain
   appropriate for local app/presentation flow, but the networked match phase is durable gameplay
   state that must recover through normal replication.
2. Support two teams, one or two human participants per team, with automatic least-populated-team
   assignment and a hard capacity of four. A match may run 1v1 or 2v2; the automated gate uses four
   clients to prove 2v2 support.
3. Use a 3-second countdown, 180-second active timer, target score 10, 3-second respawn delay,
   1.5-second spawn protection, and a 1-second completed-state input lock. These are validated
   server-owned `WipeoutRules`, not map-authored behavior.
4. Resolve simultaneous scoring as one fixed-tick batch. If both teams reach the threshold with an
   equal score in the same tick, the match is a draw. At timer expiry, the higher score wins and an
   equal score is a draw. M07 does not add unbounded overtime.
5. Award one point only for a hostile player-authored defeat. Self-defeats, friendly outcomes, and
   environmental defeats award no point. Every outcome is still recorded in match telemetry.
6. Replace the sandbox combat reset with mode-scheduled respawn. Combat emits mode-neutral damage
   and defeat facts; the match lifecycle schedules and performs resets. Fighter and weapon systems
   contain no Wipeout score or victory branches.
7. Choose respawn points deterministically from the resolved team spawn catalog: prefer clear
   candidates, maximize distance from the nearest living hostile, and break ties by stable spawn
   point ID. If every point is occupied, choose the best deterministic fallback and rely on spawn
   protection rather than blocking the match indefinitely.
8. Spawn protection blocks hostile damage/effects, is visible to clients, expires after 90 ticks,
   and ends early when the protected fighter accepts an attack. It does not block movement.
9. Accept new match participants only in `Waiting`. A disconnect in `Countdown` returns the match to
   `Waiting`; an active disconnect removes that participant and causes a forfeit only when its team
   becomes empty while the opposing team remains represented. There is no in-match backfill or
   session resumption in M07.
10. Preserve connected fighters, stable player/network identity, assigned team, and selected weapon
    across restart, but reset every match-scoped runtime value. A restart creates a new monotonic
    `MatchId`; the map instance remains unchanged.
11. Require every connected participant to mark ready before countdown and every remaining
    participant to mark restart-ready after completion. `A`/Space/Enter drives both flows after
    weapon selection, through a reliable idempotent match-command transaction scoped to the current
    match ID.
12. Do not add combat bots in the approved scope. Four headless clients prove 2v2, while the visual
    handoff may use 1v1. If implementation evidence shows a deterministic process match cannot be
    completed without bots, adding them requires specification review rather than an implicit scope
    expansion.

Changing authority, phase semantics, attribution, disconnect/reconnect policy, wire shapes, timing,
or the bot decision returns this milestone to specification review. Numeric balance tuning that
keeps these contracts intact may be recorded as an implementation/playtest adjustment.

## Source requirements

- [Product direction](../../00-product-direction.md): short readable matches, combat-first evidence,
  reusable content primitives, and network-first authority.
- [Fighter model](../../02-fighter-model.md): definition/build/resolved/runtime separation and the
  existing fighter runtime categories that match cleanup must preserve.
- [Maps and game modes](../../04-maps-and-game-modes.md): Wipeout rules, team-spawn requirements,
  map/mode separation, and developer-owned executable mode plugins.
- [Gameplay MVP](./gameplay-mvp.md): 2v2-capable Wipeout, controller and keyboard parity,
  two-to-four-minute match target, telemetry, server authority, and combat gate criteria.
- [Network architecture](../../08-network-architecture.md): authoritative teams, respawns, scores,
  timers, victory, restart, disconnect handling, durable replicated state, and explicit recovery.
- [Version 1 roadmap](./roadmap.md): Milestone 07 scope, automated verification, exit criteria, and
  the combat vertical-slice review before Milestone 08.
- [Milestone 06](./milestone-06.md): resolved arena/map lifecycle, client presentation/HUD/audio,
  startup ordering, protocol/content fingerprints, impairment evidence, and closeout lessons.

## Scope boundaries

### In scope

- a cohesive shared match model, reusable authoritative fighter lifecycle, Wipeout rules plugin, and
  client match presentation without another crate or service layer;
- concrete Wipeout map layout requirements for exactly team slots 0 and 1, one spawn area and three
  to eight safe spawn points per team, and no required objective anchor;
- migration of the built-in arena from sandbox mode to a stable Wipeout mode definition, including
  recipe revision and content/protocol fingerprint version changes;
- stable match identity; waiting, countdown, active, and completed phases; validated phase
  transitions and tick deadlines;
- automatic bounded team assignment, per-participant ready/restart-ready state, roster display, and
  clear join rejection outside `Waiting`;
- mode-neutral authoritative combat damage/defeat facts with stable attribution, including hostile,
  self, friendly-invalid, environmental, and same-tick simultaneous outcomes;
- team score, target score, active timer, timeout winner/draw, threshold winner/draw, forfeit, and
  immutable completed result;
- deterministic server-owned initial spawn and respawn selection, respawn deadlines, visible spawn
  protection, early protection break on accepted attack, and safe fallback policy;
- fixed-tick match gating so fighters cannot move, fire, take match damage, or score outside the
  active phase;
- exact match restart cleanup for fighter transient state, projectiles, pending combat work, effects,
  defeat/respawn/protection state, score, clocks, input epochs, and match telemetry accumulators;
- persistent score/time HUD, held scoreboard, waiting/countdown/results overlays, controller-first
  ready/restart flow, respawn/protection feedback, and bounded match audio cues;
- match telemetry and bounded summaries for duration, time to first hostile damage, fight duration,
  hit rate, damage by distance band, defeats/deaths by preset, respawn-to-defeat time, moving versus
  stationary time, final score, score margin, disconnects, forfeits, and draws;
- pure, small-App, deterministic network, real-process UDP, performance, visual, controller, and
  cleanup verification, including a four-client match and repeated matches in one process set.

### Out of scope

- matchmaking, parties, accounts, named profiles, persistent lobbies, server browsing, host
  migration, authentication, or internet deployment;
- client-selected teams, team switching, parties kept together, spectators, active-match backfill,
  reconnect session resumption, or migration of a departed player's fighter to a new connection;
- AI navigation, combat bots, bot difficulty, bot matchmaking, or fixed dummies as participants;
- overtime, best-of series, rounds, rematches with map/build voting, surrender votes, pause voting,
  kill assists, last-damage environmental credit, streaks, medals, MVP selection, or ranked rules;
- build editing, ultimates, passives, active items, build budgets, or named brawler presets from M08;
- Hot Zone objectives from M09 or destructible-terrain behavior/recovery from M10;
- prediction, lag compensation, replay/spectator feeds, persistence, analytics upload, dashboards, or
  production anti-cheat;
- production menu art, localization, accessibility settings, announcer voice, music, or elaborate
  results animation.

## Research questions and conclusions

### Why authoritative ECS data instead of mirrored Bevy states?

Bevy 0.19 describes `States` as app-wide control flow. A queued `NextState` is normally applied in
the `StateTransition` schedule after `PreUpdate`, whereas Bevy explicitly identifies `FixedUpdate`
as the home for game rules and networking. Brawler needs score, timer, simultaneous outcomes, and
restart to resolve at the authoritative 60 Hz tick and to arrive for packet-loss recovery as durable
replicated data.

Use one server-owned `MatchState` component on one replicated `MatchRoot`. Fixed systems mutate it
directly through explicit sets. Clients never run an independent match state machine; they render the
latest replicated phase. Client-local Bevy states may later organize menus, but are not gameplay
truth. This avoids a second clock, `NextState` latency across fixed ticks, and manual synchronization
of `OnEnter`/`OnExit` behavior between processes.

### Durable state versus discrete messages

Lightyear's pinned material distinguishes durable replicated components from messages and sends
entity/component actions reliably and in order. The current project already uses ordinary replicated
components for health, weapon state, defeat, map identity, and map snapshot, and uses ordered combat
messages only for transient presentation facts.

Therefore `MatchState`, participant ready state, respawn deadline, and spawn protection are
components. A late/current client can reconstruct the scoreboard and phase without replaying history.
The only new client-to-server message is an idempotent match command because ready/restart is a
discrete transaction. Score/result changes do not need an additional event stream; presentation can
observe changed replicated state.

### Mode boundary and map compatibility

The M06 catalog deliberately identifies its mode as `sandbox-base` and requires one practice-dummy
anchor. Reinterpreting that stable ID as Wipeout would violate stable-ID semantics. M07 adds a new
Wipeout mode definition, changes the built-in recipe to that ID, removes the dummy anchor, increments
the recipe revision, and resolves with `MapLayoutRequirements::wipeout()`.

Wipeout requires team slots 0 and 1, one non-overlapping spawn area per team, three to eight safe and
reachable points per team, and no objective anchor. The map recipe supplies layout only; target score,
timer, respawn, protection, scoring, and victory remain code-owned `WipeoutRules`.

### Participant count and bots

The server already supports four clients and the shared Crossbeam harness can create four independent
Apps. The existing headless input path can be extended to target hostile fighters for a deterministic
four-client match. That proves the 2v2 network and authority contract without introducing server AI
intent, bot identity, navigation, or bot-specific selection.

The user-facing visual check can remain a manageable two-window 1v1 because the rules support one or
two participants per team. Fixed practice dummies are removed from production Wipeout composition and
remain only explicit combat-test fixtures.

### Attribution and simultaneous outcomes

Scoring must not scrape presentation cues or inspect only the eventual presence of `Defeated`, because
both lose useful source context and can make same-tick behavior dependent on deferred commands.
Combat emits bounded, mode-neutral internal `CombatOutcomeFact` messages for authoritative protected
contact, damage, and defeat as those outcomes are applied. A fact carries stable event/tick, source
identity and team when present, target identity and team, weapon/recipe identity when present,
damage/distance, and outcome kind. It never carries a process-local entity over a network boundary
and is not registered as a wire message.

Wipeout drains all defeat facts for one tick, sorts by stable event ID, applies eligible points, and
only then evaluates threshold/tie/forfeit completion. A source disconnect in that tick does not erase
an already-authored outcome. One target can yield at most one defeat fact per life because combat's
existing same-tick defeated-target guard remains authoritative.

### Respawn and spawn protection ownership

The current `Defeated.reset_at_tick` and `combat::reset_due_fighters` encode a sandbox policy inside
combat. M07 replaces that with a reusable lifecycle seam:

```text
combat outcome
  -> Defeated + CombatOutcomeFact
  -> Wipeout schedules RespawnState when the match remains active
  -> generic authoritative fighter lifecycle performs due respawn
  -> Wipeout spawn selector supplies the resolved team point and protection duration
```

`Defeated` records the durable defeat identity. `RespawnState` records a mode-owned deadline.
`SpawnProtection` records the active protection deadline. The lifecycle reset restores health,
weapon economy, pose, collision, movement/input epoch, and effects from existing fighter/resolved
loadout state; it does not select a weapon or award score.

### Disconnect and restart policy

Only `Waiting` accepts participants. Existing Lightyear `ControlledBy` session lifetime removes the
spatial fighter after disconnect; a match roster keyed by stable `PlayerId` observes the link outcome
and removes the participant exactly once.

- `Waiting`: remove the participant and recompute readiness/capacity.
- `Countdown`: remove the participant, invalidate ready state as needed, and return to `Waiting`.
- `Active`: remove the participant; continue shorthanded if both teams remain represented, otherwise
  complete immediately by forfeit when only one team remains, or as a draw if both teams lose their
  last participant in the same lifecycle batch. Do not backfill.
- `Completed`: remove the participant from the restart quorum without changing the stored result.

Reconnect remains the M02 policy: a new session receives a new player/network identity. It is
rejected while a match is not waiting and may join after the next waiting phase. Once the completed
lock expires, an empty remaining roster satisfies the restart quorum and automatically reopens
`Waiting`, preventing an abandoned result screen from locking the server. Restart preserves the map
and connected session fighters, creates a new match ID, resets transient state, clears ready flags,
and reopens `Waiting` before another countdown.

### Telemetry definitions

Metrics are match-scoped and tick-derived:

| Metric | M07 definition |
|---|---|
| Match duration | active end tick minus active start tick |
| Time to first damage | first hostile nonzero damage tick minus active start tick |
| Fight duration | for a life that ends in defeat, defeat tick minus that life's first hostile-damage tick |
| Hit rate | attacks with hostile contact divided by accepted attacks, by preset and recipe fingerprint |
| Damage by distance | hostile applied damage in existing close/mid/long bands |
| Defeat/death rate | credited defeats and suffered deaths per participant-active minute, plus raw counts |
| Respawn-to-defeat | defeat tick minus the preceding initial-spawn/respawn tick |
| Movement time | alive active ticks whose authoritative displacement exceeds 0.25 world units; report moving/eligible ticks |
| Score margin | absolute difference between final team scores |

Every summary includes match ID, map identity/fingerprint, content fingerprint, rules revision, final
result, participant/team/build/weapon identity, disconnects, and dropped-record counters. Telemetry is
observational and cannot feed scoring or match transitions.

## Research log

| Date | Source | Finding | Decision |
|---|---|---|---|
| 2026-08-15 | `docs/{00-product-direction,02-fighter-model,04-maps-and-game-modes,05-gameplay-mvp,08-network-architecture}.md` and `docs/implementation/v1/{roadmap,milestone-06}.md` | M07 closes the combat vertical slice and owns the first reusable match lifecycle, concrete Wipeout schema, authoritative score/respawn/restart, and match evidence. | Keep match rules developer-owned, map data non-executable, and all results server-authored. |
| 2026-08-15 | Current `src/{gameplay,protocol,server,client,map,combat,movement}/`, `tests/network/`, and `tests/performance.rs` | The tree already has fixed schedule sets, four-client-capable harness composition, stable IDs, session-owned fighters, resolved team spawns, durable combat state, ordered cues, bounded telemetry, and a reserved match HUD. Sandbox reset/dummy/team mapping and presentation-only scoreboard are the remaining seams. | Add focused match modules, replace sandbox lifecycle in production, and extend the existing harness/evidence owners rather than adding parallel orchestration. |
| 2026-08-15 | `references/bevy/examples/state/{states,sub_states,computed_states}.rs`, `references/bevy/examples/ecs/state_scoped.rs`, and [Bevy 0.19 `State`](https://docs.rs/bevy/0.19.1/bevy/state/state/struct.State.html) / [`FixedUpdate`](https://docs.rs/bevy/0.19.1/bevy/app/struct.FixedUpdate.html) documentation | Bevy states are app-wide flow whose queued transitions normally apply in `StateTransition`; fixed schedules are intended for game rules and networking. | Use fixed-tick ECS match data as authority; do not mirror a Bevy state machine across peers. |
| 2026-08-15 | `references/lightyear/examples/lobby/src/{protocol,server,client}.rs` and its `Cargo.toml` | The pinned lobby example uses replicated ECS lobby state, reliable ordered commands, stable peer ownership, and explicit disconnect cleanup, but its permissive mid-game join and client-triggered start are not Brawler rules. | Reuse the protocol patterns, not its authority policy; Brawler validates readiness and phase on the server and rejects active-match joins. |
| 2026-08-15 | `references/lightyear/book/src/concepts/replication/{protocol,replicate}.md`, `concepts/advanced_replication/replication_logic.md`, and `concepts/reliability/channels.md` | Replicated component actions are ordered/reliable; current component updates provide recoverable state; ordered reliable channels fit idempotent discrete commands. | Replicate match/participant/respawn/protection state and send only ready/restart requests on the existing session channel. |
| 2026-08-15 | [Lightyear 0.29 lobby protocol](https://github.com/cBournhonesque/lightyear/blob/0.29.0/examples/lobby/src/protocol.rs) and [server](https://github.com/cBournhonesque/lightyear/blob/0.29.0/examples/lobby/src/server.rs) | Version-pinned primary examples confirm direct component registration, `OrderedReliable` commands, server-owned replicated lobby data, and connection-owned entities. | Keep exact APIs pinned to 0.29 and the current project composition. |
| 2026-08-15 | `src/combat/{effects,authority,attack,telemetry,cues}.rs`, `src/map/{model,definitions,server}.rs`, and M06 learn-from-errors | Combat already suppresses duplicate defeat per target/tick and has source/team/recipe context; map startup ordering is explicit; current reset embeds a delay in combat. | Emit internal outcome facts, batch scoring after damage, move respawn policy to match lifecycle, and make all new producer/consumer ordering schedule-visible. |

## Technical specification

### Application and module composition

Keep the current package and role features. Add one cohesive match family:

```text
src/matchplay/
  mod.rs                 shared composition, intentional public API, and schedule sets
  model.rs               stable match IDs, replicated phase/result/participant lifecycle shapes
  lifecycle.rs           reusable server fighter activation, respawn, protection, and cleanup
  telemetry.rs           bounded match records, aggregates, and summaries
  wipeout.rs             code-owned rules, team assignment, phase transitions, scoring, spawn policy
  client.rs              client-local match observation and presentation helpers (client-gated)
  tests.rs               pure and small-App match tests
src/combat/
  outcomes.rs            mode-neutral authoritative damage/defeat facts
src/client/hud.rs         readiness, score/time, scoreboard, result, respawn/protection presentation
tests/network/match.rs    lifecycle, authority, recovery, disconnect, and restart scenarios
```

The module is named `matchplay` because `match` is a Rust keyword; the user-facing concept remains
“match.” Do not create a mode trait, registry, separate protocol module, generic state-machine
framework, or one plugin per phase. M09 can compose a second mode plugin against the concrete
lifecycle seam.

Plugin responsibilities:

| Plugin | Installed in | Responsibility |
|---|---|---|
| `MatchModelPlugin` | client, server, tests | Register shared match resources/messages needed locally; no authority systems. |
| `AuthoritativeFighterLifecyclePlugin` | server/tests | Activate/freeze fighters, perform scheduled respawns, protection expiry/break, and exact match reset from existing resolved loadouts. |
| `WipeoutPlugin` | server/tests | Validate rules/map mode, create match root, assign teams/capacity, process commands/disconnects, advance phases, score outcomes, select spawns, complete/restart, and collect telemetry. |
| existing `ProtocolPlugin` extension | client, server, tests | Register match commands/outcomes and replicated match/participant/respawn/protection components; bump protocol identifiers. |
| `ClientMatchPresentationPlugin` | windowed client only | Observe replicated state, update HUD/scoreboard/results/protection/audio, and never mutate gameplay. Headless clients omit it. |

`ServerNetworkPlugin` remains connection/session ownership and delegates match admission/team
assignment to a narrow match API/helper. It must not absorb scoring or phase logic. `ServerCombatPlugin`
emits generic outcome facts and no longer owns timed reset. `GameplayPlugin` remains shared schedule
composition and contains no server-only Wipeout mutation.

### Shared and authoritative data

Recommended shared wire shapes (exact field visibility remains minimal):

```text
MatchId(u64)

MatchPhase
  Waiting
  Countdown { starts_at_tick }
  Active { ends_at_tick }
  Completed { completed_at_tick, restart_unlocked_at_tick, result }

MatchResult
  TeamVictory { team }
  Draw
  Forfeit { winner, departed_team }

MatchState component on MatchRoot
  match_id
  mode_definition_id
  phase
  team_scores: [u16; 2]
  target_score
  rules_revision

MatchParticipant component on each session fighter
  match_id
  ready
  restart_ready

RespawnState component
  respawn_at_tick

SpawnProtection component
  expires_at_tick

MatchCommandRequest message
  request_id
  match_id
  command: SetReady(bool) | ReadyForRestart

MatchCommandOutcome message
  request_id
  match_id
  decision: Accepted | Stale | WrongMatch | WrongPhase | NotParticipant | Locked
```

`PlayerId`, `NetworkEntityId`, `TeamId`, `SpawnAssignment`, and selected/resolved weapon types remain
their current stable shared identities. Do not duplicate them in `MatchState`. `MatchRoot` is a
server-created replicated entity and not the map root. Match state updates normally; root identity and
match ID may use once-replication only if a new root is created for every match. The recommended
simpler lifecycle keeps one root for the server process and updates its ordinary replicated
`MatchState`, including monotonic match ID on restart.

Server-only state includes `WipeoutRules`, `MatchRoster`, per-player request watermarks, respawn
ordinals, per-life telemetry state, prior poses, bounded records/summaries, and exact match-member
indexes. No server resource is reconstructed from client data.

### Validated Wipeout rules

`WipeoutRules::default()` validates once at plugin construction/startup:

| Rule | Production value |
|---|---:|
| team count | 2 |
| minimum participants per team | 1 |
| maximum participants per team | 2 |
| target score | 10 |
| countdown | 180 ticks / 3 s |
| active limit | 10,800 ticks / 180 s |
| respawn delay | 180 ticks / 3 s |
| spawn protection | 90 ticks / 1.5 s |
| completed input lock | 60 ticks / 1 s |
| movement displacement epsilon | 0.25 world units/tick |
| retained match summaries | 32 |
| retained match records | 2,048 |

Tests and process automation may inject smaller valid timing/score values through explicit
configuration; production composition must prove it uses the values above. Reject zero/overflowing
durations, fewer than two teams, min greater than max, capacity beyond the map/server limit, zero
score target, and any tick deadline calculation that would overflow.

### Phase and command lifecycle

The fixed-tick state machine is:

```text
Waiting
  all connected participants selected + ready
  and each team meets minimum
    -> Countdown(starts_at_tick = now + 180)

Countdown
  deadline reached with roster/readiness still valid
    -> Active(ends_at_tick = now + 10_800)
  disconnect / readiness invalidated
    -> Waiting

Active
  threshold batch / timeout / empty-team forfeit
    -> Completed(immutable result)

Completed
  lock elapsed + every remaining participant restart-ready
    -> exact cleanup + new MatchId + Waiting
```

Weapon selection precedes match readiness. An `INTERACT` press while the local controlled fighter is
still `SelectingWeapon` remains selection confirm and sends no match command. In `Waiting`, it sets
ready; in `Completed`, it sets restart-ready after the lock. Cancel/B may clear ready in `Waiting` if
the existing client input shell exposes that cleanly; this is presentation behavior, not a required
new wire command beyond `SetReady(false)`.

The server scopes every command to the receiving link's fighter, match ID, phase, and monotonic
request ID. Equal request IDs replay the stored outcome; older IDs return `Stale`; commands never name
another player/team/entity. Readiness is cleared on roster-invalidating disconnect, match restart,
and any accepted loadout change. No loadout change is accepted after a participant becomes ready.

### Team assignment and admission

During `Waiting`, after compatibility succeeds and before fighter spawn:

1. Count current participants for team 0 and team 1.
2. Select the team with fewer members; ties choose the lower team ID.
3. Reject with explicit `MatchFull` if both teams are at capacity.
4. Select an initial spawn through the same deterministic spawn selector used for respawn.
5. Create the fighter/session participant with `ready = false`, inactive collision/gameplay state,
   and existing weapon-selection state.

Any join while `Countdown`, `Active`, or `Completed` is rejected with `MatchInProgress` before fighter
spawn. Compatibility, server-full, and content mismatch retain their current more specific outcomes.
Tests must cover duplicate hello, same-frame joins, disconnect/rejoin, and capacity without depending
on query iteration order.

### Combat gating, facts, and scoring

A server-only `ActiveCombatant` marker gates authoritative movement, fire, and target eligibility.
It is inserted for every valid participant at `Active` entry and removed at completion or invalidated
countdown/restart. This is a generic lifecycle marker; movement and combat add no mode-name checks.

Combat writes `CombatOutcomeFact::Damage` for nonzero applied damage and
`CombatOutcomeFact::Defeat` when it creates a defeat. Wipeout scoring consumes only defeat facts whose
target is an active participant in the current match:

- source player differs from target, source team differs from target team, and source team is 0/1:
  add exactly one point to source team;
- source equals target: record self-defeat, no score;
- missing/environmental source: record environmental defeat, no score;
- same-team non-self source: record invalid/friendly outcome, no score and emit a diagnostic;
- stale/wrong-match/duplicate event ID: no score and increment the relevant diagnostic counter.

Collect and order all current-tick facts before score mutation. Saturating score arithmetic is a
last-resort safety guard; validated target and bounded match duration should keep ordinary scores far
below `u16::MAX`. Evaluate completion once after the batch: when either score meets the target, the
higher score wins and equal scores draw. No presentation cue, client counter, or telemetry aggregate
may award a point.

### Spawn, defeat, respawn, and protection

On valid active defeat, combat immediately disables collision and active effects as today. Wipeout
adds `RespawnState` only if the match is still active after the complete score batch. A completion in
the same tick cancels pending respawn.

The pure respawn selector receives sorted team spawn points and current living participant poses:

1. Mark a point clear when no living fighter center is within two fighter radii plus skin width.
2. Prefer the clear set when nonempty; otherwise use all team points.
3. For each candidate, compute its minimum squared distance to any living hostile.
4. Choose the greatest minimum distance; break equal scores by lowest `SpawnPointId`.
5. With no living hostile, cycle by stable `(match_id, player_id, respawn_ordinal)` over sorted points.

At the deadline, the generic lifecycle restores maximum health, weapon capacity/ready phase,
resolved spawn pose/facing, collision layers, zero velocity, neutral input/freshness epoch, empty
effects, and updated `SpawnAssignment`/`SpawnState`. It removes defeat/respawn state and inserts
spawn protection. Accepted attack removes protection before that tick's damage resolution; expiry is
processed in lifecycle before movement/fire. Protected targets reject hostile damage, knockback, and
slow. A shield contact may produce presentation feedback and a distinct protected-contact telemetry
fact, but does not count as a hostile hit, applied damage, attack-with-hostile-contact, or defeat.
Owner self-effects follow the existing recipe policy.

Initial activation uses the same reset/spawn function and protection policy so first spawn and
respawn cannot drift into separate code paths.

### Scheduling and deferred-command boundaries

Keep the existing 60 Hz `SimulationTick`. Add explicit sets rather than relying on plugin insertion
order:

| Schedule position | Required work |
|---|---|
| `Update`, after Lightyear receive | process match command messages and link lifecycle into bounded pending server state; no timer/score mutation |
| `FixedUpdate`, `GameplaySet::Lifecycle` | apply roster changes; validate/advance waiting/countdown/active deadlines; schedule/perform respawns; expire protection; install/remove active markers; explicit `ApplyDeferred` before input/simulation |
| `FixedUpdate`, `GameplaySet::Simulation` | authoritative movement for active combatants; capture post-movement telemetry pose |
| `FixedUpdate`, `GameplaySet::Fire` | break protection on accepted attacks and execute normal authoritative fire |
| `FixedPostUpdate`, after `CombatSet::Damage` | drain/order authoritative outcome facts; update per-life telemetry; apply Wipeout score/result exactly once |
| explicit `ApplyDeferred` after match outcome resolution | make completion gating and cleanup visible before later observers/finalization |
| `FixedPostUpdate`, telemetry/cues/finalize | emit existing combat cues, update durable authoritative tick, finalize match telemetry, then advance `SimulationTick` |
| `Last` | extend the existing single summary/process evidence owner; no independent exit clock |

Add schedule-order tests that record these phases. In particular, a threshold defeat must be applied,
scored, completed, and prevented from scheduling a respawn in one deterministic tick. A fighter
activated after countdown must be visible to movement/fire in that same fixed tick only after the
documented deferred boundary.

### Protocol, replication, and recovery

Extend root `src/protocol.rs`; do not add a match protocol module. Register:

- `MatchCommandRequest` client-to-server and `MatchCommandOutcome` server-to-client on the existing
  ordered reliable `SessionChannel`;
- `MatchRoot`, `MatchState`, `MatchParticipant`, `RespawnState`, and `SpawnProtection` as ordinary
  replicated components with no prediction/interpolation;
- any stable `MatchId` member marker required to clean projectiles/effects by exact match instance.

Bump the supported protocol version/ID and registry fingerprint expectations. Add the new Wipeout
mode definition and layout schema to canonical map material; bump the map/content fingerprint format
versions and built-in recipe revision. Client and server must reject old sandbox content before
fighter/match admission.

The current `MatchState` component is the recovery snapshot. Packet loss, duplicate updates, late
component arrival, and reconnect do not require score-event replay. M07 rejects active late join and
session resumption, but deterministic tests must still prove that two already-connected clients
converge after dropped/reordered updates and that the next waiting match is joinable after restart.
Client-local edits to match/participant/respawn/protection components never affect the server.

### Restart and exact cleanup

Restart is an explicit transaction, not an App reset. When quorum is reached:

- archive one bounded immutable `MatchSummary` and clear only the live accumulator;
- despawn every projectile/delivery and other entity marked with the completed match ID;
- clear active attack trackers, pending payload/delivery messages, combat outbox entries, transient
  effects/motion/knockback, defeat/respawn/protection/active markers, and stale automation inputs;
- restore each connected fighter through the shared initial-spawn reset path while retaining
  `PlayerId`, `NetworkEntityId`, `TeamId`, connection ownership, selected/resolved weapon, and map;
- reset team scores, phase deadlines, ready/restart-ready state, request epoch as specified, and
  per-life/movement telemetry state;
- allocate a strictly greater nonzero `MatchId`, publish `Waiting`, and accept new participants.

Cleanup uses stable match/member markers and current resources, not broad world despawn or local
client entity IDs. Repeating a restart request or cleanup system is idempotent. The arena/map root,
static colliders, client asset handles, and process endpoint survive restart.

### Client HUD, results, and audio

Extend the existing HUD shell rather than create a second UI tree:

- persistent top match strip: team scores, target, phase, and authoritative countdown/time remaining;
- `View`/Tab scoreboard: both teams, player IDs, selected weapon, ready/alive/respawning/disconnected
  state, score, and local-player marker without relying only on color;
- waiting overlay: select weapon, then `A`/Space/Enter to ready, with team counts and readiness;
- countdown overlay: large 3/2/1 and frozen combat controls;
- active feedback: respawn countdown, visible protection shield/icon and expiry, score changes, and
  timer warning without obscuring aim/combat;
- completed overlay: winner/draw/forfeit, final score/margin, `A` restart-ready prompt after lock,
  and quorum progress.

All deadline display uses replicated authoritative tick/deadline facts, never an independent match
timer. Existing `AuthoritativeTick` on replicated fighters is sufficient while participants exist;
if implementation proves completed/waiting display can lack a fighter, add one bounded match clock
component rather than estimating from render time.

Phase/score/result audio derives locally from changed replicated state, is deduplicated by match ID
and transition, and obeys M06 live/coalescing caps. No new audio asset is required unless the existing
licensed set cannot distinguish countdown, score, and result; any addition follows the asset manifest.

### Match telemetry and evidence

`MatchTelemetry` owns a bounded live accumulator and up to 32 summaries. It consumes authoritative
outcome facts and post-movement poses, and reads existing weapon telemetry deltas keyed by preset plus
recipe fingerprint. It does not duplicate combat's detailed record stream.

Add match checkpoints to `server/verification.rs`, the existing client automation/cue capture, and
the shared process report. Required real-process facts include match/map/content identity, four
accepted participants for the 2v2 profile, countdown/active/completed/restarted phases, final score
and result agreement, at least one defeat/respawn, cleanup counts, telemetry completeness/drop counts,
and zero client-authored mutations. Use shortened injected rules for bounded automation while a
separate production-composition test asserts the 180-second/10-point defaults.

## Trackable implementation plan

Implement as five green vertical slices. Each slice ends with affected role-specific checks and the
M06 regression subset; do not begin client polish while authoritative lifecycle/network recovery is
red.

### Prerequisite and mode-content foundation

- [x] Re-run the accepted M06 format, role-specific Clippy/tests/builds, server feature graph,
  45-case network suite, 7 performance cases, and relevant process smoke baseline against the exact
  M07 starting commit.
- [x] Add Wipeout stable mode ID/layout schema and migrate the built-in map recipe revision away from
  sandbox/practice-dummy requirements; bump and test canonical content/protocol fingerprints.
- [x] Add shared match IDs/phases/results/components, validated `WipeoutRules`, pure transition/team/
  spawn/attribution helpers, and focused model tests.

### Authoritative lifecycle and Wipeout rules

- [x] Add mode-neutral combat outcome facts and preserve one-defeat-per-life behavior without using
  presentation cues as gameplay input.
- [x] Extract sandbox timed reset into reusable authoritative activation/respawn/protection/reset
  lifecycle; remove the production practice dummy and duplicate reset authority.
- [x] Add match root/roster, deterministic team admission, ready/countdown/active/completed flow,
  fixed-tick gating, score/timer/tie/forfeit rules, disconnect handling, and restart cleanup.
- [x] Add deterministic spawn selection, `RespawnState`, protection target filtering/early break,
  and initial-spawn parity.

### Protocol and recovery

- [x] Register match commands/outcomes and replicated durable components; implement per-link request
  idempotency, wrong-match/phase rejection, and explicit active-match join rejection.
- [x] Extend the shared harness with four-client match helpers and `tests/network/match.rs`; prove
  identical phase/score/result, authority, impairment convergence, disconnect policy, restart, and
  new-waiting admission.
- [x] Extend the single process verification/evidence path for a shortened four-client UDP match and
  repeated-match cleanup under local, typical, and adverse profiles.

### Telemetry and client presentation

- [x] Add bounded match/per-life/movement telemetry, summary archival, drop counters, existing weapon
  aggregate deltas, and deterministic serialization/report checks.
- [x] Replace the reserved match shell/scoreboard with phase, roster, score/time, respawn/protection,
  result, and restart-quorum presentation; preserve readiness/error and aspect-ratio behavior.
- [x] Route controller/keyboard interaction to weapon selection versus ready/restart by replicated
  phase, add bounded match audio transitions, and keep headless clients renderer/audio independent.

### Verification and handoff

- [x] Run format, role-specific Clippy/tests/builds, isolated server features, all deterministic
  network/performance suites, three-profile match process runs, and repeated-process fixed-port
  cleanup.
- [x] Record the verification disposition for two-window keyboard/mouse, physical-controller 1v1, and
  automated/headless 2v2 checks. Automated 2v2 and partial native-window checks passed; the physical
  controller, perceptual audio, 1440x900, and full human match observations were explicitly deferred
  to the M11 hardening playtest by the user's 2026-08-15 closeout decision.
- [x] Produce match telemetry summaries for all four weapon presets across controlled matches and
  record whether duration/range/counterplay evidence is adequate for the vertical-slice review.
- [x] Resolve the playtest gate. The user approved closeout on 2026-08-15 with the remaining supervised
  observations recorded as known limitations and moved visibly to M11 rather than represented as
  completed M07 evidence.

## Test plan

### Pure rule tests

- [x] Validate production and injected `WipeoutRules`; reject zero, overflow, invalid capacity/team,
  and min/max combinations.
- [x] Team assignment is deterministic and balanced for every join/leave order up to capacity;
  same-frame joins cannot overfill a team.
- [x] Transition table covers waiting readiness, cancellation, countdown deadline, invalidation,
  active timeout, completion immutability, lock, quorum, and monotonic restart ID.
- [x] Attribution covers hostile, self, same-team invalid, environmental, stale match, duplicate event,
  source disconnect, simultaneous threshold, score overflow guard, timeout win, both scoring draw
  paths, and simultaneous last-team disconnect draw.
- [x] Spawn selection covers clear preference, hostile-distance ranking, stable tie break, no-hostile
  cycling, occupied fallback, finite geometry, and stable result independent of input order.
- [x] Telemetry definitions cover no-damage match, first damage, multiple lives, incomplete fights,
  movement epsilon, per-minute zero-duration guard, distance bands, score margin, forfeit, and drops.

### Small-App/ECS and schedule tests

- [x] Production composition creates exactly one match root after the Wipeout-compatible map is
  resolved and before admission; invalid mode/map rules fail before endpoint bind.
- [x] Explicit trace proves lifecycle/deferred boundary precedes movement/fire, combat damage precedes
  match score, score batch precedes completion/respawn decision, and tick advancement remains last.
- [x] Fighters are frozen/ineligible in waiting/countdown/completed and active in `Active`; no stale
  buffered input crosses selection, activation, respawn, or restart epochs.
- [x] Defeat schedules one respawn only while active; due respawn restores exact loadout/runtime,
  selects a legal team point, protects the fighter, and emits no score/reset twice.
- [x] Protection blocks hostile damage/effects, permits movement, expires at deadline, breaks on an
  accepted attack, records shield contact without hit-rate credit, and cannot survive defeat/
  completion/restart.
- [x] Threshold defeat completes in the same tick without scheduling respawn; simultaneous facts are
  ordered and evaluated as a batch.
- [x] Restart twice in one App preserves session/loadout/map identity, increases match ID, and leaves
  exact zero stale projectiles/effects/trackers/pending messages/defeats/respawns/protections/inputs.
- [x] Practice dummy is absent from production Wipeout composition and remains available only through
  explicit test fixture composition.

### Deterministic network tests

- [x] Four clients join during waiting, receive balanced 2v2 teams, select weapons, ready, observe one
  countdown/active phase, complete a match, agree on score/result/match ID, and restart.
- [x] Ready/restart messages are link-scoped and idempotent; stale, duplicate, wrong-match, wrong-phase,
  and forged target/team attempts cannot change another participant or match authority.
- [x] Clients cannot mutate team, phase, timer, score, result, respawn, protection, or match ID by
  local component edits or fighter input.
- [x] Packet delay/loss/duplication/reordering still converges current phase, scores, result,
  participant state, respawn deadline, and protection; no score event history is required.
- [x] Disconnect in each phase follows the specified result; active shorthanded continuation and
  empty-team forfeit/both-empty draw are distinct; completed result remains immutable; restart quorum
  drops departed participants; an empty completed roster returns to waiting after the lock; reconnect
  is rejected until waiting and then receives a new identity.
- [x] Repeated matches do not accumulate replicated/local match roots, fighters, projectiles, effects,
  HUD generations, cues, or summaries beyond explicit bounds.
- [x] Map/content/protocol mismatch rejects before participant spawn; both roles resolve the built-in
  preset against Wipeout requirements and no client can restore sandbox/dummy semantics.

### Process, performance, and visual verification

- [x] A real dedicated server plus four headless clients completes and restarts a shortened 2v2 match
  under local, typical, and adverse UDP profiles with identical final evidence and clean ports.
- [x] Record the normal 1v1 disposition: process restart behavior is covered by automated and native
  window evidence; a human normal-duration 1v1 timing judgment was not performed and is explicitly
  deferred to M11.
- [x] Match additions retain fixed-step p95 `< 16.67 ms` in the 4-participant/projectile worst cases;
  telemetry and scoreboard work are bounded and do not scan historical records each tick.
- [x] Isolated server features still exclude window, renderer, UI, text, asset, audio, PNG/Vorbis, and
  device-input capabilities and run with no `assets/` tree.
- [x] Record the aspect-ratio disposition: 1280x720, 960x540, and a taller 4:3 layout were inspected and
  resulting defects fixed; 1440x900 and the complete state matrix are explicitly deferred to M11.
- [x] Record the input-device disposition: keyboard/mouse and automated action paths are covered;
  physical Xbox-like controller comprehension and the complete held-scoreboard flow are explicitly
  deferred to M11.
- [x] Record the audio disposition: caps, deduplication, and asset/output-free degradation have
  automated coverage; perceptual distinguishability during simultaneous combat is explicitly deferred
  to M11.

### Evidence rules

- Score and result assertions inspect server `MatchState` plus authoritative outcome facts; HUD text,
  audio, client cues, or telemetry totals do not prove authority.
- Network comparisons use stable match/player/network/team/spawn/weapon/map IDs and replicated values,
  never process-local Bevy entities.
- A 2v2 gate uses four actual client Apps/processes. Fixed dummies do not count; no direct server score
  mutation may stand in for gameplay input in the end-to-end case.
- Rule tests may use short injected timings/targets, but production-composition tests and user
  playtest must separately prove the approved defaults.
- Match telemetry is checked against the same authoritative fact stream used for observation, while
  scoring is checked independently so a shared analytics bug cannot falsely validate the result.
- Extend the existing harness, `server/verification.rs`, evidence checkpoints, and client automation
  completion gate. Do not add wall-clock sleeps to gameplay tests or a second process exit owner.
- Visual/controller/audio evidence complements automated rule, authority, recovery, and cleanup
  checks; it cannot replace them.

## Implementation and verification evidence

Implementation completed on 2026-08-15 and moved to `Verifying`. The resulting slice adds the
server-owned Wipeout lifecycle, a replicated match root and participant state, balanced 1v1/2v2
admission, ready/countdown/active/completed/restart phases, fixed-tick combat gating, batched scoring,
timeout/tie/forfeit rules, deterministic respawn/protection, exact restart cleanup, match commands,
phase-aware client HUD/input/audio, and bounded match summaries. The production map now resolves
against Wipeout mode ID `2`; the practice dummy remains available only through explicit legacy test
fixture composition.

Automated evidence on the implementation tree:

- `cargo fmt --all -- --check` and role-specific client/server Clippy with `-D warnings` passed. The
  initial baseline run exposed Rust 1.95 lint drift in pre-existing performance/network tests; those
  item-scoped warnings were repaired without changing their behavior.
- `cargo test --all-targets --features client,server,network-test -- --test-threads=1` passed with
  128 library tests, 56 deterministic/loopback network tests, and 8 performance tests.
- `cargo build --locked --no-default-features --features {client,server}` passed for both binaries,
  and `scripts/check-server-features.sh` confirmed the dedicated server still excludes client
  presentation/device capabilities.
- The M07 four-participant match/telemetry performance case measured fixed-tick p95 `0.392 ms` on
  Apple Silicon/macOS, below the `16.67 ms` budget; the existing 100-fighter/200-projectile and M05
  composed worst cases also remained green.
- `scripts/network-match.sh` used an explicit target-3/1,200-tick verification ruleset, completed a
  four-process 2v2 match, archived a summary, and restarted
  from match ID `1` to `2` under local, typical, and adverse profiles. Current representative facts
  were local `2-1`, 3 defeats/3 respawns; typical `1-3`, 4/3; and adverse `1-1` draw, 2/2. Every run
  retained four participants, nonzero per-team participant ticks, map/content/rules identity,
  bounded records with zero drops, and preset/fingerprint attack evidence. Reusing the adverse run's
  fixed UDP port immediately for a local run also passed.
- Controlled local runs covered all four M06 presets. Pulse recorded 244 accepted attacks/12 hostile
  contacts, Scatter 58/15, Arc 33/10, and Blade 156/8. These shortened automated matches prove
  attribution and collection, but they are not sufficient evidence for final range/counterplay or
  normal-duration balance decisions; those remain playtest questions.

During verification, audit failures were fixed rather than waived: countdown cancellation was
initially rejected as wrong-phase, and headless clients could auto-ready before the requested
four-player roster had replicated. Tests now cover countdown cancellation/quorum clearing and the
roster readiness barrier, and the process verifier fails closed unless the archived summary contains
four participant identities plus map, content, rules, participant-time, weapon, defeat, respawn, and
drop evidence. A legacy harness activation shim was also isolated from M07 tests after it masked the
real waiting lifecycle, projectile membership/cleanup was narrowed to exact match IDs, and the shared
fixed schedule now has an explicit deferred-command flush between lifecycle and input/simulation/fire.
The first final adverse sample produced no defeat in the shortened stochastic window and was rejected;
the rerun completed with full evidence, confirming the verifier fails closed rather than treating mere
connectivity as match proof.

Recovery evidence is intentionally layered: the deterministic suite injects delayed, dropped,
duplicated, and reordered raw packets plus duplicate/reordered commands, asserts durable phase,
score, respawn/protection, result, and spawn-assignment convergence, and runs two restarts with exact
server cleanup; local/typical/adverse real UDP runs add latency, jitter, statistical loss,
client-observed completion, restart convergence, and process cleanup. Client presentation caches reset
on match ID and retain only bounded current/disconnected roster facts, while cue histories and archived
summaries retain their independently tested bounds.

Native window inspection initially could not attach to the raw Bevy executable because it has no
macOS application bundle identifier. A temporary out-of-tree application-bundle wrapper made the two
real production clients addressable without changing product composition. That inspection exercised
weapon confirmation, ready commands, Waiting-to-Countdown-to-Active flow, authoritative score/timer,
arena/camera presentation, and client focus at the default 1280x720 window, compact 960x540, and a
taller 4:3 layout. It exposed unsupported separator glyphs, low-contrast selection text over arena
geometry, clipped bottom/right HUD text, a redundant READY label, and a missing large countdown
numeral. Those defects were fixed with ASCII-safe labels, a dedicated UI overlay camera, bounded
high-contrast selection panel, corrected anchors/widths, and authoritative-deadline countdown text;
the affected client Clippy/tests and the complete regression suite passed afterward.

The automation display cannot host a complete 1440x900 window, cannot supply a physical controller,
and cannot establish whether audio transitions are perceptually distinguishable. It also does not
replace a human normal-duration match, held-scoreboard, defeat/respawn/protection/result/restart, and
counterplay judgment. The user's 2026-08-15 closeout decision accepts these as non-blocking M07
limitations and defers the supervised observations to the M11 hardening playtest. This is a disposition,
not evidence that the checks passed.

## Implementation feedback — 2026-08-15

All six review findings were accepted as specification gaps and corrected without expanding the
milestone's product scope:

| Finding | Decision and remediation |
|---|---|
| Respawn after completion | Implemented now. All completion causes use one fighter cleanup operation that removes active, respawn, and protection state; respawn and protection expiry also require the current match to be active. Threshold and forfeit regression coverage asserts no lifecycle state survives results. |
| Missing waiting/results presentation | Implemented now. The HUD shell has a large waiting prompt with readiness progress and a large result overlay with winner/draw/forfeit, final score, margin, restart lock/prompt, and quorum progress derived from authoritative state and ticks. |
| Missing preset death evidence | Implemented now. Summaries archive credited defeats, suffered deaths, participant-active ticks, and derived per-participant-minute rates by preset; the process report fails closed when preset defeat/death evidence is absent. |
| Stale replicated spawn assignment | Implemented now. `SpawnAssignment` uses change replication so deterministic respawn selection reaches clients. |
| Missing reusable lifecycle seam | Implemented now. `AuthoritativeFighterLifecyclePlugin` and `matchplay/lifecycle.rs` own activation/reset mechanics, due respawn, protection expiry, and completion cleanup; Wipeout retains phase and scoring decisions. |
| Cumulative per-match drop evidence | Implemented now. Each live match stores its starting global drop counter and archives only the saturating delta; a later clean match reports zero drops after an earlier overflowing match. |

The affected unit, schedule, network, role-isolation, and process checks must be green before this
feedback is considered verified. The remaining supervised visual/controller/audio observations are
unchanged.

## External implementation review — 2026-08-15

The external Wipeout review was validated against the approved specification. All behavioral,
coverage, presentation, and repository-organization findings were accepted; the duplicated render
input read was treated as ownership debt rather than a demonstrated same-frame input race.

| Finding | Decision and remediation |
|---|---|
| 2v2 countdown departure continued as 2v1 | Implemented now. A roster snapshot detects any departure during `Countdown`, returns the same match to `Waiting`, and clears every remaining ready flag. A four-client regression advances beyond the cancelled deadline and proves no activation. |
| Monolithic phase system and duplicated reset derivation | Implemented now. Roster observation, waiting/countdown transition, active entry, active completion, restart, cleanup, and respawn selection are separate explicitly chained systems. Shared lifecycle code derives health/ammunition once for activation, respawn, and restart; schedule sets live at the match composition root and lint exceptions are item-scoped. |
| Verification environment changed gameplay rules | Implemented now. Production defaults always use production rules. The process script selects a validated `WipeoutRulesProfile::ProcessVerification` through the explicit `--wipeout-rules verification` server option; the assertion environment flag only enables evidence collection. |
| Wildcard matchplay API | Implemented now. `matchplay/mod.rs` exposes explicit public and crate-private items instead of wildcard re-exports. |
| Missing deterministic match impairment evidence | Implemented now. The shared raw-link harness deterministically delays, drops, duplicates, and reorders packets, then proves current respawn, protection, spawn assignment, score, completed result, and match state converge. |
| Process verifier accepted zero respawns | Implemented now. Process completion fails closed unless the archived match includes at least one completed respawn. |
| Headless ready retry deadlocked after countdown cancellation | Implemented now. Observing countdown re-arms the prior Waiting command key, allowing the same `MatchId` to ready again after cancellation; a focused client regression covers the transition. |
| Scoreboard omitted local marker | Implemented now. The held roster prefixes the local entry with `YOU`, independently of team color. |
| Initial spawn bypassed the shared selector | Implemented now. Admission uses `select_spawn` with the same clearance, hostile-distance, stable-ID, match/player seed, and fallback policy as respawn; integration coverage reconstructs and compares every initial assignment. |
| Missing attribution rejection diagnostics and off-tick visibility | Implemented now. Bounded process-lifetime counters cover stale ticks, duplicate event IDs, unknown/wrong-match targets, and friendly-invalid defeats; facts are retained in telemetry before scoring rejection and same-team defeats emit a warning. |
| Match verifier lived outside its specified owner | Implemented now. Match process verification and report helpers live in `server/verification.rs`. |
| Same-frame admission depended on query order | Implemented now. Pending link receivers are ordered by stable Lightyear `RemoteId` before identifier/team assignment; repeated four-client integration runs assert identical per-client teams. |
| `INTERACT` was sampled independently for match commands | Implemented now. Ready/restart reads the shared `PendingLocalActions` interaction indicator populated by the native input sampler. |

Feedback verification passed format, client-only/server-only/all-feature Clippy with `-D warnings`,
the 128-library/56-network/8-performance full suite, server feature isolation, and a real local
four-process match. The process match completed 3–3 as a draw with 6 defeats, 4 respawns, one bounded
summary, and zero dropped records before restarting from match ID 1 to 2.

## Closeout decision and learning review — 2026-08-15

The user explicitly requested that Milestone 07 be closed after the accepted review findings were
implemented and the affected automated, deterministic-network, role-isolation, performance, and real
process checks passed. No known blocking authority, input, collision, cleanup, replication, telemetry,
or match-loop defect remains. The supervised physical-controller, perceptual-audio, complete
aspect-ratio/state-matrix, and human normal-duration 1v1 observations were not performed; they are
accepted as non-blocking for this closeout and recorded in the roadmap backlog for M11. Milestone 08
may proceed from specification review.

Learn-from-errors findings and prevention:

- Completion cleanup drifted because timeout, threshold, forfeit, respawn, and restart had independent
  mutation paths. Keep lifecycle mutation behind one reusable seam, gate due work by current match and
  phase, and test every completion cause for absence of stale lifecycle components.
- Required waiting/results states were initially treated as presentation polish. Translate every
  specified player-visible state and datum into focused HUD-model tests before calling presentation
  complete.
- Telemetry answered attacker/team questions but not target-preset or per-match drop questions. Define
  the comparison questions and counter baselines before fixing the summary schema, then test sequential
  matches as well as one match in isolation.
- One large phase system, wildcard exports, and broad lint suppression obscured ownership. Keep schedule
  sets at the composition root, split systems by lifecycle phase/state owner, expose explicit APIs, and
  attach unavoidable lint exceptions to the smallest item.
- Statistical process impairment and one roster-departure case were mistaken for complete deterministic
  coverage. Distinguish deterministic packet transformation from statistical UDP profiles and test
  both invalidating and still-valid roster changes, including 2v2 countdown departure.
- Environment-controlled verification silently changed gameplay rules. Behavioral profiles must be
  explicit validated configuration; evidence/assertion switches may observe but must not alter rules.
- Initial admission and match commands bypassed shared spawn/input seams. New lifecycle and input paths
  must reuse the authoritative selector and sampled action state, with equivalence tests at first spawn,
  respawn, and restart.

These prevention rules are captured in the milestone checklist and repository architecture guidance.
No new reusable skill was created: the failures are project-specific applications of the existing Bevy
ECS/scheduling guidance rather than a distinct repeatable workflow.

## Risks and follow-up decisions

- **Lifecycle extraction regression:** moving reset from combat can disturb M04–M06 defeat/cue and
  impairment evidence. Preserve defeat facts/cues and introduce the generic respawn seam in a green
  vertical slice before Wipeout scoring.
- **Deferred same-tick ordering:** threshold defeat, commands, score, completion, cleanup, and cue
  emission cross existing fixed sets. Schedule-trace and threshold-no-respawn tests are mandatory.
- **Four-client automation realism:** headless targeting must still send ordinary client input; it may
  not mutate server poses/health/score. If it proves insufficient, revisit bots through specification
  review rather than hiding server test controls in production systems.
- **Spawn safety versus spawn trapping:** the distance selector and short protection are deterministic
  safety measures, not proof of arena fairness. The user playtest remains the decision point for
  timing/cover tuning.
- **No active reconnect:** rejecting reconnection during active play is explicit v1 behavior, but a
  production session-resumption design will later need persistent participant identity separate from
  connection lifetime.
- **Telemetry interpretation:** the operational fight/movement definitions are comparable diagnostics,
  not universal balance truth. Record raw counts/ticks alongside derived rates so M08 can reinterpret
  without replaying assumptions.
- **Mode seam sufficiency:** M09 must reuse match identity, phase/result, participant activation,
  restart, HUD shell, and lifecycle while replacing only rule composition. If Wipeout types leak into
  fighter/weapon code, M07 has not met its architectural exit criterion.

## Exit checklist

- [x] Specification is validated by the user before production implementation begins.
- [x] M06 accepted baseline was green on the exact M07 starting commit, as recorded in Tracking and the
  implementation evidence.
- [x] Built-in arena resolves through concrete Wipeout requirements with new stable mode semantics;
  sandbox dummy requirements are not silently reinterpreted.
- [x] Server-owned fixed-tick match state covers waiting/countdown/active/completed, teams, capacity,
  readiness, timer, score, tie/forfeit, disconnect, result, and restart.
- [x] Combat emits mode-neutral authoritative facts; fighter/weapon/movement code contains no Wipeout
  score/victory branch and generic lifecycle owns activation/respawn/reset.
- [x] Spawn selection, respawn, protection, early break, and input epochs are deterministic and
  server-authoritative.
- [x] Four real client Apps complete a repeatable 2v2 match; all clients converge on match ID, phase,
  final score, result, and next-match restart without process restart.
- [x] Client commands cannot author teams, scores, timers, respawns, protection, results, or restart;
  duplicate/stale/wrong-scope commands are harmless.
- [x] Every phase's disconnect policy, active-match rejection, no-resumption reconnect, forfeit, and
  restart quorum behavior is verified.
- [x] Repeated matches leave no stale match-scoped fighter/projectile/effect/input/timer/score/HUD or
  unbounded telemetry state while preserving map/session/loadout identity as specified.
- [x] Match telemetry captures every required metric with bounded raw evidence and drop counters and
  is sufficient for the first weapon/arena comparison.
- [x] Normal-duration and full controller/keyboard readability observations have an explicit closeout
  disposition: not performed in M07, accepted by the user as non-blocking, and deferred to M11 without
  claiming a passing result.
- [x] Role feature isolation, fixed-step performance, local/typical/adverse process evidence, and all
  automated M03–M06 authority/combat/map regressions remain green; unperformed 1440x900, physical-
  controller, and perceptual-audio observations have the explicit M11 disposition above.
- [x] Combat vertical-slice technical review and partial native-window review are recorded; all blocking
  authority, input, collision, readability, cleanup, replication, telemetry, and match-loop findings
  were resolved. The user approved the documented human-observation deferral before M08 implementation.
- [x] User feedback is triaged, affected verification rerun, learn-from-errors recorded, deferred checks
  entered in the roadmap backlog, and roadmap status updated for completion.
