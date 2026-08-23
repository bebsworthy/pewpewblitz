# V9 Milestone 02 — Self cloak and reveal scan

## Status

**User playtest.** M01 completed and was accepted on 2026-08-23. M02 research and this
specification were prepared on 2026-08-23, the user approved implementation on 2026-08-23, and
the implemented slice passed its canonical automated gates on 2026-08-23. The first native
playtest validated terrain reveal and identified targeted-input and reveal-signifier corrections
that were implemented and passed affected canonical verification on 2026-08-23.

## Player-visible outcome

Two new selectable ultimates form one complete stealth/counter pair:

- **Self Cloak** conceals its owner from every enemy for a bounded duration. Enemy proximity does
  not reveal it. The first accepted attack or positive applied damage consumes the current
  cloak permanently and starts the ordinary attack/damage reveal lock.
- **Reveal Scan** instantly targets a bounded area and reveals every hostile fighter inside it to
  the caster's whole team for a lingering duration, including fighters hidden by terrain or Self
  Cloak. The scan has no warning, dodge window, or cleanse after acceptance.

The owner and allies retain the accepted M01 alpha signifier while Self Cloak is effective. A scan
creates a public activation footprint, and legally visible affected fighters receive a durable
ground-ring treatment. Hidden live spatial state remains absent from unauthorized client Worlds.

## Scope

### In scope

- two stable ultimate definitions, saved-brawler selection, build resolution, immutable match
  handoff, charge, activation, HUD, presentation, audio, telemetry, recovery, and lifecycle;
- one fighter-owned Self Cloak source and one instant targeted Reveal Scan transaction;
- team-keyed lingering forced reveal, exact deadline/refresh behavior, and composition with M01
  terrain, proximity, attack, and damage rules;
- the M01 cue-order correction required for same-tick reveal and presentation facts;
- targeted local preview for Reveal Scan and post-acceptance public scan/revealed visuals;
- schema, fingerprint, profile snapshot, Balance Lab, process evidence, network security, routed,
  capacity, and native playtest coverage affected by the new inventory and runtime states.

### Out of scope

- the allied concealment field, which remains M03;
- reveal cleanse, immunity, delayed detonation, warning telegraph, counter-counter, target memory,
  line-of-sight, walls blocking a scan, or team-shared proximity reveal;
- generic status/effect scripting, generic runtime areas, arbitrary client-authored radii/ranges,
  ability prediction, or client-authoritative target acceptance;
- changing M01 attack/damage durations, reveal-proximity values, or the accepted 52% alpha value
  unless playtest exposes a direct interaction defect;
- final balance claims for the provisional M02 numbers.

## Research findings

### Can the existing input carry a targeted instant ultimate?

Yes. `FighterInput` already carries a quantized `aim_update` and an optional whole-world-unit
`aim_distance`. Mouse input supplies cursor direction/distance; controller input supplies bounded
stick direction/distance; the server already treats missing distance as maximum range for lobbed
targeting. Lightyear's native `ActionState<FighterInput>` is resolved at fixed tick and the current
activation latch already turns a held ultimate button into one attempt.

Ability activation currently precedes authoritative movement/facing mutation in `FixedUpdate`, so
Reveal Scan must resolve the current input's `aim_update` through the same server `committed_aim`
rule and fall back to the existing authoritative `Rotation` only when no new aim commits. Reading
`Rotation` alone would aim one tick behind a simultaneous mouse/stick update.

Reveal Scan therefore needs no new input message or device-specific server path. On the accepted
edge, authority computes:

```text
direction = committed_aim(aim_update).unwrap_or(authoritative_facing)
requested_distance = aim_distance.unwrap_or(max_range).clamp(0, max_range)
desired_center = fighter_position + direction * requested_distance
accepted_center = desired_center clamped to PlayableBounds
```

The scan circle may extend outside playable bounds; targets exist only inside the arena, so clamping
the center is sufficient and keeps cursor/controller behavior consistent. Non-finite pose/facing,
stale or invalid input, inactive/defeated lifecycle, missing charge, or wrong ability kind rejects
without consuming charge or creating a cue.

