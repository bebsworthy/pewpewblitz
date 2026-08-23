# Environment gameplay direction

## Purpose and authority

This document defines the forward-looking direction for environmental gameplay: surfaces,
concealment, hazards, traversal, interactive geometry, and temporary areas that change player
decisions inside a match. It is a candidate catalog and a set of promotion constraints, not a
commitment to implement every family or a schedule for a future version.

The [map and mode specification](./04-maps-and-game-modes.md) owns the map recipe, resolved map,
runtime map, terrain, mode-anchor, and mode-runtime boundaries. The
[network architecture](./08-network-architecture.md) owns replication, interest management, and
concealment transport. The [art, presentation, and asset specification](./11-art-and-presentation-direction.md)
owns how environment facts are rendered. The [backlog](./backlog.md) is the canonical candidate
index, and a promoted version milestone becomes the implementation scope contract.

Brawler uses original names, values, art, layouts, and combinations. Genre examples help describe
the vocabulary; they are not content to copy.

## Established foundation

Authoritative gameplay is planar and independent from its 3D presentation. The existing foundation
provides:

- sparse 32-unit grid recipes with stable map-asset, gameplay-profile, visual, spawn, and typed
  mode-anchor identities;
- exact profile-owned rectangle/circle collision independent of authoring footprints;
- explicit destructible map-asset cells with whole-cell removal or bounded replacement;
- server-owned map installation, placement mutation, restart, recovery, and teardown;
- mode-owned Hot Zone objective occupancy, progress, and scoring derived from a map anchor;
- client-owned visual profiles, themes, imported scenes, primitives, generated meshes,
  and deterministic fallbacks.

The current map-asset catalog implements ground, blocking water, honestly non-concealing tall
grass, walls, round obstacles, destructible cover, rubble replacement, inert decorations, and
spawn markers. It does not imply hazards, movement modifiers, concealment, or arbitrary scripted
interactions.

A visible tile is not independently authoritative. Gameplay comes from the shared profile attached
to a placed `MapAssetId`; client surfaces, props, vegetation, particles, materials, and shaders
present those facts without deciding them.

## Environment extension principles

1. **Promote one player-visible capability at a time.** A future version selects one coherent
   surface, concealment, hazard, traversal, interactive, or temporary-area outcome. It does not
   implement an environment framework or the complete catalog first.
2. **Keep authored definitions, placements, and runtime state distinct.** A map may place a stable
   approved profile; developer-authored definitions provide its validated rules; server ECS state
   owns current overlaps, timers, effects, transitions, and revisions.
3. **Let the demonstrated mechanic own its rules.** Start with focused components, resources, and
   systems for the selected capability. Extract shared region/effect machinery only when another
   real use demonstrates the same ownership and lifecycle.
4. **Preserve server authority.** Clients send movement or ability intent. They do not claim region
   membership, movement modifiers, damage, healing, reveal, displacement, collision transitions,
   or activation results.
5. **Keep presentation replaceable.** Stable gameplay/profile identities may resolve to client art,
   audio, and effects, but presentation readiness never controls the rule.
6. **Keep bounds explicit.** Shapes, active instances, overlap work, effect frequency, lifetime,
   collision rebuilds, replication, recovery, and transient presentation all require limits
   proportional to the selected slice.
7. **Keep mode ownership intact.** Maps provide validated space and anchors; a game mode owns
   objective progress, scoring, victory, and mode-specific overrides. An objective is not converted
   into generic environment behavior merely because it occupies an area.
8. **Do not imply vertical gameplay.** A jump pad or similar candidate may perform a bounded
   server-authored planar displacement with presentation-only height unless a future architecture
   decision explicitly changes the authoritative movement model.

## Environment candidate catalog

| Family | Candidate examples | Player-visible decision | Principal risk |
|---|---|---|---|
| Movement surface | Speedway, slow ground, mud, snow, web | Route through a positional movement advantage or penalty | Movement composition, overlap, prediction |
| Directional surface | Conveyor, wind current | Enter a space that adds or forces directional movement | Collision/displacement ordering |
| Slippery surface | Ice, oil | Trade control for retained momentum or speed | Input feel, deterministic movement, prediction |
| Damage hazard | Fire, acid, electricity, danger boundary | Respect telegraphed area denial or accept damage/status risk | Damage attribution, tick rate, stacking, readability |
| Beneficial field | Healing, shielding, haste, energy | Contest or remain inside supportive space | Team/owner filters, stacking, combat pacing |
| Tactical field | Reveal, silence, anti-heal, projectile modifier | Change information or combat rules within bounded space | Cross-system coupling and exceptions |
| Concealment | Tall grass, smoke, darkness, invisibility field | Use uncertain information for ambush, escape, or area control | Observer-specific gameplay and replication |
| Traversal device | Teleporter, one-way gate, planar jump pad | Commit to a route transition with known destination/risk | Placement safety, cooldown, arrival collision |
| Interactive geometry | Door, switch, moving or retractable cover | Change available paths or sight/projectile lines | Stateful collision transitions and recovery |
| Ability-created area | Smoke, temporary wall, speed/slow field, hazard | Spend build power to temporarily reshape local space | Ownership, lifetime, cleanup, interaction with authored areas |

