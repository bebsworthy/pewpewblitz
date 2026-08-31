# Technical specification

## Outcome

Resolve two evidenced weapon-editor defects and complete the weapon-family follow-up to BRL-0091 without changing authoritative gameplay, canonical weapon values, accepted content, snapshot/persistence shapes, content fingerprints, protocol, or public module paths.

The non-persisted Balance Lab editor manifest changes intentionally:

- Lobbed and Splash `max_flight_ticks` must advertise the authored `WeaponRecipePolicy.minimum_lob_flight_ticks` (embedded value 6 ticks / 0.10 seconds), not 1 tick.
- Heal amount must advertise the same maximum accepted by authoritative validation (`min(EngineWeaponLimits.max_damage, WeaponRecipePolicy.max_damage)`, embedded value 1,000), not `u16::MAX`.

Bump only `EDITOR_SCHEMA_VERSION` from 9 to 10. Do not widen authoritative validation to preserve incorrect UI metadata.

## Ownership and decomposition

Create private `src/server/balance_lab/editor/weapons.rs` and move only weapon descriptor projection from `editor.rs`. `editor.rs` retains manifest composition/order, serialized descriptor types, shared helpers, and cross-family tests.

The weapon adapter must consume the existing validated `WeaponRecipePolicy` and `EngineWeaponLimits` rather than introduce a parallel schema or editor-owned policy. Replace remaining duplicated engine/policy safety literals only when there is an exact existing rule owner or a small named crate-private bound with the same semantics:

- Lobbed/Splash minimum flight time from authored recipe policy;
- Heal maximum from the intersection of engine and recipe policy;
- DestroyMap radius from `EngineWeaponLimits.max_map_destruction_radius`;
- sticky/splash active-per-owner caps from named authority bounds rather than editor-only 16/8 literals;
- Cold amount through one exact shared bound only if combat and weapon-part validation demonstrate identical semantics and the dependency remains role-clean. Otherwise keep the current authoritative owners and prove exact adapter parity without forcing a shared abstraction.

Do not move labels, grouping, paths, display units, controls, or serialization into combat definitions. Do not add reflection, proc macros, trait-per-variant, public API, or a general schema framework.

## Exhaustive projection contract

Exhaustively destructure every numeric-bearing weapon recipe variant, binding structural fields explicitly even when they are not editable:

- economy and firing;
- all delivery methods and persistent-area shapes;
- target selection;
- payload effects, falloff, and recipient policy;
- world effects.

Do not use `{ .. }` on numeric-bearing variants. Preserve established field ordering, paths, subjects, labels, units, scale, steps, controls, and help except the two approved range corrections.

## Coverage

1. Recursively collect every numeric leaf in the seven embedded weapon recipes and assert exact one-to-one unique descriptor coverage. The current embedded inventory has 87 numeric leaves; assert the audited count explicitly.
2. Add synthetic focused recipes for supported topology absent from embedded bases, at minimum Rectangle persistent shape, Cold, DamageOverTime, HostilesAndOwner, and DestroyMap. Prove each supported numeric leaf receives exactly one descriptor.
3. Assert exact corrected descriptor values for Lobbed/Splash flight minimums and Splash Heal maximum, including display scaling/units/step.
4. Retain whole-manifest path-resolution, ordering, and field-count coverage, updating only expectations changed by the editor schema/range correction.
5. Add focused validator/editor boundary tests showing values below authored lob minimum and above Heal maximum are rejected consistently while boundary values pass.
6. Verify web inline validation consumes the corrected manifest values.

## Documentation and native evidence

Update `docs/15-balance-lab.md` to the current snapshot/persistence/editor schema versions and state that weapon timing/effect controls use the intersection of authored recipe policy and engine safety bounds. Document the corrected operator values.

Capture native Balance Lab evidence showing:

- Arc Launcher and Splash flight-time minimum is 0.10 s / 6 ticks;
- Splash Healing maximum is 1,000 health;
- canonical weapon values remain unchanged;
- invalid below/above-bound input is rejected consistently by inline validation and/or Apply.

Record every observation and any correction/defer/rejection disposition in this ticket.

## Verification

Run and record:

- `cargo fmt --all -- --check`;
- `git diff --check`;
- focused combat recipe-policy/engine-limit and weapon-part tests affected by shared bounds;
- focused Balance Lab editor descriptor, numeric-leaf, validation, apply/reset, and schema tests;
- Balance Lab web tests/build;
- Balance Lab all-target check/test and strict Clippy;
- client/server role checks;
- `just check`;
- `just lint`;
- `just test`.

Independent review must confirm no authoritative behavior/content drift, exact corrected manifest output, exhaustive variant coverage, minimal visibility, and no client/server role contamination.

## Acceptance criteria

