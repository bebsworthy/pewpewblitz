# Outcome

Player names stay readable in overhead plates and the HUD roster, whatever characters the player chose.

# Context

- BRL-0145 moved all in-match HUD and overhead text to Lilita One and added a glyph-coverage test for the code-owned string templates.
- Display names are validated by `src/lobby.rs::normalize_proposed_display_name` (NFC, grapheme/byte bounds, control-character rejection) and may contain any script. Lilita One covers Latin only, so names in other scripts render missing-glyph boxes.

# Scope

Choose between a bundled fallback font chain for names and a validation-time character restriction. Then apply it to overhead names and roster rows.

# Acceptance criteria

- A name containing characters outside Lilita One renders without missing-glyph boxes, or is rejected at validation with clear feedback.
- A focused test covers the chosen rule.
