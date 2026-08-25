# V10 Milestone 01 — Damageable target foundation and oil barrel

## Status

`Complete`

V10 M01 was explicitly started by the user on 2026-08-24. The user approved this specification
and production implementation began the same day.

## Player-visible outcome

One new routed Wipeout 1v1 game type uses the focused **Barrel Yard** proof map. Its public oil
barrels block movement and projectiles while live, show authoritative damage, explode exactly once
at zero health, damage nearby fighters, sentries, and other barrels, and then leave nonblocking wood
debris.
Every admitted, late-joining, reconnecting, and restarted client converges on the same health,
collision, terminal state, cue, and presentation.

This milestone does not implement a treasure chest or a Heist safe. The barrel is an ordinary
neutral map feature; a future safe remains a team-owned mode objective with no loot behavior.

## Research record

### Pinned implementation sources

Research used the checked-in implementation and exact dependency snapshots before selecting the
design:

- `Cargo.toml`: Bevy `0.19.1`, Lightyear `0.29.0`, and Avian2D `0.7.0`;
- `src/map/catalog.rs`, `runtime.rs`, `server.rs`, and `client.rs` for V8/V9 catalog validation,
  independent dynamic colliders, generation-tagged terminal state, recovery traffic, and client
  readiness;
- `src/combat/delivery.rs`, `effects/mod.rs`, `effects/application.rs`, `model.rs`, `cues.rs`,
  `outcomes.rs`, and `server.rs` for composed-payload ordering, bounded fact/cue reservation,
  first-contact geometry, target assumptions, and finalization;
- every `CombatOutcomeFacts` reader in abilities, concealment, Wipeout, Hot Zone, common match
  telemetry, and tests;
- `src/gameplay.rs`, `src/matchplay/mod.rs`, and `src/map/runtime.rs` for fixed-post and synchronous
  restart ordering;
- `src/concealment/network.rs` and `src/concealment/mod.rs` for V9 observer filtering and accepted-
  attack reveal;
- `src/server/balance_lab/` for the persisted `BalanceLabSnapshotV2` contract;
- `references/bevy/examples/app/plugin.rs` for focused plugin composition;
- `references/lightyear/book/src/concepts/replication/replicate.md` and
  `references/lightyear/examples/network_visibility/src/server.rs` for registered component
  replication, replicate-once identity, server despawn, and visibility ownership;
- the checked-in Avian collision-layer, sensor, spatial-query, and kinematic-controller examples.

The production tree already uses the required pinned Avian APIs: `SpatialQueryFilter::from_mask`,
`cast_shape_predicate`, `shape_intersections`, and circle/rectangle colliders. The local snapshots
therefore answered the implementation questions; no internet source or unpinned API is required.

### Current constraints found

- `MapDestructionBehavior` means atomic `DestroyMap` removal/replacement, not ordinary health.
- `MapDynamicState` already supplies the durable terminal placement result and recovery path, while
  partial health requires replicated runtime entities.
- `CombatOutcomeFacts` assumes fighter or deployable targets. Charge, passives, concealment,
  Wipeout, Hot Zone, and telemetry depend on that assumption.
- delivery currently treats any live `ArenaWall` as blocking, but payload target lookup recognizes
  only fighters and sentries. A barrel would otherwise absorb a projectile without taking damage.
- map reset and additional environment reset systems currently share one set without internal
  ordering; M01 must make restoration order explicit.
- client map readiness does not wait for live damageable entities matching the installed map
  generation.
- the Balance Lab currently serializes builds and weapons only.

### Alternatives rejected

- **Put object targets in `CombatOutcomeFacts`:** rejected because it silently changes all existing
  fighter/deployable consumers and would let object hits resemble fighter damage, charge, passive,
  or score facts.
- **Represent durability as `MapDestructionBehavior`:** rejected because it would let `DestroyMap`
  bypass partial health, source lineage, and exact-once reactions.
- **Store partial health in `MapDynamicState`:** rejected because that terminal-state channel is not
  the live replicated-entity contract needed by Heist objectives and world health presentation.
