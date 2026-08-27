# V12 Milestone 03 — combat sustain, ammunition recovery, and instant evidence capture

| Field | Value |
|---|---|
| Status | Complete |
| Depends on | Accepted V12 M01 maps and completed M02 Balance Lab correctness/presentation |
| User direction | Add idle-attack health recovery, continuous per-ammunition recovery, and a one-press keyboard/gamepad screenshot paired with the client-visible engine state |
| Outcome | Fighters recover health and ammunition through readable server-authoritative timing rules, while a player can preserve a transient visual defect and its matching client observation without opening a menu |

## Scope decision

This is the first V12 M03 gameplay-balancing slice. It changes two core sustain/economy mechanics
from playtest evidence and adds the client evidence tool needed to report short-lived defects during
later tuning. It does not yet perform the broader fighter, weapon, passive, or ability rebalance.

The implementation remains server authoritative. Health and ammunition change only on the match
worker. The screenshot and state file are client-only observations and cannot mutate or claim
authoritative state.

## Research and current implementation findings

Local sources reviewed on 2026-08-27:

- `src/timing.rs`, `src/gameplay.rs`, `src/combat/attack.rs`, `src/combat/model.rs`,
  `src/combat/server.rs`, and `src/combat/effects/application.rs` for the 60 Hz fixed simulation,
  accepted-attack boundary, exclusive weapon phase, and integer damage/health transaction;
- `src/builds/model.rs`, `src/builds/definitions.rs`, `content/catalogs/builds.ron`, and
  `content/catalogs/weapons.ron` for resolved fighter stats, recipe fingerprints, and current
  refill values;
- `src/client/input.rs`, `src/client/settings/mod.rs`, `src/client/mod.rs`, and the existing
  scheduled screenshot path for device sampling, persisted bindings, and automated capture;
- exact Bevy 0.19.1 screenshot implementation at
  `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_render-0.19.1/src/view/window/screenshot.rs`
  and the checked-in Bevy `examples/window/screenshot.rs`; the request entity is extracted for the
  current rendered frame and emits asynchronous `ScreenshotCaptured` with the image;
- the checked-in Lightyear book's `concepts/replication/protocol.md` and
  `concepts/advanced_replication/replication_logic.md`; `WeaponState` remains one replicated
  component so ammunition and both deadlines arrive as one consistent entity update;
- `docs/08-network-architecture.md`, which requires one global protocol-version increment for an
  incompatible registered-component shape change.

The current weapon state cannot express the requested mechanic. `WeaponPhase` is exclusively
`Ready`, `Cooldown`, or `Reloading`; only empty ammo starts reload, and completion restores the
entire capacity. Per-round recovery must coexist with fire cooldown and therefore needs independent
deadlines inside the same replicated state.

The client already supports frame-scheduled PNG capture for automated visual checks, but it has no
player input, default save location, completion feedback, or paired client-state record. Bevy's
capture is asynchronous, so the client must retain a serialized request-frame observation until
the captured image arrives rather than querying the World later.

## Accepted timing semantics

### Health recovery

- `health_recovery_rate` is expressed as **health points per second**. It is not authored per frame,
  tenth-second, or simulation tick, so a future simulation-frequency change does not rewrite
  balance content.
- `idle_attack_delay` is displayed in **seconds** and stored as an exact positive fixed-tick count.
- A server-accepted player attack sets the fighter's recovery inactivity origin to the current
  authoritative tick and clears fractional healing progress.
- Aiming, held fire rejected for cooldown or empty ammo, invalid attack attempts, taking damage,
  movement, and automated attacks by an owned sentry do not reset the delay. An accepted attack
  still resets the delay when its delivery immediately strikes cover.
- After the delay, the server adds `rate / 60` health each fixed tick through a deterministic
  fractional accumulator, publishes only integer `CurrentHealth`, and clamps to the resolved
  maximum. Recovery stops while defeated or outside active combat and resets on spawn, respawn,
  match restart, build replacement, and disconnect cleanup.
- Under this literal attack-idle rule, a fighter who has not attacked recently can continue
  recovering while taking damage. That is intentional unless later playtest feedback adds a
  separate damage-idle delay.

The accepted initial canonical values are `10` health per second and a `3.0` second idle-attack
delay for all three fighter profiles. They remain independent profile
fields in Balance Lab so later tuning can create a deliberate sustain tradeoff.

### Ammunition recovery

- `refill_ticks` and `recharge_ticks` mean the duration for **one** ammunition or charge. Balance
  Lab displays the value in seconds.
