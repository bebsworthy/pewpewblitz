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
