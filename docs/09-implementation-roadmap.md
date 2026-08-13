# Implementation roadmap

## Planning rule

Every milestone begins with a short, in-depth technical-design pass and its own implementation plan. The design pass should answer the milestone's boundaries, data ownership, network behavior, test strategy, and exit criteria before production code is written.

Each milestone follows the same loop:

1. Technical design and decision records.
2. Small implementation plan with explicit file/module boundaries.
3. Implementation in shared, server, and client slices as applicable.
4. Automated tests and local network verification.
5. Playtest or visual verification.
6. Milestone review: keep, revise, or defer follow-up work.

The roadmap is intentionally sequential at the feature level. Art, UI, and infrastructure should not outrun the combat and networking loop.

## Physics policy

Use Bevy and Lightyear as the baseline from the first milestone. Add Avian 2D when collision queries or contact handling justify it, but do not let a general-purpose physics engine define weapon behavior.

Recommended division:

- **Avian 2D:** fighter/world collision shapes, static obstacles, overlap queries, raycasts, and collision layers if needed.
- **Game simulation:** projectile trajectories, firing cadence, hit validation, damage, payloads, status meters, and terrain brushes.
- **Lightyear:** input transport, replication, prediction/interpolation where needed, and server/client networking.

The first technical design should confirm whether Avian is needed immediately. Simple kinematic movement and custom circle/segment queries may be sufficient for the earliest combat slice. If Avian is adopted, use mostly static and kinematic bodies; avoid introducing full rigid-body simulation unless a gameplay requirement appears.

## Milestone overview

| Milestone | Deliverable | Main validation |
|---|---|---|
| M0 | Rust workspace and project foundation | The codebase can build, test, lint, and run client/server targets |
| M1 | Networked sandbox | Two clients connect to one authoritative server |
| M2 | Movement and aiming | Controller-first movement and aiming work over the network |
| M3 | Combat core | Server-authoritative firing, hits, damage, death, and respawn work |
| M4 | Weapon framework | Multiple weapon definitions share reusable projectile and payload systems |
| M5 | First arena and presentation baseline | A readable playable arena exists with replaceable placeholder art |
| M6 | Wipeout match loop | A complete short networked match can be played end to end |
| M7 | Abilities and build presets | Weapons, ultimates, and passive choices create distinct builds |
| M8 | Hot Zone | Combat supports spatial control and objective pressure |
| M9 | Heist | Persistent objectives and objective damage work authoritatively |
| M10 | Gem Grab | Pickups, carriers, drops, and countdown state work authoritatively |
| M11 | Flexible destructible terrain | Arbitrary terrain holes and collision updates work locally and over the network |
| M12 | Solo Showdown | Free-for-all, loot, shrinking boundary, and placement work |
| M13 | Systemic status interactions | Accumulating meters such as cold-to-freeze work across weapon sources |
| M14 | Playtest hardening | The prototype is measurable, debuggable, and ready for broader internal testing |

The milestones are not release promises. Each one may be split further during its planning pass.

The first playable product gate is **M0–M6**: foundation, networking, controls, combat, one arena, and a complete Wipeout match. **M7–M13** expand the validated loop with builds, the remaining mode families, flexible terrain, and systemic status interactions. **M14** is the evidence and stability pass before broader internal testing.

## M0 — Rust workspace and project foundation

### Deliverable

A Bevy/Rust workspace with client, server, shared, and protocol boundaries.

### Broad scope

- pin Bevy and Lightyear versions;
- create shared definitions and protocol crates;
- create a client binary with rendering/input plugins;
- create a server binary with headless/minimal plugins;
- establish fixed-tick scheduling;
- configure rustfmt, Clippy, tests, logging, and basic CI commands;
- add placeholder asset and data directories;
- add local launch commands for client, server, and multiple clients.

### Exit criteria

- `cargo test`, formatting, and Clippy commands are documented and pass;
- client and headless server compile independently;
- shared crate compiles without rendering or window dependencies;
- a blank server and client can be launched predictably.

## M1 — Networked sandbox

### Deliverable

Two local clients connect to one server and observe replicated placeholder entities.

### Broad scope

- Lightyear transport and connection lifecycle;
- shared network protocol;
- client input frames;
- server-owned player identity and spawn;
- replicated transforms or placeholder entities;
- connection, rejection, disconnect, and shutdown states;
- local multi-process test harness.

### Exit criteria

