# Scope

Add development/test verification for ambiguities at Brawler-owned schedule composition boundaries.

# Acceptance

- Owned schedule tests enable ambiguity detection with owning-set reporting.
- Start at Warn for inventory and graduate reviewed schedules to Error.
- Every allowed ambiguity is explicit, local, and documented.
- Third-party internal schedules are not blindly forced under a global policy.
- Critical fixed-tick, Lightyear, Avian, and presentation boundaries have no unreviewed ambiguity.
- Canonical role and schedule tests pass.

# Constraints

This identifies missing edges; it does not justify broad chains. Coordinate with SCHED-01.

## Local API verification

- Bevy 0.19.1 `ScheduleBuildSettings` defaults ambiguity detection to `Ignore` and supports owning-set reports: `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ecs-0.19.1/src/schedule/schedule.rs`.
- Bevy 0.19.1 `App::edit_schedule` creates or edits one named schedule, while `configure_schedules` affects every schedule currently present. The focused API is therefore the correct boundary for Brawler-owned checks: `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_app-0.19.1/src/app.rs`.

## Implementation

- The shared test helper enables `LogLevel::Error` and owning-set reporting on one explicit schedule label only. A focused test proves another schedule retains Bevy's default `Ignore` policy.
- Reviewed schedule tests gate client and server session `Update`, client combat and world-presentation `Update`, gameplay and ability `FixedUpdate`/`FixedPostUpdate`, match/Avian/combat `FixedUpdate`/`FixedPostUpdate`, and concealment `FixedPostUpdate`.
- The initial Warn inventory found an accidental insertion-order assumption between the match physics probe and projectile sweep. The probe now expresses the real physics-before-sweep boundary.
- The Warn inventory also found a false order between independent concealment prerequisites. Separate resources now verify that both prerequisites complete before source resolution without serializing them against each other.
- No ambiguity suppression was added; allowed independence is modeled with disjoint test state instead of global component/resource exceptions.

## Verification (2026-08-28)

- All eight owned schedule contracts passed first with Warn inventory and then with ambiguity detection promoted to Error.
- Strict client and server Clippy gates passed with warnings denied.
- Client tests passed: 414/414.
- Server tests passed: 331/331.
