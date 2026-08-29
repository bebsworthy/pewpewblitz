# Outcome

A new Splash weapon base launches or places a bounded persistent combat area. At each authoritative pulse, every live fighter inside receives the authored effects for which its owner/ally/enemy relationship is eligible. One area may combine up to two typed effects, including a hostile damage effect and an owner/allied healing effect.

# Scope

- Add one stable Splash weapon base and a typed persistent-area delivery/runtime family.
- Support authored Circle and Rectangle shapes with bounded size, placement/range, duration, pulse interval, maximum active areas per owner, and presentation profile.
- Support a bounded payload bundle containing one or two typed effects selected from Cold contribution, Fire, Poison, immediate periodic Damage, Slow, and Heal.
- Give each effect its own explicit Owner, Allies, and Enemies recipient mask. The first content must demonstrate the combined case by damaging enemies while healing its owner and allies in the same area lifecycle.
- Cold, Fire, Poison, and Heal reuse BRL-0003's target-owned rules, resistances where applicable, healing clamp/no-resurrection behavior, attribution, and outcome facts; Damage and Slow reuse existing combat outcomes.
- The server evaluates authoritative occupancy and resolves all eligible effects in deterministic source/target/effect order. Clients never report entry, membership, pulses, damage, healing, or status application.
- Define impact/placement validation, initial-overlap behavior, entry/exit, pulse deadlines, overlapping areas, per-effect strongest/refresh rules, damage/heal same-tick ordering, attribution, owner defeat/disconnect, respawn, restart, late join, and teardown.
- Replicate stable public area facts for presentation and evidence while keeping effect application server-only.
- Add aim preview, area warning, remaining lifetime, effect identities, allied/hostile recipient readability, audio, HUD/status feedback, Balance Lab controls, and Practice-bot awareness.

# Boundaries

- No more than two effect entries per area, arbitrary effect lists, user-authored scripts, unbounded stacking, moving areas, terrain ignition, projectiles modified in flight, or client prediction.
- Healing remains positive health application: it is clamped to resolved maximum health, cannot resurrect, and grants no damage-based ultimate charge.
- Recipient masks and effect entries are validated and bounded. Combining effects does not create a generic behavior graph or permit client-authored payload composition.
- A continuous channeled spray is owned by a separate ticket.

# Acceptance criteria

- Every supported shape and single-effect choice follows the same bounded area lifecycle and exact source attribution.
- A two-effect area can damage eligible enemies and heal its eligible owner/allies on the same pulse, with deterministic ordering, clamping, no resurrection, and correct per-effect cues/outcomes.
- Per-effect recipient masks are enforced server-side; owner, ally, and enemy eligibility cannot be claimed by clients and converges through routed replication/recovery.
- Entry after creation, remaining inside, leaving, re-entering, overlap, concealment, defeat, restart, late join, and recovery pass focused and routed tests for both single- and dual-effect areas.
- Maximum active areas and maximum two-effect 3v3 overlap stay within fixed-tick, evidence, wire, and presentation budgets.
- Native playtests confirm clear shape boundaries, friendly/hostile effect identity, healing versus damage feedback, duration, and counterplay.

# Feedback disposition — 2026-08-29

Accepted the request to include healing splash and dual-effect splash. The earlier single-choice/Everyone-only contract was unnecessarily narrower than BRL-0003's existing typed recipient-aware effect model. BRL-0040 now supports at most two typed effects with independent recipient masks, explicitly including enemy damage plus owner/allied healing, while retaining a hard bound and excluding arbitrary effect lists or scripting.


# Implementation contract — 2026-08-29

- Add `DeliveryMethod::Splash` as a targeted lob-to-stationary-area primary delivery. It authors maximum placement distance, flight/arc/clearance/muzzle geometry, `Circle` or oriented `Rectangle` area shape, duration, pulse interval, map occlusion, per-pulse target ceiling, and per-owner active ceiling.
- Keep area geometry in the delivery definition/state and require exactly one `Direct` payload bundle with one or two effects. The area authority chooses occupants; the existing composed payload transaction applies each effect's recipient policy, resistance, health ordering, attribution, cues, and outcomes.
- Splash permits Cold, Fire/Poison damage-over-time, immediate Damage, Slow, and Heal, but not Knockback or world effects. Effect entries must have distinct typed identities; Fire and Poison are distinct identities. The engine hard ceiling is two effects per Splash bundle.
- Add one replicated public `PersistentSplashState` with stable attack/source facts carried separately by `ReplicatedAttackSource`. Server-only runtime owns the recipe, match generation, next pulse, and delivery index. No process-local entity identity crosses the wire.
- Pulse immediately on authoritative landing and then at each interval through the inclusive expiry deadline. Owner defeat or disconnect does not remove an already-created area; match completion/restart/generation teardown does. In-flight delivery continues to use the ordinary validated projectile ownership lifecycle.
- Count in-flight and active Splash deliveries against the authored per-owner ceiling at attack acceptance, and retain a hard match ceiling. Deterministically reject or settle a landing that cannot acquire bounded capacity.
- First content is preset ID 7, `Splash`: a circular 96-unit area placed up to 480 units away, lasting 240 ticks, pulsing every 30 ticks, with at most two active per owner. Each pulse deals 36 damage to hostiles and heals owner/allies for 24, using map occlusion and a 6-target ceiling. All values remain authored and Balance Lab editable.
- Add Circle/Rectangle geometry tests, structural/limit validation, immediate and repeated dual-recipient pulse tests, capacity/expiry/cleanup tests, replication registration, aim/area presentation, Practice-bot range handling, catalog/fingerprint/schema updates, and role-specific verification before native handoff.