### How should the new balance data enter an immutable loadout?

The current `UltimateDefinition` carries identity, kind, cost, and display metadata; Dash and
Sentry use code-owned constants. M02 needs authored duration/range/radius values in the immutable
match handoff without refactoring accepted Dash/Sentry behavior.

Add one closed `UltimateParameters` enum to authored and resolved ultimate data:

```text
UltimateParameters
  Dash
  Sentry
  SelfCloak { duration_ticks }
  RevealScan { maximum_range_milliunits, radius_milliunits, reveal_ticks }
```

Validation requires the parameter variant to match `UltimateKind`, rejects zero/out-of-engine-bound
values, and converts integer milliunits to finite `f32` only at the authoritative geometry boundary.
Existing Dash/Sentry constants and behavior remain unchanged in M02. The build catalog, canonical
fingerprint, immutable match snapshot, global protocol compatibility, profile backup snapshot, and
Balance Lab snapshot advance through their existing fail-closed paths. SQLite profile storage needs
no table migration because it already stores a bounded integer `ultimate_id`; validation and backup
schema still change because IDs 3 and 4 become legal.

### Should Self Cloak be a generic status effect?

No. Self Cloak is the selected ultimate's own runtime phase and has source-specific break behavior.
Model it as:

```text
AbilityPhase::Cloaked { generation, activated_at_tick, expires_at_tick }
```

Activation spends all charge and installs that phase. Generation is a bounded monotonically
increasing per-fighter activation identity used by cues and telemetry. Expiry or consumption returns
the ability to `Charging` at zero charge. This keeps authored definition, immutable loadout, mutable
ability state, and observer-derived visibility distinct without introducing a general effect graph.

Activating while an existing attack/damage or forced-reveal deadline is active remains legal but
the cloak is suppressed until that reveal expires. The HUD must communicate both timers so the
player understands the tradeoff; authority does not silently refund or extend the cloak.

### How should scan reveal be keyed and cleaned up?

A scan is a one-tick bounded selection followed by lingering subject-owned records:

```text
ForcedRevealSource
  revealing_team
  source_network_id
  source_generation
  applied_at_tick
  expires_at_tick

ForcedRevealSources
  sorted bounded Vec<ForcedRevealSource>
```

One source refreshes its own record to the latest deadline. Multiple casters on one team coexist;
the effective team deadline is their maximum. Records are sorted by team/source identity and capped
by admitted active-fighter capacity, not an arbitrary unbounded collection. Expired records and
records whose source is disconnected, replaced, or no longer in the match are removed. Caster
defeat does not cancel an already accepted scan: that would create post-acceptance counterplay.
Leaving the scanned area does not remove a record. Subject respawn, restart, map/match replacement,
and shutdown clear records according to their owned lifecycle.

This representation preserves the durable team-keyed rule and makes source-owner cleanup exact. It
also avoids the incorrect behavior where removing the latest of two scans would erase an earlier
still-active scan or resurrect one that was never retained.

### What observer rule composes proximity and non-proximity concealment?

Replace the M01 terrain-shaped boolean helper with one source-neutral pure decision:

```text
if self or ally                          => visible
else if observer is not alive           => hidden when any concealment is active
else if forced reveal for observer team => visible
else if attack/damage lock active       => visible
else if Self Cloak active               => hidden (ignore proximity)
else if terrain concealment active      => visible only within observer reveal radius
else                                    => visible
```

M03 may later add allied-area membership to the same proximity-concealment input without changing
Self Cloak semantics. M02 does not prebuild the field or a general source registry.

### What schedule correction is required before ability cues?

The M01 implementation safely derives Lightyear gain/loss in `PostUpdate`, before
`ReplicationSystems::Send`, but `send_combat_cues` runs earlier in `FixedPostUpdate`. A same-tick
attack reveal therefore filters cues against the prior cached observer decision. This fails closed
and does not leak state, but it can suppress a legal same-tick reveal cue.

