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

## Visual tiles and gameplay regions

Visual tiles are replaceable client presentation. Authoritative gameplay should be composed from
geometry, authored regions, runtime environment entities, and destructible-terrain data instead of
giving every floor or wall sprite a bespoke rule.

- ordinary ground is a walkable surface without a modifier;
- permanent walls and cover are blocking geometry;
- destructible terrain uses mask-backed solidity and generated collision;
- speedways, slow ground, hazards, objectives, and concealment are shaped gameplay regions;
- smoke, temporary walls, and similar ability-created areas are server-owned runtime entities;
- decorative grass, puddles, decals, and props have no gameplay effect unless map data explicitly
  associates them with a region or geometry definition.

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

Destruction should not be implemented as replacing individual visible tiles. Brawler will use a flexible, mask-backed terrain system suitable for arbitrary craters, tunnels, and cutouts.

```text
Terrain appearance
        +
Destruction mask
        +
Generated collision polygons
```

### Terrain representation

- Store authoritative terrain solidity in a CPU-side image mask, bitset, or similarly compact resource/component representation owned by the server `World`.
- Treat filled mask pixels as solid and erased pixels as empty.
- Apply circular, capsule, rectangular, or authored brush shapes for explosions and digging.
- Update dirty regions of the client-visible Bevy image/texture independently from authoritative collision generation.
- Generate collision outlines from the modified mask using marching squares and polygon simplification.

The v1 Milestone 10 technical design must choose and validate the Bevy/Rust implementation for mask storage, dirty texture uploads, contour generation, simplification, and collider replacement. This may combine small maintained crates with project-owned code; do not adopt a large terrain framework merely to avoid a focused algorithm.

For a historical reference, [Spell-Splosion](https://github.com/MitchMakesThings/Spell-Splosion) demonstrates several Worms-style terrain-destruction techniques in an older Godot project. Its mask-to-visual-to-collision workflow is conceptually useful, but its engine APIs and scene structure are not Brawler implementation dependencies.

### Chunking

The mask should be divided into implementation chunks, such as 128×128 or 256×256 mask pixels. Chunks are an internal optimization, not gameplay tiles. An explosion should rebuild only the chunks touched by its brush instead of regenerating the entire map collision.

Stable chunk identity links separate runtime representations:

- **Authoritative server chunk:** solidity data, generated collision, terrain revision, and collision dirty/rebuild state.
- **Client presentation chunk:** visual mask/material region and visual dirty/upload state derived from replicated terrain state or recovery data.

The dedicated server does not own or upload terrain textures. A client visual update cannot change authoritative solidity or collision.

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
