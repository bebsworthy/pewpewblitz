# Context
Found during BRL-0208 verification (2026-11-14). The first `just test` run failed
`prediction::dash_converges_without_snapping` (tests/network/prediction.rs:301, "dash: no visible
teleport"; reported largest frame step 40.755). It passed 3/3 in isolation and in the immediately
repeated full `just test`. BRL-0208 did not touch prediction or movement code. Its harness change,
skipping suspended clients in `Harness::step`, leaves non-suspended clients unaffected.

# Scope
- Reproduce (loop the full network suite or the test under load) and find the nondeterminism
  (wall-clock dependence, test ordering, or a threshold too tight for the dash step).
- Fix the cause; do not just loosen the assertion without evidence.

# Acceptance criteria
- The test passes in repeated full-suite runs (for example 10 consecutive `just test` network runs).
