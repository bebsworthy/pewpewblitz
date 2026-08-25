# V10 Milestone 03 — Treasure chest, restoration pickup, and V10 closeout

## Status

`Complete`

M03 entered research on 2026-08-25 after the user accepted and closed M02b. The user approved the
technical specification and explicitly started implementation on 2026-08-25.
Implementation and the specified automated, routed, performance, imported, and primitive evidence
completed on 2026-08-25. Two accepted feedback corrections, their affected verification, the user
rerun, documentation reconciliation, and the learn-from-errors review all passed. The user accepted
M03 on 2026-08-25; M03 and V10 are complete.

## Player-visible outcome

Every Feature Yard mode variant contains two mirrored treasure chests. Attacks damage a chest;
destroying it opens the chest, removes its collision, and drops exactly one public restoration
pickup. A living damaged fighter from either team automatically collects the pickup on contact and
recovers health. Full-health and defeated fighters cannot consume it, and an uncollected pickup
expires visibly after a fixed lifetime.

The chest, pickup, oil barrel, and Heist idol remain visually and behaviorally distinct. M03 then
closes V10 with the complete barrel/chest/pickup/Heist/Feature Yard lifecycle,
routed, capacity, native, documentation, feedback, and learning evidence.

## Scope decisions

### Specified for M03

- one authored treasure-chest damage profile and one restoration-pickup definition;
- two mirrored chest placements in all three Feature Yard recipes;
- exact-once chest terminal commitment and pickup creation;
- stable generation-derived pickup identity, public replication, collection, expiry, reset,
  recovery, and teardown;
- deterministic server-owned overlap winner and one ordered positive-health mutation stage;
- bounded pickup facts, cues, telemetry, Balance Lab tuning, presentation, and audio;
- imported, primitive, reduced-effects, routed, impairment, recovery, capacity, and native evidence;
  and
- V10-wide closeout reconciliation and learning review.

### Explicitly not in M03

- random or weighted loot, multiple pickup types, rarity, inventory, carried items, currency,
  account rewards, persistence, ownership, or team locks;
- manual interaction prompts, pickup magnetism, throwing, moving chests, chest respawn during a
  match, healing-over-time, overheal, shields, buffs, revives, safe repair, or deployable repair;
- a generic object behavior registry, reward table, interaction framework, scripting language, or
  new map representation;
- new modes, maps, topology choices, asymmetric teams, or competitive Feature Yard tuning; or
- release-readiness claims beyond the V10 feature slice.

## Research record

### Local product and architecture sources

Research inspected:

- `docs/18-damageable-world-objects-and-heist.md`, especially the chest/pickup, deterministic
  transaction, reset/recovery, presentation, telemetry, and verification contracts;
- V10 M01 for the implemented damageable-target identity, dedicated facts, terminal placement,
  collider repair, environment lineage, Balance Lab, and object-readiness path;
- V10 M02 for the ordered mode-objective stage, protocol evolution, public objective presentation,
  and synchronous restart transaction;
- V10 M02b and the three `content/maps/builtin/feature-yard-*.ron` recipes for the accepted shared-
  geometry and exact 1v1/2v2/3v3 product family;
- `src/map/objects.rs`, `runtime.rs`, `catalog.rs`, and `client.rs` for bounded object state,
  terminal commitment, map generation, recovery, reset, and presentation readiness;
- `src/combat/effects/application.rs`, `src/combat/server.rs`, and `src/combat/mod.rs` for the sole
  ordinary combat-health writer and the explicit `CombatDamageSet` chain;
- `src/matchplay/mod.rs` and `lifecycle.rs` for restart ownership and defeated/active-fighter state;
- `src/protocol.rs` for current replicated object components and the one global compatibility
  handshake;
- `src/server/balance_lab/`, `tools/balance-lab-web/`, and `docs/15-balance-lab.md` for snapshot,
  persistence, UI, apply/reset, and validation obligations; and
- the checked-in Bevy `references/bevy/examples/README.md` and `examples/app/plugin.rs` for the
  existing plugin/schedule composition pattern.

The repository already contains Kenney Mini Dungeon `chest.glb` and `potion.glb` with the same CC0
provenance used by the accepted barrel. They are suitable first imported visuals; generated
footprint-matched chest and glowing pickup primitives remain the degradation path.

### Current primary sources