- two clients connect to one server;
- each client sees both players;
- the server remains authoritative for entity creation and removal;
- disconnects do not leave permanently owned entities behind;
- a headless server can run without client assets or rendering.

## M2 — Movement and aiming

### Deliverable

Controller-first movement and right-stick aiming, with keyboard/mouse support, operate in a replicated arena.

### Broad scope

- abstract input actions;
- Xbox-like controller mapping;
- WASD/mouse mapping;
- fixed-tick movement intent;
- aim vector and facing direction;
- camera follow and interpolation;
- collision approach decision: custom queries or Avian 2D;
- local prediction/reconciliation only if the measured feel requires it.

### Exit criteria

- two players can move and aim simultaneously;
- movement and facing remain server-authoritative;
- remote players interpolate acceptably;
- controller deadzone and neutral-stick behavior are defined;
- keyboard/mouse remains fully playable.

## M3 — Combat core

### Deliverable

A single direct projectile weapon supports authoritative hit resolution and the complete fighter damage loop.

### Broad scope

- fighter health and damage events;
- fire intent validation;
- ammo, cooldown, and reload;
- straight projectile movement;
- projectile collision and lifetime;
- hit confirmation and impact events;
- death, respawn delay, and reset state;
- health bar, hit flash, defeat feedback, and basic HUD.

### Exit criteria

- clients cannot fabricate hits, damage, deaths, or ammo state;
- two clients see consistent impacts and health results;
- projectiles behave consistently under packet delay tests;
- a fighter can die and respawn without corrupting match state.

## M4 — Weapon framework

### Deliverable

Reusable data-driven weapon and projectile primitives support the first four weapon types.

### Broad scope

- weapon definitions and runtime state;
- firing patterns;
- straight projectile;
- pellet spread;
- ballistic/lobbed projectile;
- melee arc;
- payload composition;
- immediate effects such as damage, knockback, slow, and area damage;
- projectile visual and impact effect hooks;
- weapon telemetry.

### Exit criteria

- the four weapons use shared systems rather than bespoke fighter scripts;
- weapon values can be changed in data without rewriting simulation code;
- server and client agree on weapon identity and event presentation;
- each weapon has a clear range, burst, and counterplay profile.

Defer bouncing, homing, curved steering, piercing, splitting, boomerang behavior, and accumulating status meters.

## M5 — First arena and presentation baseline

### Deliverable

One readable arena using the provisional Sci-Fi Facility and Shape Characters assets.

### Broad scope

- map definition format;
- spawn points and team colors;
- static walls and cover;
- map collision representation;
- camera bounds;
- placeholder fighter sprites;
- HUD layout for controller play;
- projectile, impact, hit, and defeat feedback;
- asset manifest and license records.

### Exit criteria

- players can understand walkable space, cover, teams, health, ammo, and abilities;
- the placeholder art can be replaced without changing gameplay code;
- the arena supports open sightlines, side routes, and at least one chokepoint;
- all map state needed by the server is representable without loading client-only visuals.

## M6 — Wipeout match loop

### Deliverable

A complete short networked Wipeout-style match.

### Broad scope

- match lifecycle;
- team assignment;
- spawn and respawn rules;
- takedown attribution;
- team score;
- match timer;
- score threshold and timeout resolution;
- scoreboard and results screen;
- basic bots or fixed test opponents;
- match restart.

### Exit criteria

- a match starts, plays, ends, and can restart without process restart;
- the server owns score, respawns, and victory;
- two or more clients can complete a match;
- match results are identical for all clients;
- a complete match is roughly two to four minutes.

This is the first major gameplay gate. Do not add large content systems before the team has playtested this loop.

## M7 — Abilities and build presets

### Deliverable

Players can choose a bounded build and use an ultimate during a match.

### Broad scope

- ultimate resource and charge events;
- dash ultimate;
- deployable ultimate;
- one active item slot if the combat loop needs it;
- passive item modifiers;
- build budget or bounded slot rules;
- Runner, Bruiser, Controller, and Duelist presets;
- server validation for build definitions and ability use;
- ability HUD and cooldown/charge feedback.

### Exit criteria

- builds create different combat behavior rather than only larger numbers;
- ability state is authoritative and replicated clearly;
- deployables have bounded lifetime and cleanup;
- players understand ability availability from a controller.

## M8 — Hot Zone

### Deliverable

One-zone Hot Zone match using the existing combat and match infrastructure.

### Broad scope