M02 splits decision from network mutation:

1. fixed-tick lifecycle and ability activation become current;
2. movement, attacks, delivery, and positive damage complete;
3. Self Cloak break/expiry, attack/damage locks, forced reveals, and terrain membership resolve;
4. observer decisions and transition reasons update the cache in `FixedPostUpdate`;
5. ability/combat cues filter from that completed cache;
6. `PostUpdate` applies only queued `gain_visibility`/`lose_visibility` commands and an explicit
   `ApplyDeferred` before `ReplicationSystems::Send`.

Schedule tests must prove this order. Cue policy remains deny-by-default and Rust-exhaustive.

### Can existing presentation support the pair without a second renderer?

Yes. M01 already clones/caches imported and primitive fighter materials and restores exact source
handles. Extend its source-neutral active-concealment test so local/allied Self Cloak uses the same
accepted alpha treatment. A proximity-revealed enemy remains opaque.

Reveal Scan needs two additional optional client presentation families inside the existing 3D
renderer:

- a local-only targeting line and bounded ring while Reveal Scan is charged/ready, using current
  aim distance and authoritative map bounds; it is never replicated as an enemy warning;
- a public active-area ring at scan center for the full reveal duration plus a durable world ring on
  legally visible affected fighters. Shape and duration preserve meaning under reduced effects and primitive fallback rather
  than relying only on color.

No custom shader or new bitmap asset is required. Existing Bevy meshes, `StandardMaterial`, screen
projection, cue deduplication, and audio paths are sufficient.

## Alternatives rejected

- **Replicate an `Invisible` boolean:** cannot express different observers and would expose hidden
  entities to unauthorized clients.
- **Let proximity reveal Self Cloak:** contradicts the accepted product rule and removes Reveal
  Scan's distinct counter role.
- **Create a persistent scan-area entity:** invents area membership and teardown for an effect that
  is explicitly instant; the public footprint is a transient accepted cue.
- **Store one global forced-reveal deadline:** incorrectly shares one team's scan with every team and
  cannot clean up multiple sources correctly.
- **Send a client target point:** duplicates information already represented by fixed-tick aim
  direction/distance and widens the input trust surface.
- **Put all ultimate tuning into arbitrary maps:** loses exhaustive validation and permits unsupported
  fields; the closed parameter enum is smaller and fail-closed.
- **Refactor Dash and Sentry to fully authored tuning in M02:** useful future work, but not required
  to deliver this pair and would expand regression scope without player-visible M02 value.
- **Add a new renderer or custom cloak shader:** unnecessary after the accepted M01 material path.

## Initial balance inputs

These values are implementation/playtest hypotheses, not final balance claims:

| Ability | Cost | Range | Radius | Duration |
|---|---:|---:|---:|---:|
| Self Cloak | 4 points | Self | — | 360 ticks / 6.0 s |
| Reveal Scan | 4 points | 640 world units | 192 world units | 300 ticks / 5.0 s forced reveal |

Both consume the existing full 1,000 charge. Reveal Scan is instant and immediately returns to
`Charging`; Self Cloak remains in `Cloaked` until expiry or permanent break. Existing charge gain,
waste, defeat survival, and restart reset rules remain unchanged.

Add two ordinary preset recipes without changing existing IDs or recipes:

| Preset | Weapon | Ultimate | Passives | Cost | Pattern |
|---|---|---|---|---:|---|
| Infiltrator (ID 5) | Pulse Sidearm | Self Cloak | Close Quarters, Tenacity | 11 | choose an unseen approach, then commit once |
| Tracker (ID 6) | Scatter Cannon | Reveal Scan | Quick Cycle, Tenacity | 12 | expose a clustered concealment play and punish it |

The saved-brawler editor and pre-match custom editor must enumerate catalog entries rather than
retaining hard-coded two-ultimate/four-preset modulo arithmetic.

## ECS ownership and module composition

The second real concealment source justifies decomposing the M01 file while preserving its public
paths:

