# Version 10 implementation roadmap

## Purpose and scope

V10 promotes health-bearing world objects and Heist into the playable product. It delivers a
target-aware authoritative damage foundation, one explosive oil-barrel environment slice, a
simultaneous mirrored two-safe Heist mode with one dedicated map, and one treasure-chest/restoration-
pickup slice. These concrete consumers share damage receipt and stable runtime identity without
creating a universal object behavior framework or confusing a loot chest with a mode objective.

The durable capability contract is
[Damageable world objects and Heist specification](../../18-damageable-world-objects-and-heist.md).
V10 stages the work as complete player-visible slices: environment damage first, then Heist, then
one consolidated product-visible Feature Yard map family, and finally the pickup-bearing chest and
cross-feature closeout. V9 is complete; its concealment and
server-authoritative brawler-catalog contracts are preserved as dependencies rather than changed by
this prepared roadmap.

## Version status

| Field | Value |
|---|---|
| Status | Not started |
| Current milestone | M03 — Treasure chest, restoration pickup, and V10 closeout (`Not started`) |
| Entry gate | Satisfied: V9 completed and was accepted on 2026-08-24, including final playtest, feedback triage, verification, documentation reconciliation, and learning review |
| Completion gate | Oil barrels, mirrored Heist, treasure chests, and restoration pickups use one server-owned damageable-target contract; map/mode/object identity, collision, damage, terminal behavior, pickup, HUD/results, routed admission, recovery, boundedness, balance, native feedback, and learning gates all pass |

Allowed statuses are `Not started`, `Researching`, `Specification review`, `Implementing`,
`Verifying`, `User playtest`, `Feedback review`, `Complete`, and `Blocked`.

V10 M01 was explicitly started by the user on 2026-08-24. Its final-V9/pinned-reference research,
approved technical specification, production implementation, canonical automated verification,
explicit Barrel Yard routed 1v1 smoke, native feedback cycle, affected reruns, and learning review
completed with user acceptance on 2026-08-25.

V10 M02 implementation and its initial automated/native evidence completed on 2026-08-25. The user
playtest confirmed safe damage and victory at zero health, then reported a non-scrollable long game
list, an objective model that did not read as a safe, and visible geometry outside the authoritative
hit footprint. The bounded selection-screen and idol-presentation corrections plus affected
automated/imported/primitive verification passed. The user accepted the focused rerun and closed
M02 on 2026-08-25, then approved the prepared M02b specification and explicitly started its
implementation.

V10 M02b completed and was accepted on 2026-08-25 after the consolidated Feature Yard family,
exact Wipeout/Hot Zone/Heist 1v1/2v2/3v3 catalog, migrated regression coverage, obsolete-map
retirement, canonical and routed verification, imported/primitive native evidence, documentation
reconciliation, learning review, and user playtest all passed.

## Accepted product decisions

1. Health-bearing world targets are a shared authoritative combat capability, not a generic object
   behavior or scripting framework.
2. Existing V8 `DestroyMap` removal/replacement remains distinct from ordinary attack damage and
   partial health.
3. A health-bearing map placement is immune to `DestroyMap` in V10 so no world effect bypasses
   health or exact-once terminal behavior.
4. Oil barrel is the first player-visible environment-object proof. It explodes once through a
   bounded same-tick environmental-damage transaction and then becomes nonblocking.
5. The barrel damages all eligible fighter teams and ordinary damageable objects, preserves an
   optional valid initiating-player lineage through bounded chains, and does not damage Heist safes
   in V10.
6. Heist is one simultaneous mirrored round: each team attacks the opposing safe while defending
   its own. V10 has no attacker/defender role swap, round aggregation, repair, overtime, or safe
   regeneration.
7. Heist safes are typed team-owned mode anchors and runtime objectives. They are not treasure-
   chest assets, never drop loot, and own match-result meaning only through `HeistModePlugin`.
8. Destroying exactly one safe wins; destroying both on the same completed damage tick draws;
   timeout compares exact remaining-health fractions; common forfeit precedence remains.