## Implementation progress — 2026-08-29

Implemented the first complete server-authoritative Splash vertical slice:

- Added schema-versioned `Splash` delivery and bounded circle/oriented-rectangle persistent-area definitions.
- Added authored Splash preset 7 with a hostile Damage 36 effect and AlliesAndOwner Heal 24 effect, nine inclusive pulses over 240 ticks, per-owner cap 2, and match cap 16.
- Added lob landing, stationary replicated area state, deterministic target ordering, fighter-footprint intersection, optional map occlusion, ordinary recipient filtering, composed payload execution, lifecycle cleanup, and telemetry settlement.
- Added client aim preview and distinct 3D persistent-area presentation for both supported shapes.
- Added Balance Lab numeric editing, persistence migration, catalog/build/protocol compatibility revisions, bot range awareness, durable weapon documentation, and backward migration coverage.
- Added focused validation/geometry/recipient tests and a separate-App network scenario proving lob-to-area replication and repeated authoritative damage.

Verification completed:

- `cargo test --locked --no-default-features --features client --lib` — 445 passed.
- `cargo test --locked --no-default-features --features server --lib` — 365 passed.
- `cargo test --locked --no-default-features --features balance-lab --lib` — 384 passed.
- `cargo test --locked --no-default-features --features network-test --test network combat_splash:: -- --test-threads=1` — 1 passed.
- `cargo clippy --locked --no-default-features --features client,server,balance-lab --all-targets -- -D warnings` — passed.
- `cargo clippy --locked --no-default-features --features network-test --tests -- -D warnings` — passed.
- `npm run --prefix tools/balance-lab-web test` — 10 passed.
- `npm run --prefix tools/balance-lab-web build` — passed.
- `git diff --check` — passed.

The ticket remains doing. Still required before closeout: broader network/capacity regression evidence, native gameplay/readability playtest, feedback disposition, and closeout learning.

## Native feedback and closeout — 2026-08-29

The user completed the native gameplay test and reported: “works for me.” The implemented Splash placement, persistent-area behavior, combined enemy damage and owner/allied healing, and presentation are accepted with no corrective feedback or deferred items from this playtest.

Closeout learning:

- Reusing the composed payload transaction kept recipient rules, damage/heal ordering, attribution, resistance handling, cues, and outcomes consistent instead of introducing a second effect engine for persistent areas.
- Keeping landing, pulse selection, payload application, and cleanup as separate ordered ECS phases made server authority and lifecycle boundaries testable while preserving the existing combat schedule.
- Historical Balance Lab migrations must advance through explicit prior schema pairs before adding new canonical catalog entries; tests now preserve that migration edge.
- Catalog-count assumptions were the main integration risk when adding a weapon base. Role-specific catalog, profile, parts, Balance Lab, and network tests caught every affected boundary.

All acceptance evidence is satisfied by the recorded automated verification, the separate-App replication/pulse test, bounded validation and capacity enforcement, and the accepted native playtest. BRL-0040 is ready for `done`.

## Durable documentation reconciliation — 2026-08-29

Reconciled the accepted Splash implementation into the enduring documentation:

- `docs/03-weapons-and-abilities.md` now records the seven-base catalog, corrected Sticky Blomb summary, exact Splash content values, inclusive pulse lifecycle, deterministic target/capacity rules, recipient-aware dual effects, persistence/cleanup, and replicated public facts.
- `docs/08-network-architecture.md` now records the durable `PersistentSplashState`/`ReplicatedAttackSource` boundary, late-join recovery model, server-private occupancy/effect authority, and disconnect lifecycle.
- `docs/10-bots.md` now records Splash range derivation and the explicit absence of private occupancy or a bot-only execution path.
- `docs/11-art-and-presentation-direction.md` now records circle/rectangle reconstruction, healing/hostile effect readability, fixed geometry, and presentation-only lifetime/pulse animation.
- `docs/15-balance-lab.md` now records schema versions 16/11/7, seven weapon bases, the 3×7 validation matrix, Splash numeric controls and structural locks, clean-epoch application, and all canonical recovery timings.

`git diff --check` passed. Historical implementation documents remain unchanged as delivery records.
