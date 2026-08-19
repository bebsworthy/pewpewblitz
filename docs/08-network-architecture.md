# Network architecture

## Architectural decision

Brawler is first and foremost a networked competitive game. The target architecture is a **dedicated-server-authoritative simulation**. The local gameplay prototype is only a development harness; it must not become a separate client-authoritative implementation that later needs to be rewritten for online play.

The planned implementation is Bevy 0.19 with Lightyear 0.29. Bevy core is intentionally modular and does not provide first-party networking; Lightyear supplies the networked game layer. Bevy supports headless applications by omitting rendering/window features, while Lightyear provides separate client/server plugins, input buffering, replication, prediction, rollback, interpolation, and transport layers. See the version-pinned [Bevy headless example](https://docs.rs/crate/bevy/0.19.0/source/examples/app/headless.rs), [Lightyear 0.29 documentation](https://docs.rs/lightyear/0.29.0/lightyear/), and [Lightyear repository](https://github.com/cBournhonesque/lightyear).

This dependency choice is a deliberate risk, not an invisible assumption. v1 Milestones 01–03 validate the actual version combination through application composition, two-client connection/replication, and server-authoritative movement before the project commits to substantial combat content; there is no separate throwaway engine spike.

V2 concurrency extends this authority model through isolated match-worker processes rather than
placing multiple mutable matches in one Bevy world. The proposed supervisor, single-public-port,
and UDP/IPC routing decision is recorded in
[Multi-process server and single-port UDP/IPC transport](./14-multiplayer-server-architecture.md).
This document remains the gameplay authority and replication contract inside each match worker.
The lobby worker follows the same server-authority and stable-identity principles but owns session,
catalog, queue, and reservation state rather than combat simulation.

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
- request selection of a preset or brawler build using stable identity/revision data;
- send timestamped input commands or input frames;
- predict local movement later if needed;
- render the latest authoritative state;
- interpolate remote fighters;
- play visual, audio, and camera effects;
- display HUD and scoreboard;
- never decide whether a weapon recipe is legal or directly install resolved weapon values;
- never decide authoritative damage, hits, deaths, status triggers, scores, or terrain edits.

### Server responsibilities

- own the match lifecycle and mode rules;
- retrieve or receive candidate brawler builds, validate their weapon recipes, and create immutable
  resolved match loadouts;
- simulate fighter movement and abilities;
- validate fire commands and cooldowns;
- perform projectile, hit, damage, and collision resolution;
- own status meters and threshold effects;
- own pickups, objectives, scores, respawns, and victory;
- own destructible terrain occupancy grids and collision regeneration;
- derive observer-specific concealment and reveal outcomes before replication when those mechanics
  are implemented;
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

Authored content/rule definitions may be serializable Rust data or Bevy assets/configuration loaded
into the worlds that need them. Player-authored brawler builds and weapon recipes are separate from
those definitions, and immutable resolved match loadouts are separate from mutable runtime state.
Runtime authority lives in server components, resources, entities, states, and scheduled systems.
Client-only presentation observes replicated gameplay state or presentation messages and must not
become gameplay truth.

Use a shared gameplay plugin or module only for systems that genuinely execute on both server and client, primarily when prediction requires identical fixed-step behavior. Server-only match, validation, damage, score, terrain, and lifecycle rules remain server-only. Package and folder boundaries are implementation decisions made from feature and dependency evidence, not part of the network contract.

## Application protocol compatibility and evolution

Brawler uses one current Lightyear application-protocol schema guarded by one global compatibility
handshake. Before a lobby or match authority admits the session, it validates exact agreement on:

- `SUPPORTED_PROTOCOL_VERSION`;
- the application build version;
- the protocol-registry fingerprint covering registered inputs, replicated components, messages,
  channels, directions, and delivery configuration; and
- the gameplay-content fingerprint covering the shared authored definitions required by that
  authority.

Application messages use responsibility-based names such as `LobbyHello`, `LobbyJoinOutcome`,
`MatchHello`, and `MatchJoinOutcome`; they do not carry `V1`/`V2` suffixes or negotiate independent
message versions. There is one decoder for the current global protocol. An incompatible change to a
registered message/component/input/channel shape, enum meaning, direction, delivery contract, or
canonical application encoding increments `SUPPORTED_PROTOCOL_VERSION` and updates every client,
server role, automation path, fixture, and protocol-fingerprint expectation atomically. A peer with
a different global protocol, build, registry, or required content fingerprint is rejected rather
than partially supported or retried with another message dialect.

The compatibility hello remains small and decode-time bounded. A bounded hello that can be decoded
but carries mismatched compatibility fields receives the relevant structured rejection; malformed
or undecodable input is closed and classified as an incompatible handshake. All later variable
collections and strings remain independently bounded at decode time even after compatibility has
succeeded.

Brawler does not maintain parallel application-message decoders, fallback hellos, or compatibility
shims for unreleased schemas. A future need for rolling deployments, public-server/client skew, or a
released migration window must return to architecture review and define the supported-version
matrix, lifetime, negotiation boundary, and removal gate before adding such machinery.

Independent schema/version fields remain appropriate when the artifact is decoded outside this
Lightyear handshake or survives a connection: local persistence files, operator configuration,
public route envelopes, packet/control IPC frames, and process manifests. Those versions protect
their own storage or pre-handshake framing boundaries; they do not create per-message application
compatibility. Each boundary still keeps only its current decoder unless a concrete deployment or
migration requirement explicitly justifies more.

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

Build selection and weapon editing are not per-tick combat input. They use ordered, idempotent
requests tied to the receiving authenticated/session connection. A request never authoritatively
names another fighter or installs ECS runtime state.

### Weapon-recipe authority

Keep these network concepts separate:

```text
Shared content/rule catalog fingerprint
  proves client/server schema and known primitive compatibility

Candidate brawler build / weapon recipe
  preset lookup in Milestone 05
  bounded client proposal in Milestone 08
  server-side stored arsenal revision when persistence exists

Resolved match loadout
  created and owned by the server
  replicated as stable identity plus the bounded public configuration needed for HUD/presentation

Runtime weapon state
  ammo/charges, cooldowns, projectiles, effects; mutated only by server simulation
```

Milestone 05 clients request a built-in preset ID; the server loads and resolves the recipe from its
own catalog instead of trusting client-supplied damage, range, behavior, or payload values. A later
bounded editor may send a typed recipe proposal, but the server must independently validate known
primitives, finite/ranged values, supported combinations, budget/slot policy, and—when persistence
exists—build revision and entitlement. Invalid or stale recipes do not create or mutate a fighter.

Per-player recipes are authoritative session/build data, not part of the global gameplay-content
fingerprint. The fingerprint covers the shared schema, primitive catalog, and built-in presets. The
server replicates the accepted resolved public configuration so late join and reconnect do not
depend on every client having a player's custom recipe preinstalled.

### Map-recipe and mode authority

Map authoring follows the same definition/recipe/resolved/runtime separation without allowing map
recipes to become game-mode programs:

```text
Shared map-content catalog and mode schemas
  known presentation, geometry, terrain, entity, region, and anchor definitions

Candidate map recipe
  one built-in preset in Milestone 06
  future bounded user-authored layout/revision

Resolved map
  immutable server-validated geometry, regions, placements, presentation references, and mode ID

Runtime map state
  spawned entities, objective state, active regions, and terrain revisions owned by the server
```

Milestone 06 loads one built-in recipe through the future-compatible resolver. A future builder may
produce a typed candidate recipe, but the server validates catalog/schema versions, IDs, finite and
bounded geometry, complexity/count budgets, spawn safety, objective requirements, mode
compatibility, and allowed references before installing it. A client cannot submit collision,
spawn, region, entity, or terrain changes directly to a running match.

The selected `ModeDefinitionId` resolves only to a server-installed mode plugin. A map recipe may
place that mode's required anchors and choose explicitly exposed parameters, but cannot define or
replace scoring, victory, respawn, objective, or other executable rules. The server replicates the
accepted map identity/revision and resolved data needed for client reconstruction. Distribution,
asset upload, persistence, publishing, discovery, moderation, and migration of user maps are later
network/service concerns.

User-map identity/revision is authoritative match data, not a substitute for the shared catalog and
mode-schema fingerprint. The shared fingerprint proves that both roles understand the referenced
primitive and presentation IDs; the server-approved recipe/revision identifies the actual layout.

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

Terrain destruction is server-authoritative. The server quantizes the destruction brush into terrain
cells, updates the terrain revision, and broadcasts a compact deterministic destruction command or
affected-chunk update. Clients reconstruct the same visual crater locally. Do not send a full terrain
texture or whole-map grid after every explosion.

Milestone 10 defines the live terrain event as:

```text
TerrainDestructionEvent
  map_instance_id and match_id
  terrain_fingerprint
  terrain_revision
  attack_id and delivery_index
  half-cell brush center and radius
  affected terrain-chunk IDs
  erased-cell count
```

Brush coordinates are bounded integers in half-cell units and have exactly one canonical cell
rasterization. Peers never independently round unconstrained floating-point geometry. Live events
use a dedicated ordered-reliable terrain channel; revision recovery sends bounded current chunk
bitsets rather than replaying complete history.

The server remains responsible for collision and gameplay truth. Clients use the event for
presentation and prediction only. Each client tracks the latest applied terrain revision. A gap must
trigger authoritative recovery from initial occupancy, affected-chunk bitsets, or retained event
history; reconnecting and late-joining clients cannot be required to have observed every live
destruction event. Recovery is sparse and region/chunk scoped so its memory and bandwidth do not
scale with empty space on larger maps.

## Interest management and concealment

Future tall grass, smoke, darkness, and invisibility mechanics use server-owned network interest
management as part of their gameplay rule. The server continues to simulate the absolute state of
the match, including hidden fighters and bots, but derives network visibility separately for every
observer connection and potentially hidden spatial entity.

Lightyear 0.29 provides two complementary mechanisms:

- `RoomPlugin` and `Rooms` provide coarse, semi-static filtering for arena regions or broad spatial
  partitions inside one authority world;
- `VisibilityExt::gain_visibility` and `VisibilityExt::lose_visibility` provide dynamic
  per-entity, per-connection visibility for observer-specific concealment and reveal.

Rooms do not replace the gameplay visibility calculation. Opponents may share one arena room while
receiving different visibility outcomes for the same fighter. The authoritative concealment system
must run after relevant movement/effect/reveal state is resolved and apply visibility before the
replication send path is assembled.

Secret live spatial state uses ordinary while-visible loss semantics: a subject hidden before first
relevance is not spawned for that client, and a previously visible remote entity is despawned when
visibility is lost. Retained and always-present policies are not valid for secret live state because
they preserve the remote entity and its initial or last-known data while updates are paused.

When an always-visible participant roster is required, represent public participant information
separately from the cullable spatial fighter:

```text
Public participant
  stable player identity, team, connection/defeat state, public score

Cullable spatial fighter
  pose, facing, live presentation hierarchy, private effects and spatial state
```

The visibility boundary must include replicated descendants and related messages. Health bars,
targeting markers, weapon children, status effects, projectiles, damage events, sounds, objective
state, score updates, and telemetry can otherwise reveal a hidden subject even when its pose is
culled. Owner control remains intact; permitted owners/allies receive the spatial fighter while
opponents do not.

This closes the normal packet-sniffing wallhack path for current hidden spatial state, but it is not
a complete anti-cheat claim. Clients retain previously delivered state and can observe disappearance
or reappearance timing; in-flight historical packets and traffic side channels require separate
threat analysis if they become material.

The implementing milestone must test hidden-before-join, visible-to-hidden despawn, owner/ally
exceptions, two observers with different outcomes, current-state reappearance, late join,
reconnect, defeat/respawn, hierarchy cleanup, interpolation/prediction cleanup, and absence of
subject-derived private components/messages while hidden. See
[Environment, surface, and tile ideas](./09-environment-and-tile-ideas.md#network-interest-management)
for the complete research catalog and verification list. The exact Lightyear 0.29 behavior is
demonstrated in the checked-in `references/lightyear/examples/network_visibility/` example and the
version-pinned [network visibility example](https://github.com/cBournhonesque/lightyear/blob/0.29.0/examples/network_visibility/README.md).

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
7. **Future environment/concealment milestone:** surface effects remain server-owned while per-client visibility culls secret spatial state and recovers correctly at reveal, late join, and reconnect.
8. **Future systemic-status milestone:** accumulating meters, threshold triggers, immunity, and duration remain server-owned and recover correctly.

Prediction, lag compensation, advanced interpolation tuning, anti-cheat hardening, matchmaking, authentication, session services, and production hosting may be developed after the relevant early gates. The authority boundary, state recovery rules, and explicit connection lifecycle outcomes may not be postponed.
