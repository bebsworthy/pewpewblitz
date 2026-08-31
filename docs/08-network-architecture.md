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
- derive manual or viewport-bounded assisted aim from currently presented client state and encode
  both through the same ordinary input intent, without sending a selected target identity;
- present a caller-supplied development account ID at lobby admission and request profile mutations
  or selected-brawler admission using stable identity/revision data;
- validate and install the accepted connection-scoped profile, game-type catalog, and advertised
  brawler catalog before entering Dashboard, then clear them together when that lobby ends;
- send timestamped input commands or input frames;
- predict local movement later if needed;
- render the latest authoritative state;
- interpolate remote fighters;
- play visual, audio, and camera effects;
- display HUD and scoreboard;
- interpolate an ammunition-recovery indicator from replicated server deadlines without granting
  ammunition or deciding fire readiness;
- never decide whether a weapon recipe is legal or directly install resolved weapon values;
- never decide authoritative damage, hits, deaths, status triggers, scores, or map mutations.

### Server responsibilities

- own the match lifecycle and mode rules;
- load the server-owned profile, resolve its selected saved brawler, and create immutable resolved
  match loadouts without accepting client-authored combat values at queue admission;
- derive and advertise the bounded legal brawler inventory and player-facing metadata from active
  server content, and validate profile references against that same catalog;
- simulate fighter movement and abilities;
- validate fire commands and cooldowns;
- own current health, attack-idle health recovery, ammunition stock, and each ammunition-recovery
  interval;
- perform projectile, hit, damage, and collision resolution;
- own status meters and threshold effects;
- own pickups, objectives, scores, respawns, and victory;
- own map-asset placement outcomes and collider updates;
- derive observer-specific concealment and reveal outcomes before replication;
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

The role application builders own that selection explicitly. `GameplayContentPlugin` installs the
validated headless-safe catalogs and content fingerprint in every role that participates in
compatibility checks. `ProtocolPlugin` registers only wire messages, channels, replicated
components, interpolation, inputs, and the protocol-registry fingerprint. The server application
adds `ServerAuthoritativeGameplayPlugin`, session networking, and routed-worker transport as
siblings; the client application similarly adds `ClientReplicatedGameplayPlugin`, session
networking, and optional presentation as siblings. A protocol or session/transport plugin must not
select gameplay or presentation transitively. The minimum lobby worker opts into shared content,
protocol, lobby authority, and routed transport without installing match gameplay.

Authored content/rule definitions may be serializable Rust data or Bevy assets/configuration loaded
into the worlds that need them. Player-authored brawler builds and weapon recipes are separate from
those definitions, and immutable resolved match loadouts are separate from mutable runtime state.
Runtime authority lives in server components, resources, entities, states, and scheduled systems.
Client-only presentation observes replicated gameplay state or presentation messages and must not
become gameplay truth.

Use a shared gameplay plugin or module only for systems that genuinely execute on both server and client, primarily when prediction requires identical fixed-step behavior. Server-only match, validation, damage, score, map-mutation, and lifecycle rules remain server-only. Package and folder boundaries are implementation decisions made from feature and dependency evidence, not part of the network contract.

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

The gameplay-content fingerprint is assembled from a bounded registry populated by shared,
headless-safe content plugins. Each gameplay domain owns a stable textual domain ID, a domain
schema version, and a read-only projection from its live Bevy resource to canonical bytes. The
global envelope sorts contributions by domain ID and hashes the ID, schema version, and material,
so plugin installation order cannot change compatibility and a new shared catalog can contribute
without editing a central catalog tuple. Duplicate IDs, missing required built-in domains,
capacity overflow, invalid cross-catalog references, and unavailable resources fail closed. The
routed supervisor, lobby worker, match worker, direct server, client, and Startup finalizer all
evaluate the same registrations; pre-Startup resource overrides therefore cannot diverge from the
identity advertised by production roles.

Only authored gameplay definitions required by both sides may contribute. Client-only visual and
audio catalogs, server-only operator configuration, mutable ECS runtime state, and Balance Lab
runtime tuning remain outside this envelope. Domain contributors do not create independent
compatibility negotiation: the application still exposes one current gameplay-content fingerprint
and one global mismatch rejection boundary. Content envelope format 27 introduced this stable-ID,
sorted contribution framing without changing the handshake or `GameplayContentFingerprint` wire
shape.

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

