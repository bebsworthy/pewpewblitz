# Outcome

Authoritative passive behavior depends on one focused immutable runtime projection installed atomically with the resolved loadout generation. The replicated `ResolvedMatchLoadout` remains the client convergence, evidence, and control-plane contract.

# Scope

- Add a focused `ResolvedPassives` component containing the exact two resolved passive definitions.
- Install it through `MatchLoadoutProjection` at every production/test spawn boundary.
- Migrate authoritative passive trigger telemetry, Adrenal Response movement, Quick Cycle ammunition recovery, Close Quarters damage scaling, Tenacity slow reduction, and passive-derived combat resistances to focused projections.
- Where a coordinator currently uses multiple loadout capabilities, query the existing `ResolvedWeapon`, `ResolvedUltimate`, or `ResolvedFighterStats` projections independently rather than retaining the aggregate.
- Reconcile `ResolvedPassives` in the Balance Lab atomic apply transaction.
- Require the passive projection in match-generation readiness so partially installed generations cannot activate.
- Add pure and focused ECS characterization proving passive behavior runs without `ResolvedMatchLoadout` when the required projections exist.

# Decisions and constraints

- `ResolvedPassives` is a small concrete ECS component, not a generic capability framework, trait registry, or service.
- Preserve the fixed two-passive authored contract and the existing passive kind/parameter schema.
- Preserve server authority, deterministic fixed-tick ordering, rejection/telemetry semantics, stable wire shapes, balance values, and client/server feature isolation.
- `ResolvedMatchLoadout` intentionally remains at replication/convergence, lobby/control-plane, build resolution, client input/targeting/presentation, evidence/verification, and Balance Lab aggregate ownership boundaries.
- Combat-effect transaction staging and delivery-family decomposition are separate follow-up work; this slice narrows their inputs without redesigning their ordering.
- Preserve unrelated workspace changes.

# Acceptance criteria

1. Production authoritative passive consumers do not query `ResolvedMatchLoadout` solely for passives, fighter resistance stats, weapon data, or ultimate activation style.
2. `MatchLoadoutProjection` installs `ResolvedPassives`, Balance Lab reconciles it atomically, and match readiness rejects a missing passive projection.
3. Adrenal Response, Quick Cycle, Close Quarters, Tenacity, resistance application, and passive telemetry retain their existing behavior and authored values.
4. Focused tests omit `ResolvedMatchLoadout` where the behavior only requires projected capabilities.
5. Formatting, linting, role-specific checks, affected server/network suites, and proportional canonical verification pass.
6. Evidence and a learn-from-errors note are recorded before closeout.

# Verification

- Focused build-projection, movement, ability-passive, attack-economy, effect-transaction, Balance Lab reconciliation, and match-readiness tests.
- `cargo fmt --all -- --check`
- `git diff --check`
- `just check`
- `just lint`
- Affected server and network suites, followed by `just test` if focused verification remains green.
- No native playtest is required unless implementation changes player-visible behavior; this is intended as an ECS dependency-only refactor.


# Implementation and closeout — 2026-08-30

- Added server-local `ResolvedPassives`, containing the exact two immutable resolved passive definitions, and installed it through `MatchLoadoutProjection` beside fighter stats, body, weapon, and ultimate.
- Passive trigger telemetry now consumes `ResolvedPassives` directly for Adrenal Response, Quick Cycle, and static active-tick accounting.
- Authoritative movement consumes `ResolvedFighterStats` plus `ResolvedPassives`; the shared prediction math accepts the exact optional Adrenal Response definition so the client can continue using its replicated aggregate without replicating the server-local projection.
- Primary-fire authority now queries `ResolvedWeapon`, `ResolvedUltimate`, and `ResolvedPassives` independently. Quick Cycle ammunition recovery no longer receives the aggregate loadout.
- The composed-effect transaction now indexes Close Quarters from `ResolvedPassives` and reads target maximum health/resistances from `ResolvedFighterStats` plus Tenacity from `ResolvedPassives`.
- Match readiness now requires seven atomic generation projections, including passives. Balance Lab reconciles projected passives in the same accepted transaction as the aggregate, stats, body, weapon, and ultimate.
- Focused passive observer and movement tests omit `ResolvedMatchLoadout`; projection, readiness, Quick Cycle, Close Quarters/Tenacity, resistance, Balance Lab, network, and performance coverage remain green.
- No protocol shape, content schema, fixed-tick schedule, balance value, or presentation behavior changed. No native playtest was required.

# Verification evidence

Passed after the final implementation state:

- `cargo fmt --all -- --check`
- `git diff --check`
- `just check` for routing, client, server, network-test, Balance Lab, and web tooling
- `just lint` with warnings denied plus server-feature, retired-renderer, and map-cleanup isolation gates
- `just test`: routing/process suites; 492 client tests; 439 server tests; 461 Balance Lab tests; revised-catalog replication; 94/94 network scenarios; 12/12 performance gates
- Focused Balance Lab atomic reconciliation and point-blank object-contact network scenarios pass.
- Exact scoped search confirms no `ResolvedMatchLoadout` references remain in `abilities/passives.rs`, `movement/authority.rs`, `combat/attack.rs`, or `combat/effects/`.

# Learn-from-errors review

- The first full network run exposed a point-blank object fixture that edited only the replicated aggregate weapon. Authority correctly read the projected weapon, so the test's intended damage override never reached gameplay. The fixture now reconciles the focused weapon projection from the edited aggregate. Prevention: tests that intentionally replace a resolved generation must replace the aggregate and every focused projection atomically, preferably through `MatchLoadoutProjection`.
- The first performance run exposed the same pre-existing fixture mistake more broadly: the sentry benchmark replaced the aggregate with a Sentry loadout while retaining the old Dash ultimate projection. It now installs the complete projection bundle and again observes four live sentries within budget. Prevention: never model a generation replacement by inserting only `ResolvedMatchLoadout`; use the production projection bundle and keep match readiness as the missing-capability guard.
- Reusable lesson: focused components make stale test state fail closed instead of silently following aggregate mutations. That friction is useful evidence that runtime ownership is explicit and atomic boundaries are being enforced.
