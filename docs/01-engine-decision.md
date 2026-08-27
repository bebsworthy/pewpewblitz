# Engine specification

## Purpose and authority

PewPew Blitz uses **Bevy with Rust** for its client and authoritative gameplay applications. This
choice is settled. This document defines the enduring engine contract: the supported dependency
baseline, runtime roles, Bevy application composition, ECS ownership, scheduling, physics,
networking integration, presentation boundary, headless isolation, and evolution policy.

More focused documents own their detailed domains:

- [Network architecture](./08-network-architecture.md) owns gameplay authority, replication,
  protocol compatibility, and recovery.
- [Multiplayer server architecture](./14-multiplayer-server-architecture.md) owns supervisor,
  worker, routing, IPC, admission, and process lifecycle.
- [Art, presentation, and asset specification](./11-art-and-presentation-direction.md) owns the visual language
  and renderer-facing content policy.
- [Player UX](./13-player-ux.md) owns the product shell, input experience, settings, accessibility,
  and screen flow.
- [Map and mode specification](./04-maps-and-game-modes.md) owns map recipes, mode compatibility, terrain,
  and the player map-builder boundary.

Those documents may refine their owned behavior without weakening the engine boundaries defined
here. Version milestone specifications record changes and evidence; they do not silently establish
a second engine composition.

## Supported baseline

| Concern | Supported baseline |
|---|---|
| Language and toolchain | Rust 1.95, edition 2024 |
| Application and ECS engine | Bevy 0.19.1 |
| Networking integration | Lightyear 0.29.0 |
| Authoritative planar physics | Avian 2D 0.7.0 |
| Initial player platform | macOS client |
| Server platform | Headless dedicated-server processes; macOS is the local development baseline |
| Simulation frequency | Fixed 60 Hz |
| Licenses | Bevy is MIT/Apache-2.0; every additional dependency and shipped asset requires compatible licensing |

`Cargo.toml` is the exact build source of truth. The versions above describe the accepted engine
family and must be reconciled when the manifest changes.

Bevy, Lightyear, and Avian are exact-version dependencies with default features disabled. An engine
upgrade is a coordinated migration: verify mutual compatibility, feature graphs, protocol/content
identity, fixed scheduling, physics integration, client presentation, headless isolation, network
tests, routed process behavior, and native performance before accepting it. Do not upgrade one of
these dependencies casually or enable broad default feature sets for convenience.

## Workspace and runtime roles

The workspace has two demonstrated package boundaries:

- the main `brawler` package owns shared gameplay/data modules and composes the client and
  authoritative server applications;
- `packages/brawler-routing` owns the transport-neutral route, capability, manifest, IPC, limits,
  allocation, and supervisor/runtime boundary shared by routed processes.

The production topology contains several process roles:

```text
windowed client Bevy App/World
        |
        | one public UDP endpoint
        v
supervisor/router process (no gameplay World)
        |
        +-- lobby worker: headless Bevy App/World + Lightyear authority
        |
        +-- match worker A: headless Bevy App/World + Lightyear + Avian + gameplay
        |
        +-- match worker B: headless Bevy App/World + Lightyear + Avian + gameplay
```

The supervisor is infrastructure authority, not Bevy gameplay or lobby authority. The lobby worker
owns authenticated lobby sessions, advertised game types, saved profiles, queues, and allocation
requests. Each match worker owns exactly one match's mutable gameplay, mode, map, terrain, physics,
replication, outcomes, and cleanup. A client owns local session state and presentation of the
authority to which it is currently connected.

Headless client automation may omit presentation while retaining the Lightyear client and normal
session/gameplay protocols. It is a verification configuration, not a dedicated server or a
separate offline simulation.

Create another package, executable, or public API only for a demonstrated platform, process,
feature-isolation, compile-time, testing, or reuse boundary. Organize ordinary gameplay concerns as
focused modules and plugins within the existing package.

## Bevy application composition

Every Bevy `App` is assembled from role-appropriate base plugins plus cohesive PewPew Blitz
plugins. A plugin groups systems, components, resources, messages, lifecycle, and schedule
registration for a recognizable responsibility; it is not required for every type or file.

### Windowed client

The supported player client composes:

- Bevy `DefaultPlugins` with the selected window, assets, nearest-neighbor image sampling, input,
  rendering, UI, animation, and audio features;
- Lightyear client plugins and the registered application protocol;
- shared timing and gameplay schedule definitions needed to consume network state and, where
  explicitly installed, execute client-side behavior;
- client session, flow, Dashboard, settings, input, diagnostics, and presentation plugins; and
- the sole 3D gameplay-world presentation plugin.

