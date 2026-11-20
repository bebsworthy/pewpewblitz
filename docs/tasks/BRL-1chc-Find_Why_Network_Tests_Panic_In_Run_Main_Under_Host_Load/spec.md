# Found (BRL-01c8, 2026-11-20)
During BRL-01c8 the first full `just test` at load average ~9 had 8 network test failures (`lobby_takeover::*`, `map_publishing::*`, `queue::a_handed_off…`), each a bare `run_main` panic with no message in the captured output. All passed alone, then the full network suite passed 136/136 and a second `just test` passed. Same class as BRL-xbhb (a system failing validation when wall-clock time yields zero or several fixed steps).

# Plan
Reproduce under load (`yes` processes on every core) with `RUST_BACKTRACE=1 --nocapture`, capture the real error, and fix the cause (harness time stepping or a missing resource), not the symptom.

# Acceptance
Root cause recorded; the network suite passes 5 consecutive full runs under heavy load.
