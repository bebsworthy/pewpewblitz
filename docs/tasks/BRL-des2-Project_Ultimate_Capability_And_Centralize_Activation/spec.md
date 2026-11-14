# Outcome

Ability authority depends on a focused `ResolvedUltimate` runtime component and one shared pure admission policy for input edges, lifecycle, readiness, and generation rollover. Individual ability plugins continue to own semantic restrictions, targeting, capacity, allocation, mutation, telemetry outcomes, and presentation facts.

# Scope

- Make `ResolvedUltimate` a server-local-capable ECS projection installed atomically by `MatchLoadoutProjection`.
- Migrate authoritative ability activation and charge/lifecycle consumers from `ResolvedMatchLoadout` to `ResolvedUltimate` where only ultimate data is needed.
- Add a private `abilities/activation.rs` containing:
  - pure raw request/rising-edge evaluation;
  - pure common activation admission;
  - explicit pre-readiness and post-readiness restrictions preserving existing rejection precedence;
  - pure checked ultimate-generation rollover.
- Apply the helper to Dash, Sentry, Self Cloak, Reveal Scan, Concealment Field, Demolition Strike, the elemental-field family, and Big Blob.
- Preserve raw latch semantics: a rejected rising edge is consumed until release/repress.
- Preserve exact charge equality plus `AbilityPhase::Ready` and the fixed activation schedule chain.
- Add pure table characterization and focused ECS regression tests.

# Rejection-order contract

`StaleInput -> Defeated -> Inactive -> ability pre-readiness restriction -> exact charge/Ready -> ability post-readiness restriction`.

- Dash: `AlreadyExecuting` before readiness.
- Sentry: `AlreadyExecuting` before readiness; `ExistingSentry` after readiness.
- Self Cloak: `ObjectiveCarrier` before readiness.
- Concealment Field and Big Blob: active ceiling before readiness.
- Elemental fields: `Frozen`, then active ceiling, before readiness.
- Reveal Scan and Demolition Strike: no semantic restriction in the common gate.

# Constraints

- No wire-shape, content schema, schedule-order, balance, or gameplay changes.
- Do not create a service, command bus, dynamic dispatch layer, or shared targeting abstraction.
- Parameter decoding, expensive candidate/capacity queries, targeting, allocation, charge spend, generation commit, cues, and accepted telemetry remain ability-owned.
- Do not silently normalize existing adjacent telemetry inconsistencies.
- Preserve raw requested-button latch updates even for stale or rejected attempts.

# Acceptance criteria

1. Authoritative ultimate activation and charge systems no longer traverse `ResolvedMatchLoadout` for ultimate data.
2. All eight activation coordinators use the shared pure request/admission contract while preserving the documented reason ordering.
3. Generation rollover is one checked pure helper and commits only after required allocations succeed.
4. Tests cover raw request edges, rejected-edge consumption, rejection precedence, exact readiness boundaries, generation exhaustion, projection installation, and production schedule order.
5. Formatting, linting, role-specific checks, affected unit/network suites, and proportional canonical verification pass.
6. Evidence and learn-from-errors notes are recorded before closeout.


# Implementation

- `ResolvedUltimate` is now a focused ECS component installed by `MatchLoadoutProjection` beside fighter stats, body, and weapon.
- match roster readiness now requires all six generation projections, including the ultimate capability, before activation.
- Balance Lab atomic apply reconciles the aggregate and projected ultimate in the same transaction.
- Added private `abilities/activation.rs` with pure raw request/rising-edge evaluation, ordered admission restrictions, exact Ready/charge admission, an explicit permit, and checked generation rollover.
- Dash, Sentry, Self Cloak, Reveal Scan, Concealment Field, Demolition Strike, the elemental-field family, and Big Blob now use the shared request/admission contract.
- Ability activation, charge observation, concealment-field cleanup, and elemental-field lifecycle consume `ResolvedUltimate`; Dash separately consumes the already projected `ResolvedWeapon` for its recipe fingerprint.
- Elemental-field targets now consume `ResolvedFighterStats` directly for health and resistance rules.
- Ability-specific targeting, ceiling checks, placement, allocations, charge/generation commit, cues, and telemetry remain in their owning modules; fixed activation ordering is unchanged.

# Verification evidence

- Four pure admission tests pass: raw edges/release, rejected-edge consumption, rejection precedence/exact readiness, and checked generation exhaustion.
- Focused ability suite: 21 passed.
- Full server library suite: 439 passed.
- projection installation and six-capability match readiness tests pass.
- Balance Lab atomic practice-roster re-resolution test passes with the ultimate projection.
- networked Dash/Sentry authority and durable replication scenario passes.
- `just check` passes for routing, client, server, network-test, Balance Lab, and web tooling.
- `just lint` passes with warnings denied plus server feature-isolation and retired renderer/map gates.
- `cargo fmt --all -- --check` and `git diff --check` pass.
- No native playtest required: no balance, input mapping, schedule, wire, or presentation behavior changed; server and network characterization cover the affected authority path.

# Learn-from-errors review

- Migrating Dash exposed that its activation owns two independent capabilities: ultimate semantics and the primary weapon recipe fingerprint used for source attribution. The correction queries `ResolvedUltimate` plus `ResolvedWeapon`, rather than retaining the aggregate or duplicating the fingerprint. Prevention: inventory every field read before narrowing a query and project each demonstrated capability explicitly.
- Focused activation tests initially inserted only the replicated aggregate, so the new focused queries correctly skipped those fixtures. Updating the fixtures to insert `ResolvedUltimate` made the ownership contract explicit and proved activation works without the aggregate. Prevention: characterization fixtures should install the same focused capabilities production composition installs.
- Adding a projection requires auditing every atomic boundary, not only spawn bundles. Balance Lab reconciliation and match-generation readiness were updated so partial generations cannot activate. Reusable lesson: every new immutable runtime projection needs installation, live reconciliation, readiness gating, and a missing-projection test.
- Clippy rejected one inferred `Default::default()` in a test; using `ActivationRestrictions::default()` preserved the repository's explicit-type style. No production defect was involved.