Windowed application state, widgets, cameras, render entities, audio instances, input devices, and
asset handles are client-owned. They never decide an authoritative gameplay outcome.

### Headless client

The headless client replaces `DefaultPlugins` with `MinimalPlugins`, a schedule runner, Bevy states,
and logging. It retains the client protocol/session composition and may drive bounded automation
intent through the same application contracts. It must not become an alternative authority path.

### Lobby and match workers

Server workers use `MinimalPlugins`, a fixed schedule runner, Bevy states, logging, Lightyear server
plugins, protocol registration, diagnostics, and their role-owned plugins. They do not install
windowing, rendering, UI, audio, device input, or client assets.

A match worker additionally installs authoritative map, movement, match, combat, ability, terrain,
and exactly one selected game-mode composition. Mode choice occurs during validated application
construction; a running match does not hot-swap its authoritative mode plugin graph.

The diagram and plugin descriptions are ownership contracts, not a requirement that every concern
become a separate crate or architectural layer.

## ECS data and authority contract

Bevy's `World` is the runtime model. Use components for entity-scoped state, resources for genuine
World-scoped state, systems for scheduled behavior, messages or observers for bounded facts and
lifecycle reactions, and states only for mutually exclusive application phases that benefit from
Bevy's state machinery.

Keep these concerns distinct even when their Rust types live near one another:

1. developer-authored content and rule definitions;
2. player-selected recipes or builds;
3. immutable server-resolved match snapshots;
4. mutable runtime ECS state;
5. protocol registration and stable wire shapes;
6. telemetry, diagnostics, and verification evidence; and
7. client presentation state.

Authoritative mutation belongs to the lobby or match worker that owns the relevant World. Clients
send intent, not positions, hits, damage, effects, scores, objective results, or map mutations.
Presentation and diagnostics may observe authoritative facts but do not become a second mutation
path.

Networked state uses stable player, match, definition, placement, projectile, and other domain IDs.
Process-local Bevy `Entity` identity never crosses the wire. A gameplay component may also be a
registered replicated component when that is the simplest correct representation; do not create a
duplicate transport DTO solely to imitate an architectural layer.

## Timing and schedule contract

Authoritative gameplay advances at a fixed 60 Hz from the shared `SIMULATION_TICK`. Wall-clock
`Update` may coordinate sessions, presentation, and process work, but it must not become a second
rate-dependent gameplay simulation.

The shared `FixedUpdate` phase orders the high-level gameplay sets as:

```text
Lifecycle
  -> ApplyDeferred
  -> Input
  -> Simulation
  -> Fire
  -> Finalize
```

The deferred-command boundary after lifecycle must remain explicit so newly spawned, defeated,
respawned, or removed entities have deterministic visibility before input and simulation. Combat
resolution continues through the ordered `FixedPostUpdate` sets:

```text
ProjectileSweep
  -> Damage
  -> Lifecycle
  -> TelemetryAndCues
  -> Finalize
  -> advance SimulationTick
```

Focused subsystems may add their own sets and ordering constraints, including physics refresh and
terrain collision work. Preserve meaningful `.before`, `.after`, `.chain()`, and `ApplyDeferred`
relationships at the composition point. A refactor must not change ordering or deferred-command
semantics accidentally; changes require schedule-focused tests.

Time-dependent tests advance Bevy fixed time or run the relevant schedule explicitly. They do not
wait on wall-clock sleeps to exercise gameplay rules.

## Physics and world coordinates

Avian 2D owns authoritative planar collision. Fighters, projectiles, permanent map geometry, and
generated terrain colliders use explicit collision layers and filters appropriate to their
interactions. Objectives, pickups, hazards, and decorative presentation are not implicitly one
undifferentiated collision family.

Destructible terrain is an authoritative quantized-occupancy subsystem. It rebuilds only dirty
collision chunks at the explicit physics-safe boundary. Visible meshes, materials, crater edges,
particles, and debris present terrain state but never define solidity.

The client maps authoritative planar coordinates through one tested adapter onto Bevy's X/Z ground
plane. Rendering the game in 3D does not introduce 3D gameplay physics, vertical authority, or a
second coordinate model. A future change to the physics dimensionality or authoritative movement
plane is an architecture change requiring explicit research and specification review.

## Networking integration

Lightyear supplies Bevy-native client/server lifecycle, input networking, replication,
interpolation, and transport integration. PewPew Blitz owns its registered application protocol,
stable identities, compatibility/content handshake, validation, authority rules, and routed
transport adapter.