- **Add a generic object behavior registry or scripting layer:** rejected; M01 has one concrete
  environment behavior and M02 will prove the second target owner.
- **Modify Tidal Garden or another V9 acceptance map:** rejected so the completed concealment maps
  remain stable regression baselines.
- **Give barrels `TeamId` or a fighter-style `NetworkEntityId`:** rejected; neutral environment
  identity is derived from the map instance, generation, and placement and never masquerades as a
  combatant.

## Technical specification

### Ownership and module boundary

Add a focused `src/map/objects/` concern installed by the authoritative and client map plugins:

```text
src/map/objects/
  mod.rs         composition, system sets, narrow shared re-exports
  model.rs       stable identity, profiles, components, facts, cues, source lineage
  authority.rs   spawn/reset, primary object damage, transaction commit
  barrel.rs      bounded explosion planning and secondary damage
  telemetry.rs   bounded records and aggregates
  client.rs      convergence/readiness and health/cue presentation state
  tests.rs       pure and focused ECS tests
```

The exact file split may stay smaller if code remains cohesive. `map::objects` owns environment-
object identity, lifecycle, and barrel semantics. Combat delivery writes bounded object-damage
requests and combat authority applies secondary damage to fighters/deployables. Map runtime owns
the committed terminal placement/collider repair. Protocol registration remains in `protocol.rs`;
3D world rendering and audio remain client-owned.

M01 creates no top-level environment framework, public behavior trait, callback registry, or new
crate. M02 may extract only the demonstrated shared target mechanics needed by its safe.

### Authored content and validation

Extend the headless-safe map catalog with the independent durability axis already accepted in the
durable feature specification:

```rust
enum MapDurabilityBehavior {
    Indestructible,
    HitPoints(MapDamageProfileId),
}

struct MapDamageProfile {
    id: MapDamageProfileId,
    maximum_health: u16,
    terminal: MapObjectTerminalBehavior,
}

enum MapObjectTerminalBehavior {
    Explode {
        explosion_profile_id: EnvironmentExplosionProfileId,
        outcome: MapPlacementOutcome,
    },
}
```

Only the `Explode` variant is authored in M01; later variants are not added early. The catalog also
owns the one concrete bounded explosion profile. Durability is valid only on a `Feature` with a
combat-target collider, no concealment or interaction, and `MapDestructionBehavior::Indestructible`.
The validator rejects zero/unknown values, unsupported slots, durability plus `DestroyMap`, a
nonterminal replacement, and values above code-owned bounds.

Initial stable content reservations are:

| Item | Stable identity | Initial value |
|---|---:|---|
| Oil barrel asset | `MapAssetId(24)` | `oil-barrel`, 1x1 `Feature` |
| Barrel gameplay profile | `MapGameplayProfileId(9)` | block players, block/consume projectiles, circle radius 16 |
| Barrel visual profile | `MapVisualProfileId(37)` | Kenney Mini Dungeon `barrel.glb` plus primitive fallback |
| Terminal debris asset | `MapAssetId(25)` | nonblocking `barrel-wood-debris` replacement |
| Terminal debris visual | `MapVisualProfileId(38)` | Kenney Graveyard Kit `debris-wood.glb` plus low rubble fallback |
| Barrel damage profile | `MapDamageProfileId(1)` | 60 maximum health, explosion terminal |
| Explosion profile | `EnvironmentExplosionProfileId(1)` | 35 damage, radius 128, no falloff |
| Proof map | `MapPresetId(5)` | `barrel-yard`, recipe/admission revision 2 after accepted feedback |
| Routed game type | `barrel-yard-1v1` | revision 2, Wipeout, two teams, one player each |

Barrel Yard contains six barrels, including one deliberate two-barrel chain, one ordinary Dungeon
wall tile directly beside placement `100` for scale comparison, and enough clear space to verify
line of sight and blast radius. It retains three spawn placements per team
to satisfy the common map contract, but only the new 1v1 game type advertises it in M01. No barrel
starts within one explosion radius of a spawn.

