# Maps and game modes

## Map model

A map recipe should contain:

- playable bounds;
- static walls and cover;
- destructible geometry;
- walkable and blocked surfaces;
- visual tile/decal layers and decorative entity placements through stable presentation IDs;
- spawn points;
- mode-required objective points, anchors, or volumes;
- pickup spawn rules;
- hazards;
- visibility or concealment regions;
- a compatible server-owned mode definition ID and only the bounded map parameters that mode's
  schema explicitly exposes.

Keep these authoring levels distinct:

```text
MapContentCatalog       developer-authored tiles, entities, regions, geometry primitives, bounds
MapRecipe               user-authored arrangement of allowed content
MapPreset               a named, developer-authored legal recipe
ResolvedMap             immutable server-validated match snapshot
MapRuntimeState         spawned entities, active regions, terrain revisions, objective state
ModeDefinition/Plugin   developer-authored server rules; never authored by the map recipe
```

The first v1 arena is a built-in preset, but it must flow through the same versioned typed recipe
parser/serializer, validator, resolver, and runtime-instantiation path intended for future
user-authored maps. Map systems must not branch on the first map's identity, and a serialized ECS
`World` is not the authored source of truth.

## Player map-builder boundary

The eventual map builder may let a user:

- select and arrange approved visual tiles, decals, decorations, and presentation themes;
- shape playable bounds and place bounded permanent or destructible geometry;
- place approved props, pickups, hazards, and other map-entity definitions;
- place and configure supported gameplay regions;
- place team, free-for-all, or other mode-required spawn points;
- place objective anchors/volumes required by the selected built-in mode;
- edit only those sizes, orientations, counts, and parameters exposed by the authoritative catalog.

It does not let a user provide Rust, scripts, systems, arbitrary component data, executable rules,
new game modes, or unrestricted asset/network references. Choosing `Wipeout`, `Hot Zone`,
`Showdown`, or another supported mode selects a developer-authored server rules plugin and schema;
the recipe supplies compatible layout data only.

Server resolution must validate at least schema/revision compatibility, stable IDs, finite and
bounded coordinates, map dimensions, geometry complexity, collider/region counts, allowed entity
and presentation references, spawn safety/counts, required objective anchors, mode compatibility,
destructible-terrain limits, and performance budgets. Competitive fairness checks, asset upload and
licensing, persistence, publishing, discovery, moderation, and version migration belong to later
specifications and must not be folded into collision or mode systems.

Milestone 06 establishes the typed recipe/resolved/runtime boundaries with one built-in arena and a
non-preset validation fixture. It does not implement the player-facing editor. A future builder
edits a candidate recipe; the server remains responsible for accepting and resolving it before a
match can use it.

Mode-facing validation grows with implemented modes. Milestone 06 needs only the sandbox/base map
requirements and a stable place for mode layouts; Milestone 07 adds Wipeout's concrete requirements,
Milestone 09 adds Hot Zone's, and later modes add their own schemas without making map recipes
executable or requiring a universal mode trait in advance.

## Visual tiles and gameplay regions

Visual tiles are replaceable client presentation. Authoritative gameplay should be composed from
geometry, authored regions, runtime environment entities, and destructible-terrain data instead of
giving every floor or wall sprite a bespoke rule.

- ordinary ground is a walkable surface without a modifier;
- permanent walls and cover are blocking geometry;
- destructible terrain uses a chunked quantized occupancy grid and generated collision;
- speedways, slow ground, hazards, objectives, and concealment are shaped gameplay regions;
- smoke, temporary walls, and similar ability-created areas are server-owned runtime entities;
- decorative grass, puddles, decals, and props have no gameplay effect unless map data explicitly
  associates them with a region or geometry definition.

A user-authored recipe may arrange both visual and gameplay layers, but presentation never implies
collision or an effect. The recipe must reference each gameplay shape/region explicitly, and the
headless server resolves those references without loading textures or other client assets.

See [Environment, surface, and tile ideas](./09-environment-and-tile-ideas.md) for the future-facing
catalog, property model, and promotion rules. That catalog is research, not automatic v1 scope.

## Concealment regions

The map model reserves visibility and concealment regions for future tall grass, bushes, smoke,
darkness, and invisibility effects. All sources should feed one server-owned observer-versus-subject
visibility decision. Clients do not declare themselves concealed and do not decide which opponents
they may observe.

Static concealment geometry can be known to every client; the hidden fighter's live spatial state
is the secret. The server retains the complete simulation, including bots, and applies per-client
network visibility before replication. Match/arena rooms are suitable coarse interest partitions,
while dynamic grass, reveal, and invisibility decisions require per-entity, per-connection
visibility. Public roster identity must remain separate from cullable live fighter state when both
behaviors are needed.