9. Treasure chest is a neutral damageable map feature that opens once and drops exactly one public
   restoration pickup. It has no mode-objective semantics.
10. The first pickup immediately restores bounded fighter health, is available to either team,
    cannot be consumed at full health, expires, and creates no inventory or persistent reward.
11. Barrels, chests, pickups, safes, and objective health are public facts. V9 concealment still
    protects hidden fighter state and source-derived facts according to its accepted observer rule.
12. One dedicated Heist map is required for the complete mode slice. Further maps are evidence-
    driven content, not an infrastructure prerequisite.
13. Environment-object and mode-objective damage use a dedicated bounded fact stream. Existing
    fighter/deployable combat facts remain fighter-centric; only environmental damage applied to a
    fighter or deployable enters that established stream.
14. The product advertises one **Feature Yard** test-map family rather than one visible map per
    feature. Its Wipeout, Hot Zone, and Heist recipes share the same normalized geometry, expose
    exact symmetric 1v1/2v2/3v3 game types, and differ only where mode validation requires distinct
    typed anchors. Feature Yard demonstrates implemented gameplay features but makes no fun,
    competitive-balance, or release-map claim.
15. Obsolete focused maps leave the operator catalog before deletion. Regression coverage migrates
    to Feature Yard first; a non-advertised legacy recipe may remain only while it proves a named
    contract that Feature Yard or a focused fixture does not yet cover. Stable IDs are never reused.

## Milestone overview

| Milestone | Status | Player-visible deliverable | Plan |
|---|---|---|---|
| 01 | Complete | One accepted map contains damageable oil barrels: ordinary attacks damage them, zero health commits one readable bounded explosion and nonblocking terminal state, and every client/recovery path converges | Closed with accepted native presentation, affected verification, documentation reconciliation, and learning review on 2026-08-25 |
| 02 | Complete | One dedicated Heist mode and map, advertised as exact routed 1v1/2v2/3v3 game types, provide a complete simultaneous attack/defense loop with two distinct team objectives, objective damage, HUD/audio/results, restart, and recovery | Closed with accepted core loop, scrolling, collider-aligned idol presentation, affected verification, feedback triage, and learning review on 2026-08-25 |
| 02b | Complete | One product-visible Feature Yard family consolidates permanent cover, water, concealment, destructible barriers, oil barrels, spawns, and mode objectives through Wipeout/Hot Zone/Heist variants for exact 1v1/2v2/3v3 | Closed with accepted shared-map coherence, canonical/routed verification, imported/primitive native evidence, documentation reconciliation, and learning review on 2026-08-25 |
| 03 | Not started | Treasure chests open exactly once and drop one contested restoration pickup in Feature Yard; V10 then closes with full cross-feature lifecycle, capacity, routed, native, feedback, documentation, and learning evidence | Create after M02b evidence and acceptance |

## Ordering rationale

### M01 — Damageable target foundation and oil barrel

M01 proves the new combat/environment boundary with one visible object whose terminal behavior also
exercises the retained environmental-source obligation. The slice must extend delivery and damage
processing without making every `CurrentHealth` entity fighter-like, preserve V8 map destruction,
commit a stable whole-placement terminal state, and prove current health plus recovery before a
mode objective relies on the same capability.

Gate:

- one exact damageable-target identity/state contract serves live map placements without exposing
  process-local `Entity` identity on the wire;
- straight, lobbed, melee, ultimate, and deployable deliveries have an explicit accepted or rejected
  oil-barrel policy and deterministic first-contact behavior;
- only positive damage applies; status, knockback, fighter passives, charge, spawn protection, and
  fighter-only defeat semantics do not leak onto objects;
- zero health commits exactly one terminal reaction, nonblocking map outcome, bounded explosion,
  environmental source lineage, telemetry, cue, and recovery state;
- barrel chains are stable, bounded, same-tick, duplicate-safe, and incapable of partial unrecorded
  application at capacity;
- `M08-ENV-SOURCE` is resolved with explicit self/allied/hostile/missing/chain attribution and no
  client-authored lineage;
- a dedicated bounded object/objective fact stream cannot contaminate fighter damage, defeat,
  charge, passive, Wipeout, Hot Zone, or common match aggregates;
