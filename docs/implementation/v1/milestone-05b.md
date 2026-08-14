Below is the concrete work needed to bring Milestone 5 to its stated exit criteria. I would execute it in this order so later evidence is trustworthy.

## 1. Replace fabricated state-convergence evidence

Affected: [src/server.rs](/Users/boyd/wip/brawler/src/server.rs:306), [src/combat/mod.rs](/Users/boyd/wip/brawler/src/combat/mod.rs), [scripts/network-combat-profiles.sh](/Users/boyd/wip/brawler/scripts/network-combat-profiles.sh:53)

Implementation:

- Define a bounded, normalized combat snapshot keyed only by stable IDs.
- Include, for every relevant fighter:

  - `NetworkEntityId`
  - `SelectedBuild`
  - resolved recipe fingerprint and public configuration
  - `WeaponState`
  - `ActiveEffects`
  - `Defeated`
  - authoritative position/facing
  - `AuthoritativeTick`

- Include active projectiles keyed by `(AttackId, delivery_index)` with:

  - presentation profile
  - current position
  - `LobbedFlight`, when applicable
  - launch/landing deadlines

- Have the server and both clients capture snapshots at named checkpoints:

  - active scatter flight
  - active lob flight
  - active slow/knockback
  - defeat
  - reset

- Compare client snapshots with the authoritative server snapshot after replication settles.
- Write `state_converged=1` only after a real equality comparison.
- Record state-mutation and client-observation timestamps so state-convergence median/p95 is actually measured.
- Report cue count/volume per run; the current script does not do that.
- Remove the unconditional `state_converged=1`.

Proof required:

- A deliberately altered client snapshot makes the profile command fail.
- A missing `ActiveEffects`, projectile, deadline, or resolved fingerprint also fails.
- The twelve impairment runs pass with real convergence results.

## 2. Add the missing M05 network authority tests

Affected: [tests/network.rs](/Users/boyd/wip/brawler/tests/network.rs:63)

Add separate deterministic tests rather than expanding the existing scatter smoke test into one large scenario.

### Selection validation

Test through the registered `WeaponSelectionRequest` channel:

- Unknown preset returns `UnknownPreset` and leaves the fighter selecting.
- Duplicate request ID returns the byte-equivalent previous outcome.
- Lower request ID returns `StaleRequest`.
- Higher request after acceptance returns `NotSelecting`.
- Two requests queued in one update cannot both activate.
- A request received on one connection cannot alter another connection’s fighter.
- Client writes to replicated `SelectedBuild`, `ResolvedWeapon`, `WeaponState`, or pose do not mutate the server.
- Pre-selection buffered fire/movement remains neutral until a newer native input tick arrives.

### Launcher

Set up owner, hostile, ally, neutral dummy, and occluding terrain, then prove:

- The lob remains non-colliding during all 45 flight ticks.
- The replicated landing matches the server-resolved landing.
- It explodes exactly once.
- Hostiles receive 40 damage, knockback, and slow.
- The owner receives 20 damage and full knockback, but no slow.
- Allies are unaffected.
- Terrain-occluded targets are unaffected.
- Both clients converge on health, slow deadline, pose, projectile removal, and cues.
- No client-authored landing, damage, radius, or effect value can enter the request path.

### Blade

Prove:

- All visible hostile targets inside the sector are hit.
- Targets outside the reach/angle are not hit.
- Targets behind terrain are not hit.
- Owner, allies, and defeated fighters are excluded.
- No projectile entity is spawned.
- Target processing and cues follow stable `NetworkEntityId` order.
- Damage and knockback converge on both clients.

### Late join

Create focused cases for joining during:

- weapon selection;
- scatter flight;
- lob flight;
- reload/recharge;
- active slow;
- active knockback;
- defeat.

Assert durable state and deadlines, and verify historical cues are not required.

### Content mismatch

Start a client with a deliberately different `GameplayContentFingerprint` and prove:

- `ContentMismatch` is returned;
- no fighter is spawned;
- the session and placeholder are cleaned up;
- reconnect with the correct fingerprint succeeds.

### M04 generalized regressions

Repeat the important M04 invariants through the resolved Pulse recipe, not the legacy flat pipeline:

