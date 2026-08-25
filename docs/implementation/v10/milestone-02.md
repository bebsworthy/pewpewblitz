# V10 Milestone 02 — Mirrored Heist and dedicated map

## Status

`User playtest`

The user started M02 preparation on 2026-08-25 while M01 remained in `User playtest`. M01 completed
with user acceptance later that day, including its feedback reruns and learning review. The user
approved this specification and explicitly started production implementation on 2026-08-25.

## Player-visible outcome

One original map, **Twin Vaults**, provides a complete simultaneous attack-and-defence mode. Each
team protects one public safe while attacking the opposing safe. Destroying one safe wins;
destroying both in the same completed damage tick draws; reaching the deadline compares exact
remaining-health fractions. The routed product advertises exact Heist 1v1, 2v2, and 3v3 entries,
with practice, HUD, results, audio, restart, reconnect, and recovery support.

The Heist safe is a large team-owned structural objective. It is not the Kenney Mini Dungeon
treasure chest reserved for M03, never opens, never drops loot, and never runs ordinary map-object
terminal behavior.

## Scope decisions

### Specified for M02

- one simultaneous mirrored round with teams `0` and `1`;
- exactly two typed safe anchors and two persistent runtime safe entities;
- hostile objective damage from accepted straight, lobbed/area, melee, dash, and sentry delivery;
- sentry targeting of hostile fighters first and the enemy safe only as an in-range fallback;
- threshold, simultaneous-destruction draw, exact-fraction timeout, and common forfeit results;
- a dedicated map validated for exact 1v1, 2v2, and 3v3 topology;
- persistent world health, local `DEFEND`/enemy `ATTACK` HUD, critical/destroyed cues, audio, and
  results;
- public replication, late join, reconnect, restart, worker recovery, practice, and requeue;
- Heist tuning through the V6 Balance Lab and bounded objective telemetry.

### Explicitly not in M02

- treasure-chest damage, opening, loot, or restoration pickup behavior;
- role swaps, multiple rounds, aggregate scoring, overtime, repair, regeneration, armor, or
  build-specific objective modifiers;
- barrel damage to safes, destructible safe debris, movable objectives, or payload mechanics;
- a generic mode trait, target callback registry, behavior scripting, arbitrary objective list, or
  a second Heist map.

## Research record

### Local product and architecture sources

The specification was prepared against the current post-V9/M01 workspace, especially:

- `docs/18-damageable-world-objects-and-heist.md` for the accepted durable safe/mode contract;
- `src/matchplay/{mod,server,wipeout,hot_zone,telemetry}.rs` for mode ownership, deadline
  precedence, post-damage evaluation, restart, and summaries;
- `src/map/{catalog,objects,runtime,server,client}.rs` and `content/maps/` for typed anchors,
  reservations, navigation validation, dynamic generation, collision, and recovery;
- `src/combat/{delivery,server,model,cues,outcomes}.rs` and `src/combat/effects/` for delivery,
  staged damage, schedule ownership, facts, and publication;
- `src/abilities/sentry.rs` for stable target ordering and deployable ownership;
- `src/protocol.rs`, `src/config.rs`, `src/server/{admission,worker}.rs`, `src/server/lobby/`,
  `src/lobby.rs`, `packages/brawler-routing/`, and `config/server/game-types.ron` for complete
  routed-mode dispatch;
- `src/client/{hud,audio,session}.rs` and `src/client/presentation_3d/` for readiness, objective
  presentation, overlays, and audio;
- `src/server/balance_lab/` for the current snapshot schema `6`, persistence schema `2`, atomic
  validation, and restart handoff;
- `references/bevy/examples/README.md` and `references/bevy/examples/app/plugin.rs` for focused
  plugin composition; and
- `references/lightyear/book/src/concepts/replication/replicate.md`,
  `advanced_replication/replication_logic.md`, and `bevy_integration/system_order.md` for registered
  component replication, entity/group consistency, receive/send boundaries, and fixed scheduling.

### Current primary sources