- V9 attack reveal and cue privacy remain correct when a concealed fighter attacks a public object;
  public object cues retain any fighter-derived source as a filtered cue subject;
- restart, map replacement, late join, reconnect, requeue, and shutdown leave no stale object,
  collider, terminal state, chain cache, cue, or visual;
- focused, separate-App, routed, impairment, capacity, imported/primitive, reduced-effects, and
  native playtest evidence pass before M02 production implementation begins.

### M02 — Mirrored Heist and dedicated map

M02 builds the complete new mode on the proven target contract. It introduces exactly two typed
safe anchors, one dedicated Heist map, safe runtime entities, hostile objective damage, threshold/
timeout rules, mode summary, HUD, world presentation, audio, routing, lobby/admission, Dashboard
flow, recovery, restart, and balance evidence.

Gate:

- one stable Heist mode identity and three exact topology-specific advertised game types are
  registered through routing, config, lobby catalog, queue,
  worker manifest, admission, protocol, content identity, Dashboard selection, practice, E2E, and
  closeout reporting;
- the selected map contains exactly one valid safe for teams `0` and `1`, safe feature reservations
  do not overlap ordinary assets, and every advertised spawn/topology can reach legal attack and
  defense space;
- safe entities are public, generation-tagged, replicated, independently health-bearing, immune to
  friendly and non-active damage, immune to V10 barrel explosions, and restored only by mode reset;
- the complete damage tick precedes Heist evaluation so one destroyed safe wins, simultaneous
  destruction draws, and timeout fraction/forfeit precedence is deterministic;
- HUD, world health, team ownership, critical state, threshold/timeout/draw overlays, results,
  audio, and primitive/reduced presentation never resemble chest loot behavior;
- reconnect/recovery arrival mismatches display `SYNCING OBJECTIVE` rather than stale safe state;
- objective damage has separate facts/telemetry and does not grant fighter damage, charge, passive,
  or weapon-specific defeat credit;
- routed 1v1/2v2/3v3 threshold, timeout, draw, forfeit, restart, and fresh-lobby requeue pass along
  with concurrent heterogeneous Wipeout/Hot Zone/Heist operation;
- Balance Lab, native controller/keyboard playtest, pacing, turtling, spawn pressure, build variety,
  and safe/chest readability gates pass before M02b begins.

### M02b — Feature Yard map-family consolidation

M02b consolidates accumulated proof maps before another feature adds content to them. One shared
Feature Yard geometry contains representative permanent blockers, projectile-passable water,
concealing tall grass, remove/replace destructible cover, damageable oil barrels, movement lanes,
and enough symmetric spawns for 1v1/2v2/3v3. Three independently validated recipes apply Wipeout,
Hot Zone, or Heist mode identity and only that mode's required anchors.

Gate:

- the operator catalog advertises only Feature Yard Wipeout, Hot Zone, and Heist at exact symmetric
  1v1/2v2/3v3 topology; focused Barrel Yard, Tidal Garden, Twin Vaults, Ashen Court, Crossroads, and
  First Blood entries are not player-visible;
- normalized dimensions, theme, surface, placements, filled rectangles, spawn markers, and stable
  placement identities agree across all three variants; only recipe identity/revision, compatible
  mode, and typed mode anchors may differ;
- each recipe remains a complete ordinary `MapRecipe` parsed, validated, fingerprinted, resolved,
  admitted, replicated, recovered, and presented through the existing production path; M02b adds no
  map inheritance, include language, scene format, procedural generation, or runtime mode override;
- the Wipeout variant has no objective anchor, Hot Zone has its one valid capture anchor, and Heist
  has exactly two safe anchors with legal attack/defence sectors;
- all three recipes validate safe and reachable spawns for 1v1/2v2/3v3, including after every
  destructible or damageable placement reaches its terminal state;
- focused regressions migrate before an obsolete recipe is deleted; every retained non-advertised
  recipe has one recorded unique contract and an explicit later-removal condition;
