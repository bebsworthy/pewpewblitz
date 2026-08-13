# Network architecture

## Architectural decision

Brawler is first and foremost a networked competitive game. The target architecture is a **dedicated-server-authoritative simulation**. The local gameplay prototype is only a development harness; it must not become a separate client-authoritative implementation that later needs to be rewritten for online play.

The planned implementation is Bevy 0.19 with Lightyear 0.29. Bevy core is intentionally modular and does not provide first-party networking; Lightyear supplies the networked game layer. Bevy supports headless applications by omitting rendering plugins, while Lightyear provides separate client/server plugins, input buffering, replication, prediction, rollback, interpolation, and transport layers. See [Bevy plugins and headless servers](https://bevy.org/learn/quick-start/getting-started/plugins/), [Lightyear](https://docs.rs/lightyear/latest/lightyear/), and [Lightyear repository](https://github.com/cBournhonesque/lightyear).

This dependency choice is a deliberate risk, not an invisible assumption. The M0–M1 foundation and networked-sandbox milestones validate the actual version combination before the project commits to substantial content production; there is no separate throwaway engine spike.

## Authority model

```text
Client input commands
          ↓
Dedicated authoritative server
  simulation, validation, scoring,
  combat, status, terrain, match rules
          ↓
Snapshots and gameplay events
          ↓
Client presentation and local feedback
```

### Client responsibilities

- read controller or keyboard/mouse input;
- send timestamped input commands or input frames;
- predict local movement later if needed;
- render the latest authoritative state;
- interpolate remote fighters;
- play visual, audio, and camera effects;
- display HUD and scoreboard;
- never decide authoritative damage, hits, deaths, status triggers, scores, or terrain edits.

### Server responsibilities

- own the match lifecycle and mode rules;
- simulate fighter movement and abilities;
- validate fire commands and cooldowns;
- perform projectile, hit, damage, and collision resolution;
- own status meters and threshold effects;
- own pickups, objectives, scores, respawns, and victory;
- own destructible terrain masks and collision regeneration;
- send authoritative snapshots and discrete gameplay events.

## Shared simulation boundary

Keep shared gameplay definitions and simulation rules separate from presentation:

```text
shared/
  definitions/      weapons, fighters, items, maps
  simulation/       movement, projectiles, effects, status, modes
  protocol/         input commands, snapshots, events

client/
  rendering/
  animation/
  audio/
  hud/
  input/

server/
  Lightyear server plugins/
  match hosting/
  persistence hooks later
```

The exact folder names may change, but the dependency direction should remain: server and client consume shared simulation/definitions; shared simulation must not depend on rendering or UI.

## Input and replication

The client sends intent, not results:

```text
InputFrame
  sequence
  client_tick
  move_vector
  aim_vector
  primary_fire
  active_item
  ultimate
  interact
```

The server returns authoritative state, for example:

```text
Snapshot
  server_tick
  fighters
  projectiles
  active_effects
  objectives
  scores
  terrain_revision

GameplayEvent
  event_id
  source_entity
  target_entity
  event_type
  payload
```

Use snapshots for state that changes continuously and events for discrete outcomes such as firing, impact, elimination, freeze trigger, pickup, objective damage, and terrain destruction.

## Terrain synchronization

Terrain destruction is server-authoritative. The server should process the destruction brush, update the terrain revision, and broadcast a compact destruction command or affected-region update. Clients reconstruct the same visual crater locally. Do not send a full terrain texture after every explosion.

For the first network test, a terrain event can contain:

```text
TerrainDestructionEvent
  terrain_chunk_id
  brush_type
  position
  size
  rotation
  terrain_revision
```

The server remains responsible for collision and gameplay truth. Clients use the event for presentation and prediction only.

## Status synchronization

The server owns internal status meters such as `cold`, threshold checks, freeze duration, decay, resistance, and immunity. Clients may receive the meter value for HUD feedback, but cannot apply or trigger the status themselves.

## Local development modes

Support three development configurations without changing gameplay rules:

1. **Dedicated server + one client:** normal local debugging path.
2. **Dedicated server + multiple local clients:** multiplayer and replication testing on one machine.
3. **In-process loopback server/client:** fast automated tests and sandbox iteration, using Lightyear's local transport/testing support where practical.

An offline training mode may use bots, but it should still run through the same authoritative match simulation. “Local-only” describes where the server runs during development, not client-authoritative architecture.

## Initial networking milestone

Before adding online matchmaking, prove:

- two local clients connect to one server;
- the server owns fighter movement, firing, damage, deaths, respawns, and scoring;
- both clients see consistent projectile impacts;
- status effects and threshold triggers resolve identically for both clients;
- a server-issued terrain event produces the same crater on both clients;
- disconnect and reconnect behavior has an explicit test outcome.

Prediction, lag compensation, interpolation tuning, anti-cheat hardening, matchmaking, authentication, session services, and production hosting can be developed after this vertical slice, but the authority boundary should not be postponed.