Permanent walls, ordinary ground, destructible cover, and cosmetic dressing are established map or
presentation capabilities rather than candidates in this catalog. Additional visual themes or props
without gameplay consequences belong to map content and art direction.

Water and vegetation are not universal gameplay types. Deep blocking water, shallow slowing water,
damaging liquid, and a visual-only puddle are different compositions. Decorative plants remain
presentation; only an authored or runtime concealment rule changes visibility.

## Authored and runtime composition

The extension model should remain direct:

```text
Map recipe
  stable MapAssetId + cell placement or typed mode anchor
          |
          v
Developer-authored capability definition
  validated immutable rules for the selected mechanic
          |
          v
Server runtime
  overlaps, timers, effects, transitions, state and revisions
          |
          v
Client presentation
  replicated facts/cues + stable presentation identity
```

Static environment behavior begins in a validated map placement. Temporary or stateful behavior may
instead begin from an ability, fighter, mode, or interactive entity, but it must instantiate an
explicit server-owned runtime entity/state with a stable owner and bounded lifetime. Authored and
runtime sources may reuse the same rule definition when they genuinely produce the same gameplay
behavior; they do not need to share creation, persistence, or network lifecycle.

Stable definitions should contain only the parameters the selected mechanic requires. Do not begin
with a universal `SurfaceRegionDefinition`, movement-rule language, environment effect graph, or
arbitrary property bag. The existing map placement/profile boundary can be extended with focused
definitions and lowering when a milestone proves the need.

## Questions for every promoted capability

Before implementation, the milestone specification must decide:

- Which subjects are affected: fighters, projectiles, deployables, terrain, objectives, or a
  deliberately smaller set?
- Is the rule continuous, periodic, entry-triggered, exit-triggered, activated, or latched for a
  duration?
- How do authored, runtime, and overlapping instances combine: no stacking, strongest value,
  additive/multiplicative composition, explicit priority, or another bounded rule?
- How do team, owner, immunity, status, defeat, respawn, and mode state filter the effect?
- What stable source identity owns damage, healing, status, displacement, telemetry, and cues?
- What happens when an area is created, removed, disabled, or replaced around an entity already
  inside it?
- How does the capability reset on match restart and clean up on map replacement, worker shutdown,
  or owner removal?
- Which state must replicate, which state can be reconstructed, and how do late join and recovery
  obtain a current bounded result?
- Which telegraph and feedback communicate the rule before commitment, during application, and on
  exit or expiry?
- If prediction is later justified, can the exact authoritative rule execute deterministically for
  the predicted owner without leaking secret state?

Movement surfaces must additionally specify whether they change maximum speed, acceleration,
turning, momentum, or add forced displacement. They should not collapse these distinct behaviors
into one ambiguous speed multiplier.