```text
src/concealment/
  mod.rs          plugin composition, sets, and cohesive source/observer orchestration
  model.rs        terrain, cloak/forced-reveal, presentation, pure decision shapes
  network.rs      queued per-link gain/loss and visibility cache API
  telemetry.rs    bounded transition/source/scan aggregates
  tests.rs        pure and small-App schedule/lifecycle tests

src/abilities/
  self_cloak.rs   activation, generation, break/expiry, ability telemetry/cues
  reveal_scan.rs  target resolution, bounded hostile selection, source application
```

`abilities` owns activation and mutable ultimate phase. `concealment` owns how active sources and
reveal records produce observer visibility. `combat` owns the ordered presentation cue channel and
attack/damage outcomes. Client presentation remains under `client/presentation_3d` and never mutates
authority state.

### Fixed-tick composition

```text
FixedUpdate
  GameplaySet::Input
  AbilitySet::Activation
    activate Self Cloak / accept Reveal Scan / ApplyDeferred
  AbilitySet::Movement
  GameplaySet::Simulation and CombatSet fire/delivery

FixedPostUpdate
  CombatSet::Damage
  AbilitySet::ObserveOutcomes
    consume/expire Self Cloak; prune/apply forced-reveal sources
  ConcealmentSet::ResolveSources
    terrain membership; attack/damage locks; presentation facts
  ConcealmentSet::DecideObservers
    complete pair cache and queued transitions
  CombatSet::TelemetryAndCues
    exhaustive per-link filtering and send
  match outcomes / finalize

PostUpdate
  apply queued Lightyear visibility changes
  ApplyDeferred
  ReplicationSystems::Send
```

Actual `.chain()`, `.after()`, `.before()`, system sets, and deferred boundaries remain visible at
composition points and receive initialization/order tests.

## Network and cue contract

Extend the existing ordered `CombatCue` family rather than create a parallel unordered channel:

- `SelfCloakActivated { source, generation, expires_at_tick }` — subject-filtered;
- `SelfCloakEnded { source, generation, reason }` — subject-filtered;
- `RevealScanActivated { revealing_team, center, radius_milliunits, tick }` — public footprint with
  no caster position or subject list;
- `ForcedRevealApplied { target, revealing_team, source_generation, expires_at_tick }` — target
  filtered after the completed observer decision.

Every variant receives stable event identity, codec/evidence coverage, client deduplication,
telemetry classification, audio/presentation handling, and an explicit privacy classification.
The public scan cue contains only its accepted gameplay footprint. It does not contain the caster's
position, hidden targets, rejected target intent, or target count.

`ConcealmentPresentationState` grows only durable facts needed by a client already permitted to
hold that fighter: active terrain/Self Cloak state, effective attack/damage reveal deadline, and
bounded team-keyed forced-reveal deadlines. It never grants replication relevance itself.

Protocol and content changes use the one global compatibility handshake. No per-message versions,
compatibility decoder, retained hidden entity, or parallel direct-UDP rule is introduced.

## Lifecycle and recovery

- Defeat consumes Self Cloak. It does not cancel already accepted scans cast by that fighter; the
  defeated subject's own forced-reveal records clear at the respawn lifecycle boundary.
- Respawn starts with ordinary `Charging` state and no cloak/reveal source while preserving the
  existing charge-survives-defeat rule only when no accepted activation already spent it.
- Build replacement in Waiting clears ability phase, generation runtime, and source records before
  the new loadout is installed.
- Match restart resets charge, phase, generations, forced reveals, caches, pending transitions, and
  transient cues while retaining the selected recipe as the existing flow requires.
- Disconnect/source removal prunes that source's scan records from every bounded subject list;
  reconnect receives only current authoritative loadout, ability, concealment, and reveal state.
- Late join and recovery never replay expired scans or hidden movement; a permitted client receives
  current component state and future cues only.
- Match/map teardown removes all M02 runtime state without depending on client presentation cleanup.

## Product surfaces and compatibility work

