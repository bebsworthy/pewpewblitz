# Damageable world objects and Heist specification

## Purpose and authority

This document defines PewPew Blitz's server-authoritative contract for health-bearing world
objects and the first gameplay families built on it: oil barrels, treasure chests with one
pickable restoration bonus, and the mirrored two-safe Heist mode. It deliberately separates a
reusable damage target from the terminal behavior and match meaning owned by each concrete object.

The [grid map-asset specification](./16-grid-map-asset-system.md) remains authoritative for sparse
placement, footprint, collision, immutable map identity, terminal placement transitions, and map
recovery. The [weapon and ability specification](./03-weapons-and-abilities.md) owns accepted
attacks, deliveries, payloads, and combat source identity. The
[maps and game modes specification](./04-maps-and-game-modes.md) owns mode composition and map
compatibility. The [network architecture](./08-network-architecture.md) owns stable wire identity
and replication. The [V10 roadmap](./implementation/v10/roadmap.md) owns staged delivery.

V9 completed on 2026-08-24. V10 M01 was explicitly initiated and approved the same day, then its
oil-barrel vertical slice completed with user acceptance on 2026-08-25. This document remains the
accepted version-wide capability boundary.

## Player-facing outcomes

V10 adds three intentionally distinct pieces of gameplay:

1. **Oil barrel.** A neutral blocking map feature can be damaged by attacks. At zero health it
   explodes once, damages nearby eligible fighters and ordinary damageable objects, and becomes a
   clearly nonblocking terminal placement.
2. **Heist safe.** Each team has one public durable safe. Players simultaneously attack the enemy
   safe and defend their own. Destroying exactly one safe wins immediately; timeout compares the
   remaining safe-health fractions.
3. **Treasure chest.** A neutral blocking map feature can be damaged by attacks. At zero health it
   opens once, becomes a clearly nonblocking terminal placement, and drops exactly one public
   restoration pickup that either team may collect.

The treasure chest is never a Heist safe:

- a chest is an ordinary neutral map-asset placement;
- a safe is a team-owned, mode-owned objective created from a typed Heist anchor;
- a chest drops a pickup and has no match-result meaning;
- a safe never drops loot and its zero-health transition is consumed only by Heist rules;
- normal and primitive-fallback presentation must distinguish their silhouette, team ownership,
  health treatment, terminal state, and audio.

## Scope vocabulary and ownership

### Map asset

A developer-authored catalog noun placed by a sparse recipe. Oil barrels and treasure chests are
`Feature` assets with footprints, collision, visual identities, durability profiles, and bounded
terminal behaviors. Recipes select the asset and placement parameters; they do not author health,
damage formulas, explosion formulas, pickup effects, or executable callbacks.

### Damageable world target

A server-owned runtime entity that can receive accepted positive damage. It has a stable domain
identity, target class, maximum/current health, position, collision, lifecycle generation, and
damage policy. It is a shared combat capability, not a universal object or behavior framework.

The first target classes are:

```text
Fighter
Deployable
EnvironmentObject
ModeObjective
```

The existing fighter and deployable behavior remains unchanged except where a V10 specification
explicitly adds an environmental source or collision interaction.

### Terminal behavior

One code-owned response committed exactly once when an environment object's health reaches zero.
V10 implements only the concrete behavior families it ships:

```text
Break        -> remove or replace the authored placement
Explode      -> emit one bounded explosion, then remove or replace the placement
DropPickup   -> spawn one specified pickup, then remove or replace the placement
```

This is a closed enum with validated embedded profiles, not a script, callback list, arbitrary
effect graph, dynamic component-reflection scheme, or map-authored property bag. A later object
family adds a new bounded variant only with a complete authoritative and player-visible lifecycle.

### Mode objective

A runtime target whose damage eligibility, health, reset, completion, telemetry, and results are
owned by one mode. A Heist safe shares the damage-receipt mechanics but does not use an environment
object's terminal-behavior profile.

### Pickup

A public server-owned runtime entity with one stable source identity, one bounded lifetime, and one
exact-once collection outcome. V10's first pickup is an immediate restoration bonus; it is not an
inventory item, account entitlement, currency, carried objective, or persistent reward.

## Explicit non-goals

- A generic interaction or behavior framework, arbitrary scripts, data-authored effect graphs, or
  user-authored executable rules.
