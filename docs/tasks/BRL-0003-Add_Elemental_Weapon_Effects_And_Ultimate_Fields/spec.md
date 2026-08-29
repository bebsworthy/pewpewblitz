# Outcome

Deliver Cold/Freeze, Poison, Fire, and Healing as meaningful saved-brawler choices. Each family is available through a weapon effect and a targeted ultimate-created area under the accepted decisions below. The server remains the only authority for contributions, damage, healing, freeze, overlap, resistance, defeat, and cleanup.


# Product interpretation

The four families should share carriers where their lifecycle truly matches, but they are not one universal status engine:

1. Cold is an accumulating target-owned meter that can trigger a temporary Freeze.
2. Poison and Fire are two bounded damage-over-time conditions using the same runtime shape but distinct identities, resistances, tuning, cues, and presentation.
3. Healing is positive health application. It is not a hostile condition and does not occupy a damage-over-time slot.
4. Weapon payloads and ultimate fields are sources. They feed the same effect rules rather than implementing separate gun and area versions of damage, buildup, resistance, or healing.

# Recommended player-visible content

## Weapon effects

- Cryogenic Module: trades direct weapon power for Cold contribution on eligible hits.
- Incendiary Module: trades direct weapon power for a short, higher-pressure Fire condition.
- Toxin Module: trades direct weapon power for a longer, lower-pressure Poison condition.
- Healing Module: exact projectile/recipient behavior is an open decision; it must not create free self-sustain accidentally.

These should be weapon-part effects resolved into the existing immutable operational weapon recipe. A part must impose an authored opportunity cost such as lower direct damage, capacity, cadence, reach, or another bounded sidegrade.

## Ultimate fields

- Cryogenic Field: periodic Cold contributions to hostile fighters inside the targeted area.
- Fire Field: periodic Fire application or refresh to hostile fighters.
- Poison Field: periodic Poison application or refresh to hostile fighters.
- Restoration Field: periodic Healing to eligible allied fighters and normally the owner.

All four use one bounded targeted-area carrier with typed effect definitions. They have distinct stable ultimate definitions, names, art/audio profiles, values, and recipient policies. A fighter may own at most the one active field permitted by its ultimate phase; total live fields remain proportional to the roster.

# Runtime model

## Active fighter conditions

Evolve the current bounded ActiveEffects state into an explicit fixed-shape condition record rather than a dynamic map or one marker-component combination per status:

- existing strongest-refreshes Slow;
- ColdState: meter, last contribution tick, optional frozen deadline, optional post-thaw immunity deadline, and source attribution;
- optional PoisonDamageOverTime;
- optional FireDamageOverTime.

Poison and Fire may coexist. Each kind has at most one active record per target. This keeps wire size, per-fighter work, late-join state, evidence, and cleanup bounded.

Healing is applied through health outcomes and cues. A healing field pulses while occupied; it does not install a permanent healing-status component unless later playtesting demonstrates a real heal-over-time requirement after leaving the field.

## Effect definitions

Add focused typed payload effects rather than a behavior graph:

- ColdContribution { amount, recipients }
- DamageOverTime { kind: Poison | Fire, damage_per_tick, tick_interval, duration, stacking, recipients }
- Heal { amount, recipients }

Area fields carry the same effect definitions plus center, radius, pulse interval, activation/expiry ticks, stable field/source identity, and owner/match identity.

## Resistance

Add resolved fighter values using integer basis points:

- cold buildup reduction;
- poison damage reduction;
- fire damage reduction.

Cold resistance modifies incoming meter contribution, not the authored threshold. Poison and Fire resistance modify their matching damage ticks independently. Healing uses a separately named healing-received multiplier only if a real modifier is selected; it is not treated as elemental resistance.

Recommended build source: three new mutually exclusive resistance passives so players cannot cover every weakness with two passive slots. Exact names and values belong to balancing/specification review. All values must be visible in saved-brawler resolution and Balance Lab.

# Recommended condition semantics

## Cold and Freeze

- Hits and Cryogenic Field pulses add bounded Cold after resistance.
- The target-owned meter begins decaying after a contribution-free delay.
- Reaching the threshold triggers Freeze, resets or consumes the meter, and starts a short post-thaw immunity window.
- Recommended initial Freeze effect: movement is rooted and primary/ultimate activation is rejected, while facing and readable aim presentation may continue.
- Freeze interrupts Dash movement. Existing deployables and already-created fields continue under their own lifecycle.
- Defeat, respawn, match restart, build replacement, and teardown clear meter, Freeze, immunity, and source attribution.

The exact threshold, decay, duration, input restrictions, and immunity require playtest-driven values.

## Fire and Poison

Both use the same deterministic damage-over-time record and strongest-refreshes policy in the first slice. Reapplication replaces a weaker instance or refreshes an equal/stronger instance without creating an unbounded source list.

Recommended differentiation:

- Fire: shorter duration and higher damage per second.
- Poison: longer duration and lower damage per second.
- Each uses its own resistance and may coexist with the other.
- Both retain the source that currently owns the active condition so tick damage, defeat, ultimate charge policy, telemetry, and cues have stable attribution.

No anti-heal, spreading, explosion, panic, armor reduction, stacking count, or environmental ignition is implied in this feature.

## Healing

- Healing never exceeds resolved maximum health.
- Defeated entities cannot be healed or resurrected.
- Healing does not award damage-based ultimate charge.
- Restoration Field healing stops when the fighter leaves the area; it does not leave a hidden lingering condition.
- Healing amount, source, target, applied amount, resulting health, and rejection reason are authoritative outcome facts.
- Self, ally, deployable, Heist objective, chest, and other world-target eligibility must be explicit. Recommended first slice heals live fighters only.

# Area authority and lifecycle

A server-only activation system validates ultimate readiness, targeting range, map bounds, and field capacity, then spawns one replicated public field fact. The server evaluates fighter centers against the field circle at fixed pulse deadlines. Clients never report membership or effect application.

Fields persist to their authored expiry after the caster is defeated, but are removed on match restart, build replacement before activation, worker teardown, or invalid match ownership. Disconnect behavior and whether an already-active field survives owner disconnect require specification review.

Leaving a field stops future pulses but does not remove already-applied Cold, Poison, or Fire. Overlapping fields resolve through the target-owned per-kind rules; they do not multiply active runtime records.

# Schedule composition

Keep direct combat and condition outcomes in the existing authoritative fixed-tick path:

1. Gameplay input and ultimate activation create attacks/fields.
2. Movement and physics establish authoritative positions.
3. Direct weapon impacts and due field pulses produce typed pending effects.
4. The composed combat transaction applies immediate damage/heal and condition contributions in deterministic source/target order.
5. A focused condition-tick phase applies due Poison/Fire ticks with stable ordering and attribution before match outcome evaluation.
6. Cold decay, Freeze expiry/immunity, and ordinary condition expiry run in explicit lifecycle/condition sets.
7. Outcome facts drive charge, passives, modes, telemetry, replication, HUD, audio, and presentation.

The exact same-tick ordering of direct defeat, healing, and condition ticks must be frozen in the milestone specification and tested; it must not depend on Bevy system interleaving.

# Presentation and feedback

- Cold: visible buildup meter near fighter UI, escalating frost treatment, distinct threshold/frozen and thaw/immunity cues.
- Fire: warm high-energy flame treatment and sharp tick feedback.
- Poison: visibly different hue, silhouette, icon, and slower tick rhythm from Fire.
- Healing: positive restoration cue, health delta, and field language that cannot be mistaken for poison or concealment.
- HUD shows active condition kind, remaining deadline where useful, resistance/build facts, and Freeze state.
- Presentation may degrade, but missing effects never change authority.
- Concealment privacy remains intact; fields and conditions expose only facts allowed for the observing client.

