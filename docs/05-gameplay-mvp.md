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

The MVP must be playable through a local dedicated server plus one client. A local in-process server/client mode may be used for rapid iteration, but it must use the same simulation and authority boundaries.

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
- **Bruiser:** high health, scatter cannon, shield;
- **Controller:** launcher, deployable sentry;
- **Duelist:** impact blade, dash or pull.

Presets let the team evaluate the design while avoiding UI and persistence work.

## Milestones

The detailed implementation sequence is maintained in [09-implementation-roadmap.md](./09-implementation-roadmap.md). This document defines the gameplay scope and acceptance criteria; the roadmap is the source of truth for milestone ordering.

There is no separate engine go/no-go spike. The Bevy/Lightyear stack is adopted for the MVP, and the first foundation and networked-sandbox milestones provide the practical validation. If those milestones expose a blocking integration problem, resolve it before expanding gameplay content.

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

It must run as a macOS client connected to a headless authoritative server, with the same shared simulation boundary used by later milestones.

### M1 — weapon comparison

- four weapons;
- straight pulse projectile;
- short-range pellet spread;
- ballistic/lobbed splash projectile;
- circular explosion payload;
- ammo and reload;
- weapon cooldowns;
- weapon selection before a test round;
- basic combat telemetry.

### M2 — teams and respawn

- teams;
- spawn points;
- defeat state;
- respawn delay;
- score tracking;
- match timer and victory condition.

### M3 — ability layer

- ultimate meter;
- dash ultimate;
- deployable ultimate;
- cooldown and charge UI;
- effect cleanup on death.

### M4 — build decisions

- passive item slots;
- build budget;
- four named build presets;
- end-of-match comparison of build performance.

### M5 — second mode

Implement Hot Zone on the same map or a small variant. This tests whether the combat loop supports contesting space, not only chasing eliminations.

### M6 — destructible terrain prototype

- one destructible terrain chunk;
- circular explosion brush;
- terrain mask update;
- visual crater and edge feedback;
- generated collision update;
- player/projectile interaction with changed terrain;
- unstuck behavior after terrain changes.

This milestone validates Worms-like terrain flexibility without making the entire MVP map dependent on destruction.

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
- the same fighter and weapon code can run under Wipeout and Hot Zone rules.
- the complete combat loop is playable with an Xbox-like controller;
- the same actions are playable with keyboard/mouse without separate gameplay implementations.
- two local clients can play one server-authoritative match;
- clients cannot authoritatively alter positions, damage, status meters, scores, or terrain.
- terrain destruction can create holes and tunnels without replacing visible map tiles;
- terrain collision updates do not modify unrelated props, objectives, or fighter bodies.

## Deliberately postponed

- production-scale online services and matchmaking;
- internet-scale hosting and deployment;
- lag compensation and advanced client-side prediction;
- account and inventory persistence;
- matchmaking;
- progression and unlocks;
- cosmetics;
- touch controls;
- multiple body sizes;
- procedural content;
- automated balance generation.
- terrain bandwidth optimization and persistent terrain synchronization;
- structural collapse, fluids, and persistent terrain deformation.
- advanced projectile trajectories and multi-stage projectile effects.
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