- Giving hit points to every wall, decoration, surface, concealing placement, or existing V8
  destructible cover cell.
- Replacing `DestroyMap`, changing accepted 32-unit whole-placement destruction, or compatibility
  loading an older map format.
- Alternating attacker/defender rounds, best-of-series scoring, asymmetric role swaps, payload
  escort, safe repair, safe healing, safe regeneration, or overtime in the first Heist mode.
- Inventory, rarity, random loot tables, currencies, ownership, purchases, crafting, persistent
  rewards, or chest-opening UI.
- Physics impulses, movable props, structural collapse, fire propagation, material simulation, or
  unbounded barrel chains.
- Autonomous sentry objective selection unless the Heist milestone explicitly proves it as a
  readable and balanced build behavior. Existing projectiles may still hit an eligible object that
  geometrically intercepts them.
- Concealed world objects, safes, pickups, or objective health. V10 objects are public facts.
- Final numeric balance. Values are authored through the Balance Lab and accepted by playtest.

## Authored content contract

### Durability is not V8 map destruction

`MapDestructionBehavior` retains its current meaning:

```text
DestroyMap world effect
  -> atomic whole-placement Remove or Replace
  -> no partial health and no ordinary-damage attribution
```

V10 adds a separate durability axis to an eligible `Feature` gameplay profile. Conceptually:

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
    Break(MapPlacementOutcome),
    Explode {
        explosion_profile_id: EnvironmentExplosionProfileId,
        outcome: MapPlacementOutcome,
    },
    DropPickup {
        pickup_definition_id: PickupDefinitionId,
        outcome: MapPlacementOutcome,
    },
}
```

The exact Rust ownership may be refined during V10 M01 research, but the semantic separation is
fixed. Health is not added to `MapDestructionBehavior`, and attack damage is not represented as a
`MapInteractionBehavior`.

In V10, a hit-point object must be indestructible to `DestroyMap`. This prevents a world effect
from bypassing health, terminal attribution, exact-once explosion/drop behavior, or Heist rules.
Supporting both mechanisms on one placement requires a later explicit policy and migration.

### Catalog validation

The shared headless-safe catalog rejects:

- zero maximum health or unknown durability, explosion, pickup, replacement, or visual IDs;
- durability on surfaces, decorations, markers, concealing features, or profiles without a combat
  target collider;
- a health-bearing placement that is also removable or replaceable through `DestroyMap`;
- a terminal replacement that is directly authorable when the V8 catalog forbids it, blocks an
  occupied feature slot, changes footprint illegally, or is itself nonterminal;
- explosion radii, damage, target counts, chain counts, or pickup lifetimes outside code-owned
  bounds;
- a pickup definition with an unsupported effect or a chest with more than one drop;
- Heist anchors in non-Heist recipes, ordinary environment durability masquerading as a Heist
  objective, or a chest asset used as a safe identity.

Catalog, recipe, content, admission, recovery, and client-visual fingerprints change through the
one global compatibility contract. V10 adds no per-message versions or compatibility decoder.

### Concrete initial assets

V10 promotes exact assets only when their full behavior exists:

| Asset | Placement | Collision while live | Damage owner | Terminal outcome |
|---|---|---|---|---|
| Oil barrel | Ordinary `Feature` placement | Blocks players; intercepts eligible projectiles | Environment-object runtime | One explosion, then nonblocking broken/removal state |
| Treasure chest | Ordinary `Feature` placement | Blocks players; intercepts eligible projectiles | Environment-object runtime | One restoration pickup, then nonblocking opened state |
| Heist safe | Typed Heist mode anchor, not an ordinary chest placement | Blocks players; intercepts eligible projectiles | Heist mode | Public destroyed objective state and mode outcome; no loot |

The implementing milestones select original names, stable IDs, exact footprints, normal/damaged/
terminal visual profiles, and primitive fallbacks. Placeholder identity cannot erase the chest/safe
distinction.

## Runtime ECS model

### Shared damageable state

The server creates one runtime entity for each live damageable environment placement and Heist
safe. Conceptually, the shared bundle contains:

```text
DamageableTargetIdentity   stable map-placement or mode-objective identity
DamageableTargetClass      EnvironmentObject or ModeObjective
DamageableMaximumHealth    immutable for this generation
CurrentHealth              authoritative integer health reused by combat
DamageableLifeState        Live or terminally committed
Position / collider        authoritative planar contact
NetworkEntityId            stable network gameplay identity when required by delivery
```

`DamageableTargetIdentity` is the recovery and reconciliation key. It contains stable values, never
a process-local Bevy `Entity`:

```text
Map object:
  map_instance_id + map_generation + placement_id

