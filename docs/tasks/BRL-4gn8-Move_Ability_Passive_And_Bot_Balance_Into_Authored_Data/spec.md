# Outcome

Player-facing ability/passive tuning and Practice-bot policy are authored, validated, fingerprinted data; runtime systems own algorithms and safety ceilings rather than balance literals.

# Scope

- Add complete authored parameters for Dash and Sentry, including Sentry projectile recipe/configuration.
- Move ultimate-charge policy and passive durations, percentages, thresholds, and resistance modifiers into validated balance definitions.
- Load a validated Practice Bot profile from a headless-safe content catalog/resource.
- Replace literal 60 Hz and duplicated input-staleness values with canonical timing and shared authoritative input policy.
- Resolve authored values once into runtime configuration/components and avoid duplicating projectile geometry or deadlines during spawn.
- Extend Balance Lab descriptors for the newly authored fields where Balance Lab already owns that tuning family.
- Preserve stable semantics, current built-in values, server authority, and bounded serialized sizes.

# Acceptance criteria

- Current built-in balance is reproduced from content with no player-visible drift.
- Dash/Sentry/passive/bot tuning changes require data edits rather than gameplay-system edits.
- Invalid or unsafe tuning is rejected at content load/resolution.
- Sentry shot geometry, collider, deadline, presentation, and damage derive from one resolved recipe.
- Practice bots continue to emit only ordinary FighterInput.

# Verification

- Definition validation and fingerprint tests.
- Focused Dash, Sentry, passive, charge, and bot-policy tests with non-default tuning.
- Balance Lab round-trip tests for exposed fields.
- Server-only build check and representative routed Practice test.


# Implementation evidence (2026-08-30)

- Authored and validated Dash, Sentry, ultimate-charge, passive, and Practice-bot policies; runtime snapshots derive Sentry geometry, deadlines, presentation, and damage from one resolved recipe.
- Replaced duplicated staleness and 60 Hz assumptions with shared timing/input policy and preserved server-only FighterInput emission.
- Extended Balance Lab schema, descriptors, migration, persistence, and round-trip coverage.
- Verification passed: `just test` (routing, 456 client tests, 391 server tests, 413 Balance Lab tests, revised-catalog routed scenario, 90 network scenarios, 12 performance gates), `just check`, `just lint`, and `git diff --check`.
- Learn-from-errors: requiring a resolved loadout in condition advancement accidentally filtered test entities; using the optional snapshotted Dash runtime preserved unrelated condition ownership. Canonical verification caught the regression. A stale 177 GiB target directory exhausted disk during the first run; `cargo clean` recovered rebuildable artifacts and the complete clean suite then passed.
