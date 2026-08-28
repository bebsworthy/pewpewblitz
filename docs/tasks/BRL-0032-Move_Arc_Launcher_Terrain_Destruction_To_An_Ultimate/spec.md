# Outcome

Ordinary Arc Launcher attacks retain their lobbed damage, knockback, and slow behavior but can no longer destroy map cells. A new targeted Demolition Strike ultimate spends full charge to apply the existing bounded terrain-destruction brush at an accepted landing point.

# Scope

- Remove the DestroyMap world effect from the canonical Arc Launcher weapon recipe, resolved weapon data, Balance Lab defaults, presentation copy, evidence expectations, and fingerprints.
- Add one stable targeted ultimate definition with authored maximum range, landing telegraph, destruction shape/radius, charge cost, and presentation profile.
- Reuse the existing authoritative map-destruction transaction, mutation replication, recovery snapshot, replacement rules, and attribution.
- Demolition Strike changes terrain only in the first slice. It does not add fighter damage, status, structural collapse, chained destruction, or arbitrary brushes.
- The client shows targeting range, landing area, blocked/rejected activation, charge consumption, impact, and resulting map mutation without deciding the outcome.
- Practice bots may activate the ultimate only through ordinary validated input and permitted map facts.

# Constraints

- No ordinary weapon recipe or weapon part may retain terrain destruction.
- Destruction remains bounded to existing destructible placements and cannot affect indestructible walls, mode anchors, objectives, spawns, or out-of-bounds cells.
- Exact-version content/protocol fingerprints advance; no compatibility decoder is added.
- Defeat, respawn, disconnect, restart, late join, and map recovery converge through existing authority paths.

# Acceptance criteria

- Repeated Arc Launcher attacks never mutate the map.
- An accepted fully charged Demolition Strike mutates each eligible placement exactly once and consumes charge exactly once.
- Rejected or interrupted activations do not mutate terrain or consume charge.
- Routed clients, late joiners, bots, evidence, Balance Lab, HUD, audio, and 3D presentation agree with the authoritative result.
- Focused, network, recovery, performance, and native readability/balance checks pass.