- Add stable ultimate IDs 3/4 and preset IDs 5/6; preserve IDs and recipes 1–4.
- Advance build catalog/fingerprint and build/protocol compatibility revisions.
- Update profile validation, backup snapshot schema, routed match snapshot fixtures, saved-brawler
  creation/edit cycling, pre-match build editor option counts, Dashboard labels, and recovery tests.
- Expose the new definitions and provisional tuning in Balance Lab snapshots/editor validation;
  advance its snapshot schema and persisted compatibility contract.
- Update HUD phase copy (`CLOAKED` and remaining seconds), build descriptions, process
  evidence summaries, headless automation selection, and canonical scenario fixtures.
- Do not migrate the SQLite table schema solely for the newly legal IDs; prove existing profiles and
  backups continue to load or fail closed according to their owned version contracts.

## Telemetry and bounds

Add bounded records/aggregates for cloak attempts, accepts, rejected reasons, active ticks, natural
expiry, attack break, damage break, suppressed-by-reveal ticks, scan attempts/accepts, target count,
hidden targets revealed, visible targets marked, refreshes, forced-reveal active ticks, and
time-to-hostile-damage after reveal.

Engine ceilings:

- scan targets are capped by resolved active-fighter capacity;
- forced-reveal source records per subject are capped by admitted active fighters;
- at most one live Self Cloak generation exists per fighter;
- cue fan-out is bounded by links × accepted scan targets plus one public active-area ring;
- source cleanup and observer decisions use stable sorted vectors bounded by match capacity;
- telemetry retains the existing bounded-record/drop-count pattern and never records secret
  positions in ordinary player-visible output.

## Implementation checklist

- [x] Rebaseline M01 and add schedule evidence for decision-before-cue filtering.
- [x] Decompose `concealment.rs` by demonstrated source/network/telemetry ownership while preserving
  public paths and role gates.
- [x] Add closed ultimate parameters, IDs 3/4, presets 5/6, validation, resolution, fingerprints,
  immutable snapshots, profile/Balance Lab compatibility, and catalog-driven editor enumeration.
- [x] Add `AbilityPhase::Cloaked`, generation ownership, activation, expiry, attack/damage break,
  lifecycle reset, cues, and telemetry.
- [x] Add authoritative scan targeting, bounds, stable hostile selection, team/source reveal records,
  refresh/prune/cleanup, cues, and telemetry.
- [x] Extend the pure observer decision and transition reason model for forced reveal, global locks,
  Self Cloak, terrain proximity, and future M03 proximity-source input.
- [x] Move pair-decision derivation before cue filtering and retain deferred Lightyear mutation before
  replication send.
- [x] Extend cue privacy classification, client dedupe, audio, HUD, world presentation, alpha
  treatment, scan preview/active area, revealed marker, and cleanup.
- [ ] Add pure, ECS, protocol, profile, Balance Lab, separate-App, routed, impairment, recovery,
  capacity, and native evidence specified below.
- [ ] Triage playtest feedback, rerun affected gates, complete the learning review, and obtain user
  acceptance before M03 research begins.

## Automated verification evidence

Passed on 2026-08-23 against the implemented tree:

- `just check`: every independently buildable client, server, network-test, routing, and Balance
  Lab role graph compiled;
- `just lint`: formatting, Balance Lab web build, all-role Clippy, dedicated-server feature
  isolation, sole V3 renderer, and V8 legacy-map cleanup passed;
- `just test`: routing, client, server, Balance Lab, 80 serialized network scenarios, and all 11
  fixed-tick/performance gates passed;
- focused authority/security coverage activates preset 5 Self Cloak and preset 6 Reveal Scan
  through real fixed-tick client input, proves zero-distance cloak absence, team reveal, exact
  lingering source state, expiry re-concealment, and client-World absence;
- routed `just e2e 2`, `just e2e 4`, and `just e2e 6` reached Active with exact 1v1, 2v2, and 3v3
  rosters. The six-client rotation now exercises every authored preset, including Infiltrator and
  Tracker;
