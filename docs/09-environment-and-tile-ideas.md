# Environment, surface, and region ideas

## Purpose and scope

This document is a research catalog for environmental gameplay across future Brawler versions. It
records useful map primitives, composable properties, concealment rules, and networking constraints
without committing every idea to v1 or assigning implementation order. A version roadmap and its
milestone specifications remain the authority for scheduled scope.

V3 changed the presentation vocabulary: gameplay-world art now uses imported GLB scenes, cached
primitives, generated meshes, procedural indicators, and Bevy UI rather than sprite tiles. The
server-owned region/property model below remains valid because it was intentionally independent of
the renderer. A tile now means an authoring-grid/layout unit, not a runtime sprite requirement.

Brawler should use original names, values, art, layouts, and combinations. Genre examples are input
to the vocabulary, not content to copy.

## Working terminology

The word **tile** can describe how client art is assembled, but a visible tile is not automatically
an authoritative gameplay object.

- **Visual placement:** replaceable client-only surface, model, vegetation, edge, or decoration.
- **Map recipe:** a user-authorable bounded arrangement of presentation layers, geometry, terrain,
  entities, regions, spawn points, and mode-required anchors.
- **Map preset:** a developer-authored legal map recipe used as built-in content or a test fixture.
- **Authored region:** stable map data describing a shape and gameplay properties such as speed,
  concealment, or periodic damage.
- **Geometry:** authoritative blocking or queryable shapes for movement, projectiles, placement,
  and vision.
- **Runtime environment entity:** a server-owned temporary or stateful area such as smoke, fire,
  a healing field, a temporary wall, or a teleporter.
- **Destructible terrain chunk:** quantized occupancy-grid solidity and generated collision. Chunks
  are a sparse implementation unit, not a visible-tile replacement system.

This separation lets art change without changing simulation and prevents a large `TileKind` enum
from becoming the owner of unrelated movement, collision, status, and networking rules.

The eventual map builder edits map recipes using stable catalog references and bounded values. It
does not create environment systems or game-mode rules. Adding a new region behavior, entity family,
terrain material rule, or mode remains developer-authored engine/content work; users compose the
capabilities that have been exposed.

## Environment idea catalog

| Family | Candidate examples | Main gameplay properties | Research status |
|---|---|---|---|
| Neutral surface | Floor, road, platform, packed earth | Walkable; no modifier | v1 foundation |
| Permanent geometry | Boundary wall, pillar, permanent cover | Blocks movement; independently chooses projectile and vision blocking | v1 foundation |
| Destructible geometry | Breakable cover, quantized wall or terrain | Occupancy bits, optional health, collision revision, recovery state | Chunked occupancy-grid terrain planned for v1 Milestone 10 |
| Concealment | Tall grass, bushes, smoke, darkness, invisibility field | Observer-specific visibility, proximity/action reveal, network culling | Future-version candidate |
| Mobility surface | Speedway, conveyor, wind current | Speed or acceleration multiplier, optional forced direction | Future-version candidate |
| Hindering surface | Mud, snow, webs, shallow water | Slow, acceleration/turning modifier, optional status contribution | Future-version candidate |
| Slippery surface | Ice, oil | Friction or retained momentum, steering limits | Later candidate; higher prediction risk |
| Hazard | Fire, acid, lava, electricity, danger boundary | Periodic damage, status, knockback, team filtering, telegraph | Map model reserves hazards; concrete content unscheduled |
| Beneficial field | Healing, shield, haste, energy field | Periodic or continuous positive effect | Compatible with ability/effect primitives; content unscheduled |
| Tactical field | Reveal, silence, anti-heal, projectile modifier | Information or combat-rule modifier | Later candidate after base combat is measurable |
| Traversal | Jump pad, teleporter, one-way gate | Server-authored displacement or route transition, cooldown | Later candidate |
| Interactive geometry | Door, switch, moving cover, retractable wall | Authoritative state, collision transition, activation rules | Later candidate |
| Objective region | Capture zone, pickup area, delivery point | Occupancy, progress, ownership, scoring | Hot Zone enters v1; other objective families are future candidates |
| Cosmetic-only | Decals, trim, foliage outside gameplay grass | Presentation only | May appear whenever assets are authored |

Water is not one universal type. Deep blocking water, shallow slowing water, damaging liquid, and a
purely visual puddle should be separate compositions. The same rule applies to vegetation: only an
authored concealment region changes visibility; decorative plants do not.

## Recommended first future environment slice

