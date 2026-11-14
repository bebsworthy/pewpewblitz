# Outcome
The local fighter's first step after a match starts doesn't snap backward.

# Context
BRL-0169 found the cause in native traces. Every Practice match has exactly one owner reconciliation correction, on the first walking tick after start: 2.3–4.7 u, behind the fighter. It is the existing late-arriving-input misprediction. Evidence: scratchpad/brl0169-vibrate/trace-after2-*.

# Acceptance criteria
- The first walk after match start reconciles with no visible correction (offset < 0.1 u).
- There's a regression test in tests/network/prediction.rs.