- the corrected decision-before-cue schedule has an explicit schedule test. A regression found by
  the full suite also proves that public non-player combat targets remain outside the secret
  player-fighter cue filter;
- after first-playtest feedback, focused client tests prove Reveal Scan requires an Ultimate-button
  arming edge followed by a fresh Fire edge, suppresses simultaneous/held primary fire, cancels
  without intent, and leaves immediate ultimates unchanged. `just lint` and the complete `just
  test` matrix passed again: 355 client, 273 server, 283 Balance Lab, 80 serialized network, and 11
  performance tests, in addition to routing and focused revised-catalog coverage;
- after adding the dedicated test configuration, `BRAWLER_PRODUCT_GAME_TYPE=hot-zone-1v1 just e2e
  2` reached Active with one exact 1v1 roster on Crossroads Facility Hot Zone;
- after the second-playtest visual/input corrections, exact client tests cover world-scale Hot Zone
  geometry, authoritative reveal-area lifetime, and confirmation release-gating, while the authority concealment scenario sends combined
  Ultimate+Fire and proves Reveal Scan succeeds without consuming weapon ammo. `just lint` and the
  complete `just test` matrix passed again: 356 client, 273 server, 283 Balance Lab, 80 serialized
  network, and 11 performance tests, in addition to routing coverage.

Native visual/controller judgment and user acceptance remain pending; therefore M02 is not
`Complete`.

## User playtest handoff

Run `just run 2`. In the Dashboard create or edit one saved brawler with **Self Cloak**, and the
other with **Reveal Scan**; the new ultimates are available during both creation and editing. Use a
Tidal Garden match for terrain interaction, then repeat on an open map. Charge the ultimate through
ordinary combat. For Reveal Scan, press the Ultimate control once to enter targeting, aim, then
press Fire to confirm. Press Ultimate again or Cancel to leave targeting without spending charge.

Please check:

1. Self Cloak shows `CLOAKED`, uses the accepted translucent local/allied body treatment, hides from
   an enemy even at point-blank range, and ends permanently on the first accepted shot or positive
   damage.
2. An existing attack/damage or scan reveal visibly suppresses cloak without refunding or extending
   its six-second timer.
3. Reaching 100% displays `READY` without a targeting reticle. Pressing Ultimate then shows the
   local line/ring and `TARGETING` HUD prompt; Fire confirms without also firing the weapon, while
   Ultimate again or Cancel exits cleanly.
4. The accepted scan shows its exact-radius bright magenta area ring, and affected legal targets
   have a clearly visible matching ground ring outside their ordinary team ring, with no reveal
   wording overhead or in the player HUD. The reveal remains shared with the caster's team after
   targets leave the scanned circle.
5. In Hot Zone 1v1, the central objective has a visible translucent fill and bright boundary that
   change with empty, contested, and controlled state without disappearing beneath the floor.
6. When the five-second scan expires, an unexpired cloak or terrain source conceals again; the scan
   never consumes the underlying source.
7. Repeat once with reduced effects and once with
   `BRAWLER_FORCE_PRIMITIVE_WORLD=1 just client`; shape/text should preserve the scan meaning and no
   stale ring, label, nameplate, shadow, or audio should remain.

Please report whether 6 s cloak, 5 s reveal, 192-unit radius, and the targeting range feel useful
without dominating normal combat, plus any readability or controller-aim issue.

## Verification plan

### Pure/catalog tests

- Definition kind/parameter agreement, stable IDs, costs, milliunit bounds, duration bounds, exact
  catalog inventory, preset legality, fingerprints, snapshot versions, and invalid-ID failures.
- Target distance missing/zero/exact/max/over-max, arena-edge clamp, non-finite rejection, inclusive
  scan radius, hostile-only selection, stable ordering, and capacity ceiling.
- Self/ally, dead observer, forced reveal, attack/damage lock, Self Cloak, terrain proximity, exact
  deadline, and exact distance truth tables.
- Multiple source/team refresh and cleanup retain the correct maximum without duration stacking.