- same-tick spawn/hit;
- closest-impact ordering;
- atomic ID exhaustion;
- neutral hostility;
- disconnect-before-impact cleanup;
- global cue ordering;
- bounded evidence;
- completed authoritative tick publication.

## 3. Correct and complete weapon telemetry

Affected: [src/combat/telemetry.rs](/Users/boyd/wip/brawler/src/combat/telemetry.rs), [src/combat/mod.rs](/Users/boyd/wip/brawler/src/combat/mod.rs)

### Damage totals

Change both top-level and source-keyed aggregates from:

```rust
hostile_damage += 1;
self_damage += 1;
```

to saturating addition of the applied damage amount:

```rust
hostile_damage = hostile_damage.saturating_add(u64::from(amount));
```

If hit-event counts are useful, add separately named fields such as `hostile_damage_events`.

### Distance semantics

For every payload, calculate:

```text
engagement_distance = source attack origin → target fighter center at contact
```

Keep delivery travel separate:

- straight: accumulated projectile path;
- lob: launch-to-landing ground path;
- melee: zero, because it is instantaneous.

Specific corrections:

- Straight currently uses only the current tick’s projectile step.
- Launcher currently uses landing-to-target blast distance for both fields.
- Melee already has the intended engagement-distance basis.

### Exact outcome records

The bounded history should contain records for:

- selection accepted;
- attack accepted;
- delivery impact/landing/melee contact;
- hostile damage and self-damage;
- knockback applied;
- slow applied/refreshed;
- defeat;
- cancellation/expiry where required for tracker completion.

Effect records must contain the state after that effect was committed, not the pre-effect snapshot.

### Attack tracker

Use `ActiveAttackTracker.had_hostile_contact` as the authoritative fold:

- mark it on contact;
- increment `attacks_with_hostile_contact` once when all deliveries finish;
- remove the tracker on resolution, expiry, disconnect, or shutdown;
- do not depend on a long-lived global contacted-attack set for correctness.

Proof required:

- One 25-damage Pulse hit reports 25 damage, not 1.
- Seven scatter pellets report their exact total applied damage.
- Launcher self-damage is separated from hostile damage.
- A multi-target blade swing increments attack hit count once.
- Close/mid/long classification uses shooter-to-target distance.
- Histories exceeding 512 preserve aggregate totals and increment drop counters.
- Hit rate never exceeds 100%.

## 4. Complete preview and durable effect presentation

Affected: [src/combat/client.rs](/Users/boyd/wip/brawler/src/combat/client.rs:9), [src/combat/mod.rs](/Users/boyd/wip/brawler/src/combat/mod.rs)

Use a bounded number of reusable preview primitives.

### Pulse

Render:

- range line;
- distinct maximum-range endpoint marker.

### Scatter

Render:

- both cone boundaries;
- a segmented end arc at maximum range;
- visually shorter/range-focused styling than Pulse.

### Launcher

Render:

- flight-path hint;
- distinct landing-point marker;
- circular explosion-radius ring centered on the landing;
- different styling for normal, clamped/repaired, and invalid landing results.

The current four offset diameters should be replaced with bounded ring segments centered around the landing point.

### Blade

Render:

- both sector boundaries;
- a segmented reach arc joining them;
- optional subtle sector fill if readability needs it.

### Durable slow state

Add a client system that reads replicated `ActiveEffects` and `AuthoritativeTick`:

- attach a persistent slow visual/status marker to the affected fighter;
- show remaining slow ticks in the local HUD when applicable;
- update it from durable state, not only `EffectApplied` cues;
- remove it on expiry, defeat, reset, or entity despawn;
- make late join display an already-active slow immediately.

Knockback can remain pose-driven, but the feedback needs to remain visually distinguishable from normal movement.

Proof required:

- Headless presentation tests assert the expected bounded segment count for each recipe.
- Preview transforms and sizes are finite.
- Invalid/clamped launcher states use distinct colors.
- Active slow creates a durable marker without a cue.
- Removing or expiring `ActiveEffects` removes the marker.
- Compact-window text does not overflow the intended layout.
- Manual 30/60/high-refresh and controller checks are recorded.

## 5. Implement the approved catalog policy boundary

Affected: [src/combat/definitions.rs](/Users/boyd/wip/brawler/src/combat/definitions.rs:60), [content/v1/weapons.ron](/Users/boyd/wip/brawler/content/v1/weapons.ron)

