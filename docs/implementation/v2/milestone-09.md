# Milestone 09 — Minimal v2 closeout

## Tracking

| Field | Value |
|---|---|
| Status | Complete |
| Prepared | 2026-08-20 after the user accepted four rounds of bot and multiple-client gameplay |
| Objective | Close v2 with one final regression pass and truthful records, without speculative hardening or optimization |
| Entry dependency | Satisfied: M01–M08 are complete |
| Scope authority | The user explicitly reduced M09 to the bare minimum and returned production-scale work to the backlog |
| Specification validation | Approved 2026-08-20 when the user directed M09 implementation |

## Scope

M09 performs only these actions:

1. Run the existing canonical `just lint`, `just test`, and `just e2e 2` paths once against the
   completed v2 tree.
2. Fix only a blocking regression found by that pass. Any scope-expanding or non-blocking finding
   goes to the backlog.
3. Record the final verification result, known limitations, feedback disposition, and concise
   learn-from-errors review; then ask the user to accept v2 closeout.

M09 adds no product feature, protocol, architecture, abstraction, dependency, benchmark harness,
optimization target, soak campaign, exhaustive device/layout matrix, security program, production
credential system, match resumption, or legacy direct-UDP cleanup.

## Exit criteria

- [x] The three canonical commands pass on the final tree.
- [x] No known blocking regression remains undisposed.
- [x] Final limitations and reusable learning are recorded without claiming production readiness.
- [x] The user accepted v2 closeout on 2026-08-20.

Production/network optimization, full performance and CPU campaigns, dual-stack MTU research,
fleet and internet-facing hardening, production credentials, exhaustive manual matrices, match
resumption, lifecycle soaks, and direct-UDP retirement remain in the roadmap backlog until a real
product or deployment need justifies them.

## Verification evidence — 2026-08-20

| Command | Result |
|---|---|
| `just lint` | Passed formatting, routing/client/server Clippy with warnings denied, and dedicated-server feature isolation |
| `just test` | Passed routing, client, server, 82 network integration, and 14 fixed-tick performance tests |
| `just e2e 2` | Passed one real routed exact 1v1 through supervisor, lobby, match worker, both clients, authoritative Active, and clean worker reap |
| `git diff --check` | Passed after the closeout documentation update |

No blocking regression was found, so M09 changes no production code.

The Lightyear late-input and invalid-sequence messages emitted by impairment/soak scenarios remained
expected diagnostic output inside passing tests; they did not fail a test or expose a new player
defect.

## Known limitations

- This is a development-ready v2 milestone, not a production-hosting or release-readiness claim.
- Practice bots remain deliberately inert and multiplayer queues remain human-only.
- Match resumption, public internet hosting, production credentials, fleet operation, exhaustive
  device/layout coverage, extended lifecycle soaks, and routed optimization remain backlog work.
- The direct-UDP comparison path remains available until its debugging value no longer justifies
  keeping it.

## Feedback disposition

- The user's four gameplay rounds with bots and multiple clients closed the outstanding M05/M08
  playtest gates before M09 implementation.
- The request to remove premature optimization from M09 was implemented in the roadmap and
  milestone contract.
- No M09 verification defect required implementation. The user accepted M09 and directed v2
  closeout on 2026-08-20.

## Learn-from-errors review

- Closeout work should prove that the delivered product still composes; it should not create a
  production-hardening project without a production requirement.
- A missed diagnostic target remains recorded honestly, but does not justify optimization until it
  causes a measured player, correctness, or deployment problem.
- One canonical regression pass plus hands-on gameplay is sufficient evidence for the current
  development stage. Broader campaigns stay visible in the backlog instead of silently becoming
  completion requirements.

## Closeout — 2026-08-20

The user accepted M09 and directed v2 closeout after the minimal regression gate passed. M09 and v2
are complete. This is a development-ready player-experience milestone, not a production-hosting or
release-readiness claim; the roadmap backlog retains every deferred item.
