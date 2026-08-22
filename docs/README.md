# Brawler design documentation

Brawler is an original, cross-platform top-down arena shooter built around player-authored
brawlers. Players choose bounded primary-weapon behavior and specifications that the server resolves
into an operational recipe, then combine them with body choices, abilities, and equipment rather
than selecting a fixed hero with a fixed kit.

The product centers a readable combat loop, short objective matches, and bounded build learning.
Practice, multiplayer, arsenal, creator, progression, and social loops extend that core without
creating alternate gameplay authority; server-owned validation and simulation remain foundational.

V3 completed on 2026-08-20. V4 completed on 2026-08-21, evolving the fixed-camera 3D presentation
with restrained perspective while retaining the completed V2 product flow/routed architecture and
planar authoritative simulation. Independently embedded map documents, semantic placement lowering,
two reusable themes, and catalog-backed routed admission now form the current foundation. The map
editor remains deferred to the root backlog.

## Documents

- [Product direction](./00-product-direction.md) — vision, differentiation, principles, and non-goals.
- [Engine specification](./01-engine-decision.md) — supported Bevy/Rust baseline, runtime roles,
  application composition, scheduling, physics, presentation, and feature boundaries.
- [Fighter and build specification](./02-fighter-model.md) — build selection, authoritative
  resolution, fighter attributes and runtime, statuses, arsenal, and equipment boundaries.
- [Weapons and abilities specification](./03-weapons-and-abilities.md) — combat authoring layers,
  weapon recipes, delivery, effects, abilities, presentation, and extension boundaries.
- [Map and mode specification](./04-maps-and-game-modes.md) — authored/resolved/runtime map layers,
  mode composition, terrain, topology, supported rules, and extension boundaries.
- [Gameplay loops](./05-gameplay-loops.md) — combat, fighter-life, objective, match, session,
  build-learning, arsenal, creator, progression, and social loops.
- [Research sources](./06-research-sources.md) — external references used for the baseline.
- [Network architecture](./08-network-architecture.md) — authority model, global application-protocol evolution, Lightyear replication, Bevy server/client world composition, and local network testing.
- [Environment gameplay direction](./09-environment-gameplay.md) — established environment
  boundaries, focused candidate families, promotion rules, lifecycle questions, concealment, and
  presentation constraints.
- [Bots](./10-bots.md) — server-hosted practice-controller decision, bounded policy contract,
  integration seams, first playable slice, and deferred hosting alternatives.
- [Art, presentation, and asset specification](./11-art-and-presentation-direction.md) — visual
  direction, presentation ownership, readability, themes, asset policy, provenance, lifecycle,
  degradation, and future-art boundaries.
- [Player UX and server-local matchmaking](./13-player-ux.md) — completed V2 player flow, queues, build selection, settings, accessibility, and verification decisions.
- [Multi-process server and single-port UDP/IPC transport](./14-multiplayer-server-architecture.md) — completed V2 supervisor, routed transport, isolated match-worker, and connection-handoff decisions.
- [Version 1 implementation roadmap](./implementation/v1/roadmap.md) — completed gameplay MVP milestones and closeout.
- [Version 2 implementation roadmap](./implementation/v2/roadmap.md) — completed product UX, routed transport, matchmaking, concurrent workers, and closeout.
- [Version 3 implementation roadmap](./implementation/v3/roadmap.md) — completed 3D gameplay-world presentation migration and deferred art/render backlog.
- [Version 4 implementation roadmap](./implementation/v4/roadmap.md) — accepted game-object taxonomy, reusable map presentation, scalable storage, second-map proof, and closeout order.
- [Version 5 implementation roadmap](./implementation/v5/roadmap.md) — dashboard-centered launch, connected player hub, selection/return simplification, and product-shell closeout plan.
- [V5 M01 — auto-connect and player-dashboard vertical slice](./implementation/v5/milestone-01.md) — current research and discussion record for the new launch and connected home.
- [V4 M01 — reusable environment library and first themed arena](./implementation/v4/milestone-01.md) — completed current-map improvement and production-reusable library.
- [V4 M02 — scalable map documents and reusable object definitions](./implementation/v4/milestone-02.md) — completed per-map authored storage and semantic placement resolution.
- [V4 M03 — second map/theme proof and V4 closeout](./implementation/v4/milestone-03.md) — completed Ashen Court, client theme profiles, usability hardening, feedback, and V4 closeout.
- [V3 M01 — 3D presentation feasibility and foundation](./implementation/v3/milestone-01.md) — completed feasibility foundation and accepted learning review.
- [V3 M02 — default 3D arena, map, terrain, camera, and input cutover](./implementation/v3/milestone-02.md) — completed world-cutover record.
- [V3 M03 — complete 3D combat presentation](./implementation/v3/milestone-03.md) — completed fighter/combat/world-HUD replacement record.
- [V3 M04 — renderer retirement and closeout](./implementation/v3/milestone-04.md) — completed retirement, readability, lifecycle, feedback, and learning record.

## Working vocabulary

- **Brawler:** a player-authored, potentially persistent fighter configuration in the player's
  arsenal.
- **Fighter:** the server-owned in-match combatant instantiated from a validated brawler build.
- **Player weapon selection:** the preset identity or bounded typed specification chosen for a
  brawler's primary weapon.
- **Weapon recipe:** the operational, server-validated composition derived from a player weapon
  selection or developer-authored preset.
- **Build:** the brawler's bounded weapon selection, ultimate, passive choices, body choices when
  supported, and future equipment selections.
- **Preset:** a developer-authored legal recipe/build used for onboarding, testing, or quick choice;
  it is not a separate combat implementation.
- **Map recipe:** a bounded arrangement of presentation layers, geometry, terrain, entities,
  regions, spawn points, and mode-required anchors; built-in maps are preset recipes.
- **Mode:** a server-owned rules implementation selected for a compatible map; players may lay out
  its required anchors but do not author the rules.
- **Item:** an equipable passive or active modifier.
- **Payload:** the gameplay effect produced by an attack or ability.
- **Map:** validated geometry, terrain, spawns, environment entities, gameplay regions, and
  mode-required anchors.
- **Gameplay region:** an authored or runtime area that applies movement, concealment, hazard, objective, or other server-owned rules independently from its visual tiles.