Physical-device selection, gamepad neutral calibration, reticle visibility, and viewport-bounded
neutral-fire assistance are client-local. Assistance may choose direction and requested distance
from currently presented entities, but it cannot reveal off-screen or unreplicated state and does
not create a distinct protocol action. The server receives the ordinary aim vector/distance and
continues to own action acceptance, range and bounds repair, collision, recipients, and effects.

Saved-brawler and weapon-equipment edits are not per-tick combat input. They use ordered,
idempotent profile requests tied to the receiving authenticated lobby connection. A request never
authoritatively names another fighter or installs match ECS runtime state.

### Lobby profile and brawler-catalog installation

The accepted lobby envelope installs one coherent membership:

```text
LobbyJoinOutcome::Accepted
  logical server and player identity
  bounded game-type catalog + revision
  bounded AdvertisedBrawlerCatalog + revision
  bounded server-owned ProfileSnapshot
```

The brawler advertisement is derived by lobby authority from its active build and weapon catalogs.
It carries stable fighter-profile, weapon-base, ultimate, and passive identities plus the bounded
names, kinds, eligibility, statistics, weapon configuration/policy, and limits required by the
player flow. The complete welcome remains under the existing 64 KiB envelope bound; each variable
collection and the brawler catalog's 16 KiB encoded ceiling are enforced while decoding.

The client validates game types, brawler-catalog structure/revision, profile structure, and profile
references before installing any of them. A conflicting, partial, malformed, or incompatible batch
fails the lobby join; there is no locally reconstructed fallback. Membership, profile, and both
catalogs are connection-scoped and clear on disconnect, server change, rejected replacement, or
lobby-generation change. A reconnect repeats the authoritative load and atomically replaces the
old client mirror.

This defensive client validation does not transfer authority. Clients still send only stable-ID,
revision-bound profile intent. Storage owns structural validity, while lobby authority validates
active content and returns the complete authoritative profile outcome.

### Weapon-recipe authority

Keep these network concepts separate:

```text
Shared content/rule catalog fingerprint
  proves client/server schema and known primitive compatibility

Advertised brawler catalog
  bounded server-derived legal inventory and presentation/preview metadata for this lobby session

Selected saved-brawler identity and revision
  bounded client intent tied to the accepted lobby session
  resolved against the lobby's server-owned profile snapshot

Resolved match loadout
  created and owned by the server
  replicated as stable identity plus the bounded public configuration needed for HUD/presentation

Runtime weapon state
  ammo/charges, fire cooldown, and next-ammunition start/target ticks; mutated only by server simulation
```

The client derives a filling ammunition segment from the replicated interval and its latest
authoritative tick. It may extrapolate presentation, but it never restores ammunition locally;
fire intent continues normally and the server alone accepts or rejects it from current state.

V7 clients request admission with a selected `SavedBrawlerId` and expected brawler revision. The
lobby resolves the authored fighter profile, weapon base, four or fewer owned part instances,
ultimate, and passives from its accepted profile cache against active catalogs. Invalid, stale,
unselected, incompatible, or mutation-in-flight state does not create a queue ticket. The resulting
V3 snapshot crosses routing opaquely with fixed-order canonical weapon modifiers and the accepted
resolved identity; it contains no `AccountId`, part-instance metadata, inventory, or database
authority. The match worker re-resolves and verifies that immutable snapshot before spawning combat.

Per-player equipped instances and exact persisted rolls are authoritative profile/session data, not
part of the global gameplay-content fingerprint. The fingerprint covers the shared schema,
primitive/base catalogs, and authored weapon-part catalog. The server replicates the accepted
resolved public configuration so late join and reconnect do not depend on every client having a
player's inventory preinstalled.

### Map-recipe and mode authority

Map authoring follows the same definition/recipe/resolved/runtime separation without allowing map
recipes to become game-mode programs:

```text
Shared map-content catalog and mode schemas
  known map assets, gameplay profiles, presentation profiles/themes, and anchor schemas

Candidate map recipe
  bounded dimensions, default surface, sparse MapAssetId placements, and typed anchors

Resolved map
  immutable server-validated placements, derived collision/spawns, presentation references, and mode ID

Runtime map state
  spawned colliders, damageable-object health, pickups, objective state, and terminal placement
  outcomes owned by the server
```

