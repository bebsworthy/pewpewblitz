# Milestone 03 — Movement, aiming, and greybox collision

## Tracking

| Field | Value |
|---|---|
| Version | v1 — gameplay MVP |
| Roadmap | [roadmap.md](./roadmap.md) |
| Status | Verifying |
| Specification validation | Accepted by explicit implementation request on 2026-08-13 |
| Implementation | Implemented; owner prediction remains disabled pending the required impairment evidence |
| Verification | Automated authority, collision, UDP/process, HUD, pause, and performance checks pass; visual, hardware, and prediction evidence remain open |
| User validation/playtest | Pending interactive controller/windowed playtest |

Update this table and the roadmap together whenever the milestone changes phase.

## Outcome

Two local players can move and aim simultaneously in a minimal replicated arena. Controller and
keyboard/mouse device state becomes the same tick-indexed gameplay input, the dedicated server
owns fighter position, facing, and collision, remote motion is interpolated, and local prediction
is adopted only if the recorded latency/convergence comparison meets this specification's gate.

Milestone 02's lifecycle corrections landed in commit `6a0bacf`; its interactive playtest remains
separate open work. Before Milestone 03 implementation begins, the complete locked Milestone 02
baseline must still pass. Milestone 03 extends the accepted server-owned entity and preserves the
compatibility, rejection flush, disconnect, reconnect, readiness, and graceful-shutdown contracts;
it does not absorb or close the remaining Milestone 02 playtest.

### Implementation decision

The authoritative/interpolated baseline is the delivered path. Owner prediction is deferred: the
repository has no impairment/measurement harness yet, so enabling rollback prediction would be an
unmeasured architectural expansion. The server remains authoritative, remote and owner views use
the same canonical replicated pose, and the prediction comparison is a scoped v1 backlog item for
the next evidence pass.

## Source requirements

- [Product direction](../../00-product-direction.md): combat readability, short feedback cycles,
  content composition, and network-first simulation.
- [Gameplay MVP](../../05-gameplay-mvp.md): controller-first action mapping, keyboard/mouse parity,
  deadzone and last-valid-aim behavior, server authority, and two-client local play.
- [Fighter model](../../02-fighter-model.md): movement speed and runtime position/facing remain
  distinct from later authored fighter definitions and selected builds.
- [Maps and game modes](../../04-maps-and-game-modes.md): plain symmetric test geometry and
  separation of indestructible terrain, future destructible terrain, objectives, and actors.
- [Network architecture](../../08-network-architecture.md): clients send intent, Lightyear-native
  inputs and component replication are preferred, remote state is interpolated, and prediction
  requires evidence.
- [Version 1 roadmap](./roadmap.md): Milestone 03 outcome, architecture, collision, protocol,
  verification, and exit requirements.
- [Milestone 01](./milestone-01.md) and [Milestone 02](./milestone-02.md): fixed-tick composition,
  exact-version discipline, server feature isolation, stable identity, ownership, compatibility,
  lifecycle, deterministic separate-app testing, and recorded learn-from-errors constraints.

## Scope boundaries

### In scope

- one abstract local action state covering move, aim, primary fire, active item, ultimate,
  interact, cancel, pause, and scoreboard;
- one active input device per client process, Xbox-like controller mapping and hotplug behavior,
  WASD/mouse parity, radial deadzones, trigger hysteresis, aim threshold, and last-valid facing;
- tick-indexed Lightyear native input carrying only server-relevant intent;
- ownership, tick-window, duplicate/reorder, rate, bit-mask, and magnitude validation;
- server-owned circular fighters with fixed-speed movement and facing at 60 Hz;
- Avian 2D 0.7 kinematic move-and-slide collision against a greybox arena;
- explicit collision layers and future-facing owner, ally, and self-hit policy;
- a locally authored symmetric arena with bounds, static cover, and stable spawn markers;
- replicated authoritative pose, remote snapshot interpolation and, if owner prediction passes
  its evidence gate, fixed-to-render smoothing, plus a follow camera, greybox fighter/arena
  visuals, and a minimal pause overlay;
- a measured baseline-versus-owner-prediction decision under latency, jitter, and loss;
- extensions to the existing deterministic Crossbeam harness and real UDP/process smoke path.

### Out of scope

- further Milestone 02 readiness, shutdown, or lifecycle corrections unless a regression is found
  and separately triaged;
- weapons, projectile entities, hit resolution, health, damage, defeat, reset, or combat feedback;
- teams, friendly-fire configuration UI, match phases, respawns, scoring, or mode rules;
- authored fighter definitions, builds, stats beyond one provisional movement speed/body size,
  abilities, items, aim assist, or target selection;
- destructible terrain masks or generated terrain colliders; this milestone only reserves their
  collision layer and proves that Avian can accept non-box geometry later;
- dynamic rigid-body fighter response, fighter body blocking, knockback, forces, joints, or
  solver-owned gameplay motion;
- input rebinding/settings UI, multiple local players in one process, touch input, accessibility
  remaps, a production pause menu, full HUD, audio, or production art;
- lag compensation, server rewind, remote-player prediction, predicted projectiles, prespawned
  client-authoritative entities, matchmaking, authentication, or internet deployment.

## Research questions and conclusions

### Exact versions, features, and composition

- [x] Keep Bevy `=0.19.1` and Lightyear `=0.29.0`; add Avian 2D `=0.7.0` with only `2d`, `f32`,
  `parry-f32`, and `serialize`. Do not enable default Avian features, debug rendering, picking,
  scene support, joints, or parallel physics without measured need.
- [x] Add Lightyear `input_native`, `interpolation`, and `avian2d` in every role that builds the
  identical protocol. Enable prediction/frame interpolation only for the accepted owner-prediction
  slice; do not rebroadcast inputs or predict remote fighters.
- [x] Add Bevy mouse, `bevy_gilrs`, sprite, text, and UI features only to the client composition.
  The dedicated-server feature check must continue to reject renderer, window, audio, asset-
  presentation, keyboard, mouse, gamepad, sprite, text, and UI features.
- [x] Preserve role ordering: Bevy base plugins, Lightyear role group, protocol registration,
  Lightyear/Avian integration, role-specific gameplay/input/presentation plugins, then endpoint
  entities. Protocol registration still completes before startup creates network entities.

### Input representation and transport

- [x] Use Lightyear's native input plugin, not an application gameplay-message channel and not a
  client-authored position. Lightyear owns the input tick and redundancy through its timeline and
  `InputMessage`; Brawler does not add a second sequence field.
