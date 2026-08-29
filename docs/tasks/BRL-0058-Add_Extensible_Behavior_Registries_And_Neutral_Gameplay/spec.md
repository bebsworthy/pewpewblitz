# Outcome

A new supported ability behavior, game mode, or world-object terminal reaction can be added through an owned plugin/descriptor/handler and bounded schema change without rewriting unrelated coordinators.

# Scope

- Define a minimal neutral authoritative phase vocabulary in gameplay.rs for delivery/effects, environment, objectives, visibility, publication, and finalization where current cross-domain ordering proves the need.
- Split ability behavior registration into focused plugins that install systems into stable AbilitySet/neutral phases.
- Centralize process-local mode descriptors that associate stable mode identity/key with rules resolution, compatible-map policy, server installation, and optional presentation/bot projection hooks.
- Refactor world-object terminal resolution into deterministic terminal plans and additive reaction handlers while retaining the exclusive atomic mutation transaction.
- Introduce generic owner/disconnect/restart cleanup hooks so matchplay/networking do not call Sentry-specific cleanup functions.
- Preserve stable serialized enums/IDs where wire compatibility requires them; do not put trait objects in replicated schemas.
- Do not implement a new game mode, ability, or terminal behavior solely to prove the abstraction unless a minimal test fixture is necessary.

# Acceptance criteria

- Existing abilities, modes, and terminal reactions register through the new owned boundaries.
- Adding a test behavior requires additive registration and does not edit unrelated existing behavior algorithms.
- The fixed-tick transaction remains deterministic and schedule ordering is explicit.
- Mode/map compatibility, protocol fingerprinting, authority, and bounded-state contracts remain enforced.
- No opaque dynamic dispatch obscures protocol or schedule ownership.

# Verification

- Plugin/registry composition and duplicate/missing-registration tests.
- Schedule-order and ambiguity tests.
- Existing ability, match-mode, map-object, restart, disconnect, and routed authority suites.
- Protocol/content fingerprint assertions when registered shapes change.

## Implementation evidence (2026-08-30)

- Added the neutral fixed-post `AuthoritativePhase` chain: delivery, effects, environment, objectives, visibility, publication, and finalization. Existing domain sets remain focused children and retain their concrete ordering constraints.
- Split authoritative ability composition into focused core, Dash, Sentry, Stealth, impact, and outcome plugins. Generic cleanup facts and additive cleanup schedules replace Sentry-specific calls from matchplay and networking while retaining the same lifecycle boundaries.
- Added a process-local mode descriptor registry that owns stable config/key/definition/rules/map/routing identity and static server installation, plus feature-gated UI and bot projections. Admission, lobby, worker, map compatibility, client selection, and bots now consume descriptors; wire enums and protocol fingerprints are unchanged.
- Added a stable-ID world-object terminal reaction registry. Exclusive terminal resolution now creates deterministic immutable plans and invokes additive explosion/pickup handlers inside the same bounded atomic transaction.
- Added additive, duplicate, missing, coverage, projection, and schedule-order tests for the new extension boundaries.

## Verification evidence (2026-08-30)

- `just test`: passed — routing, 465 client tests, 410 server tests, 432 Balance Lab tests, the revised-catalog routed scenario, all 90 network scenarios, and all 12 performance gates.
- Focused suites passed: abilities 21/21, matchplay 19/19, map runtime 8/8, modes 3/3 per client/server role, admission 12/12, lobby catalog 4/4, bots 22/22, client flow 45/45, and protocol fingerprint coverage.
- `just check`: passed for routing, client, server, network-test, and Balance Lab feature graphs.
- `just lint`: passed with strict Clippy for every supported role and all architecture guard scripts.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

## Learn-from-errors review

- The first ability plugin split duplicated Self Cloak outcome registration and omitted deferred boundaries from the monolithic chain. Review restored one outcome registration and explicit activation/cleanup flushes before integrated tests.
- A hand-built Sentry characterization fixture bypassed the new generic cleanup-fact publisher. Updating the fixture to use the production fact boundary preserved its same-tick assertion and exposed the new dependency clearly.
- Bevy reported redundant schedule hierarchy edges when systems were assigned to both child movement sets and their parent. Removing the duplicate parent memberships retained the parent relation through set nesting and eliminated all schedule warnings.
- Feature-specific mode projections initially caused unused imports/fields in the opposite role. Feature-gating those descriptor fields and imports preserved the dedicated-server boundary without lint allowances.