Adding durability changes the map catalog schema and canonical fingerprint format from 4 to 5 and
changes the global protocol/content/admission fingerprint. Recipe schema remains 3 because the
sparse placement shape does not change. The new recipe and game type receive their own stable
revision; existing V9 recipes and game types do not.

### Runtime state and stable identity

Each live damageable placement has one server-owned entity:

```text
DamageableTargetIdentity::MapObject {
  map_instance_id,
  map_generation,
  placement_id,
}
DamageableTargetClass::EnvironmentObject
MapAssetId / MapDamageProfileId
DamageableMaximumHealth
CurrentHealth
DamageableLifeState::Live
Position / Rotation / RigidBody::Static / Collider
ArenaWall / DamageableWorldObject / MapInstanceMember
Replicate::to_clients(NetworkTarget::All)
```

Identity, class, asset/profile ID, maximum health, and fixed pose replicate once; current health and
life state replicate on change. No process-local `Entity` crosses the wire. The entity uses
Lightyear's entity mapping but does not consume fighter/deployable `NetworkEntityId` space.

At zero health, terminal state, map outcome, collider removal, and explosion reservation commit as
one authoritative transaction. The server then despawns the live object through normal replicated
despawn. `MapDynamicState` records `ReplacedWith(BARREL_WOOD_DEBRIS_ASSET)`, so every connected,
late, or recovering client presents the nonblocking terminal debris without reconstructing an
intact mesh or collider. Duplicate hits, repeated fact observation, recovery requests, and cue
delivery cannot trigger a second explosion.

### Bounded object fact and request contracts

`WorldTargetDamageFacts` is a one-tick, server-owned bounded resource separate from
`CombatOutcomeFacts`. Each fact records tick/event/attack identity, stable target identity/class,
immediate source kind, optional initiating player/team/fighter lineage, requested/applied damage,
health after, and optional terminal transition. Environment telemetry, recovery evidence, terminal
behavior, and presentation facts may consume it; fighter-only ability and mode consumers do not.

Combat deliveries enqueue server-internal `PendingWorldTargetDamage` values. Only positive authored
`Damage` effects are retained. Status, slow, knockback, healing, spawn protection, charge,
passives, weapon-contact telemetry, and fighter-defeat semantics are not evaluated for the object.
The match must be active and identity/generation/source ownership must still be valid when the
request is planned.

Initial hard ceilings are:

| Budget | Ceiling |
|---|---:|
| Live damageable environment objects per map | 32 |
| Terminal barrel reactions per tick | 16 |
| Secondary targets selected by one explosion | 16 |
| Secondary damage applications per tick | 128 |
| Pending world-target facts | 256 |
| Pending world-object cues | 256 |

Planning collects and sorts the complete transaction, dry-runs capacities, and reserves combat
event/fact/cue space before mutation. A capacity fault rejects the whole uncommitted reaction; it
never leaves zero-health live collision or applies an unrecorded partial explosion. Catalog
validation prevents authored content from requiring more live objects than the runtime ceiling.

### Delivery policy

| Delivery | Oil-barrel rule |
|---|---|
| Straight projectile | The first live barrel collider is a valid blocking target; permitted damage is queued once and the projectile is consumed. |
| Lobbed/area payload | Barrels use the authoritative area geometry and line of sight. Fighters, deployables, and objects share the authored target budget and deterministic candidate order. |
| Melee | Barrels use the authoritative sector and line-of-sight test; only damage effects apply. |
| Dash ultimate | The first blocking barrel truncates the dash as today and receives one 35-damage request for that activation; knockback is ignored. |
| Sentry | A sentry does not autonomously select an object, but its accepted projectile damages a barrel on first contact. |
| Concealment/reveal abilities | Objects are never concealment subjects or Reveal Scan targets. |

When an area payload contains combatants and objects, the existing authored `max_targets` is one
shared budget. Eligible candidates sort by the stable target class/key so maps without damageable
objects preserve existing fighter order. Explosion candidates sort by squared distance, target
class, and stable identity; this makes the nearest eligible contacts win when the 16-target safety
limit is reached.

