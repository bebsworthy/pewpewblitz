# Outcome

A player sees the same circular body that the server collides, and the straight-weapon aim trace shows the exact swept area and first client-observed contact for that body. Changing projectile radius changes authority, the live projectile footprint, and the aim-trace width together.

# Current implementation findings

1. `DeliveryMethod::Straight.radius` is the authored authority input.
2. The server uses that radius for the muzzle-clearance shape cast, spawned Avian `Collider::circle`, and per-tick swept-circle collision.
3. Radius is duplicated privately in `ComposedProjectileRuntime`; it is absent from replicated `StraightFlight`, and `Collider` is not a protocol component. The client therefore cannot render an arbitrary resolved projectile from authoritative body data.
4. The client renders every straight projectile as the same fixed `Cylinder::new(4.0, 28.0)`. Its 8-unit width and 28-unit travel-aligned length do not describe the authoritative circular body (Pulse radius 6, Scatter radius 4).
5. The current straight aim guide starts at the fighter center, has a fixed 3-unit width, extends only `range`, ignores `muzzle_offset`, and performs no collision trace. Spread shows only two boundary lines rather than every emitted pellet path.
6. The client has the resolved local weapon and replicated map/dynamic state, but it does not run the server physics world or receive map/projectile `Collider` components.
7. Avian 0.7 shape casting defines hit distance as travel of the complete cast shape before first contact. The public `contact_query::time_of_impact` path can apply the same collider narrow phase without adding an authoritative physics schedule to the client.

# Scope decisions

## Included

- Current straight delivery with one circular planar projectile body.
- One canonical runtime body fact consumed by server collision, replication, evidence, rendering, and aim preview.
- Exact body-width aim corridors clipped to map perimeter, projectile-blocking map assets, live damageable objects/objectives, and currently client-visible hostile fighters/deployables.
- Single and spread firing; every emitted straight projectile receives its own trace.
- Current-protocol migration, focused/network/presentation tests, native wall-corner playtest, and enduring documentation.

## Excluded

- New projectile shapes or curved/homing trajectories.
- Client authority, client-side hit prediction, rollback, or a replicated physics world.
- Explosion/area radii, lobbed landing geometry, and melee arcs except regression protection.
- A decorative trail or motion blur in the first pass. Such effects may be added later only if they remain clearly subordinate to the solid collision body.

# Gameplay and data model

Add a shared, immutable, replicated-once `ProjectileBody` component with the current field `radius: f32`. Keep it separate from `StraightFlight`: the body owns collision/presentation geometry, while `StraightFlight` owns trajectory inputs. Do not add a one-variant future-shape enum yet; evolve `ProjectileBody` when a second real shape is implemented.

The resolved straight recipe remains the authored source. On accepted fire, the server constructs one validated `ProjectileBody` and uses it to:

- validate muzzle clearance;
- construct the Avian circle collider;
- sweep the projectile each fixed tick;
- replicate the body to all observers;
- record evidence.

Remove the private duplicate radius from `ComposedProjectileRuntime`, or replace it with the shared body value so collision cannot drift from replication. Validation remains finite and positive, and `muzzle_offset >= fighter_radius + projectile_radius` remains enforced.

Register `ProjectileBody` in the global protocol and replicate it once. The current exact-version handshake handles the protocol-fingerprint change; no compatibility decoder is added.

# Projectile presentation

Replace the fixed travel-aligned 4×28 cylinder with a unit mesh whose X/Z footprint is scaled from `ProjectileBody.radius`. Recommended initial form: a compact low-height sphere/puck with a circular planar footprint and diameter exactly `2 * radius`. Vertical thickness, material, and height remain presentation-only, but there is no solid-looking extension ahead of or behind the collision body.

Projectile visual creation waits for `ProjectileBody`; it must not silently display a fixed fallback hitbox. Pulse Sidearm therefore displays a 12-unit diameter body and Scatter Cannon an 8-unit diameter body. Team/presentation-profile color remains independent from collision shape.

# Aim-trace contract

For each actual straight delivery angle, compute the same conceptual swept-circle query as the server:

1. Resolve direction and `muzzle = fighter_origin + direction * muzzle_offset`.
2. Sweep the circular body from fighter origin to muzzle to detect the same adjacent-cover/muzzle-blocked case as authority.
3. If clear, sweep from muzzle through the exact maximum projectile travel `range`.
4. Select the first eligible client-observed blocker using deterministic distance and stable-identity tie-breaking.
5. Return launch center, stop center, contact point/normal, travelled distance, and blocker class.