### ECS/schedule tests

- A held button produces one activation attempt; stale/defeated/inactive/uncharged/wrong-kind input
  consumes nothing.
- Self Cloak spends charge once, ignores proximity, expires exclusively, and is consumed on the
  first accepted attack or first positive damage; rejected fire and zero damage do not break it.
- Attack/damage locks suppress an already-active or newly activated cloak until their latest
  deadline without extending the cloak.
- Scan applies on the acceptance tick to hidden and visible hostiles, persists after area exit,
  reveals only to caster team, and never mutates concealment sources.
- Decision cache updates after completed outcomes but before cue filtering; queued network mutation
  applies before replication send.
- Defeat consumes cloak but preserves already accepted scan outcomes; respawn, replacement,
  disconnect, restart, teardown, and identifier exhaustion clean up exactly their owned state.

### Separate-App/network security tests

- A distant enemy lacks a Self-Cloaked fighter even at zero distance/proximity extremes; owner and
  ally retain it.
- Scan reveals that fighter to every caster-team client on the exact tick while a third unauthorized
  observer remains absent in a synthetic multi-team decision fixture.
- Scan expiry re-hides an unexpired cloak without replaying hidden movement; attack/damage consumption
  makes it remain visible unless another legal source applies after its lock.
- Hidden caster scan cues expose the public footprint but not caster pose or hidden target list.
- Late join, reconnect, jitter/loss/duplication, visible-hidden-visible churn, defeat/respawn, restart,
  and requeue converge without stale models, markers, messages, or unauthorized components.
- Packet/client-World inspection proves absence; screenshots alone are not security evidence.

### Canonical and native gates

- `just check`, `just lint`, `just test`, role feature isolation, V3 renderer isolation, routed
  1v1/2v2/3v3, Balance Lab persistence, restart soaks, performance gates, and affected recovery
  fixtures pass.
- Native playtest uses two builds on Tidal Garden and one open map, normal/reduced/primitive modes,
  keyboard/mouse and controller where available.
- Observe cloak ownership readability, lack of proximity reveal, attack/damage break timing, scan
  aiming/range comprehension, public pulse, affected marker, team-wide reveal, expiry/re-conceal,
  HUD/audio clarity, and whether the provisional 6 s / 5 s / 192 radius tradeoff is useful without
  dominating ordinary combat.

## Exit criteria

- the user approves this specification before production implementation;
- both ultimates are selectable, persisted, resolved, charged, activated, presented, recovered,
  reset, and cleaned through the real routed product flow;
- Self Cloak and Reveal Scan obey every accepted proximity, break, team, deadline, and source
  composition rule;
- same-tick observer decisions precede cue filtering and unauthorized clients never receive hidden
  spatial state or source-derived leaks;
- schema/fingerprint/profile/Balance Lab/global compatibility paths fail closed and preserve legal
  existing data;
- automated, impairment, capacity, native, feedback, and learning gates pass;
- the user accepts the pair before M03 is created.

## Feedback review

The first native playtest produced these accepted decisions:

- **Validated:** terrain-hidden fighters were successfully revealed by Reveal Scan; the harder Self Cloak setup
  was not reproduced manually, while the separate-App authority/security scenario continues to
  cover that interaction;
- **Implemented:** remove reveal wording from the overhead name and player HUD. Make the world-space
  reveal ring actually visible: its status is public wherever the fighter is legally replicated,
  and it uses a larger dedicated high-contrast material outside the ordinary team ring;
- **Implemented:** reaching full charge must not enter targeting. Reveal Scan and future targeted ultimates use a
  local two-phase interaction: Ultimate enters targeting without spending charge, Fire confirms
  and sends the existing authoritative ultimate intent, and Ultimate again or Cancel exits. While
  targeting, primary weapon fire is suppressed and a held trigger must be released before it can
  confirm. Immediate ultimates retain direct Ultimate-button activation;
