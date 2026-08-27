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
