# Network architecture

## Architectural decision

Brawler is first and foremost a networked competitive game. The target architecture is a **dedicated-server-authoritative simulation**. The local gameplay prototype is only a development harness; it must not become a separate client-authoritative implementation that later needs to be rewritten for online play.

The planned implementation is Bevy 0.19 with Lightyear 0.29. Bevy core is intentionally modular and does not provide first-party networking; Lightyear supplies the networked game layer. Bevy supports headless applications by omitting rendering/window features, while Lightyear provides separate client/server plugins, input buffering, replication, prediction, rollback, interpolation, and transport layers. See the version-pinned [Bevy headless example](https://docs.rs/crate/bevy/0.19.0/source/examples/app/headless.rs), [Lightyear 0.29 documentation](https://docs.rs/lightyear/0.29.0/lightyear/), and [Lightyear repository](https://github.com/cBournhonesque/lightyear).

This dependency choice is a deliberate risk, not an invisible assumption. v1 Milestones 01–03 validate the actual version combination through application composition, two-client connection/replication, and server-authoritative movement before the project commits to substantial combat content; there is no separate throwaway engine spike.

## Authority model

```text
Client input commands
          ↓
Dedicated authoritative server
  ECS gameplay, validation, scoring,
  combat, status, terrain, match rules
          ↓
Replicated state and gameplay messages
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
- replicate authoritative components and send discrete gameplay messages where required.

## Bevy and Lightyear world composition

The authoritative server and each client own separate Bevy worlds with different plugin compositions:

```text
Client World
  device input
  Lightyear client plugins
  replicated/interpolated state
  predicted gameplay systems, if adopted
  rendering, animation, audio, camera, HUD
                 ↕ intent / replication / messages
Authoritative server World
  Lightyear server plugins
  gameplay components and resources
  fixed-step movement, combat, effects, modes
  validation, ownership, cleanup, recovery
```

Authored definitions may be serializable Rust data or Bevy assets/configuration loaded into the worlds that need them. Runtime authority lives in server components, resources, entities, states, and scheduled systems. Client-only presentation observes replicated gameplay state or presentation messages and must not become gameplay truth.

Use a shared gameplay plugin or module only for systems that genuinely execute on both server and client, primarily when prediction requires identical fixed-step behavior. Server-only match, validation, damage, score, terrain, and lifecycle rules remain server-only. Package and folder boundaries are implementation decisions made from feature and dependency evidence, not part of the network contract.

## Input and replication

The client sends intent, not results. The exact Lightyear input type is selected during the relevant milestone; conceptually it contains:

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

The server normally exposes authoritative state through Lightyear-replicated ECS components. Discrete outcomes use explicitly registered messages when they are not adequately represented by replicated state:

```text
Replicated components
  stable player / match / definition identity
  fighter and projectile state
  active effects and objective state
  scores and terrain revision

Gameplay message, when required
  event_id
  stable source/target network identity
  event_type
  payload
```

Do not introduce a custom aggregate `Snapshot` wrapper when Lightyear component replication provides the needed behavior. Register concrete inputs, replicated components, messages, and channels through Bevy plugins, including delivery and ordering semantics. Network data must not expose process-local ECS entity identity unless Lightyear's entity mapping explicitly handles it.

## Terrain synchronization

Terrain destruction is server-authoritative. The server should process the destruction brush, update the terrain revision, and broadcast a compact destruction command or affected-region update. Clients reconstruct the same visual crater locally. Do not send a full terrain texture after every explosion.

For the first terrain network test, a terrain event can contain:

```text
TerrainDestructionEvent
  terrain_chunk_id
  brush_type
  position
  size
  rotation
  terrain_revision
```

The server remains responsible for collision and gameplay truth. Clients use the event for presentation and prediction only. Each client tracks the latest applied terrain revision. A gap must trigger authoritative recovery from an initial mask, affected-chunk snapshot, or retained event history; reconnecting and late-joining clients cannot be required to have observed every live destruction event.

## Status synchronization

The server owns internal status meters such as `cold`, threshold checks, freeze duration, decay, resistance, and immunity. Clients may receive the meter value for HUD feedback, but cannot apply or trigger the status themselves.

## Local development modes

Support three development configurations without changing the authoritative gameplay path:

1. **Dedicated server + one client:** normal local debugging path.
2. **Dedicated server + multiple local clients:** multiplayer and replication testing on one machine.
3. **In-process loopback server/client:** fast automated tests and sandbox iteration, using Lightyear's local transport/testing support where practical.

An offline training mode may use bots, but it must still run the authoritative server systems and validation. “Local-only” describes where the server runs during development, not client-authoritative architecture.

## Staged network validation

Networking is validated incrementally rather than treated as one oversized milestone:

1. **v1 Milestone 02 — connection and replication:** two local clients connect, receive stable server-owned identities and entities, and clean up on rejection, disconnect, reconnect, and shutdown under explicit rules.
2. **v1 Milestone 03 — movement:** the server validates input frames and owns movement and facing; clients interpolate remote state and add local prediction only if measurement justifies it.
3. **v1 Milestone 04 — combat:** the server owns firing, projectiles, hits, damage, defeats, and sandbox reset under packet delay, loss, duplication, and jitter tests.
4. **v1 Milestone 07 — match:** the server owns teams, respawns, scores, timers, victory, restart, and disconnect behavior throughout the match lifecycle.
5. **v1 Milestone 09 — objectives:** Hot Zone proves that continuous objective state remains authoritative while reusing the same gameplay and match-lifecycle components/plugins.
6. **v1 Milestone 10 — terrain:** connected and late/reconnecting clients converge on the authoritative terrain revision and crater state.
7. **Future systemic-status milestone:** accumulating meters, threshold triggers, immunity, and duration remain server-owned and recover correctly.

Prediction, lag compensation, advanced interpolation tuning, anti-cheat hardening, matchmaking, authentication, session services, and production hosting may be developed after the relevant early gates. The authority boundary, state recovery rules, and explicit connection lifecycle outcomes may not be postponed.