- [x] Define a compact `FighterInput` with a signed quantized two-axis move value, an optional
  signed quantized two-axis aim update, and an allowed gameplay-button bit set for primary fire,
  active item, ultimate, and interact. Signed integers make NaN and infinity unrepresentable on the
  wire and give stable equality during buffering/rollback. Pause, cancel, and scoreboard remain
  local UI actions and never enter the authoritative input payload.
- [x] Send at the fixed-tick interval with Lightyear's five-packet redundancy and unordered-
  unreliable input channel. Always write a value for every simulated tick, including neutral;
  absence means a missing packet, not “no buttons pressed.”
- [x] Read current device state once before Bevy's fixed loop, keep held state current, and latch a
  short press until one fixed tick consumes it. If several fixed ticks run in one render frame,
  axes/held buttons repeat and a latched short press is emitted once.

### Authority and validation

- [x] Keep Lightyear `ControlledBy` as the ownership relationship and opt in to its controlled-
  target validator before Brawler validation. A connection can target only its accepted fighter;
  input before compatibility acceptance or after owned-entity cleanup has no valid target.
- [x] A Brawler validator runs after ownership filtering and accepts only one target, known button
  bits, at most 16 ticks of input history, a server-relative end tick in `[-120, +16]`, and a
  monotonic packet watermark per connection. An equal end tick is a duplicate; a lower end tick is
  stale or reordered. Both are ignored. Lightyear 0.29's native channel is unordered, so this
  watermark is the required application policy for reordered delivery. A newer packet's redundant
  history may overlap already accepted ticks, but it cannot rewrite an already simulated tick and
  the authoritative server never rolls backward.
- [x] Rate-limit accepted input messages with a token bucket of 120 messages/second and a burst of
  30 per connection. Extra packets are dropped and cannot advance movement because simulation
  consumes at most one input state per authoritative tick.
- [x] Lightyear repeats the last known state when a tick is missing. Brawler permits that for at
  most 12 server ticks (200 ms), then forces neutral movement/buttons while preserving facing until
  a fresh valid input arrives. Opening the pause menu writes neutral immediately.
- [x] Dequantization radially clamps move magnitude to one. An aim update must exceed the protocol
  aim threshold, is normalized, and becomes the authoritative facing. No/neutral aim preserves
  the previous facing; spawn facing is positive X. The authoritative pose is checked finite before
  and after collision even though malformed floats cannot arrive in `FighterInput`.

### Collision approach

- [x] Adopt Avian now rather than writing a temporary circle-versus-box solver. Milestones 04–05
  need swept projectile and overlap queries, while Milestone 10 needs generated polygon colliders.
  Avian supplies collider shapes, layers, spatial queries, depenetration, and move-and-slide without
  taking ownership of Brawler's movement, team, damage, or terrain rules.
- [x] Use a circular collider, kinematic rigid-body classification, custom position integration,
  zero gravity, and Avian `MoveAndSlide` against static terrain. A kinematic body is not moved by
  contact response automatically; Brawler computes desired velocity and writes the returned
  canonical position each fixed tick. The projected velocity is diagnostic only because current
  input directly determines next tick's desired velocity.
- [x] Every fighter `MoveAndSlide` call uses an explicit `SpatialQueryFilter` that excludes the
  moving entity and masks only blocking terrain: indestructible terrain in this milestone and
  destructible terrain when it becomes solid later. The default query filter includes all layers
  and does not inherit the fighter's `CollisionLayers`, so it cannot enforce the matrix by itself.
- [x] Fighters do not physically block teammates or opponents. This avoids body-block divergence
  and means owner prediction only needs deterministic static arena collision; remote fighters can
  remain interpolated.
- [x] Use Lightyear's Avian Position replication mode with `Position` and `Rotation` as canonical
  simulation pose, one-way pose-to-`Transform` synchronization, and Avian's normal transform and
  physics interpolation plugins disabled. Do not maintain a duplicate authoritative `Transform`
  or custom snapshot.

### Interpolation and prediction

- [x] Give baseline owner and remote fighters Lightyear `Interpolated` views and interpolate
  canonical pose on the delayed server timeline. Do not add a second frame-interpolation pass to
  those views. If owner prediction passes, its fixed-simulated view alone gets between-fixed-tick
  frame interpolation using `Time<Fixed>::overstep_fraction()`.
- [ ] Begin implementation with the fully authoritative owner path and record input-to-visible
  latency at local, 50 ms, and 100 ms round-trip conditions. Then run the identical input/collision
  system on an owner-only `Predicted` entity with zero configured input delay. Static arena
  colliders are built from the same definition in both worlds; remote fighters remain nonblocking
  and interpolated.
- [ ] Adopt owner prediction only if it reduces p95 input-to-visible latency by at least two fixed
  ticks at 100 ms RTT, returns its corrected canonical pose to within one world unit of the
  authoritative pose for the same simulation tick within 12 ticks after an impairment/correction,
  never crosses or persistently penetrates terrain, and has p95 render-space correction no larger
  than the 24-unit fighter radius. Record all results even if prediction is deferred.
- [x] Prediction does not weaken authority: the server still validates inputs and owns the pose;
  the client only simulates the same deterministic movement for immediate presentation and rolls
  back/replays on authoritative disagreement.

### Device input, camera, and pause

- [x] Bevy 0.19.1 represents each connected controller as an entity with `Gamepad`. Select the
  first controller that produces meaningful activity, switch to keyboard/mouse on meaningful
  activity, and fall back cleanly on disconnect. The local Bevy entity ID never enters the wire.
- [x] Apply radial move deadzone `0.20`, radial aim deadzone `0.25`, and commit right-stick aim only
  at magnitude `>= 0.35`; remap the remaining magnitude to `[0, 1]`. Configure RT press/release
  hysteresis at `0.55/0.45`. These values are provisional playtest tuning, not build attributes.
- [x] Mouse aim converts the primary window cursor through the active 2D camera and sends only the
  normalized direction from the local rendered fighter. Missing/outside cursor, failed conversion,
  a near-zero delta, or a non-finite result produces no aim update.
- [x] Follow the local rendered pose directly at first. Use an orthographic fixed vertical span of
  720 world units and clamp the camera center to arena bounds reduced by the current viewport half-
  extents. If a viewport is larger than an axis, center that axis. Do not add look-ahead or damping
  until visual evidence justifies it.
- [x] Pause is a local input/presentation context. It shows a simple overlay and emits neutral
  gameplay intent, but does not pause Bevy fixed time, Lightyear, the dedicated server, or an
  in-process server. The fighter remains present and vulnerable. Resume clears latches so an old
  combat action cannot replay.

## Research log