The first environmental damage capability must revisit `M08-ENV-SOURCE` in the
[V1 backlog](./implementation/v1/roadmap.md#v1-backlog) so outcomes preserve correct source
attribution rather than appearing as unattributed map damage.

## Concealment gameplay model

Concealment is a high-risk candidate because its gameplay rule, bot perception, UI/audio policy,
and replication boundary must agree. Static grass, temporary smoke or darkness, and fighter
invisibility may feed one observer-versus-subject decision when their actual reveal semantics are
compatible, but that shared decision should be demonstrated by the selected slice rather than
assumed as a generic framework.

The authoritative server retains absolute match state, including hidden fighters, bots,
projectiles, effects, objectives, and terrain. For each observer and potentially hidden subject it
derives whether live spatial state is observable. A client never declares itself concealed and
never decides which opponents it may observe.

The first concealment specification must explicitly decide:

- self, ally, enemy, spectator, and defeated-player visibility;
- whether occupants of the same concealment instance see one another;
- proximity reveal distance and whether geometry blocks proximity reveal;
- whether attacking, using an ability, taking damage, carrying an objective, or colliding reveals;
- reveal duration, expiry, exit, and re-entry behavior;
- whether projectiles, trails, sounds, damage indicators, overhead UI, target markers, statuses,
  objective facts, or telemetry reveal the hidden subject;
- how server-hosted bots perceive and remember concealed targets;
- how late join, reconnect, defeat, respawn, interpolation, and any future prediction behave at
  visibility transitions.

A reasonable research hypothesis is that owners and allies retain visibility, opponents lose it
unless an explicit proximity or reveal rule succeeds, attacking reveals for a short validated
duration, and a mode may declare an objective-carrier override. These are candidate semantics, not
accepted balance or implementation requirements.

The [network architecture](./08-network-architecture.md#interest-management-and-concealment) owns
the exact Lightyear visibility mechanism, public-participant versus private-spatial split, hierarchy
and message audit, security limitation, schedule boundary, and transport verification.

### Concealment verification expectations

An implementing milestone should prove at least:

- an unauthorized observer does not receive a subject hidden before initial relevance;
- a visible subject becoming hidden disappears for opponents but remains valid for its owner and
  permitted allies;
- two observers can receive different authoritative outcomes for the same subject;
- reveal returns current state without replaying the hidden path;
- proximity, action, damage, objective, and expiry rules transition on exact authoritative ticks;
- private descendants, messages, effects, UI, and cues do not leak hidden state;
- bots follow the same gameplay perception rules without requiring network connections;
- late join, reconnect, defeat, respawn, restart, and map replacement do not retain stale visibility;
- presentation clearly communicates boundaries, reveal, disappearance, reappearance, and the
  reason a subject became observable.

## Networking and prediction boundary

Ordinary environment gameplay replicates the smallest bounded authoritative state or cues needed by
the client; it does not replicate renderer handles or a generic environment event stream. Static
map placements may be reconstructed from the accepted resolved map. Runtime areas and interactive
geometry need stable identity plus enough current state for late join and recovery.

Concealment is different because withholding unauthorized state is part of the gameplay rule; its
network contract is owned by the network architecture. Coarse interest management is an optional
capacity tool, not the concealment decision and not a prerequisite for the first non-concealment
environment slice.

Movement and collision outcomes remain server-authoritative. Prediction or lag compensation is
introduced only when impairment evidence identifies a player-visible problem and the selected rule
can preserve collision, terrain, secret-state, and reconciliation correctness.

## Presentation contract

Every gameplay-relevant environment capability must be legible independently of decorative art:

- its active boundary and affected space are readable at gameplay camera scale;
- damaging, beneficial, movement-altering, blocking, interactive, and concealed space do not rely
  on one ambiguous material treatment;
- activation, application, expiry, cooldown, and blocked transitions have bounded cues;
- reduced-effects mode preserves required boundaries and state while reducing redundant particles,
  debris, animation, and audio;
- primitive/degraded presentation preserves the gameplay distinction when imported assets or
  custom shaders are unavailable;
- presentation cleanup follows the authoritative owner and cannot keep a stale area visible after
  removal or replacement.

Themes and compatible visual profiles may change the art treatment without changing the capability
definition. Conversely, a visual prop or material never grants gameplay behavior merely because it
resembles grass, fire, ice, a door, or a teleporter.

## Candidate selection guidance

The canonical backlog promotes one environment outcome, not a predetermined bundle. Selection
should compare player value, interaction with existing builds/modes/maps, authority complexity,
presentation cost, and verification risk.

- A single speed or slow surface is a relatively focused proof of authored overlaps and movement
  composition, but its value depends on a map that creates meaningful route choices.
- A readable damage hazard exercises combat attribution and area denial while requiring careful
  telegraphing and pacing.
- Concealment offers a distinctive information game but is a larger networking, perception,
  presentation, and security slice; it should stand alone.
- Traversal and interactive geometry should be selected only with a map loop that makes their state
  changes valuable rather than as technology demonstrations.
- Ability-created areas should begin with one real build tradeoff and reuse existing combat/effect
  primitives only where their behavior and lifecycle truly match.

Implementing one family does not commit Brawler to the others. A later capability may reuse proven
parts, but the catalog is not a promise of a universal surface, region, interaction, or navigation
system.

## References

- [Product direction](./00-product-direction.md)
- [Fighter and build specification](./02-fighter-model.md)
- [Weapons and abilities specification](./03-weapons-and-abilities.md)
- [Map and mode specification](./04-maps-and-game-modes.md)
- [Network architecture](./08-network-architecture.md)
- [Art, presentation, and asset specification](./11-art-and-presentation-direction.md)
- [Canonical backlog](./backlog.md)
- [V1 implementation roadmap](./implementation/v1/roadmap.md)
