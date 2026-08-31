# Context

BRL-0091 found that authoritative passive validation accepts `resistance_basis_points` only in `1..=6_000`, while the Balance Lab manifest advertises a 0.0% minimum. BRL-0091 preserves this pre-existing observable mismatch through a local compatibility adapter because its scope forbids validation or editor-output changes.

# Outcome

Choose and apply one intentional minimum across authoritative validation and the Balance Lab editor. Determine whether zero resistance is a valid authored passive bonus or whether the editor minimum should become 0.01%, then advance any editor/snapshot/content compatibility contract required by the chosen observable change.

# Constraints

- Inspect historical intent and current authored values before choosing the rule.
- Do not weaken authoritative validation merely to match accidental UI metadata.
- Do not change player-visible/editor behavior without recording the decision, version impact, and native/operator evidence.
- Preserve exact-version fail-closed behavior; add no compatibility decoder unless a separately approved architecture decision requires it.
- Reuse BRL-0091's shared passive bounds and local editor adapter rather than reintroducing duplicated literals.

# Verification

Cover boundary validation, serialized manifest metadata, Apply/reset server rejection behavior, web inline validation, persistence/migration as applicable, canonical role/network gates, and a native Balance Lab operator check. Reconcile `docs/15-balance-lab.md` and sync Ticket before closeout.

This ticket is a discovered BRL-0091/BRL-0070 deferral and does not block their organization-only behavior-preserving closeout.

## Cancellation — 2026-08-31

Canceled before implementation. The reported mismatch did not exist: baseline passive metadata already uses `NumberSpec::basis_points(1, 6_000)`, matching authoritative passive validation. The audit had inspected the separate fighter-resistance helper. BRL-0091's exact manifest parity test caught the error; no product, editor, validation, schema, or documentation correction is required.
