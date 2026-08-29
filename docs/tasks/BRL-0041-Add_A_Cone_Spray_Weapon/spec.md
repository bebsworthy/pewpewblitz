# Outcome

Players can equip a Spray weapon whose accepted primary attack instantly emits one stationary gas spray. The gas propagates through a short cone, is clipped locally by blocking geometry, lingers, and applies authoritative damage over time without launching a projectile.

# First slice

- Add one stable Spray weapon base using the existing magazine/ammunition recovery and fire cooldown.
- Each accepted attack immediately spends one round and captures an immutable world-space origin and facing. The player may move or turn afterward without moving, rotating, extending, or otherwise controlling that spray.
- Create one bounded server-owned spray runtime with authored propagation speed, reach, angle, linger duration, damage-pulse interval, damage, distance falloff, map-occlusion policy, and maximum targets per pulse.
- Propagate the gas front outward from the captured origin at the authored speed until it reaches maximum range. Gas already reached remains present until the runtime expires after its authored linger duration.
- Apply damage at fixed authoritative pulse intervals to eligible targets overlapping the gas area reached at that tick. Re-evaluate stable target selection, cover, concealment, and falloff for every pulse and apply the existing attribution and outcome facts.
- Clip propagation independently across the cone: the first blocking surface stops only the angular portion behind it while every unobstructed portion continues normally. Gas does not bend, diffuse, or find a route around blocking geometry.
- If blocking geometry disappears while the spray is active, the newly opened portion becomes occupied up to the spray's current global travel distance; propagation does not restart from the former blocker.
- Show the eventual local cone preview clipped by client-observed geometry and targets. After firing, show the gas front advancing through that footprint and provide distinct emission, contact, damage, audio, and controller feedback.
- Preserve server authority under movement, latency, concealment, spawn protection, defeat, disconnect, reconnect, restart, and dynamic map changes.
- Add Balance Lab fields, saved-brawler/weapon-base content, evidence, telemetry, and Practice-bot range/use behavior.

# Boundaries

- One accepted attack is one short perfume-like spritz, not a continuous or held-input channel.
- The spray is fixed to its captured origin and facing; it does not follow its owner or later aim input.
- No projectile entity, homing, client hit prediction, elemental status, healing, knockback, terrain destruction, or gas diffusion around cover.
- The lingering cone spray remains distinct from the persistent splash-area weapon owned by BRL-0040.

# Acceptance criteria

- Propagation timing, partial fill, full-reach transition, linger duration, damage cadence, entry, exit, and re-entry pass pure/ECS/network tests.
- The spray remains fixed when its owner moves or turns, and is cleaned up correctly on defeat, disconnect, reconnect, restart, and expiry.
- Cone geometry, boundary/tangency, locally clipped map occlusion, unaffected neighboring propagation, dynamic blocker removal, stable per-pulse multi-target selection, falloff, cooldown/ammo, attribution, concealment, and lifecycle pass focused tests.
- No projectile is spawned and no client can claim a spray contact or damage pulse.
- Aim preview, advancing gas presentation, contacts, and native impacts agree with the authoritative geometry observable by that client.
- Maximum 3v3 target density, bots, routed recovery, performance, presentation, documentation, and feedback gates pass.

## Implementation record — 2026-08-29

- Added stable Spray weapon base/presentation profile 6 and a validated `ConeSpray` delivery with authored propagation speed, reach, angle, linger, pulse cadence, map occlusion, and per-pulse target bound.
- Attack acceptance spends ammunition immediately and spawns one bounded replicated `ConeSprayState` with immutable origin/facing. The server runtime advances global reach, re-evaluates eligible targets and current blocking geometry on every pulse, routes damage through the existing payload/outcome pipeline, emits typed pulse cues, and never creates a projectile.
- Geometry blocking is ray-local: each target and each presented cone ray tests its own line against current static/destructible geometry. Neighboring rays continue, and a removed blocker exposes the current global reach on the next pulse/frame.
- Added replicated cone evidence/checkpoints, deterministic telemetry/cues, clipped advancing 3D gas presentation, aim preview, audio, controller rumble, saved-brawler/catalog support, weapon-part reach resolution, Practice-bot range behavior, and bounded active-spray cleanup.
- Added Balance Lab numeric fields and sequential snapshot/persistence migration to snapshot schema 15 and persistence schema 10.
- Updated the enduring weapons and Balance Lab documentation.

## Verification record — 2026-08-29

- `just check` — passed for routing, client, server, and network-test role graphs.
- `just lint` — passed, including Balance Lab web tests/build, all Clippy role matrices, dedicated-server feature isolation, V3 presentation guard, and V8 map cleanup guard.
- Focused server tests for Spray timing/validation — passed.
- Focused client geometry test for locally clipped neighboring rays and reopening after dynamic blocker removal — passed.
- Routed network Spray test — passed: replicated stationary cone state, no projectile, repeated authoritative damage, immutable origin/facing after owner movement/turn.
- Balance Lab migration/editor tests — passed, including migration of older snapshots to the canonical Spray entry.
- Complete deterministic role suites passed: client 444 tests, server 361 tests, Balance Lab 380 tests, routing suites, Balance Lab routed catalog case, network 89 tests, and performance 12 tests.
- Performance gate remained within the fixed-tick budgets on the aarch64 macOS host.

## Feedback and remaining evidence

- The clarified perfume-spritz behavior and partial geometry blocking are implemented in this pass.
- Native subjective confirmation is still required for gas readability, advancing-front feel, clipped-cover agreement, audio, and controller impact. Keep the ticket in `doing` until that playtest is accepted and any resulting feedback is dispositioned.

## Learn-from-errors review

- Adding a sixth weapon exposed several exact-count and sparse-ID fixtures plus an old Balance Lab migration fixture that still modeled a five-weapon catalog. The first complete test run found these assumptions; the fixtures now explicitly model all six bases and the sequential migration removes both post-schema additions before replaying migrations.
- The initial rumble system assumed the gamepad message resource existed in minimal network-test apps. It now treats that presentation resource as optional, preserving headless composition.
- Spray timing validation originally derived pulse counts before rejecting a zero interval. Validation now rejects primitive invalid values before performing division, and a regression test covers zero cadence.

## Balance Lab decimal-entry correction — 2026-08-29

- Tick-backed duration fields now accept ordinary decimal seconds and round to the nearest authoritative tick instead of requiring the typed decimal to already equal an exact 1/60-second multiple.
- `0.17` seconds therefore resolves to 10 ticks; unscaled integer fields such as health continue to reject fractional values.
- Tick durations display to two decimal places and explain that values are saved to the nearest server tick.
- Balance Lab web tests/build, the focused editor manifest test, and Balance Lab Clippy pass after the correction.


## Native playtest acceptance — 2026-08-29

- The user confirmed the stationary perfume-like spray behavior works for them after reviewing the distance falloff controls and exercising the Balance Lab decimal-entry correction.
- Native playtest evidence is accepted; no remaining BRL-0041 feedback or closeout gate remains.


## Documentation clarification — 2026-08-29

- Updated the durable weapons specification to define Spray falloff start, falloff end, and minimum damage scale, including what falloff does not affect.
- Updated the Balance Lab operator guide to explain ordinary decimal-second entry, nearest-60-Hz-tick storage, two-decimal display, and the three Spray falloff controls.
- `git diff --check -- docs/03-weapons-and-abilities.md docs/15-balance-lab.md` passed. Concurrent Splash documentation changes in the weapons guide were preserved unchanged.