- [Bevy 0.19 fixed-time documentation](https://docs.rs/bevy/0.19.0/bevy/time/struct.Fixed.html)
  confirms that fixed schedules advance in stable timestep increments and can run zero or multiple
  times per rendered frame. Pickup availability, collection, and expiry therefore use the existing
  authoritative `SimulationTick`, never wall time or presentation frames.
- [Bevy 0.19 release notes](https://bevy.org/news/bevy-0-19/) confirm the engine baseline. No new
  Bevy, Lightyear, or Avian API is necessary: M03 extends existing components, fixed schedule sets,
  Lightyear replication registration, and authoritative center-distance overlap rules.

### Alternatives considered

1. **Represent the pickup only as a cue.** Rejected. A late or reconnecting client needs durable
   current availability and position, while collection/expiry must converge independently of cue
   delivery.
2. **Encode chest behavior as another explosion with zero damage.** Rejected. Drop identity,
   lifetime, collection, healing, and telemetry are different terminal semantics; disguising them
   as an explosion would make validation and cue meaning false.
3. **Create a generic terminal-behavior callback or loot table.** Rejected. V10 has two concrete
   terminal behaviors and one fixed reward. A closed enum plus one definition is smaller and
   exhaustively validated.
4. **Use collision-start events as the pickup winner.** Rejected. Event arrival order is not the
   product tie-break contract. Complete candidate collection followed by stable network-ID sorting
   is deterministic and directly testable.
5. **Let the pickup system mutate health in ordinary `Update`.** Rejected. It could race fixed-tick
   damage/defeat and would make rendered frame rate authoritative. Collection belongs in the
   explicit fixed-post damage transaction.
6. **Place one central chest.** Rejected. One placement biases approach and under-exercises exact-
   once/capacity behavior. Two mirrored placements preserve Feature Yard's symmetric test contract
   while still using only one chest and one pickup definition.

## Technical specification

### Authored definitions and schema evolution

Extend `MapObjectTerminalBehavior` with one closed variant:

```text
DropPickup {
  pickup_definition_id,
  outcome
}
```

Add one headless-safe `RestorationPickupDefinition` containing stable ID, positive heal amount,
collection radius, lifetime ticks, and presentation profile ID. It is a direct catalog member, not
a loot table. Validation requires unique nonzero IDs, health restoration within `1..=1000`, radius
within `8..=64` world units, lifetime within `60..=3600` ticks, a known client visual profile, and a
nonblocking legal terminal replacement.

Initial canonical values are:

| Property | Value |
|---|---:|
| Chest asset | `MapAssetId(26)` |
| Chest gameplay profile | `MapGameplayProfileId(10)`, blocking circle radius `16` |
| Chest damage profile | `MapDamageProfileId(2)` |
| Pickup definition | `RestorationPickupDefinitionId(1)` |
| Chest / pickup visuals | `MapVisualProfileId(40)` / `42` |
| Chest maximum health | `80` |
| Pickup restoration | `40` health |
| Collection radius | `32` world units |
| Lifetime | `600` ticks / 10 seconds at 60 Hz |
| Chest terminal state | `Removed` plus one pickup |

The initial values are Balance-Lab-tunable within engine bounds and remain subject to playtest
feedback. Schema/version changes are explicit:

- map catalog/gameplay-profile schema `5 -> 6`;
- map fingerprint format `6 -> 7` because canonical catalog material gains pickup definitions and
  a terminal variant;
- recipe schema remains `4` because recipes still contain ordinary placements;
- gameplay content envelope `15 -> 16`;
- application protocol `27 -> 28` for new replicated components and cue variants; and
- Balance Lab snapshot `7 -> 8` and persistence envelope `3 -> 4`.

### Feature Yard placement and admission

Add chest placements `MapPlacementId(260)` at cell `(31, 15)` and `MapPlacementId(261)` at
`(32, 24)` to all three complete recipes. These cells are related by the arena's accepted 180-degree
symmetry, avoid spawns and typed Heist anchors, sit on traversable ground near contested routes, and
do not block the center of the Hot Zone. Implementation must rerun all-terminal spawn/objective
reachability rather than relying on visual inspection.

The three recipe revisions and admission revisions advance from `1` to `2`. All nine game-type
configuration revisions advance once because their admitted map fingerprints change; stable game-
type, preset, recipe, mode, and topology IDs remain unchanged. The M02b normalized-equivalence test
continues to require exact geometry equality across variants and now asserts two chest placements.

### Runtime identity and replicated state

Add these focused shared shapes in map/environment ownership:

```text
RestorationPickupIdentity {
  generation: MapDynamicGeneration,
  source_placement_id: MapPlacementId
}

RestorationPickup
RestorationPickupDefinitionId
PickupAvailableAtTick
PickupExpiresAtTick
```

Identity is derived only from map instance, map generation, and source chest placement. Since a
chest commits one terminal transition per generation, no sequence allocator is needed and duplicate
damage, repeated terminal observation, recovery, or message delivery cannot create a second
identity. A pickup is a `MapInstanceMember`, has authoritative `Position`, and replicates its stable
identity, definition, availability, and expiry to all admitted clients. It has no rigid body,
blocking collider, team, inventory identity, or client-authored interaction.

Durable availability comes from the replicated pickup entity. Spawn/collect/expire cues add
readability but are never recovery state. A late client sees either the live pickup plus the removed
map placement or only the removed placement after collection/expiry.

### Exact-once chest terminal transaction

The existing world-target transaction handles both closed terminal variants exhaustively. When a
live chest reaches zero, it atomically:

1. commits `TerminalCommitted` and its removed/nonblocking `MapPlacementOutcome`;
2. removes the chest collider and live damageable entity;
3. creates exactly one pickup at the authored placement center with `available_at_tick = tick + 1`
   and `expires_at_tick = tick + 600`;
4. records one bounded drop fact and public opening/drop cue; and
5. publishes the map mutation and replicated pickup state from the same generation.

The one-tick availability delay guarantees the pickup exists for at least one authoritative state
before collection and prevents the chest-destroying fighter from consuming it inside the same
terminal loop. If complete event, pickup, fact, cue, or entity capacity cannot be reserved, the
terminal request is rejected before health or map mutation; a chest never opens without its one
drop. Barrel secondary damage may destroy a chest and preserves the same exact-once behavior, but
the pickup itself is not damageable and cannot chain.

### Pickup eligibility, deterministic winner, and health mutation

On each active fixed tick, collect every pickup/fighter pair whose authoritative centers are within
the definition's collection radius. A fighter is eligible only when it:

- is an active `Fighter` and `ActiveCombatant` in the current matching match;
- is not `Defeated`, has `0 < CurrentHealth < resolved maximum_health`, and belongs to either team;
- matches the current map instance/generation; and
- overlaps at or after `available_at_tick` and before `expires_at_tick`.

For each pickup, sort eligible candidates by `NetworkEntityId` and select the lowest. Sort pickup
transactions by stable pickup identity before application. A fighter may collect multiple distinct
pickups in one tick only while still below maximum; each later transaction observes the health
result of the earlier stable transaction. Healing is `min(restoration, maximum - current)` and can
never overflow, overheal, revive, repair a sentry/safe/object, grant charge, trigger passives, or
alter fighter damage/defeat facts.

`apply_restoration_pickups` is the sole positive-health gameplay writer. It runs after every damage
and objective stage, so current-tick lethal damage always wins and nonlethal damage may be restored
afterward. The system mutates health and despawns the pickup in one exclusive transaction, then
emits one collection fact/cue. At the expiry boundary, collection eligibility is evaluated first;
if no candidate wins, the pickup expires. This makes the exact `expires_at_tick` behavior explicit.

### Schedule and lifecycle ownership

Use a focused `src/map/pickups.rs` (or equivalently named map-owned module) rather than growing the
already broad `runtime.rs`. Map composition registers its systems and resources without a new
crate or one-plugin-per-type architecture.

Required fixed-post order:

```text
CombatDamageSet::Combatants
  -> CombatDamageSet::WorldTargets       chest damage/drop and barrel chains
  -> CombatDamageSet::ModeObjectives     final Heist safe damage
  -> CombatDamageSet::EnvironmentReactions
       pickup collection/expiry and sole positive-health mutation
  -> CombatDamageSet::Publish
  -> AbilitySet::ObserveOutcomes
  -> MatchSet::ModeRules
  -> MatchSet::Outcomes
  -> CombatSet::Lifecycle
```

Restart preserves the accepted transaction:

```text
MatchRestartSet::Prepare
  -> MatchRestartSet::ModeReset
  -> MatchRestartSet::EnvironmentReset
       remove every pickup
       clear pickup facts/cues/telemetry epoch state
       advance map generation and restore unopened full-health chests
  -> MatchRestartSet::Commit
```

Map replacement, requeue, disconnect cleanup, worker shutdown, and process recovery remove or
reconstruct pickup state exclusively through the owning map generation. No downstream input,
physics, replication extraction, or HUD observes a new match paired with an old pickup.

### Facts, cues, telemetry, and Balance Lab

Add bounded `PickupLifecycleFacts` with `Spawned`, `Collected`, and `Expired` outcomes carrying
event ID, tick, stable pickup identity, definition, position, collector when present, requested and
applied restoration, and health after collection. Add public pickup cues on the existing reliable
combat channel. No cue contains hidden source data unless it retains the existing observer-filtered
fighter subject; collection identifies the visible collector only under V9's normal visibility
rule.

Extend bounded telemetry with chest damage/terminal counts, pickup spawn/collect/expire/capacity
counts, requested/applied/wasted healing, lifetime-to-collection, and collector team. Existing
fighter damage, defeat, Wipeout, Hot Zone, charge, passive, and Heist objective aggregates do not
consume pickup facts.

Balance Lab snapshot `8` adds a `chest` section containing the immutable profile/definition IDs and
terminal topology plus tunable chest health, restoration, collection radius, and lifetime. Apply
and restore-default rebuild the validated map catalog and start a clean Practice epoch. Structural
identity, terminal replacement, and pickup type cannot be edited. The web UI shows units and the
exact engine bounds; persistence migration accepts the previous V3 envelope and fills canonical
chest defaults.

### Client presentation and audio

Promote the vendored Kenney Mini Dungeon chest and potion GLBs into the admitted asset manifest.
The intact chest uses the chest model and damaged-only projected object health bar. Its terminal
state removes the chest presentation completely. The pickup uses the potion model with a bounded
ground glow and gentle motion; the forced-primitive path uses a footprint-matched chest and bright
capsule/diamond pickup.

Presentation requirements:

- chest silhouette is smaller and more obviously loot-bearing than the team-colored Heist idol;
- pickup availability, collection, and expiry remain visible with reduced effects;
- visual footprint matches the chest's authoritative 1x1 circle/feature collider;
- the destroyed chest disappears and the pickup is nonblocking; and
- distinct bounded open/drop, collect/heal, and expire feedback may reuse admitted audio handles at
  calibrated pitch/volume, with silence as a valid fallback.

## Implementation checklist

- [x] User approves this specification and M03 enters `Implementing`.
- [x] Extend catalog schemas, validation, canonical fingerprints, protocol, and content envelope.
- [x] Promote chest/potion assets and add mirrored placements to all Feature Yard recipes.
- [x] Implement exact-once chest drop, stable replicated pickup state, collection, expiry, reset,
  recovery, and teardown in focused map ownership.
- [x] Add the ordered positive-health mutation stage, bounded facts/cues/telemetry, and V9-safe cue
  filtering.
- [x] Evolve Balance Lab snapshot/persistence/server apply/UI and update its durable guide.
- [x] Implement imported/primitive/reduced presentation, health/heal feedback, and silence-capable audio policy.
- [x] Add focused catalog, ECS, schedule, network, impairment, recovery, lifecycle, capacity,
  regression, and presentation tests.
- [x] Run all canonical, routed nine-game-type, heterogeneous-worker, performance, and native gates.
- [x] Deliver the user playtest, triage feedback, rerun affected verification, reconcile durable
  docs, complete the learning review, and close V10.

## Verification evidence

The implementation tranche completed on 2026-08-25:

- `just check` passed every independently buildable role and the Balance Lab web build;
- `just lint` passed formatting, every Clippy role, server feature isolation, sole-renderer
  enforcement, and the V8 map-cleanup gate;
- a clean `just test` passed routing `83 + 4 + 5 + 5 + 3`, client `387`, server `300`, Balance Lab
  `310`, combined Balance Lab/network `1`, separate-App network `87`, and performance `12` tests;
- the pickup coverage includes exact-once chest termination, generation-derived identity, stable
  lowest-fighter-ID collection, full-health rejection, exact-tick collection-before-expiry,
  capped healing, reset cleanup, and two-client authoritative replication/collection;
- the routing isolation suite retained clean independent two-worker routing, stall containment, and
  crash cleanup, while every exact Wipeout, Hot Zone, and Heist 1v1/2v2/3v3 product entry formed
  its requested roster, reached authoritative `Active`, and shut down through the production
  lobby/supervisor/match-worker path;
- the initial capacity rerun correctly exposed that two new central chest colliders consumed two
  projectiles in a long-lived 200-projectile benchmark. The fixture now keeps that capacity stream
  in its intended left-side open lanes while retaining static and destructible map collision; the
  complete performance suite then passed with a `3.137 ms` p95 for that case and all other p95
  values below `3.781 ms`; and
- controlled native imported/primitive reports passed at `1280x720` for all three modes. Wipeout
  reported `231`/`221` mesh entities and `17.175`/`17.825 ms` p95, Hot Zone `247`/`229` and
  `17.113`/`17.151 ms`, and Heist `237`/`229` and `17.709`/`17.078 ms`. Each retained exactly 14
  dynamic map visuals. Hot Zone teardown emitted the existing duplicate-despawn warning in both
  profiles, but both locked reports and all worker shutdown checks passed.

The catalog and compatibility migration is now map schema `6`, canonical map fingerprint schema
`7`, gameplay content envelope `16`, network protocol `28`, Balance Lab snapshot `8`, and Balance
Lab persistence envelope `4`.

The first-feedback correction rerun passed the focused removed-terminal exact-once test, Feature
Yard capability validation, the complete 19-test map catalog suite, imported profile coverage,
primitive scale parity, two-client drop/collection convergence, Balance Lab snapshot/apply and
persistence tests, and the complete `just lint` gate. Refreshed Wipeout imported and primitive
native reports retained 14 dynamic visuals and passed at `17.499`/`17.535 ms` p95 with `231`/`221`
mesh entities. The user accepted the corrected presentation in the final M03 rerun.

The point-blank collision correction rerun passed all 10 separate-App map scenarios, all four
projectile collision scenarios, and the complete `just lint` role/isolation gate. Its new exact-
adjacency regression places a fighter tangent to a chest with a barrel behind it, then tangent to
the barrel with the chest behind it. In both directions the first object loses health, the object
behind retains its prior health, and no projectile is spawned through the blocker.

## User playtest handoff (completed)

Run `just run 2`, choose `Feature Yard Wipeout 1v1` in both clients, and ready both players. Check
this short sequence:

1. Find either of the two mirrored chests and confirm that it reads as loot, remains distinct
   from an oil barrel, and blocks movement consistently with its visible footprint.
2. Shoot it to zero health. It should disappear exactly once and leave one clearly readable glowing
   potion pickup at the chest position.
3. Walk a full-health fighter over the potion. It should remain available. Have the other player
   damage that fighter, then cross it again; the potion should disappear for both clients and
   restore health without overheal.
4. Destroy the second chest and do not collect its potion. It should expire visibly after about ten
   seconds.
5. Spot-check the same chest silhouette in `Feature Yard Hot Zone 1v1` and `Feature Yard Heist
   1v1`; in Heist, confirm the chest remains clearly different from the team idol.
6. If practical, repeat one pickup collection with Reduced Effects enabled and confirm availability
   and healing remain readable.

Please report chest/potion readability, visible collision alignment, damage pacing,
disappearance/drop,
full-health rejection, collection/healing, expiry, and whether the contested heal feels excessively
snowbally. Feature Yard fun or competitive balance is not part of this gate.

## Feedback review

The first M03 user playtest on 2026-08-25 reported that the intact chest felt too small, the chest
should disappear when destroyed, and the potion was much too small. All three items were accepted
for immediate correction: chest visual scale `48 -> 64`, terminal outcome
`ReplacedWith(MapAssetId(27)) -> Removed` with the unused opened asset/profile retired, and potion
visual scale `38 -> 72` with correspondingly enlarged glow and primitive fallback. Authority,
collection radius, chest collider, health, restoration, and lifetime remain unchanged. A focused
visual rerun was accepted during final closeout.

The second M03 gameplay review confirmed chest destruction, the restoration drop, and damaged-only
collection, then reported that firing while exactly against either a chest or barrel stopped the
projectile without damaging the foreground object or anything behind it. Accepted for immediate
correction. Root cause: the authoritative fighter-origin-to-muzzle clearance sweep intentionally
suppressed projectile creation through nearby cover but emitted only an impact cue; unlike the
ordinary projectile sweep, it did not queue the direct world-target payload. The blocked-delivery
record now retains the first hit entity and queues damage only when that entity is a live explicit
damageable target. Static cover remains cue-only, no projectile is created beyond the blocker, and
the first-contact/no-pass-through contract is unchanged. The user confirmed the corrected
point-blank chest/barrel behavior and accepted M03 on 2026-08-25. No feedback item remains open,
deferred, rejected, or awaiting evidence.

## Learn-from-errors review

1. **A pre-muzzle collision is authoritative gameplay contact.** The existing clearance sweep was
   introduced to stop projectile origins from appearing beyond nearby cover, but its blocked path
   retained only point/normal presentation data. It now retains the first hit entity and applies
   the same direct world-target eligibility as an ordinary straight impact without spawning a
   projectile through the blocker.
2. **Regression geometry must reproduce the boundary, not merely a nearby shot.** The new network
   scenario places fighter and object circles exactly tangent, aligns a second damageable object
   behind the first, and checks foreground health, background health, and projectile absence in
   both chest-first and barrel-first directions.
3. **Terminal presentation must follow the authored terminal state.** Keeping an opened-chest
   replacement after the user expected disappearance created unnecessary visual/catalog state.
   The accepted `Removed` outcome now drives both authority and presentation, and the unused opened
   asset/profile was retired instead of left as dormant content.
4. **Imported model scale requires native judgment.** Catalog validity and primitive parity did not
   establish that the first chest and potion scales were readable in play. Focused native feedback
   produced bounded scale corrections while leaving collider, collection radius, and authority
   unchanged.
5. **A broad fire system can reach Bevy's parameter budget during a focused fix.** The muzzle
   spatial query, explicit-target view, and two damage queues are grouped in one local
   `SystemParam`; this preserves the established schedule and ownership without introducing a new
   plugin or suppressing complexity warnings.

## Verification plan

### Focused authority and content

- validate known/unknown/duplicate pickup definitions, values, terminal references, nonblocking
  removal, exact mirrored placements, and unchanged Feature Yard geometry equivalence;
- prove one chest produces one generation-derived pickup under duplicate hits, simultaneous primary
  and barrel damage, repeated terminal observation, and capacity boundaries;
- prove full-health, zero-health, defeated, inactive, disconnected, stale-match, wrong-generation,
  safe, sentry, and object candidates cannot collect;
- prove lowest stable fighter ID wins regardless of query/entity insertion order and exact expiry
  checks collection before removal;
- prove healing caps at resolved build maximum and produces no charge, passive, defeat, damage,
  mode-score, objective, inventory, or persistence outcome; and
- prove the schedule and restart chains retain their accepted ordering.

### Network and lifecycle

- separate-App convergence for intact/damaged/removed chest, live pickup, collected pickup, expired
  pickup, late join, reconnect, loss/duplication/delay/jitter, recovery, and client-forgery attempts;
- restart, map replacement, practice apply/reset, requeue, fresh lobby, and shutdown leave no stale
  pickup identity, entity, collider, cue, fact, or presentation root;
- all Wipeout/Hot Zone/Heist 1v1/2v2/3v3 routed entries retain exact roster/admission and shared
  Feature Yard content identity; and
- concurrent heterogeneous workers preserve mode dispatch, manifests, reports, and isolation.

### Capacity, performance, and native

- retain `MAX_DAMAGEABLE_MAP_OBJECTS = 32`; add `MAX_LIVE_RESTORATION_PICKUPS = 16` and bounded
  per-tick pickup fact/cue/collection ceilings sufficient for every legal Feature Yard chest to
  terminate together without partial mutation;
- measure maximum 3v3 Feature Yard entities, colliders, replicated bytes, recovery bytes, facts,
  cues, fixed-tick p50/p95/p99/max, and repeated restart/reconnect/requeue growth;
- capture imported and forced-primitive evidence for Wipeout, Hot Zone, and Heist, plus reduced-
  effects evidence for the pickup lifecycle; and
- playtest chest recognition, damage pacing, opening/drop clarity, pickup contest, restoration
  feedback, expiry, safe/chest distinction, collision, keyboard/controller feel, and whether the
  feature increases snowballing. Fun or competitive Feature Yard balance is not an exit criterion.

## Exit criteria

M03 and V10 may enter `Complete` only when:

1. the user approves this specification before production implementation;
2. chest destruction, pickup identity, collection, expiry, health mutation, reset, recovery, and
   teardown are server-authoritative, deterministic, exact-once, and bounded;
3. all three Feature Yard variants retain identical validated geometry and all nine advertised
   topologies pass;
4. barrel, concealment, Wipeout, Hot Zone, Heist, fighter health, charge, passive, telemetry, and
   persistence contracts remain uncontaminated;
5. Balance Lab, protocol/content versions, imported/primitive/reduced presentation, and capacity
   evidence pass;
6. affected canonical and routed verification passes with no lifecycle growth;
7. the user playtest is accepted and every feedback item is triaged;
8. affected verification is rerun after accepted feedback; and
9. V10 roadmap/durable documentation reconciliation and the learn-from-errors review are complete.

All nine criteria were satisfied and accepted on 2026-08-25.