- capture volume;
- occupancy evaluation;
- per-team progress;
- contesting rules;
- progress HUD;
- timer expiry and tie handling;
- one objective-focused map variant.

### Exit criteria

- the zone creates meaningful movement and combat decisions;
- progress is server-authoritative;
- simultaneous occupancy behaves predictably;
- the mode reuses fighters, weapons, abilities, and match lifecycle code.

## M9 — Heist

### Deliverable

Two teams can attack and defend durable objectives.

### Broad scope

- objective entities and health;
- objective targeting and damage rules;
- objective destruction victory;
- timeout comparison;
- objective-specific feedback;
- optional threshold shielding only if playtests require it.

### Exit criteria

- objective state is authoritative and replicated;
- fighter damage and objective damage can be balanced independently;
- the map supports both attacking and returning to defense;
- no weapon or fighter script contains Heist-specific victory logic.

## M10 — Gem Grab

### Deliverable

Teams collect, carry, drop, and defend objective items.

### Broad scope

- map-authored spawn source;
- pickup ownership;
- carrier state;
- drop-on-defeat;
- team totals;
- winning countdown activation and cancellation;
- carrier HUD and off-screen information;
- deterministic handling of simultaneous pickup/drop events.

### Exit criteria

- the entire objective state is server-authoritative;
- a carrier's defeat produces the correct drops exactly once;
- countdown state cannot be fabricated by a client;
- the central objective creates contestable routes.

## M11 — Flexible destructible terrain

### Deliverable

Server-authoritative arbitrary terrain destruction with client reconstruction.

### Broad scope

- terrain solidity mask;
- chunked dirty-region updates;
- circular destruction brush;
- visual terrain update;
- generated collision polygons;
- projectile and fighter collision against changed terrain;
- unstuck behavior;
- Lightyear terrain destruction events and terrain revisions;
- client crater/debris presentation.

### Exit criteria

- an explosion creates holes or tunnels without TileMap cell replacement;
- only affected chunks rebuild;
- both clients reproduce the server-issued crater;
- terrain changes do not modify unrelated objectives, props, or fighter state;
- server collision remains the gameplay authority.

Defer structural collapse, fluids, material simulation, persistent terrain saves, and full terrain snapshots.

## M12 — Solo Showdown

### Deliverable

A solo free-for-all mode with no respawns and a closing playable area.

### Broad scope

- free-for-all team model;
- elimination and placement tracking;
- loot/pickup entities;
- temporary power scaling;
- shrinking boundary and damage;
- spawn fairness;
- results by placement.

### Exit criteria

- the mode is clearly separate from team scoring modes but reuses shared combat primitives;
- late-game encounters are forced without invalidating builds;
- all placement and loot state is server-authoritative.

Duo and trio variants remain later extensions.

## M13 — Systemic status interactions

### Deliverable

One complete accumulating status interaction, such as cold-to-freeze.

### Broad scope

- target-owned status meters;
- per-hit contribution from a projectile;
- per-tick contribution from an area;
- decay delay and decay rate;
- threshold trigger;
- frozen duration;
- resistance, immunity, and trigger cooldown;
- shared contribution from compatible weapons;
- server-authoritative status HUD data.

### Exit criteria

- ice pellets and an ice area contribute to the same target meter;
- the threshold triggers exactly once under defined rules;
- the meter and frozen state replicate correctly;
- status logic remains reusable for future heat, poison, shock, or similar systems.

## M14 — Playtest hardening

### Deliverable

A stable internal prototype with useful measurement and repeatable test scenarios.

### Broad scope

- combat and match telemetry;
- fixed-tick replay or deterministic event logs where practical;
- packet-loss, latency, and reconnect test profiles;
- server load and entity-count measurements;
- build matchup reports;
- automated simulation tests for mode rules;
- crash and error reporting for local development;
- input remapping and controller settings;
- technical debt review and next-phase plan.

### Exit criteria

- the team can reproduce major combat and networking bugs;
- balance decisions are based on captured data and playtests;
- the prototype supports repeatable two-client and multi-client sessions;
- next work is prioritized by evidence rather than content volume.

## Explicitly outside this roadmap

- production matchmaking and party services;
- accounts, persistence, progression, and monetization;
- ranked/live-operations systems;
- final art direction and production animation;
- platform certification and store release work;
- advanced projectile trajectories until the core weapon model is validated;
- production anti-cheat and fleet orchestration.
