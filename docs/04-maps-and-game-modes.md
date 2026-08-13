# Maps and game modes

## Map model

A map definition should contain:

- playable bounds;
- static walls and cover;
- destructible geometry;
- walkable and blocked surfaces;
- spawn points;
- objective points or volumes;
- pickup spawn rules;
- hazards;
- visibility or concealment regions;
- mode-specific parameters.

## Destructible terrain

Destruction should not be implemented as replacing individual visible tiles. Brawler will use a flexible, mask-backed terrain system suitable for arbitrary craters, tunnels, and cutouts.

```text
Terrain appearance
        +
Destruction mask
        +
Generated collision polygons
```

### Terrain representation

- Store terrain solidity in an `Image` or `BitMap` mask.
- Treat filled mask pixels as solid and erased pixels as empty.
- Apply circular, capsule, rectangular, or authored brush shapes for explosions and digging.
- Update the visible terrain texture independently from the physics collision.
- Generate collision outlines from the modified mask using marching squares and polygon simplification.

Godot's `BitMap.opaque_to_polygons()` is suitable for the mask-to-polygon step. `Geometry2D` can support polygon subtraction, clipping, and convex decomposition when the terrain system needs more explicit geometry operations.

For a practical reference implementation, see [Spell-Splosion](https://github.com/MitchMakesThings/Spell-Splosion), an older Godot 3 GDScript project demonstrating several Worms-style terrain-destruction techniques. It is useful for understanding the basic terrain-mask and collision workflow, but Brawler should adapt the idea to Godot 4, chunk dirty-region updates, and our own rendering and collision boundaries.

### Chunking

The mask should be divided into implementation chunks, such as 128×128 or 256×256 mask pixels. Chunks are an internal optimization, not gameplay tiles. An explosion should rebuild only the chunks touched by its brush instead of regenerating the entire map collision.

Each terrain chunk owns:

- its visual mask region;
- its material/terrain texture region;
- its generated collision polygons;
- dirty state and queued rebuild state.

### MVP destruction scope

The first destruction prototype includes:

- one destructible terrain chunk;
- circular explosion brushes;
- visual holes and crater edges;
- projectile and fighter collision against the generated terrain;
- collision regeneration between physics frames;
- basic unstuck behavior when a fighter is embedded by a terrain change.

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
3. **Heist** — adds a persistent objective without item ownership.
4. **Gem Grab** — adds pickups, carrier state, drops, and a win countdown.
5. **Hot Zone** — adds continuous spatial progress.
6. **Solo Showdown** — adds exploration, loot, and a shrinking boundary.
7. **Duo/trio variants** — reuse team and respawn infrastructure.

This order is an engineering recommendation, not a statement about which mode is most important to the final game.

## Mode interface

Keep mode rules independent from fighter and weapon code:

```text
GameMode
  create_match()
  on_fighter_spawned()
  on_fighter_defeated()
  on_pickup_collected()
  on_objective_damaged()
  tick(delta)
  is_match_over()
  get_scoreboard()
```