- routed 1v1/2v2/3v3 admission, practice, restart, reconnect, recovery, requeue, concurrent modes,
  imported presentation, primitive fallback, concealment privacy, destruction, barrels, Hot Zone,
  and Heist evidence pass; and
- a user playtest confirms that the family reads as one deliberate feature test yard across modes.

### M03 — Treasure chest, restoration pickup, and V10 closeout

M03 adds one exact pickup-bearing environment behavior rather than a loot framework. A chest opens
once, transitions to a nonblocking terminal placement, creates one stable public restoration
pickup, and resolves deterministic server-owned overlap, healing, collection, expiry, recovery,
and reset. Chest placements join the already accepted shared Feature Yard geometry in all compatible
mode variants. The milestone then runs the complete cross-feature closeout and reconciles durable
docs.

Gate:

- chest and pickup identities derive from the matching map generation/placement and cannot
  duplicate across damage, terminal observation, message duplication, recovery, or restart;
- the pickup restores one validated amount up to resolved maximum health, rejects full-health,
  defeated, disconnected, stale-match, and wrong-generation collectors, and chooses simultaneous
  candidates by stable fighter identity;
- one ordered health-mutation owner prevents pickup healing from racing combat damage or defeat;
- collection and expiry are exact-once, public, bounded, recoverable, and leave no inventory,
  currency, entitlement, ultimate charge, or persistent profile state;
- chest/safe silhouettes, health, terminal cues, audio, pickup glow, team ownership, and primitive/
  reduced presentation remain unmistakably different;
- barrel, chest, pickup, safe, concealment, combat, map, mode, recovery, restart, reconnect, requeue,
  shutdown, and heterogeneous-worker matrices pass together;
- declared entity, collider, secondary-damage, pickup, cue, fact, bandwidth, recovery, memory, and
  fixed-tick budgets pass at maximum supported 3v3 Feature Yard content and repeated lifecycle;
- all affected canonical commands in `justfile`/`README.md`, full tests, routed E2E, native checks,
  closeout reports, and documentation reconciliation pass;
- every feedback item is implemented, deferred, rejected with rationale, or marked as needing
  evidence, and the learn-from-errors review is complete before V10 is accepted.

## Version-wide architecture boundaries

```text
embedded map/mode/object definitions
        |
        v
resolved placement or typed Heist anchor
        |
        v
server-owned damageable runtime entity
  stable identity + target class + maximum/current health + collision
        |
        v
target-aware composed damage transaction
        |
        +--> environment terminal behavior: barrel or chest
        |       +--> V8-compatible terminal placement state
        |       +--> bounded explosion or exact-one pickup
        |
        +--> Heist safe health
                +--> mode threshold/timeout evaluation
        |
        v
durable replicated current state + bounded cues/facts
        |
        v
client world presentation, HUD, audio, results, recovery
```

The map catalog authors bounded identity and behavior selection. Combat owns delivery, target-aware
damage, and the ordered damage transaction. Environment behavior owns barrel/chest/pickup terminal
rules. Heist owns safe meaning and match outcome. Clients present replicated facts only.

No universal `GameMode` trait, object callback registry, behavior script, arbitrary effect list,
client-authored interaction, per-message protocol version, second map representation, or public
process-local entity identity enters V10.

## Schedule and lifecycle contract

The V10 technical specifications preserve or refine this fixed-post ordering explicitly:

```text
CombatSet::ProjectileSweep
  -> CombatSet::Damage
       primary payload damage
       exact-once object terminal reactions
       bounded stable secondary environment damage
       final damage/terminal/fact publication
  -> AbilitySet::ObserveOutcomes
  -> MatchSet::ModeRules
       Heist observes final safe health
  -> MatchSet::Outcomes
  -> CombatSet::Lifecycle
  -> ConcealmentSet::ResolveSources
  -> ConcealmentSet::DecideObservers
  -> CombatSet::TelemetryAndCues
  -> CombatSet::Finalize
```

