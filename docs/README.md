# Brawler design documentation

Brawler is an original, cross-platform top-down arena shooter built around player-authored fighters. Players assemble a fighter from a bounded set of stats, weapons, abilities, and equipment rather than selecting a fixed hero with a fixed kit.

The project is intentionally starting with the gameplay loop. Production matchmaking, account services, monetization, cosmetics, live operations, and release engineering are out of scope initially. Core networking and the server-authoritative simulation are in scope from the beginning.

## Documents

- [Product direction](./00-product-direction.md) — vision, differentiation, principles, and non-goals.
- [Engine decision](./01-engine-decision.md) — Bevy/Rust recommendation, networking stack, and technical constraints.
- [Fighter model](./02-fighter-model.md) — attributes, derived values, loadout rules, and runtime state.
- [Weapons and abilities](./03-weapons-and-abilities.md) — weapon primitives, payloads, and initial content set.
- [Maps and game modes](./04-maps-and-game-modes.md) — map grammar and the five planned mode families.
- [Gameplay MVP](./05-gameplay-mvp.md) — the smallest playable slice, milestones, and acceptance criteria.
- [Research sources](./06-research-sources.md) — external references used for the baseline.
- [MVP asset shortlist](./07-mvp-asset-shortlist.md) — open/licensed stand-in tilemaps, characters, props, and icons.
- [Network architecture](./08-network-architecture.md) — authority model, replication, server/client boundaries, and local network testing.
- [Implementation roadmap](./09-implementation-roadmap.md) — deliverable milestones from workspace foundation through playtest hardening.

## Working vocabulary

- **Fighter:** the player-controlled combatant.
- **Build:** the fighter's selected weapon, ability, and equipment.
- **Item:** an equipable passive or active modifier.
- **Payload:** the gameplay effect produced by an attack or ability.
- **Mode:** the rules that determine scoring, respawning, and victory.
- **Map:** geometry, spawn points, objectives, hazards, and interactables.