# Implementation sequence

## Slice 1 — Fire and Poison weapon/field vertical slice

Implement the shared bounded damage-over-time record, two resistances, two weapon parts, two targeted field ultimates, attribution, cues, HUD, Balance Lab, bot observation, and complete lifecycle. This proves both carriers with the simpler ongoing-effect model.

## Slice 2 — Cold/Freeze weapon/field vertical slice

Add cold contribution, meter/decay/threshold, Freeze, immunity, Cryogenic Module and Field, input/movement interruption, presentation, resistance, bots, and recovery evidence.

## Slice 3 — Healing weapon/field vertical slice and feature balance

Add the approved healing-gun behavior, allied recipient/contact policy, Restoration Field, healing outcomes, UI/cues, bot use, full 3v3/routed/recovery/performance evidence, balance pass, feedback triage, documentation reconciliation, and learning review.

Each slice must end in usable gun and ultimate content; none is an infrastructure-only status framework milestone.

# Prerequisites and boundaries

- Reconcile the legacy full-build path before expanding weapon/ultimate/passive catalogs so new content has one saved-brawler authority path.
- Update build, weapon-part, protocol, content, Balance Lab, routed snapshot, evidence, telemetry, bot observation/action, and client presentation schemas together.
- No active-item slot, resurrection, shields, armor, lifesteal, anti-heal, status spreading, elemental terrain reaction, arbitrary effect graph, client prediction, or client authority is included.
- V13 art work may supply presentation assets later but does not own gameplay semantics or gate headless verification.

# Verification themes

- Pure condition math: resistance, threshold, decay, refresh/replacement, pulse deadlines, tick catch-up, expiry, immunity, heal clamp, and same-tick ordering.
- ECS lifecycle: spawn, defeat, respawn, restart, build replacement, disconnect, late join, field cleanup, and bounded overlap.
- Authority/network: clients cannot claim membership/effects; replicated condition/field facts converge and concealment is not leaked.
- Cross-source composition: gun and field applications produce identical effect semantics and attribution.
- Gameplay targets: fighters only unless a later explicit scope adds deployables or objectives.
- Performance: maximum 3v3 fields, all fighters carrying all condition slots, dense overlapping pulses, and no unbounded per-source history.
- Native playtest: readability under 3v3 overlap, build tradeoffs, counter-resistance comprehension, Freeze fairness, Fire/Poison distinction, and healing target clarity.

# Accepted planning decisions — 2026-08-27

The user accepted all three recommended decisions:

1. A Healing Module projectile heals an allied fighter it contacts while the same base weapon retains its ordinary hostile damage contact. Authoritative contact policy, aim tracing, projectile feedback, and target highlighting must distinguish allied healing contact from hostile damage contact without allowing one collision to claim both outcomes.
2. Cold, Poison, and Fire resistance come from three new mutually exclusive passives competing for the existing two passive slots. The resolved loadout carries all three resistance values explicitly.
3. Freeze roots movement, rejects primary and ultimate activation, preserves facing/aim presentation, and interrupts an active Dash.

The user added one Poison rule: an actively poisoned fighter cannot receive passive healing.

For this feature, "passive healing" means the implemented attack-idle fighter health recovery only. Poison does not block a Healing Module projectile, Restoration Field pulse, or another future explicit allied healing action. While Poison is active:

- the server-owned recovery system applies no recovery health;
- the last-accepted-attack origin continues to age;
- existing fractional recovery progress is preserved rather than discarded; and
- when Poison expires, recovery resumes immediately if the attack-idle delay is already satisfied.

Defeat, respawn, restart, build replacement, and teardown still reset recovery and conditions through their ordinary lifecycle. Poison suppression must be represented in Balance Lab, evidence, telemetry, HUD/status feedback, bot observation, and focused same-tick tests. A later anti-heal capability that reduces or blocks active healing would be a separate mechanic and is not implied by Poison.

# Acceptance criteria

- Four selectable weapon-part effects deliver Cryogenic, Incendiary, Toxin, and Healing behavior through the resolved saved-brawler weapon recipe with explicit sidegrade costs.
- Four selectable targeted ultimates create Cryogenic, Fire, Poison, and Restoration fields through one bounded server-owned area carrier with typed effects and stable identities.
- Cold contribution, decay, resistance, threshold, Freeze, thaw, and immunity follow the accepted rule; Freeze roots movement, rejects primary/ultimate activation, preserves facing/aim, and interrupts Dash.
- Fire and Poison use bounded separately resisted damage-over-time records, remain visually and mechanically distinguishable, coexist safely, and retain exact source attribution.
- Poison suppresses only passive attack-idle health recovery while active, preserves its timer and fractional progress, and does not block Healing Module or Restoration Field healing.
- Healing projectiles resolve allied healing contact and hostile damage contact explicitly; healing is clamped, cannot resurrect, and produces no damage-based ultimate charge.
- Three mutually exclusive resistance passives expose Cold, Poison, and Fire resistance through saved-brawler resolution and Balance Lab.
- Defeat, respawn, restart, build replacement, disconnect, late join, field expiry, and worker teardown leave bounded authoritative state and converge through the routed protocol.
- Bots observe and use the implemented content through permitted authoritative facts and ordinary validated input without a parallel gameplay path.
- Focused rule/ECS tests, representative network tests, canonical checks, maximum-overlap performance evidence, and native 3v3 readability/balance playtesting pass.
- Durable gameplay, network, UX, Balance Lab, bot, and presentation documentation reflects the accepted behavior; feedback and substantial-work learning are recorded before the ticket moves to done.

# Implementation-readiness review — 2026-08-28

## Readiness and dependency disposition

- BRL-0007 is done. Its saved-brawler-only authority path is the prerequisite this ticket needed, so BRL-0003 is ready to enter `doing` when implementation starts.
- BRL-0043 remains a related deferred cleanup, not a blocker. BRL-0003 must not restore player-facing point-budget meaning or broaden into removing the legacy machinery. New ultimate/passive catalog rows may carry only the inert nonzero placeholder values required by the current schema; saved-brawler resolution continues to report zero points and does not enforce a budget. BRL-0043 removes those fields later.
- BRL-0033 is related bot breadth work, not a blocker. BRL-0003 owns the minimum bot observation, targeting, and activation behavior needed for its new weapon modules and fields; it should reuse any generic loadout-derived bot seams BRL-0033 has delivered by implementation time.
- No unresolved human decision remains. Initial values below are implementation baselines exposed through Balance Lab and are expected to receive native balance correction before closeout.

## Live-code findings and owned seams

