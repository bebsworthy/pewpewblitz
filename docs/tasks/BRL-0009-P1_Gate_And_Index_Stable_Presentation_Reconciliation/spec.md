# Scope

Measure and reduce stable per-frame reconciliation in client 3D presentation without changing visual behavior or authority.

# Acceptance

- Add a representative client diagnostic or benchmark for a maximum stable dynamic map.
- Gate topology work on generation/revision, terminal-state, readiness, and addition/removal changes.
- Index placements once per accepted snapshot/revision.
- Update damage materials only when health/life state changes.
- Keep required per-frame projection while caching durable object-to-UI joins.
- Remove redundant safe/pickup identity sets.
- Capture before/after evidence and pass native visual, recovery, and role checks.

# Constraints

Do not move presentation state into authority or add a cache without a clear invalidation owner.

## Implementation progress (2026-08-27)

- Removed redundant safe and restoration-pickup identity sets from stable reconciliation.
- Added Bevy change-detection gates so safe and barrel materials are only reassigned when the visual is added or health/life inputs change.
- Added a dynamic-map reconciliation stamp keyed by accepted root, map instance, generation, dynamic revision, imported-asset readiness, and materialized visual count. Stable frames now skip terminal-map construction and topology scans.
- Indexed accepted placements once per reconciliation pass instead of performing a nested placement scan for every existing visual.

Verification and representative before/after measurement remain in progress.

## Additional implementation and verification (2026-08-27)

- Added a durable damageable-health UI key-to-entity index; projection still runs every frame, while object-to-widget ownership no longer reconstructs an existing-key set.
- Restoration pickups now spawn only on component addition; per-frame work is limited to required bob/rotation and stale-owner cleanup.
- Added a representative maximum-built-in-dynamic-map diagnostic: 600 stable frames produce one topology reconciliation, while revision, readiness, and removal-count changes each invalidate exactly once.
- Combined client/server and network-test checks and combined Clippy pass. The stable-map diagnostic passes.

## Final verification in this pass (2026-08-27)

- Full combined client/server library suite passes: 581 tests.
- Network loadout/authority/restart scenarios pass after the schedule and presentation changes.
