# Scope

Extract only the proven pure geometry and style helpers shared by client-flow screens.

# Acceptance

- One helper normalizes wheel delta with a caller-supplied multiplier.
- One helper clamps offset to content/viewport bounds.
- One helper computes the minimal offset keeping a focused interval visible.
- Focus policy, markers, layout constants, gating, and render trees stay screen-specific.
- Preserve or convert per-screen coverage to focused helper plus integration tests.
- Client UI tests and native wide/compact behavior pass.

# Constraints

Coordinate with ARCH-01. Do not introduce a generic widget hierarchy or erase screen customization.