Heist safe:
  match_id + anchor_id + defending_team
```

The environment-object entity belongs to the installed map instance and is torn down with it. The
safe belongs to the match and selected map anchor and is torn down with that match/map lifecycle.
Default visibility is public to all admitted clients.

### Authoritative fact ownership

V10 does not widen the existing fighter/deployable `CombatOutcomeFacts` stream with environment-
object or mode-objective targets. Its current consumers own fighter defeat, charge, passive,
Wipeout/Hot Zone, and match-telemetry assumptions that must remain valid.

Object and objective damage instead publishes one dedicated bounded authoritative fact stream with
stable target identity, target class, source lineage, applied amount, resulting health, terminal
transition when present, match/map generation, and tick identity. Environment behavior, Heist,
object telemetry, recovery evidence, and public presentation cues consume that stream. Barrel
secondary damage applied to a fighter or deployable still emits the existing combat outcome fact
with an environmental source, so fighter lifecycle, concealment reveal, match telemetry, and
defeat attribution continue through their established owner.

M01 must audit every existing combat-outcome reader before introducing the dedicated stream and
prove that object/safe damage cannot enter fighter damage, defeat, charge, passive, Wipeout, Hot
Zone, or common match aggregates. A future unification requires an explicit migration of every
reader; shared health alone is not permission to reuse a fighter-centric fact contract.

### Replication and recovery

Current partial health is durable replicated state, not a transient cue and not a terminal-only map
mutation. Clients receive the stable identity, class, maximum/current health, life state, and
authoritative pose needed for presentation. Damage/terminal cues may enhance feedback but never
replace current-state convergence.

An environment object that reaches zero commits its V8-compatible terminal placement outcome into
the map's generation/revision state. Late and reconnecting clients therefore receive exactly one of:

- a live replicated object with current health and its normal placement;
- the placement's committed removed/replacement state with no live object;
- a bounded syncing state while matching map and object generations arrive.

A Heist safe remains a replicated objective entity in a public destroyed state through match
completion so world presentation, HUD, results, and late recovery agree. It is restored only by the
mode restart transaction.

No client sends object damage, health, terminal behavior, explosion, drop, pickup collection,
objective damage, or victory commands.

### Reset and teardown

Restart uses the existing synchronous transaction:

```text
common prepare
  -> mode reset: restore both safes and Heist mode facts
  -> environment reset: restore barrels/chests, remove pickups, reset map terminal states
  -> common commit