A focused post-v1 slice should compare a small set with distinct decisions:

1. tall grass or another static concealment region;
2. one spell-created concealment region using the same visibility contract;
3. a speedway surface;
4. a slow surface;
5. one readable damage hazard;
6. permanent and destructible cover already established by v1.

This set exercises routing, ambushes, mobility, area denial, per-client visibility, and environment
effect composition without immediately requiring teleport chains, moving platforms, or material
simulation.

## Composable authored properties

Exact Rust types belong to the milestone that implements them. The conceptual authored model should
keep orthogonal properties separate and use stable definition IDs.

```text
SurfaceRegionDefinition
  stable_region_id
  authored_shape
  movement_profile_id?
  periodic_effect_id?
  team_filter
  stacking_priority
  presentation_id

MovementProfile
  maximum_speed_multiplier
  acceleration_multiplier
  turning_multiplier
  friction_or_momentum_rule
  forced_direction_and_speed?

GeometryDefinition
  blocks_movement
  blocks_projectiles
  blocks_vision
  placement_policy
  destructibility_profile?

ConcealmentDefinition
  concealed_subject_tags
  observer_rule
  proximity_reveal_radius?
  action_reveal_duration?
  damage_reveal_duration?
  objective_carrier_override?
  network_visibility_policy
```

Runtime ECS state should describe current overlaps, active effects, reveal windows, geometry state,
and authoritative revisions. Client visual tiles, textures, particles, and shaders resolve stable
presentation IDs and never decide those results.

## Surface and area-effect rules

Before implementing any surface or area, its specification must answer:

- Does it affect fighters, projectiles, deployables, objectives, or some combination?
- Is the effect continuous, periodic, applied on entry, applied on exit, or latched for a duration?
- Do multiple overlapping regions stack, select the strongest value, or use authored priority?
- Does team, owner, immunity, airborne state, or terrain permission filter the effect?
- Does it change maximum speed, acceleration, turning, friction, or apply forced displacement?
- What happens when a region is created or removed around an entity already inside it?
- Is the region static authored data, replicated runtime state, or reconstructable from a stable
  definition and a small activation message?
- Which cue makes the rule readable before the player commits to entering it?
- If client prediction is active, can the same deterministic rule run on the predicted client?

Movement modifiers remain server-authoritative even when predicted for responsiveness. Clients send
movement intent, not the claim that they are standing on a speedway or entitled to a multiplier.

## Concealment gameplay model

Concealment can come from authored grass, a temporary smoke or darkness area, or a fighter effect
such as invisibility. These sources should feed one server-owned visibility decision instead of each
inventing a networking path.

The authoritative server retains the absolute match state, including every fighter, bot,
projectile, effect, objective, and terrain change. For every observer connection and potentially
hidden subject, it derives whether the subject's live spatial entity is network-visible. The client
never declares itself concealed and never decides which opponents it may see.

The first concealment specification must explicitly decide:

- self, ally, enemy, spectator, and defeated-player visibility;
- whether occupants of the same grass region see each other;
- proximity reveal distance and whether walls block that reveal;
- whether firing, using an ability, taking damage, carrying an objective, or colliding reveals;
- reveal duration and re-entry behavior;
- whether projectiles, trails, sounds, damage indicators, health bars, target markers, or status
  effects remain visible while their source is hidden;
- how bots perceive concealed targets;
- how reconnect, late join, interpolation, and future owner prediction behave at visibility edges.

A reasonable initial research baseline is: owners and allies retain visibility; opponents lose it
inside concealment unless an explicit proximity or reveal rule succeeds; attacking reveals for a
short authored duration; objective carriers can opt into a mode-specific reveal override. These are
starting hypotheses, not accepted balance values.

## Network interest management

Lightyear network visibility is the planned mechanism for concealing live entity state from an
opposing client:

- use `RoomPlugin`/`Rooms` for coarse, semi-static interest such as match instances, arena regions,
  or broad spatial partitioning;
- use per-entity, per-connection `VisibilityExt::gain_visibility` and
  `VisibilityExt::lose_visibility` for dynamic observer-specific grass, smoke, reveal, and
  invisibility decisions;
- use the ordinary `WhileVisible` loss behavior for secret spatial entities: a subject hidden
  before first relevance is not spawned, and a previously visible remote entity is despawned;
- do not use retained or always-present visibility for secret live state. Those policies preserve
  a remote entity and its initial or last-known state while pausing updates.

