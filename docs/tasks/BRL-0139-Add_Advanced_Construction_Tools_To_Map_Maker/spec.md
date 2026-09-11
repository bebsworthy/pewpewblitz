# Advanced Map Maker construction tools

## Outcome

After the foundational BRL-0037 editor is accepted, creators can build and revise repeated grid structures quickly using predictable brush, shape, picker, clipboard, and reusable-stamp workflows. All operations continue to author bounded typed map recipes and remain one atomic undo transaction per gesture.

## Scope

- Add explicit freehand, line, rectangle-outline, filled-rectangle, and bounded flood-fill operations for compatible map assets.
- Add a picker/eyedropper that selects an existing authorable asset and its legal rotation without mutating the document.
- Add copy, cut, paste, duplicate, and Delete for the current rectangular/multi-item selection.
- Allow a selected multi-cell arrangement to become a reusable local stamp with a preview, readable name, footprint, and legal rotations.
- Apply active symmetry to the completed operation, previewing and validating all generated placements before commit.
- Preserve existing slot ownership, default-surface behavior, placement identity rules, recipe budgets, local persistence, validation, and server-authoritative Practice admission.
- Keep runtime-derived states and unsupported mode anchors out of generic asset stamps. Add bounded typed support only where a real authored family requires it.

## Interaction contract

- The active construction operation is explicit in the contextual viewport toolbar and has a shortcut and tooltip.
- Drag previews update continuously; releasing commits once. Escape cancels with no history entry.
- Invalid cells/placements are identified before commit with a non-color-only explanation.
- Picker and selection modifiers do not conflict with Space/middle-button pan, wheel zoom, or UI focus.
- Paste and stamp ghosts include the complete rotated footprint and all symmetry copies.
- Flood fill and shapes have deterministic work limits and fail without partial mutation when the result would exceed document or history budgets.

## Implementation sequence

1. Extract a bounded editor-operation model from the current input coordinator: operation kind, anchor/current cell, captured palette item/rotation/symmetry, preview recipe/result, and cancel/commit lifecycle.
2. Add pure cell-generation helpers for interpolated line, rectangle outline/fill, and bounded contiguous fill. Test odd/even dimensions, reversed drags, degenerate shapes, map edges, holes, and hard work limits.
3. Route every operation through existing MapEditorSession candidate validation and atomic commit rather than mutating presentation state or placements directly.
4. Add picker, copy/cut/paste, and duplicate over the existing selection model. Preserve legal placement parameters and allocate new stable placement identities on paste.
5. Add bounded local stamp persistence and an asset-browser section for saved stamps. Store only typed authoring data; thumbnails remain disposable presentation caches.
6. Add contextual toolbar controls, shortcuts, live world ghosts, actionable status text, and controller-accessible equivalents.
7. Verify focused pure/model/UI tests, client role checks and Clippy, native mouse/controller gestures, save/reopen behavior, and routed Practice admission for representative Wipeout, Hot Zone, and Heist maps.

## Acceptance criteria

- Each operation previews continuously and produces exactly one undo entry on successful commit.
- Cancelled, rejected, or no-op gestures do not change recipe revision or history.
- Lines and rectangles match their visible preview for every drag direction and stay within bounds.
- Flood fill is deterministic, bounded, and never partially commits.
- Picker selects the exact authorable asset/rotation under the cursor without editing the map.
- Copy/cut/paste and stamps preserve multi-cell relative layout, reject conflicts atomically, and respect active symmetry.
- Keyboard, pointer, and controller paths cannot activate UI and world editing in the same interaction.
- Saved drafts reopen identically and accepted recipes still pass authoritative validation and routed Practice admission.
- Native evidence demonstrates each tool on real 3D assets at normal and reduced window sizes.

## Dependencies and exclusions

Blocked by acceptance of BRL-0037. This ticket does not add new gameplay asset families, executable map rules, publishing, collaboration, procedural generation, AI generation, or a professional-engine outliner.