```

No downstream input, physics, combat, replication extraction, or presentation fact may observe a
new match ID paired with old object health, stale pickups, destroyed safes, or old map generation.
Map replacement, requeue, server shutdown, disconnect, and recovery must remove every owned runtime
entity and bounded cache.

## Combat integration

### Delivery and collision

Straight projectiles, lobbed impact payloads, melee arcs, ultimates, and deployable shots continue
to originate from accepted server-owned attacks. V10 extends geometric candidate collection and
payload planning to recognize explicit damageable target colliders.

The collision contract is:

- a damageable blocking object participates in authoritative collision before an overlapping
  static map blocker can consume the delivery without a target;
- a straight projectile that first hits an eligible blocking object commits one impact, applies
  permitted damage, and is consumed according to its existing delivery policy;
- melee and area deliveries use exact authoritative geometry, stable candidate ordering, and the
  same line-of-sight rule as their implemented fighter behavior unless the profile explicitly
  documents an exception;
- the client does not predict object contact, health, explosions, safe outcomes, or pickups;
- a terminal nonblocking placement repairs collision atomically with the committed state.

### Effect eligibility

Only positive `Damage` payload effects apply to V10 environment objects and safes. Slow,
knockback, fighter passives, spawn protection, healing, concealment, forced reveal, charge, and
other target-owned effects do not apply merely because an entity has `CurrentHealth`.

| Target | Friendly/hostile policy | Damage payload | Status/knockback | Fighter defeat/charge/passive credit |
|---|---|---|---|---|
| Neutral barrel/chest | Any valid attacking team | Allowed | Rejected | None |
| Heist safe | Only the opposing team while the matching match is active | Allowed | Rejected | Objective telemetry only |
| Destroyed object/safe | No source | Rejected | Rejected | None |

Objective-damage multipliers, per-weapon safe coefficients, safe armor classes, build bonuses, and
safe-specific critical hits are not introduced in the first Heist slice. The ordinary applied
positive damage amount is the objective damage. Balance is achieved first through safe health,
map access, match duration, respawn cadence, and existing build tradeoffs.

An accepted attack retains V9 attack-reveal behavior even when it only contacts a world object.
Object health, safe health, barrels, chests, pickups, and their public cues never become concealment
subjects. Per-observer filtering must still prevent a cue or source field from leaking a concealed
attacker before the accepted reveal transition is authoritative.

Public object cues follow an explicit subject policy. Object identity, resulting health, terminal
state, and a purely environmental explosion are public. A cue carrying an initiating fighter,
team, weapon, deployable owner, or other fighter-derived source keeps that fighter as a cue subject
and passes through V9's connection-specific visibility filter. Removing the subject or copying it
into an unfiltered object field is forbidden. Because attack acceptance already reveals the
attacker in the same authoritative transaction, V10 adds no object-contact reveal exception.

### Deterministic damage transaction

Primary payload damage, object terminal reactions, bounded secondary environmental damage, defeat
facts, map collision repair, and mode evaluation form one explicit fixed-post transaction. The
required semantic order is:

```text
collect and sort primary deliveries by stable combat identity
  -> apply primary damage once per target/effect
  -> collect newly terminal world objects by stable object identity
  -> resolve each terminal behavior exactly once
  -> apply a bounded, stably ordered secondary environment-damage queue
  -> repeat only for newly terminal objects within the declared object/chain ceiling
  -> publish final damage, defeat, terminal, map, pickup, and cue facts
  -> evaluate mode rules from the complete current-tick health snapshot
  -> common combat lifecycle and match outcome commitment