Explosion line of sight uses authoritative Avian geometry. Static and live dynamic map blockers
occlude the blast; the source barrel itself is excluded. Explosion damage affects living fighters,
live sentries, barrels, and later ordinary damageable objects of either team, including the
initiator and allies. It never damages a future Heist safe in V10. A stable, sorted same-tick queue
processes newly terminal barrels until no work remains or the declared reaction ceiling is reached.

### Environmental source attribution

Evolve `DamageSource::Environment` from an opaque `cause_id` to a stable environment cause plus an
optional server-validated initiating lineage. Add `CombatSourceKind::Environment` for secondary
damage applied to a fighter/deployable and audit every exhaustive reader. The dedicated world fact
records each immediate barrel; a chain retains the original valid initiator.

Secondary fighter/deployable damage uses combat-owned mutation helpers so `CurrentHealth`,
`Defeated`, collision, lifecycle, and existing combat outcomes remain coherent. It emits ordinary
combat outcome facts with an environmental source, but never records weapon contact, ultimate
charge, passive triggers, or weapon-specific damage/defeat credit. A hostile fighter defeat may
credit the valid initiating team for Wipeout. Self, allied, missing, disconnected, stale-match, or
pure-environment lineage grants no team defeat credit. Clients cannot author cause or lineage.

### Fixed-tick and restart ordering

Add explicit sub-sets inside `CombatSet::Damage`:

```text
CombatSet::ProjectileSweep
  -> CombatDamageSet::Combatants
  -> CombatDamageSet::WorldTargets
  -> CombatDamageSet::EnvironmentReactions
  -> CombatDamageSet::Publish
  -> AbilitySet::ObserveOutcomes
  -> MapRuntimeSet::ApplyDestruction
  -> MatchSet::ModeRules
  -> MatchSet::Outcomes
  -> CombatSet::Lifecycle
  -> ConcealmentSet::ResolveSources
  -> ConcealmentSet::DecideObservers
  -> world/combat cue send
  -> CombatSet::Finalize
```

The sub-set names may be refined, but their order remains visible at composition. Object primary
damage, all bounded chains, secondary fighter defeat facts, and terminal collision repair complete
before ability observation and mode rules. The existing V8 `DestroyMap` pass remains separate and
later; durability profiles reject it.

Refine environment restart into an ordered chain:

```text
MatchRestartSet::Prepare
  -> MatchRestartSet::ModeReset
  -> MapEnvironmentResetSet::RestoreMapGeneration
  -> MapEnvironmentResetSet::RestoreDamageableObjects
  -> MatchRestartSet::Commit
```

Map reset clears terminal placements and advances generation first. Object reset then removes old
entities, queues, facts, cues, and lineage caches and creates full-health objects keyed to the new
generation. Map replacement and shutdown remove every `MapInstanceMember`. Inactive/countdown
barrels keep collision but reject health mutation.

### Replication, recovery, and client readiness

Register the new components and cues in `protocol.rs` and advance the single global protocol
version from 25 to 26. Use the existing reliable combat channel for bounded object hit/explosion
cues; do not add a channel or per-message version. Durable health/life state comes from component
replication, while terminal convergence remains anchored in `MapDynamicState`.

Add `ClientWorldObjectReadiness` to `ClientPlayableGate`. For the installed map generation, every
expected live damageable placement must have a matching replicated identity and every terminal
placement must be absent. During map/object arrival mismatch the client shows syncing and cannot
submit gameplay input. Late join after partial damage receives the current health; late join after
destruction receives the terminal map state without a live object. Recovery traffic is idempotent
and bounded by the existing map generation contract.

Object identity, resulting health, terminal state, and a purely environmental blast are public.
Any object cue containing an initiating fighter, team, weapon, or deployable owner retains that
fighter as `source_subject` and passes through V9's per-connection `ObserverVisibilityCache`.
Accepted attacks already reveal their source in the same authoritative tick; M01 introduces no
object-contact reveal exception and never copies a concealed subject into an unfiltered field.

### Presentation and audio