1. `ActiveEffects` in `src/combat/model.rs` is already a replicated, fixed-shape target-owned component, but currently contains only strongest-refreshes Slow. Extend that shape with Cold, Poison, and Fire; do not introduce a dynamic map or one marker component per condition.
2. `PayloadEffectDefinition`, recipe policy validation, canonical fingerprinting, effect planning/application, cues, telemetry, and the weapon-part resolver are the closed typed seams for weapon effects. Elemental module resolution must merge one typed effect into eligible hostile payload bundles and then run the ordinary weapon validator.
3. Straight-projectile sweep currently treats only live hostiles as fighter collision candidates. Healing Module therefore requires delivery eligibility to consider the first live fighter that at least one direct payload can affect. It is insufficient to add a Heal match arm after collision. Walls and live world objects remain blockers, and one collision produces one recipient outcome, never both allied healing and hostile damage.
4. The current composed payload transaction already sorts work, dry-runs health/defeat state, reserves event IDs, applies records, and commits deferred effects. Preserve this atomicity. Add elemental/healing planning to the same deterministic transaction where the source is a weapon; field pulses and condition ticks use focused batches but call the same pure resistance, refresh, healing, condition, and outcome helpers.
5. Targeted ultimate aim/range/bounds validation already exists in the Reveal Scan, Concealment Field, and Demolition Strike paths. Add one elemental-field activation system using that helper and one typed field carrier; do not duplicate four activation systems.
6. Concealment Field is useful evidence for replicated field presentation and cleanup, but its owner-defeat privacy lifecycle is not the elemental-field contract. Elemental fields use their own stable ID/state/runtime types and cleanup rules.
7. Attack-idle recovery runs in `FixedUpdate` before projectile/field outcomes in `FixedPostUpdate`. Preserve that schedule: Poison active at the start of a tick suppresses that tick's recovery; Poison first applied later in the tick begins suppression on the next tick. Its idle origin and fractional remainder are left unchanged while suppressed.
8. Fighter reset/defeat/restart paths already replace `ActiveEffects` with default state. Extend those assertions for every new slot and explicitly interrupt Dash on a new Freeze.
9. `ResolvedMatchLoadout`, `ActiveEffects`, `AbilityState`, and field state are replicated through `src/protocol.rs`. The changed serialized shapes require the one global protocol compatibility bump plus build/weapon/weapon-part/profile catalog fingerprints and any Balance Lab/diagnostic schema floors affected by their persisted or emitted shape. Add no compatibility decoder.
10. Balance Lab weapon descriptors currently enumerate Damage/Knockback/Slow and the web roster summarizes only current modifiers. Extend editor descriptors, roster views, apply/reset/persistence migrations, and the web UI together. Saved profiles retain choices by stable ID; profile storage needs a migration only if its serialized saved shape actually changes.

## Concrete content contract

### Stable inventory

- Weapon parts: 9 Cryogenic Module, 10 Incendiary Module, 11 Toxin Module, 12 Healing Module.
- Ultimates: 7 Cryogenic Field, 8 Fire Field, 9 Poison Field, 10 Restoration Field.
- Passives: 7 Cryogenic Insulation, 8 Filtered Circulation, 9 Heat Shielding.
- The three resistance passives are pairwise mutually exclusive. The four elemental weapon modules are also one mutually exclusive module family, so four part slots cannot equip every elemental behavior at once.
- Cryogenic, Incendiary, and Toxin modules may resolve on a weapon with an eligible hostile fighter payload. Healing Module is initially compatible only with a single-firing straight-projectile recipe; incompatible equipment is rejected and explained by the saved-brawler UI.

### Initial weapon values and sidegrades

- Cryogenic Module: 250 Cold per attack/target and -15% direct hostile damage.
- Incendiary Module: Fire at 25 damage every 30 ticks for 120 ticks and -15% direct hostile damage.
- Toxin Module: Poison at 14 damage every 30 ticks for 240 ticks and -15% direct hostile damage.
- Healing Module: 140 healing on allied non-owner contact, -20% direct hostile damage, and +10% fire interval.
- Cold contribution is capped once per stable weapon `AttackId` and target. A Scatter volley therefore contributes Cold once to one fighter even if several pellets connect. Fire/Poison use their one-record strongest-refreshes rule, so same-attack pellet duplication cannot create extra stacks.
- A Healing Module projectile damages the first eligible live hostile it contacts through the weapon's ordinary hostile payload, heals the first eligible live ally it contacts, passes through its owner, and stops on that fighter. One contact cannot apply both branches. It does not heal deployables, objectives, map objects, or defeated fighters.

### Initial resistance and condition values

- Each matching resistance passive grants 3,000 basis points (30%) reduction. Resolved fighter stats carry `cold_resistance_basis_points`, `poison_resistance_basis_points`, and `fire_resistance_basis_points` explicitly; values are bounded to 0..=6,000.
- Apply resistance with integer round-half-up: `(authored * (10_000 - resistance) + 5_000) / 10_000`, preserving a minimum of one for a positive authored value below full immunity.
- Cold threshold: 1,000. Decay begins after 90 contribution-free ticks and removes 10 meter per tick. Threshold consumes the meter, freezes for 60 ticks, then grants 90 ticks of post-thaw Cold immunity. Contributions during Freeze or immunity are ignored. Source attribution clears when the meter returns to zero or Freeze resolves.
- Poison and Fire each have at most one record. Stronger incoming damage-per-tick replaces the record and its source; equal strength refreshes expiry and retains the newest exact source; weaker input is ignored and does not refresh. Their first damage tick is one full interval after application, never immediate. Poison and Fire may coexist.
- Weapon-sourced Fire/Poison tick damage remains `PrimaryWeapon` damage for existing dealt/received charge policy. Ultimate-field damage remains `Ultimate` damage and awards no charge. Cold and Healing award no charge.

### Initial field values

- All four fields use maximum targeting range 520 world units, radius 150, duration 300 ticks, and pulse every 30 ticks. The activation tick is the first pulse deadline.
- Cryogenic Field contributes 125 Cold per hostile pulse.
- Fire Field applies/refreshed Fire at 18 damage every 30 ticks for 90 ticks.
- Poison Field applies/refreshed Poison at 10 damage every 30 ticks for 180 ticks.
- Restoration Field heals the owner and live allied fighters for 45 per pulse.
- Each owner may have at most one active elemental field and the match may have at most six. Field runtime work is bounded by six fields times six fighters; candidate fields and fighters are sorted by stable field ID and network entity ID before application.
- Public replicated field state carries stable ID, kind, team, center, radius, activation/next-pulse/expiry ticks, and owner identity. A server-only runtime component retains the typed effect definition and exact source attribution so a field can finish after its owner disconnects.
- An active elemental field persists through owner defeat and owner disconnect until authored expiry. It is removed on expiry, match completion/restart, build replacement in a mutable development session, or worker teardown. Ability phase settles when an owned field ends and the owner still exists; ordinary fighter respawn does not remove the field.

## Fixed-tick ordering contract

Within one authoritative tick:

1. Input validation and ultimate activation run; new fields become visible before simulation through the existing deferred-command boundary.
2. Authoritative movement and physics establish poses. A fighter already Frozen cannot translate or activate primary/ultimate; facing/aim state may still update.
3. Direct weapon contacts resolve in the existing deterministic attack/delivery/target order. Immediate hostile damage resolves before allied healing, then Cold/Fire/Poison contributions are installed. Lethal damage marks the target defeated and later healing cannot resurrect it in the same tick.
4. Due field pulses resolve by `(field_id, target_network_id)` using the same resistance/heal/condition helpers.
5. Due Poison ticks then Fire ticks resolve by target ID and retained source identity. A condition newly installed this tick cannot tick because its first deadline is in the future.
6. Cold contribution/threshold is finalized, new Freeze interrupts Dash and settles its ability phase, then Cold decay, thaw/immunity transitions, and ordinary condition expiry run. A contribution on the current tick prevents decay.
7. Defeat/mode outcomes, charge/passive observers, telemetry/cues, replication facts, and the authoritative tick publish observe the committed result.

Recovery remains earlier in `FixedUpdate`: already-active Poison suppresses it without changing the idle origin or remainder; Poison acquired during steps 3–5 suppresses from the next tick. Focused schedule tests must lock all of these boundaries.

## Implementation plan by playable slice

### Slice 1 — Fire and Poison weapon plus field content

1. Add typed condition/source/field IDs and definitions, extend closed recipe/recipient/part/passive/catalog validation, add exact stable content rows, and advance compatibility/fingerprint floors.
2. Add bounded Poison/Fire records and pure resistance/strongest-refresh/tick-deadline rules. Extend the composed weapon transaction, outcomes, exact attribution, charge observation, cleanup, evidence, and lifecycle tests.
3. Add the single elemental-field carrier and activation path, Fire/Poison field runtime, replicated public facts, aim preview, world presentation, HUD/audio/cues, Balance Lab controls/migration, and minimum bot targeting/use.
4. End the slice with usable saved-brawler weapon modules and targeted ultimates plus focused server/client/network tests; do not stop at infrastructure.

