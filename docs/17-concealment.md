# Concealment and reveal specification

## Purpose and authority

This document defines PewPew Blitz's authoritative concealment and reveal gameplay contract. It
owns how terrain, a self-cloak ultimate, an allied concealment-area ultimate, proximity, accepted
attacks, applied damage, and a reveal-area ultimate combine into one observer-specific visibility
decision. It does not make rendering visibility authoritative and it does not treat network
interest management as an optional optimization when live spatial state is secret.

The [fighter specification](./02-fighter-model.md) owns resolved fighter attributes and build
modifiers. The [weapon and ability specification](./03-weapons-and-abilities.md) owns ultimate
selection, charge, activation, targeting, and combat outcomes. The
[map specification](./04-maps-and-game-modes.md) owns placed terrain and resolved map state. The
[network architecture](./08-network-architecture.md#interest-management-and-concealment) owns the
Lightyear replication boundary. The [V9 roadmap](./implementation/v9/roadmap.md) owns delivery.

V9 completed and this contract entered production on 2026-08-24. Its three sources, reveal rules,
server-owned observer decision, sentry integration, bounded replication, presentation, recovery,
and Balance Lab controls passed the verification and user-playtest gates recorded by that roadmap.

## Player-facing outcomes

Concealment has three sources and one explicit build-based counter:

1. **Terrain concealment.** A fighter uses a concealing grass or bush placement to hide from
   sufficiently distant enemies.
2. **Self cloak.** A fighter activates an ultimate and becomes concealed for a bounded duration;
   an accepted attack or applied damage consumes that cloak permanently.
3. **Allied concealment area.** A fighter targets a visible area within range. For a bounded
   duration, the public area conceals the caster and friendly fighters inside it.
4. **Reveal scan.** A fighter targets an area within range. Activation immediately marks all enemy
   fighters in that area as revealed for a bounded duration, whether they were visible or
   concealed. Once accepted, the scan has no reaction or avoidance window.

The concealment area, terrain boundary, and reveal scan activation are visible gameplay facts.
Hidden fighters' current spatial state is not.

## Core visibility model

Visibility is derived for an **observer–subject pair**. There is no global `Invisible` component
whose value can describe every client's permitted view. At the same authoritative tick, a subject
may be visible to itself and allies, revealed to one nearby enemy, and concealed from another
distant enemy.

The server retains and simulates absolute match state. For each connected observer it derives
whether the subject's live spatial fighter and subject-derived private facts may be replicated. The
client never declares concealment, proximity, reveal, or visibility.

The decision is evaluated in this order:

```text
public or unconditional observer permission
  self, ally, or authorized non-playing observer
        |
        v
forced reveal
  active reveal-scan deadline
        |
        v
temporary reveal locks
  accepted-attack deadline or applied-damage deadline
        |
        v
active concealment eligibility
  terrain membership, live self cloak, or allied concealment-area membership
        |
        v
source-specific proximity reveal
  observer reveal-proximity radius for terrain and allied areas only
        |
        v
visible or concealed for this observer–subject pair
```

If no concealment source is active, the subject is visible. Forced reveal and temporary reveal
locks suppress every concealment source. Self and allies always receive the subject's current
spatial state. A normal enemy observer receives it only when the decision permits it.

## Reveal-proximity fighter attribute

`reveal_proximity_radius` is a resolved fighter attribute belonging to the **observer**. It is not
a terrain constant and not a property of the hidden subject. A larger value is beneficial: it lets
that observer reveal enemies concealed by terrain or an allied concealment area from farther away.

The base value is authored in each fighter profile alongside maximum health and movement speed. A
validated fighter/passive grant or future owned-equipment effect may apply a bounded bonus or malus
before the match.
Resolution produces one finite, positive value in `ResolvedFighterStats`; runtime concealment never
queries profile storage, inventory, part instances, rarity, or display metadata.

Modifier composition adopts the established deterministic modifier order: combine all validated
flat changes, then the combined percentage change, clamp to the code-owned minimum and maximum, and
round once at the resolved boundary. Exact base values, modifier inventory, and bounds are selected
through the Balance Lab and playtest. The implementing milestone must add the attribute to content
fingerprints, immutable match snapshots, preview text, validation, persistence/backup fixtures,
and Balance Lab surfaces that already own fighter attributes.

Proximity uses authoritative planar center-to-center distance and the observer's resolved radius.
The first implementation does not add line-of-sight occlusion: walls do not block proximity
reveal. Being in the same concealment placement or field does not bypass the distance rule. This
keeps the attribute meaningful and avoids introducing a second vision system. Allies remain exempt
from the distance check.

Self cloak is not proximity-revealed. Its counterplay is its finite duration, permanent break on an
accepted attack or applied damage, and the reveal-scan ultimate.

## Concealment sources and break rules

| Source | Active eligibility | Accepted attack | Applied damage | Enemy proximity | End condition |
|---|---|---|---|---|---|
| Terrain | Living fighter overlaps an active concealing placement | Reveal all enemies for `M` ticks | Reveal all enemies for `N` ticks | Observer-specific reveal | Exit, placement removal/replacement, defeat, reset, or teardown |
| Self cloak | Accepted ultimate with unexpired source | Consume cloak permanently | Consume cloak permanently | No proximity reveal | Attack, damage, deadline, defeat, reset, or teardown |
| Allied area | Living friendly fighter overlaps the public active field; includes caster | Reveal all enemies for `M` ticks | Reveal all enemies for `N` ticks | Observer-specific reveal | Exit, field expiry/removal, defeat, reset, or teardown |

V9 M01 initially authors `M = 90` ticks (1.5 seconds) and `N = 120` ticks (2 seconds) at 60 Hz.
These are Balance Lab/playtest values and may change through ordinary balance revision without
changing the reveal-lock semantics. A deadline is active while `current_tick < deadline`.

`M`, `N`, source durations, radii, ranges, charge rules, and cooldowns are bounded authored balance
values. `M` and `N` may differ. If multiple temporary locks exist, the effective deadline is the
latest deadline; durations do not add.

An attack breaks or temporarily reveals only when the authoritative attack transaction is
accepted. Pressing fire while cooling down, reloading, defeated, stale, or otherwise rejected does
not reveal the fighter. Dash or a non-attacking interaction does not count as shooting unless its
accepted definition explicitly produces an attack.

Damage breaks or temporarily reveals only when a harmful authoritative outcome applies a positive
amount to the fighter. A miss, blocked delivery, zero result, or rejected effect does not reveal.
Damage over time reveals on each positive application. A future shield must specify whether an
absorbed hit counts before joining this contract; V9 does not infer that rule in advance.

Attack and damage locks suppress all concealment sources. For example, shooting while self-cloaked
consumes the self cloak and also prevents grass or an allied field from immediately hiding the
fighter until `M` ticks expire. Consuming one self-cloak instance does not permanently forbid a
later terrain, field, or newly activated self-cloak source.

## Reveal-scan ultimate

Reveal scan is a targeted, instant, server-authoritative counter ultimate:

- the caster chooses a bounded point within the ultimate's legal range;
- the accepted activation creates one public scan cue and evaluates one bounded area immediately;
- every living hostile fighter intersecting the area receives a forced-reveal deadline of
  `current_tick + R`, even if that fighter was visible at activation;
- the affected fighter remains revealed to the caster's whole team after leaving the scanned area;
- while the deadline is active, terrain, allied-area concealment, and self cloak cannot hide the
  fighter from that team;
- repeated friendly scans keep the latest deadline rather than stacking duration;
- scan does not consume the underlying concealment source, so an unexpired cloak or current area
  membership may conceal the fighter again after the forced reveal expires;
- scan affects fighters only. It does not reveal, destroy, retarget, or otherwise mutate
  projectiles, deployables, or map assets.

There is no pre-acceptance world warning and no post-acceptance dodge or cleanse in the initial
contract. Strategic counterplay comes from build knowledge, charge/use awareness, spacing, and not
committing every concealed teammate to one scan area. Activation and the resulting revealed state
must nevertheless be visually and audibly legible after the server accepts them.

## Team, mode, and lifecycle rules

- A fighter always sees itself and current allies, including allies concealed by an enemy source
  if a future capability ever permits that composition.
- The allied concealment field includes its caster.
- Proximity reveal is local to the observing fighter. It does not grant team-wide reveal.
- Reveal scan is explicitly team-wide for the caster's team. Forced-reveal deadlines are keyed by
  revealing team rather than stored as one global value so future legal multi-team topologies
  remain correct.
- Defeated players do not receive secret live enemy positions through a free camera or detached
  observer. V9 retains the ordinary player camera policy unless a later spectator milestone defines
  authenticated observer permissions and information delay.
- Public roster, team, connection, defeat, score, match, and objective-summary facts remain
  available without exposing current hidden pose or subject-derived spatial facts.
- Objective carriers are ineligible for concealment in the first implementation. A future mode may
  opt in only with an explicit carrier visibility rule and playtest evidence.
- Defeat consumes the fighter's Self Cloak but does not cancel a Reveal Scan that it already cast;
  canceling an accepted scan on caster defeat would create forbidden post-acceptance counterplay.
  Subject respawn clears that subject's forced-reveal records. Match restart, map/match replacement,
  disconnect or source-owner removal, worker shutdown, and teardown clear remaining source records,
  reveal locks, and observer-pair caches owned by the ending lifecycle.

## Terrain contract

A concealing terrain asset uses an explicit server-known concealment capability referenced by its
`MapGameplayProfileId`. Visual appearance never grants concealment. V9 deliberately promotes the
existing `TALL_GRASS` identity from V8's non-concealing presentation proof into real concealing
terrain. V8 accepted it as non-concealing only because observer-specific concealment did not yet
exist; that historical behavior is not a permanent identity constraint. `TALL_GRASS` keeps its
stable map-asset and visual identities but is rebound from V8's shared inert pass/pass profile to a
new explicit concealing gameplay profile. The shared profile is not mutated because ground, rubble,
and other non-concealing assets also reference it. The V9 schema, catalog, content fingerprint,
gameplay description, and every affected map/playtest are revised together so the semantic change
is explicit rather than a compatibility accident.

The resolved map derives bounded concealment volumes from placed footprints. Runtime membership is
server-owned and updates from authoritative fighter position. Destruction or replacement removes
the capability before the next visibility decision; reset and recovery restore the selected map's
current authoritative result without replaying historical membership events.

## Ability-area contract

Self cloak is fighter-owned state with a stable ultimate definition, activation tick, expiry tick,
and source generation. The allied concealment field is a bounded runtime entity with stable field,
owner, team, center, radius, activation, and expiry identity. Clients may receive the public field
geometry and deadline regardless of whether its occupants are hidden.

The area ultimate uses the normal ability charge and accepted-input path. The server clamps or
rejects its targeted point according to the authored range policy and validates finite coordinates,
active match phase, fighter lifecycle, charge, ownership, and active-field ceilings. One ability
does not create a generic region scripting system.

## ECS ownership and fixed-tick order

The implementation should begin with focused concealment ownership rather than embedding network
calls throughout combat, maps, and abilities:

```text
src/concealment/
  mod.rs          composition, system sets, narrow shared types
  model.rs        source, membership, lock, reveal, and observer decision facts
  authority.rs    fixed-tick source/membership/decision lifecycle
  network.rs      connection mapping, visibility application, cue filtering
  telemetry.rs    bounded transitions and aggregates
  tests.rs        pure and small-App behavior

src/concealment/client/
  presentation.rs  public boundaries and permitted reveal/conceal cues
```

Exact extraction follows implementation evidence, but authoritative state remains server-gated and
client presentation remains optional. `mod.rs` is a composition surface, not the implementation
owner for every algorithm.

The fixed-tick semantic order is:

1. lifecycle and map/field creation/removal become current;
2. authoritative movement and collision finish;
3. attacks are accepted and attack reveal/consumption facts are recorded;
4. projectile/melee outcomes and positive damage applications are resolved;
5. concealment memberships, source expiries, reveal locks, forced reveals, and observer–subject
   outcomes are derived from the completed tick;
6. subject-derived cues are filtered per connection;
7. `gain_visibility`/`lose_visibility` changes are applied before Lightyear's `PostUpdate`
   `ReplicationSystems::Send` path;
8. presentation receives only public facts and facts permitted for that observer.

The implementing milestone must make the actual `SystemSet`, `.before`/`.after`, `.chain()`, and
`ApplyDeferred` relationships visible at composition. A visibility change queued through
`Commands` needs an explicit deferred-command boundary before replication send; relying on the end
of an unrelated schedule is not sufficient evidence.

## Network privacy and cue policy

Lightyear 0.29's ordinary `VisibilityExt::lose_visibility` policy is required for secret spatial
fighters: a hidden-before-relevance subject is absent, and a previously visible subject is
despawned for that connection. Retained and always-present visibility policies are forbidden for
secret live state because they preserve remotely inspectable state.

The cullable boundary includes the fighter's replicated hierarchy and every subject-derived private
fact. At minimum, implementation must audit:

- pose, facing, movement, interpolation and any future predicted history;
- health bars, nameplates, selection rings, target markers, status visuals, and fighter children;
- accepted-attack, muzzle, trail, projectile source, impact, damage, hit-direction, defeat, and
  ability cues;
- sentry acquisition and target facts;
- audio origin and controller/HUD feedback;
- telemetry or diagnostics transmitted to ordinary clients;
- reconnect, recovery, map readiness, and late-join snapshots.

Public projectiles and effects may remain visible only if their wire shape and cues do not disclose
the hidden fighter's prohibited current state beyond the accepted gameplay rule. An accepted attack
reveals its source on the same authoritative tick before source-derived attack cues are sent, which
normally makes those cues legal. Damage received by a hidden subject similarly applies its reveal
rule before subject-derived damage cues are filtered. Every message path is deny-by-default for a
hidden subject until audited.

The public-participant record remains distinct from the cullable spatial fighter. The concealment
system maps stable `PlayerId`/`NetworkEntityId` and connection ownership without placing
process-local `Entity` identity on the wire.

## Bots and targeting

Server-hosted bots and autonomous sentries must consume the same permitted observer decision as
human opponents. They may not query absolute hidden fighter positions through convenient server
ECS access. A bot observation adapter may include a subject only when that bot would see it;
temporary target memory requires a separate explicit rule and cannot contain current hidden motion.
The proposed [V11 bot specification](./implementation/v11/milestone-01.md) supplies that rule by
retaining only a bounded last-permitted delayed pose with its source tick and expiry; hidden state
never refreshes it.

A sentry does not acquire or continue tracking a concealed enemy unless its owner/team currently
has a rule that permits targeting that subject. Reveal scan enables normal targeting while active.
Losing visibility clears live target locks that would otherwise disclose or exploit current hidden
state.

## Presentation and accessibility

- Terrain and allied concealment boundaries are readable to every player at gameplay camera scale.
- The allied field remains visible for its entire active duration even when all occupants are
  concealed.
- Reveal scan has an immediate public activation footprint and affected fighters receive a clear
  revealed marker while they are legally visible.
- The owner and allies receive an explicit treatment showing that a fighter is concealed from
  enemies; enemy clients do not receive a hidden model merely to fade it locally.
- Conceal/reveal transitions avoid stale models, nameplates, shadows, particles, trails, or audio.
- Reduced-effects and primitive fallback modes preserve boundaries, deadlines, and revealed status
  without depending on particles or color alone.
- Opponent disappearance does not display an exact last-position marker. Previously perceived
  information remains part of human memory, not a continuously updated client object.

## Bounds and telemetry

V9 defines and validates ceilings for concealment placements, simultaneous fields, scan targets,
active source records, observer–subject decisions, visibility transitions per tick, cue fan-out,
and recovery bytes. Pair evaluation is bounded by the admitted active-fighter ceiling; current
ordinary matches cap at six fighters, but implementation must use the resolved topology ceiling
rather than hard-code 3v3.

Telemetry records stable source kind, subject, observing team or permitted aggregate, transition
reason, tick, duration, proximity band, attack/damage break, scan application, and bounded drop
counts. Ordinary player telemetry must not reveal secret positions. Useful balance aggregates
include concealed time, proximity reveals, attacks from concealment, cloak breaks, scan targets,
and time-to-damage after reveal.

## Verification contract

The completed implementation proves:

- pure observer-decision truth tables across teams, source combinations, deadlines, proximity
  bonuses/maluses, and exact boundary distances;
- rejected attacks and zero damage do not reveal, while accepted attacks and positive damage do so
  on the exact authoritative tick;
- self cloak is consumed and terrain/field concealment resumes only after the global reveal lock;
- scan affects visible and hidden enemies in its accepted area and persists after they leave;
- a subject may be visible to one enemy observer and absent for another at the same tick;
- hidden-before-join subjects are never spawned, visible-to-hidden subjects despawn, and reveal
  restores current state without replaying the hidden path;
- owner/allies retain permitted state while ordinary opponents, defeated-player views, cues,
  hierarchies, projectiles, audio, sentries, bots, telemetry, and diagnostics do not leak it;
- late join, reconnect, defeat, respawn, restart, map replacement, field expiry, source removal,
  and shutdown clear stale state;
- repeated transitions remain bounded in entity, observer-pair, queue, cache, message, and memory
  ownership;
- native imported and primitive-fallback playtests make boundaries, concealment ownership, attack
  and damage reveal, proximity reveal, and scan results understandable.

Packet capture or client-World inspection must accompany visual review: a locally hidden mesh is
not evidence of network concealment.

## Explicit non-goals

- General fog of war, wall-based vision, light/shadow simulation, or a universal perception engine.
- Client-authored concealment, local-only opacity as the gameplay rule, or retained hidden fighter
  entities.
- Concealed projectiles, deployables, objectives, map assets, or arbitrary status effects unless a
  later specification extends the audited privacy boundary.
- Reveal cleanses, reveal immunity, counter-counter ultimates, or a generic dispel framework.
- Spectator mode, kill-cam, replay, last-known-position UI, team-shared proximity reveal, or target
  memory.
- A generic runtime-area/effect language before another implemented capability demonstrates the
  same lifecycle.
- Final balance numbers in this durable specification.

## References

Local version-pinned sources:

- `references/lightyear/examples/network_visibility/src/server.rs`;
- `references/lightyear/crates/replication/replication/src/visibility/immediate.rs`;
- `references/lightyear/crates/replication/replication/src/send.rs`;
- `references/lightyear/book/src/concepts/advanced_replication/interest_management.md`;
- `src/gameplay.rs`, `src/combat/server.rs`, `src/combat/authority.rs`,
  `src/combat/effects/application.rs`, and `src/server/mod.rs`.

Primary released references:

- [Lightyear 0.29 replication crate](https://docs.rs/crate/lightyear_replication/0.29.0);
- [Lightyear 0.29 network-visibility example](https://github.com/cBournhonesque/lightyear/tree/0.29.0/examples/network_visibility).
