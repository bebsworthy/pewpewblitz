# Brawler design documentation

Brawler is an original, cross-platform top-down arena shooter built around player-authored
brawlers. Players compose a bounded primary-weapon recipe and combine it with body choices,
abilities, and equipment rather than selecting a fixed hero with a fixed kit.

The project is intentionally starting with the gameplay loop. Production matchmaking, account services, monetization, cosmetics, live operations, and release engineering are out of scope initially. Core networking and the server-authoritative simulation are in scope from the beginning.

V3 is the active version. It is migrating the client gameplay world from 2D sprites/meshes to a
fixed orthographic 3D presentation while retaining the completed V2 product flow and routed server
architecture and the existing 2D authoritative simulation.

## Documents

- [Product direction](./00-product-direction.md) — vision, differentiation, principles, and non-goals.
- [Engine decision](./01-engine-decision.md) — Bevy/Rust recommendation, networking stack, and technical constraints.
- [Fighter model](./02-fighter-model.md) — attributes, derived values, loadout rules, and runtime state.
- [Weapons and abilities](./03-weapons-and-abilities.md) — weapon primitives, payloads, and initial content set.
- [Maps and game modes](./04-maps-and-game-modes.md) — map grammar and the five planned mode families.
- [Gameplay MVP](./05-gameplay-mvp.md) — the smallest playable slice, milestones, and acceptance criteria.
- [Research sources](./06-research-sources.md) — external references used for the baseline.
- [MVP asset shortlist](./07-mvp-asset-shortlist.md) — open/licensed stand-in tilemaps, characters, props, and icons.
- [Network architecture](./08-network-architecture.md) — authority model, global application-protocol evolution, Lightyear replication, Bevy server/client world composition, and local network testing.
- [Environment, surface, and tile ideas](./09-environment-and-tile-ideas.md) — future-facing environment catalog, composable region properties, concealment, and network interest management.
- [Bots](./10-bots.md) — decision record for player-filling bots as external headless clients, first-version scope, and open questions.
- [Art and presentation direction](./11-art-and-presentation-direction.md) — superseded 2D art proposal whose enduring readability and authority boundaries carry into V3.
- [Sprite inventory](./12-sprite-inventory.md) — historical inventory for the superseded 2D proposal.
- [Player UX and server-local matchmaking](./13-player-ux.md) — completed V2 player flow, queues, build selection, settings, accessibility, and verification decisions.
- [Multi-process server and single-port UDP/IPC transport](./14-multiplayer-server-architecture.md) — completed V2 supervisor, routed transport, isolated match-worker, and connection-handoff decisions.
- [Version 1 implementation roadmap](./implementation/v1/roadmap.md) — completed gameplay MVP milestones and closeout.
- [Version 2 implementation roadmap](./implementation/v2/roadmap.md) — completed product UX, routed transport, matchmaking, concurrent workers, and closeout.
- [Version 3 implementation roadmap](./implementation/v3/roadmap.md) — active 3D gameplay-world presentation migration.
- [V3 M01 — 3D presentation feasibility and foundation](./implementation/v3/milestone-01.md) — completed feasibility foundation and accepted learning review.
- [V3 M02 — default 3D arena, map, terrain, camera, and input cutover](./implementation/v3/milestone-02.md) — current user-playtest contract.

## Working vocabulary

- **Brawler:** a player-authored, potentially persistent fighter configuration in the player's
  arsenal.
- **Fighter:** the server-owned in-match combatant instantiated from a validated brawler build.
- **Weapon recipe:** the chosen behavior and bounded specifications for a brawler's primary weapon.
- **Build:** the brawler's weapon recipe, ability, body choices, and equipment.
- **Preset:** a developer-authored legal recipe/build used for onboarding, testing, or quick choice;
  it is not a separate combat implementation.
- **Map recipe:** a bounded arrangement of presentation layers, geometry, terrain, entities,
  regions, spawn points, and mode-required anchors; built-in maps are preset recipes.
- **Mode:** a server-owned rules implementation selected for a compatible map; players may lay out
  its required anchors but do not author the rules.
- **Item:** an equipable passive or active modifier.
- **Payload:** the gameplay effect produced by an attack or ability.
- **Map:** geometry, spawn points, objectives, hazards, and interactables.
- **Gameplay region:** an authored or runtime area that applies movement, concealment, hazard, objective, or other server-owned rules independently from its visual tiles.