The normal 3D presentation uses the Kenney Mini Dungeon barrel, damaged-state response, a projected
character-style health bar only while `0 < current < maximum`, Kenney Graveyard Kit wood debris at
terminal state, and one bounded flash/ring showing the authoritative 128-unit blast radius. The
primitive override uses a simple cylinder and low rubble fallback with the same footprint, health,
and terminal meaning. Reduced effects retain blast radius, health response, and terminal debris.

The explosion may reuse the existing defeat audio handle
at a distinct bounded pitch/volume if native review finds it readable. Audio and effects are cue
consumers and cannot delay health, collision repair, despawn, or reset.

### Balance Lab and telemetry

Evolve the persisted Balance Lab snapshot to V3 and add the exact barrel health and explosion
numeric fields: maximum health, radius, damage, and target/reaction ceilings where safely
adjustable. Structural identities, terminal outcome, eligible target classes, same-tick chaining,
and source policy are not runtime-editable. Validation rebuilds the map catalog/fingerprints and
rejects an incompatible or out-of-bounds candidate atomically.

Bounded telemetry records primary object damage, terminal reactions, secondary applications,
chains, rejected/stale/capacity requests, cue/fact counts, live entities/colliders, replication and
recovery bytes, and fixed-tick cost. It remains observation only and cannot become a mutation path.

## Implementation checklist

Specification approved; implementation and automated verification are complete.

- [x] Add durability/damage/explosion catalog types, validation, schema/fingerprint changes, and
      exact catalog tests.
- [x] Add oil-barrel asset/profile/visual definitions, Barrel Yard recipe/index entry, routed 1v1
      game type, admission identity, and affected golden tests.
- [x] Add shared stable object identity/state/source/fact/cue types and protocol registrations;
      advance protocol version once.
- [x] Implement authoritative spawn, collider, replicated state, teardown, ordered reset, and
      terminal `MapDynamicState` commit.
- [x] Extend straight, lobbed/area, melee, dash, and sentry-projectile delivery according to the
      explicit policy and shared target budget.
- [x] Implement bounded primary object damage, capacity reservation, same-tick explosion chain,
      line of sight, exact-once commit, and secondary combatant damage.
- [x] Evolve environmental source lineage, audit every combat-outcome/source reader, and preserve
      Wipeout/Hot Zone/ability/concealment semantics.
- [x] Add observer-filtered object cues, client convergence/readiness, health/terminal/blast
      presentation, primitive fallback, reduced-effects behavior, and audio.
- [x] Evolve Balance Lab snapshot/persistence/UI and add bounded telemetry/evidence.
- [x] Update affected durable architecture, map, environment, UX, network, content, README,
      backlog, and source-layout documentation after behavior is verified.

## Verification plan

### Automated

- Pure/catalog tests: durability/`DestroyMap` exclusion, slot/collider/concealment constraints,
  unknown/zero/bounds rejection, stable identity/generation, source credit, chain ordering, target
  eligibility, line of sight, shared target limits, and exact-once terminal planning.
- Focused `App`/`World` fixed-schedule tests: each delivery policy, status rejection, inactive
  rejection, primary/secondary ordering, self/allied/hostile/environment lineage, capacity
  rollback, collider/terminal atomicity, restart generation, teardown, and fact finalization.
- Separate-App network tests: two-client partial-health convergence, terminal/despawn convergence,
  late join before/after destruction, reconnect/recovery, restart, map replacement, stale
  generation rejection, forged client mutation non-authority, impaired/duplicate cue handling, and
  concealed-attacker source privacy.
- Routed tests: Barrel Yard 1v1 selection/practice/multiplayer, Wipeout scoring after valid hostile
  environmental defeat, existing Wipeout/Hot Zone regression, restart, fresh-lobby requeue, and
  heterogeneous existing game types.
- Capacity/performance tests at 32 objects and declared chain/secondary ceilings, including
  p50/p95/p99/max fixed-tick cost, entity/collider/fact/cue/message counts, recovery bytes, and
  repeated restart/requeue cleanup.

Run the affected canonical commands from `justfile` and the root README, including formatting,
role-specific checks/lints, focused tests, the full suite, routed `just e2e` coverage, and closeout
report validation. No wall-clock sleeps are used in fixed-time tests.

