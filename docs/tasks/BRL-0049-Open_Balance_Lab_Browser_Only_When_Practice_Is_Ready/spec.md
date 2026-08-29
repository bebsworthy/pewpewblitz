# Scope

- Remove the eager `open` call from the Balance Lab launcher.
- Retain the bounded readiness loop that opens the configured URL once after the Practice worker serves it.
- Keep manual-open error reporting and process cleanup behavior.
- Update operator documentation so it describes the single deferred open.

# Acceptance criteria

- `just balance-lab` does not open a browser before Practice starts.
- Entering Practice opens the configured Balance Lab URL once when its HTTP endpoint becomes reachable.
- Shell syntax validation and repository whitespace checks pass.

# Implementation and verification

Removed the eager browser-open branch from `scripts/dev.sh`. The retained readiness subprocess polls the configured endpoint while the supervisor lives, opens the URL once after the first successful response, reports a manual URL if that open fails, and remains owned by the existing cleanup trap.

Updated `README.md` and `docs/15-balance-lab.md` to describe the deferred single-open workflow.

Verification:

- `bash -n scripts/dev.sh` — passed.
- Static assertion confirms exactly one Balance Lab `open` call remains — passed.
- `git diff --check` — passed.