- The initial canonical duration is `1.3` seconds, exactly `78` fixed ticks at 60 Hz. All current
  built-in weapon bases start from that default; later playtesting may differentiate them.
- Whenever ammo is below capacity, exactly one continuous next-ammo timer exists. At its deadline,
  the server restores one ammo and starts the following interval if another slot remains empty.
- The match worker is the sole owner of ammunition count, timer creation, interval progress,
  restoration, and correction. Clients never increment ammunition or decide that an interval has
  completed.
- **Firing never resets or delays an active ammo timer.** If the next ammo is 90% ready, firing one
  stocked round leaves it 90% ready and progressing. The newly empty slot waits behind that
  in-progress round.
- Firing from full capacity creates the first empty slot and starts the timer. Reaching capacity
  clears it. Fire cooldown and ammo recovery advance independently.
- At a tick where a round completes and fire is held, recovery advances before fire validation, as
  it does for the existing deadline boundary. That restored round may therefore be consumed on the
  same authoritative tick.
- The recipe validator requires a positive duration. The expected tuning range is about `0.1` to
  `3.0` seconds, but this is a useful Balance Lab control range rather than a hardcoded gameplay
  rejection ceiling; only representation and safe deadline conversion remain engine constraints.
- Quick Cycle continues to shorten one next ammunition interval by 40%. A prime never rewrites an
  interval already in progress; it is consumed when the next interval actually starts.

## Technical specification

### 1. Fighter recovery definition and runtime ownership

Extend each authored/resolved fighter profile with health-recovery rate and idle-attack delay. Bump
the build catalog schema, build fingerprint format, resolved match snapshot schema, gameplay
content fingerprint expectations, and global application protocol version together.

Add one server-only runtime component owned by combat lifecycle with:

- last accepted player-attack tick;
- sub-health recovery remainder;
- enough definition identity/revision context to reset cleanly when the resolved loadout changes.

The component is initialized beside `CurrentHealth`/`WeaponState`, reset by the shared match
lifecycle, and never replicated. A focused recovery system runs in `FixedUpdate` after lifecycle
and before authoritative fire. Accepted attacks update it in the same transaction that consumes
ammo and reserves attack identity. The system reads the immutable resolved loadout and mutates only
`CurrentHealth` plus its own remainder.

Use integer fixed-point accumulation rather than repeated floating-point health mutation. The
authored/UI rate may be decimal, but resolution converts it to one canonical bounded unit so tests
can prove exact totals across fixed ticks without frame-time dependence.

### 2. Independent weapon deadlines

Replace the exclusive `WeaponPhase` runtime contract with one atomic replicated `WeaponState` that
contains:

- current ammunition;
- next tick at which firing is permitted;
- optional start and target ticks for the next single ammunition recovery.

Readiness/reloading labels become derived presentation facts, not authority variants. Server fire
requires `ammo > 0` and `tick >= fire_ready_at_tick`. The state-advance helper restores every due
round in deterministic deadline order, bounded by the existing maximum capacity, and carries the
deadline forward from its previous value so a delayed update cannot accumulate timing drift.

Update initialization, respawn/restart, build replacement, bots, HUD, audio, combat evidence,
Balance Lab reset, and network fixtures together. Bots consume a derived `weapon_fire_ready`
observation and may fire stocked ammunition while another round is recovering. The HUD continues
to show discrete ammo segments and adds a filling segment/countdown for the single next round
without mislabeling fire cooldown as reload. Reload audio plays on an observed ammo increment, not
merely while a timer exists.

The replicated recovery interval carries both `started_at_tick` and `ready_at_tick`, rather than
asking the client to infer the start from the recipe. This remains correct when Quick Cycle shortens
one interval or a future authoritative modifier changes its duration. Together with the replicated
`AuthoritativeTick`, the client has sufficient information to calculate clamped progress:

```text
progress = (observed_tick - started_at_tick) / (ready_at_tick - started_at_tick)
```

Presentation may advance that ratio smoothly between received authoritative ticks using local
elapsed time anchored to the most recent observation, but it must clamp to `[0, 1]`, reconcile to
every newer server state, and never change `WeaponState` or grant ammunition locally. Reaching 100%
means the client estimates that the authoritative target tick has arrived; it does not wait for a
round-trip replication acknowledgement before showing the full segment or forwarding fire intent.
The server advances due ammo before validating that intent and accepts or rejects the shot from its
own current tick/state. A later replicated state reconciles the display without adding an artificial
network-latency delay to firing.

