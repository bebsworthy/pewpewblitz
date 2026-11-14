# Outcome

Automation client logs stay readable: the "headless movement automation enabled" info line is written once per run.

# Context (found during BRL-1fam)

- `packages/brawler-client/src/automation.rs` logs the line while `automation.elapsed_ticks == 0`.
  `elapsed_ticks` stays 0 until the controlled fighter exists, so a Practice render-evidence
  client logged it 1,773 times in one ~20 s run (`weapon-7.txt.client-1.log`).

# Scope

- Log once (for example a `Local<bool>` or the first tick that writes the input), keeping the
  same fields. Automation-feature code only; no gameplay or input change.

# Acceptance criteria

- One line per run in a Practice render-evidence client log.
- `just lint` and `just test` pass.