- [Bevy 0.19 release notes](https://bevy.org/news/bevy-0-19/) confirm the milestone's engine
  baseline.
- [Bevy `SystemSet` documentation](https://docs.rs/bevy/0.19.1/bevy/prelude/trait.SystemSet.html)
  supports explicit set configuration and ordering rather than hidden cross-plugin execution.
- [Lightyear component replication](https://cbournhonesque.github.io/lightyear/book/concepts/replication/replicate.html)
  confirms that only registered components replicate and that changed registered state is sent.
- [Lightyear replication logic](https://cbournhonesque.github.io/lightyear/book/concepts/advanced_replication/replication_logic.html)
  documents per-entity consistency and `ReplicationGroup` for multi-entity same-tick consistency.
- [Lightyear setup](https://cbournhonesque.github.io/lightyear/book/tutorial/setup.html) confirms the
  server-authoritative composition and explicit component registry used here.

No current API was needed beyond the checked-in Bevy 0.19/Lightyear 0.29 patterns. Avian remains
the existing authoritative planar collider owner; M02 adds no new physics API.

### Findings and alternatives

1. **Topology is part of the game-type identity.** `players_per_team` is exact in the lobby queue,
   reservation, and worker manifest. Therefore one variable-size Heist game type cannot truthfully
   provide 1v1/2v2/3v3. M02 adds `heist-1v1`, `heist-2v2`, and `heist-3v3`, all sharing one stable
   Heist mode identity and map. The bounded catalog rises from eight to ten entries and the
   welcome-size bound is remeasured. A variable-topology abstraction is rejected.
2. **Map-object processing cannot own safes.** M01's map runtime consumes `DamageableWorldObject`
   requests and commits `MapPlacementOutcome`. Feeding a safe into that transaction could invoke
   map terminal behavior. M02 shares stable target/health/fact vocabulary but gives mode objectives
   a marker, pending buffer, damage stage, and terminal meaning owned by `HeistModePlugin`.
3. **Root and safe entities form one result-critical snapshot.** Root completion and safe health
   must not arrive from different server ticks. The match root and two safes therefore share one
   match-scoped Lightyear `ReplicationGroup`, while exact match/map/generation readiness still
   protects initial arrival and recovery. The HUD displays `SYNCING OBJECTIVE` until the complete
   set agrees.
4. **Safes should persist after destruction.** Despawning at zero would make completed results and
   recovery indistinguishable from incomplete replication. The entity remains at zero health in a
   terminal state, loses collision once, and is restored in-place during mode reset with the new
   match identity.
5. **The safe needs its own visual language.** Reusing the Mini Dungeon chest is rejected. The
   preferred imported core is Kenney Blaster Kit `crate-wide.glb`, composed on a much larger
   team-coloured plinth/housing. It reads as a secured installation and is already compatible with
   an established Kenney material path. The primitive fallback preserves that silhouette and is
   not a cube.
6. **Sentry behavior needs an explicit objective policy.** Merely allowing a stray sentry shot to
   hit a safe would leave the ultimate unable to pressure an undefended objective. The existing
   stable hostile-fighter selection remains first priority; the enemy safe is the sole fallback.
   Sentries never autonomously target friendly safes, barrels, or future chests.

## Technical specification

### Stable identities and compatibility

| Concern | M02 value |
|---|---|
| Mode | `HEIST_MODE_DEFINITION = ModeDefinitionId(4)` |
| Map | `TWIN_VAULTS_PRESET = MapPresetId(6)`, key `twin-vaults` |
| Rules revision | `1` |
| Safe anchors | `ModeAnchorId(1)` for team `0`, `ModeAnchorId(2)` for team `1` |
| Game types | `heist-1v1`, `heist-2v2`, `heist-3v3` |
| Map recipe schema | `4` |
| Map fingerprint format | `6` |
| Operator catalog schema | `2` |
| Balance Lab snapshot/persistence | snapshot `7`, persistence `3` |
| Application protocol/content envelope | protocol `27`, content envelope `14` |

Adding `GameMode::Heist = 3` does not change the routing enum's encoded width. The routing control,
packet, and match-manifest format versions therefore remain unchanged unless implementation proves
that their byte layout changes. All `GameMode` matches become explicit; an unknown value is
rejected and no `else means Hot Zone` fallback remains.

`DamageableTargetIdentity` gains:

```rust
HeistSafe {
    match_id: MatchId,
    anchor_id: ModeAnchorId,
    defending_team: TeamId,
}
```

`DamageableTargetClass` gains `ModeObjective`. Its protocol registration changes from
`replicate_once()` to normal `replicate()` because the in-place safe receives a new `match_id` on
restart. Map-object identities remain stable and therefore incur no routine updates.

The shared world-target terminal fact becomes target-neutral, for example:

```rust
enum WorldTargetTerminalFact {
    MapPlacement(MapPlacementOutcome),
    ModeObjectiveDestroyed,
}
```

No process-local Bevy `Entity` crosses the wire.

### Authored map and validation

`MapModeAnchorKind` gains the concrete authoring shape:

```rust
HeistSafe {
    team_slot: u8,
    origin_cell: MapCell,
    width_cells: u16,
    height_cells: u16,
    quarter_turns: u8,
    objective_visual_profile_id: MapVisualProfileId,
}
```

`ResolvedMap` replaces its Hot-Zone-only optional objective with an explicit bounded resolved-mode
anchor representation capable of holding one Hot Zone area or exactly two Heist safes. This is a
closed enum/collection for today's modes, not arbitrary objective scripting.

Twin Vaults starts as a symmetric 64-by-40-cell, three-lane map. Each safe reserves a 3-by-2-cell
footprint and a 96-by-64-world-unit collider, with mirrored orientation. Each team has three spawn
markers. Four mirrored oil barrels occupy contestable central positions; they exercise M01
coexistence but are outside safe collision/access envelopes and cannot damage objectives.

Catalog resolution rejects a Heist recipe unless all of these hold:

- exactly two nonzero, unique anchor/placement identities exist with team slots `{0, 1}`;
- footprints fit with a two-cell playable-bounds inset and do not overlap placements, other
  anchors, spawn reservations, or each other;
- orientation is normalized to `0..=3` and the objective visual profile is the safe profile;
- every supported spawn reaches both a legal own-safe defence ring and enemy-safe attack ring at
  the existing fighter-radius navigation clearance;
- each safe exposes at least two cardinal attack sectors, each with at least two adjacent clear
  fighter cells, so one permanent choke cannot seal the objective;
- objective collision participates in navigation/reservation validation; removing destructible
  map objects can only increase, never create, required access; and
- no Hot Zone or unsupported anchor appears.

The exact authored geometry is adjusted during implementation only to satisfy these validators and
native pacing. Any material size/lane/cover change is recorded here before verification.

### Rules and runtime ownership

`src/matchplay/heist.rs` owns `HeistModePlugin`, `HeistRules`, `HeistState`, `HeistSafe`, objective
damage application, outcome evaluation, reset, cues, and summary collection. Common matchplay
continues to own phase, clock, fighter respawn, forfeit precedence, outcome commit, and restart.

Initial validated defaults are:

| Rule | Default |
|---|---:|
| Safe maximum health | 2,000 |
| Active match limit | 180 seconds |
| Countdown | 3 seconds |
| Respawn delay | 3 seconds |
| Critical threshold | 25% remaining |
| Teams | exactly 2 |
| Players per team | exact 1, 2, or 3 per advertised entry |

Safe health is equal at match start. `HeistRules::validate` rejects zero health, invalid topology,
critical thresholds outside `1..99`, and lifecycle-incompatible timing. The operator catalog uses
`safe_health` only for Heist and rejects mixed Wipeout/Hot Zone objective fields.

At selected-map installation the server creates exactly two public safe entities with:

- `HeistSafe`, target identity/class, `MaximumHealth`, `CurrentHealth`, and `LifeState`;
- stable anchor/team/pose/map-instance/dynamic-generation association;
- the same match-scoped `ReplicationGroup` as the match root and other safe;
- Avian static collision while live and no collider after terminal commit; and
- public replication to every admitted observer, independent of concealment.

The match root carries one replicated `HeistState` containing the match ID, rules revision,
expected map instance/dynamic generation, ordered safe anchor/team identities, and optional
completion cause. It does not duplicate current health. Startup fails closed if the selected map,
mode, anchors, rules, or safe installation disagree.

At zero health a safe remains replicated with `CurrentHealth(0)` and terminal life state. One
terminal fact/cue is published and collision is removed in the same authoritative transaction.
The safe never transitions to `MapPlacementOutcome`, explodes, opens, or drops an item.

Restart order remains:

```text
MatchRestartSet::Prepare
  -> MatchRestartSet::ModeReset
       update safe identities to the new match
       restore full health/live state/colliders
       reset HeistState and mode facts
  -> MatchRestartSet::EnvironmentReset
  -> MatchRestartSet::Commit
```

Reset uses immediate/exclusive mutation or an explicit deferred flush before common commit. Requeue
or map replacement despawns the old root/safes and installs a fresh generation; shutdown leaves no
objective entity, collider, cue, or fact residue.

### Objective damage transaction

M01 map objects keep `DamageableWorldObject` and `PendingWorldTargetDamages`. M02 adds `HeistSafe`
and a separate bounded `PendingModeObjectiveDamages`. Combat delivery discovers both explicit
target classes and routes an accepted hit to the owning buffer.

The combat composition point exposes:

```text
CombatDamageSet::Combatants
  -> CombatDamageSet::WorldTargets
  -> CombatDamageSet::ModeObjectives
  -> CombatDamageSet::EnvironmentReactions
  -> CombatDamageSet::Publish
  -> AbilitySet::ObserveOutcomes
  -> MatchSet::ModeRules
  -> MatchSet::Outcomes
  -> CombatSet::Lifecycle
```

The objective stage collects, validates, reserves capacity, sorts, and applies all safe damage
before Heist mode evaluation. Stable ordering uses simulation tick, attack/delivery/effect order,
then safe team/anchor. Capacity rejection occurs before mutation; duplicate delivery cannot commit
a second hit or terminal fact.

A safe damage request is eligible only when:

- damage is positive, the match is active, and identity matches the current Heist state;
- the authoritative source fighter/deployable is active and belongs to team `0` or `1`; and
- the source team differs from the defending team.

Friendly contact still blocks/consumes a projectile or truncates a dash according to existing
geometry, but emits only a restrained immune cue and applies no damage. Countdown, completed,
stale-match, disconnected/invalid source, zero-damage, healing, status, knockback, and world-effect
requests do not mutate safes. Safe damage grants no fighter-damage aggregate, defeat credit,
ultimate charge, passive trigger, Wipeout score, Hot Zone progress, or spawn-protection behavior.

Delivery policy is:

| Delivery | Safe behavior |
|---|---|
| Straight projectile | First blocking safe contact consumes the projectile; enemy safe receives the accepted payload. |
| Lobbed/area | Exact authoritative overlap applies damage once to each eligible enemy safe in the area. |
| Melee | Exact arc/shape overlap applies once per activation. |
| Dash ultimate | First blocking safe truncates movement and receives the accepted dash damage once. |
| Sentry projectile | Existing projectile contact can damage the enemy safe. |
| Sentry selection | Hostile fighters retain stable priority; enemy safe is the sole in-range/line-of-sight fallback. |
| Barrel explosion | Never targets or damages a safe in V10. |

Fighter-derived cues retain the V9 observer filter for their source. Public safe health and the
fact that a safe was damaged remain visible to all clients; a concealed attacker is not identified
to observers who cannot reveal that source.

### Outcome rules and precedence

Post-damage `MatchSet::ModeRules` evaluates final safe state:

```text
only team 0 safe is zero -> team 1 wins
only team 1 safe is zero -> team 0 wins
both safes are zero      -> draw
neither is zero          -> continue
```

At the active deadline, common pre-game outcome resolution retains this order:

1. common forfeit;
2. Heist deadline result;
3. no boundary-tick combat after the deadline.

Timeout compares exact fractions using widened integer cross multiplication:

```text
health_0 * maximum_1  <=>  health_1 * maximum_0
```

The greater fraction wins and equality draws. `HeistState.completion` records
`SafeDestroyed { destroyed_teams }` or `Timeout { comparison }` before common outcome commit;
forfeit remains identified by common match state. Stale or duplicate offers cannot replace the
one-slot outcome.

### Replication and client readiness

All safe identity, health, life state, pose, team, and Heist root state are registered in the one
global application protocol. Clients never send safe health, damage, cue, or result mutation.

HUD/world/results are ready only when all of these agree:

- current `MatchState` and `MatchClock`;
- one `HeistState` for that match/rules revision;
- selected map instance and current dynamic generation; and
- exactly one safe for each expected anchor/team with matching match/map identity.

Until then the objective slot reads `SYNCING OBJECTIVE`; stale health is never retained. This same
predicate gates normal join, late join, reconnect, recovery, restart, and completed results.
Destroyed safe entities persist so completed clients remain ready. Unexpected duplicates or mixed
generations are a fail-closed diagnostic, not an arbitrary first-query choice.

### HUD, world presentation, audio, and results

The top-right mode slot becomes an explicit three-mode dispatch. For Heist it shows both current/
maximum values and percentages, with local `DEFEND Tn` and opposing `ATTACK Tm` labels plus the
common timer. It remains usable at supported window sizes and with controller focus elsewhere.

The safe visual uses the promoted Kenney Blaster Kit `crate-wide.glb` as the secured core, mounted
inside a larger 3-by-2-cell plinth/housing with team-coloured bands and structural ribs. The
primitive path uses an equivalent wide vault/plinth silhouette. Both paths provide:

- always-visible compact objective health;
- unmistakable team ownership without relying on colour alone;
- semantic hit response, one critical transition at 25%, and one destroyed state;
- live collision and destroyed nonblocking meaning; and
- no lid, loot glow, opening motion, pickup burst, or treasure-chest audio.

Reduced effects may simplify flashes, particles, and debris but preserve team, health, critical,
destroyed, and collision meaning. Audio adds bounded distinct safe-hit, critical, and destroyed
cues; frequent hit sounds are rate-limited and source privacy is preserved.

The completed overlay states `SAFE DESTROYED`, `BOTH SAFES DESTROYED — DRAW`, `TIME EXPIRED`, or
the common forfeit cause and shows final health for both safes. Requeue and replay continue through
the existing common flow.

### Routing, lobby, practice, and Balance Lab

`GameMode::Heist` is added explicitly through config, routing allocation policy, supervisor CLI,
lobby parsing/advertisement, queue, worker manifest, admission, server mode installation, client
flow, diagnostics, and E2E selection. The lobby adds `AdvertisedRulesSummary::Heist {
safe_max_health, active_limit_ticks }`.

The operator catalog moves to schema `2`, adds the three exact topology entries, and raises
`MAX_GAME_TYPES` from `8` to `10`. This remains a small bounded product catalog, not an unbounded
operator surface. Parser/codec/size tests remeasure the combined V9 brawler catalog plus ten game
types against `MAX_LOBBY_WELCOME_BYTES`; any limit change requires recorded measured evidence.

The worker manifest retains its existing `objective_target: u16`; for Heist it carries validated
safe maximum health. Admission verifies mode `4`, map `6`, admission revision, lifecycle timings,
safe health, topology, content fingerprint, and both resolved anchors before startup.

Balance Lab evolves to snapshot schema `7` and persistence schema `3` with:

```rust
HeistTuning { safe_max_health: u16 }
```

The baseline is the canonical Heist default. Atomic validation accepts `100..=20_000`, applies it
to a Heist practice match at the existing restart transaction, restores it through the current
rollback path, and rejects it for non-Heist ownership. Match duration/countdown/respawn remain
operator game-type values already selectable through the practice entry; M02 does not create a
second timing owner in Balance Lab.

### Telemetry and boundedness

`ModeSummary` gains `Heist(HeistSummary)`. The bounded summary records:

- final/maximum health for both safes and completion cause;
- objective damage by attacking team and delivery/source family;
- participant objective-damage totals;
- first-damage tick for each safe;
- destroying source when visible/valid, simultaneous destruction, and timeout cross-product
  margin; and
- rejected friendly/stale/non-active requests and capacity faults as aggregates.

Per-hit records remain in the dedicated bounded world-target fact stream; process summaries carry
aggregates, not an unbounded event history. Initial ceilings are two safe entities, two terminal
facts per match, at most 64 pending objective hits in one tick, and the existing bounded cue/fact
queues. Implementation measures rather than assumes the final byte/entity/collider budgets.

## Implementation checklist

Implementation and automated/native verification completed on 2026-08-25. The delivered slice
includes stable application and routed identities, Twin Vaults validation, two persistent public
safes, all approved objective-damage sources, exact threshold/timeout/draw resolution, critical and
terminal cues/audio, generation-safe HUD/readiness, restart, bounded telemetry, Balance Lab tuning,
and exact routed 1v1/2v2/3v3 admission. It is now awaiting the required user playtest and feedback
triage before closeout.

- [x] Add Heist IDs, rules/state/summary/cues, explicit mode plugin composition, and pure rule tests.
- [x] Extend target identity/class and terminal fact without routing safes through map-object
  behavior.
- [x] Add Heist-safe anchor authoring/resolution, mode-specific navigation/access validation, Twin
  Vaults content, index/fingerprints, and negative catalog tests.
- [x] Install two public authoritative safe entities and implement objective damage, collision,
  exact-once terminal state, sentry fallback, and ordered mode evaluation.
- [x] Implement reset, map replacement, teardown, admission rejection during an active match, and
  generation-safe reconnect/recovery convergence through the existing durable replication path.
- [x] Add protocol registration and advance the global compatibility/content versions once.
- [x] Add explicit Heist routing/config/supervisor/lobby/queue/manifest/admission/worker branches and
  the three advertised exact topologies.
- [x] Add HUD, world health, imported/primitive safe visual, source-filtered cues, audio, completed
  overlay, diagnostics, and report summary.
- [x] Evolve Balance Lab, validation, persistence, practice handoff, and UI for safe health.
- [x] Run focused, full, separate-App, routed, capacity, native, and canonical verification; record
  exact evidence below.
- [ ] Deliver a user playtest handoff and triage every feedback item before closeout.

## Verification plan

### Pure and focused ECS

- validate exact anchor teams, identity, footprint/reservations, attack sectors, spawn-to-attack/
  defence reachability, and invalid mode/map combinations;
- test positive/zero, enemy/friendly, active/non-active, current/stale identity, source validity,
  duplicate delivery, stable ordering, capacity reservation, and exact-once terminal behavior;
- cover straight, lobbed/area, melee, dash, sentry contact/fallback, and explicit barrel immunity;
- prove one-safe win, both-safe same-tick draw, widened exact-fraction timeout, forfeit precedence,
  deadline boundary, and duplicate outcome rejection;
- prove mode reset restores identity/health/collider before commit and teardown leaves no residue;
- assert safe damage does not enter fighter damage, charge, passive, defeat, Wipeout, or Hot Zone
  readers; and
- add schedule assertions for objective damage before ability observation/mode rules/outcomes.

### Separate-App and network impairment

- both clients converge on partial health, terminal state, collision, cues, and results;
- late join/reconnect/recovery converge from live and destroyed objective states;
- missing, duplicate, or mismatched root/safe/map generations display `SYNCING OBJECTIVE`;
- forged client health/damage/result mutation has no authority;
- concealed source identity remains observer-specific while public safe state agrees; and
- delay, loss, duplication, jitter, restart, map replacement, and repeated lifecycle leave no stale
  health, collider, cue, entity, or result.

### Routed product

- run threshold, timeout, simultaneous draw, forfeit, restart, replay/requeue, and fresh-lobby
  requeue across representative `heist-1v1`, `heist-2v2`, and `heist-3v3` scenarios;
- verify practice and Balance Lab apply/restore/persistence/rejection;
- run concurrent heterogeneous Wipeout, Hot Zone, and Heist workers;
- verify unknown mode, wrong map, wrong admission revision, wrong safe health, bad topology, and
  incompatible protocol/content fail closed; and
- remeasure lobby welcome, worker manifest, recovery snapshot, objective facts/cues, and network
  bandwidth within declared bounds.

### Performance and native checks

- measure fixed-tick p50/p95/p99/max, server/client entities, colliders, visuals, memory, bandwidth,
  and repeated restart/reconnect growth on Twin Vaults at 3v3 with both safes, four barrels, and a
  worst-case same-tick objective-damage burst;
- run normal imported, forced primitive, reduced-effects, keyboard/mouse, and controller paths;
- assess safe/chest distinction, ownership without colour alone, health/critical/destroyed
  readability, collision change, audio density, and source privacy; and
- playtest lane travel, turtling, spawn pressure, comeback potential, timeout rate, topology pacing,
  build variety, and whether sentry fallback is useful without becoming dominant.

Canonical commands come from the root `justfile` and `README.md`; exact commands and artifacts are
recorded during `Verifying` rather than invented in advance. Visual evidence complements but does
not replace authority, recovery, exact-once, and non-mutation tests.

### Implementation-phase evidence — 2026-08-25

- `cargo check --all-targets --features server,client,balance-lab,network-test` passed.
- Heist pure destruction, simultaneous draw, exact-fraction timeout, and rules-validation tests
  passed.
- Existing focused map-catalog tests passed after the recipe schema/fingerprint advance; the new
  Twin Vaults anchor regression and client safe-visual catalog regression also passed.
- The checked-in ten-entry operator catalog and its updated canonical revision regression passed.
- Balance Lab web `npm run typecheck --prefix tools/balance-lab-web` passed.
- The focused sentry-objective fallback regression passed, covering deterministic safe selection,
  visibility/range rejection, and preservation of fighter-first priority.
- `cargo check --workspace --all-targets --all-features` reaches the pre-existing experimental
  `owner-prediction` test incompatibility (`resolve_static_arena`/legacy snapshot geometry); the
  production server/client/balance-lab/network-test feature graph passes and M02 did not modify
  that experimental test path.

### Verification evidence — 2026-08-25

- `just check` passed every independently buildable routing, client, server, network-test, Balance
  Lab, and web role.
- `just lint` passed formatting, all four Clippy graphs, the dedicated-server feature isolation
  check, sole-world-renderer check, and V8 map-cleanup check. The Heist mode label was extracted
  from the supervisor entry point after the added branch crossed the 100-line Clippy threshold.
- The canonical `just test` stages passed routing (83 library plus 17 binary/process/integration tests),
  client (383), server (294), and Balance Lab (304). Its first network pass found that the new
  test-only cue capture assumed every synthetic client owned the capture resource; the capture was
  made optional and the complete network suite then passed `86 passed; 0 failed`.
- The four focused Heist separate-App scenarios passed: invalid/friendly/barrel immunity, partial
  and critical health convergence, terminal result/cues, exact restart, same-tick draw,
  exact-fraction timeout, bounded/deduplicated objective ingress, and actual sentry fallback fire.
- `v10_m02_heist_objective_burst_stays_within_fixed_tick_budget` passed at supported 3v3 content
  with 65 objective requests per tick; the final measured p95 was `582.500µs` against the `16.667ms` fixed
  tick and every over-cap request was counted.
- `BRAWLER_PRODUCT_GAME_TYPE=heist-1v1 just e2e 2`, `heist-2v2 just e2e 4`, and
  `heist-3v3 just e2e 6` each admitted the exact roster and reached authoritative `Active` through
  the routed supervisor/lobby/match-worker topology.
- Native imported evidence passed at 2560x1440 Metal with 1,800 samples, p95 `16.936ms`, no frame
  over 100ms, two fighters, map recipe `6`, and mode `4`; artifact:
  `target/v10-m02-heist-render.txt` plus its peer report.
- Forced-primitive native evidence passed with 1,801 samples, p95 `16.973ms`, no frame over 50ms,
  and the same map/mode identity; artifact: `target/v10-m02-heist-primitive-render.txt` plus its
  peer report.

### Implementation variance

The approved specification named a public Lightyear `ReplicationGroup` API for the match root and
both safes. The checked-in Lightyear 0.29 dependency does not expose that public component. M02
therefore uses ordinary reliable per-entity replication and an explicit match/map/generation
readiness barrier across the root and exactly two safes. The HUD and safe presentation remain on
`SYNCING OBJECTIVE` until that complete set agrees, preserving the intended player-facing atomicity
without inventing an unavailable API.

## Playtest handoff

Run `just run 2`, choose **Heist 1v1** in both windows, select a brawler, join the same queue, and
ready both fighters. Match controls are WASD/mouse, left click for primary fire, `E` for ultimate,
and `Tab` for the scoreboard; controller uses the two sticks, right trigger, right bumper, and
Select.

Please exercise both lanes and both objectives, damage a safe past 25%, destroy one safe, and ready
for one restart. If practical, try a Sentry build near an undefended enemy safe. Report:

1. whether each safe reads as a structural objective rather than the treasure chest reserved for
   M03, including without relying only on colour;
2. whether `DEFEND`/`ATTACK`, health, critical, destroyed, result, and restart feedback are clear;
3. whether lanes, safe collision, travel time, spawn pressure, turtling, and comeback potential feel
   reasonable; and
4. whether ordinary-hit, critical, and destroyed audio/effects are useful without becoming noisy.

Known limitation: this is one simultaneous mirrored round on one map; role swaps, overtime,
repair/regeneration, barrel-to-safe damage, and safe debris are intentionally outside M02.

## Exit criteria

M02 may enter `Complete` only when:

1. the user has approved this specification and relevant M01 foundation feedback is resolved;
2. all checklist items and automated/routed/native verification pass with recorded evidence;
3. Twin Vaults is admitted and playable through practice plus routed 1v1/2v2/3v3 product paths;
4. authority, eligibility, threshold/timeout/draw/forfeit, restart, recovery, concealment privacy,
   boundedness, and Wipeout/Hot Zone regressions pass;
5. the imported and primitive safes are clearly structural objectives and never resemble the M03
   treasure chest or imply loot;
6. Balance Lab exposes and safely persists the owned safe-health tuning;
7. performance/capacity evidence passes at maximum supported 3v3 content;
8. user playtest feedback is implemented, deferred, rejected with rationale, or marked as needing
   more evidence and affected checks are rerun; and
9. the learn-from-errors review, roadmap, durable docs, commands, and closeout evidence are current.

## Deferred from M02

- treasure-chest damage, opening, and restoration pickup behavior;
- attacker/defender role swaps, multi-round aggregation, overtime, repair, and safe regeneration;
- barrel damage to Heist safes;
- additional Heist maps, movable objectives, payloads, or arbitrary objective scripting.

## Feedback and closeout learning

User feedback is pending. Implementation review found three reusable lessons:

1. A conceptual dependency feature is not an API guarantee. Verify the exact checked-in crate API
   before encoding a named component into the specification; M02's `ReplicationGroup` assumption
   was replaced by the tested readiness barrier.
2. Every new mode evaluator must be explicitly ordered after `prepare_mode_rule_facts`; sharing
   `MatchSet::ModeRules` alone is insufficient because unordered systems can clear a newly offered
   outcome. The Heist evaluator now mirrors the established Wipeout/Hot Zone ordering.
3. Transient network messages need a scheduled test capture. Reading a Lightyear receiver after
   many convergence ticks can miss frame-scoped traffic, and a test-only capture must remain
   optional for synthetic client worlds that do not install it.

The final feedback decisions and affected reruns will be appended before `Complete`.
