# Map and mode specification

## Purpose and authority

This document defines the durable contract for authored maps, server-resolved map snapshots,
runtime map state, game-mode rules, and their composition into a match. It also defines the bounded
authoring boundary for a future player-facing map builder.

[Product direction](./00-product-direction.md) owns why player-authored arenas matter.
[Network architecture](./08-network-architecture.md) owns replication and visibility.
[Art, presentation, and asset specification](./11-art-and-presentation-direction.md) owns client rendering.
Version roadmaps and milestones own delivery history, evidence, and temporary implementation stages.

The central rule is:

> A map supplies validated space and mode-compatible layout data. A developer-authored mode supplies
> executable rules. Neither presentation nor map data becomes gameplay authority.

## Authored, resolved, and runtime layers

Keep these layers distinct:

```text
MapContentCatalog          allowed objects, regions, geometry, themes, and authoring limits
MapRecipe                  one bounded, mode-compatible authored layout
MapPreset                  a named developer-authored legal recipe
ResolvedMap                immutable server-validated match snapshot
MapRuntimeState            instantiated geometry, regions, entities, and terrain revisions

ModeDefinition             stable developer-authored rule identity and map-facing schema
ModeConfiguration          bounded server-operator values for one advertised game type
ResolvedMatchComposition   compatible map, mode, topology, capacity, and rules snapshot
ModeRuntimeState           phase, score, objective progress, timers, and mode-owned participant state
```

A serialized ECS `World` is not authored map data. The server lowers a recipe into runtime entities
and resources; the client independently derives presentation from the resolved snapshot and stable
presentation IDs.

Each map recipe targets one mode definition because its spawns and objective anchors must satisfy a
specific schema. A visual arena, semantic object arrangement, or presentation theme may be reused
across several recipes, but a Wipeout-compatible recipe and a Hot Zone-compatible recipe remain
independently validated documents. Selecting a map therefore means selecting a compatible recipe,
not attaching arbitrary rules to an unvalidated scene.

Built-in and future player-authored recipes use the same parser, validator, resolver, and runtime
installation path. Systems must not branch on a built-in map's identity.

## Map recipe contract

The supported recipe model can express:

- stable recipe identity and revision;
- playable bounds;
- camera/presentation bounds that do not alter authoritative collision;
- a presentation theme and semantic visual-object placements;
- permanent blocking geometry and walkable space;
- destructible-terrain reservations and initial occupancy;
- spawn points and team slots;
- the compatible mode definition;
- mode-required point or area anchors;
- bounded gameplay regions and server-known environment entities supported by the active catalog.

The schema may grow to support additional approved pickups, hazards, concealment regions, movement
surfaces, and other environment primitives. Adding a catalog capability is an explicit content and
gameplay decision; unknown fields, IDs, or executable behavior are rejected rather than interpreted
dynamically.

Presentation and gameplay layers are independent. A visible wall does not block movement unless the
recipe resolves blocking geometry. A decorative puddle does not slow a fighter unless the recipe
also references a supported gameplay region. Conversely, invisible authoring helpers may describe
spawns, objectives, collision, or occupancy without becoming rendered objects.

## Resolution and validation

The authoritative server resolves a candidate recipe before admission can use it. Validation must
cover at least:

- schema and catalog compatibility;
- stable identities and allowed references;
- finite, bounded coordinates, dimensions, orientations, and counts;
- playable bounds and geometry complexity;
- collider, region, semantic-object, and entity budgets;
- spawn count, team capacity, safety, and reachable re-entry space;
- required mode anchors and rejection of unsupported anchors;
- mode compatibility and topology intersection;
- destructible-terrain allocation and mutation ceilings;
- headless operation without client assets;
- deterministic identity and content fingerprints needed by admission and recovery.

Resolution produces an immutable `ResolvedMap` and contributes to a
`ResolvedMatchComposition`. Mutable runtime state never flows back into the authored recipe. A map
author cannot publish a terrain revision, objective score, fighter position, or other live match
fact.

Competitive fairness analysis, asset licensing, persistence, publishing, discovery, moderation,
and version migration are separate product concerns. They may gate whether a valid recipe can be
distributed or selected, but they must not be hidden inside collision or mode systems.

## Player map-builder boundary

An envisioned map builder may let a player:

- arrange approved themes, surfaces, decorations, and semantic objects;
- shape bounded playable space and permanent or destructible geometry;
- place approved environment entities and gameplay regions;
- place spawn points and team slots supported by the selected mode;
- place and configure the selected mode's required anchors;
- edit only parameters and ranges exposed by the authoritative catalog;
- validate and playtest a candidate recipe before saving or publishing it.

It does not accept Rust, scripts, systems, arbitrary component data, new game modes, unrestricted
assets, network references, or executable objective logic. Choosing Wipeout, Hot Zone, or another
supported mode selects developer-authored rules and a validation schema; the player authors only a
compatible layout.

The builder is an external authoring surface, not a second map representation. Editor state may
include selections, handles, undo history, and invalid intermediate geometry, but only an accepted
recipe may enter match resolution.

## Runtime ownership

The authoritative map runtime owns:

- installed permanent geometry and collision;
- spawn and mode-anchor indexes derived from the resolved map;
- gameplay-region and environment-entity instances;
- destructible occupancy, colliders, revisions, and rebuild work;
- installation, reset, recovery, and teardown of those map-owned facts.

The authoritative mode runtime owns:

- match phase and transition rules;
- scoring, victory, timeout, and tie behavior;
- objective progress and mode-specific timers;
- respawn, elimination, or round policy;
- mode-specific participant state and outcome summaries.

Mode systems consume stable map indexes and authoritative gameplay outcomes such as defeats or
objective occupancy. They do not mutate authored recipes. Map systems expose compatible space but
do not decide what a capture zone scores or when a match ends.

Client presentation observes resolved and replicated facts. It may render geometry, terrain,
objective volumes, timers, scores, and results, but it does not decide collision, occupancy,
visibility, scoring, or victory.

## Visual presentation and gameplay space

Authoritative gameplay is planar and composed from exact geometry, regions, environment entities,
and terrain occupancy. Client 3D presentation resolves those facts to cached or generated meshes and
validated presentation profiles. Visual height, animation, material, lighting, or decorative
placement never changes authoritative collision or occupancy.

Useful gameplay-space primitives include:

- ordinary walkable ground;
- permanent walls and cover;
- destructible terrain;
- shaped movement, hazard, and concealment regions;
- mode-owned objective areas derived from validated anchors;
- server-owned runtime areas such as smoke or temporary barriers;
- server-known pickups or interactables when a supported mode or content definition owns them.

See [Environment gameplay direction](./09-environment-gameplay.md) for the candidate catalog and
promotion rules. An idea becomes part of this contract only when a concrete feature adopts and
validates it.

## Destructible terrain contract

Destructible solidity is a server-owned, chunked, quantized occupancy grid rather than the presence
of visible tiles:

```text
Terrain appearance
        +
Quantized occupancy grid
        +
Generated collision
```

The supported representation uses 8-world-unit cells, sparse 32×32-cell chunks, and `[u64; 16]`
occupied/empty bitsets per chunk. Global chunk coordinates use Euclidean floor division, so terrain
resolution does not vary with map size or destructible-region dimensions.

Authoritative terrain rules must:

- convert supported brushes to deterministic integer cell operations;
- allocate only chunks intersecting authored destructible regions;
- rebuild only dirty collision chunks between physics frames;
- reconcile orthogonal-neighbor topology across chunk seams;
- generate bounded chunk collision rather than one entity per cell;
- keep permanent geometry, fighters, projectiles, objectives, pickups, and props separate;
- recover a current snapshot without replaying unbounded destruction history;
- provide basic correction when a terrain mutation embeds a fighter.

The current engine bounds playable maps from 1024–4096 units wide and 720–3072 units high.
Destructible shapes may be up to 2048 units across, with at most four reservations, 221 intersected
chunks, and 196,608 occupied cells. These are validation limits of the supported format, not targets
derived from one built-in arena. Any revision must continue bounding allocation, accepted mutation
work, dirty rebuilds, collision complexity, and recovery bytes.

The authoritative server owns occupancy, collision, revisions, and rebuild state. The client owns
only visual mesh/material state derived from replicated terrain facts. Deformation animation,
falling debris, layered materials, fluids, structural collapse, persistent deformation, and broader
bandwidth strategies are envisioned extensions rather than implied behavior of the base grid.

## Concealment and visibility extension

