# Context
`presentation_3d/combat/fighter_ui.rs` places overhead plates using a fixed 120 px width (BRL-0145 `resolve_overhead_overlaps`). In the BRL-0168 capture, "Łukasz Жора Αλέξης" lays out wider than the health bar. A 24-grapheme or wide-script name (CJK or emoji are full width) can extend past the plate the stacking logic assumes, and overlap a neighbouring plate.

# Scope
Clip or ellipsize the name to the plate width, or scale it down, or feed the measured text width into overlap resolution. Choose the smallest change that keeps names legible.

# Acceptance criteria
- A 24-grapheme full-width name does not overlap an adjacent plate at minimum separation (ECS/unit test plus native capture).