```

Implementation may add focused sub-sets inside `CombatSet::Damage`, but it must preserve the public
schedule boundary: `MatchSet::ModeRules` observes the final current-tick damage result and remains
before `CombatSet::Lifecycle`. Deferred commands cannot make a zero-health object appear live to one
reader and terminal to another.

The completed V9 ordering is also preserved: ability outcome observation runs after the entire
damage transaction; concealment source resolution and per-observer decisions run after combat
lifecycle and before cue extraction/replication send. Consequently, primary object damage and all
bounded barrel reactions finish inside `CombatSet::Damage`; they cannot be deferred into a later
environment schedule that V9 reveal, lifecycle, or mode rules would observe incompletely.

Work is bounded by validated per-map ceilings for live damageable placements, terminal reactions,
secondary targets, chain depth/count, pickups, cues, and facts. A capacity fault rejects or defers
the complete transaction safely; it never partially applies an unrecorded explosion or drop.

### Environmental source attribution

V10 is the first authoritative environmental-damage author and therefore resolves the retained
`M08-ENV-SOURCE` obligation. An explosion records both:

- the immediate stable environment-object cause; and
- an optional valid initiating fighter/team lineage inherited through bounded barrel chains.

The initial oil-barrel policy is:

- explosion damage may affect fighters, deployables, barrels, and chests in range;
- it affects all teams, including the initiator and allies;
- it does not damage a Heist safe in V10;
- hostile fighter defeat may credit the valid initiating team for ordinary mode score and summary
  attribution, but never grants weapon contact, ultimate charge, passive triggers, or weapon-
  specific damage/defeat credit;
- self, allied, missing, disconnected, stale-match, or purely environmental lineage receives no
  team defeat credit;
- a chain preserves the original valid initiator while recording each immediate barrel cause.

The stable `DamageSource::Environment` protocol shape must be evolved deliberately to express this
policy. A client-supplied initiator or cause is never accepted.

## Oil-barrel behavior

An oil barrel is neutral and publicly damageable during `MatchPhase::Active`. Before active play it
may block movement/projectiles but cannot lose health. Its health and explosion profile are
developer-authored and Balance-Lab-visible.

At zero health, the server atomically:

1. marks the barrel terminal so no later payload can damage or trigger it again;
2. records its stable cause and optional initiating lineage;
3. commits its nonblocking removed/replacement placement state and collider repair;
4. emits one bounded radial environment-damage request and one public explosion cue;
5. includes newly terminal barrels in the same bounded stable chain transaction;
6. retains no burning field, damage-over-time area, movable debris, or loot.

Every client must present a readable intact/damaged/terminal progression, exact authoritative blast
radius feedback where useful, and the same collision/terminal meaning in imported and primitive
fallback rendering. Cosmetic debris and audio are bounded and cannot delay authority.

## Treasure chest and restoration pickup

### Chest lifecycle

A treasure chest is neutral and publicly damageable during `MatchPhase::Active`. At zero health,
the server atomically marks it terminal, commits the opened/nonblocking placement state, and creates
exactly one pickup whose stable identity is derived from the map generation and source placement.
Duplicate damage, terminal observation, recovery, or message delivery cannot create a second drop.

The chest does not randomly select a reward. V10 ships one concrete pickup definition so the whole
collection loop can be balanced and verified before any loot-table abstraction exists.

### Restoration pickup

The first pickup restores a fixed positive amount of current fighter health up to that fighter's
resolved maximum. It is collected automatically by authoritative overlap and follows these rules:

- only a living, active fighter in the matching match may collect it;
- either team may collect it;
- a full-health fighter is ineligible and does not consume it;
- when multiple eligible fighters overlap on the same tick, the lowest stable fighter network ID
  wins after complete candidate collection and sorting;
- collection mutates health and removes the pickup exactly once in the same fixed transaction;
- the pickup expires at a server-authored tick if uncollected;
- defeat, disconnect, restart, map replacement, requeue, and shutdown cannot duplicate or retain it;
- collection emits public bounded presentation/telemetry facts but grants no inventory item,
  currency, account state, ultimate charge, or persistent entitlement.

The implementing milestone selects the heal amount, collision radius, lifetime, presentation, and
audio through Balance Lab and playtest. If current combat health ownership cannot accept healing
without a second writer, the milestone must add one explicit ordered health-mutation stage rather
than mutating fighter health from an `Update` presentation or overlap system.

## Heist mode

### Mode structure

The first Heist mode is one simultaneous mirrored round:

- teams `0` and `1` each own one durable safe;
- both safes are active and publicly visible for the whole active phase;
- each team attacks only the opposing safe and defends its own;
- normal fighter defeat, respawn, protection, disconnect, forfeit, restart, and match duration use
  the common authoritative lifecycle;
- there is no attacker/defender role swap, intermission, aggregate round score, repair, overtime, or
  sudden-death rule.

`HeistModePlugin` owns validated `HeistRules`, objective installation from the resolved map,
objective damage eligibility, threshold/timeout evaluation, restart, telemetry, summary, and
results. It does not own ordinary attack acceptance or duplicate map geometry.

### Typed Heist-safe anchors

`MapModeAnchorKind` gains one concrete Heist-safe anchor shape. Conceptually:

```rust
HeistSafe {
    team_slot: u8,
    origin_cell: MapCell,
    footprint_cells: MapFootprint,
    quarter_turns: u8,
    objective_visual_profile_id: MapVisualProfileId,
}
```

The exact field placement may use a bounded server-known Heist objective profile instead of placing
all constants inline. In either form, the recipe describes position, orientation, team association,
and stable anchor identity only. `HeistRules` owns maximum health and match behavior.

A Heist recipe must contain exactly two unique safe anchors, one for each team. Anchor reservations
participate in the feature-slot occupancy validator so an ordinary chest, barrel, wall, decoration,
spawn, or second safe cannot overlap the objective footprint. The safe is not represented by the
treasure-chest asset ID or terminal behavior.

Validation also proves:

- both safe footprints and attack-access envelopes fit inside playable bounds;
- each team has sufficient safe spawn/re-entry points and no spawn overlaps a safe;
- each living team can navigate from every supported spawn to legal attack space around the enemy
  safe and legal defense space around its own;
- permanent collision cannot make one safe unreachable while the other remains exposed;
- safe-to-spawn distances, lane widths, cover, and objective surroundings meet bounded
  mode-specific constraints for every advertised 1v1/2v2/3v3 topology;
- no barrel/chest terminal transition can invalidate required objective access;
- the map contains no Hot Zone anchor or unsupported mode anchor.

### Objective state and replication

Each safe is a replicated public objective entity with stable match/anchor/team identity,
maximum/current health, terminal state, authoritative pose, collision, and network identity. The
match root carries a replicated generation-tagged `HeistState` that identifies the two anchors and
their expected match/rules generation; current health has one canonical writer on the safe entities
rather than a duplicated mutable copy on the root.

HUD and presentation enter `SYNCING OBJECTIVE` unless the `MatchState`, `MatchClock`, `HeistState`,
both safe entities, and resolved map generation agree. They never display cached health from a
previous match, map, reconnect, or restart generation.

### Win and timeout rules

After the complete current-tick damage transaction:

```text
enemy safe only reaches zero     -> attacking team wins
both safes reach zero same tick  -> draw
neither reaches zero             -> continue
```

At or after the active deadline, the existing deadline phase resolves before boundary-tick combat.
Timeout compares exact remaining-health fractions by integer cross multiplication:

```text
safe 0 remaining / safe 0 maximum
vs
safe 1 remaining / safe 1 maximum
```

The team with the greater remaining fraction wins; equal fractions draw. The initial rules require
equal maximum health, but fraction comparison prevents a future validated asymmetric profile from
silently changing timeout meaning. Forfeit retains common precedence. Duplicate or stale objective
facts cannot offer a second or wrong-generation outcome.

### HUD, presentation, audio, and results

During active play the common objective HUD displays both team-colored safe health values and
percentages, clear local `DEFEND` and enemy `ATTACK` labels, remaining match time, and a syncing
state for incomplete generations. It does not reuse a chest pickup label or Wipeout kill score.

World presentation includes:

- a safe silhouette materially larger and more structural than a treasure chest;
- unambiguous defending-team color visible in normal, reduced-effects, and primitive fallback;
- public current/max health treatment and readable damage/critical-health states;
- exact live/destroyed collision meaning and a bounded destruction cue;
- no loot glow, pickup burst, opening animation, or chest audio vocabulary.

The completed overlay and results identify threshold, timeout, draw, or forfeit correctly. The
typed `HeistSummary` records at minimum final/max safe health, objective damage by team and source
family, first objective-damage ticks, destroying source when present, simultaneous destruction,
timeout margin, and participant objective damage. Common fighter damage, defeat, respawn, build,
ability, movement, and disconnect telemetry remains available without counting safe damage as
fighter damage or weapon charge.

### Routing, admission, and product flow

Heist is a complete routed product mode:

- routing, server config, lobby catalog, queue advertisements, worker manifests, admission,
  fingerprints, game-type summaries, Dashboard selection, practice, and E2E scripts recognize one
  stable Heist mode identity;
- the lobby admits only a Heist game type whose selected map resolves exactly two valid safe
  anchors and compatible team capacity;
- the match worker composes `HeistModePlugin` only for that mode and rejects mode/map/rules
  mismatches before gameplay;
- client flow, HUD, audio, results, replay/requeue, reconnect, and shutdown dispatch explicitly on
  Heist rather than treating every non-Wipeout mode as Hot Zone;
- Wipeout and Hot Zone retain their current behavior and mixed-worker operation.

## Presentation and readability contracts

All health-bearing objects are public, but information density remains bounded:

- safes show persistent objective health because it is always decision-critical;
- barrels and chests show compact world health only while damaged, recently hit, or targeted if
  playtest confirms persistent bars are noisy;
- hit, terminal, explosion, drop, pickup, safe-critical, and safe-destroyed cues have distinct
  shapes/colors/audio and remain legible against every supported theme;
- imported and primitive paths communicate identical footprint, collision, targetability, team,
  health, blast, pickup, and terminal facts;
- reduced effects may simplify particles, debris, flashes, and animation but cannot hide blast
  extent, safe ownership, health, pickup availability, or collision change;
- screen-space HUD remains presentation-only and never owns object health, collection, or victory.

Damage feedback must explain whether an attack hit a fighter, deployable, neutral object, or safe.
An ineligible friendly-safe hit may show a restrained blocked/immune response, but it cannot emit
damage numbers or mutate health.

## Telemetry and Balance Lab

The Balance Lab exposes only values owned by completed V10 behavior:

- barrel maximum health, explosion radius/damage, target policy, and terminal profile;
- chest maximum health, restoration amount, pickup radius/lifetime, and terminal profile;
- Heist safe maximum health, match duration, countdown, respawn delay, and supported topology;
- later tuning values only after their behavior is implemented.

Authoritative telemetry distinguishes fighter, deployable, environment-object, and mode-objective
damage. It records object damage, terminal causes, explosion chains, environmental lineage,
friendly/self/environmental defeats, pickup spawn/collect/expire outcomes, safe damage, safe
destruction, and dropped/capacity-fault facts using bounded aggregates and deques.

Object and objective aggregates consume the dedicated world-target fact stream. Existing fighter
and common match aggregates consume only their established combat facts; V10 must not make those
readers infer whether a stable target ID happens to name a fighter or a map object.

Balance evidence must answer:

- time and committed attacks required to destroy each object across representative builds;
- safe time-to-destruction, comeback frequency, timeout frequency, and build/objective damage share;
- whether respawn and lane travel create sustainable attack/defense decisions rather than spawn
  traps or indefinite turtling;
- barrel damage, chain frequency, self/allied defeats, and readability;
- chest contest, pickup collection/expiry, effective healing, and snowball contribution;
- whether any weapon, ultimate, deployable, or topology becomes nonviable or dominant.

## Verification contract

### Pure and focused ECS tests

- catalog/profile validation, stable identities, generation matching, damage eligibility, timeout
  fraction comparison, simultaneous-safe destruction, terminal exact-once rules, stable candidate
  ordering, explosion geometry/lineage, pickup eligibility, and deterministic overlap winner;
- fixed-schedule tests explicitly advance Bevy fixed time and prove primary damage, terminal
  reactions, secondary damage, ability observation, mode evaluation, lifecycle, concealment
  resolution/observer decisions, publication, and tick-finalize order;
- duplicate/stale/wrong-match payload, terminal, pickup, recovery, and outcome facts are rejected;
- existing fighter charge, passive, Wipeout, Hot Zone, defeat, and common match-telemetry readers
  remain unchanged by object/safe facts, while barrel damage to fighters remains visible to their
  established combat-outcome readers;
- zero damage, rejected attacks, friendly-safe contact, non-damage effects, pre-match and completed-
  match contact do not mutate object health;
- terminal collision repair and replacement are atomic and recoverable.

### Separate-App and routed verification

- current object health, terminal state, pickups, both safes, HUD facts, and results converge for two
  clients, late join, reconnect, packet delay/loss/duplication/jitter, restart, map replacement,
  requeue, and process recovery;
- clients cannot author health, damage, explosion, pickup, safe, score, or result state;
- concealed-attacker and per-observer cue tests retain V9 privacy while public object state agrees;
- routed Heist 1v1, 2v2, and 3v3 cover threshold, timeout, simultaneous draw, forfeit, restart, and
  fresh-lobby requeue;
- heterogeneous Wipeout, Hot Zone, and Heist workers can run concurrently without mode dispatch or
  report-schema confusion;
- barrel and chest scenarios exercise straight, lobbed, melee, ultimate, deployable interception,
  environmental chain, and pickup collection as supported by their accepted policies.

### Capacity and lifecycle evidence

Before each implementing milestone closes, select and enforce bounded ceilings for damageable
placements, safe entities, secondary targets, barrel chains, pickups, facts, cues, replication
bytes, recovery bytes, and per-tick work. Measure fixed-tick p50/p95/p99/max, server entity high
water, client entity/visual high water, memory, bandwidth, recovery size, and repeated-match growth
at the maximum supported 3v3 map content.

No queue, cache, entity family, collider, pickup, terminal record, or presentation asset may grow
across repeated restart/reconnect/requeue cycles.

### Native playtest

Native normal, primitive-fallback, reduced-effects, keyboard/mouse, and controller checks cover:

- target recognition and health readability for barrels, chests, and safes;
- barrel blast anticipation, attribution, chain readability, and collision change;
- chest destruction, exact pickup appearance, collection contest, healing feedback, and expiry;
- safe/chest visual distinction, safe team ownership, attack/defense navigation, critical-health
  pressure, threshold/timeout/draw results, and replay/requeue;
- 1v1/2v2/3v3 pacing, respawn travel, turtling, spawn pressure, build variety, and match duration;
- imported/primitive and reduced-effects parity.

Visual verification complements authority and network tests; it cannot prove health, exact-once
behavior, recovery, or client non-authority.

## Initial implementation boundaries

Likely production ownership is:

```text
src/map/
  catalog.rs            durability profile IDs and map-authoring validation
  runtime/ or objects/  environment-object install, identity, terminal state, reset, recovery