The visible guide is a ground-plane capsule/corridor of width `2 * radius`, beginning at the muzzle and ending with a circular cap at the projectile center at first contact or maximum range. A blocked result uses the existing blocked color at the terminal cap/short blocked path. A small non-collision connector from fighter center to muzzle is allowed only if visually distinct from the collision corridor.

The trace must use projectile collision policy, not player collision policy. Water and other player-only blockers therefore do not clip shots; walls, crates/cacti, live Heist safes, and other `BlockAndConsume` objects do.

Currently visible hostile fighters and sentries participate with their known circular body radii because authority consumes the projectile on them. Allies do not participate because the server predicate lets allied shots pass. Concealed/non-replicated enemies are never synthesized, so the preview cannot leak hidden state. Dynamic-target clipping is accurate to the client-observed frame, not a promise about future server position under latency.

# Client collision-query implementation

Do not install a second gameplay physics schedule. Build a client-only, read-only `AimTraceBlockerIndex` from the replicated resolved map and embedded map catalog:

- index only potential projectile blockers by grid cells/footprints;
- apply `MapDynamicState` terminal transitions when resolving the effective blocker;
- include map perimeter explicitly;
- derive Heist-safe rectangles from the resolved mode anchors and replicated live objective state;
- append the bounded visible fighter/sentry set per frame.

Query only buckets intersected by the swept capsule. For narrow phase, construct the same Avian circle/rectangle/circle colliders and use Avian's public time-of-impact/contact query so wall corners and tangency follow the physics library rather than a second hand-written approximation. This avoids scanning up to the full 512×512 placement envelope every render frame.

The index is presentation-only. It reads replicated state and cannot mutate collision, damage, projectiles, or map state.

# Preview representation

Replace anonymous `(center, angle, size, color)` tuples for straight aiming with named preview primitives/results that distinguish:

- collision corridor;
- circular start/end cap;
- blocked terminal marker;
- non-collision connector, if retained;
- existing lobbed/melee primitives.

Increase or redesign the bounded preview pool so the maximum permitted 16-delivery spread can display every projectile without truncating accuracy. The pool remains bounded and allocation-stable during aiming.

# Implementation order

1. **Canonical projectile body** — add/validate/register `ProjectileBody`; make server spawn and sweep consume it; remove duplicate radius ownership; extend evidence and protocol tests.
2. **Body-matched projectile visual** — replace the fixed cylinder, scale from replicated body, hide until body is present, and add footprint/presentation tests.
3. **Shared collision trace** — implement indexed blocker extraction and Avian time-of-impact tracing for perimeter, rectangular/circular map geometry, dynamic transitions, objectives, hostile fighters, and sentries.
4. **Body-width aim presentation** — render per-delivery capsules/end caps from muzzle to first contact; handle blocked muzzle, full range, and spread without truncation.
5. **Balance Lab and documentation** — retain one radius value, clarify that it owns visible body, collision, and trace width; update weapon/network/presentation/player-UX docs.
6. **Verification and playtest** — run focused, client/server/protocol/network/performance suites and the native matrix below; record feedback before closeout.

# Verification

## Focused rules

- Radius-to-diameter/planar-scale mapping for Pulse (6 → 12) and Scatter (4 → 8).
- Body validation rejects zero, negative, non-finite, and muzzle-overlap configurations.
- Circle sweep against rectangle face/corner, circular blocker, map bounds, exact tangency, initial overlap, and equal-distance tie.
- Destroyed/removed/replaced map placement changes the trace on the replicated dynamic revision.
- Player-only water does not clip; projectile-blocking cover does.
- Hostile fighter/sentry clips; ally, defeated, or absent concealed target does not.
- Muzzle-blocked and ordinary travel traces agree with authority start/range semantics.
- Every spread delivery is represented within the bounded pool.

## Network/authority

- Spawned straight projectiles replicate the exact `ProjectileBody` to both clients and late observers.
- Client input cannot mutate body geometry or collision.
- Static-world preview contact distance agrees with the server Avian sweep within a documented epsilon across face, corner, circle, perimeter, destructible-object, and Heist-safe cases.
- Existing lobbed, melee, concealment, destruction, and evidence behavior remains unchanged.

## Performance

- Measure aim tracing for the maximum 16-delivery spread on representative dense maps and a synthetic maximum-dimension indexed map.
- Index construction is bounded by map content and does not rebuild every render frame.
- Per-frame tracing visits intersected buckets rather than every placement and causes no material render-frame regression.