The supported gameplay path is dedicated-server authority with replicated and interpolated client
state. Lightyear capabilities such as prediction, rollback, and lag compensation are not enabled
merely because the dependency provides them. Owner prediction remains an isolated experimental
feature, and any production prediction or lag-compensation path requires measured player benefit,
terrain and collision correctness, explicit lifecycle ownership, protocol verification, and
acceptance through a future milestone.

The routed transport carries opaque Lightyear datagrams between the public supervisor endpoint and
the selected lobby or match authority. It does not wrap replication in another snapshot protocol
or move gameplay decoding into the supervisor. Direct UDP remains an explicitly named diagnostic
comparison path, not the ordinary product topology.

Protocol registration remains centralized in `protocol.rs`. Application messages follow one global
compatibility handshake and current schema; do not add per-message versions or compatibility
decoders without a new validated network decision.

## Client presentation, UI, input, and assets

The supported gameplay-world renderer is 3D. It uses `Camera3d`, meshes, PBR materials, lighting,
generated map and terrain meshes, GLB scenes, and animation while presenting the authoritative
planar game. Bevy UI owns the Dashboard, menus, HUD, overlays, and projected fighter information.
The primitive-world override supplies deterministic fallback meshes inside this same renderer; it
is not a second renderer or a permanent content mode.

Gameplay emits or replicates presentation-independent state and cues. Client systems resolve those
facts into models, animation, particles, audio, camera response, controller feedback, and UI.
Missing, disabled, reduced, or late presentation must not change navigation, networking, saving,
shutdown, or gameplay outcomes.

Device input is sampled on the client and converted into abstract gameplay and product actions.
Aim, movement, fire, abilities, interaction, and menu actions remain independent from physical
bindings. Keyboard/mouse and an Xbox-like controller are supported desktop schemes; later device
or touch schemes must adapt at this boundary rather than enter gameplay systems.

Client assets are presentation data resolved from stable definition or cue references. The server
uses gameplay definitions and content fingerprints without loading client meshes, textures, fonts,
audio, animation, or shaders.

## Content and authoring boundary

Gameplay content is expressed through bounded serializable Rust definitions and authored data
files. Focused Bevy systems implement behavior; data selects among supported capabilities. Do not
use executable user scripts, client-selected system names, unbounded numeric maps, or a serialized
ECS `World` as the authored source of truth.

Built-in and future user-authored maps use the same typed recipe, validation, resolution, and
runtime-instantiation path. User-editable layout data remains separate from developer-authored
authoritative mode plugins. Likewise, built-in brawler and weapon presets use the same bounded
recipe and resolution path as player-authored variations.

Prefer Bevy-native components, resources, systems, messages, assets, states, and UI before adding a
custom framework. Add a scripting language, editor framework, general command bus, or other engine
layer only when a concrete player-facing slice demonstrates the need.

## Cargo features and role isolation

The main package's feature graph expresses compile-time execution roles:

- `client` is the default and enables the windowed/player client dependency surface;
- `server` enables the dedicated-server and worker surface without Bevy presentation features;
- `network-test` combines the required client/server and in-process transports for integration and
  performance tests;
- `process-metrics` is an isolated measurement feature because its recorder is process-global; and
- `owner-prediction` is an experimental comparison feature, not part of the supported player path.

Keep role-owned modules gated at their ownership boundary. The server graph must not acquire
windowing, rendering, PBR, scenes, UI, text rendering, audio, gamepad/keyboard/mouse input, or client
asset dependencies through a convenient shared module. Moving a type or dependency across a role
boundary requires role-specific compilation and dependency checks.

Runtime `headless` selection controls whether a client installs presentation; it does not replace
Cargo feature isolation. Conversely, the `server` feature is not a windowed client with rendering
disabled at runtime.

## Verification and evolution

Engine-facing changes are verified in proportion to the boundary they affect:

- pure rules use focused unit tests;
- components, resources, lifecycle, states, and schedules use small Bevy `App`/`World` tests;
- authority and replication use the separate-App network harness;
- routing and process isolation use the routed process tests and canonical E2E path;
- client rendering, UI, input, animation, and audio use automated diagnostics plus bounded native
  visual/controller checks; and
- feature-boundary changes run client, server, and relevant combined-role compilation checks.

Visual evidence complements automated correctness; it does not replace authority, lifecycle,
protocol, or schedule tests. Test-only composition should reuse production plugins and schedules
unless the test explicitly owns a smaller unit boundary.

Reconsider an engine, physics, or networking dependency only when a concrete blocker, unsupported
platform, unacceptable maintenance burden, correctness failure, or measured product limitation
justifies the migration cost. Alternative engines and networking stacks are not standing fallback
architectures. A replacement proposal must specify authority preservation, content migration,
feature isolation, process topology, verification parity, and player-visible value before changing
the supported baseline.
