# Outcome

Authoritative systems that need fighter stats, primary-weapon identity, or loadout-generation presence query those focused server-local runtime components directly instead of reaching through `ResolvedMatchLoadout`.

# Scope

- Change attack-idle health recovery to consume `ResolvedFighterStats`.
- Change public participant weapon projection to consume `ResolvedWeapon`.
- Change sentry owner visibility indexing to consume `ResolvedFighterStats`.
- Change ready-command admission to use a focused runtime projection as the proof that the resolved loadout generation is installed.
- Narrow the pure fighter-runtime constructor to its fighter-stat and weapon inputs if that removes an aggregate-only dependency cleanly.
- Add focused characterization proving these systems operate without `ResolvedMatchLoadout` when their required projection exists.
- Preserve `ResolvedMatchLoadout` at intentional replication/convergence, lobby/control-plane, evidence/verification, build-resolution, and Balance Lab reconciliation boundaries.

# Decisions and constraints

- This is an organization-only ECS dependency change. No wire shape, fixed-tick ordering, gameplay value, or balance policy changes.
- `MatchLoadoutProjection` remains the atomic installation owner. Do not add duplicate runtime state or fallback defaults.
- Use the smallest focused component that represents the system's real dependency; do not introduce a generic capability framework.
- Preserve client/server feature isolation and existing public paths unless a focused re-export is required by integration tests.

# Acceptance criteria

1. The scoped authoritative systems no longer query or accept `ResolvedMatchLoadout` solely to reach fighter stats, weapon identity, or loadout presence.
2. Focused tests prove recovery, public participant projection, sentry visibility, and ready admission work from their projected components without the aggregate.
3. Existing runtime projection installation/reconciliation tests remain green.
4. Formatting, linting, client/server checks, and affected test suites pass.
5. Verification evidence and a learn-from-errors note are recorded before closeout.


# Implementation

- `restore_attack_idle_health` now queries `ResolvedFighterStats` directly.
- public participant roster projection now queries `ResolvedWeapon` for the stable preset identity.
- sentry owner indexing and visibility use `ResolvedFighterStats` directly.
- match ready-command admission now treats the atomically installed `ResolvedWeapon` projection as the resolved-generation capability.
- `resolved_fighter_runtime` accepts only `ResolvedFighterStats` and `ResolvedWeapon`; aggregate producers pass those explicit fields.
- Focused characterization removes `ResolvedMatchLoadout` from recovery, public projection, sentry-owner, and ready-admission fixtures.

# Verification evidence

- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.
- focused recovery, public participant projection, sentry cadence/visibility, and ready-admission tests — passed.
- `just check` — passed for routing, client, server, network-test, Balance Lab, and web tooling.
- `just lint` — passed with warnings denied plus server-feature and retired-renderer/map isolation gates.
- `just test` — passed: 83 routing library tests plus routing process suites; 491 client tests; 434 server tests; 456 Balance Lab tests; revised-catalog replication; 94 network scenarios; 12 performance gates.
- No native playtest required: this slice changes query dependencies only and the complete authoritative/network suites preserve gameplay values and behavior.

# Learn-from-errors review

- A broad textual patch initially matched the sentry activation query instead of the owner-index query because both used the same aggregate type. The immediate scoped `rg` review caught it before verification. Prevention: patch repeated Bevy query shapes with surrounding function context and re-run a scoped dependency inventory before compiling.
- The first network command combined a substring test name with `--exact`, producing zero executed tests. The command was corrected to the module-discovered substring form and the intended scenario then ran and passed. Prevention: confirm the reported test count for filtered Cargo invocations; zero tests is not evidence.
- Reusable lesson: focused runtime components make both authority dependencies and negative characterization stronger—tests can delete the replicated aggregate entirely and prove the system still owns exactly the capability it needs.
