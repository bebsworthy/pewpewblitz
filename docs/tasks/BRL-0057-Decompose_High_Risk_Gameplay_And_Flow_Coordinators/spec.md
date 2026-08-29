# Outcome

Four high-risk coordinators retain their existing schedule-facing contracts but delegate focused, independently testable planning and commit responsibilities.

# Prerequisite

The characterization coverage ticket linked to this work must establish the current observable behavior before material decomposition.

# Scope

- Keep resolve_flow_action as the precedence owner while extracting narrow session, connection, profile, brawler, persistence, and matchmaking reducers.
- Split projectile sweep into deterministic candidate/index collection and trajectory-specific straight, lobbed, splash, and sticky plans plus shared world-target damage construction.
- Split worker control into ordered receive, match outbox, lobby outbox, heartbeat, and flush stages with one shared encode/enqueue helper.
- Split Sentry owner/target indexing, visibility/LOS, target selection, fire planning, projectile emission, and cleanup helpers/systems.
- Preserve explicit SystemSet, chain, ApplyDeferred, physics, replication, and Last-schedule relationships at composition points.
- Avoid introducing service layers, trait objects, or one-plugin-per-type abstractions without a second use.

# Acceptance criteria

- No schedule-facing coordinator mixes unrelated validation, planning, mutation, telemetry, and publication responsibilities.
- Public/wire contracts and authoritative outcomes are unchanged.
- Existing characterization tests pass without weakening assertions.
- Duplicated predicates, literals, and world-target construction identified by the audit are removed.
- Composition roots remain readable and make ordering/deferred boundaries visible.

# Verification

- All characterization coverage from the prerequisite ticket.
- Focused subsystem tests after each extraction.
- Schedule ambiguity checks.
- Role-specific checks, relevant network tests, and canonical repository verification.

## Implementation evidence (2026-08-30)

- `resolve_flow_action` remains the precedence owner and now delegates profile, explicit/session, connection/queue, persistence, brawler/equipment, and match-navigation decisions to focused reducers.
- Projectile sweeping now separates immutable fighter/object indexing, straight and lobbed trajectory planning, direct world-damage construction, fighter payload queuing, and impact publication from the schedule-facing coordinator.
- Worker control now delegates receive/dispatch, stop handling, match and lobby outboxes, shared encode/enqueue, and heartbeat work while preserving the existing `Last` chain.
- Sentry orchestration now separates indexing, visibility/LOS, target selection/revalidation, fire planning/commit, and cleanup/coalescing. Existing schedule composition and deferred-command boundaries are unchanged.

## Verification evidence (2026-08-30)

- `just test`: passed (routing, 462 client tests, 405 server tests, 427 Balance Lab tests, revised-catalog network scenario, all 90 network scenarios, and 12 performance gates).
- Focused projectile network coverage passed: four `combat_projectiles` scenarios, `combat_splash`, and disconnect-before-sweep lifecycle coverage.
- Focused flow tests passed 45/45; worker tests 16/16; Sentry tests 8/8; abilities tests 20/20.
- `just check`: passed for routing, client, server, network-test, and Balance Lab feature graphs.
- `just lint`: passed, including strict Clippy for every supported role and architecture guard scripts.
- `git diff --check`: passed.

## Learn-from-errors review

- A flow helper initially attempted to return data borrowed from a transient profile entry; returning stable saved-brawler identity and revision instead made ownership explicit.
- Strict Clippy exposed an oversized helper and unnecessary clone/pass-by-value choices; splitting the helper and tightening ownership kept the extracted stages reviewable.
- The first projectile-index extraction accidentally merged two independent bundle predicates. Restoring the exact contact-delivery and affecting-payload predicates preserved multi-bundle collision semantics before canonical verification.
