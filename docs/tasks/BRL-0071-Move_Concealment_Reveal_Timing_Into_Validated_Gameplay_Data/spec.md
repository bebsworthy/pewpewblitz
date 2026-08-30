# BRL-0071 specification

## Outcome

Attack and damage reveal-lock timing has one authored, validated, fingerprinted source under `content/catalogs/`. Server concealment authority reads a focused immutable resource installed by `GameplayContentPlugin`; the old gameplay constants are removed.

## Scope

1. Add a headless-safe `ConcealmentRules` schema with:
   - exact schema version;
   - `attack_reveal_ticks`;
   - `damage_reveal_ticks`.
2. Add an embedded RON document preserving `90` and `120` ticks.
3. Validate non-zero durations against a code-owned engine ceiling. Do not impose a relative ordering between the two balance values.
4. Provide canonical fingerprint material and a `ConcealmentRulesResource` initialized by a small content plugin.
5. Install the content plugin from `GameplayContentPlugin` and include its canonical material in the global gameplay-content envelope. Advance the envelope version because stale peers must fail closed.
6. Make `observe_attack_and_damage_reveal_locks` consume `Res<ConcealmentRulesResource>`.
7. Remove the old public timing constants and update imports/tests.
8. Add focused tests for embedded values, invalid zero/out-of-bound durations, fingerprint sensitivity, content-plugin installation, and runtime deadline application.
9. Update durable concealment/network documentation only where it currently names a code-owned timing source.

## Constraints

- Preserve server authority, fixed-tick placement, fact consumption, deadline max/refresh behavior, and end-tick exclusivity.
- Preserve values 90/120; this is source-of-truth migration, not a balance pass.
- Keep the rules shared/headless-safe and free of client assets or presentation types.
- Do not change wire message/component shapes or add compatibility decoders.
- The maximum allowed duration is an engine safety bound and remains in Rust.
- Do not alter unrelated concealment sources, Reveal Scan, proximity reveal, or presentation relevance.

## Verification

- Focused concealment rules and runtime tests.
- `cargo test --no-default-features --features server --lib concealment`
- `cargo test --no-default-features --features client --lib content::tests`
- `cargo check --no-default-features --features server --lib`
- `cargo check --no-default-features --features client --lib`
- `cargo fmt --all`
- `git diff --check`

Run `just check` if focused role checks expose composition or feature-gate issues. No native evidence is required because values and presentation remain unchanged.

## Acceptance criteria

- [ ] No production reveal-lock duration constant remains.
- [ ] Embedded rules validate and preserve 90/120 ticks.
- [ ] Invalid zero, excessive, or wrong-schema rules fail deterministically.
- [ ] The rules participate in the global gameplay-content fingerprint and a value change alters that fingerprint.
- [ ] `GameplayContentPlugin` installs the rules resource without installing protocol or presentation plugins.
- [ ] Server attack/damage observations use the authored resource and preserve max/refresh semantics.
- [ ] Server and client role checks pass with no feature leakage.
- [ ] Verification evidence and learning are recorded before closeout.


## Implementation record — 2026-08-30

Implemented the concealment timing source-of-truth migration:

- added validated `content/catalogs/concealment.ron` with preserved 90/120 values;
- added headless `ConcealmentRules`, its resource/content plugin, schema validation, and local fingerprint tests;
- installed it through `GameplayContentPlugin`, advanced the gameplay-content envelope from 25 to 26, and included both durations in the global fingerprint without changing the public fingerprint API;
- changed server authority to consume the resource and removed the old constants;
- strengthened protocol/content isolation, custom deadline/refresh, deferred schedule visibility, and global fingerprint sensitivity tests;
- documented the authored source in `docs/17-concealment.md`.

Verification passed:

- `cargo test --locked --no-default-features --features server --lib concealment` — 16 passed;
- `cargo test --locked --no-default-features --features client --lib content::tests` — 2 passed;
- `cargo test --locked --no-default-features --features client --lib protocol::tests` — 13 passed;
- server and client library `cargo check` — passed;
- `just check` — client, server, network-test, Balance Lab, routing, and web checks passed;
- `cargo fmt --all` and `git diff --check` — passed.

No native evidence was required because values, wire shapes, visuals, and player behavior are unchanged.

Learning: the first arithmetic regression ran the producer in ordinary `Update`, which did not prove the production deferred-command boundary. Review caught that gap; the schedule test now runs the real producer in `AbilitySet::ObserveOutcomes` and proves its command-written deadline is visible in `ConcealmentSet::ResolveSources` in the same fixed cycle. Future ECS policy migrations should test both value calculation and the production schedule handoff.
