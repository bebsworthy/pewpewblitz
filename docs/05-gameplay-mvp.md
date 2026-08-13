# Gameplay MVP

## MVP statement

Two teams of two or three fighters fight on one small map. Each player chooses one of four weapons and one of two ultimates. Players move, aim, fire, take damage, die, respawn, and earn points for eliminations. The first team to the target score wins.

This is a Wipeout-style networked prototype because it exercises the important loop with minimal mode-specific machinery. The first test may run locally, but it uses a server-authoritative simulation.

## Content scope

- one arena;
- one fighter body profile;
- four weapons;
- two projectile trajectories in the first combat pass: straight and ballistic/lobbed;
- immediate hit effects only; accumulating status meters are deferred;
- two ultimates;
- four to six passive items, optional for the first combat pass;
- one team-vs-team mode;
- Xbox-like controller as the primary control scheme;
- keyboard and mouse as a supported secondary scheme;
- basic bots or fixed test dummies;
- placeholder visuals and audio.

The MVP must be playable through a local dedicated server plus one client. A local in-process server/client mode may be used for rapid iteration, but it must use the same server-owned gameplay systems and authority rules.

## Controls

The first version should be designed around a standard Xbox-like controller. Keyboard/mouse support must be present, but controller play is the reference experience for movement, aiming, combat rhythm, HUD layout, and usability decisions.

### Provisional controller layout

| Action | Controller input |
|---|---|
| Move | Left stick |
| Aim | Right stick |
| Primary weapon | Right trigger (RT) |
| Active item | Left bumper (LB) |
| Ultimate | Right bumper (RB) |
| Interact / confirm | A |
| Cancel / back | B |
| Pause / menu | Menu button |
| Scoreboard / match information | View button |

This mapping is provisional and should be validated through playtesting. The gameplay code should use actions such as `move`, `aim`, `primary_fire`, `active_item`, `ultimate`, `interact`, and `cancel`, not direct button checks.

### Keyboard/mouse layout

| Action | Keyboard/mouse input |
|---|---|
| Move | WASD |
| Aim | Mouse position |
| Primary weapon | Left mouse button |
| Active item | Q |
| Ultimate | E |
| Interact / confirm | Space or Enter |
| Cancel / back | Escape |
| Pause / menu | Escape |

Keyboard/mouse should remain fully playable rather than being a debug-only input path. It is especially useful for development, bot testing, and comparing aim precision against controller play.

### Controller usability requirements

- Configure a stick deadzone and expose it as a setting later.
- Treat a right-stick magnitude below the aim threshold as no new aim input.
- Preserve the last valid aim direction when the right stick returns to neutral.
- Support aim assist or target selection only as an explicit, tunable gameplay rule; do not hide it inside input handling.
- Make primary fire, active item, and ultimate states readable without requiring a mouse cursor.
- Ensure every combat action has a controller-friendly HUD indicator and feedback response.
- Do not make precise cursor placement mandatory for objectives or menu navigation.

## Build test presets

Before exposing a full editor, provide named presets:

- **Runner:** fast movement, sidearm, dash;
- **Bruiser:** high health, scatter cannon, defensive passives, and a legal first-iteration ultimate;
- **Controller:** launcher, deployable sentry, and control-oriented passives;
- **Duelist:** impact blade, dash, and short-range passives.

Presets let the team evaluate the design while avoiding UI and persistence work.

## Delivery gates

The detailed implementation sequence is maintained in the [v1 implementation roadmap](./implementation/v1/roadmap.md). This document defines the gameplay scope and acceptance criteria; the roadmap is the source of truth for milestone ordering and progress.

There is no separate engine go/no-go spike. The Bevy/Lightyear stack is adopted for the MVP, and v1 Milestones 01–03 provide the practical foundation, connection, replication, and authoritative-movement validation. If those milestones expose a blocking integration problem, resolve it before expanding gameplay content.

### Early implementation scope

The first implementation work combines the former engine validation with a minimal playable sandbox:

- movement;
- aiming;
- controller movement and right-stick aiming;
- keyboard/mouse movement and mouse aiming;
- one projectile weapon;
- collision and damage;
- health bar;
- death and reset;
- hit and defeat feedback.

It must run as a macOS client connected to a headless authoritative server, using the same server-owned Bevy gameplay systems, validation, and outcomes that later milestones extend.

### Combat vertical slice — Milestones 01–07

The first playable gate adds the Rust/Bevy application and networking foundation, controller-first movement, a greybox collision environment, authoritative combat, four selectable weapons, one readable arena, and a complete Wipeout match. It is sufficient to validate combat and networking, but it does not yet validate Brawler's buildcraft differentiation.

### First product iteration — Milestones 01–08

The first product iteration adds the two initial ultimates, four to six passive items, a fixed build budget, two passive slots, and four legal named presets. This is the first gate that tests the product direction rather than only the underlying arena-shooter loop.

### Gameplay MVP verification — Milestones 01–10

Hot Zone then verifies that the same fighter, weapon, ability, and lifecycle code works under spatial-control rules. The destructible-terrain milestone verifies arbitrary holes, generated collision, authoritative terrain revisions, and late/reconnecting client recovery. Completion of Milestone 10 is the point at which every acceptance criterion below must have evidence; Milestone 11 hardens and closes v1.

Projectile behaviors beyond straight and ballistic movement—such as bouncing, homing, curved steering, boomerang return, piercing, splitting, and delayed trajectories—are second-phase content.

Accumulating weapon interactions are also second-phase content. The first reference interaction should be cold accumulation from ice projectiles and ice zones, triggering a temporary freeze at a target threshold. The status system must support contributions from multiple compatible weapons, but only one status type needs to be implemented initially.

## Acceptance criteria

The MVP is successful when:

- a player can understand the controls without a tutorial;
- every weapon has a clear preferred distance and counterplay;
- fighters can reliably hit, damage, defeat, and respawn;
- players can identify why they lost a fight;
- different presets produce visibly different match behavior;
- a complete match finishes in roughly two to four minutes;
- the team can change weapon values without rewriting combat code;
- the same fighter and weapon code can run under Wipeout and Hot Zone rules;
- the complete combat loop is playable with an Xbox-like controller;
- the same actions are playable with keyboard/mouse without separate gameplay implementations;
- two local clients can play one server-authoritative match;
- clients cannot authoritatively alter positions, damage, status meters, scores, or terrain;
- terrain destruction can create holes and tunnels without replacing visible map tiles;
- terrain collision updates do not modify unrelated props, objectives, or fighter bodies.

## Deliberately postponed

- production-scale online services and matchmaking;
- internet-scale hosting and deployment;
- lag compensation and advanced client-side prediction;
- account and inventory persistence;
- progression and unlocks;
- cosmetics;
- touch controls;
- multiple body sizes;
- procedural content;
- automated balance generation;
- terrain bandwidth optimization and persistent terrain synchronization;
- structural collapse, fluids, and persistent terrain deformation;
- advanced projectile trajectories and multi-stage projectile effects;
- accumulating status meters and threshold-triggered crowd control.

## What to measure

Record simple local telemetry from test matches:

- time to first damage;
- average fight duration;
- weapon hit rate;
- damage by distance band;
- elimination and death rate by preset;
- ultimate charge time;
- time spent moving versus stationary;
- average respawn-to-death time;
- score margin at match end.

These measurements are more useful than adding more content before the core loop is understood.
