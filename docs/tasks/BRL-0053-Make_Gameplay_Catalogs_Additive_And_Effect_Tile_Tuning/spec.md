# Outcome

Gameplay catalogs accept additive content that satisfies generic engine invariants, and resolved effect-tile tuning remains authoritative through movement and damage execution.

# Scope

- Replace exact weapon/build/weapon-part inventory cardinality, ID-range, kind, and cost assertions in production validation with generic uniqueness, ordering, bounds, cross-reference, and encoded-size validation.
- Preserve current inventory expectations as focused tests or release fixtures where they remain valuable.
- Replace exact-value effect-tile validation with bounded engine ceilings.
- Preserve resolved speed, slow, damage, and cadence values in runtime ECS state and consume them in movement and periodic damage systems.
- Update gameplay-content schema/fingerprint constants when required by the changed serialized contract.
- Do not add new delivery, payload, tile-operation, or protocol behavior families.

# Acceptance criteria

- Adding an eighth valid weapon preset using existing primitives requires only catalog/build-cost data and does not require production validator edits.
- Valid non-default effect-tile tuning in RON is accepted and changes authoritative runtime behavior.
- Invalid IDs, duplicate keys, broken references, unsafe bounds, and oversized artifacts are rejected.
- Existing built-in content resolves to unchanged gameplay values.
- Dedicated-server feature isolation and server authority remain intact.

# Verification

- Focused catalog validation and fingerprint tests.
- Effect-tile movement and periodic-damage ECS tests using non-default authored values.
- Existing map/content compatibility tests.
- Canonical client/server checks proportional to schema and feature-boundary changes.


# Implementation evidence

- Weapon, build, and weapon-part validators now accept additive definitions while enforcing non-empty bounded inventories, ascending unique non-zero IDs, valid metadata, parameter compatibility, point-cost limits, and encoded-size ceilings.
- Build-to-weapon validation requires exact point-cost coverage and is enforced before content fingerprinting and player-facing catalog advertisement.
- Effect-tile validation accepts the established safe authored ranges: Speed 1001..=2000 milli, Slow 100..=999 milli, Damage 1..=100, and cadence 6..=600 ticks.
- Replicated `EffectTileOccupancy` retains the complete resolved behavior. Authoritative movement and periodic damage consume those retained values directly. The incompatible component-shape change advances the global protocol compatibility version from 38 to 39; catalog fingerprint formats remain unchanged because their serialized catalog material did not change.
- Balance Lab installs candidate effect-tile tuning before ordinary map validation, so development tuning and production catalog validation share one truthful rule path.

# Verification evidence

- `just check` passed for routing, client, server, network-test, Balance Lab, and Balance Lab web build/tests.
- `just test` passed: routing suites; 455 client tests; 382 server tests; 403 Balance Lab tests; the mixed Balance Lab/network replication test; 90 network tests; and 12 performance gates.
- `just lint` passed formatting, all role-specific Clippy runs with warnings denied, server feature isolation, sole 3D renderer enforcement, and map cleanup enforcement.
- Post-lint focused tests passed for client effect tiles (2 tests) and server effect tiles (5 tests), including non-default authoritative movement, damage, and cadence behavior.

# Learn-from-errors review

- The first bounds draft used broader generic ceilings than the accepted BRL-0036 safety contract. Reviewing the owning specification before final verification corrected the bounds to the established envelope. Future data-driven refactors must distinguish removal of exact content snapshots from preservation of engine safety policy.
- The first test run correctly exposed two characterization assertions that encoded the old closed inventory/value behavior. They were replaced with explicit built-in fixture checks plus additive and invalid-bound cases, preserving today’s content expectations without keeping them as production constraints.


# Final review evidence

- Adversarial review identified two lifecycle risks before closeout: stale/no-map effect-tile occupancy could pulse damage, and catalog count ceilings were narrower than the advertised compatibility envelope. Runtime pulses now require the current live map generation, the ceilings match the advertised bounds, and focused regressions cover both cases.
- The final canonical `just test` run passed 455 client tests, 383 server tests, 404 Balance Lab tests, the mixed Balance Lab/network replication test, 90 network tests, and 12 performance gates.