### Slice 2 — Cold and Freeze

1. Add Cold meter/decay/threshold/thaw/immunity and per-attack contribution de-duplication through the existing transaction.
2. Gate movement, primary fire, and every ultimate activation through one shared frozen rule; preserve facing/aim and interrupt Dash without disturbing deployables or already-active fields.
3. Add Cryogenic Module/Field/passive, replicated meter/freeze state, escalating fighter treatment, meter/HUD feedback, cues/audio, bot observation, Balance Lab controls, and lifecycle/network tests.

### Slice 3 — Healing and closeout balance

1. Generalize direct projectile fighter eligibility from hostile-only to payload-eligible, then add explicit allied-non-owner Heal contact while preserving hostile damage, geometry blocking, deterministic first contact, transaction reservation, and world-target boundaries.
2. Add Restoration Field and active-healing outcomes/rejections, clamp/no-resurrection/no-charge rules, target highlighting, projectile/field feedback, bot ally selection, Balance Lab controls, and representative network tests.
3. Run maximum 3v3 overlap/performance, routed recovery/late-join/reconnect, Practice bot, Balance Lab, and native readability/balance evidence. Reconcile durable gameplay, fighter/build, network, bots, UX, Balance Lab, and presentation docs; triage feedback and record the substantial-work learning review.

## Focused verification map

- `src/weapon_parts/tests.rs`, `src/builds/tests.rs`, `src/profiles/tests.rs`: IDs, applicability, mutual exclusions, deterministic aggregation, resistance resolution, advertised catalogs, saved-choice preservation, and fingerprints.
- `src/combat/effects/tests.rs`, new condition/field tests, `src/abilities/tests.rs`, `src/movement/tests.rs`: resistance rounding, once-per-attack Cold, strongest refresh, due ticks, event reservation rollback, healing contact, Freeze input/Dash rules, pulse/expiry ordering, and cleanup.
- `tests/network/combat_composed.rs`, `combat_projectiles.rs`, `combat_recovery.rs`, `lifecycle.rs`, and a focused elemental scenario: authority, replication, late join, disconnect, restart, concealment-safe facts, Poison recovery suppression, and exact source convergence.
- `tests/performance.rs`: six simultaneous fields, six fully populated condition slots, dense overlap, bounded event/fact/state collections, and fixed-tick budget.
- Balance Lab Rust/web tests: editor descriptors, roster modifiers/resistances, transaction apply/reset, exact-version persistence migration, bot rows, and visual labels.
- Canonical closeout after each affected slice: format, focused role checks/tests, then full `just check`, `just lint`, `just test`, representative routed E2E/Practice E2E, maximum-overlap performance, and native 3v3 playtest.

## Implementation start — 2026-08-28

Implementation began from commit `ed56d89` with the previously recorded BRL-0032 closeout mirrors and BRL-0003 planning mirrors already uncommitted. Those Ticket-owned changes remain in scope for preservation; unrelated untracked local tooling directories remain untouched.

## Implementation progress — 2026-08-28

Implemented the authoritative elemental combat slice: four weapon modules, four field ultimates, three mutually exclusive resistance passives, typed Cold/Freeze/Poison/Fire/Healing runtime state, deterministic direct/field/DOT ordering, poison recovery suppression, replicated bounded field lifecycle, allied healing delivery, frozen movement/input suppression, cues/telemetry, client field and condition presentation, bot targeted-field use, Balance Lab ultimate tuning/migration, catalog/schema/fingerprint/protocol bumps, and durable weapon/Balance Lab documentation.

Verification so far:

- `cargo check --lib --features client,server` — passed.
- `cargo check --lib --features client,server,balance-lab` — passed.
- `cargo test --all-targets --all-features --no-run` — BRL-0003 exhaustiveness failures corrected; the gate remains blocked by concurrent map/prediction test errors (`ResolvedMapSnapshot::geometry` and `client::prediction::resolve_static_arena`) outside this ticket.

Native gameplay and the three required subjective slices remain pending after automated closeout.

## Automated implementation handoff — 2026-08-28

Implementation is complete through automated closeout. The authoritative slice now includes:

- Cryogenic, Incendiary, Toxin, and Healing weapon modules with one mutually exclusive elemental-module family and explicit recipe compatibility;
- Cryogenic Insulation, Filtered Circulation, and Heat Shielding as pairwise-exclusive 30% resistance passives carried in resolved fighter stats;
- target-owned Cold/Freeze/Poison/Fire state, strongest-condition refresh, round-half-up resistance, cold decay/thaw immunity, poison recovery suppression, deterministic field and DOT order, exact attribution, cues, outcomes, and telemetry;
- Cryogenic, Fire, Poison, and Restoration targeted fields with editable authored effects, immediate/fixed pulses, a six-field ceiling, one live field per owner, replication, expiry/restart/build-replacement cleanup, and defeat/disconnect persistence;
- allied straight-shot healing with clamping/no resurrection/no ultimate-charge gain, frozen input/movement/Dash interruption, client aim/status/field presentation, Practice bot condition observation and healing/Restoration targeting;
- Balance Lab schema 11 / persistence schema 6 migration, 104 numeric editor leaves, elemental roster modifiers/resistances, web UI support, and protocol version 33.

Final automated evidence:

- `just check` — passed, including client/server/network/Balance Lab feature graphs and Balance Lab web test/build.
- `just lint` — all constituent gates passed after corrections: formatting, web test/build, routing/client/server/network/Balance Lab Clippy with `-D warnings`, server feature isolation, V3 presentation guard, and V8 map cleanup guard.
- `cargo test --lib --all-features` — 623 passed.
- `just test` — passed: routing 83 + 4 + 5 + 5 + 3; client 429; server 344; Balance Lab 361; focused Balance Lab network 1; network 88 sequential; performance 12. Representative performance p95 results remained below the fixed-tick budget, including the combined combat gate at 3.284 ms and 100-fighter/200-projectile gate at 2.491 ms.
- Balance Lab web tests — 10 passed; TypeScript/Vite production build passed.
- `git diff --check` — passed.

Native closeout remains required. Exercise three saved-brawler slices in 3v3 Practice:

1. Cryogenic Module + Cryogenic Field + Cryogenic Insulation: verify meter readability, Freeze timing/fairness, movement/fire lock, thaw immunity, and field overlap clarity.
2. Incendiary or Toxin Module + matching field + matching resistance: compare Fire/Poison identity, refresh readability, poison recovery suppression, resistance comprehension, and overlapping-field clarity.
3. Healing Module + Restoration Field: verify allied projectile targeting, no enemy/self projectile effect, capped healing, field ownership/readability, and bot ally support.

Also inspect Balance Lab’s field controls and player-loadout elemental modifier/resistance labels. Record every observation before moving the ticket from `doing` to `done`.

## Learn-from-errors review — implementation handoff

- Adding constrained parts invalidated old tests and Balance Lab validation that assumed every starter part and every four-part combination fit every weapon. Prevention: future part-family work must distinguish universal legacy sidegrades from deliberately constrained compatibility before enumerating combinations.
- Evolving a persisted ultimate list exposed order-sensitive migration after older migrations appended rows. Prevention: migrations for ordered identity lists must deduplicate and restore canonical ID order before validation.
- Extending runtime enums compiled under ordinary client/server features before Balance Lab and network exhaustive matches were visible. Prevention: run the combined client/server/Balance Lab check immediately after each serialized enum expansion, then the role-specific Clippy matrix before full tests.
- The implementation initially made field effect values derived constants, which would have made Balance Lab controls cosmetic or impossible. Correction: effect strength is now authored inside `ElementalFieldEffect`, validated by field kind, persisted, editable, and used directly by authority.