The implementing milestone must define proximity, ally, attack, damage, objective-carrier,
spectator, projectile, audio, and reappearance rules and verify that related components or messages
cannot leak the hidden state. See [Network architecture](./08-network-architecture.md#interest-management-and-concealment)
for the transport contract.

## Destructible terrain

Destruction should not be implemented as replacing individual visible tiles. Brawler will use a
chunked, quantized occupancy grid that supports readable holes and passages at a deliberately chosen
gameplay resolution. Grid cells are hidden simulation data; client visuals may present or soften the
quantized edge without becoming collision truth.

```text
Terrain appearance
        +
Quantized occupancy grid
        +
Generated collision
```

### Terrain representation

- Store authoritative terrain solidity as `[u64; 16]` occupied/empty bitsets owned by the server
  `World`, using 8-world-unit cells and sparse 32×32-cell chunks.
- Derive global chunk coordinates from world space with Euclidean floor division; the grid resolution
  does not vary with map or destructible-region dimensions.
- Convert circular, capsule, rectangular, or authored brushes to integer cell operations so the
  authoritative result is deterministic.
- Update dirty client presentation regions independently from authoritative collision generation.
- Generate bounded collision with one Avian/Parry voxel shape per occupied chunk, reconciling
  orthogonal-neighbor topology across chunk seams. Do not create one replicated entity per cell.

The [v1 Milestone 10 technical specification](./implementation/v1/milestone-10.md) selects and
budgets the cell size, chunk dimensions, bitset layout, half-cell brush quantization, client image
updates, voxel collision generation, and collider replacement. Smooth mask carving and
marching-squares contours remain deferred unless playtest evidence rejects the quantized result.

The initial occupancy state, allowed material/brush profiles, placement bounds, and stable
terrain-chunk IDs
are authorable map-recipe data. Runtime destruction, collision regeneration, and terrain revision
remain server-owned match state; a map author cannot directly publish a runtime revision or crater
event.

For a historical reference, [Spell-Splosion](https://github.com/MitchMakesThings/Spell-Splosion)
demonstrates several Worms-style terrain-destruction techniques in an older Godot project. It remains
useful alternative evidence for smooth carving, but its mask workflow, engine APIs, and scene
structure are not Brawler implementation requirements.

### Chunking

The occupancy grid is divided into fixed-cell-count implementation chunks. Chunk coordinates are
derived from world space, so maps and destructible regions may vary in size without changing terrain
resolution. Allocate only chunks intersecting authored destructible regions; do not allocate a
whole-map grid when most of the map is permanent terrain. An explosion rebuilds only chunks touched
by its quantized brush.

The implementation must support the complete engine-owned playable-size range (currently
1024–4096 units wide and 720–3072 units high), rather than deriving capacity from the current
192×192-unit Crossroads reservation. Every otherwise-valid map size may host destructible regions.
The current limits allow up to four reservations and authored shapes up to 2048 units across. The
Milestone 10 specification supports those per-region limits while adding aggregate ceilings of 221
intersected chunks and 196,608 occupied cells.

Engine-owned budgets must bound total allocated destructible cells/chunks, dirty chunks rebuilt per
fixed tick, generated collision complexity per chunk, recovery snapshot bytes, and destruction work
accepted per tick. Validation must reject recipes that exceed any new aggregate terrain budget.
Terrain concurrency derives from the resolved game-mode/map participant capacity, not from the
current Wipeout implementation's temporary 2v2 limit. The v1 terrain format is verified for up to 24
simultaneously active fighters and does not encode how they are divided among teams.

Stable chunk identity links separate runtime representations:

- **Authoritative server chunk:** occupancy bits, generated collision, terrain revision, and collision
  dirty/rebuild state.
- **Client presentation chunk:** visual/material region and visual dirty/upload state derived from
  replicated terrain state or recovery data.

The dedicated server does not own or upload terrain textures. A client visual update cannot change authoritative solidity or collision.

### MVP destruction scope

The first destruction milestone includes:

- one clearly marked destructible region in the built-in map plus fixtures covering multiple chunks,
  separated regions, and the supported map-size extremes;
- circular explosion brushes;
- quantized visual holes, passages, and readable crater edges;
- projectile and fighter collision against the generated terrain;
- collision regeneration between physics frames;
- basic unstuck behavior when a fighter is embedded by a terrain change.

The small built-in reservation is a playtest scenario, not a capacity target. Milestone completion
requires bounded automated and process evidence across the full supported map-size range and at the
legal destructible-region limits.

Defer terrain deformation animation, falling debris, material layers, fluid behavior, structural collapse, persistent terrain saving, and internet-scale terrain bandwidth optimization. Terrain authority and basic event synchronization remain part of the network architecture.

Keep terrain collision separate from indestructible walls, fighters, projectiles, objectives, pickups, hazards, and decorative props.

## Map grammar

Useful geometry archetypes:

- **open arena:** emphasizes aim and range;
- **chokepoint arena:** emphasizes area denial and crowd control;
- **lane arena:** gives teams predictable routes;
- **cover maze:** enables ambushes and close-range play;
- **central objective arena:** concentrates conflict at a contested location.

The first test map should be symmetrical and intentionally plain:

- rectangular bounds;
- two team spawn areas;
- central open fight area;
- two side routes;
- permanent cover;
- one clearly marked destructible terrain region;
- no water, bushes, teleporters, or moving hazards.

This makes weapon and build differences easier to observe while providing a contained test area for flexible terrain destruction.

## Match topology and capacity ownership

Team count and participants per team are properties of the selected game-mode and map composition,
not global constants and not terrain rules. A game-mode definition supplies the legal topology and
per-team participant ranges. A map supplies compatible team slots, spawn areas/points, playable
space, and any mode-required anchors. Resolution accepts only their compatible intersection and
produces the maximum active-fighter count used by admission and subsystem capacity checks.

The current Wipeout/Hot Zone server code supports two teams with at most two participants per team;
that is an implementation-stage profile, not the engine direction. Common IDs, map indexing,
networking, terrain, and future mode-neutral systems must not bake in 2v2 or exactly two teams. The
planned range includes ordinary 3v3 and larger arrangements such as `1v1 × 12`, `2v5 × 2`, and
`3v3 × 3`. Each concrete game mode still owns whether those topologies make sense for its scoring,
respawn, objective, HUD, and matchmaking rules.

## Mode inventory

### Showdown

Survival mode. Fighters or teams fight until one remains. A complete implementation typically needs elimination state, optional respawn rules, pickups, and a closing danger area to prevent indefinite matches.

**Map needs:** distributed spawn points, exploration space, cover, pickup locations, and a late-game boundary mechanic.

**Complexity:** high. It combines free-for-all participant tracking, no-respawn elimination, loot progression, and a shrinking playable area.

### Wipeout

Team elimination score mode. Each enemy defeat grants a point; the first team to the target score wins, or the highest score wins when the timer expires. Fighters normally return to the match after a respawn delay.

**Map needs:** team spawn areas, safe re-entry routes, enough cover to prevent spawn trapping, and no mandatory objective geometry.

**Complexity:** low. This is the best first complete mode because it validates combat, teams, death, respawn, scoring, and match end.

### Gem Grab

Teams contest periodically spawned collectibles. Carrying the objective creates a risk/reward state: the carrier is valuable, visible, and loses carried items on defeat. A team win requires reaching a threshold and surviving a countdown or hold period.

**Map needs:** a contested central spawn area, routes around the objective, carrier escape paths, and clear pickup/drop readability.

**Complexity:** medium.

### Heist

Teams attack the opposing team's durable objective while defending their own. The match ends when one objective is destroyed or time expires with one objective having more health remaining.

**Map needs:** two objective locations, attack lanes, defensive cover, and routes that support both pushing and returning to defense.

**Complexity:** medium-low. Objective damage may require separate balance rules from fighter damage.

### Hot Zone

Teams contest one or more capture areas. Progress increases while a team occupies a zone and is paused or contested when both teams are present. The first team to complete the required progress wins; otherwise the leading team wins when time expires.

**Map needs:** one or more capture volumes, approach routes, cover around zones, and enough geometry to make zone entry a decision.

**Complexity:** medium. Continuous occupancy, progress, simultaneous contesting, and timeout tie handling all need clear rules.

## Recommended implementation order

1. **Combat sandbox** — no formal mode; reset quickly after death.
2. **Wipeout** — validates the full combat loop.
3. **Hot Zone** — validates the same combat code under continuous spatial progress early.
4. **Heist** — adds a persistent objective without item ownership.
5. **Gem Grab** — adds pickups, carrier state, drops, and a win countdown.
6. **Solo Showdown** — adds exploration, loot, and a shrinking boundary.
7. **Duo/trio variants** — reuse team and respawn infrastructure.

Hot Zone is intentionally earlier than its isolated rules complexity would otherwise suggest. It is part of the gameplay MVP verification because it proves that fighter, weapon, ability, and match-lifecycle code is not coupled to elimination scoring. The remaining order is an engineering recommendation, not a statement about which mode is most important to the final game.

## Bevy mode composition

Keep mode rules out of fighter and weapon systems, but do not require an object-oriented `GameMode` trait. A mode should be composed from the smallest Bevy-native pieces its rules need:

- a focused rule plugin, such as `WipeoutRulesPlugin` or `HotZoneRulesPlugin`;
- authored mode configuration and stable definition identity;
- match-phase state or resources for waiting, countdown, active play, completion, and restart;
- mode-owned resources/components for scores, objectives, timers, progress, and participant state;
- fixed-step systems or observers that consume authoritative gameplay facts such as fighter defeat, disconnect, or objective occupancy;
- client presentation systems that observe replicated mode state and render the scoreboard, timer, objective progress, and results.

Common match lifecycle components, resources, and systems should emerge from implementing Wipeout and be reused by Hot Zone where their behavior is truly identical. Mode-specific rule plugins may use specialized systems. Introduce a shared trait, registry, or generic mode abstraction only after multiple implemented modes demonstrate a concrete need that plugin composition and ECS queries do not already solve.

A mode plugin owns the validation schema for its map-facing requirements. For example, Wipeout may
require safe team spawns, Hot Zone a bounded capture volume, and Showdown distributed spawns plus a
boundary-compatible play area. A map recipe supplies these placements but cannot change what they
mean, how scoring advances, or how victory is decided.
