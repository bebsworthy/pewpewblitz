# Observed

`just rejoin-e2e shard-rush-3v3` on branch `claude/bold-fermi-964944` (BRL-wrer, 2026-11-21): the match worker panicked in `brawler::combat::effects::resolve_composed_payloads` at `src/combat/effects/application/transaction.rs:492` with `payload event reservation matches healing`, and the supervisor reported `WorkerCrash`. The Practice roster bots were fighting; no combat code differs from `main`.

# Likely cause

`required_payload_event_count` (`src/combat/effects/planning.rs`) reserves no event for `Heal` (or any non-damage effect) once the planned target is `defeated`. The transaction's `Heal` branch only skips `target.healing_blocked`, not `target.defeated`, so a heal applied after a same-batch defeat calls `reserved_events.next()` past the reservation and hits the `expect`. BRL-35ns fixed the same class of mismatch for defeat events.

# Scope

- Make the transaction skip effects on a defeated target exactly as the planner counts them, for heal and any other non-damage effect that consumes events. Prefer one shared predicate over two hand-kept copies.
- A focused combat test: one batch where a damage payload defeats a target and a heal payload then targets it.

# Acceptance

- The test reproduces the panic before the fix and passes after; `just lint`, `just test`.

# Resolution (2026-11-21)

- Confirmed cause: the planner (`required_payload_event_count`) counted 0 events for any non-damage effect on a target `defeated` earlier in the batch, while the transaction's `Heal` branch only skipped `healing_blocked`. Knockback/Slow/Cold/DoT were already consistent because the transaction skips `plan_runtime_effects` for a defeated target; only `Heal` drifted.
- Fix: one shared predicate `non_damage_effect_applies(effect, defeated, healing_blocked)` in `src/combat/effects/planning.rs`. The planner's non-damage arm counts `usize::from(predicate)`; the transaction's `Heal` branch skips on `!predicate`. Damage keeps its own path (`plan_damage_application` already returns `None` for a defeated target, matching the planner's `applied == 0` count).
- Test: `combat::effects::tests::composed_heal_after_same_batch_defeat_is_skipped_without_an_event` runs `resolve_composed_payloads` once on a `World` with a lethal hostile hit (contact 0.0) and an ally heal (contact 0.5) on the same target.

# Verification

- New test before fix: panicked at `src/combat/effects/application/transaction.rs:492:22` `payload event reservation matches healing` (exact production panic). After fix: passes; all 15 `combat::effects` tests pass.
- `just lint`: pass (after `cargo fmt` and a Clippy `nonminimal_bool` simplification).
- `just test`: pass (all suites, incl. network 140 and performance).
- `just rejoin-e2e shard-rush-3v3`: pass — "Practice shard-rush-3v3 rejoined the same running match after an outage beyond the session bound"; workers reaped `success=true`, no panic / `WorkerCrash` in the log (4 defeats in the run).

# Learnings

- Mistake class: planner and transaction each hand-encoded "does this effect apply" with different guards, so a gate added to one (defeat) silently missed the other. Prevention: event-consuming gates live in one named predicate used by both sides; damage/defeat counting already shares `plan_damage_application` math in the same way.
- The server feature is not default: focused `brawler` lib tests need `--no-default-features --features server`, otherwise the filter silently runs 0 tests.