| Date | Source | Finding | Decision impact |
|---|---|---|---|
| 2026-08-13 | `docs/implementation/v1/{milestone-01.md,milestone-02.md,roadmap.md}` and current `src/{gameplay,protocol,server,client}.rs`, `tests/network.rs` | The current gameplay plugin executes in both roles; accepted fighters are server-owned through session `ControlledBy`; protocol registration and lifecycle ordering are already tested in separate apps. | Keep authoritative movement in a server role plugin, reuse only a deliberately predicted movement system on clients, evolve the owned entity in place, and extend the existing harness. |
| 2026-08-13 | Milestone 02 learn-from-errors, lifecycle commit `6a0bacf`, locked network test, and isolated server-feature check | Reliable rejection needs a flush boundary, protocol mismatch must remain non-panicking, exit must pass through network lifecycle, and readiness requires `Started` plus `Linked`. After the concurrent lifecycle commit landed, all 10 locked `network-test` cases and the current server feature-isolation check passed. | Preserve every lifecycle contract/test and re-run the complete baseline before Milestone 03 implementation; do not subsume its pending interactive playtest. |
| 2026-08-13 | `references/lightyear/examples/README.md` and `simple_box/{Cargo.toml,src/protocol.rs,src/client.rs,src/server.rs,src/shared.rs}` | Native inputs are written in Lightyear's fixed input set; the server consumes them at the matching tick; owners can be predicted while other clients interpolate. | Use native ticked input, `Controlled` to identify the local fighter, and one shared deterministic movement function only when prediction is enabled. |
| 2026-08-13 | Lightyear book `src/SUMMARY.md`, `concepts/advanced_replication/{inputs,interpolation,prediction,visual_interpolation,avian}.md`, `concepts/bevy_integration/system_order.md`, and `concepts/reliability/channels.md` | Network interpolation, fixed-to-render interpolation, prediction rollback, input redundancy, and replication send order are distinct concerns. | Specify each timeline and prohibit temporary render values from entering fixed simulation or replication. |
| 2026-08-13 | Cargo-resolved Lightyear 0.29 sources: `lightyear_inputs{,_native}-0.29.0`, `lightyear_{interpolation,prediction,frame_interpolation,replication,avian2d}-0.29.0` | Exact APIs use `FixedPreUpdate`, native `ActionState`, optional controlled-target validation, repeated-last-input fallback, input tick bounds, `PredictionTarget`/`InterpolationTarget`, and Avian Position-mode registration/correction. | Quantize input, add stricter Brawler validation/neutral timeout, use Position mode, and measure owner-only prediction. |
| 2026-08-13 | Live `BRAWLER_INPUT_TRACE=1` investigation plus Cargo-resolved `lightyear_avian2d-0.29.0/src/plugin.rs` and `lightyear_interpolation-0.29.0/src/{archetypes.rs,rules/bundle.rs}` | Focused WASD reached `PendingLocalActions`, Lightyear, and authoritative movement, and replicated `ConfirmedHistory<Position>` advanced. The client pose stayed at spawn because Avian's priority-4 `(Position, Rotation, LinearVelocity, AngularVelocity)` Hermite rule suppressed the single-component rules but could not sample Brawler's intentionally absent velocity histories. | Keep velocities server-local and register a priority-5 `(Position, Rotation)` interpolation bundle matching Brawler's wire contract. Retain an opt-in boundary trace and deterministic rule-precedence regression. |
| 2026-08-13 | `references/lightyear/examples/avian_2d/{README.md,Cargo.toml,src/shared.rs,src/protocol.rs}` | The supported integration disables Avian's duplicate transform/interpolation plugins and treats physics pose as canonical. | Follow the smallest Position-mode integration and omit dynamic interacting-player prediction. |
| 2026-08-13 | `references/bevy/examples/{README.md,input/gamepad_input.rs,input/gamepad_input_events.rs,2d/2d_viewport_to_world.rs,camera/2d_top_down_camera.rs,ecs/fixed_timestep.rs}` | The checked-in Bevy snapshot demonstrates device, cursor conversion, camera, and fixed/render concepts but is 0.20-dev. | Reuse the architecture only and verify all spellings against resolved 0.19.1. |
| 2026-08-13 | Cargo-resolved Bevy 0.19.1 `bevy_input/src/{lib.rs,gamepad.rs}`, `bevy_camera/src/{camera.rs,projection.rs}`, and released examples | Gamepads are ECS entities, raw input is processed before the fixed loop, `viewport_to_world_2d` returns a result, and camera projection exposes the area needed for clamping. | Define active-device lifecycle, before-fixed input latching, safe mouse aim, and aspect-aware camera bounds. |
| 2026-08-13 | Cargo-resolved Avian 2D 0.7 `examples/move_and_slide_2d.rs`, `src/character_controller/move_and_slide.rs`, `src/collision/collider/layers.rs`, `src/spatial_query/`, and `src/schedule/` | Kinematic bodies require explicit movement/collision handling; `MoveAndSlide` sweeps, depenetrates, and slides against filtered collider layers. | Adopt kinematic move-and-slide, reserve concrete physics layers, and keep Brawler in charge of outcomes. |
| 2026-08-13 | `references/avian/crates/avian2d/examples/{move_and_slide_2d.rs,kinematic_character_2d/}` | The checked-in official examples pass an explicit `SpatialQueryFilter` excluding the moving entity; the query does not inherit entity collision layers. | Require a moving-entity exclusion plus a terrain-only mask on every fighter movement query. |
| 2026-08-13 | [Lightyear 0.29 tag](https://github.com/cBournhonesque/lightyear/tree/0.29.0), [Avian 0.7 docs](https://docs.rs/avian2d/0.7.0/avian2d/), and [Bevy 0.19.1 tag](https://github.com/bevyengine/bevy/tree/v0.19.1) | Current primary sources confirm the pinned compatibility line and Avian's collision/query surface. Local resolved sources contain the most exact APIs used by this specification. | No version or architecture change was needed after the current-primary-source check. |

## Technical specification

### Decisions

| Concern | Selected design | Rejected/deferred alternative | Rationale |
|---|---|---|---|
| Gameplay input | Quantized `FighterInput` through Lightyear native inputs | Custom reliable input message; client position message | Reuses tick buffering/redundancy, prevents non-finite wire axes, and sends intent only. |
| Input sequence | Lightyear timeline/end tick plus Brawler per-connection watermark | Second application sequence number | One tick identity avoids disagreement and redundant validation state. |
| Canonical pose | Avian `Position` and `Rotation` on the server-owned fighter | Replicated Bevy `Transform`; aggregate snapshot | Keeps physics/network state singular; client transforms are presentation. |
| Fighter collision | Avian kinematic `MoveAndSlide` against terrain | Dynamic rigid body; custom AABB/circle solver | Provides stable arena movement and reusable query/collider infrastructure without solver-owned gameplay. |
| Fighter interaction | Fighters overlap; no body blocking | All-player collision/prediction | Avoids divergent dynamic contacts and preserves clear aim/movement control. |
| Remote rendering | Lightyear snapshot interpolation | Latest-state snapping; bespoke transform history | Uses advertised authoritative state history and remains smooth below render rate. |
| Local rendering | Authoritative baseline, then gated owner prediction and frame interpolation | Assume prediction; permanently defer measurement | Captures evidence while preserving a responsive path for the arena-shooter requirement. |
| Arena data | One code-authored immutable greybox definition shared by server/client composition | Asset pipeline; server-replicated wall entities | Small, deterministic, headless-safe scope; asset authoring belongs to Milestone 06. |
| Camera | Direct rendered-pose follow with projection-aware clamp | Fixed world camera; damped/look-ahead camera | Exercises follow/bounds without hiding movement latency behind camera lag. |
| Pause | Client-local context emitting neutral input | Pause fixed time/server | Multiplayer authority continues while a local menu is open. |

### Cargo and application composition

The package remains one crate with the existing client, server, and `network-test` configurations.
Implementation adds no service layer, domain crate, or networking facade.

- Pin `avian2d = "=0.7.0"` with default features disabled and `2d`, `f32`, `parry-f32`, and
  `serialize` enabled. Add the upstream-required `arrayvec` Serde feature explicitly if the pinned
  Avian build requires it; record that compatibility dependency so it can be removed on upgrade.
- Add Lightyear `input_native`, `interpolation`, and `avian2d` to identical protocol-building
  client/server/test feature sets. The accepted prediction slice adds `prediction` wherever the
  prediction-aware protocol is built and `frame_interpolation` to client/test composition for its
  public plugin, without adding render/device features to the server.
- Extend `bevy-client` with mouse, `bevy_gilrs`, sprite/sprite-render, text, UI/UI-render, and only
  the minimal dependencies those features require. Do not enable audio or a general default Bevy
  feature group.
- Keep `network-test` allowed to unify client/server features; it is not server-isolation evidence.
  The isolated `--no-default-features --features server` graph remains the proof.
- Re-run exact-version metadata/tree evidence and update the server-feature check for the new
  explicit forbidden features.

Plugin responsibilities:

| Plugin | Installed in | Responsibility |
|---|---|---|
| `GameplayPlugin` | client, server, tests | `Time<Fixed>`, `SimulationTick`, and fixed gameplay ordering only. Rename the fixed `Presentation` set to `Finalize`; variable-rate presentation does not run there. |
| `ProtocolPlugin` | client, server, tests | Existing compatibility types plus native `FighterInput`, fighter marker/identity, and the replicated Avian pose contract. Its priority-5 pose-only interpolation bundle overrides Avian's incomplete four-component Hermite bundle without replicating velocities. Bump both network protocol ID and Brawler protocol version for the incompatible registry change. |
| `AvianNetworkPlugin` | client, server, tests | Identical Lightyear/Avian `Position { sync_to_transform: false }` integration with automatic physics-component registration disabled, plus the one-way `Position`/`Rotation`-to-`Transform` writeback. `ProtocolPlugin` explicitly registers the narrower pose contract. Disable Avian's duplicate transform synchronization and physics interpolation paths. |
| `ArenaCollisionPlugin` | server; predicted client/test composition | Zero gravity, collision layers, local arena colliders, kinematic move-and-slide, and explicit terrain-only query filtering. Baseline interpolated clients do not run this simulation plugin. |
| `AuthoritativeMovementPlugin` | server and authoritative test app | Accepted-fighter initialization, input validators, stale-input fallback, facing, move-and-slide, and authoritative diagnostics. |
| `PredictedMovementPlugin` | client only if gate passes; prediction tests | The same deterministic facing/move-and-slide systems filtered to owner `Predicted` entities; no authority, lifecycle, damage, or presentation side effects. |
| `ClientInputPlugin` | windowed client; automation test client | Active-device tracking, raw-device-to-local-action mapping, before-fixed latching, pause context, and Lightyear input writes for the `Controlled` fighter. |
| `MovementPresentationPlugin` | windowed client | Greybox visuals, interpolation/prediction observers, camera follow/clamp, controls/status text, and pause overlay. Lightyear's Avian integration owns pose-to-`Transform` writeback. |

Do not put server-authoritative movement unconditionally in `GameplayPlugin`: the current plugin is
installed in both worlds. Shared code consists of pure quantization/deadzone helpers, immutable
tuning/arena data, and the deterministic pose step deliberately called by the authoritative
server and, only if accepted, an owner-predicted client system.

### Authored configuration and provisional tuning

`GreyboxArenaDefinition` is immutable data inserted into each applicable world:

- playable center `(0, 0)` and bounds `x = [-800, 800]`, `y = [-500, 500]`;
- four solid border colliders outside those inner limits;
- two symmetric indestructible cover rectangles centered at `(0, -220)` and `(0, 220)`, each
  `180 × 120` world units;
- eight stable spawn markers: `x = -620` and `x = 620`, each at
  `y = {-300, -100, 100, 300}`; accepted player ID selects `(id - 1) mod 8`;
- camera bounds equal the playable bounds and fixed vertical span `720` units.

`MovementTuning` begins with:

- fixed rate: existing 60 Hz source of truth;
- fighter radius: `24` world units;
- speed: `320` world units/second, with normalized diagonal movement;
- spawn facing: `0` radians (positive X);
- Avian move-and-slide: four movement iterations, default depenetration, and a provisional one-
  world-unit skin width using `100` world units per physics meter;
- stale input neutralization: 12 ticks;
- position validity: finite and within playable bounds after collision, allowing only the numeric
  skin tolerance.

`InputTuning` begins with move deadzone `0.20`, aim deadzone `0.25`, aim commit threshold `0.35`,
RT press/release `0.55/0.45`, tick window `[-120, +16]`, and rate/burst `120/30`. All values live in
focused configuration/resources rather than being scattered through device or movement systems.
Changing them during implementation requires updating tests and this specification; playtest
tuning after implementation is recorded in feedback review.

### Network protocol

Conceptual input shape (exact Rust field visibility/names may follow existing style):

```text
FighterInput
  move_axis: QuantizedAxis2       // signed x/y, radial magnitude <= 1 after decode
  aim_update: Option<QuantizedAxis2>
  gameplay_buttons: u8            // primary, active item, ultimate, interact only
```

Both axes quantize normalized values to signed 16-bit integers. Encoding clamps, maps `[-1, 1]`
to the symmetric usable integer range, and has pure round-trip/error-bound tests. Decoding cannot
produce a non-finite value. Unknown button bits invalidate that target entry.

Register `input::native::InputPlugin<FighterInput>` with a send interval equal to
`SIMULATION_TICK`, five-packet redundancy, no input rebroadcast, no lag-compensation payload, and
normal rollback behavior. Do not create a second application channel for input.

The accepted fighter keeps replicated-once `PlayerId`, `NetworkEntityId`, and fighter marker.
Configure `LightyearAvianPlugin` with automatic physics-component registration disabled, because
its default Position mode would also register `LinearVelocity` and `AngularVelocity`. Register only
`Position` and `Rotation` explicitly in `ProtocolPlugin` for replication and interpolation; when
owner prediction is retained, those components also use explicit rollback tolerances and visual
correction. `Transform`, velocities, collider shape, arena walls, spawn markers, client device
state, pause state, and camera state are not replicated.

The application-owned interpolation rule is the priority-5 `(Position, Rotation)` bundle. It must
win over Lightyear/Avian's automatic priority-4 pose-and-velocity Hermite bundle because the latter
cannot produce a complete sample when Brawler correctly omits velocity histories from the wire.

Server replication targets are selected by the recorded prediction decision:

- baseline: authoritative entity replicated and interpolated to all clients;
- accepted prediction: `PredictionTarget` only to the owning connection and
  `InterpolationTarget` to every other connection;
- `ControlledBy { owner: connection, lifetime: SessionBased }` remains in both cases and creates
  receiver-local `Controlled` only for the owner.

Set explicit replication metadata to one simulation tick. Late join receives current fighter pose
through component replication and constructs the immutable arena locally. Reconnect remains a fresh
accepted fighter with fresh stable IDs and the deterministic spawn selected from the new player ID.

### Input validation and missing-input behavior

Validation occurs before Lightyear writes received input into the entity buffer:

1. Lightyear's controlled-target validator removes entity targets not owned by the sending link.
2. Brawler accepts exactly one authorized `InputTarget::Entity` that maps to the active accepted
   session. It explicitly rejects zero/multiple targets and every `InputTarget::PreSpawned` target;
   Lightyear's generic validator intentionally passes prespawn targets through. It also rejects
   unknown bits, more than 16 ticks of history, end ticks outside
   `server_tick - 120 ..= server_tick + 16`, and rate-budget excess. Rejected or empty messages
   never refresh the fighter's last-fresh-input tick.
3. Per-connection `InputValidationState` records the greatest accepted packet end tick and token-
   bucket state. Equal/older packets do not update buffers or diagnostics as new intent.
4. Axis decoding and radial clamping happen before simulation. Move magnitude is at most one. An
   aim update below `0.35` after decode is treated as absent; a valid update is normalized.
5. The fixed movement system consumes at most one `ActionState<FighterInput>` for that tick. The
   last accepted state may repeat through at most tick 12 after its last fresh end tick; tick 13 and
   later force neutral movement/buttons while leaving `Rotation` unchanged.

Delayed input for a tick already simulated is diagnostic evidence only; the server does not rewind.
A newer redundant packet may fill not-yet-simulated buffer history. A duplicate or reordered older
packet cannot regress the accepted watermark. Invalid input increments bounded counters/log fields
without logging every packet at warning level.

### ECS ownership and entity lifecycle

#### Accepted authoritative fighter

Evolve the Milestone 02 placeholder entity in place after compatibility acceptance. It owns:

- `PlayerId`, `NetworkEntityId`, and a `Fighter` marker;
- `Replicate`, role-dependent interpolation/prediction target, and the existing session
  `ControlledBy` relationship;
- Avian `Position`, `Rotation`, local `LinearVelocity`/`AngularVelocity`, circular `Collider`,
  `RigidBody::Kinematic`, `CustomPositionIntegration`, and fighter `CollisionLayers`; Avian requires
  the velocity components locally, but the constant-speed input-driven design does not put them on
  the wire;
- Lightyear native input state/buffer components associated with that controlled entity;
- server-only last-fresh-input tick/neutralization state where it belongs per fighter.

There is no second authoritative fighter entity and no cached process-local entity ID in protocol
data. If `PlaceholderPlayer`/`PlaceholderState` are removed, update the protocol version and adapt
the Milestone 02 roster/lifecycle assertions to `Fighter`; do not discard their meaning.

#### Arena entities

Server arena entities own static Avian colliders, semantic wall/bounds markers, and stable local
spawn-marker IDs. Client arena entities own corresponding greybox presentation; a prediction-
enabled client also constructs identical static colliders. Static arena entities are app/match-
scope and are not owned by a connection. They are spawned once idempotently and removed only on
app/match teardown, not on player disconnect.

#### Client-local state and presentation

- `ActiveInputDevice`: keyboard/mouse or one connected gamepad entity;
- `PendingLocalActions`: current axes/held buttons plus unconsumed short-press latches;
- `ClientInputContext`: gameplay or paused-menu routing;
- presentation components/children attached to confirmed/interpolated/predicted fighter views;
- one camera tagged to follow the receiver-local `Controlled` view;
- optional measurement resources for input sample, visible-motion sample, rollback/correction,
  and final-error evidence.

Camera targeting is derived from Lightyear `Controlled`, not from a cached Bevy entity in network
state. When the controlled identity has a separate interpolated/predicted render view, the client
uses Lightyear's confirmed-to-render relationship rather than inventing a second identity map.
Disconnect/despawn removes fighter presentation; reconnect retargets when a new controlled view
arrives. Headless clients install automation input but no device, camera, sprite, text, or UI systems.

If owner prediction is retained, an add-observer/blueprint initializes every owner `Predicted`
view with local-only `RigidBody::Kinematic`, `LinearVelocity`, `AngularVelocity`, circular
`Collider`, `CustomPositionIntegration`, fighter collision layers, and `FrameInterpolate` from its
first simulated frame. Interpolated views remain non-rigid presentation entities. Lightyear does
not replicate or synthesize these application/body components, so prediction must not rely on
their presence by accident.

### Collision layers and gameplay policy

Reserve eight typed Avian physics layers now:

1. fighter;
2. projectile;
3. indestructible terrain;
4. destructible terrain;
5. objective;
6. pickup;
7. hazard;
8. deployable.

`B` means solid block/slide, `H` means a swept gameplay hit query, `S` means sensor/overlap, and
`—` means no physical/query interaction by default:

| A / B | Fighter | Projectile | Indestructible | Destructible | Objective | Pickup | Hazard | Deployable |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Fighter | — | H | B | B | S | S | S | — |
| Projectile | H | — | H | H | H* | — | — | H |
| Indestructible | B | H | — | — | — | — | — | placement |
| Destructible | B | H | — | — | — | — | — | placement |
| Objective | S | H* | — | — | — | — | — | — |
| Pickup | S | — | — | — | — | — | — | — |
| Hazard | S | — | — | — | — | — | — | — |
| Deployable | — | H | placement | placement | — | — | — | — |

`H*` applies only when a mode makes an objective damageable. This milestone implements fighter
movement against indestructible terrain and tests the reserved masks; it does not implement the
future rows' gameplay systems.

Future combat policy fixed before Milestone 04:

- direct projectiles ignore their owner for their full lifetime;
- friendly fire is off: allied fighters/deployables do not take damage and do not consume a direct
  projectile; opposing targets do according to the projectile definition;
- direct hits never self-damage;
- an area payload affects its owner only through an explicit authored `affects_owner` flag; the v1
  launcher splash is expected to enable it, with its scalar decided in the weapon milestone;
- objectives, pickups, hazards, and initial deployables are nonblocking sensors/targets; deployable
  placement separately validates overlap with terrain and other deployables.

### Schedule and ordering contract

```text
PreUpdate (shared/client device)
  Bevy device event/state processing

PreUpdate (server Lightyear receive)
  MessageSystems::Receive
    -> InputSystems::ValidateInputs
       controlled-target authorization -> Brawler packet/tick/rate validation
    -> InputSystems::ReceiveInputs / native input buffering

PreUpdate (client Lightyear receive, independently ordered within the client role)
  ReplicationSystems::Receive
    -> PredictionSystems::Rollback when prediction is installed

RunFixedMainLoop::BeforeFixedMainLoop (client)
  restore canonical frame-interpolated state
  latch current device actions and short presses once per render frame

FixedPreUpdate
  client: write FighterInput in Lightyear WriteClientInputs
  server: apply native ActionState for the current authoritative tick

FixedUpdate
  GameplaySet::Input
    select fresh-or-neutral input -> decode/clamp -> update authoritative facing
  GameplaySet::Simulation
    desired velocity -> Avian MoveAndSlide against terrain -> write canonical Position
  GameplaySet::Finalize
    validate pose -> record completed-tick diagnostics -> increment SimulationTick

FixedPostUpdate
  Avian fixed physics/query maintenance
  prediction/frame history update when installed

Update (client)
  Lightyear remote interpolation
  local pause/status UI state that does not feed fixed simulation directly

PostUpdate
  replication sends canonical state before any render-only value
  owner frame interpolation when prediction is enabled -> rollback visual correction
  Lightyear/Avian pose-to-Transform writeback
  camera follow/clamp -> transform propagation -> render

Last
  existing client Disconnect / server Stop AppExit bridges
```

`SimulationTick.0` means the number of completed fixed simulation steps. It increments once in
`Finalize`, strictly after pose validation. Tests and input diagnostics name the tick being
simulated before that increment. Client presentation remains variable-rate.

Use explicit system-set ordering and a narrow deferred-command flush only where an accepted fighter,
its ownership/input components, or a newly created predicted/interpolated view must be visible to a
same-frame consumer. Do not chain unrelated independent systems.

### Controller, keyboard/mouse, and UI mapping

| Action | Controller | Keyboard/mouse | Wire behavior |
|---|---|---|---|
| Move | Left stick | WASD | Quantized axis |
| Aim | Right stick | Cursor world direction | Optional quantized direction |
| Primary fire | RT (`RightTrigger2`) | Left mouse | Held button bit; no M03 gameplay effect |
| Active item | LB (`LeftTrigger`) | Q | Held button bit; no M03 gameplay effect |
| Ultimate | RB (`RightTrigger`) | E | Held button bit; no M03 gameplay effect |
| Interact | A (`South`) | Space or Enter | Held bit; a short press is latched for one tick; no M03 gameplay effect |
| Cancel | B (`East`) | Escape | Local UI only |
| Pause | Menu (`Start`) | Escape | Local UI only; emits neutral gameplay input |
| Scoreboard | View (`Select`) | Tab | Local UI only |

Meaningful gamepad stick/trigger/button activity selects that controller. Meaningful key, mouse
button, or mouse motion selects keyboard/mouse. On active-controller disconnect, prefer another
connected recently active controller, otherwise keyboard/mouse. Supporting two controllers in one
client process is explicitly deferred.

### Prediction evidence gate

Use deterministic input automation and receive-side link conditioners on both directions. Compare
the authoritative-only baseline and owner prediction with the same 10-second movement/aim script:

| Profile | RTT target | Jitter | Loss |
|---|---:|---:|---:|
| Local | local transport | 0 | 0% |
| Typical | 50 ms | ±10 ms | 0% |
| Adverse | 100 ms | ±20 ms | 2% independent |

Record simulation/input tick, first visible-motion frame/tick, authoritative and client pose,
rollback count, correction magnitude, wall penetration/crossing, and convergence time. Run enough
repetitions to report median and p95 rather than one trace. Lightyear 0.29's built-in receive link
conditioner does not expose a random seed, so fix and retain the input script/profile while logging
each statistical run; use the separately seeded deterministic test transport for regression cases.

If the gate passes, insert application-global `PredictionManager::default()` before any predicted
entity can arrive and configure
`InputTimelineConfig::default().with_input_delay(InputDelayConfig::no_input_delay())`. Keep
`PredictionTarget` only for the owner, frame interpolation from the predicted-view blueprint's
spawn, and Avian visual correction. If it fails, record which criterion failed, remove/disable the
owner prediction target and predicted simulation, retain remote/owner authoritative interpolation,
and add a scoped backlog item rather than weakening a criterion silently.

## Preparation evidence

Research/specification preparation on 2026-08-13 passed:

- `cargo fmt --all -- --check`;
- `cargo test --locked --no-default-features --features network-test --test network -- --test-threads=1`
  — 10 passed, 0 failed;
- `scripts/check-server-features.sh` — the current server graph excludes client presentation
  capabilities.

These checks establish a usable Milestone 02 code baseline; they are not Milestone 03
implementation verification, and the remaining Milestone 02 interactive playtest stays open.

## Implementation and verification evidence

Implementation and verification on 2026-08-13 passed for the automated/headless scope:

- `cargo fmt --all -- --check`;
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`;
- `cargo clippy --locked --no-default-features --features server --all-targets -- -D warnings`;
- `cargo test --locked --no-default-features --features client --all-targets` — passed;
- `cargo test --locked --no-default-features --features server --all-targets` — passed;
- `cargo test --locked --no-default-features --features network-test --test network -- --test-threads=1` — 18 passed;
- `cargo test --locked --no-default-features --features network-test --test performance -- --nocapture` — 100-fighter median 252.542 µs, p95 336.417 µs on aarch64 macOS;
- `./scripts/check-server-features.sh`;
- `BRAWLER_NETWORK_HEADLESS=1 BRAWLER_NETWORK_TIMEOUT_SECONDS=30 ./scripts/network.sh` — two
  headless UDP clients sent opposing movement/aim scripts; the server-side readiness marker was
  written only after both movement and facing assertions passed.

The focused tests now cover server-only shaping and center bounds, input watermark/rate helpers,
controller fallback selection, fixed-loop sampling order, wall stop/slide/corner behavior,
maximum-speed sweeps, deep depenetration, nonblocking fighter overlap, configured spawn resources,
facing rotation, interpolation markers, hostile input rejection diagnostics, late-join pose recovery,
static-arena stability, HUD/pause behavior, a real loopback UDP movement/pose case, the supervised
UDP/process movement smoke, and the 100-fighter performance measurement. Windowed interpolation
quality, hardware controller behavior, and prediction impairment evidence remain open.

The focused windowed movement investigation additionally proved the complete live path. Physical
`KeyD` and logical `d` produced `move_axis=(1, 0)`, the server advanced from the spawn pose, and the
client received advancing position history. Before the fix, its live `Position` and `Transform`
remained at `x=-620`; after the pose-only rule was installed, both advanced smoothly with the
history while client velocity components remained absent. The new small-App precedence test and
Crossbeam authoritative-pose convergence assertion protect that boundary. The deterministic
Crossbeam harness explicitly moves its presentation timeline to the newest history sample because
its synthetic time loop does not accumulate the wall-clock ping samples used for live timeline
synchronization.

## Trackable implementation plan

### Prerequisite and dependency composition

- [x] Confirm the in-progress Milestone 02 lifecycle corrections compile and its locked unit,
  network, UDP, process, and server-feature checks pass; record rather than absorb any remaining
  issue.
- [x] Add exact Avian and minimal Lightyear/Bevy features, compatibility dependency if required,
  and locked feature-isolation evidence.
- [x] Add the Avian/Lightyear Position-mode composition with duplicate Avian transform/physics
  interpolation disabled and no renderer/device leakage into the server.
- [x] Rename the fixed finalization set and lock `SimulationTick` completed-step semantics in tests
  and Milestone 01 documentation.

### Protocol and validation

- [x] Implement quantized axes, allowed button bits, `FighterInput`, native input registration,
  serialization/quantization tests, and protocol version/ID bump.
- [x] Attach client input production to the receiver-local controlled fighter and always write one
  input state per simulated tick.
- [x] Install ownership validation followed by Brawler tick/rate/shape validation and bounded
  diagnostics.
- [x] Implement monotonic packet handling and 12-tick repeat-then-neutral behavior without server
  rollback.

### Arena and authoritative movement

- [x] Implement the immutable arena/tuning data, stable spawn markers, local arena spawning, typed
  collision layers, and server static colliders.
- [x] Evolve the accepted placeholder into the owned kinematic fighter without regressing any
  Milestone 02 lifecycle or roster test.
- [x] Implement deterministic facing, normalized fixed-speed desired velocity, Avian move-and-slide,
  explicit moving-entity exclusion/terrain-only query filtering, pose validity enforcement, and
  completed-tick diagnostics in the declared set order.
- [x] Prove bounds, wall stop/slide/corner behavior, no tunneling at maximum speed, depenetration,
  and nonblocking fighter overlap.

### Client input and presentation

- [x] Implement active-device/hotplug state, controller mapping/tuning, keyboard/mouse mapping, and
  render-frame short-press latching into the shared local action state.
- [x] Implement safe cursor-to-world aim, last-valid facing, pause context/neutralization, and
  resume latch clearing.
- [x] Render distinguishable fighters and arena geometry, attach/detach presentation with replicated
  fighter lifecycle, and show concise controls/connection/pause text.
- [x] Implement direct rendered-pose camera follow with aspect-aware bounds clamping and no server
  dependency.

### Interpolation, prediction decision, and workflow

- [x] Configure explicit one-tick replication metadata and authoritative network interpolation;
  if owner prediction passes, verify its fixed-to-render smoothing at multiple render rates without
  double-smoothing network-interpolated views.
- [ ] Capture the authoritative-owner baseline, implement the owner-prediction comparison behind a
  removable configuration boundary, run all three impairment profiles, and record the gate result.
- [ ] For the prediction candidate, install `PredictionManager` before replicated spawn, configure
  explicit no-delay input timeline behavior, and initialize each predicted view's local physics
  blueprint plus `FrameInterpolate` at spawn.
- [ ] Keep or remove owner prediction according to the gate; the current composition deliberately
  retains authoritative interpolation while the required evidence is outstanding.
- [x] Extend the Crossbeam harness, bounded UDP/process smoke, supervised two-client scenario,
  CI lanes, developer commands, logs, and user playtest instructions.

## Test plan

### Pure and small-App tests

- [x] Quantization round trips endpoints/zero within its stated error and can never decode a non-
  finite axis.
- [x] Radial move/aim deadzones, magnitude remap, diagonal clamp, aim commit threshold, trigger
  hysteresis, and last-valid facing cover boundary values.
- [x] Known input ticks produce the exact fixed-speed displacement/facing and increment completed
  `SimulationTick` only after validated movement.
- [x] Missing input repeats only through tick 12, tick 13 is neutral, and facing persists.
- [x] Unknown buttons, rate excess, too-old/future, duplicate, reordered, and unauthorized packet
  targets have transport-fixture coverage and bounded diagnostics; delayed/lost packet recovery
  remains open.
- [x] Wall stop, tangential slide, inside/outside corner, maximum-speed sweep, spawn depenetration,
  bounds, fighter overlap, and terrain-only movement casts are deterministic in the headless Avian
  network harness.
- [x] Camera clamp handles common aspect ratios, viewport changes, and a viewport larger than the
  arena's horizontal axis.
- [x] Pause writes neutral gameplay input, leaves fixed time advancing, and clears latches.
- [x] Protocol registration is identical for both Lightyear roles and round-trips the new types;
  an intentionally mismatched registry is still rejected before fighter spawn.
- [x] The Brawler pose-only bundle overrides Avian's incomplete pose-and-velocity interpolation
  rule, applies populated position/rotation histories, and keeps velocity off the wire.

### Deterministic separate-app network tests

- [x] Two clients connect, receive two owned server fighters, and move/aim simultaneously to the
  same final authoritative poses observed by both clients.
- [x] A client cannot move the other fighter, insert/replicate a pose, exceed maximum displacement,
  leave bounds, or cross a wall.
- [x] Duplicate, reordered, stale, future, burst/rate-excess, and malformed inputs match the
  documented watermark behavior without advancing more than once per server tick.
- [ ] Delayed and lost packet recovery still needs a transport-impairment fixture proving the
  repeat-through-tick-12 then neutral rule end to end.
- [x] A late join receives current authoritative poses; disconnect removes the owned fighter and
  reconnect creates one fresh identity. Windowed presentation removal and camera retargeting remain
  part of the open presentation check.
- [x] All Milestone 02 rejection, timeout, cleanup, reconnect, readiness, and shutdown assertions
  retain their meaning after the placeholder-to-fighter migration.
- [x] Interpolation entities exist only for the intended receivers and numeric pose convergence is
  checked; visual quality and prediction corrections under impairment profiles remain open.
- [x] Arena static entities are not duplicated by join/reconnect and are not removed by session
  cleanup.

### Real UDP, process, performance, and visual verification

- [x] A real loopback UDP case proves ticked movement input, authoritative movement, facing, and pose
  replication, not only connection/roster behavior.
- [x] A supervised one-server/two-client automation scenario sends a known movement/aim script,
  asserts both server outcomes through a readiness marker, and propagates child failure/timeout/
  clean shutdown.
- [x] Locked format, Clippy, unit, isolated-role, network-test, UDP/process, and server-feature
  commands pass without weakening Milestone 01/02 lanes.
- [x] At least 100 simultaneously simulated headless kinematic fighters remain within the 16.67 ms
  fixed-tick budget; the recorded aarch64 macOS median is 252.542 µs and p95 is 336.417 µs.
- [ ] Windowed verification covers 30, 60, and high-refresh rendering; remote and local motion have
  no obvious fixed-tick judder, camera leakage, wall crossing, or correction oscillation.
- [ ] A real Xbox-like controller covers discovery, activity selection, disconnect/reconnect,
  radial deadzones/stick drift, RT threshold, aim neutral behavior, every mapped action, and parity
  with keyboard/mouse.

### Evidence rules

- End-to-end input evidence must pass through Lightyear's native input buffer; direct insertion of
  authoritative pose or `ActionState` is only a focused lower-level test.
- Time-dependent tests advance Bevy/Lightyear time and schedules explicitly; they do not sleep on
  wall-clock time except bounded real-process supervision.
- Deterministic transport impairment tests use fixed seeds and retain the input script/profile in
  failure output. Built-in Lightyear conditioner measurements retain the fixed script/profile and
  run identifier, but report statistics across repetitions because that conditioner is not seedable.
- Interpolation/prediction assertions inspect semantic markers and numeric convergence; screenshots
  or “looks smooth” alone are insufficient.
- Visual/controller checks complement automated authority/collision tests and cannot replace them.

## Visual and user smoke-test plan

The implementation handoff must provide one documented command for a dedicated server and two
distinguishable clients, plus focused individual commands. The user scenario:

1. Connect both clients and confirm each camera follows its own colored fighter.
2. Move diagonally and along each wall with controller, then repeat with keyboard/mouse.
3. Aim with the right stick, release it, and confirm facing remains; compare mouse aim while the
   camera moves and after window resize.
4. Press every mapped combat/interact control and confirm the input indicator changes even though
   combat is not implemented.
5. Hotplug/disconnect/reconnect the controller and confirm predictable active-device fallback.
6. Open pause on one client while moving the other; confirm the paused fighter stops sending motion,
   the other client and server continue, and resume does not replay a latched action.
7. Run the selected latency profile and report local responsiveness, remote smoothness, correction
   visibility, wall sliding, camera behavior, and any stick drift.

Known limitations in the handoff must explicitly state: fighters overlap; combat inputs have no
effect; pause does not protect the fighter or pause the match; greybox data is code-authored; input
remapping/aim assist are absent; and the recorded prediction decision applies only to movement and
static collision.

## Feedback review

Pending the user playtest. Each playtest item will be recorded as implemented now, deferred to the
v1 backlog, rejected with rationale, or awaiting evidence. Tuning changes update the relevant
constants/tests and preserve authority/collision invariants.

## Learn from errors

Implementation review completed. The main reusable lessons were to keep Lightyear's native input
buffer as the only input timeline, sample render input in `BeforeFixedMainLoop`, apply analog
shaping exactly once on the authoritative path, use strict fighter-center bounds, read configured
arena/tuning resources at spawn, and make ECS queries explicitly disjoint. Avian's zero-velocity
move-and-slide path required an explicit deep penetration threshold plus a regression test. The
first network smoke also exposed that ECS error logs can leave a process exit code green, so the
smoke path now requires a server-written movement/facing readiness marker. Bounded validation
diagnostics and viewport-aware camera tests are useful reusable patterns, but no new reusable
Codex skill is justified. The live movement failure also showed that an integration plugin's
automatic interpolation bundle must be checked against the application's narrower replication
contract: component history can advance while a higher-priority incomplete bundle suppresses the
live pose. A rule-precedence test must assert the applied component, not only marker presence or
client-to-client agreement. These lessons are specific to the current Bevy/Lightyear composition.
Final closeout remains pending delayed/lost transport evidence, prediction measurements, and user
windowed/controller playtest feedback.

## Exit checklist

- [x] Research questions are resolved or explicitly deferred with rationale.
- [x] Technical specification is accepted by the user.
- [x] The separate Milestone 02 lifecycle prerequisite is green before implementation begins.
- [x] All accepted implementation tasks are complete without silent scope expansion.
- [x] Controller and keyboard/mouse feed the same gameplay input and fixed movement system.
- [x] Two players move/aim simultaneously while only the server owns authoritative pose/collision.
- [ ] Invalid, stale, duplicate, reordered, excessive, delayed, lost, and missing input all have
  end-to-end verified outcomes; hostile validation is covered, but delayed/lost transport evidence
  remains open.
- [x] Fighters cannot leave bounds/cross walls and the collision matrix/policies are represented in
  code/tests where this milestone implements them.
- [ ] Remote network interpolation is visually acceptable; numeric pose convergence and
  interpolation markers are verified, but the windowed judder/correction check remains open. If
  owner prediction is retained, its fixed-to-render interpolation is also acceptable and is not
  applied to already network-interpolated views.
- [ ] The prediction gate has been executed and the keep/defer decision is supported by its required
  local/50 ms/100 ms RTT, jitter/loss, p95, and convergence evidence.
- [x] The dedicated server remains headless by dependency graph and runtime behavior.
- [ ] User smoke-test feedback is incorporated or triaged.
- [x] Learn-from-errors review is complete and no new reusable skill is justified by this milestone-
  specific feedback.
- [x] Roadmap status and current milestone are updated.