### 3. Balance Lab maintenance

M02's server-owned editor manifest gains:

- recovery rate and idle-attack delay for each fighter profile;
- one-ammunition refill/recharge duration for every weapon recipe and custom Pulse magazine;
- seconds/health-per-second units, authoritative positive minima, useful slider ranges, exact
  inputs beyond those ranges, canonical/applied comparison, copy-difference output, and inline
  validation.

The snapshot schema advances because these are new fighter fields and changed weapon-economy
semantics. Persistence envelope 4 migrates its prior snapshot by filling the new recovery fields
from canonical content while retaining existing tuning. Restore Defaults and Apply & Reset retain
their current transaction contract; this milestone does not introduce live application.

### 4. Instant client evidence capture

Add a client-only `EvidenceCapturePlugin` to every windowed client. It is absent from headless and
server feature graphs.

Default bindings are `F12` on keyboard and the unused north face button on gamepad. Both are
rebindable through the existing settings model. A just-pressed edge requests capture directly
during gameplay, including while moving, aiming, or firing; it opens no menu and consumes no
network/gameplay input bit.

On request, a `PostUpdate` system builds a bounded `ClientEvidenceCaptureV1` from the state that the
client can actually observe, attaches that already-serialized payload to Bevy's screenshot request
entity, and lets the current frame be extracted. The asynchronous capture observer writes one pair
with the same collision-resistant basename:

```text
PewPew Blitz/Captures/brawler-UNIX-MILLIS-NNN.png
PewPew Blitz/Captures/brawler-UNIX-MILLIS-NNN.json
```

The default directory is the platform Pictures directory supplied by the existing `directories`
dependency, falling back to the app data directory when Pictures is unavailable. An explicit client
`BRAWLER_CAPTURE_DIR` override supports automation and troubleshooting. The existing scheduled screenshot
CLI remains compatible and may reuse the low-level PNG writer without acquiring player-state
capture semantics.

The JSON record contains only bounded, diagnostic observations:

- schema/capture identity, wall time, app build, protocol/content fingerprints, client identity,
  transport and join phase;
- window size/scale, authoritative tick, camera transform and projection values;
- selected map identity, playable bounds, dynamic generation, match phase/clock, scores and mode
  objective state;
- the controlled fighter's pending local input/device context and replicated position, facing,
  health, weapon deadlines/ammo, loadout, ability, effects, concealment/reveal, defeat/protection,
  and participant state;
- the same core replicated gameplay fields for every currently client-visible fighter, projectile,
  sentry, damageable object, and pickup using stable IDs where available.

It does not serialize the entire Bevy World, asset contents, connection secrets, raw socket data,
or arbitrary process-local component memory. Collections are deterministically sorted and bounded;
truncation is explicit in the JSON. Image encoding and both file writes run on Bevy's I/O task pool
after GPU readback so the input path does not synchronously encode a PNG. A small non-modal HUD
toast reports the saved absolute basename or a write failure without pausing gameplay.

The serialized request state and rendered image share the same main-world frame boundary as closely
as Bevy's extraction model permits. The JSON records both the client-observed authoritative tick
and local frame/update identity; it must not claim that interpolated presentation equals current
server authority.

## Schedule and authority composition

```text
FixedUpdate (server)
  Lifecycle/reset
  -> health recovery
  -> input/freshness
  -> movement/simulation
  -> ammo deadline advance + authoritative fire
  -> deferred commands

FixedPostUpdate (server)
  projectile/effect damage
  -> lifecycle/outcomes/cues
  -> publish authoritative tick

Update/PostUpdate (windowed client)
  receive replication -> sample device/gameplay input -> update presentation/camera
  -> just-pressed evidence request + client observation snapshot
  -> Bevy render extraction/GPU screenshot
  -> async paired PNG/JSON write -> non-modal completion/failure toast
```

Damage remains after the FixedUpdate recovery pass under the existing combat transaction. A fighter
can recover and then take damage in one tick; defeat evaluates the final ordered integer health.

## Implementation checklist