## Native playtest feedback — 2026-08-28

The user received `Practice Error / The selected build was rejected by the server` with no explanation or visible logs.

Investigation reproduced the offending persisted selection in `target/dev/server/profiles.sqlite3`: the selected Arc Launcher brawler carries both Cryogenic Insulation and Filtered Circulation, although BRL-0003 defines resistance passives as pairwise mutually exclusive. The edit path allowed that invalid pair to persist; Practice admission later failed resolution. The lobby then discarded `ProfileAuthorityError` and `BuildResolutionError` with `.ok()` / blanket `map_err`, collapsed every cause into `PracticeStartRejection::InvalidBuild`, and emitted no rejection log.

Accepted correction scope:
- reject incompatible resistance-passive edits before persistence with actionable saved-brawler UI copy;
- preserve an actionable incompatible-build Practice rejection for already-persisted invalid selections and log the server-side classification/context;
- keep legacy invalid profiles loadable so the player can edit them back to validity;
- migrate existing starter inventories from catalog revision 1 to revision 2 by granting only missing part definitions, since the current seeder hard-codes revision 1 and otherwise hides BRL-0003's four new modules from established profiles;
- add regression tests and rerun affected canonical verification before requesting another playtest.

## Native rejection correction complete — 2026-08-28

Disposition: implemented and automatically verified; BRL-0003 remains `doing` for the resumed native playtest.

Root cause and repair:
- The selected persisted brawler combined Cryogenic Insulation with Filtered Circulation. Resistance-passive mutual exclusion existed in final loadout resolution but not in the profile edit transaction, so the invalid choice was saved and rejected only at Practice admission.
- Profile create/edit commands now reject multiple elemental resistance passives before persistence as `ProfileDecision::IncompatibleBuild`. The brawler editor keeps the draft open and displays: “Choose only one elemental resistance passive for this brawler.”
- Previously persisted incompatible brawlers remain loadable and editable. Practice classifies them as `PracticeStartRejection::IncompatibleBuild` and displays: “This brawler has incompatible choices. Edit it and choose only one elemental resistance passive.”
- Lobby admission no longer discards the underlying profile/build error. It emits a bounded warning with client ID, request ID, brawler ID/revision, cause, and rejection classification. Repeat-resolution failures are also logged.
- The wire-enum addition advances the global protocol compatibility version from the BRL-0003 implementation value 33 to 34.
- Existing starter inventories now migrate from the stored starter-set revision to the embedded catalog revision, append only missing definitions, preserve existing part identities/equipment, and run exactly once. This grants definitions 9–12 to established profiles without replacing their legacy parts.

Regression evidence:
- focused server profile tests: 15 passed, including persisted-incompatible repair/admission and revision-1-to-2 starter inventory migration;
- focused client flow tests: 39 passed;
- focused lobby tests: 11 passed;
- `just check` — passed;
- `just lint` — passed for routing, client, server, network-test, Balance Lab, feature isolation, renderer cleanup, and map cleanup;
- `just test` — passed: routing 83 + 4 + 5 + 5 + 3; client 429; server 346; Balance Lab 363; focused Balance Lab network 1; network 88 sequential; performance 12. Representative p95 remained below the fixed-tick budget, including combined combat at 3.289 ms and 100-fighter/200-projectile at 2.637 ms;
- `git diff --check` — passed.

Native retry:
1. Restart `just run` so both client and supervisor/lobby use protocol 34 and the inventory migration runs.
2. Open the selected brawler and replace either Cryogenic Insulation or Filtered Circulation with a non-resistance passive (or keep only one resistance).
3. Save, then start 3v3 Practice. The invalid pair should now be rejected inline during editing; the repaired brawler should enter Practice.
4. Confirm the four elemental modules are present in the established profile inventory.

## 2026-08-28 playtest feedback: loading and cancellation

- Feedback: Match Loading could remain visible with `Your accepted build: 0/12 points`, and Cancel Match Start appeared not to work.
- Finding: saved-brawler resolution intentionally reports legacy `total_points = 0` and does not enforce the retired 12-point budget. The label was stale presentation, not an admission or readiness gate.
- Finding: the interactive playable gate requires an active join plus ready map and world-object snapshots. A stable-data native automated Hot Zone 3v3 Practice run reached match-ready and Active, so the accepted BRL-0003 loadout is valid; the earlier generic invalid-build failure remains corrected.
- Correction: remove point-cost copy from Queue and Match Loading immediately. BRL-0043 continues to own removal of the internal legacy fields, outcomes, constants, and fingerprint inputs.
- Correction: preserve a Match Loading cancellation request until a match-session sender actually exists, send lobby cancellation only over a lobby session, and clear redundant queued cancellation after an authoritative cancellation/return outcome.
- Verification required: focused client cancellation/model tests, client role check/Clippy/tests, and routed Practice E2E after the correction.

### Loading/cancellation correction evidence (2026-08-28)