Expand `WeaponRecipePolicy` to cover the specification’s actual policy seam.

At minimum, policy needs:

- permitted economy families;
- permitted firing patterns;
- permitted delivery methods;
- permitted target-selection methods;
- permitted payload effects;
- permitted recipient policies;
- narrowed maximum values for:

  - capacity;
  - cooldown/refill/effect durations;
  - projectile/lob lifetime;
  - damage;
  - speed/distance;
  - radii;
  - knockback;
  - angle;
  - collection counts.

Validation must:

- reject policy values wider than `EngineWeaponLimits`;
- reject recipes using disabled capabilities;
- validate recipe values against both engine and catalog limits;
- reject duplicate or noncanonical capability entries;
- reject nonascending preset IDs/keys if that remains the approved contract;
- retain sorting during fingerprint generation as defensive canonicalization;
- include the entire policy in the content fingerprint.

Proof required:

- Narrowing catalog damage to 50 rejects a 51-damage recipe.
- Disabling `Lobbed` rejects the launcher recipe.
- Content cannot widen any code-owned limit.
- Reordered catalog presets fail validation if ascending order is mandatory.
- Comments and whitespace still do not alter the fingerprint.
- Policy changes do alter the fingerprint.

## 6. Finish combat architecture consolidation

Affected: [src/combat/mod.rs](/Users/boyd/wip/brawler/src/combat/mod.rs), [src/combat/](/Users/boyd/wip/brawler/src/combat)

The intended end state should be:

- `definitions.rs`: authored catalog, recipes, limits, resolver, fingerprints.
- `attack.rs`: economy advancement, attack acceptance, ID reservation, firing-pattern expansion.
- `delivery.rs`: straight sweep, lob progression/landing, melee sector geometry.
- `effects.rs`: target resolution, damage, knockback, slow, defeat ordering.
- `telemetry.rs`: trackers, aggregates, exact records, summary generation.
- `server.rs`: server plugin composition and schedule registration.
- `client.rs`: preview, projectile, cue, effect, and HUD presentation.
- `mod.rs`: public types/re-exports, system sets, and shared composition root.

Most importantly:

- Remove the independent legacy `authoritative_fire → sweep_projectiles → PendingDamage` gameplay path.
- Run Pulse through the same `ResolvedWeapon` and composed delivery/payload systems as all other presets.
- Preserve M04 compatibility cues through a small adapter emitted from the shared pipeline.
- Convert tests that currently depend on flat `WeaponDefinition` or legacy projectiles to resolved Pulse fixtures.
- Remove obsolete flat weapon/runtime types once no live or test consumer needs them.

Proof required:

- No authoritative gameplay system branches on preset ID.
- No fighter can fire without `ResolvedWeapon`.
- Only one economy, attack, payload, damage, defeat, and telemetry path exists.
- All M04 regression tests still pass through resolved Pulse.
- `combat/mod.rs` becomes a composition/public-API root rather than the implementation container.
- Client-only code remains absent from the server feature graph.

### Architecture-remediation gate approved after the first remediation pass

The first remediation pass did not satisfy this section. At commit `e19ee83`, `src/combat.rs`
still contains shared protocol-facing state, both combat plugins, authoritative attack/delivery/
effect systems, client presentation, telemetry orchestration, and process evidence. The other
large roots also mix distinct ownership boundaries: `src/client.rs` combines connection, input,
selection, presentation, and headless automation; `src/server.rs` combines sessions, selection,
shutdown, and process verification; `src/movement.rs` combines arena data, input validation,
collision, and authoritative movement; and `tests/network.rs` combines its reusable harness with
all network scenarios.

This gate is behavior-preserving remediation, not a new gameplay feature and not a line-count-only
exercise. Keep one package and Bevy `World`; do not introduce service/use-case layers, facade
crates, or one plugin per source file. Organize by ECS state ownership, execution role, plugin
composition, schedule phase, and test scenario.

Execute in independently verifiable slices:

1. Capture the deterministic pre-refactor unit, role-specific, network, and performance baseline.
   Record the local preset-1 impairment failure as known pre-refactor evidence; do not complete the
   twelve-run matrix until the code moves are verified.