Built-ins resolve through one sparse-grid catalog and recipe path. A future builder may produce a
typed candidate recipe, but the server validates catalog/schema versions, IDs, bounded cells,
footprints and counts, gameplay-profile coherence, spawn safety, objective requirements, mode
compatibility, and allowed references before installing it. A client cannot submit collision,
spawn, placement, anchor, or map-state changes directly to a running match.

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
  fighter and projectile state, including CurrentHealth and WeaponState
  immutable ProjectileBody shape plus trajectory-specific flight state
  authoritative tick reference used with fire and ammunition-recovery deadlines
  active effects, damageable-object health/life state, restoration pickups, and objective state
  scores, resolved map snapshot, and map dynamic generation/revision

Gameplay message, when required
  event_id
  stable source/target network identity
  event_type
  payload
```

Do not introduce a custom aggregate `Snapshot` wrapper when Lightyear component replication provides the needed behavior. Register concrete inputs, replicated components, messages, and channels through Bevy plugins, including delivery and ordering semantics. Network data must not expose process-local ECS entity identity unless Lightyear's entity mapping explicitly handles it.

`ProjectileBody` is the one replicated planar geometry fact for projectile collision and
presentation. It currently carries `ProjectileShape::Circle { radius }` independently from
`StraightFlight`. The server derives muzzle clearance, installed Avian collider, and fixed-tick
sweep from that body. Clients may reconstruct a read-only shape sweep over their resolved map and
currently replicated entities to clip an aim guide, but that index is presentation state: it runs
no gameplay physics schedule, predicts no hidden target, and cannot accept a hit or mutate a map.
Adding another shape is an exact current-protocol/content-schema change and updates collision,
replication, visual footprint, aim trace, and verification as one capability.

Persistent Splash areas are ordinary replicated combat entities after their authoritative lob
lands. `PersistentSplashState` carries only the public presentation/recovery facts: center, facing,
circle or oriented-rectangle shape, activation/next-pulse/expiry ticks, pulse interval, occlusion
and target ceilings, and the two bounded effect definitions. `ReplicatedAttackSource` carries the
stable attack and owner attribution. The server alone retains the resolved recipe, match generation,
delivery index, candidate occupancy, recipient decisions, and payload application. Late join and
reconnect therefore reconstruct a live area from durable component state rather than replaying its
landing or earlier pulse cues.

Created areas remain valid if their owner is defeated or disconnects; they are removed by expiry or
match-generation teardown. Clients never send area entry, exit, pulse, damage, healing, or status
claims, and no process-local entity identity is part of the Splash wire contract.

## Dynamic map synchronization

Map destruction and V10 environment-object state are server-authoritative. An accepted world effect
resolves its bounded radius against current destructible placements and commits each overlapping
placement once as either removed or replaced. The same transaction updates authoritative colliders
and increments one map revision; clients never rasterize collision or independently decide which
cells were affected.

Damageable barrels and chests retain stable generation/placement identity plus replicated current
health and life state. Their terminal transaction commits the map placement outcome and collider
change exactly once. A chest additionally creates one generation-derived public restoration pickup
with replicated position, definition, availability, and expiry. Heist objectives use stable
match/map/anchor/team identity and replicated public health but remain mode-owned rather than map
placements. Clients never submit contact, damage, health, terminal, pickup, or objective outcomes.

The map root replicates `ResolvedMapSnapshot` and the current `MapDynamicState` once for bootstrap
and late join. Live ordered-reliable `MapMutationEvent`s carry an exact generation, revision, and
bounded list of `(MapPlacementId, MapPlacementOutcome)` transitions. A reset publishes the exact
old/new generation pair. Clients accept only the embedded catalog/schema/fingerprint identity,
apply contiguous transitions, ignore duplicates and stale generations, and close presentation
readiness on invalid state.

A revision gap triggers `MapDynamicRecoveryRequest` for the current generation. The active server
session may return a bounded `MapDynamicRecoverySnapshot` containing current terminal outcomes;
requests for foreign/stale generations, inactive or disconnected sessions, and response-rate
exhaustion are rejected. Recovery never replays unbounded destruction history. The server remains
responsible for collision and gameplay truth; client reconstruction is presentation and optional
prediction support only.

Damageable-object readiness joins the resolved map generation with the complete expected live
object set; Heist readiness additionally requires the matching mode objective set. Live pickups are
ordinary durable replicated entities and therefore converge for late join/reconnect without cue
replay. A missing or generation-mismatched set keeps presentation in its explicit syncing state
rather than showing stale health, collision, or loot availability.

## Interest management and concealment

Concealing tall grass, Self Cloak, and Concealment Field use server-owned network visibility as part
of their gameplay rule; future smoke or darkness must join the same observer-specific contract.
The server continues to simulate the absolute state of
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

V10 barrels, chests, restoration pickups, Heist objectives, and their health/availability facts are
public and never become concealment subjects. A cue that carries an initiating fighter or other
fighter-derived source still keeps that subject identity and passes through the observer-specific
filter after the accepted attack/reveal transition; public object state cannot be used to copy a
hidden source into an unfiltered field.

This closes the normal packet-sniffing wallhack path for current hidden spatial state, but it is not
a complete anti-cheat claim. Clients retain previously delivered state and can observe disappearance
or reappearance timing; in-flight historical packets and traffic side channels require separate
threat analysis if they become material.

The completed concealment suite tests hidden-before-join, visible-to-hidden despawn, owner/ally
exceptions, two observers with different outcomes, current-state reappearance, late join,
reconnect, defeat/respawn, hierarchy cleanup, interpolation/prediction cleanup, and absence of
subject-derived private components/messages while hidden. See
[Environment gameplay direction](./09-environment-gameplay.md#concealment-gameplay-model) for the
gameplay questions and presentation-facing verification expectations. The exact Lightyear 0.29
behavior is demonstrated in the checked-in `references/lightyear/examples/network_visibility/`
example and the version-pinned
[network visibility example](https://github.com/cBournhonesque/lightyear/blob/0.29.0/examples/network_visibility/README.md).

## Status synchronization

The server owns internal status meters such as `cold`, threshold checks, freeze duration, decay, resistance, and immunity. Clients may receive the meter value for HUD feedback, but cannot apply or trigger the status themselves.

## Combat recovery synchronization

Health recovery is simulation state, not a client timer. The match worker owns the accepted-attack
idle origin and fractional recovery remainder, mutates integer `CurrentHealth` on fixed ticks, and
replicates only the resulting public health. Clients do not need the private accumulator to present
health accurately.

Ammunition recovery needs visible partial progress, so the replicated `WeaponState` contains:

- current authoritative ammunition or charges;
- fire readiness as `Ready` or a server-authored cooldown target tick; and
- an optional `AmmoRecovery` interval with authoritative start and target ticks.

The fighter also receives the latest `AuthoritativeTick` observed alongside its replicated state.
The HUD may interpolate `(tick - started) / (ready - started)` between updates and clamp it to the
valid interval. That interpolation is presentation only: the client never increments stock, clears
the interval, shortens a deadline, or treats a visually complete segment as permission to fire.
Only the server deadline transaction restores one unit, and only replicated stock confirms that
the unit is usable. This keeps firing authoritative under delay, loss, late join, and reconnect
while still supplying enough information for a smooth filling bar.

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
6. **V8 map dynamics:** connected and late/reconnecting clients converge on authoritative
   whole-placement removal/replacement state and exact map generations.
7. **V9 concealment and reveal:** server-owned terrain/ability sources feed per-client visibility,
   which withholds secret spatial state and recovers current state correctly at reveal, late join,
   and reconnect.
8. **V12 combat recovery:** health accumulation, ammunition stock, fire cooldown, and per-round
   recovery deadlines remain server-owned and converge under delayed replication, late join, and
   reconnect.
9. **Future systemic-status milestone:** accumulating meters, threshold triggers, immunity, and duration remain server-owned and recover correctly.

Prediction, lag compensation, advanced interpolation tuning, anti-cheat hardening, matchmaking, authentication, session services, and production hosting may be developed after the relevant early gates. The authority boundary, state recovery rules, and explicit connection lifecycle outcomes may not be postponed.
