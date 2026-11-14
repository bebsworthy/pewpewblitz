# Outcome

Authoritative gameplay consumes authoritative combat facts, while CombatCue and CombatOutbox remain downstream presentation/network projections.

# Scope

- Introduce a bounded, fixed-tick accepted-attack fact shape owned by combat authority.
- Emit the fact at the authoritative attack-commit boundary.
- Make concealment reveal-lock resolution and Self Cloak lifecycle consume the authoritative fact.
- Project AttackAccepted presentation cues independently from the same committed attack outcome.
- Define and test the fact-clear lifecycle so no attack leaks into later ticks.
- Audit other server gameplay reads of CombatOutbox and remove any equivalent cue-to-authority dependency in scope.
- Preserve cue schema and wire behavior unless evidence requires an explicit protocol change.

# Acceptance criteria

- No authoritative gameplay decision reads CombatCue or CombatOutbox.
- Accepted attacks still reveal concealed fighters and end Self Cloak with unchanged timing and precedence.
- Cue suppression, filtering, or publication timing cannot change reveal or cloak state.
- Facts are bounded, deterministic, and cleared at an explicit schedule phase.
- Client presentation continues receiving the expected AttackAccepted cue.

# Verification

- Focused combat/concealment/Self Cloak schedule tests.
- Regression test proving gameplay is unchanged when presentation cue publication is absent.
- Existing combat cue, concealment, and routed authority tests.
- Schedule ambiguity/deferred-boundary checks.


# Implementation evidence

- Combat authority owns a bounded 256-entry `AcceptedAttackFacts` fixed-tick buffer containing only stable event, attack, tick, and source network identities.
- Primary-fire admission checks fact capacity before committing ammunition, cooldown, recovery, or deliveries; the accepted fact and unchanged `AttackAccepted` cue are then projected independently from the same committed record.
- Concealment reveal locks and Self Cloak lifecycle consume accepted-attack facts rather than `CombatOutbox`; no authoritative gameplay decision reads `CombatCue` or `CombatOutbox`.
- The fact lifecycle is registered by combat itself and clears in `CombatSet::Finalize`, including compositions without matchplay. Match restart also clears the buffer defensively.

# Verification evidence

- Focused tests cover the hard capacity bound, two-tick combat-only clearing, cue-free Self Cloak termination with Attack precedence, cue-free concealment reveal, and real accepted-fire fact emission with stable identities.
- `just check` passed all role and Balance Lab build configurations.
- `just lint` passed formatting, strict Clippy for all feature graphs, server feature isolation, renderer ownership, and map cleanup checks.
- `just test` passed routing suites; 456 client tests; 386 server tests; 407 Balance Lab tests; the mixed Balance Lab/network replication test; 90 network tests; and 12 performance gates.

# Learn-from-errors review

- The first implementation attached fact clearing to matchplay because the existing outcome facts clear there. Independent review exposed that combat-only compositions would retain accepted attacks. The lifecycle is now registered at the owning combat boundary, and a no-match two-tick regression prevents recurrence.
- Adding a seventeenth direct Bevy system parameter exceeded the supported system-function arity. The two commit outputs are now grouped in a focused `SystemParam`, keeping the attack coordinator schedulable without hiding unrelated inputs.
