# V5 Milestone 02 — Dashboard-owned selection and connected-loop convergence

| Field | Value |
|---|---|
| Status | Researching |
| Depends on | V5 M01 accepted and completed on 2026-08-21 |
| Outcome | Dashboard-owned brawler and game-type selection plus queue, loading, match, and Results paths whose ordinary connected exits return to Dashboard |

## Research question

What is the smallest revision of the existing connected screen flow that makes build and game-type
selection true Dashboard children, preserves accepted queue/build authority, and returns every
ordinary connected exit to Dashboard without introducing a second navigation framework?

## Accepted product boundary

- Dashboard remains the authenticated connected home established by M01.
- Build and game-type selection need explicit confirm/back semantics and deterministic focus
  restoration; they must not become a carousel or a new client authority path.
- A queued build remains the server-validated frozen candidate. Editing a draft must not silently
  change an accepted queue request.
- Queue cancellation, loading/match-start cancellation, confirmed match leave, successful match
  completion, and Results exit should return to Dashboard while the lobby session remains valid.
- Unexpected connection loss and explicit disconnect continue to use the recovery paths established
  by M01.
- Results must retain authoritative outcomes. Play Again may reuse a valid selection, but Change
  Game and Disconnect should not remain duplicated there when Dashboard owns those choices.
- No screen may claim a selected map while the server advertises a map pool and formation owns the
  actual map choice.

## Research starting points

- `src/client/flow.rs`, `src/client/shell.rs`, `src/client/dashboard.rs`, and existing flow tests for
  screen ownership, overlays, focus restoration, and authenticated-session transitions;
- the existing build editor, game selection, queue/loading, pause/leave, and Results systems for
  their current state and action ownership;
- `src/client/session.rs` and routed lifecycle tests for lobby validity, admission cancellation,
  match handoff, result return, and unexpected-loss behavior;
- `docs/13-player-ux.md`, `docs/14-multiplayer-server-architecture.md`, and completed V2 milestone
  evidence for enduring navigation and authority decisions;
- checked-in Bevy 0.19 state/UI examples and local Lightyear lifecycle material where exact APIs or
  ordering need confirmation.

## Research checklist

- inventory every current transition into and out of Build Select, Game Select, queue, loading,
  match, pause/leave confirmation, and Results;
- identify which state owns the accepted build, editable draft, selected advertised game type,
  lobby validity, and focus-return target at each transition;
- determine the smallest action/state changes needed to make Dashboard the sole connected home;
- specify loss, cancellation, stale-advertisement, rejected-build, and worker-handoff recovery;
- define focused state/ECS tests and representative routed lifecycle cases;
- record the native playtest scenarios needed to validate confirm/back and input parity.

## Specification gate

Research findings, the exact state-transition contract, implementation checklist, verification plan,
and playtest handoff will be added here before the milestone advances to `Specification review`.
Production implementation does not begin until that specification is accepted.