src/combat/
  delivery.rs           damageable-target geometry/contact
  effects/              target-aware damage and bounded secondary transaction
  outcomes.rs           existing fighter/deployable facts; environmental fighter damage only

src/environment/ or focused map-owned module
  outcomes.rs           dedicated bounded environment-object/objective facts and source lineage
  barrel.rs             oil-barrel terminal reaction and telemetry
  chest.rs              chest terminal reaction
  pickup.rs             restoration spawn, overlap, collection, expiry

src/matchplay/
  heist.rs              rules, safe ownership, outcome, restart, telemetry

src/client/presentation_3d/
  world-object and Heist-safe presentation bound to replicated state

src/client/
  HUD, audio, results, and product-flow dispatch
```

The exact split is decided from demonstrated ownership during V10 research. `map/mod.rs`,
`combat/mod.rs`, and `matchplay/mod.rs` remain composition/public surfaces. Do not place all object
behaviors in `map/runtime.rs`, widen one existing combat system without decomposition, create one
plugin per data type, or add a new crate without a proven role/platform/reuse boundary.

## Deferred after V10

- Additional pickups, random loot, weighted tables, carried bonuses, temporary buffs, inventories,
  account rewards, currencies, rarity, or chest ownership.
- Safe repair/healing, objective armor/multipliers, objective-specific build parts, barrel damage to
  safes, overtime, asymmetric Heist, alternating rounds, or tournaments.
- Movable/physics props, damage states for ordinary walls, repairable objects, hazards with lasting
  fields, fire propagation, elemental interactions, structural collapse, or material simulation.
- Generic interaction prompts, switches, doors, teleporters, launchers, healing pads, or behavior
  scripting.
- Concealed objects/pickups/objectives, spectator-specific objective permissions, replay, or kill
  cam.
- More Heist maps or object content beyond the accepted reference slice unless playtest evidence
  makes them necessary for V10 balance or closeout.

## Preparation sources

Local, version-pinned sources inspected while preparing this contract:

- `src/map/catalog.rs`, `runtime.rs`, `server.rs`, and `client.rs` for sparse placements, typed mode
  anchors, colliders, whole-placement mutation, generation/revision recovery, and client
  convergence;
- `src/combat/delivery.rs`, `effects/mod.rs`, `effects/application.rs`, `model.rs`, `cues.rs`, and
  `outcomes.rs` for fighter/deployable targeting, composed payload staging, `CurrentHealth`, stable
  combat identity, existing environmental source shape, cue subject filtering, and fighter-centric
  outcome consumers;
- `src/concealment/mod.rs`, `network.rs`, and `field.rs` for completed attack/damage reveal,
  connection-specific observer decisions, cue privacy, public entities, and replication ordering;
- `src/profiles/catalog.rs` and `src/client/profile.rs` for the bounded server-advertised brawler
  catalog and authoritative resolved maximum-health source used by restoration pickups;
- `src/gameplay.rs`, `src/matchplay/mod.rs`, `hot_zone.rs`, `wipeout.rs`, `server.rs`, and
  `telemetry.rs` for fixed-post ordering, restart transactions, mode-owned state, deadline/
  threshold precedence, summaries, and result publication;
- `src/protocol.rs`, `src/server/admission.rs`, `src/server/lobby/catalog.rs`, and routed game-mode
  mappings for global registration, compatibility, admission, worker, and product dispatch;
- `src/client/hud.rs`, `audio.rs`, `session.rs`, and `presentation_3d/` for objective generation
  matching, HUD/results, cues, and world presentation;
- `references/bevy/examples/app/plugin.rs` for focused Bevy plugin composition;
- `references/lightyear/examples/network_visibility/src/server.rs` and the checked-in Lightyear
  replication material for replicated entity lifetime and public/observer visibility interaction;
- [environment gameplay](./09-environment-gameplay.md),
  [grid map assets](./16-grid-map-asset-system.md),
  [concealment and reveal](./17-concealment.md), and the retained `M08-ENV-SOURCE` obligation in the
  [V1 roadmap](./implementation/v1/roadmap.md#v1-backlog).

The checked-in Lightyear 0.29 material matches Brawler's Bevy 0.19 dependency. The checked-in Bevy
source is 0.20-dev and was used only for plugin-structure guidance. No internet research was needed
for this preparation; V10 M01 must recheck exact installed Bevy, Lightyear, and Avian APIs before
implementation.
