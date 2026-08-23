# PewPew Blitz documentation

PewPew Blitz is an original cross-platform top-down arena shooter built around player-authored
brawler builds, readable combat, meaningful tradeoffs, short objective matches, and reusable
content primitives. The server resolves bounded player choices and owns gameplay simulation;
practice, multiplayer, creator, arsenal, progression, and social directions extend that foundation
without creating alternate gameplay authority.

Start with [Product direction](./00-product-direction.md) for the product promise and design
principles. Use the specifications below for durable product and technical contracts, the
[candidate index](./backlog.md) for unresolved future work, and versioned implementation records for
delivery status and evidence.

## How the documentation is organized

- **Product direction and specifications** are aspirational and forward-looking. They describe the
  intended product, settled boundaries, supported foundations, and envisioned extensions. They are
  not implementation-status reports.
- **The candidate index** is the canonical cross-version view of unresolved work. A candidate is
  not a commitment until it is promoted into a version roadmap.
- **Version roadmaps and milestones** are implementation records. They own scope, status, research,
  accepted technical specifications, verification, playtest feedback, and closeout evidence for a
  particular delivery period.
- **Source and root project documentation** own current commands, configuration, and operational
  behavior. Historical milestones may refer to older paths or implementations without redefining
  the current architecture.

## Product and gameplay direction

- [Product direction](./00-product-direction.md) — product promise, differentiation, principles,
  creator direction, and non-goals.
- [Fighter and build specification](./02-fighter-model.md) — authored choices, accepted build
  identity, resolved loadouts, fighter runtime, statuses, and future arsenal/equipment boundaries.
- [Weapons and abilities specification](./03-weapons-and-abilities.md) — combat authoring layers,
  weapon recipes, delivery, payloads, effects, abilities, presentation facts, and extension rules.
- [Map and mode specification](./04-maps-and-game-modes.md) — authored, resolved, and runtime map
  layers; mode composition; topology; terrain; supported rules; and the future builder boundary.
- [Gameplay loops](./05-gameplay-loops.md) — combat, encounter, objective, match, session,
  build-learning, practice, creator, progression, and social loops.
- [Environment gameplay direction](./09-environment-gameplay.md) — established environment
  boundaries, focused candidate families, promotion rules, lifecycle questions, concealment, and
  presentation constraints.
- [Bots](./10-bots.md) — server-hosted practice-controller decision, bounded policy contract,
  integration seams, first playable slice, and deferred hosting alternatives.
- [Art, presentation, and asset specification](./11-art-and-presentation-direction.md) — visual
  direction, renderer and dashboard presentation ownership, readability, themes, asset policy,
  provenance, lifecycle, degradation, and future-art boundaries.
- [Player experience specification](./13-player-ux.md) — canonical Dashboard-centered flow,
  admission, selection, settings, accessibility, recovery, and envisioned UX extensions.

## Technical foundation

- [Engine specification](./01-engine-decision.md) — supported Bevy/Rust baseline, runtime roles,
  application composition, scheduling, physics, presentation, and feature boundaries.
- [Network architecture](./08-network-architecture.md) — gameplay authority, application-protocol
  evolution, Lightyear replication, client/server world composition, recovery, and network
  validation boundaries.
- [Multi-process server architecture](./14-multiplayer-server-architecture.md) — supervisor,
  single-port routed transport, lobby and isolated match workers, IPC, connection handoff, failure,
  and security contracts.
- [Balance Lab guide](./15-balance-lab.md) — local operator workflow, validation philosophy,
  persistence, limitations, and the required maintenance checklist for fighter and weapon changes.
- [Grid map-asset system specification](./16-grid-map-asset-system.md) — implemented V8 sparse-grid
  recipes, unified map assets, gameplay properties, client visual profiles, conversion, and
  legacy-removal contract.
- [Research sources](./06-research-sources.md) — external references used to establish the original
  product and technical baseline.

## Future candidates

[Canonical cross-version candidate index](./backlog.md) is the single comparison and promotion
surface for unresolved product, service, architecture, release, and maintenance candidates. Detailed
specifications and historical rationale remain in their owning documents; the index links to them
rather than copying them.

## Implementation history

Each roadmap is the entry point for its version and links to the corresponding milestone records.
The milestone files preserve research, implementation scope, verification evidence, user feedback,
and closeout learning.

| Version | Recorded delivery focus | Entry point |
|---|---|---|
| V1 | Server-authoritative gameplay MVP and direct-UDP comparison baseline; not a release-readiness claim | [V1 roadmap](./implementation/v1/roadmap.md) |
| V2 | Product client shell, server-local matchmaking, routed multi-process hosting, concurrent matches, and practice | [V2 roadmap](./implementation/v2/roadmap.md) |
| V3 | Client-side 3D gameplay-world presentation migration while retaining planar authoritative simulation | [V3 roadmap](./implementation/v3/roadmap.md) |
| V4 | Independent map documents, semantic object placement, reusable themes, and second-map proof | [V4 roadmap](./implementation/v4/roadmap.md) |
| V5 | Auto-connect, Player Dashboard, connected-loop convergence, responsive presentation, and lifecycle hardening | [V5 roadmap](./implementation/v5/roadmap.md) |
| V6 | Development-only, server-authoritative Practice Balance Lab for rapid fighter and weapon tuning | [V6 roadmap](./implementation/v6/roadmap.md) |
| V7 | Persistent server-owned player profiles, saved brawlers, weapon bases, and four-slot weapon-part equipment | [V7 roadmap](./implementation/v7/roadmap.md) |
| V8 | Sparse-grid maps, unified map-asset/gameplay/visual catalogs, full built-in conversion, and retirement of the old production map system | [V8 roadmap](./implementation/v8/roadmap.md) |

Historical milestones are evidence for the choices and implementation of their version. They do not
override later durable specifications or accepted changes recorded by subsequent versions.

## Core vocabulary

These definitions provide cross-document orientation. The linked owning specifications remain
authoritative for exact schemas, supported fields, and lifecycle rules.

- **Brawler:** the product-level player-authored configuration or identity from which a match
  fighter is instantiated. V7 owns the promoted long-lived server-side identity, persistence, and
  equipment loop; earlier versions retain match-scoped selections only.
- **Fighter:** the server-owned in-match combatant instantiated from a validated, resolved brawler
  loadout.
- **Build:** a bounded player selection that the server validates and resolves. The supported
  foundation contains a primary-weapon selection, one ultimate, and two passives; additional body,
  item, or equipment choices require an explicitly supported extension.
- **Player weapon selection:** the bounded preset identity or typed specification chosen for the
  brawler's primary weapon.
- **Weapon recipe:** the operational, server-resolved weapon composition derived from a legal
  player selection or developer-authored preset; it is not arbitrary client-authored runtime data.
- **Map recipe:** a bounded authored layout that resolves into an immutable server-validated map
  snapshot. Built-in presets and future player-authored maps use the same recipe boundary.
- **Mode:** a developer-authored authoritative rules implementation. A compatible map supplies
  validated space, spawns, and required anchors; it does not author executable mode logic.
- **Game type:** one stable server advertisement combining a mode, compatible map pool, exact team
  topology, bounded rule configuration, and content revisions for admission.