## Native playtest matrix

- Aim just inside and just outside a wall corner: clipped guide and bullet impact/clearance agree.
- Repeat with Pulse and Scatter radii; the wider Pulse clips sooner.
- Aim through player-only water and into blocking cover.
- Aim at a cactus/crate before and after destruction.
- Aim at a live Heist safe, hostile fighter, sentry, and ally.
- Fire while moving and at 30/60/high render profiles; visual body remains centered on replicated projectile motion.
- Change radius in Balance Lab and Apply & Reset; bullet footprint and trace width change together.

# Acceptance criteria

- No solid-looking part of the current straight projectile extends beyond its authoritative circular planar body.
- The aim corridor width equals projectile diameter and terminates where that body first contacts the client-observed blocker set.
- Pulse and Scatter visibly differ according to their authored radii.
- Wall-edge, object, objective, and visible hostile-body playtests show no reproducible guide-clears/bullet-hits mismatch for identical observed state.
- Server authority and fixed-tick sweep remain the only collision/damage decision path.
- Protocol, role isolation, bounded map capacity, and existing delivery modes pass their required gates.


## Clarification: extensible projectile geometry

This section supersedes the earlier recommendation to use `ProjectileBody { radius }` and to defer a shape enum. Multiple projectile geometries are now an explicit product requirement, so the shared geometry fact will be shape-bearing from the start:

```rust
pub struct ProjectileBody {
    pub shape: ProjectileShape,
}

pub enum ProjectileShape {
    Circle { radius: f32 },
}
```

Only `Circle` is implemented in this milestone. The enum is nevertheless justified now because it prevents the replicated protocol, authoritative collision code, renderer, and aim trace from treating radius as the permanent definition of all projectile bodies.

Geometry and trajectory remain independent concerns:

- `ProjectileBody` / `ProjectileShape` defines the authoritative planar collision footprint.
- `StraightFlight` defines how that footprint moves today.
- Future shapes may include capsules or rectangles when a concrete weapon requires them.
- Future trajectories may include curved, lobbed, accelerating, or homing motion independently of body shape.

Each implemented shape must provide one coherent set of behavior: authored validation, Avian collider construction, authoritative sweep/contact handling, replicated body data, a visual whose ground-plane footprint matches the collider, and an aim trace swept with that same shape. Adding a new enum variant without all of those adapters is not complete.

The authoritative geometry remains 2D because gameplay physics is planar. The 3D projectile presentation may choose height, material, rotation, and effects freely, but its XZ footprint must match the replicated shape. Protocol evolution remains exact-version: adding a later shape variant is a deliberate protocol/content-schema change, not a compatibility decoder.

## Implementation evidence — 2026-08-27

Implemented the accepted first slice:

- Added replicated immutable `ProjectileBody { shape: ProjectileShape::Circle { radius } }` as the shared geometry fact for straight projectiles and sentry bullets.
- Authoritative muzzle clearance, installed Avian collider, and fixed-tick sweep now derive from that body.
- Straight-projectile presentation scales its solid X/Z footprint from the replicated body.
- Straight aim preview sweeps the projectile body through projectile-blocking map geometry, the inset perimeter, live objectives, and currently visible hostile fighters/sentries. It renders the exact-width corridor and terminal body for every spread delivery.
- The map trace index follows dynamic removal/replacement state and remains empty for a supported 512 × 512 all-grass map.
- Client/server evidence snapshots include the body geometry.

Verification:

- `cargo test --lib --features client,server`: 586 passed.
- `cargo check --no-default-features --features server`: passed.
- `cargo check --no-default-features --features client`: passed.
- `cargo clippy --all-targets --features client,server -- -D warnings`: passed.
- Pulse network module: 4 passed, including exact radius-6 body replication to both clients.
- 100-fighter/200-projectile benchmark: passed at approximately 1.02 ms p95.
- `git diff --check`: passed.

Native visual acceptance remains a user playtest gate.

### Final verification note

After replacing the diagonal trace bounding-box scan with swept-corridor bucket traversal and adding water, dynamic-removal, static-contact, dynamic-contact, and maximum-map traversal checks, the final library result is 589 passed. This supersedes the earlier 586-test count.

## User playtest acceptance — 2026-08-27

The user confirmed that the body-matched projectile visual and aim trace work for them. Native visual acceptance is complete; BRL-0001 may close.