- [x] Add fighter recovery definitions, schema/fingerprint/version changes, and Balance Lab fields.
- [x] Add/reset server-only recovery timing and deterministic fractional accumulation.
- [x] Reset inactivity only from accepted player-owned attacks and cover literal non-reset cases.
- [x] Replace exclusive weapon phases with independent fire/refill deadlines.
- [x] Implement continuous one-at-a-time ammo recovery without fire-progress reset.
- [x] Replicate authoritative ammo interval start/target ticks and render a client-only filling bar.
- [x] Reconcile Quick Cycle, bots, HUD, audio, lifecycle, evidence, and Balance Lab behavior.
- [x] Add rebindable keyboard/gamepad capture actions and windowed-only plugin composition.
- [x] Add bounded deterministic client snapshot construction and paired async PNG/JSON saving.
- [x] Add non-modal success/failure feedback and default/override directory documentation.
- [x] Run focused, canonical, routed, recovery/late-join, and performance checks.
- [x] Complete the native combat/capture playtest.
- [x] Record user playtest feedback and complete the learning review.

## Verification plan

Focused server/ECS tests prove:

- no health before the exact idle boundary, exact accumulated health after it, maximum clamp, and
  no recovery while defeated/inactive;
- accepted attack resets delay/remainder, while aim, rejected/dry fire, damage, movement, and sentry
  fire do not;
- respawn, restart, disconnect, build apply/reset, and late join converge;
- firing from full starts one timer; every deadline restores exactly one; partial progress survives
  later shots; queued empty slots refill sequentially; refill and held fire share the documented
  same-tick order; capacity is never exceeded;
- fire cooldown and refill progress coexist; Quick Cycle affects the next not-yet-started interval;
- bot decisions use fire readiness rather than the removed phase enum.

Protocol/network tests prove one authoritative state on both clients during cooldown plus refill,
partial-ammo late join, recovery, defeat/respawn, packet loss/recovery, and forged-client attempts.
They also prove that clients cannot grant ammo, shorten an interval, or fabricate a completion, and
that a late join receives enough interval information to render the same bounded progress.
Balance Lab tests cover descriptors, conversion, canonical diffs, validation, persistence, Apply &
Reset, and restored defaults. Performance gates cover six continuously recovering fighters and
maximum-capacity refill catch-up without unbounded per-tick work.

Capture tests prove keyboard/gamepad edge detection, headless/server exclusion, stable sorting and
bounds, secret omission, unique paired paths, asynchronous success/failure handling, and existing
scheduled-screenshot compatibility. A native playtest captures while moving/firing, opens the PNG
and JSON, correlates tick/frame/map/fighter data with the image, verifies no perceptible menu/pause,
and checks the output toast and directory.

## Verification evidence

Automated verification passed on 2026-08-27:

- `just check` passed the routing, client, server, network, Balance Lab web, role-isolation, and
  retained-renderer contract checks;
- `just lint` passed for routing plus the client, server, and Balance Lab feature graphs with
  warnings denied;
- the client suite passed 415 tests, the server suite passed 329 tests, and the Balance Lab Rust
  suite passed 346 tests plus its routed build-handoff integration test;
- the complete routed network matrix passed 90 tests, including health recovery, partial-ammo
  late join, firing without ammo-progress reset, impairment/recovery, and cadence coverage;
- the performance suite passed all 12 gates, including the combined worst-case and
  fighter/projectile capacity cases below the 16.67 ms fixed-tick budget;
- the Balance Lab web suite passed all 10 tests and its production build.

The final version-closeout rerun included the accumulated projectile-geometry and balance changes:

- the current client, server, and Balance Lab feature suites passed `425`, `331`, and `348` tests
  respectively, plus the focused routed Balance Lab handoff;
- all `90` routed network scenarios passed after mechanics-focused fixtures stopped assuming the
  former fighter health, Pulse damage/range, Scatter capacity, and seven-projectile count; and
- all `12` performance gates passed, including the current 32-attack Scatter load of `160`
  projectiles at a `3.064167 ms` p95, below the `16.67 ms` fixed-tick budget.

The native GPU/file-I/O path was exercised through the controller-triggered playtest. That test
exposed the routed-identity JSON defect below; the corrected capture was accepted through the
user's subsequent commit request. Subjective projectile readability was separately accepted after
the rendered body and obstruction-aware aim preview were matched to authoritative geometry.

## Playtest and exit criteria

- Recovery timing feels legible on all three fighter profiles and never depends on render rate.
- The user can fire repeatedly while visibly preserving current next-ammo progress, and each round
  returns independently at its weapon's configured recovery duration.
- The filling ammo segment is derived from replicated authoritative start/target ticks, remains
  smooth between updates, never mutates ammo locally, and does not delay fire input while awaiting
  replication after the authoritative target tick.
- HUD, audio, bots, reconnect, and reset behavior agree with the new independent deadlines.
- Balance Lab exposes every new balance value in player-facing units without inventing a balance
  ceiling.