- **Implemented:** targeting clears on loss of readiness, defeat, build/controlled-fighter change, build selection,
  non-gameplay input context, or loss of the playable gate. No new wire message is required because
  arming has no gameplay effect; authority still validates and consumes charge only on confirmation.
- **Implemented test enablement:** the server operator catalog now advertises `hot-zone-1v1`, and
  Crossroads Facility Hot Zone has mirrored concealing tall-grass strips along the objective
  approaches. The grass sits on explicit sand beds required by the authored asset's surface
  compatibility. The map recipe/admission revisions advance together so stale workers fail closed.
- **Implemented:** restore generation-owned Hot Zone fill/boundary materialization lost during the
  V8 map cutover, give Reveal Scan an exact-radius high-contrast ring for the full authoritative
  reveal duration, and retain both visuals above the playable floor without shadow occlusion. Fire confirmation is now consumed
  until physical release, and server authority also rejects a simultaneous targeted-ultimate and
  primary-fire intent so one confirmation cannot produce a weapon shot.

Affected automated verification passed. Native confirmation of the corrected input behavior and
the restored scan/objective readability remains pending the updated handoff above.

## Learn-from-errors review

Interim verification lessons recorded on 2026-08-23:

- adding inventory entries requires searching every product and automation count, not only the main
  build editor. The saved-brawler creation flow, legacy match selector, CLI bound, help text, and
  routed preset rotation now derive from or match the six-entry catalog;
- queued component insertion loses same-tick updates when multiple accepted scans independently
  clone the pre-tick source list. M02 now accumulates stably ordered scan mutations before one
  insertion and tests distinct same-team sources;
- moving observer decisions before cue filtering correctly exposed that the old subject lookup
  classified the public combat dummy as a secret-capable fighter. Cue filtering now names the same
  active, replicated, ability-bearing fighter population as concealment authority;
- a network wait helper bounded below the authored five-second scan duration was unsuitable for
  expiry evidence. The scenario now advances to the authoritative deadline and inspects both
  durable source state and client-World absence.

The final learning review remains open until user feedback is triaged and affected gates rerun.

## References

Local version-pinned sources inspected:

- `src/concealment.rs`, especially fixed-post source resolution, observer cache, cue filtering seam,
  and deferred Lightyear mutation;
- `src/abilities/{mod.rs,charge.rs,dash.rs,sentry.rs,telemetry.rs,tests.rs}`;
- `src/builds/{model.rs,definitions.rs,server.rs,tests.rs}` and `content/catalogs/builds.ron`;
- `src/protocol.rs`, `src/combat/{authority.rs,cues.rs,evidence.rs,server.rs}`;
- `src/movement/{input.rs,authority.rs}` for same-tick committed aim and input freshness;
- `src/client/{input.rs,build_editor.rs,flow.rs}`,
  `src/combat/client/{hud.rs,preview.rs}`, and `src/client/presentation_3d/combat.rs`;
- `src/profiles/{model.rs,storage.rs,tests.rs}` and `src/server/balance_lab/`;
- `references/lightyear/book/src/concepts/advanced_replication/{inputs.md,interest_management.md}`;
- `references/lightyear/book/src/tutorial/basic_systems.md`;
- `references/lightyear/examples/network_visibility/src/{server.rs,client.rs,protocol.rs}`.

Primary released references checked:

- [Lightyear replication 0.29.0](https://docs.rs/crate/lightyear_replication/0.29.0), confirming
  ordinary per-client visibility despawn and hierarchy propagation;
- [Lightyear native input 0.29.0](https://docs.rs/crate/lightyear_inputs_native/0.29.0),
  confirming fixed-tick `ActionState` consumption; exact APIs remain pinned to the local 0.29 source;
- [Bevy 0.19 `AlphaMode`](https://docs.rs/bevy/0.19.0/bevy/prelude/enum.AlphaMode.html), confirming
  standard background blending used by the accepted fighter signifier.

The local 0.29 Lightyear source remains authoritative for exact APIs because search indexing for the
standalone native-input rustdoc currently resolves an older 0.28 package page despite the lockfile's
0.29.0 pin.
