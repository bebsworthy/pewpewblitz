# Context

Deferred from BRL-pw9k, which ships `--shell-capture <route>` (`packages/brawler-client/src/flow/automation.rs`, `scripts/shell-capture.sh`) for the 19 shell routes reachable from an idle Dashboard. These routes were not included:

- `results`: needs a completed match. A Practice match lasts up to 180 s (Heist bots end it in 18-40 s, but not reliably), so a route would either wait minutes or need a server-side short-match rule. Leaving with `ConfirmLeaveMatch` releases the seat and does not show Results.
- `match-menu`: the in-match menu is `ClientInputContext::Menu` with a `MatchMenuState` selection (`presentation.rs`), not a `FlowUiAction`. Opening it from the driver needs a new flow action or a second navigation path, which BRL-pw9k's scope rules out.
- `leave-confirmation` (`ClientOverlay::LeaveConfirmation`) is set from that match menu, so it has the same problem.
- `match-loading`: transient, so it needs a mid-transition capture policy.

# Plan sketch

1. Add a Practice prefix to the route table: `OpenPracticeMaps` -> `StartPractice`, waiting up to the formation deadline for `ClientFlow::Match`.
2. Decide how the match menu is opened canonically (for example a `FlowUiAction::OpenMatchMenu` that the existing Escape/Start path also uses) before adding `match-menu` and `leave-confirmation` routes.
3. For Results, choose either a bounded wait on a Heist Practice match or an operator match rule that ends quickly, plus capture offsets inside the Results reward burst.

# Acceptance

- `just shell-capture` adds PNG/JSON pairs for the new routes at 1280x720 and 914x411, with no second navigation path.
