# Problem

`scripts/network-product-match.sh` defaults `BRAWLER_ROUTED_TIMEOUT_SECONDS` to 60 seconds, while the current `wipeout-1v1` configuration has a 180-second authoritative match limit and ten-elimination target. The requeue smoke can reach Active cleanly yet be killed by its watchdog before a legitimate terminal result. On 2026-08-28 the default and 90-second runs timed out; the identical run with a 220-second watchdog completed at about 187 seconds and passed fresh-lobby requeue.

# Specification

- Make the requeue smoke's default completion bound derive from or explicitly cover the current authoritative game duration plus bounded routing/cleanup margin.
- Prefer a deterministic faster terminal trigger if one already exists in production verification paths and preserves the real Results-to-requeue lifecycle; do not weaken match authority or fake the client-side result.
- Keep normal product Active smokes fast and retain explicit environment overrides.
- Emit enough phase evidence to distinguish failure to complete, failure to return to lobby, and failure to obtain the new Joined outcome.

# Acceptance criteria

- The documented default requeue command passes without a timeout override.
- The smoke observes an authoritative terminal result, match-worker cleanup, fresh lobby authentication, and a new queue Joined outcome for both clients.
- The watchdog remains bounded and reports the failed lifecycle phase on timeout.
- Normal 1v1 Active and Practice routed smokes remain green.
- Focused script/config tests plus relevant routing, client, server, and network tests pass.
- Verification and learn-from-errors evidence are recorded before completion.

# Non-goals

- No gameplay balance, match-duration, protocol, queue, or result-authority change solely to shorten the smoke.