- The two known descriptor/validator mismatches are corrected without widening validation.
- Weapon projection has private focused ownership and compile-visible exhaustive numeric-bearing matches.
- Embedded and absent-topology numeric leaves have bidirectional unique coverage.
- Only editor schema 9 to 10 changes; snapshot, persistence, content, protocol, and public APIs remain compatible.
- Durable documentation and native operator evidence describe the corrected contract.
- Focused and canonical gates pass, review findings are resolved, learning is recorded, Ticket sync is conflict-free, and the ticket is done.

## Exclusions

Balance changes; new weapons/effects/delivery methods; web UX redesign; snapshot or persistence migration; content/protocol revision; public API changes; ultimate/passive/fighter/world-object metadata; general reflection/code generation; and unrelated editor/apply decomposition.

## Implementation and evidence — 2026-08-31

Implemented the weapon-family follow-up with one intentional non-persisted editor-contract correction:

- extracted weapon descriptor projection to private `server::balance_lab::editor::weapons` ownership;
- made every numeric-bearing recipe, delivery, target, shape, payload, falloff, recipient, and world-effect match compile-visible without `{ .. }`;
- changed editor schema 9 to 10 while preserving snapshot 19, persistence 13, weapon catalog 11, fingerprint 9, content, protocol, public APIs, and authoritative accepted values;
- projected Lobbed/Splash flight minimums from authored recipe policy (6 ticks / 0.10 s), Heal/Damage/DoT maxima from the effective policy/engine intersection, named per-owner caps from combat validation, shared the exact Cold amount cap in the role-clean weapon-parts-to-combat direction, and retained DestroyMap's deliberate Balance Lab engine ceiling of 128;
- added exact one-to-one coverage for all 87 embedded recipe numeric leaves plus validated synthetic Rectangle, Cold, DamageOverTime, HostilesAndOwner, and DestroyMap topology;
- added independent literal full-descriptor contracts, scalar and cross-policy boundary tests, web inline-validation tests, and current schema/ownership documentation.

Automated verification passed:

- focused editor tests: 17 passed;
- focused combat-definition tests: 18 passed;
- focused weapon-part tests: 7 passed;
- Balance Lab all-target tests: 536 passed plus map public API 1;
- Balance Lab web tests: 11 passed and Vite production build passed;
- client/server role checks and strict Balance Lab Clippy with `-D warnings`;
- `just check`;
- `just lint`;
- `just test`, including all 97 network scenarios and all 12 performance gates;
- `cargo fmt --all -- --check` and `git diff --check`.

Independent review found three P2 evidence/documentation gaps: unexpected descriptor prefixes were filtered, corrected descriptors were only partially frozen, and documentation overstated scalar parity across cross-field rules. All were corrected. Follow-up review is clean with no remaining actionable finding.

## Native operator evidence and feedback disposition

Ran an isolated routed Balance Lab session with separate state/data paths and real windowed Practice automation, then inspected the live Chrome operator page:

- Arc Launcher canonical flight remained 0.75 s; the input exposed min 0.10, max 10, step 1/60.
- Splash canonical flight remained 0.60 s and canonical Heal remained 24; inputs exposed flight min 0.10/max 10/step 1/60 and Heal min 1/max 1,000/step 1.
- Entering 0.09 s showed `Must be at least 0.1 s.` and disabled Apply.
- Restoring 0.10 s and entering Heal 1,001 showed `Must be at most 1000 health.` and disabled Apply.
- Current draft matched server defaults with zero canonical differences before edits.

Feedback accepted: corrected scalar bounds are visible, canonical values are unchanged, and invalid values fail inline before Apply. No presentation correction was required.

One intermediate Hot Zone automation worker used only to keep the local endpoint open exited with code 0 but without the expected result envelope and was classified `WorkerExitMismatch`; the subsequent Heist evidence worker completed and reaped cleanly, automated routed suites remained green, and the editor observations were unaffected. Disposition: not a BRL-0093 product finding because it was not reproduced and occurred in the deliberately overlapping temporary evidence setup; retain the canonical BRL-0070 Hot Zone evidence run as the closeout authority.

## Learning review

The follow-up audit correctly checked serialized descriptors against validator source rather than assuming the existing UI was authoritative; this exposed two live scalar mismatches. The first implementation tests still risked self-confirmation through partial assertions and permissive filtering. Prevention: external adapters need an independently literal full-contract oracle, exact prefix accounting before path-set equality, and both ordinary embedded data plus valid synthetic absent-topology cases.

The native session also showed why endpoint lifecycle must be part of Balance Lab evidence: the page can retain a stale read-only snapshot while waiting for the next Practice worker. Operator checks must record whether controls are authoritative/enabled before testing validation, and isolated automation clients should be sequenced rather than overlapped when worker-result evidence itself is under test.
