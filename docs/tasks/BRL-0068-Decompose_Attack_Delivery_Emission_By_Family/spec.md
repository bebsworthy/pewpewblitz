# Outcome

`authoritative_composed_fire` retains one explicit accepted-attack transaction, while each delivery family owns a focused emission helper with a narrow, testable responsibility. Adding or changing one existing delivery family no longer requires editing one 380-line mixed dispatcher body.

# Scope

- Decompose `emit_attack_deliveries` into focused handlers for:
  - straight and sticky-straight projectile emission;
  - blocked muzzle contact and legacy impact publication;
  - lobbed and persistent-splash launch emission through one shared lob-flight spawn primitive;
  - melee message emission;
  - cone-spray entity/runtime emission.
- Introduce only the smallest local immutable request/mutable sink structures justified by repeated parameters; do not create a public delivery framework.
- Keep the exhaustive `DeliveryMethod` match at the attack composition point so stable serialized family coverage remains visible.
- Preserve the existing event reservation cursor, blocked-delivery ordering, match membership, projectile/runtime bundles, replication/interpolation components, telemetry, and emitted-delivery counts.
- Keep attack admission, economy, ID/event reservation, accepted-fact publication, and fixed-tick schedule ordering unchanged.

# Decisions and constraints

- This is an organization-only server-authority refactor: no protocol, content schema, balance, physics, cue, or presentation changes.
- Do not split one unchanged giant function into another file. Extract named algorithms/commit helpers with concrete responsibilities and reuse the shared lob-flight construction where the current Lobbed and Splash branches duplicate it.
- Preserve deferred `Commands` behavior and the current `GameplaySet::Fire` transaction boundary.
- Do not begin projectile sweep or composed-effect application decomposition in this ticket.
- Keep new items private and attach any unavoidable Clippy exception narrowly.
- Preserve unrelated workspace changes.

# Acceptance criteria

1. `emit_attack_deliveries` becomes a short exhaustive coordinator, with each family implementation independently readable and no duplicated Lobbed/Splash projectile bundle construction.
2. Straight blocked-hit event/cue cursor semantics and sticky arming behavior remain exact.
3. Every current delivery family retains authoritative behavior, match membership, replication state, telemetry counts, and deterministic ordering.
4. Existing focused pure, server, network, and performance coverage for straight, sticky, lobbed, splash, melee, and cone spray passes; add characterization where an extracted contract lacks direct evidence.
5. Formatting, linting, role-specific checks, affected network scenarios, and canonical tests pass.
6. Evidence and a learn-from-errors note are recorded before closeout.

# Verification

- Focused attack/delivery unit tests.
- Network scenarios covering primary straight fire, point-blank blocked contact, sticky attachment, launcher/lobbed delivery, persistent splash, melee/blade sector, and cone spray.
- `cargo fmt --all -- --check`
- `git diff --check`
- `just check`
- `just lint`
- `just test`
- No native playtest is required unless behavior changes; this slice must preserve gameplay exactly.

## Implementation and verification — 2026-08-30

- Moved accepted primary-attack delivery emission behind the private `combat::attack::emission` module while leaving admission, ID/event reservation, economy mutation, accepted facts, trackers, cue publication order, and the `GameplaySet::Fire` schedule boundary unchanged.
- `emit_attack_deliveries` is now a short exhaustive `DeliveryMethod` coordinator. Straight/sticky, blocked contact, melee, cone spray, lobbed, and splash behavior have focused helpers.
- Lobbed and Splash now share one projectile construction primitive, retaining exact `LobbedFlight`, runtime, deadline, replication/interpolation, `MatchMember`, and flight-duration behavior.
- Blocked straight contact retains ordered event-cursor consumption and legacy cue/log publication. Blocked sticky emission still arms without publishing an impact event. Tracker cardinality remains one per straight/lob/melee delivery, cone pulse count, and splash pulse count plus landing.
- Added a routed melee characterization proving Impact Blade emits no projectile, resolves authoritative damage, and records preset telemetry. Existing routed coverage characterizes straight thin-cover and point-blank contact, sticky attachment, lobbed focal distance, persistent splash, cone spray, and same-fixed-tick projectile collision.

Verification passed:

- `cargo fmt --all -- --check`
- `git diff --check`
- focused server attack tests: 5 passed
- focused routed family scenarios: 8 passed (straight thin cover, point-blank object contact, sticky, lobbed, splash, melee, cone spray, first-fixed-tick projectile)
- `just check`
- `just lint`
- `just test`: 492 client, 439 server, 461 Balance Lab, 95 network, and 12 performance tests passed; routing/package suites also passed

No native playtest was required because this was an organization-only authority refactor with direct routed characterization for every delivery family.

## Learn-from-errors review

- The first strict-lint run caught a manual `match` destructuring that should have used `let ... else`; it was corrected before closeout and the full lint matrix was rerun.
- The focused routed test was initially invoked without the required `network-test` feature. The command failed before executing tests, was corrected, and the successful feature-explicit command plus the canonical suite were recorded.
- Reusable lesson: preserve the schedule-facing attack transaction and extract deterministic family commits beneath it. This reduces mixed responsibility without introducing unordered systems or a speculative public delivery framework.