- `cargo check --locked --no-default-features --features client --all-targets` — passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings` — passed.
- `cargo test --locked --no-default-features --features client --all-targets` — passed, 429 client-library tests.
- Focused Queue/Match Loading copy regression — passed and asserts neither screen contains `points`.
- `just practice-e2e hot-zone-3v3` — passed; Practice reached Active with one human.
- Stable-data native automated Hot Zone 3v3 Practice with the persisted selected brawler — emitted `handoff-connected`, `match-accepted`, and `match-ready`, then completed its bounded render run.
- Disposition: implemented. The misleading point copy is removed, cancellation survives the transport handoff, lobby cancellation is session-scoped, and full legacy cost-field removal remains BRL-0043.

## 2026-08-28 playtest feedback: Balance Lab Practice worker panic

- Observed: the routed match worker reached Ready, accepted the client transport, then panicked in `process_client_hellos` at `validated manifest build resolution: InvalidCombination`; the client remained on Match Loading.
- Root cause: match-manifest validation correctly resolved the saved-brawler snapshot against canonical catalogs, but Balance Lab then installed persisted weapon tuning before the client hello. A loadout with equipped weapon modifiers recomputed a different weapon recipe fingerprint against the tuned catalog, and the general snapshot resolver treated that expected Balance Lab identity change as an invalid manifest combination.
- Required correction: retain strict canonical identity validation at the manifest boundary; after that boundary, allow Balance Lab to re-resolve admitted snapshots against its validated revised catalogs and publish the recomputed runtime identity. Apply/reset must use the same path.
- Failure handling: a runtime manifest resolution failure must reject the client with a bounded server outcome and diagnostic instead of panicking the worker and leaving Match Loading stranded.
- Regression: reproduce with persisted non-canonical weapon tuning plus a human saved brawler carrying non-default weapon modifiers; verify the worker admits the client, starts Practice, and the tuned resolved identity is installed.

### Balance Lab manifest correction evidence (2026-08-28)

- Added a regression for an admitted Arc Launcher snapshot with non-default damage/slow weapon modifiers. Canonical resolution remains strict; revised Balance Lab weapon tuning recomputes the runtime weapon/build identity successfully.
- Persisted Lab catalogs are now checked against every admitted human and bot snapshot before installation. A genuinely incompatible persisted snapshot is ignored with a warning and canonical defaults remain active.
- Match hello resolution now returns a bounded `ContentMismatch` rejection with an error diagnostic instead of panicking the worker.
- `cargo check --locked --no-default-features --features server --all-targets` — passed.
- `cargo check --locked --no-default-features --features balance-lab --all-targets` — passed.
- Server and Balance Lab Clippy with `-D warnings` — passed.
- `cargo test --locked --no-default-features --features balance-lab --lib` — passed, 364 tests.
- `cargo test --locked --no-default-features --features network-test,balance-lab --test network builds::revised_catalog_loadout_keeps_build_identity_and_replicates_authoritative_values -- --test-threads=1` — passed.
- Exact native reproduction with `target/balance-lab/session-v2.json` revision 4 and the persisted selected modified Arc brawler emitted `handoff-connected`, `match-accepted`, and `match-ready`; the gameplay render report observed the full six-fighter Hot Zone 3v3 roster. No manifest-resolution panic occurred.
- Documentation: the Balance Lab guide now records strict canonical admission followed by revised-catalog roster re-resolution and fail-safe persisted-tuning handling.

## 2026-08-29 playtest feedback: Cold buildup context

- Observation: Balance Lab labels Cryogenic Field as `Cold buildup 125` without showing that the target-owned Freeze threshold is 1,000, so the value has no visible denominator.
- Current rule: each hostile pulse contributes 125 to the target Cold meter after resistance; Cryogenic Insulation reduces matching contributions by 30% (125 becomes 88 with round-half-up). At 0% resistance, eight pulses reach the 1,000 threshold. Cold then consumes the meter and applies Freeze.
- UI finding: the Players & loadouts card reports only nonzero elemental resistances and renders `None` for a fighter without a resistance passive. Fighter-profile resistance values exist in the authoritative snapshot but are not exposed by the numeric editor manifest, and the fixed 1,000 Cold threshold is not shown beside Cold controls.
- Disposition: explanation provided; operator-facing denominator/resistance visibility remains unresolved playtest feedback under BRL-0003.

## Proposed correction: per-fighter elemental baselines (2026-08-29)

### Product rule

Make elemental durability an authored property of every fighter profile, even while the initial profiles share the same values. The resolved fighter loadout is the authority for the target's elemental baselines; sources continue to author contribution or damage strength.

Initial independently authored profile defaults:

| Fighter property | Default | Lightweight | Reinforced | Authored bounds |
| --- | ---: | ---: | ---: | ---: |
| Cold capacity | 1,000 | 1,000 | 1,000 | 1..=10,000 |
| Cold resistance | 0% | 0% | 0% | 0..=60% |
| Poison resistance | 0% | 0% | 0% | 0..=60% |
| Fire resistance | 0% | 0% | 0% | 0..=60% |

Use basis points for resistance storage as today. Add `cold_capacity` to authored and resolved fighter stats. Do not add Fire or Poison capacities in this correction: those conditions are strongest-refresh damage-over-time records with duration and tick cadence, not thresholded buildup meters. Their per-fighter tuning surface is resistance. Healing is beneficial delivery, not an elemental attack, and gets neither capacity nor resistance here.

### Resolution and authority

- Replace both hard-coded 1,000 Cold caps/thresholds with the affected target's resolved `fighter_stats.cold_capacity` in direct weapon-effect and field-pulse application.
- Apply the target's resolved Cold resistance first, then add the rounded contribution and cap/compare it against that target's Cold capacity. Reaching capacity consumes the meter and applies Freeze under the existing thaw-immunity rule.
- Preserve the three authored baseline resistance values already carried by fighter profiles and resolved loadouts.
- Change Cryogenic Insulation, Filtered Circulation, and Heat Shielding from replacement values to additive +3,000 basis-point modifiers, clamped to the existing 6,000 maximum. This ensures a passive augments rather than erases a fighter type's baseline.
- Keep server authority, deterministic integer resistance rounding, lifecycle reset, attribution, and bounded state unchanged.

### Balance Lab and player-facing explanation

- Add four Fighter controls for every fighter profile: Cold capacity, Cold resistance, Poison resistance, and Fire resistance. Profiles must remain independently editable even when their initial values match.
- Display resistance controls as percentages while retaining basis-point storage. Always show all four baseline values in Players & loadouts, including zero values, and separately show the equipped passive bonus/effective resolved resistance where applicable.
- Show Cold strength with its denominator or derived outcome. At minimum, Cryogenic Field should read as `125 Cold per pulse` and the selected/inspected fighter should show `Cold capacity 1,000`; a derived `8 unresisted pulses to Freeze` value is preferred where the UI has both values.
- Any client Cold meter/progress treatment must divide by the replicated target capacity rather than assume 1,000.

### Compatibility, persistence, and verification

- Include Cold capacity in fighter/build fingerprints and the replicated resolved loadout. Bump the one global protocol compatibility version and affected catalog, Balance Lab snapshot/editor, and persistence schemas together; add no compatibility decoder.
- Migrate persisted Balance Lab snapshots that predate Cold capacity to the canonical 1,000 value. Existing resistance fields retain their stored values.
- Add focused tests for independently authored equal profile defaults, capacity validation, resistance-baseline plus passive addition/clamping, boundary contributions immediately below/at capacity, direct-effect and field use of target capacity, and client percentage derivation.
- Extend Balance Lab Rust/web coverage for all four controls, zero-value visibility, apply/reset, persistence migration, derived Cold explanation, and effective resistance display.
- Add representative routed replication/recovery coverage with two fighter profiles using different Cold capacities and resistances.
- Native playtest should compare otherwise equivalent targets with different Cold capacities/resistances and confirm the Balance Lab explains why Freeze timing differs.

### Scope recommendation

Accept this as a correction within BRL-0003 because it makes the delivered elemental rules tunable and comprehensible. Treat any future Fire/Poison buildup capacity or healing-received modifier as a separate gameplay-design ticket: either would change those mechanics rather than expose an existing baseline.

## Per-fighter elemental baseline correction implemented (2026-08-29)

Implemented the approved playtest correction:

- `ResolvedFighterStats` and all three independently authored fighter profiles now carry `cold_capacity`; initial values are 1,000 and validation bounds them to 1..=10,000.
- Direct Cold weapon effects and Cryogenic Field pulses now apply the target's resolved Cold resistance and accumulate against that target's resolved capacity through one shared deterministic helper. The duplicated hard-coded 1,000 thresholds were removed.
- Cold, Poison, and Fire resistance passives now add 3,000 basis points to the matching fighter baseline and clamp at 6,000 rather than overwriting the baseline.
- Balance Lab exposes Cold capacity and all three resistance baselines for Default, Lightweight, and Reinforced. Resistance controls display percentages while retaining basis-point storage. Cryogenic values are labeled per hit/per pulse and explain that they are compared with target capacity.
- Players & loadouts always shows Cold capacity and all three resistance baselines, including zero; equipped passive bonuses appear as a distinct effective resistance.
- Build catalog/fingerprint formats advanced to 10, advertised catalog format to 3, protocol compatibility to 35, Balance Lab snapshot/editor schemas to 12/4, and persistence to 7. Persistence schema 6/snapshot 11 migrates missing Cold capacities to the canonical profile values.
- Durable weapon/ability and Balance Lab documentation now owns these rules. Fire/Poison remain duration-based conditions without capacities; Healing remains outside elemental resistance.

### Verification

- Focused Cold capacity/resistance boundary test — passed.
- Focused independently authored fighter baseline plus additive/clamped passive resolution test — passed.
- Focused Balance Lab fighter control/bound test — passed.
- Focused sequential persistence migration through Cold capacity — passed.
- Balance Lab web tests — 10 passed; TypeScript/Vite production build passed.
- `just check` — passed for routing, client, server, network-test, Balance Lab, and web feature/build graphs.
- `just lint` — passed formatting, web, all Clippy `-D warnings` feature matrices, server isolation, renderer cleanup, and map cleanup gates.
- `just test` — passed: routing 83 + 4 + 5 + 5 + 3; client 430; server 348; Balance Lab 366; focused Balance Lab routed replication 1; network 88 sequential; performance 12. Representative p95 remained within the fixed-tick budget, including combined combat at 3.070 ms and 100-fighter/200-projectile at 2.197 ms.
- `git diff --check` — passed.

### Feedback verification requested

In Balance Lab, inspect Fighters for the four new Elemental baselines on each profile and Players & loadouts for zero baselines plus passive-adjusted effective resistance. Change one profile's Cold capacity and resistance, apply/reset, then compare Cryogenic Field Freeze timing against a different profile. Confirm that `Cold per pulse`, target capacity, and baseline/effective resistance make the outcome understandable.

### Learning review

The first persistence implementation changed the previous migration step to the new current-version constants. That made an old schema 5/10 document jump directly to 7/12 and deserialize before `cold_capacity` was inserted. The focused migration regression exposed the error. Correction: every historical migration step now advances to its explicit historical successor (5/10 -> 6/11), and only the new step uses the current 7/12 constants. Prevention: never use a moving current-version constant for an intermediate persistence-ladder edge; add the missing-field removal to the oldest supported end-to-end migration test before accepting a schema bump.

## 2026-08-29 playtest feedback: overhead Cold buildup indicator

Reference screenshots show a small circular status disc immediately to the left of the overhead health bar. Treat them only as visual references.

Accepted correction:

- Add a compact cyan radial/pie meter to every fighter's projected overhead, positioned directly left of the health bar without displacing the player name or ammunition row.
- Fill represents `ActiveEffects.cold.meter / ResolvedMatchLoadout.fighter_stats.cold_capacity`, clamped to 0..=100%; it must not assume the canonical 1,000 capacity.
- Hide the meter when Cold buildup is zero and the fighter is not Frozen, so the ordinary overhead remains uncluttered.
- While Frozen, show a full, strongly differentiated frozen disc even though the authoritative meter has been consumed to zero.
- Use replicated target-owned state only; the indicator is presentation and has no authority path.
- Preserve viewport projection, off-screen hiding, overhead cleanup, reduced-effects behavior, and bounded client asset/entity ownership.
- Add focused tests for percentage/clamping, zero-capacity fail-safe behavior, hidden/partial/full-frozen display policy, and overhead layout placement. Verify natively at several buildup levels and during Freeze/thaw.

### Clarification

The overhead pie is a buildup meter only: render no number or text, show it only while `cold.meter > 0`, and hide it as soon as buildup returns to zero—including when Freeze consumes the meter. Existing fighter treatment communicates active Freeze; do not substitute a full pie during Freeze. This clarification supersedes the earlier full-frozen-disc bullet.

## Overhead Cold buildup indicator implemented (2026-08-29)

Implemented the clarified playtest request in the existing projected fighter overhead:

- A 15 px cyan radial disc sits immediately left of the health bar and contains no number or text.
- It is hidden unless replicated `ActiveEffects.cold.meter` is greater than zero, and therefore disappears when Freeze consumes the meter.
- Its 32 bounded fill frames quantize the target-owned `meter / cold_capacity` ratio, clamp over-capacity values, and fail hidden for an invalid zero capacity.
- The textures are generated once as bounded client-only image assets; each fighter reuses the handles and changes only its selected frame/visibility.
- The overhead container widened from 104 px to 120 px so the new disc does not overlap the centered health bar, player name, or local ammunition row.
- Durable weapon/ability documentation now records the buildup-only, no-number presentation rule.

Verification:

- Focused ratio/visibility and overhead-layout tests — passed.
- Focused Bevy schedule query-disjointness test — passed after making the Cold pie, overhead root, and ammunition visibility queries explicitly disjoint.
- Client suite — 431 passed.
- Client Clippy with `-D warnings` — passed.
- `just check` — passed across routing, client, server, network-test, Balance Lab, and web graphs.
- `just lint` — passed all formatting, web, Clippy, feature-isolation, renderer, and map cleanup gates.
- `git diff --check` — passed.

Native verification remains: observe no disc at zero buildup, partial clockwise cyan fill after Cryogenic contributions on fighter profiles with different capacities, no numeric label, clean positioning beside the health bar, and immediate disappearance when the meter decays to zero or triggers Freeze.

## Cold buildup HUD playtest feedback (2026-08-29)

- User confirmed that the buildup-only, number-free overhead Cold pie indicator works.
- Approval scope is limited to this HUD indicator. It does not approve BRL-0003 as a whole, and the ticket remains in `doing` pending its remaining acceptance and playtest evidence.

## 2026-08-29 playtest feedback: Frosting versus Cryogenic module

- Observation: a weapon described as having a frost module did not appear to apply Cold buildup.
- Live-profile evidence: the selected development profile `Brawler 4` equips weapon-part definition 7, `Frosting Module`. That legacy part contributes only `Slow` (15%% for 36 ticks). Definition 9, `Cryogenic Module`, is the distinct part that contributes 250 Cold and drives the overhead buildup pie.
- Root cause: player-facing naming makes two mechanically distinct parts sound equivalent. This is a comprehension defect, not an authoritative Cold-application failure in the observed build.
- Disposition: awaiting product correction. Recommended smallest correction is to rename the legacy `Frosting Module` to a plainly non-elemental Slow name while preserving its stable ID and behavior; do not silently convert it to Cold or duplicate Cryogenic behavior.

### Accepted correction

- Rename weapon-part definition 7 from `Frosting Module` to `Kinetic Dampener` so its player-facing name clearly describes its non-elemental Slow role.
- Preserve definition ID 7, stable key `frosting-module`, Slow values, equipped-slot identity, and all gameplay behavior.
- Advance the starter inventory revision and migrate existing definition-7 inventory instances to the new display name without replacing their instance IDs or equipment references.
- Verify embedded catalog validity plus a persisted-profile rename migration. Keep `Cryogenic Module` as the sole Cold-buildup weapon part.

### Rename implemented

- Definition 7 now displays as `Kinetic Dampener`; its stable ID, `frosting-module` key, damage tradeoff, and 15%%/36-tick Slow remain unchanged.
- Existing stored instances are renamed idempotently during profile load. Their instance IDs and equipped slot references are preserved, and the profile revision advances so clients receive the corrected label.
- The starter-set revision remains 2. The initial plan to advance it was rejected because that revision participates in the gameplay content fingerprint; a display-only rename must not create a compatibility change.
- Focused embedded-catalog validation passed. The persisted-profile regression passed with a stale `Frosting Module` label, preserved original part IDs, installed any missing newer parts, renamed definition 7, and remained stable on a second load.

## Proposed correction: authored global Cold/Freeze rules (2026-08-29)

### Ownership recommendation

- Keep `cold_capacity` and `cold_resistance_basis_points` on fighter profiles: they describe how much Cold a target can absorb and how strongly it reduces incoming buildup.
- Move Cold decay delay/rate, Freeze duration, and post-thaw immunity out of constants into one global `ColdConditionRules` definition. These values describe the shared condition lifecycle, so duplicating them across fighter profiles would make the same meter threshold produce opaque target-specific timing and add redundant tuning controls.
- If fighter-specific Freeze duration is later desired as an intentional archetype trait, add a separately named modifier then; do not pre-model it now.

### Proposed authored and runtime shape

- Add a focused headless-safe combat-condition catalog under `content/catalogs/` containing `cold_decay_delay_ticks`, `cold_decay_per_tick`, `freeze_duration_ticks`, and `thaw_immunity_ticks`, with the current 90/10/60/90 values as defaults and explicit validation bounds.
- Load it as a server-authoritative Bevy resource consumed by Cold application and condition advancement. Remove the four gameplay constants from `conditions.rs`.
- Include its canonical material in the global gameplay content fingerprint. No client authority is introduced; clients continue presenting replicated meter/deadline state.

### Balance Lab exposure

- Add a global `Cold & Freeze` group rather than placing these controls under each fighter: Decay delay, Decay rate, Freeze duration, and Post-thaw immunity.
- Display timing in seconds with ticks visible, and display decay as both buildup/tick and the derived buildup/second at 60 Hz.
- Add the rules to Balance Lab snapshot validation, apply/reset, persistence migration, and editor schema. Applying tuning updates the authoritative resource at the ordinary safe restart boundary.

Status: recommendation prepared; awaiting user approval before implementation.

### Accepted Balance Lab information architecture

- Add a top-level `Global` tab for match-wide tuning that is not owned by a fighter, weapon, ultimate, or map object.
- Place the initial controls in a `Cold & Freeze` section within that tab: buildup decay delay, buildup decay rate, Freeze duration, and post-thaw immunity.
- The tab is intentionally extensible for later global settings, but this correction implements only the Cold rules currently required. Do not introduce a generic settings registry or speculative empty sections.
- Fighter tabs continue to own Cold capacity and elemental resistance baselines.


## 2026-08-29 — Global cold lifecycle tuning implementation

Implemented the accepted global-versus-fighter ownership split:

- Added build-embedded `content/catalogs/combat_conditions.ron` and validated `CombatConditionRules` runtime ownership for cold decay delay, cold decay rate, freeze duration, and post-thaw immunity.
- Removed the corresponding hard-coded lifecycle constants from `src/combat/conditions.rs`; direct payloads and ultimate fields now resolve freeze duration from the same server-owned rules resource.
- Included the rules in the gameplay content fingerprint (content envelope revision 19) without changing the wire schema.
- Added Balance Lab snapshot/editor/persistence support (snapshot 13, persistence 8, editor 5), including atomic restart-boundary application and migration of revision-7 sessions to canonical baseline rules.
- Added a top-level **Global** tab with a **Cold & Freeze / Lifecycle** section. Timing values are edited in seconds; decay is displayed as cold per second while preserving deterministic integer-per-tick storage.
- Kept cold capacity and resistance under each fighter, as previously accepted.
- Updated the durable combat and Balance Lab documentation with ownership, defaults, units, and migration behavior.

Current authored defaults preserve existing behavior: 1.5 s decay delay, 600 cold/s decay, 1.0 s freeze, and 1.5 s post-thaw immunity.

Verification passed:

- `cargo check --locked --no-default-features --features balance-lab --all-targets`
- Focused combat-rule, cold-lifecycle, Balance Lab editor, persistence-migration, and atomic-apply tests
- Balance Lab web unit tests and production build
- `cargo test --locked --no-default-features --features balance-lab --lib` — 370 passed
- `just check`
- `just lint`
- `just test` — canonical feature matrices passed; network suite 88 passed and performance suite 12 passed

Native operator/playtest confirmation of the new Global controls remains optional follow-up evidence; BRL-0003 remains doing for its wider elemental feature scope.


### Gamepad targeted-field placement correction

Player feedback: while a targeted elemental ultimate is armed, right-stick magnitude must control placement distance from the fighter. This is placement range, not field-effect radius.

Implementation contract:

- For gamepad input, derive `aim_distance` from the selected targeted ultimate's authored maximum range while its targeting mode is armed, including Fire Field and the other targeted ultimates sharing that interaction.
- On the initial ultimate-button frame, use the targeted ultimate range immediately so the sampled distance does not depend on the primary weapon delivery.
- Outside targeted-ultimate interaction, preserve the existing lobbed-primary range behavior; immediate ultimates and non-lobbed weapons gain no unrelated distance behavior.
- The server continues to clamp the client distance intent to the authoritative ultimate maximum and playable bounds.
- Add a focused client input test proving a non-lobbed Fire Field loadout produces different distances from different right-stick magnitudes.


Implementation and feedback disposition:

- Confirmed root cause: the existing analog-distance calculation was gated exclusively by the controlled primary weapon's `Lobbed` delivery. A straight-firing weapon paired with Fire Field therefore produced no gamepad `aim_distance`, causing authoritative targeting to use maximum range.
- Reused the existing calibrated right-stick magnitude mapping. Range selection now prefers the armed/pressed targeted ultimate's authored maximum range and otherwise preserves the lobbed-primary range.
- The correction applies to Reveal Scan, Concealment Field, Demolition Strike, Cryogenic Field, Fire Field, Poison Field, and Restoration Field through their existing `Targeted` classification. Immediate Dash, Sentry, and Self Cloak are unaffected.
- Added a focused non-lobbed Fire Field test covering the initial arm frame and two distinct stick magnitudes while targeting remains armed.

Verification passed:

- `cargo fmt --all -- --check`
- focused Fire Field gamepad placement, existing gamepad mapping, and targeted-ultimate tests
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`
- `cargo test --locked --no-default-features --features client --lib` — 434 passed