If implementation requires new damage sub-sets, their ordering remains visible at the combat
composition point. A barrel explosion that defeats a fighter or chains into another barrel and a
primary hit that destroys a safe both resolve before mode rules and lifecycle. Deferred commands
cannot split the transaction into conflicting world views. Object/objective facts publish through
their dedicated bounded stream; barrel damage to fighters publishes through the existing combat
outcome stream before ability and concealment observation.

Restart remains:

```text
prepare -> mode reset -> environment reset -> commit
```

Safes reset in mode ownership. Barrels, chests, pickups, and map terminal states reset in
environment ownership. Every state is generation-tagged and bounded.

## Content and map plan

M01 used Barrel Yard to isolate oil-barrel risk and M02 used Twin Vaults to isolate Heist risk.
M02b now consolidates those accepted mechanics with the completed V8/V9 water, destructible-cover,
barrier, and terrain-concealment contracts into one product-visible Feature Yard family. Its three
mode-specific recipes share normalized geometry while retaining independent validation and stable
identities. M03 adds treasure chests to that shared geometry rather than introducing another
feature-specific product map.

Feature Yard is an integration and playtest arena, not a promise of enjoyable pacing or competitive
balance. Proper fun/balanced maps remain later product content and should be named and evaluated as
such rather than growing the test yard into a release-map claim.

The implementing milestone chooses exact map identities after inspecting V9's accepted final
content and records all new catalog/content fingerprints through the global compatibility contract.

## Verification strategy

Every milestone uses the smallest relevant layers:

- pure profile, eligibility, geometry, ordering, identity, fraction, exact-once, and balance-rule
  tests;
- focused `App`/`World` fixed-schedule, deferred-boundary, restart, and teardown tests with explicit
  simulation time;
- separate server/client App tests for durable state, non-authority, recovery, concealment privacy,
  and generation mismatch;
- routed product tests for 1v1/2v2/3v3, threshold/timeout/draw/forfeit, impairment, late join,
  reconnect, restart, fresh-lobby requeue, and heterogeneous workers;
- bounded damageable-object, chain, pickup, entity, collider, fact, cue, message, recovery-byte,
  bandwidth, fixed-tick, memory, and repeated-lifecycle evidence;
- native normal, primitive-fallback, reduced-effects, HUD, audio, controller, keyboard/mouse, and
  results playtests.

Visual evidence cannot prove authority, exact-once terminal behavior, health convergence, safe
outcomes, pickup ownership, or absence of client mutation.

## Cross-version dependency decisions

- Completed V9 concealment, attack-reveal interaction, server-advertised brawler catalog, shared
  protocol registration, content fingerprints, and client presentation are fixed dependencies.
  V10 must extend them without restoring client-authored metadata or leaking concealed subjects
  through damageable-object facts and cues.
- The lobby welcome remains bounded after adding Heist game-type rules. M02 must remeasure the
  combined server-advertised brawler catalog and game-type summary within the existing envelope;
  it may not silently raise limits or add a second compatibility path.
- Restoration pickups use resolved authoritative fighter maximum health from the saved-brawler/
  advertised-catalog path. V10 adds no full-build presets and does not deepen the legacy build
  editor or its superseded content path.
- V8's sparse map-asset catalog, typed mode anchors, whole-placement terminal state, and recovery are
  the only map foundation. V10 adds durability beside `DestroyMap`; it does not restore terrain
  regions or legacy map objects.
- V7 profile/build authority remains the source of fighter maximum health and build identity used
  by restoration pickup and objective-damage telemetry. V10 does not query inventory during combat.
- V6 Balance Lab must expose every newly balanceable barrel, chest, pickup, safe, match, or map
  value before the owning milestone closes.
- V5 Dashboard/results and V2 routed practice/multiplayer remain the product paths. Direct UDP may
  provide a named comparison only; it cannot be the sole implementation or evidence path.
- `M08-ENV-SOURCE` is resolved in V10 M01, not by adding a dormant protocol variant during V9.

## Explicitly deferred beyond V10

- Random loot, additional pickup types, temporary buffs, carried bonuses, inventory, rarity,
  currencies, ownership, purchases, crafting, or persistent rewards.
- Safe repair/healing/regeneration, safe armor or per-build multipliers, barrel damage to safes,
  asymmetric/round-based Heist, overtime, or tournament rules.
