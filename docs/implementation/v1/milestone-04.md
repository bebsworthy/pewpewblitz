# Milestone 04 — Combat core

## Tracking

| Field | Value |
|---|---|
| Version | v1 — gameplay MVP |
| Roadmap | [roadmap.md](./roadmap.md) |
| Status | Complete |
| Specification validation | Accepted by implementation request |
| Implementation | Core combat slice, review corrections, and supervised combat evidence complete; accepted playtest feedback implemented |
| Verification | Automated core, deterministic fixture, repeated UDP profiles with measured fire-to-cue evidence, keyboard/mouse/render-profile smoke, and synthetic controller smoke green; hardware-only checks explicitly deferred by user approval |
| User validation/playtest | Keyboard/mouse smoke recorded, feedback triaged, and Milestone 04 approved; physical-controller and alternate-refresh observations remain deferred rather than represented as passed |

Update this table and the roadmap together whenever the milestone changes phase.

## Outcome

One code-authored pulse sidearm completes the first dedicated-server-authoritative combat loop:
held fire intent becomes rate-limited server shots, straight projectiles sweep against current
authoritative geometry, hostile hits reduce replicated health, defeats retain attribution, and the
sandbox restores defeated fighters after a short delay. Both clients converge on projectile,
health, weapon, defeat, and reset state and receive presentation-only combat cues. Debug HUD and
placeholder effects make hits, ammo, reload, and defeat legible. Server telemetry records shots,
hits, damage, defeats, and hit distance.

Milestone 03 remains in `Verifying` for its own delayed/lost-input, owner-prediction, windowed
interpolation, and physical-controller evidence. Milestone 04 is independently complete against
its documented combat authority and evidence contract; the remaining hardware-only observations
are explicitly deferred by user approval and are not represented as passed.

## Source requirements

- [Product direction](../../00-product-direction.md): combat readability, short feedback cycles,
  composable content, and network-first simulation.
- [Fighter model](../../02-fighter-model.md): authored definitions, selected builds, and runtime
  state are distinct lifecycles; recommended first-iteration health, movement, damage, range,
  projectile speed, reload, and ammo attributes.
- [Weapons and abilities](../../03-weapons-and-abilities.md): weapon composition, straight
  projectiles, collision and payload separation, stable presentation cue IDs, and pulse-sidearm
  role.
- [Gameplay MVP](./gameplay-mvp.md): fire, damage, defeat/reset, health bar, hit/defeat
  feedback, controller-first controls, and server authority.
- [Network architecture](../../08-network-architecture.md): clients send intent; the server owns
  firing, projectiles, hits, damage, defeat, and reset; durable state uses replicated components
  while discrete outcomes use registered messages when needed.
- [Version 1 roadmap](./roadmap.md): Milestone 04 scope, evidence rules, telemetry, impairment,
  and exit criteria.
- [Milestone 01](./milestone-01.md), [Milestone 02](./milestone-02.md), and
  [Milestone 03](./milestone-03.md): exact dependency and feature topology, fixed-tick and network
  lifecycle contracts, stable identities, native input, authoritative movement, Avian integration,
  collision policy, interpolation, and learn-from-errors constraints.

## Scope boundaries

### In scope

- one code-authored fighter definition, one code-authored pulse-sidearm definition, and one selected
  sandbox build using stable definition IDs;
- maximum/current health, sandbox team affiliation, alive/defeated state, and server-owned
  ammo/cooldown/reload state;
- held primary-fire intent through the existing `FighterInput` native-input path;
- authoritative shot cadence, automatic reload, muzzle origin, projectile spawn, straight travel,
  radius sweep, range/lifetime, first valid impact, and cleanup;
- direct hostile damage, friendly-fire and self-hit filtering, deterministic same-tick damage
  ordering, defeat attribution, posthumous projectile outcomes, and environmental attribution
  representation;
- one stationary neutral-hostile test dummy with reserved stable identity;
- a fixed sandbox defeat delay and full reset that reuses the original spawn position/facing;
- durable replication for definitions/build selection, health, weapon state, defeat state, and
  projectiles, plus a presentation-only ordered combat-cue stream;
- debug health/ammo/reload HUD, projectile visual, muzzle/impact flash, local hit confirmation,
  target hit flash, defeat feedback, and reset cleanup;
- server-side combat counters and structured logs for shots, hostile hits, applied damage,
  defeats, and distance bands;
- deterministic authority/duplication cases and statistical local/typical/adverse packet-delay,
  jitter, and loss profiles through the existing Crossbeam and UDP workflows.

### Out of scope

- Milestone 03 owner prediction implementation, predicted/prespawned projectiles, client-side hit
  prediction, lag compensation, server rewind, or changing the accepted interpolation decision;
- formal match phases, team selection, spawn selection, lives, score, respawn rules, victory, or
  restart; Milestone 07 replaces the sandbox reset policy with mode-owned lifecycle rules;
- the other three v1 weapons, weapon selection UI, weapon assets, build budgets, items, ultimates,
  or authored-file hot reload;
- ballistic motion, spread/pellets, melee, explosions, area damage, knockback, slow, shields,
  healing, status meters, terrain destruction, damage falloff, or critical hits;
- manual reload input, reload cancellation, ammo pickups, dry-fire audio, weapon switching, or
  per-shell reload;
- damageable objectives/deployables, hazards, destructible terrain, bots that aim/fire, or dummy AI;
- production art/audio, camera shake, accessibility settings, aim assist, controller vibration,
  full combat log, scoreboard, match telemetry export service, persistence, or analytics backend;
- a generalized effect/payload scripting framework, public content API, new package/crate, or
  speculative registry beyond the one concrete fighter and weapon consumer.

## Research questions and conclusions

### Exact versions, features, and application composition

- [x] Retain Rust 1.95, Bevy `=0.19.1`, Lightyear `=0.29.0`, and Avian 2D `=0.7.0` with the
  current features. Straight-projectile shape sweeps are already provided by Avian's enabled
  `parry-f32` spatial-query support; no dependency or Cargo feature is required.
- [x] Keep the single package and current client/server/network-test features. Add focused source
  modules and cohesive plugins only; do not create a content crate, combat crate, or DTO layer.
- [x] The dedicated server continues to install `GameplayPlugin`, `ProtocolPlugin`,
  `AvianNetworkPlugin`, `AuthoritativeMovementPlugin`, and `ServerNetworkPlugin`, adding an
  authoritative combat plugin. The client adds combat presentation to its existing input/network/
  presentation composition but never installs authoritative damage or reset systems.
- [x] Code-author the two definitions as validated resources in both roles. The server consumes
  numeric gameplay values. Clients resolve stable IDs only for debug labels and presentation.
  File-backed assets and hot reload wait for multiple definitions in Milestone 05.

### Authored definitions, selected build, and runtime state

- [x] Introduce narrow newtypes `FighterDefinitionId` and `WeaponDefinitionId`. IDs are serialized,
  stable, nonzero constants; missing/duplicate definitions fail plugin startup in tests rather than
  falling back to arbitrary values.
- [x] `FighterDefinitions` and `WeaponDefinitions` are immutable code-authored resources. A
  fighter carries a replicated-once `FighterDefinitionId` and `SelectedBuild` containing the pulse
  `WeaponDefinitionId`; these are selections, not mutable combat state.
- [x] Runtime state remains on the authoritative fighter entity: `CurrentHealth`, `TeamId`,
  `WeaponState`, and optional `Defeated`. The maximum health, body radius, movement speed, and
  weapon values remain definitions. `MovementTuning` is initialized from the one fighter definition
  during this milestone so movement and combat do not maintain conflicting authored values.
- [x] Use integer health, ammo, and tick deadlines. Positions/velocities remain finite `f32`
  because Avian uses them. Tick deadlines use saturating checked construction and wrap-safe
  comparison policy already bounded by practical process lifetime; identifier allocators reject
  exhaustion rather than wrapping.
- [x] Add `TeamId` now because direct-hit policy needs affiliation. Sandbox players alternate
  between `TeamId(0)` and `TeamId(1)` by server-assigned player ID and spawn on the matching arena
  side; update M03's grouped spawn-slot mapping to alternate side then row, with regression tests.
  `TeamId(u8::MAX)` is reserved for the neutral-hostile dummy and is never allied with any entity,
  including another neutral. Milestone 07 will own formal team allocation while reusing the
  component.

### Fire semantics and weapon economy

- [x] Primary fire remains one held bit in the existing native input. The server samples at most
  one authoritative action state per fighter per tick. There is no client-authored shot sequence,
  muzzle position, projectile, hit, damage, ammo, or cooldown value.