Rooms alone are not the concealment rule. Two opponents can share the same arena room while only one
is currently allowed to observe a subject. The server's visibility system owns that pairwise result
and applies it before replication is assembled for the tick.

### Public identity versus secret spatial state

If the game needs an always-visible roster, the public participant record and the cullable spatial
fighter must not be the same replicated object:

- **Public participant:** stable player ID, team, connection/defeat state, public score, and other
  deliberately non-secret match information.
- **Spatial fighter:** pose, facing, live presentation children, collider representation, private
  effects, and other components that must disappear from an unauthorized client.

Replication hierarchy inheritance must not accidentally expose health bars, targeting markers,
weapon children, effects, or other descendants after the root fighter loses visibility. Conversely,
the owner must retain the controlled entity and input association even when enemies cannot see it.

### Security boundary and limitations

Correct culling prevents an authorized but hostile client from receiving the hidden fighter's
current replicated spatial state, closing the ordinary packet-sniffing wallhack path. It does not
erase previously delivered state or become a complete anti-cheat system.

The design must also audit:

- last-known position and the visible despawn/reappearance transition;
- already delivered or in-flight historical packets around a visibility change;
- projectile, damage, status, targeting, objective, score, sound, and telemetry messages;
- traffic-size or timing side channels if the threat model later justifies padding;
- spectator and replay permissions;
- stale interpolation/prediction state when an entity disappears and returns.

The accurate guarantee is: after authoritative visibility loss is applied before replication,
opposing clients are not sent subsequent hidden spatial state through the normal replication path.

## Required verification for concealment

A milestone that implements concealment should include at least:

- a subject hidden before an opponent joins is never spawned for that opponent;
- a visible subject entering concealment is despawned for opponents but remains present for its
  owner and allowed allies;
- no pose/component updates or subject-derived private messages reach an unauthorized client while
  hidden;
- reveal respawns the subject at current authoritative state without replaying the hidden path;
- proximity, action, damage, objective, and expiry rules transition on exact authoritative ticks;
- two observers can receive different visibility outcomes for the same subject;
- bots use the same gameplay perception rules without requiring a network connection;
- disconnect, reconnect, defeat, respawn, and match restart do not retain stale visibility state;
- replicated descendants cannot leak after the fighter root is culled;
- packet/application-level inspection confirms the intended components and messages are absent;
- public roster information remains correct without exposing the cullable fighter entity.

Visual tests should additionally cover readable grass/smoke boundaries, reveal feedback, pop-in,
audio policy, and whether a player can understand why an opponent became visible.

## Relationship to completed versions

The completed V1 implementation established the smaller gameplay foundation:

- Milestone 03 proves neutral ground, permanent bounds/cover, and authoritative collision.
- Milestone 06 resolves the first readable built-in map recipe, proves recipe/preset/resolved/runtime
  separation, and reserves a destructible region without implementing the player editor.
- Milestone 09 adds an objective region through Hot Zone.
- Milestone 10 proves chunked quantized destructible terrain and recovery across the complete
  supported map-size range; its built-in map retains one visible playtest region, while scale and
  multi-region coverage come from bounded fixtures and process scenarios.

V2 preserved those rules through routed match workers. V3 replaced their client presentation with
3D meshes/scenes while leaving region authority planar. Concealment, speedways, slow/slippery
surfaces, environment hazards, traversal devices, and interactive geometry remain future-version
candidates until intentionally promoted into a roadmap and researched in a milestone specification.

## Research references

- [Maps and game modes](./04-maps-and-game-modes.md)
- [Fighter model](./02-fighter-model.md)
- [Weapons and abilities](./03-weapons-and-abilities.md)
- [Network architecture](./08-network-architecture.md)
- [Version 1 roadmap](./implementation/v1/roadmap.md)
- `references/lightyear/book/src/concepts/advanced_replication/interest_management.md`
- `references/lightyear/examples/network_visibility/README.md`
- `references/lightyear/examples/network_visibility/src/server.rs`
- `references/lightyear/crates/replication/replication/src/visibility/{immediate,room}.rs`
- [Lightyear 0.29 network visibility example](https://github.com/cBournhonesque/lightyear/blob/0.29.0/examples/network_visibility/README.md)
- [Lightyear 0.29 immediate visibility source](https://github.com/cBournhonesque/lightyear/blob/0.29.0/crates/replication/replication/src/visibility/immediate.rs)
- [Lightyear 0.29 room visibility source](https://github.com/cBournhonesque/lightyear/blob/0.29.0/crates/replication/replication/src/visibility/room.rs)