2. Extract the reusable network harness and split integration scenarios by selection, combat
   delivery, recovery, movement, lifecycle, and compatibility without changing assertions.
3. Make `combat` a directory-owned module whose root contains intentional re-exports, shared
   system sets, and plugin composition. Separate shared model/cues, definitions, attack economy,
   delivery, effects, telemetry, evidence, authoritative systems, and client presentation.
4. Decompose oversized combat systems into schedule-facing orchestration plus named helpers for
   validation, deterministic candidate collection/order, state mutation, telemetry folding, and
   cue emission. Moving a large function unchanged does not satisfy this gate.
5. Split client composition from connection lifecycle, input, selection, presentation, and
   headless automation; split server composition from sessions, handshake, selection, shutdown,
   and process checks; split movement composition from arena, input, collision, and authoritative
   execution.
6. Correct clearly misplaced ownership exposed by the extraction in small reviewed changes. Keep
   replicated/wire shapes stable unless a separately tested correction is required.
7. Remove obsolete paths and replace file-wide complexity suppressions with narrow justified
   exceptions. Default new visibility to private or `pub(crate)` and intentionally re-export only
   the protocol/gameplay API used by other concerns.
8. Re-run deterministic checks after each slice. After the architecture is stable, isolate the
   preset-1 failure, complete all twelve impairment runs, and resume visual/controller evidence.

Architecture proof required:

- module roots are composition and public-API surfaces rather than implementation containers;
- authoritative systems, client presentation, and process verification have distinct module
  ownership and remain correctly feature-gated;
- the attack, delivery, effect, telemetry, and evidence pipelines have named boundaries and no
  duplicated authoritative mutation path;
- network integration scenarios share one harness without one monolithic test source file;
- broad `too_many_lines`, `too_many_arguments`, and `type_complexity` module suppressions are
  removed or replaced by narrowly documented exceptions;
- deterministic behavior, schedule ordering, server authority, protocol compatibility, and
  client/server feature isolation remain proven by the existing and remediation-added tests.

Tracking:

- [x] Architecture analysis and target boundaries approved.
- [x] Deterministic pre-refactor baseline recorded, including the known preset-1 failure.
- [x] Network tests and harness decomposed without assertion changes.
- [x] Combat module and oversized systems consolidated.
- [x] Client, server, and movement roots decomposed by ownership.
- [x] Structural guardrails and feature-isolation checks pass.
- [x] Full deterministic and twelve-run impairment verification passes.

Pre-refactor baseline at commit `e19ee83` on 2026-08-14:

- client unit/target tests: 68 passed;
- server unit/target tests: 56 passed;
- deterministic network integration: 38 passed with one test thread;
- performance: 7 passed; the combined M05 case recorded 1.936 ms p95 and every recorded p95 was
  below 2 ms on the aarch64 macOS host;
- client, server, and network-test `cargo check --locked --all-targets`: passed;
- server feature isolation: passed;
- two-client headless network smoke: passed;
- format check: passed;
- role-specific Clippy: failed before reaching the server role because client compilation reported
  `filter_next` in `src/combat.rs` checkpoint handling and `map_unwrap_or` in `src/client.rs`
  headless automation;
- `BRAWLER_NETWORK_PROFILE_RUNS=1 BRAWLER_NETWORK_PROFILE_PRESETS=1
  BRAWLER_NETWORK_PROFILE_BASE_PORT=5800 ./scripts/network-combat-profiles.sh`: failed in the first
  local Pulse run after both clients completed but the server never completed its combat evidence
  assertion; the supervised launcher timed out after 60 seconds with an empty client-evidence
  directory. Repeated late-input mismatch diagnostics were present, but this baseline does not yet
  establish whether they are causal.

The two Clippy findings may be corrected mechanically during extraction. The Pulse process failure
remains a known behavioral/evidence defect to isolate after the behavior-preserving module moves;
do not infer a cause from the baseline diagnostics alone.

Post-remediation evidence on 2026-08-14:

- The former flat roots are directory-owned composition surfaces. `combat/mod.rs` is 482 lines and
  delegates model/cues, catalog resolution, authority, attack, delivery, effects, telemetry,
  evidence, server composition, and client presentation. `client/mod.rs` is 425 lines and delegates
  input, presentation, session/selection/automation, and tests. `movement/mod.rs` is 477 lines and
  delegates arena geometry, input shaping/validation, and tests. `server/mod.rs` delegates process
  verification and tests. The network entry point delegates to one shared harness and focused
  lifecycle, movement, selection, combat, and recovery scenarios.
- Large schedule systems were not merely relocated: attack delivery/telemetry recording, payload
  resolution/effect application, movement input shaping, arena geometry, process verification,
  evidence queueing, checkpoint normalization, and candidate retention now have named helpers and
  focused ownership. File-wide `too_many_lines`/`too_many_arguments` suppressions were removed;
  remaining exceptions are attached to specific Bevy systems or deterministic orchestration
  functions. Role-specific Clippy passes with warnings denied.
- The original Pulse failure was an evidence-lifecycle defect: clients exited at their nominal
  automation tick even when required combat evidence was incomplete. Exit is now gated by
  `ClientCombatEvidenceStatus`. Repeated transient checkpoint messages could also fill the bounded
  client queue and repeated latched snapshots could fill the server candidate history; per-kind
  client caps and distinct server candidates prevent durable defeat/reset starvation.
- Short-lived delivery/effect checkpoints now compare checkpoint-specific normalized gameplay
  schemas. Flight evidence retains exact attack/delivery identity, recipe/presentation identity,
  analytically reconstructed position, lob data, and deadline. Slow and knockback retain the exact
  named effect plus stable fighter build/configuration while excluding concurrently replicated
  damage, cadence, pose, and projectiles. Defeat retains terminal health/marker for defeated
  fighters; reset retains the neutral dummy's reset gameplay state. Empty `ActiveEffects` component
  presence is canonicalized without weakening populated-effect equality. Focused regressions prove
  altered projectile deadlines/fingerprints fail equality, queue and candidate histories cannot
  starve later checkpoints, and headless exit waits for evidence.
- `just lint` passed. `just test` passed with 73 client tests, 60 server tests, 38 deterministic
  network tests, and 7 performance tests. The combined 100-effect + 32-scatter + 16-lob + 32-blade
  case measured 1.995 ms p95 on the aarch64 macOS host; every recorded p95 remained below 2 ms.
  `just server-features` and `just network-smoke` passed.
- `BRAWLER_NETWORK_PROFILE_RUNS=1 BRAWLER_NETWORK_PROFILE_PRESETS='1 2 3 4'
  BRAWLER_NETWORK_PROFILE_BASE_PORT=6520 ./scripts/network-combat-profiles.sh` passed all twelve
  supervised runs. Each run had exact server/client cue counts and converged state evidence. Across
  the four presets, profile summaries reported server p95 22.606/23.047/23.313 seconds,
  fire-to-cue p95 18.411/17.929 ms local, 52.952/53.723 ms typical, and 80.739/93.404 ms adverse
  for clients 1/2. Summary state p95 was 20.972/19.154 ms local, 61.009/61.062 ms typical, and
  149.239/80.664 ms adverse. Per-run adverse state p95 peaked at 265.329 ms for client 2 on the arc
  launcher and still converged.

## 7. Finish milestone evidence and closeout

After the code changes:

1. Re-run format, role-specific checks, Clippy, unit, network, performance, feature-isolation, and all twelve impairment runs.
2. Update checked test-plan items only when the corresponding test exists.
3. Record that the pre-implementation M04 baseline cannot be reconstructed retroactively if no evidence exists; do not mark it passed without evidence.
4. Obtain explicit specification validation from the user.
5. Run and record keyboard/mouse checks at 30/60/high render profiles.
6. Run and record physical-controller selection, aiming, firing, pause, disconnect, and reconnect.
7. Move to `User playtest` and provide the documented scenarios.
8. Triage every feedback item.
9. Record learn-from-errors findings, particularly:

   - never hard-code evidence results;
   - do not mark broad test-plan items complete from a narrower smoke test;
   - keep metric names aligned with their units and semantics;
   - migrate compatibility behavior through adapters instead of retaining a second gameplay pipeline.

10. Only then mark all satisfied exit items complete and advance the roadmap status.

The minimum blockers before user playtest are items 1–4. Items 5–7 are also required before Milestone 5 can be marked `Complete`.