- [x] Holding fire attempts a shot every authoritative tick. A shot is accepted only when the
  fighter is active, its build/definitions resolve, ammo is positive, it is not reloading, and the
  current tick has reached `next_fire_tick`. An accepted shot consumes exactly one ammo, sets the
  next-fire deadline, allocates one `ShotId`, and spawns one projectile.
- [x] The server uses the last authoritative facing from Milestone 03. Fire does not require a new
  aim update in the same input packet. A neutral aim therefore shoots along preserved facing.
- [x] Reload is automatic and magazine-based. Consuming the final round starts reload immediately.
  An empty weapon that is not already reloading also starts reload on a fire attempt. When the
  deadline is reached, ammo refills to capacity before that tick's fire decision, so held fire may
  fire on the completion tick. Reload does not partially refill and cannot be cancelled in M04.
- [x] Cooldown and reload advance on server simulation ticks regardless of packet arrival. A
  repeated held input caused by Milestone 03's missing-input fallback may continue firing for at
  most its existing 12-tick freshness window; neutralization then stops new shots. Duplicate,
  stale, or reordered packets do not add simulation ticks or bypass weapon state.

### Projectile collision and Avian scheduling

- [x] Keep projectile motion in Brawler ECS rather than solver-driven rigid-body dynamics. Each
  projectile has canonical `Position`/`Rotation`, a server-only velocity, radius, remaining range/
  expiry tick, owner entity, stable source identity, and a circular `Collider` on the reserved
  projectile layer.
- [x] Each combat tick sweeps the projectile circle from its current position over
  `velocity * fixed_delta` with Avian `SpatialQuery::cast_shape_predicate`. The filter includes
  fighter, indestructible-terrain, and destructible-terrain memberships and excludes the projectile
  and owner entities. The predicate also skips defeated and allied fighters, so allies neither take
  damage nor consume a direct projectile.
- [x] Accept the nearest valid hit. A terrain hit creates an impact cue and consumes the projectile;
  a hostile fighter hit creates one pending damage record and consumes it. With no hit, advance the
  canonical position by the full step. Clamp the final step to remaining range so a projectile
  cannot hit beyond its authored range. Expiry without impact produces no gameplay hit.
- [x] Avian 0.7 refreshes its collider trees in the default `FixedPostUpdate` physics step. Fighter
  movement currently writes `Position` in `FixedUpdate`, so projectile sweeps must run after
  `PhysicsSystems::StepSimulation` to query current fighter/collider positions. Move the
  `SimulationTick` completed-step increment from `FixedUpdate` finalization to the final combat
  set in `FixedPostUpdate`; update Milestone 01/03 ordering tests and documentation in the same
  change. This is the only deliberate cross-milestone schedule refactor.
- [x] A narrow deferred-command flush between shot spawning and Avian preparation makes a newly
  accepted projectile visible to that tick's physics/query refresh and sweep. Do not globally
  chain unrelated systems or use `World::flush` outside the proven boundary.

### Damage, simultaneous outcomes, attribution, and reset

- [x] Collision emits internal Bevy 0.19 `Message` values (`ProjectileImpact` and
  `PendingDamage`); it does not mutate target health inline. Ordered consumers apply damage and
  produce `DamageApplied`, `FighterDefeated`, and presentation cues in the same fixed-post tick.
- [x] Collect all pending damage for the tick, sort by target stable ID, impact fraction, then
  `ShotId`, and apply sequentially. This removes ECS/query iteration order from attribution. Hits
  on different targets always resolve even if both fighters reach zero in the same tick, so mutual
  defeat is valid. After a target reaches zero, later same-tick records apply zero damage and do
  not earn another defeat.
- [x] Applied damage is `min(requested_damage, current_health)`. The first ordered record that
  changes health to zero owns the defeat credit. Damage/defeat IDs are reserved before mutating
  state, and cross-target outcomes are emitted in global event-ID order. Direct friendly fire and
  self-damage are disabled as fixed in M03; neutral is hostile to every team, including neutral.
- [x] Represent `DamageSource` as player weapon or environment. M04 only creates player-weapon
  damage; the environmental variant has no player credit and reserves an authored cause ID for
  later hazards/terrain. Do not invent environmental damage behavior now.
- [x] Defeat leaves the fighter entity and stable identity present, inserts replicated `Defeated {
  event_id, reset_at_tick }`, sets health to zero, makes the fighter non-targetable/non-colliding,
  and ignores its gameplay intent. Its existing projectiles continue to impact or expire and keep
  their original attribution, allowing explicit posthumous hits.
- [x] At the reset deadline, restore the fighter's original spawn position and definition facing,
  full health, full magazine, ready weapon state, and targetable collision layer, then remove
  `Defeated`. Reset is a sandbox lifecycle event, not a respawn, life, or score event.
- [x] On disconnect, despawn every projectile whose server-only owner is that fighter before the
  fixed combat loop can sweep, with a sweep-time guard for missing/disconnected owners. On defeat,
  projectiles persist. On reset, no prior cooldown/reload survives. No authoritative ongoing effect
  exists in M04. Client-only flashes/markers use bounded timers and are removed on expiry,
  replicated despawn, disconnect, or presentation reset.

### Replication, transient cues, and recovery

- [x] Replicate durable state through concrete components, not an aggregate snapshot:
  definition/build IDs once; `CurrentHealth`, `TeamId`, `WeaponState`, and `Defeated` as current
  state; projectile marker/source/definition once and pose with the existing pose-only
  interpolation rule.
- [x] Server projectiles use `Replicate::to_clients(All)` and
  `InterpolationTarget::to_clients(All)`. They are neither `ControlledBy` nor prediction targets.
  Adding `ControlledBy` would wrongly make them eligible native-input targets. Owner and remote
  clients therefore observe the same delayed authoritative projectile path in M04.
- [x] Register `CombatCue` server-to-client on its own ordered-reliable `CombatChannel`. The cue
  carries `CombatEventId`, completed simulation tick, stable source/target/weapon identities,
  cue kind, finite world position/normal, applied amount where relevant, and distance band. It
  never carries Bevy `Entity`.
- [x] Cues are presentation facts, not recovery state. A late/reconnecting client receives current
  fighters, health, weapon/defeat state, and active projectiles through replication but no historical
  cues. Clients deduplicate a bounded recent set of event IDs before spawning effects. Durable
  state remains correct even if a cue is delayed until after a projectile despawn or reset.
- [x] Use the same ordered cue stream for muzzle, impact, damage, defeat, and reset ordering.
  Under loss, head-of-line delay may postpone feedback but cannot change simulation. Record cue
  latency during impairment tests; prediction or an unreliable redundant cosmetic channel needs
  later evidence and is not added preemptively.
- [x] Bump the Netcode protocol ID and Brawler protocol version for new serialized messages and
  components. Preserve registry-fingerprint rejection, flush-before-rejection, reconnect, shutdown,
  and server feature-isolation behavior.

### Presentation and telemetry

- [x] Client presentation observes only replicated state and `CombatCue`. A local HUD shows health,
  ammo/capacity, `READY`, cooldown, `RELOADING`, or `DEFEATED`, and a concise controls line. Each
  fighter has a debug health bar; the fixed dummy is visually distinct.
- [x] A cue spawns bounded client-only presentation: muzzle flash, terrain/fighter impact flash,
  local-shooter hit marker, target sprite flash, defeat marker/overlay, and reset clear. Projectile
  visuals attach to replicated projectile entities and disappear with them. No presentation entity
  or timer is replicated.
- [x] Health and weapon HUD read replicated components so late join/reconnect is immediately
  correct. Cues enhance feedback but cannot be the only source for a current value.
- [x] `CombatTelemetry` is a server resource with totals for accepted shots, hostile fighter hits,
  applied damage, and defeats plus close/mid/long hit counts. Structured logs include event/tick,
  source, target, weapon, amount, and distance band for shot, hit, damage, defeat, and reset
  outcomes. Counters persist across sandbox resets; bounded diagnostic records are capped at 512,
  are inspectable in tests, and are logged on graceful shutdown; no file/database exporter is added.
- [x] Distance is straight projectile path traveled from muzzle to impact. Initial bands are close
  `< 250`, mid `250..600`, and long `>= 600` world units. Terrain impacts and expiry do not count as
  weapon hits; damage uses the actual non-overkill amount.

### Network impairment strategy

- [x] Use Lightyear's receive-side `LinkConditionerConfig` on both directions for statistical
  latency, jitter, and loss runs. The resolved 0.29 implementation uses unseeded runtime randomness,
  so retain profile/run identifiers and report repeated-run median/p95 rather than claiming bitwise
  determinism.