### Automated evidence — 2026-08-24

- `just check`: passed every independently buildable role.
- `just lint`: passed formatting, Balance Lab web build, role-specific Clippy, dedicated-server
  feature isolation, sole-world-renderer, and V8 map-cleanup guards.
- Client, server, Balance Lab, and routing suites passed; the Balance Lab authoritative catalog
  handoff test also passed after the V3 snapshot evolution.
- `cargo test --locked --no-default-features --features network-test --test network --
  --test-threads=1`: **82 passed**, including Barrel Yard partial-health/terminal convergence,
  recovery/map/mode regressions, impairment cases, and both long restart soaks.
- `cargo test --locked --no-default-features --features network-test --test performance --
  --nocapture`: **11 passed**; the combined combat workload measured 13.02 ms p95 on the acceptance
  machine.
- `BRAWLER_PRODUCT_GAME_TYPE=barrel-yard-1v1 just e2e 2`: passed the real-process routed product
  path; one exact 1v1 roster reached authoritative `Active` and shut down cleanly.
- After the accepted Kenney-barrel feedback,
  `BRAWLER_RENDER_GAME_TYPE=barrel-yard-1v1 just v3-render-evidence
  target/v10-m01-kenney-barrel-render-rerun.txt` passed native imported-world rendering with six
  dynamic barrel visuals, no lifecycle residue, 17.03 ms p95, and 17.40 ms p99.
- After the accepted comparison-wall/debris/health-bar feedback, focused catalog, asset-manifest,
  client projection, server terminal/restart, and two-client network-convergence tests passed.
  Role-specific client/server Clippy passed with warnings denied. After advancing the map,
  admission, and game-type revisions to `2`,
  `BRAWLER_RENDER_GAME_TYPE=barrel-yard-1v1 just v3-render-evidence
  target/v10-m01-debris-health-r2-retry-render.txt` passed both clients, including preload and
  validation of the new imported debris scene, at 17.44 ms and 17.55 ms p95 with no terminal
  lifecycle residue.
  `BRAWLER_FORCE_PRIMITIVE_WORLD=1` passed the same routed render gate at 17.33 ms and 17.17 ms p95.

Native presentation, input feel, and human readability passed the user playtest and accepted
feedback cycle below.

### Native and visual

Use `just run 2` on Barrel Yard and verify with keyboard/mouse and controller:

- barrel/chest/safe identities cannot be confused; only the barrel exists in M01;
- all accepted delivery policies give prompt, readable health/contact feedback;
- the 60-health pacing, 35-damage/128-radius blast, self/allied damage, line-of-sight blocking, and
  a two-barrel chain are understandable;
- explosion happens once, removes collision immediately, and leaves no invisible blocker;
- restart restores all barrels at full health without stale effects;
- normal, forced primitive, and reduced-effects presentations retain the same gameplay meaning;
- existing concealment, combat HUD, Wipeout results, audio, and Dashboard flow remain legible.

## Playtest handoff

Run `just run 2`, choose **Barrel Yard 1v1** in both Player Dashboards, and select **Play**. Use the
normal movement, aim, and primary-fire controls shown in the Dashboard/pause overlay. In one short
match:

1. compare placement `100` with the adjacent Dungeon wall and confirm the barrel scale reads
   correctly;
2. confirm no object health bar is visible while a barrel is at full health;
3. damage a lone barrel and confirm a character-style floating health bar appears and updates on
   both clients;
4. destroy placement 101 or 102 near the center and confirm the paired barrel takes one chain hit;
5. confirm the destroyed barrel becomes Kenney wood debris, its health bar disappears, and the
   location is nonblocking;
6. stand inside and then behind cover from a blast to compare self/allied damage and line of sight;
   and
7. restart the match and confirm every barrel returns at full health with no health bar or stale
   blast/debris effect.

Please report barrel recognition, health pacing, blast radius/readability, chain clarity, map
pressure, audio, and keyboard/controller feel. Also check
`BRAWLER_FORCE_PRIMITIVE_WORLD=1 just run 2` and the existing reduced-effects setting if practical.
M01 intentionally has no chest, loot, Heist safe, autonomous sentry object targeting, movable
barrels, debris collision, or persistent fire field.

