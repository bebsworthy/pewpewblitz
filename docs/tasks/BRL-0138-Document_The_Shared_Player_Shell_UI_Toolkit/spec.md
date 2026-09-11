# Scope

- Add a durable player-shell UI toolkit document beside the approved shell design.
- Reconcile the design document with the implemented native Bevy toolkit, including the current palette, shader-backed chrome, semantic surface roles, controls, focus/selection rules, responsive layout, assets, and extension workflow.
- Link the toolkit from the documentation map, art direction, player UX, delivery contract, and screen map.
- Correct stale connection/recovery descriptions and any styling guidance that conflicts with accepted feedback.
- Preserve unrelated documentation edits and historical delivery records.

# Acceptance criteria

- Contributors can identify the source of truth for colors, typography, surfaces, controls, scrolling, focus, and shared screen composition.
- The documented dark color matches the approved navy token (#081932).
- Buttons document the accepted top-right/bottom-left cuts, slanted edges, navy border, offset shadow, and shader antialiasing.
- Focused, selected, inactive, and disabled states are documented as separate concerns.
- Toggle controls explicitly require a visible track and knob.
- The guidance prohibits decorative clutter, duplicated state labels, controller-specific prompts, and one-off screen styling when a shared primitive exists.
- Server selection, connecting, and recovery documentation matches the implemented approved flow.
- All cross-links resolve and documentation formatting checks pass.

# Verification

- Run `git diff --check`.
- Search for stale palette and connection-flow wording in the updated player-shell documentation.
- Verify every newly added relative Markdown link resolves locally.
- Run `ticket sync` before handoff.


# Implementation

- Added `docs/design/player-shell/ui-toolkit.md` as the native component and styling contract.
- Documented the palette, Lilita One usage, semantic surface roles, shader-backed cut-corner geometry, clipped fills, interaction-state axes, shared controls, responsive scrolling, asset policy, gallery workflow, and extension checklist.
- Linked the toolkit from the documentation index, approved shell design, art/presentation direction, player UX, screen map, and artifact delivery contract.
- Reconciled stale Dashboard chrome, navy-token, Settings auto-save, and connection/recovery descriptions with the accepted implementation.

# Verification evidence

- `git diff --check` — passed.
- Local relative-link validation across the documentation index, art direction, player UX, and all player-shell Markdown files — 9 files passed.
- Stale wording search for `#061A25`, stretched raster chrome, decorative motif guidance, and superseded Connecting actions — no matches.
- Required toolkit-term validation for palette, cut corners, state axes, toggle construction, and gallery command — passed.
- No code tests were run because this ticket changes documentation only.