- [x] Keep exact deterministic regressions separate: the existing Crossbeam harness advances
  simulated time, direct test helpers inject duplicate/stale/reordered native input, and a small
  scripted input-stream fixture controls explicit hold/release/drop/duplicate cases where
  application outcomes must be exact. Do not modify the checked-in Lightyear source.
- [x] Exercise three symmetric profiles: local (`0 ms`, no jitter/loss), typical (`50 ms RTT`,
  `±10 ms`, no loss), and adverse (`100 ms RTT`, `±20 ms`, 2% independent loss). Use half the
  end-to-end latency/jitter configuration on each receive direction. Duplication is a targeted
  deterministic case because Lightyear's conditioner models delay/jitter/loss, not duplication.
- [x] The M04 impairment harness should also provide the missing M03 delayed/lost-input case and
  owner-prediction comparison data. Those results update M03 and backlog item `M03-PRED`; they do
  not make movement prediction part of M04 combat scope.

## Research log

| Date | Source | Finding | Decision impact |
|---|---|---|---|
| 2026-08-13 | `docs/{00-product-direction,02-fighter-model,03-weapons-and-abilities,05-gameplay-mvp,08-network-architecture}.md` and `docs/implementation/v1/{roadmap,milestone-01,milestone-02,milestone-03}.md` | M04 must prove one readable authoritative loop while preserving definition/build/runtime separation and all connection/movement contracts. M03 already fixes owner/allied/self-hit policy. | Limit scope to one pulse definition and sandbox lifecycle; reuse stable identity, native input, canonical pose, and collision matrix. |
| 2026-08-13 | Current `Cargo.toml`, `src/{gameplay,movement,protocol,server,client}.rs`, `tests/{network,performance}.rs`, `Justfile`, and workflow/scripts | One package and role features already isolate the headless server. Fighters are server-owned replicated entities; pose interpolation is intentionally position/rotation-only; the Crossbeam harness advances fixed time explicitly. | Add modules/plugins and protocol types without new dependencies or packages; extend the existing harness and feature gates. |
| 2026-08-13 | `references/lightyear/examples/README.md`, `simple_box`, `avian_2d`, and `fps` README/source/Cargo manifests | Server-spawned `Replicate` entities, interpolation targets, Avian pose replication, and projectile lifecycle are demonstrated. FPS prespawn/prediction and lag compensation solve a different, explicitly deferred problem. | Spawn projectiles only on the server and interpolate them for every client; do not copy prediction/rewind machinery. |
| 2026-08-13 | Lightyear book `src/SUMMARY.md`, `concepts/{bevy_integration/system_order,reliability/channels,advanced_replication/replication_logic,advanced_replication/avian}.md` | Receive/apply occurs in `PreUpdate`, send/replication assembly in `PostUpdate`; entity actions are ordered reliable while component updates are sequenced; custom channels select message guarantees. | Use replicated components for recoverable state and a separate ordered-reliable cue channel for transient presentation. |
| 2026-08-13 | Cargo-resolved Lightyear 0.29 sources: `lightyear_{messages,transport,replication,link,crossbeam}-0.29.0` | `MessageSender` buffers typed messages per channel; `Replicate` despawn follows entity action ordering; receive conditioning supports latency/jitter/loss but chooses unseeded runtime randomness; Crossbeam accepts caller-provided channels. | Keep statistical conditioner runs distinct from exact scripted duplicate/drop cases and retain stable event IDs. |
| 2026-08-13 | Cargo-resolved Avian 2D 0.7 `spatial_query/{system_param,query_filter,shape_caster}.rs`, `schedule/mod.rs`, and local Avian examples | `SpatialQuery::cast_shape_predicate` returns the nearest accepted shape hit with point/normal/distance. Avian's default physics and collider-tree refresh runs in `FixedPostUpdate`. | Sweep circles after `PhysicsSystems::StepSimulation`, use explicit masks/predicate exclusions, and finalize the Brawler tick afterward. |
| 2026-08-13 | Cargo-resolved Bevy 0.19.1 `bevy_ecs::message` source and [Bevy 0.19 release](https://bevy.org/news/bevy-0-19/) | Buffered cross-system communication uses `Message`, `MessageWriter`, and `MessageReader`; readers require explicit same-schedule ordering when latency matters. | Use internal ordered messages for the collision-to-damage pipeline and test the applied outcome, not only registration. |
| 2026-08-13 | [Avian 0.7 spatial-query documentation](https://docs.rs/avian2d/0.7.0/avian2d/spatial_query/struct.SpatialQuery.html) and [crate documentation](https://docs.rs/avian2d/0.7.0/avian2d/) | The released API explicitly supports on-demand shapecasts, predicate filtering, and server use with the current Bevy generation. | Exact released docs corroborate the locally resolved API; no custom collision solver or new physics dependency is needed. |
| 2026-08-13 | [Lightyear 0.29 crate source](https://docs.rs/crate/lightyear/0.29.0/source/) and [repository](https://github.com/cBournhonesque/lightyear) | Public web rustdoc/book indexing can lag the pinned crate, while the versioned crate source and checked-in snapshot expose the exact 0.29 composition. | Treat Cargo-resolved 0.29 source as the spelling authority and external links as provenance/current-primary cross-checks. |

## Technical specification

### Decisions

| Concern | Decision |
|---|---|
| Content source | Validated code-authored definition resources; stable replicated IDs |
| Fire input | Existing held `PRIMARY_FIRE` bit; server derives every accepted shot |
| Economy | Six-round magazine, fixed cooldown, automatic whole-magazine reload |
| Projectile | Server-spawned Brawler kinematic entity; Avian circle sweep; no prediction |
| Durable network truth | Concrete replicated ECS components and replicated projectile lifecycle |
| Transient feedback | Stable-ID `CombatCue` on a server-to-client ordered-reliable channel |
| Damage | Integer direct damage, friendly fire/self-hit off, deterministic batched ordering |
| Defeat | Fighter remains, becomes untargetable, projectiles persist, reset after fixed delay |
| Testing target | Current authoritative/interpolated path under exact and statistical impairment |
| Architecture | Focused modules/plugins in the existing package; no new dependency or abstraction layer |

### Provisional authored definitions

All durations are authored as integer simulation ticks at 60 Hz. These values exist to exercise
every M04 state transition and are playtest tuning, not balance commitments.

#### Fighter definition `fighter.standard` (`FighterDefinitionId(1)`)

| Field | Value | Rationale |
|---|---:|---|
| Maximum health | 100 | Four pulse hits defeat; easy HUD/debug arithmetic |
| Movement speed | 320 units/s | Preserve accepted M03 tuning |
| Body radius | 24 units | Preserve accepted M03 collider/tuning |
| Spawn facing | +X / 0 radians | Preserve M03 facing contract |
| Defeat reset delay | 90 ticks / 1.5 s | Readable feedback without a formal respawn loop |

#### Weapon definition `weapon.pulse_sidearm` (`WeaponDefinitionId(1)`)

| Field | Value | Rationale |
|---|---:|---|
| Direct damage | 25 | Four-hit defeat and two spare magazine rounds |
| Magazine capacity | 6 | Exercises ammo and reload without interrupting one defeat |
| Fire cooldown | 12 ticks / 0.20 s | Legible single-shot cadence while held |
| Reload duration | 60 ticks / 1.0 s | Clearly visible recovery window |
| Projectile speed | 900 units/s | Fast but still visually trackable in the greybox arena |
| Projectile radius | 6 units | Visible non-point sweep and stable contact |
| Maximum range | 900 units | Covers useful arena lanes without crossing the full width |
| Maximum lifetime | 60 ticks / 1.0 s | Exact range/speed backstop; range remains the primary clamp |
| Muzzle offset | 34 units | Starts outside the 24-unit owner body plus projectile radius |
| Direct collision policy | First hostile fighter or solid terrain | Pulse-sidearm identity |

Definition validation requires finite positive movement/projectile values, nonzero health/damage/
capacity/durations, a muzzle offset at least `body_radius + projectile_radius`, a representable
range step, unique IDs, and a selected weapon that exists. Invalid authored data is a startup/test
failure, not a network rejection path.

### ECS ownership and lifecycle

| Data/entity | Authoritative server | Client | Lifetime/recovery |
|---|---|---|---|
| `FighterDefinitions`, `WeaponDefinitions` | Numeric gameplay source | ID-to-debug/presentation lookup | App lifetime; identical validated constants |
| `FighterDefinitionId`, `SelectedBuild` | Set on accepted fighter/dummy | Replicated once | Fighter lifetime; recovered on join/reconnect |
| `TeamId`, `CurrentHealth`, `WeaponState`, `Defeated` | Sole mutable owner | Replicated observation only | Fighter lifetime; current value recovers |
| `Projectile` entity | Spawn/move/hit/despawn | Interpolated replicated copy | Shot until impact, range/lifetime, owner disconnect, or server cleanup |
| `ProjectileOwner(Entity)` | Server-local cleanup/query link | Absent | Projectile lifetime; never serialized |
| stable projectile source/shot IDs | Allocated and replicated | Read-only presentation | Projectile/cue lifetime; allocator rejects exhaustion |
| internal combat messages | Produced/consumed in fixed-post pipeline | Absent | Same tick; explicitly ordered readers |
| `CombatCue` | Derived from authoritative outcomes | Deduplicated presentation input | Reliable live delivery; no historical recovery |
| presentation effects/HUD | Absent | Client-local | Bounded timer or observed replicated lifecycle |
| `CombatTelemetry` | Mutable counters/logging | Absent | Server process; persists across sandbox resets |

The neutral dummy is a normal server-authoritative `Fighter` without `ControlledBy` or an input
buffer. It uses reserved `NetworkEntityId(0)`, `TeamId(u8::MAX)`, the standard definition/build,
full runtime state, static position, fighter collider, replication to all clients, and a
`TestDummy` marker. Player/network allocators continue at one, so no stable-ID collision exists.

### Runtime component shapes

Exact Rust layout may adapt to derive/API constraints, but the semantic fields are fixed:

```text
FighterDefinitionId(u16)
WeaponDefinitionId(u16)
SelectedBuild { primary_weapon: WeaponDefinitionId }
TeamId(u8)
CurrentHealth(u16)

WeaponState {
  ammo: u8
  phase: Ready | Cooldown { ready_at_tick } | Reloading { ready_at_tick }
}

Defeated {
  event_id: CombatEventId
  reset_at_tick: u64
}

ProjectileSource {
  shot_id: ShotId
  player_id: PlayerId
  owner_network_entity_id: NetworkEntityId
  team_id: TeamId
  weapon_definition_id: WeaponDefinitionId
}

ProjectileRuntime (server only) {
  owner_entity: Entity
  velocity: Vec2
  travelled: f32
  expires_at_tick: u64
}
```

`WeaponState` is one replicated component so ammo and phase/deadline cannot arrive as contradictory
independent updates. A replicated tick deadline is diagnostic/HUD state; the server clock remains
the authority.

### Network protocol

Register and fingerprint:

- components: both definition IDs, `SelectedBuild`, `TeamId`, `CurrentHealth`, `WeaponState`,
  `Defeated`, projectile marker/source, plus existing canonical pose;
- message: `CombatCue`, server-to-client only;
- channel: `CombatChannel`, ordered reliable in both role registries but direction-restricted to
  server-to-client;
- identifiers: `ShotId(u64)` and `CombatEventId(u64)` allocated by checked server resources;
- protocol compatibility: increment both explicit protocol constants when the wire types land.

`CombatCue` uses an enum payload with only the fields each cue needs:

```text
Muzzle { source, shot_id, position }
Impact { source, shot_id, target?, position, normal }
Damage { source, target, amount, health_after, distance_band }
Defeat { source?, target }
Reset { target, position }
```

Every variant also contains `event_id` and completed simulation tick. `source` and `target` are
stable player/network/definition identities, never process-local entities. Muzzle may use the same
shot identity but receives its own combat event ID so global ordering and client deduplication stay
uniform.

Durable component replication is sufficient for recovery:

- join while alive: current health, ammo/phase, build/team, and active projectiles appear;
- join while defeated: zero health and `Defeated` appear; the client does not need the original
  hit cue to render the state;
- reconnect: the existing policy creates a new player/fighter identity and full combat state;
- missed cue: state converges through component replication and future cues continue by event ID;
- projectile despawn: Lightyear's entity action removes every remote copy even if an unreliable
  pose update was lost.

### Fixed-tick schedule and ordering contract

```text
PreUpdate
  Lightyear receive/validation/apply
  -> native input buffer and replicated state/cue receive

FixedPreUpdate
  client writes FighterInput for current tick
  server applies native ActionState

FixedUpdate
  GameplaySet::Lifecycle
    reset due defeated fighters -> restore state/pose/collision -> remove Defeated
  apply_deferred (reset archetype visibility boundary)
  GameplaySet::Input
    select fresh-or-neutral input -> authoritative facing
  GameplaySet::Simulation
    authoritative fighter move-and-slide
  GameplaySet::Fire
    complete due reloads -> validate held fire -> consume ammo -> spawn projectile/cues
  apply_deferred (projectile spawn visibility boundary)

FixedPostUpdate
  Avian PhysicsSystems::Prepare -> StepSimulation
    refresh current collider tree from moved/reset fighters and new projectiles
  CombatSet::ProjectileSweep (after PhysicsSystems::StepSimulation)
    sweep -> move or emit impact/pending damage -> request projectile despawn
  CombatSet::Damage
    sort pending damage -> apply health -> emit damage/defeat
  CombatSet::Lifecycle
    mark defeated/non-targetable -> owner/disconnect projectile cleanup -> expiry
  CombatSet::TelemetryAndCues
    update counters -> buffer stable CombatCue messages
  CombatSet::Finalize
    validate combat state/pose -> increment completed SimulationTick once

Update/PostUpdate (client)
  network interpolation -> consume/deduplicate cues -> update HUD/effects
  replication send occurs before render-only pose writeback/transform propagation

Last
  existing Disconnect/Stop bridges and telemetry summary on graceful shutdown
```

The two `apply_deferred` points are narrow, tested lifecycle boundaries: reset removal must be
visible before input/movement, and new projectiles must be visible before Avian preparation.
Configure ordered sets rather than chaining individual systems. Systems within collision or
presentation may run in parallel only when their accesses and outcome ordering are independent.
Tests must assert that newly spawned projectiles can hit in their first tick, movement precedes the
current collision-tree query, damage precedes defeat, cues reflect applied values, and the tick
increments exactly once after every combat outcome.

### Projectile sweep and damage algorithm

For each active projectile in stable `ShotId` order:

1. Compute `remaining = min(speed * dt, max_range - travelled)` and expire if nonpositive or the
   lifetime deadline has passed.
2. Build a circular cast with current canonical position/rotation, direction from finite normalized
   velocity, max distance `remaining`, and an explicit filter mask.
3. Predicate-exclude self, owner, defeated fighters, and allied fighters. Accept solid terrain and
   active hostile fighter colliders. Unknown collider categories are rejected and diagnosed rather
   than treated as damageable.
4. If no hit, advance by `remaining` and add to traveled distance.
5. If hit, place the authoritative impact at the cast contact/travel distance, emit one impact,
   optionally emit one pending direct-damage record, and despawn the projectile after consumers
   have copied its stable source data.
6. Sort all pending damage and apply it with the deterministic policy above.

The cast prevents tunneling even when a projectile crosses a complete fighter or thin wall within
one tick. Starting penetration is a definition/spawn invariant; tests cover muzzle clearance and
explicitly diagnose any distance-zero hit. No bounce, pierce, multi-target, or area query exists.

### Reset and cleanup matrix

| Trigger | Fighter | Weapon | Owned projectiles | Presentation |
|---|---|---|---|---|
| Defeat | Keep identity; health 0; untargetable; no input | Frozen until reset | Persist to impact/expiry | Defeat marker and cue |
| Sandbox reset | Restore spawn/facing/health/collider | Full ammo; ready | Existing posthumous projectiles still persist | Clear defeat; reset flash |
| Owner disconnect | Existing session cleanup despawns fighter | Removed with fighter | Despawn immediately | Replicated removal clears visuals/HUD target |
| Projectile impact/range/lifetime | Unchanged unless damaged | Unchanged | Despawn that projectile | Impact cue and bounded flash |
| Server stop | Existing graceful lifecycle | Removed with world | Removed with world | Client disconnect cleanup |
| Late join/reconnect | Current/new identity per M02 policy | Current/full state as applicable | Current active set only | Rebuild from state; no old cues |

### Telemetry contract

The server exposes counters in `CombatTelemetry` and one structured log per accepted authoritative
outcome. Required fields:

| Record | Fields |
|---|---|
| Shot | tick, shot ID, source player/fighter, weapon, muzzle position, ammo after |
| Hit | tick, event ID, shot ID, source, target, weapon, impact position, traveled distance/band |
| Damage | tick, event ID, source, target, requested, applied, health after |
| Defeat | tick, event ID, credited source or environment, target |
| Reset | tick, event ID, target, reset position |
| Summary | shots, hostile hits, hit rate, applied damage, defeats, close/mid/long hits |

Do not count rejected fire attempts, friendly pass-through, terrain impacts, or expiry as shots
missed individually; `shots - hostile_hits` supplies the initial miss count. A later weapon with
multiple pellets will require a clarified attack-versus-projectile metric in Milestone 05.

## Preparation evidence

Research preparation on 2026-08-13 established:

- the locked Cargo metadata still resolves one package with independent client, server, and
  network-test configurations;
- no new dependency or feature is needed for M04;
- exact Bevy 0.19.1, Lightyear 0.29.0, and Avian 0.7.0 APIs were checked in Cargo-resolved source;
- `cargo fmt --all -- --check` and `git diff --check` pass for this documentation change;
- the locked `network-test` integration command passes the pre-implementation baseline's 28
  authority/lifecycle/movement cases;
- `./scripts/check-server-features.sh` passes and confirms that the server graph excludes client
  presentation capabilities;
- the existing M03 implementation record separately reports green isolated-role Clippy/tests,
  real UDP/process, and performance evidence.

The implementation start must freshly run the complete locked M03 baseline. This research record
does not claim the still-open M03 impairment, windowed, controller, or user-playtest evidence.

## Implementation and verification evidence

Implementation completed the authoritative combat core on 2026-08-14. The server owns fire
validation, projectile spawn/sweep, damage, defeat/reset, disconnect cleanup, telemetry, and combat
cues; the client owns only replicated-state presentation, HUD, health bars, projectile visuals, and
bounded effects. The reserved dummy and the Crossbeam harness provide a deterministic end-to-end
authority path without weakening the existing M03 movement contracts.

Green automated evidence recorded after implementation:

- `cargo fmt --all -- --check`;
- client isolated unit tests: 50 passed, including native gamepad stick/trigger/Start mapping,
  end-to-end controller sampling into the native fighter action buffer, replicated reload/defeat
  HUD text, and bounded combat-effect expiry;
- server isolated unit tests: 42 passed, including definition-driven runtime initialization, accumulated
  range/clamping, impact-fraction damage ordering, event-exhaustion atomicity, bounded diagnostics,
  and neutral targeting;
- `cargo test --locked --no-default-features --features network-test --test network -- --test-threads=1`:
  34 passed, including fixed-schedule reload/fire, same-tick first-tick projectile hit/damage,
  closest-hit and thin-cover sweeps, near-impact disconnect cleanup, and fabricated-owner rejection;
- `cargo test --locked --no-default-features --features network-test --test performance -- --nocapture`:
  2 passed; the latest sustained near-collider 100-fighter/200-projectile scene measured p95
  `2.800125ms` and the 100-fighter baseline measured p95 `822.791µs` on aarch64 macOS, below the
  16.67ms fixed-tick
  budget;
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`;
- `cargo clippy --locked --no-default-features --features server --all-targets -- -D warnings`;
- `git diff --check` and `bash -n scripts/network.sh scripts/network-combat-profiles.sh`;
- `./scripts/check-server-features.sh`;
- `BRAWLER_NETWORK_HEADLESS=1 BRAWLER_NETWORK_TIMEOUT_SECONDS=30 ./scripts/network.sh` — two
  headless movement/aim clients passed the existing server-side readiness assertion;
- `BRAWLER_NETWORK_HEADLESS=1 BRAWLER_NETWORK_ASSERT_COMBAT=1 ./scripts/network.sh` — supervised
  loopback fire/aim automation passed on run `final-local-combat-3` with
  `accepted_shots=12`, `hostile_hits=4`, `applied_damage=100`, one defeat, server reset, and
  both client ammo/health/reset observations;
- Review correction verification: `ProjectileRuntime::travelled` now accumulates every no-hit and
  final clamped sweep step; impact distance bands use accumulated travel, and equal-tick damage is
  sorted by target stable ID, current-sweep impact fraction, and `ShotId`. The server now uses the
  explicit Bevy message chain `ProjectileImpact → PendingDamage → DamageApplied/FighterDefeated`.
  The authoritative tick is replicated with fighter state for reconnect-safe HUD deadlines, and
  fighter health/ammo initialization reads the selected definitions rather than duplicate literals.
- Review follow-up verification: process combat readiness now requires both clients to serialize the
  complete ordered `CombatCue` payload stream and match the server's bounded accepted-shot
  telemetry, even when no report file is requested. The deterministic harness captures `CombatCue`
  messages directly and compares every payload field, including target, weapon, damage, health, and
  reset position. A targeted Lightyear receive-packet test duplicates and reverses a cue-producing
  batch, then proves both clients converge to the authoritative stream exactly once. Fire-to-cue
  timestamps are bounded and enabled only for the impairment evidence harness; client and server
  validate authored catalogs at startup; the authored dummy facing is reused for its runtime
  transform/collision setup; `Impact` carries `weapon_definition_id`; and the network protocol ID
  plus application version were bumped for that wire change.
- `BRAWLER_NETWORK_HEADLESS=1 BRAWLER_NETWORK_ASSERT_COMBAT=1 BRAWLER_NETWORK_RUN_ID=no-report-cue-gate ./scripts/network.sh`
  — passed with no `BRAWLER_NETWORK_COMBAT_REPORT_FILE`, proving the mandatory cue-stream gate is
  not conditional on report generation.
- Current-tree retention follow-up: `BRAWLER_NETWORK_HEADLESS=1 BRAWLER_NETWORK_ASSERT_COMBAT=1
  BRAWLER_NETWORK_RUN_ID=final-review-followup BRAWLER_NETWORK_ADDR=127.0.0.1:6233
  ./scripts/network.sh` passed without a report file; one-run local/typical/adverse profile
  checks also passed with run IDs `local-1-1786692150`, `typical-1-1786692159`, and
  `adverse-1-1786692167`.
- Latest review follow-up: disconnect cleanup now runs after transport receive and before the
  fixed combat loop, while projectile sweep rejects missing/disconnected owners as a second
  authority boundary. The sustained performance fixture uses authored fighter/runtime state,
  stable owner entities, nearby fighter/cover sweep candidates, and asserts all 200 projectiles
  remain active before every sample. Deterministic network coverage now exercises deferred
  authoritative fire, same-tick first-tick collision and damage, reload-completion fire,
  closest-target selection, thin-cover stopping, exact reset timing from `SimulationTick` records,
  near-impact disconnect cleanup, and direct orphan-projectile rejection. Damage/defeat IDs are
  reserved atomically before state mutation, outcomes are emitted by global event ID, neutral
  targeting is explicit, and diagnostic records/logs are bounded and structured.
- `BRAWLER_NETWORK_PROFILE_RUNS=3 BRAWLER_NETWORK_PROFILE_BASE_PORT=6000 ./scripts/network-combat-profiles.sh`
  — all 9 current-code runs passed. The report now includes matched server fire and client muzzle
  cue epoch timestamps, exact cue-set convergence, and the following readiness/convergence and
  fire-to-cue medians/p95 (milliseconds for process readiness, microseconds for cue latency):

  | Profile | Runs | Server median/p95 ms | Client 1 median/p95 ms | Client 2 median/p95 ms | Fire→cue client 1 median/p95 µs | Fire→cue client 2 median/p95 µs |
  |---|---:|---:|---:|---:|---:|
  | local | 3 | 5442 / 5644 | 3344 / 3344 | 3343 / 3343 | 18344 / 19515 | 16377 / 18331 |
  | typical | 3 | 5646 / 5753 | 3580 / 3580 | 3598 / 3598 | 52210 / 53256 | 56318 / 57497 |
  | adverse | 3 | 5917 / 5995 | 3816 / 3816 | 3809 / 3809 | 74224 / 81382 | 76371 / 79901 |

  Run IDs: `local-1-1786674485`, `local-2-1786674493`, `local-3-1786674502`,
  `typical-1-1786674510`, `typical-2-1786674518`, `typical-3-1786674527`,
  `adverse-1-1786674535`, `adverse-2-1786674543`, `adverse-3-1786674552`.

  The profiles use Lightyear receive conditioners on both process links; `adverse` adds fixed
  2% loss. Fire-to-cue values use same-host epoch clocks only for evidence and are statistical
  process measurements, not deterministic latency claims.

Windowed keyboard/mouse smoke evidence from two packaged clients on 2026-08-14:

- both clients rendered the connected baseline with local/remote health bars, full health, full
  pulse ammo, and the control/status HUD;
- Escape paused and resumed the simulation with the visible pause overlay, and the compact-window
  check found and fixed overlapping bottom HUD text by constraining and right-aligning the status
  block;
- the live session displayed the pulse projectile and combat feedback while the supervised server
  log recorded repeated authoritative shots, hostile hits, defeats, and subsequent reset cycles;
- the reproducible `--combat-demo` windowed mode drove the same native `FighterInput` path toward
  the dummy; one supervised run recorded 258 accepted shots, 140 hostile hits, 35 defeats, and
  35 sandbox reset cycles while the window showed active projectiles, depleted/reloading ammo,
  health bars, and impact/defeat/reset feedback;
- the native controller mapping is unit-covered for left-stick movement, right-stick aim, right
  trigger fire, and Start pause; no physical controller was available to exercise, and no
  high-refresh display comparison was available in this environment; the client status remained
  `keyboard/mouse | gameplay`, and macOS display inspection exposed only the built-in 2880x1864
  Retina display without an alternate refresh mode.
- a fresh current-tree two-window pass showed client 1 at `Health 100`, `Pulse 2/6`, and
  `COOLDOWN` with active projectile/effect sprites and health bars; client 2 showed full `Pulse
  6/6 READY`; Escape displayed the centered pause overlay and changed the status line to
  `keyboard/mouse | paused` before resume.
- A continuation audit on 2026-08-14 reran the current tree's isolated client/server tests,
  both role-specific Clippy lanes, the complete network-test suite, performance tests, and the
  server-feature isolation check; all passed (50 client tests, 42 server tests, 65 network-test
  unit tests, 34 network tests, and 2 performance tests). A fresh supervised UDP combat run
  (`continuation-local-combat`) also passed the server/client readiness and combat assertions.
- The new `just network-combat` launcher profile was exercised from the current tree; its
  supervised command line started client 1 with `--combat-demo` and client 2 without it, then
  terminated cleanly under the normal Ctrl-C cleanup path. This makes the single-shooter visual
  scenario reproducible without accidentally creating two simultaneous projectile streams.
- The launcher now supervises the built server/client binaries directly and keeps background
  children from consuming terminal Ctrl-C. A direct-script run and a `just network-combat-60` run
  both confirmed that Ctrl-C removes the launcher, both clients, the server, and releases UDP port
  5000 before returning.
- `just network-controller` launched client 1 with `--controller-demo`; the normal sampler reported
  `active_device=Gamepad`, the native input trace reported `gameplay_buttons=1`, the server accepted
  shots and defeated/reset the dummy, and the window HUD displayed `Input: gamepad | gameplay` with
  active combat, health bars, visible cover, and perimeter walls. This is a synthetic-controller
  end-to-end smoke, not physical-controller evidence.
- A fresh desktop capture from that profile showed the bright perimeter markers, central cover
  edge, distinct fighter colors, active cyan projectiles, health bars, and the local `Player 2`
  HUD with full ready ammo. This is keyboard/mouse/demo presentation evidence only; it does not
  close the physical-controller or high-refresh gate.
- The render-profile harness is now reproducible through `just network-combat-30`,
  `just network-combat-60`, and `just network-combat-high`. The profiles change only the window
  update/present path; the authoritative 60 Hz fixed simulation and network/input contracts remain
  unchanged. Unit coverage verifies the 30 Hz, 60 Hz, and high-refresh configuration selection.
- Fresh desktop captures were taken from each of those three profiles. All three showed connected
  HUD state, visible perimeter/cover geometry, distinct fighters, health bars, and the active
  combat presentation without stale effects. This closes the keyboard/mouse visual portion of the
  30/60/high-refresh check; it does not substitute for physical-controller input evidence or prove
  an unavailable monitor refresh mode.
- The follow-up report that the walls were still perfectly invisible reproduced the remaining edge
  case: the collision bodies are outside the playable rectangle and the original 10-unit marker
  could fall outside a camera following a boundary player. The client now renders a 24-unit dark
  in-bounds wall body with a 6-unit bright inner edge, both derived from shared arena bounds; cover
  bodies render above the arena marker layer. A fresh `just network-combat-60` capture showed the
  right and bottom walls and the full central cover clearly, including at the compact window size.
  `perimeter_visual_shapes_are_in_bounds_and_follow_collision_faces` locks this relationship down.
- Post-fix captures from `just network-combat-30` and `just network-combat-high` repeated the same
  check. Both showed the in-bounds wall edge, complete central cover, connected HUD, health bars,
  and the windowed combat presentation. The selected render paths are verified; an actual alternate
  physical monitor refresh remains unavailable here.

Still open and intentionally not claimed as complete: the full M03 owner-prediction comparison,
physical-controller input verification, actual alternate-monitor refresh verification, and explicit
final user acceptance of the windowed presentation. The combat conditioner profiles, exact
duplicate/reorder fixture, delayed/lost-input update, and supervised real-UDP assertions are
recorded above.

## Trackable implementation plan

### Prerequisite and schedule composition

- [x] Re-run the locked M03 format, client/server Clippy/tests, network, performance, UDP/process,
  and server-feature baseline; record any unrelated open interactive item rather than absorbing it.
- [x] Add `GameplaySet::Lifecycle` and `GameplaySet::Fire`, add fixed-post combat sets, move
  `SimulationTick` completion after combat, and update every affected M01/M03 schedule/tick test and
  document.
- [x] Add focused combat modules/plugins to the existing package and role composition without new
  dependencies, feature leakage, or authoritative systems in the client.

### Definitions and fighter runtime

- [x] Implement stable definition IDs, validated code-authored fighter/weapon catalogs, selected
  build, exact provisional values, and definition validation tests.
- [x] Initialize movement tuning from the fighter definition and prove M03 speed/radius/facing do
  not change.
- [x] Extend accepted fighters with team, health, weapon state, and build/definition components;
  alternate sandbox spawn sides/teams, and spawn the stable neutral-hostile dummy without input
  ownership.
- [x] Implement checked shot/combat-event allocators with exhaustion tests and reserved dummy ID
  invariants.

### Fire, projectile, damage, and lifecycle

- [x] Implement held-fire validation, cadence, ammo consumption, automatic reload completion,
  muzzle derivation, and one authoritative projectile spawn per accepted shot.
- [x] Implement projectile runtime/replication components, explicit collision layers, current-tree
  circle sweep, ally/owner/defeated filtering, accumulated range/lifetime, impact, and cleanup.
- [x] Implement internal message pipeline, stable sorting, applied-damage clamping, mutual defeat,
  attribution, untargetable defeated state, posthumous projectiles, and environmental source shape.
- [x] Implement fixed-delay sandbox reset and owner-disconnect projectile cleanup without changing
  M02 session/reconnect semantics.

### Protocol and client presentation

- [x] Register/fingerprint all components, `CombatCue`, and `CombatChannel`; bump both protocol
  compatibility constants and retain controlled mismatch rejection.
- [x] Configure all-client projectile interpolation without `ControlledBy`/prediction and prove
  durable state plus active projectile recovery on late join.
- [x] Implement projectile/dummy visuals, health bars, local health/ammo/phase HUD, cue deduplication,
  muzzle/impact/hit/defeat/reset feedback, and bounded presentation cleanup.
- [x] Add a reproducible synthetic-controller window smoke through the normal gamepad sampler and
  native input buffer; keep physical-controller validation explicit rather than treating the demo as
  hardware evidence.

### Telemetry, impairment, and workflow

- [x] Implement server counters, distance bands, structured records, graceful summary, and focused
  counter/log-field tests.
- [x] Add symmetric Lightyear conditioner profiles and exact scripted hold/drop/duplicate/reorder
  fixture; record run IDs, matched fire-to-cue timestamps, and median/p95 ordered cue/state
  convergence results.
- [x] Use the harness to update M03 delayed/lost-input and `M03-PRED` evidence separately; the
  input-loss result and authoritative owner baseline are recorded, while adopting prediction
  remains an explicit M03 backlog decision.
- [x] Extend real UDP/process automation to fire known scripts and require server-written combat
  readiness only after shot, hit, damage, defeat, reset, accepted-shot count, and complete ordered
  two-client cue-stream convergence assertions.
- [x] Update README/Justfile/CI commands without weakening existing lanes.
- [x] Complete the user handoff after the interactive smoke scenario and record requested
  observations; the documented scenario, controls, known limitations, and current keyboard/mouse
  evidence are recorded below, while physical-controller observations remain explicitly pending.

## Test plan

### Pure and small-App tests

- [x] Definition validation accepts the exact catalogs and rejects duplicate/missing IDs, nonfinite
  values, zero/invalid bounds, unsafe muzzle clearance, and unresolved selected builds.
- [x] Cooldown boundary, last-round reload start, reload completion/refill/fire-on-completion,
  held/neutral input, defeated input suppression, and reset-to-ready state use explicit ticks.
- [x] Known muzzle/facing produces exact finite position/velocity; shot and event allocators are
  monotonic and reject exhaustion.
- [x] Circle sweeps hit thin terrain and fighters at maximum speed without tunneling, stop at the
  closest valid target, pass through owner/allies/defeated fighters, clamp range, and expire once.
- [x] Damage clamps overkill, preserves health invariants, orders equal-tick hits by the specified
  keys, credits one defeat, allows mutual defeat, and retains posthumous attribution.
- [x] Reset restores exact spawn/facing/health/collision/weapon state after 90 ticks and not at 89;
  the authoritative reset record is stamped with the `SimulationTick` deadline, and repeated
  defeat/reset leaves no stale marker, cooldown, or presentation state.
- [x] `SimulationTick` advances once and only after movement, Avian refresh, projectile collision,
  damage, defeat, cues/telemetry, and validation; a first-tick projectile is visible across the
  deferred boundary and can hit/damage a target in that same tick.
- [x] Protocol registration/fingerprint contains every new type/channel in both roles; round trips
  preserve stable IDs and reject mismatched registration through the existing non-panicking path.
- [x] Telemetry counts accepted shots/hostile hits/applied damage/defeats and exact distance bands;
  terrain/friendly/expired projectiles do not inflate hit counts.

### Deterministic separate-App network tests

- [x] One client holds fire; only the server spawns projectiles/changes ammo and health, and both
  clients observe the same stable shot, impact, health, cue, and projectile-despawn outcome.
- [x] A malicious client cannot spawn/replicate a projectile, target another fighter with input,
  change health/ammo/weapon state, report a hit, accelerate cadence, or bypass reload/defeat.
- [x] Duplicate/redundant/stale/reordered fire-bearing inputs create no extra shot beyond the
  per-tick cadence state; missing input follows repeat-through-12 then neutral exactly.
- [x] Two reciprocal lethal hits in one tick defeat both fighters; all clients agree on event IDs,
  attribution, health, and defeat state.
- [x] Repeated defeat/reset cycles converge and leave no stale projectiles, cues, collision state,
  ammo, ownership, or session data.
- [x] Defeat preserves in-flight projectiles; disconnect removes them; reconnect creates one fresh
  fighter/build/weapon identity under the existing M02 rule.
- [x] A late join during live projectile, reload, and defeat receives current durable state and
  active projectiles without requiring historical cues.
- [x] Targeted drop/hold/duplicate/release cases eventually deliver ordered cues exactly once at
  presentation, converge durable state, and never apply authoritative damage twice; the
  duplicate/reordered case now impairs received Lightyear packets after a cue-producing shot and
  compares the complete cue payload stream.
- [x] Every existing M02/M03 rejection, timeout, roster, movement, collision, interpolation,
  reconnect, shutdown, and arena-stability assertion retains its meaning.

### Statistical, UDP/process, performance, and visual verification

- [x] Run local, typical, and adverse profiles in both directions for enough repetitions to report
  median/p95 fire-to-cue latency and state/cue convergence; record run IDs and failures.
- [x] Under each profile, the authoritative shot count respects cadence/ammo, no hit/damage/event
  duplicates, both clients converge after a bounded drain, and defeat/reset remains repeatable.
- [x] Execute and record the separate M03 delayed/lost neutralization result and authoritative-owner
  baseline; update M03/backlog while leaving the unimplemented prediction comparison explicitly
  deferred.
- [x] Real loopback UDP proves held input, authoritative projectile, hit, health, defeat, reset,
  reliable cue, and current-state recovery rather than only connection/movement.
- [x] Supervised one-server/two-client automation exits success only after a server readiness marker
  proves the known combat script and both client observations; child errors/timeouts propagate.
- [x] Locked format, isolated client/server Clippy/tests/builds, network test, UDP/process,
  server-feature check, and prior performance lane pass.
- [x] Measure at least 100 fighters plus 200 simultaneous active straight projectiles in a headless
  near-collider worst-case sweep scene with nearby fighters/cover candidates; p95 authoritative
  fixed step stays below 16.67 ms on the recorded machine.
- [x] Provide a reproducible windowed render-profile harness for 30 Hz, 60 Hz, and high-refresh/
  no-vsync paths without changing the 60 Hz authoritative simulation; configuration tests and
  bounded startup/cleanup smoke runs pass.
- [x] Windowed keyboard/mouse and selected render-profile verification confirms primary fire,
  muzzle/projectile/impact readability, health/ammo/reload HUD, hit marker, defeat/reset feedback,
  and no obvious wall tunneling or stale effects at the 30 Hz, 60 Hz, and high-refresh paths.
- [x] The user explicitly approved completion with physical-controller and alternate high-refresh
  verification deferred; synthetic controller/render-profile evidence is recorded without being
  presented as hardware proof.

### Evidence rules

- Fire authority evidence must enter through Lightyear's native input buffer; directly mutating
  `WeaponState`, health, or projectile entities is only valid for narrowly named unit tests.
- A hit test must exercise the shape sweep and authoritative damage consumer; overlapping/spawning
  directly on a target does not prove no tunneling.
- Network convergence compares stable IDs and semantic state, never local Bevy entity identity.
- Reliable cue receipt does not substitute for durable late-join/reconnect state, and replicated
  health does not substitute for visible hit/defeat feedback.
- Time-dependent tests advance Bevy/Lightyear fixed time and schedules. Wall-clock waits are limited
  to bounded real-process supervision.
- Statistical conditioner runs report repetitions and median/p95; unseeded results are not called
  deterministic. Exact duplication/reordering assertions use the scripted fixture.
- Visual/controller checks complement automated authority/lifecycle tests and cannot replace them.

## Visual and user smoke-test plan

The implementation handoff will retain one documented command for a dedicated server and two
distinguishable clients. The requested scenario:

1. Connect both clients; confirm each sees the same neutral dummy, health bars, and full local ammo.
2. Fire at terrain and confirm a projectile/impact without a hit marker or damage.
3. Fire through an ally/owner path where possible, then at the opposing player/dummy; confirm only
   hostile hits consume the projectile and damage.
4. Hold fire through cooldown, empty magazine, automatic reload, and resumed fire; compare HUD
   timing on controller RT and mouse left button.
5. Defeat the dummy and another player in four hits; identify hit confirmation, target flash,
   health loss, defeat attribution/feedback, 1.5-second reset, and restored ammo/health.
6. Trade lethal shots and look for mutual defeat; verify in-flight shots may still hit after their
   owner is defeated.
7. Disconnect a client with a projectile active, reconnect, and confirm no orphan projectile or
   stale HUD/effect while the new fighter identity starts full.
8. Repeat under the selected adverse profile and report fire responsiveness, projectile motion,
   cue delay, duplicate/missing feedback, wall tunneling, and state convergence.

Known limitations must state: projectiles are not owner-predicted; no lag compensation exists;
automatic reload has no manual/cancel input; the dummy does not act; teams/reset are provisional
sandbox rules; only direct damage exists; no score/match/respawn loop, audio polish, other weapon,
or production content authoring is present.

## Feedback review and closeout

Specification accepted; automated verification and feedback triage are complete. The user approved
Milestone 04 completion with physical-controller and alternate-refresh observations explicitly
deferred. Numeric tuning may change after playtest while authority, lifecycle, recovery, and
schedule invariants remain fixed.

The latest automated pass adds deterministic coverage for the replicated reload/defeat HUD text and
the bounded lifetime of client-only combat effects. The selected 30 Hz, 60 Hz, and high-refresh
render paths are also smoke-tested; physical-controller input and an actual alternate monitor
refresh remain awaiting evidence.

The 2026-08-14 two-window playtest exposed a presentation ambiguity rather than a world-coordinate
inversion. Both windows had been started with `--combat-demo`, so the authoritative server correctly
accepted shots from player 1 toward the dummy (+X) and player 2 toward the dummy (-X) at the same
time. The client previously rendered both player bodies with nearly identical hues and every
projectile as the same yellow square, which made source ownership and direction difficult to read.
Implemented now: stable distinct player/projectile source colors, elongated projectile sprites, and
replicated projectile rotation copied into the render transform. The single-shooter smoke command
and the intentional two-shooter behavior are now explicit in the root README. This feedback is
implemented now; controller responsiveness and high-refresh evidence remain awaiting hardware.

A final-tree sequential smoke then launched client 1 with `--combat-demo` and client 2 without it:
the HUD identified them as Player 1 and Player 2, Player 1's cyan projectiles traveled from the
left-side blue fighter toward the red dummy, and Player 2's orange fighter remained idle. This
confirmed the source-aware presentation and the documented single-shooter setup.

The follow-up playtest confirmed that the greybox collision geometry itself was invisible in the
window. Implemented now: perimeter and cover geometry are shared through `GreyboxArenaDefinition`
shape helpers, the client draws an in-bounds wall body plus bright inner edge inside the playable
bounds, and each cover has a high-contrast body and edge strip above the arena marker layer. The
client presentation test verifies that every server blocker has corresponding visible geometry;
the final windowed smoke showed these markers and the complete cover bodies.

The reported combat-direction, projectile-source, and invisible-wall feedback is therefore
triaged as implemented now. `just network-combat` now provides the reproducible two-window
single-shooter scenario used for this check. The remaining controller and high-refresh items are
awaiting hardware; they are not silently treated as passed by the automated input tests or the
scripted windowed demo.

The subsequent implementation reviews found nine combat-evidence/protocol issues. All are
implemented now: cue verification is mandatory without a report file; server-accepted shots and
the complete ordered Muzzle → Impact → Damage → Defeat → Reset payload stream are compared in both
the process smoke and deterministic harness; targeted duplicate/reordered packet impairment is
covered; client and server validate both authored catalogs at startup; evidence timestamp/stream
buffers share a bounded retention contract; the dummy uses `spawn_facing`; and Impact cues carry
weapon identity under the bumped protocol. The milestone is complete; physical-controller and
alternate-refresh observations are deferred by explicit user approval and are not claimed as
automated evidence.

The latest review found seven authority, evidence, and lifecycle gaps. All are implemented now:
disconnect cleanup is ordered before fixed combat and reinforced inside projectile sweep; the
200-projectile gate measures a sustained scene with valid owners; fixed-schedule tests cover
deferred fire, reload completion, first-tick collision, thin cover, closest-hit selection, exact
reset timing, and fabricated projectile rejection; damage/defeat ID allocation is atomic; outcome
emission is globally event-ordered; telemetry history is bounded and hit/damage/defeat/reset
outcomes emit structured logs; and neutral entities are hostile to every team. These corrections
are backed by the current green role, network, and performance lanes. The milestone remains in
`Complete` after explicit user approval; hardware-only controller and alternate-refresh evidence
remains a documented deferred observation rather than an unverified claim.

The next review tightened the remaining evidence claims. The 200-projectile fixture now keeps
paths inside the authored arena and sweeps through nearby fighter/cover candidates while asserting
all projectiles remain active. A dedicated network test places a target inside the newly spawned
projectile's first sweep and requires Shot, Impact, and Damage records to share one
`SimulationTick`. Reload firing and reset restoration likewise compare recorded event ticks
directly with authoritative deadlines; reset verification covers spawn position/facing, health,
weapon readiness, collision layers, and removal of `Defeated`. The dummy spawn was moved clear of
the lower cover so physics cannot invalidate the exact reset pose.

## Learn from errors

The implementation review recorded these reusable lessons before playtest closeout:

- Combat systems must run after authoritative movement and Avian refresh, then emit cues and
  telemetry before `SimulationTick` advances; otherwise a same-tick projectile can observe stale
  transforms or cross a deferred-command boundary without a visible first-tick state.
- Durable replicated state and ordered combat cues have different recovery contracts. Late join and
  reconnect assertions must inspect health, weapon phase, defeat, and active projectile state even
  when no historical cue is replayed.
- Stable player/shot/event IDs must be carried through damage and posthumous projectile outcomes;
  local Bevy entity identity is not valid attribution. Disconnect cleanup also needs to remove
  projectiles whose owner entity has already disappeared, not only owners still carrying a session
  marker; cleanup must be ordered before fixed combat and duplicated by a sweep-time authority
  guard.
- Reserve every event ID needed for a lethal outcome before mutating health or hit counters, and
  emit cross-target outcomes in allocation order so event IDs remain monotonic in the cue stream.
- Performance fixtures must construct the same stable owner/team/runtime components as gameplay and
  assert that the intended sustained population survives every sampled schedule step; a benchmark
  that silently cleans up its entities is measuring a different scene.
- Diagnostic telemetry needs an explicit retention policy and structured outcome logs from the
  start; process-lifetime vectors are not a safe substitute for bounded evidence history.
- The scripted input-loss case is an application-level Crossbeam fixture, not transport
  interception. It proves the repeat-through-12 then neutral contract; Lightyear conditioner runs
  provide the separate statistical UDP impairment evidence.
- A short profile run is useful for regression evidence only when the run IDs, direction, profile
  parameters, and convergence result are recorded together; the profile numbers must not be
  presented as deterministic latency guarantees.
- Windowed HUD blocks need explicit width constraints and alignment because a second client or
  compact window can otherwise make otherwise-correct control/status text overlap.
- Small-App presentation-timer tests using Bevy's `ManualDuration` need an initial schedule
  warm-up before asserting elapsed real time; otherwise the first update can observe no delta and
  make a correct bounded effect appear to outlive its contract.
- Collision presentation must be derived from the same authored shape helpers as server blockers;
  drawing only the out-of-bounds wall body makes a correct collision face appear invisible in a
  camera-clamped greybox arena.
- Evidence gates must validate the event stream itself, not only a generated report artifact or a
  sorted subset of IDs. Keep optional latency reporting separate from mandatory server/client
  convergence, and gate wall-clock evidence buffers so normal gameplay does not accumulate samples.
- Projectile range is a state invariant, not only a cast bound: every no-hit sweep must add its
  traveled step before the next tick, and same-tick attribution must use the fraction within the
  current sweep rather than the projectile's total path length.
- Internal combat messages make authority boundaries reviewable. A resource vector can preserve
  results but hide whether collision, damage, and presentation are ordered; explicit
  `ProjectileImpact`, `PendingDamage`, `DamageApplied`, and `FighterDefeated` readers/writers keep
  those stages independently testable.
- A process completion timestamp cannot support a fire-to-cue claim. The evidence harness now
  records server shot-acceptance and client muzzle-cue epoch samples keyed by `ShotId`, verifies
  both clients saw the same cue set, and reports latency distributions separately from readiness
  duration.
- Replicated absolute deadlines require a replicated clock reference. Comparing them with a local
  simulation counter is valid only while peers start together; reconnect-safe HUDs must use the
  authoritative tick arriving with the durable weapon state.
- Fixed-tick evidence must read the authoritative `SimulationTick` and recorded event ticks, not a
  transport/presentation timeline that can drift. Reset fixtures must also place authored bodies
  outside static cover, or physics correction can obscure whether the reset pose was restored.
- Process smoke launchers should supervise the built application binaries directly. Wrapping them
  in `cargo run` leaves an extra child-process lifecycle to drain during Ctrl-C and can make a
  subsequent fixed-port profile run race the previous server's shutdown.

The learn-from-errors review is complete for the current evidence. No reusable skill was created:
the lessons above are specific to this repository's Bevy/Lightyear composition rather than a
recurring cross-project workflow. Final milestone closeout is complete after explicit user approval;
physical-controller and actual alternate-display verification remain deferred observations.

## Exit checklist

- [x] Research questions are resolved or explicitly deferred with rationale.
- [x] Technical specification is accepted by the user.
- [x] The complete locked M03 implementation baseline is green before production implementation.
- [x] All tracked implementation tasks are complete without silent scope expansion.
- [x] The client sends only native intent; only the server accepts shots and owns projectiles,
  ammo/reload, hits, health, defeat, attribution, and reset.
- [x] Swept direct projectiles cannot tunnel at the accepted tuning and obey owner/ally/terrain/
  hostile collision policy.
- [x] Deterministic simultaneous damage, posthumous projectile, disconnect, repeated reset, and
  stable attribution behavior pass.
- [x] Two clients converge on durable combat state and receive deduplicated ordered feedback under
  local, delay, jitter, loss, and targeted duplication/reordering cases.
- [x] Late join/reconnect recovers current state without historical combat cues.
- [x] Debug HUD/effects make hit, health, ammo, reload, defeat, and reset understandable with
  keyboard/mouse across the selected windowed render profiles.
- [x] Physical-controller confirmation is explicitly triaged: basic final v1 testing was okay, and
  detailed target-hardware readability/feel work is deferred to `POST-V1-RELEASE-POLISH`.
- [x] Telemetry is locally inspectable and exact counter/distance rules are verified.
- [x] Dedicated server remains headless; locked automated, UDP/process, and performance gates pass.
- [x] User smoke-test feedback is incorporated or triaged.
- [x] Learn-from-errors review is complete and reusable lessons are captured where justified.
- [x] Roadmap status and current milestone are updated.