## Exit criteria

M01 reaches `Complete` only when:

1. every checklist item and automated/native gate above passes with recorded evidence;
2. barrel state, source attribution, explosion, collision, recovery, reset, and capacity are
   server-authoritative and exact-once;
3. existing fighter/deployable facts, abilities, concealment, Wipeout, Hot Zone, and V9 map
   baselines remain correct;
4. the user completes the playtest and every feedback item is implemented, deferred, rejected with
   rationale, or marked as awaiting evidence;
5. affected verification is rerun after accepted feedback; and
6. documentation reconciliation and the learn-from-errors review are complete.

## Deferred from M01

- Heist mode, safes, objective HUD/results, and barrel-to-safe damage;
- treasure chests, pickups, healing, loot, inventory, and persistent rewards;
- autonomous sentry targeting of objects;
- movable barrels, impulses, debris collision, fire fields, damage over time, repair, and arbitrary
  object behaviors;
- additional barrel maps or final release balance.

## Feedback and closeout learning

Implementation correction: the initial specification proposed a radius-20 collider for a 1x1
feature, but the existing map validator correctly requires the circle to fit its 32-unit cell.
Implementation retains the intended 1x1 barrel and uses radius 16 rather than weakening the shared
footprint invariant.

Playtest feedback accepted on 2026-08-25: the generated barrel did not match the established Kenney
environment language. M01 now promotes and uses the Kenney Mini Dungeon `barrel.glb`; the generated
cylinder remains only the forced-primitive or failed-load fallback. The pack's `chest.glb` remains
reserved for M03's treasure chest and is not reused as a Heist safe. Subsequent feedback and the
completed closeout learning are recorded below.

Additional playtest feedback accepted on 2026-08-25:

- add one ordinary wall tile beside a Barrel Yard barrel as an immediate size reference;
- replace a destroyed oil barrel with Kenney Graveyard Kit `debris-wood.glb`; and
- present a character-style floating health bar only while a damageable item is below full health.

Implemented during `Feedback review`: Barrel Yard placement `90` is one Mini Dungeon wall directly
beside barrel `100`; terminal barrel state now resolves to the nonblocking `MapAssetId(25)` backed
by the promoted Graveyard Kit `debris-wood.glb`; and the old barrel-child cuboids were replaced by
a generic projected damageable-object health UI that exists only for `0 < current < maximum`.
Affected catalog, manifest, server lifecycle, client projection, network convergence, client/server
Clippy, and routed native imported-render checks pass. Later feedback below records the accepted
damaged and terminal native states.

Verification correction: the first revision-2 native rerun ended with a supervisor
`WorkerExitMismatch` before producing reports. Its client logs also exposed Bevy warning `B0004`:
an imported dynamic scene inherited visibility from a root without an explicit `Visibility`.
Adding the required root visibility removed that warning; an immediate clean routed retry passed.
The process mismatch was not attributed to the visibility warning without evidence.

Scale feedback accepted on 2026-08-25 from the adjacent-wall screenshot: the imported barrel used
scale `32` and occupied roughly half of its 1x1 tile despite the authoritative radius-16 collider.
The imported profile now uses scale `64`, and the primitive fallback radius is `16`, so both visual
paths fill the tile without changing gameplay geometry.

Affected verification passed: the exact client catalog regression and warnings-denied client
Clippy are clean, and
`BRAWLER_RENDER_GAME_TYPE=barrel-yard-1v1 just v3-render-evidence
target/v10-m01-barrel-tile-scale-render.txt` passed both imported-world clients at 17.037 ms and
17.038 ms p95 with no reported lifecycle failure. The only log warnings were the known harmless
Bevy Winit shutdown `Destroyed` events for windows already removed. M01 has returned to
`User playtest` for confirmation that the full-tile scale reads correctly beside the wall.