### Persistent damage-field cadence correction

Player feedback: a hostile fighter remaining inside Fire Field receives no visible damage.

Root cause and implementation contract:

- Field pulses refresh equal-strength Fire/Poison damage-over-time at the same 30-tick cadence as condition damage. The current refresh replaces `next_tick` before the condition system runs, indefinitely postponing damage while overlap continues.
- Equal-strength reapplication must refresh source/expiry without postponing the already scheduled damage tick. Stronger conditions retain their existing replacement behavior; weaker conditions remain ignored.
- Preserve authoritative field overlap, hostility, resistance, spawn-protection, bounded lifecycle, and fixed schedule ordering.
- Cover the cadence boundary with a focused regression test and run the affected server combat suite.


Implementation and verification:

- Changed equal-strength damage-over-time refresh to update attribution, cadence metadata, and extend expiry while preserving the already scheduled `next_tick`. Stronger replacement and weaker rejection behavior are unchanged.
- Added a pure boundary test for a Fire Field refresh exactly when damage is due.
- Added a scheduled ECS regression that refreshes Fire before condition processing at tick 30 and proves health drops from 100 to 82, the next damage tick advances to 60, and expiry extends to 120.
- Because Fire and Poison use the same authoritative damage-over-time refresh path, the correction covers both persistent hostile damage fields.