- Damageable ordinary walls/surfaces/decorations/concealment, movable props, fire fields,
  propagation, structural collapse, material simulation, repair systems, or unbounded chains.
- Teleporters, launchers, healing pads, switches, doors, generic interactions, behavior scripting,
  or user-authored executable object/mode rules.
- Concealed objects, safes, pickups, or objective health; replay, kill-cam, or spectator policy.
- Additional product-visible maps beyond the Feature Yard family until a later content milestone
  deliberately authors and playtests fun/balanced maps.
- Final release balance; V10 records accepted defaults and maintained Balance Lab surfaces.

## Initial V10 backlog

| ID | Item | Disposition |
|---|---|---|
| V10-HEIST-ROUNDS | Alternating attacker/defender rounds, role swaps, aggregate scoring, and overtime | Deferred; the accepted first mode is simultaneous mirrored Heist |
| V10-SAFE-MODIFIERS | Objective armor, damage multipliers, repairs, healing, regeneration, and build-specific safe bonuses | Deferred until plain objective damage and build balance produce evidence |
| V10-LOOT-FAMILIES | Random tables, additional pickups, temporary buffs, inventories, rarity, currency, and persistence | Deferred; M03 ships one exact restoration pickup only |
| V10-BARREL-SAFE-DAMAGE | Attribute barrel explosions into Heist-safe damage | Deferred to later balance evidence; V10 barrels cannot damage safes |
| V10-MOVABLE-PROPS | Physics-driven barrels/chests, impulses, pushing, and dynamic placement | Rejected for V10; authored objects remain fixed authoritative placements |
| V10-GENERIC-BEHAVIORS | Scripts, callbacks, arbitrary effect lists, dynamic component reflection, or universal object traits | Rejected; add bounded code-owned behavior only with a complete consumer |
| V10-MORE-MAPS | Additional feature-specific product maps | Superseded by M02b; new mechanics join Feature Yard when compatible instead of creating another visible proof map |
| V10-FUN-MAPS | Proper player-facing maps designed and playtested for fun, pacing, and competitive balance | Deferred beyond V10; keep distinct from Feature Yard's integration-test purpose |

## Preparation sources

Local, version-pinned sources inspected for V10 preparation:

- `src/map/catalog.rs`, `runtime.rs`, `server.rs`, and `client.rs`;
- `src/combat/delivery.rs`, `effects/mod.rs`, `effects/application.rs`, `model.rs`, `cues.rs`, and
  `outcomes.rs`;
- `src/concealment/mod.rs`, `network.rs`, and `field.rs` for accepted reveal, observer visibility,
  public-entity, cue-filtering, and replication-order dependencies;
- `src/profiles/catalog.rs` and `src/client/profile.rs` for the bounded server-advertised brawler
  catalog and resolved fighter maximum-health authority;
- `src/gameplay.rs`, `src/matchplay/mod.rs`, `hot_zone.rs`, `wipeout.rs`, `server.rs`, and
  `telemetry.rs`;
- `src/protocol.rs`, `src/server/admission.rs`, `src/server/lobby/catalog.rs`, and the routed mode
  mappings;
- `src/client/hud.rs`, `audio.rs`, `session.rs`, and `presentation_3d/`;
- `references/bevy/examples/app/plugin.rs`;
- `references/lightyear/examples/network_visibility/src/server.rs` plus the checked-in Lightyear
  0.29 replication material;
- [product direction](../../00-product-direction.md),
  [maps and game modes](../../04-maps-and-game-modes.md),
  [environment gameplay](../../09-environment-gameplay.md),
  [grid map assets](../../16-grid-map-asset-system.md),
  [concealment and reveal](../../17-concealment.md), and the
  [V1 retained environmental-source obligation](../v1/roadmap.md#v1-backlog).

No internet research was needed for the initial future-roadmap pass. M01 and M02 subsequently
rechecked their exact local Bevy 0.19, Lightyear 0.29, and Avian 0.7 seams and record any current
primary sources in their owning milestone documents.