Terminal-presentation feedback accepted on 2026-08-25: destroyed barrels appeared not to change
to debris. Investigation found that static map materialization excluded only assets using the
older map-destruction behavior, while dynamic reconciliation also included HP durability. Each
barrel therefore received coincident static and dynamic visuals; terminal state correctly replaced
the dynamic visual, but the stale static barrel obscured it. M01 returned to `Feedback review` to
unify the classification, add a focused regression, and rerun affected native evidence.

Implemented and verified on 2026-08-25: static materialization and dynamic reconciliation now use
one HP/destroyable-runtime classification. The focused regression proves an oil barrel with an
otherwise-indestructible destruction policy is excluded from static materialization because it has
HP durability. The two-client terminal-convergence test passed after destroying a barrel and
observing `ReplacedWith(BARREL_WOOD_DEBRIS_ASSET)` on both clients. Routed imported-world evidence
at `target/v10-m01-terminal-debris-duplicate-fix-render.txt` passed at 17.189 ms and 17.172 ms p95;
map-member high water fell from 161 to 155 while all six intended dynamic visuals remained. This
exact six-entity reduction confirms removal of the six coincident static barrel roots. M01 returned
to `User playtest` for direct confirmation of the visible barrel-to-debris transition.

Debris-scale feedback accepted on 2026-08-25: the visible replacement is substantially smaller
than its 1x1 source tile. The imported Graveyard debris profile was compounded with the generated
rubble fallback's half-footprint and quarter-height root transform. M01 returned to
`Feedback review` to calibrate the imported scene directly to the tile while preserving the
generated fallback transform.

Implemented and verified on 2026-08-25: the promoted `debris-wood.glb` has an intrinsic horizontal
span of approximately 0.5 units, so its imported profile now uses scale `64` and a unit root scale,
producing an approximately 32x32-unit footprint. The primitive fallback retains its
`(0.5, 0.25, 0.5)` generated-rubble transform. Focused imported/fallback transform and catalog
regressions passed, as did warnings-denied client Clippy. The first native attempt ended before
either client wrote a report and exposed no asset or renderer failure; the immediate clean rerun
at `target/v10-m01-full-tile-debris-rerun-render.txt` passed both imported-world clients at 16.993
ms and 17.506 ms p95. M01 returned to `User playtest` for visual confirmation of debris scale.

User acceptance and closeout on 2026-08-25: the final full-tile barrel and debris presentation was
accepted. Every reported feedback item was implemented in M01; no feedback item remains awaiting
evidence, rejected, or newly deferred. All exit criteria are satisfied and M01 is `Complete`.

### Learn-from-errors review

1. **Collider size was specified before checking the map invariant.** The first radius-20 proposal
   did not fit a 32-unit cell. Cause: the visual concept was translated directly into collision
   geometry without first exercising catalog validation. Prevention: resolve footprint and run the
   existing validator before approving authored collider numbers.
2. **The first playable used a generated placeholder despite an appropriate promoted asset.**
   Cause: implementation optimized for a quick primitive proof before comparing the available
   Kenney families with the established environment language. Prevention: inspect admitted packs
   during presentation research and reserve primitives for explicit degradation when a suitable
   production asset already exists.
3. **HP barrels were materialized through both static and dynamic renderer paths.** Cause: the two
   paths independently defined “dynamic”; one knew only V8 destruction and the other also knew V10
   durability. Prevention: both paths now use one classification helper, with a regression proving
   HP durability excludes static materialization.
4. **Barrel and debris sizes were guessed from profile numbers rather than measured after the full
   transform chain.** Cause: intrinsic GLB bounds, parent transforms, and fallback transforms were
   considered separately. Prevention: inspect intrinsic bounds, calculate the effective world
   footprint after every parent/child transform, place an adjacent reference tile, and regress the
   imported and fallback paths separately.
5. **A transient routed evidence failure initially coincided with a real visibility warning.** The
   warning was fixed, but no unsupported causal claim was made about the worker exit. Prevention:
   preserve failed logs, fix only demonstrated faults, and require a clean rerun before acceptance.

These lessons are encoded in shared renderer classification, focused transform/catalog tests, and
the durable art-admission rules. They are project-specific enough that no separate Codex skill was
created during this closeout.