The model reserves concealment regions for tall grass, bushes, smoke, darkness, invisibility, and
similar mechanics. All sources must feed one server-owned observer-versus-subject visibility
decision. Clients do not declare themselves concealed or decide which opponents they may observe.

Static concealment geometry may be public while a hidden fighter's live spatial state remains
private. Implementing concealment requires explicit proximity, ally, attack, damage,
objective-carrier, spectator, projectile, audio, and reappearance rules, plus verification that
related replication cannot leak hidden state. See
[Network architecture](./08-network-architecture.md#interest-management-and-concealment).

## Match topology and capacity

Team count and participants per team belong to the resolved mode/map composition, not to global
constants or terrain. A mode definition declares legal topology and participant ranges. A map proves
compatible team slots, spawn capacity, playable space, and required anchors. Resolution accepts only
their intersection and publishes the maximum active-fighter count used by admission and subsystem
capacity checks.

Supported routed product game types include exact 1v1, 2v2, and 3v3 Wipeout and Hot Zone matches.
Ordinary matches are expected to center on 3v3, while the architecture may support larger bounded
arrangements such as twelve solo fighters, two teams of five, or three teams of three when a concrete
mode, map, HUD, admission profile, and capacity evidence all allow it. No common subsystem may assume
exactly two teams or use one advertised topology as an engine-wide ceiling.

## Supported modes

### Wipeout

Wipeout is a team elimination-score mode. An enemy defeat grants a point; the first team to the
target wins, or the leading team wins when time expires. Fighters normally re-enter after a
server-owned respawn delay.

Its map schema requires compatible team spawns and safe re-entry space but no objective geometry.
Its runtime owns scores, timeout resolution, respawn policy, victory, and the match summary.

### Hot Zone

Hot Zone is a spatial-control mode. Teams contest one or more capture areas. Server-owned progress
advances according to occupancy and contest rules; the first team to complete the requirement wins,
or timeout rules resolve the result.

Its map schema requires bounded capture volumes, compatible spawns, and enough legal surrounding
space for entry and contest. Its runtime owns occupancy evaluation, progress, contest state, timeout
resolution, victory, and the match summary.

Together these modes prove that combat, fighter lifecycle, map installation, and common match phases
are not coupled to one scoring model.

## Envisioned mode families

These are product candidates, not supported rules merely because their layouts can be described:

- **Heist:** teams attack an opposing durable objective while defending their own. It adds objective
  health, attack/defense lanes, and objective-specific damage policy.
- **Gem Grab:** teams collect and carry a contested resource. It adds spawn cadence, carrier state,
  drops, visibility pressure, and a threshold/hold win sequence.
- **Showdown:** solo fighters or teams survive until one remains. It adds elimination, distributed
  spawning, exploration/loot pressure, and a closing playable boundary.

A new mode is introduced as a complete player-visible loop, not as a generic framework exercise. It
must define its authoritative rules, configuration, map schema, topology, HUD/results facts,
admission compatibility, recovery behavior, and verification evidence. Shared match machinery is
extracted only where implemented modes have genuinely identical ownership and behavior.

## Bevy mode composition

Mode rules use focused Bevy-native composition rather than an object-oriented universal `GameMode`
trait:

- one focused rule plugin for each supported mode;
- stable definitions and validated rule resources;
- common match-phase components and systems only where behavior is truly shared;
- mode-owned resources/components for scores, objectives, timers, and participant facts;
- explicit fixed-step ordering around authoritative movement, combat outcomes, occupancy, scoring,
  and completion;
- replicated state and cues observed by client HUD and world presentation systems.

Introduce a shared registry, trait, or generic mode layer only after concrete modes demonstrate a
problem that plugin composition, typed definitions, and ECS queries do not solve.

## Map design principles

Map layouts should produce readable choices rather than visual complexity for its own sake. Useful
archetypes include open arenas for aim and range, chokepoints for denial, lanes for predictable
rotations, cover networks for ambushes, and central-objective layouts for concentrated conflict.

Every supported map should:

- give each weapon range profile meaningful opportunities and counterplay;
- provide safe, legible spawn or re-entry routes appropriate to its mode;
- make blocking, destructible, hazardous, and objective space visually distinguishable;
- keep important combat silhouettes and projectiles readable against its theme;
- avoid accidental dead space or indefinite disengagement;
- remain valid and bounded under its advertised participant topologies.