- One keyboard or gamepad press during active play produces a matching PNG/JSON pair without a menu
  or gameplay command, and the pair contains enough bounded client-visible context to investigate a
  transient defect.
- Server authority, current-protocol exact compatibility, role isolation, and existing automated
  screenshot workflows remain intact.

## Accepted specification decisions

1. The initial implementation used `10 health/second` after `3.0 seconds` without an accepted
   player attack. Final playtest balancing keeps the `3.0`-second delay and raises the Default
   profile to `100 health/second`; Lightweight and Reinforced remain at `10 health/second`.
2. Default capture bindings are `F12` and the north face button. Both remain rebindable.

The ammunition-progress rule is already accepted: firing does not reset an active timer.

## User playtest feedback

### Controller capture crashed on routed match identity

Status: **implemented and accepted**.

Pressing the north face button reached the intended capture action, but snapshot construction
panicked before GPU capture. Routed `MatchId` values occupy the complete `u128` space, while
`serde_json::json!` internally unwraps an error when a directly serialized integer exceeds JSON's
supported integer representation. The first failure was the match-state record; the same latent
failure also existed in match clocks, Hot Zone and Heist state, fighter participation, sentry
identity, and Heist objective identity.

Capture now writes every client-visible `MatchId` as a decimal string. This preserves the exact
identity without changing the gameplay/wire representation and avoids precision loss in ordinary
JSON consumers. Capture wall time is narrowed safely to `u64` milliseconds. A regression test uses
`u128::MAX`, checks every affected capture projection, and verifies the resulting snapshot can be
pretty-serialized. The focused regression, complete client suite, and warning-denied client
Clippy pass succeeded on 2026-08-27. The user accepted the correction and requested commit
`d1a7d28` after the controller-triggered native failure was removed.

### Projectile body, rendering, and aim-preview agreement

Status: **implemented and accepted**.

Playtest review found that a fixed four-unit rendered projectile and center-line aim trace could
disagree with the configured authoritative projectile radius. A shot could therefore appear to
clear a wall edge while its collision body struck it. Straight projectiles now replicate their
resolved circular `ProjectileBody` geometry, authoritative sweep/collision consumes the same
radius, presentation sizes the projectile from it, and aim preview performs a radius-aware sweep
against blocking map geometry. The user confirmed that the result works. The body component is
deliberately a typed geometry boundary so later projectile shapes can add explicit variants without
returning to unrelated collision, presentation, and preview constants.

### Final canonical balance pass

Status: **implemented**.

The final playtest values supersede the milestone's initial defaults where listed:

- Default fighter: `1000` maximum health, `70` world units/second movement, and `100`
  health/second recovery after the unchanged `3.0`-second attack-idle delay;
- Pulse Sidearm: four rounds, `1.0` second per round, `500` world units/second projectile speed,
  `2`-unit projectile radius, `320`-unit range, and `200` damage;
- Scatter Cannon: three rounds, `1.2` seconds per round, five projectiles, `600` world
  units/second projectile speed, `2`-unit projectile radius, `320`-unit range, and `120` damage per
  projectile;
- Arc Launcher: `1.6` seconds per round; and
- Impact Blade: `1.0` second per charge.

Lightweight and Reinforced retain their separately authored profiles. The build catalog revision
advanced with the fighter-default change, and balance-dependent fixtures now distinguish authored
default assertions from test-local combat setup.

## Closeout and learning review

V12 M03 is complete. Its required sustain, ammunition, capture, Balance Lab maintenance,
projectile-readability feedback, and final balance changes are integrated without moving gameplay
authority to the client.

The main mistakes and reusable lessons were:

- Opaque 128-bit gameplay identities cannot be projected as ordinary JSON numbers. Diagnostic
  formats must serialize them losslessly as strings and test the full representation boundary,
  including `u128::MAX`.
- Collision, rendering, and aim preview must consume one projectile-body fact. Independently tuned
  constants produce frustrating edge disagreement even when each subsystem is internally correct.
- A projectile body's shape is a gameplay contract, not merely a radius field in a renderer.
  Future shapes require explicit collision, replication, visualization, and preview support before
  they become valid authored content.
- Balance-dependent tests must use current authored defaults only when that is the behavior under
  test. Timing, range, or late-join fixtures should own explicit local values when they are proving
  mechanics rather than catalog balance.
- A fighter-default change must advance the balance revision so persisted selections, resolved
  snapshots, and operator comparisons cannot silently retain stale meaning.
