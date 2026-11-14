# Outcome

The current observable contracts of the high-risk coordinators are captured by focused tests so later decomposition can prove semantic and scheduling equivalence.

# Scope

Characterize:

- client flow action precedence and the profile/queue/practice/match branches of resolve_flow_action;
- projectile sweep behavior for straight, lobbed, splash, sticky, world-object, and cleanup outcomes;
- worker control inbound/outbound ordering, sequencing, backpressure, heartbeat, EOF, and shutdown behavior;
- Sentry acquisition, visibility revalidation, objective targeting, firing cadence, and cleanup;
- schedule ordering and ApplyDeferred boundaries used by those transactions.

Reuse production types, canonical harnesses, and deterministic fixed-time advancement. Do not refactor production coordinators except for a minimal demonstrated test seam.

# Acceptance criteria

- Each coordinator has representative happy-path, precedence/ordering, rejection, and cleanup/backpressure coverage.
- Tests assert externally meaningful state/facts/messages rather than private implementation steps.
- Time-dependent tests advance Bevy fixed time or schedules without wall-clock sleeps.
- The suite is bounded and avoids Cartesian combinations.
- Known uncharacterized behavior is documented in this ticket before closeout.

# Verification

- Focused unit/ECS suites for each coordinator.
- Relevant network/routed tests for worker and authority boundaries.
- Schedule ambiguity checks for owned schedules.
- Canonical test command defined by the repository.


# Characterization evidence (2026-08-30)

- Client flow: production-schedule tests lock profile-decision handling, explicit > session > ordinary precedence, teardown/ApplyDeferred/commit behavior, selected-brawler requirements, and queue/practice exclusion.
- Projectile delivery: existing routed tests remain the authority boundary for straight, lobbed, splash, sticky, first-tick, cover, object, and disconnect behavior; a focused test now locks exclusive MapObject versus HeistSafe damage routing. No Cartesian duplication was added.
- Worker control: real UnixStream tests lock inbound ordering/duplicates/gaps/EOF, outbound priority and contiguous sequence ownership, backpressure retention, deterministic heartbeat deadlines, and Last Result -> Exit -> AppExit flushing.
- Sentry: deterministic production-system ECS tests lock cadence, target/visibility revalidation, fighter/objective priority, authored projectile facts, cleanup priority/message purge, and cleanup ApplyDeferred before due fire.
- Owned Update, Last, FixedUpdate, and FixedPostUpdate ambiguity gates are active in the fixtures.

# Known intentionally uncharacterized behavior

- Copy-only UI branches and every individual profile rejection variant remain outside this bounded matrix; representative durable overlay/state transitions are covered.
- Every projectile recipe combination is not duplicated locally because canonical network tests already exercise each delivery family and authority boundary.
- OS-specific partial-write chunk sizes are not asserted; the real nonblocking stream contract is characterized through bounded queue saturation and subsequent ordered delivery.

# Verification and learning

- Passed focused suites (45 client-flow, 20 ability, 16 worker, projectile routing), full 460 client / 404 server / 426 Balance Lab unit suites, 90 network scenarios, 12 performance gates, `just check`, `just lint`, and `git diff --check`.
- Learn-from-errors: an empty allocation fixture tested validation failure instead of backpressure; replacing it with a valid two-participant allocation isolated the intended contract. A combined 105-line flow case triggered Clippy and was split into three named invariant tests with shared setup.
