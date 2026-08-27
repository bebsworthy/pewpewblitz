# Scope

Apply the exact Balance Lab export supplied by the user to `content/catalogs/builds.ron` and `content/catalogs/weapons.ron`. UI durations are seconds; persisted catalog durations remain authoritative 60 Hz ticks: 1.0 s = 60, 1.2 s = 72, and 1.6 s = 96 ticks.

Update enduring documentation that states canonical defaults. Do not alter unlisted fighter profiles, weapons, firing angles, effects, or validation policy.

# Acceptance

- Every listed server default resolves to the supplied value.
- Embedded catalogs validate and fingerprints update naturally.
- Existing builds and weapon-part combinations remain legal or any intended incompatibility is surfaced.
- Relevant library, client-only, server-only, and strict Clippy checks pass.
- Unrelated user worktree changes remain untouched.

# Implementation evidence — 2026-08-27

Applied all listed values exactly. Converted displayed durations to authoritative ticks: Pulse 60, Scatter 72, Arc 96, and Blade 60. Advanced the build balance revision from 4 to 5 so persisted selection validation and loadout fingerprints observe the changed fighter defaults. Updated the fighter, weapon, and Balance Lab specifications and added exact embedded-default regression coverage.

Verification passed:

- 590 library tests.
- 11 focused build-catalog tests and 12 weapon-definition tests.
- All four starter weapon-part combination tests.
- Client-only and server-only checks.
- Strict client/server Clippy.
- Routed Pulse (4), recovery (6), and build/match (6) network scenarios.
- `git diff --check` and ticket mirror sync.

Network fixtures that intentionally exercise current Pulse content now assert radius 2, 200 authored damage with target-health clamping, four-round reset economy, and the shorter live range. The late-join projectile test uses an explicit long-lived test-local flight so it continues to test replication rather than shipping travel duration.

Correction: the final sentence above refers to an explicit long-lived test-local flight.