Verification passed:

- `cargo fmt --all -- --check`
- focused damage-over-time refresh and scheduled Fire Field regression tests
- `cargo clippy --locked --no-default-features --features server --all-targets -- -D warnings`
- `cargo test --locked --no-default-features --features server --lib` — 353 passed


## Closeout — 2026-08-29

User playtest acceptance:

- The elemental status HUD was accepted.
- Frost weapon-part behavior and naming were corrected and accepted through continued playtesting.
- Targeted ultimate gamepad placement now reuses analog lob-distance shaping across all targeted ultimates.
- Fire Field damage cadence was corrected after playtesting exposed refresh starvation; the user accepted closeout after the correction.

Learning review:

- Mistake: equal-strength damage-over-time refresh replaced `next_tick`, allowing a field pulse at the same cadence to starve damage forever. Cause: refresh behavior was tested as state replacement rather than against schedule ordering at the exact due tick. Prevention: persistent-effect refresh tests must cover reapplication immediately before lifecycle processing at the due boundary.
- Mistake: gamepad distance shaping was coupled to the primary weapon's lob delivery even though targeted ultimates consume the same input field. Cause: the client range selector was named and scoped around the first consumer rather than the active targeting interaction. Prevention: when an intent field has multiple authoritative consumers, test each interaction context with a loadout where the other consumer is absent.
- Useful enduring split: global lifecycle cadence belongs in authored global combat rules; fighter capacity and resistance remain per-fighter tuning. The Balance Lab now reflects that ownership directly.

All feedback is implemented or separately tracked, required automated verification is recorded above, no Ticket questions/comments remain open, and the user explicitly requested BRL-0003 closure.
