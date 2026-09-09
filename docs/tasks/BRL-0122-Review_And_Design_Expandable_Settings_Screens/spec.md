Scope: source and native UI review; inventory implemented settings and editing behavior; distinguish existing remapping/deadzone support from missing product UX; propose category and child-screen map, controls, persistence/conflict/capture behavior, controller disconnect and calibration flow. Preserve approved clean cream/navy/teal/orange visual language, no redundant labels or global device hints, strong gamepad focus, single contextual Back, auto-save for ordinary valid changes. Design for future options without adding speculative gameplay settings. Calibration implementation remains owned by BRL-0050. Deliver a review/design proposal with mockup-ready screen requirements; no production UI implementation or generated mockups in this review step.

## Review findings — 2026-09-09

Reviewed src/client/shell.rs, settings/ui.rs, settings/mod.rs, settings/persistence.rs and input consumers. Native review opened Menu > Settings in CorrectionReview.app without changing values. The current dark overlay is visually disconnected from the approved shell, uses tiny monospaced diagnostic text, cycles a selected value separately from button focus, and clips further controls below its visible panel. Values and editing buttons are spatially disconnected. Apply/Cancel and global Reset conflict with the desired ordinary auto-save behavior.

Existing functionality: fullscreen, vsync, master volume, mute unfocused, UI scale, reduced motion/effects; single-key keyboard bindings, mouse primary button, single-button gamepad bindings; move/aim radial deadzones, inversion, aim commit threshold, trigger hysteresis; local atomic settings persistence. Binding conflicts are reported but are not rejected by numeric validation. Escape/gamepad East cancel capture before assignment. Capture accepts any connected pad. Settings are global, not per-controller profiles. Guided neutral calibration, secondary bindings, chords and stick reassignment are not implemented.

## Proposed screen structure (design proposal, not user-approved mockups)

Menu > fullscreen Settings. Fixed category rail: Display, Audio, Accessibility, Controls. Fixed header with one contextual Back; only content scrolls vertically. On narrow layouts, category selection becomes a full page with child pages rather than shrinking labels or opening popups.

- Display: fullscreen and vsync. Add other display options only when supported.
- Audio: master volume and mute when unfocused. Future mix channels belong here when implemented.
- Accessibility: UI scale, reduced menu motion, reduced combat effects. Each setting appears in one category only.
- Controls: links to Keyboard & Mouse and Gamepad. Child pages retain category context and restore focus/scroll when returning.
- Keyboard & Mouse: action rows grouped by movement, combat and system actions; clickable binding capsules beside action names. Capture replaces the selected capsule inline, waits for activation input to release, and exposes a visible cancel action. Conflicts are resolved inline before committing; swapping bindings is a safe initial policy with the existing single-binding model. Do not silently remove another essential binding. Preserve a recovery navigation path independent from remapped gameplay bindings.
- Gamepad: explicit active-controller context, readable button names/glyphs with text fallback, action rows, inversion and a Calibrate Sticks entry. Ignore other controllers during capture; disconnect cancels capture without discarding existing bindings. Do not claim per-device persistence until reliable identity is verified.
- Calibrate Sticks: dedicated screen with left/right live stick plots, neutral sampling, recommended deadzone, manual adjustment and immediate processed-output preview. Explain deadzone briefly beside the relevant control. Sequence: observe/rest sampling > review recommendation > test fine movement/aim > use calibration. Calibration is a deliberate staged change; leaving before acceptance preserves previous values. BRL-0050 owns implementation and the post-platform-handler signal, continuous renormalization, identity research and drift/reticle verification. Trigger press/release tuning belongs in an advanced controls section, not the default options list. Full-range axis calibration and reassignment are separate future scope, not existing functionality.

## Shared interaction and visual contract

Use approved clean cream/navy/teal/orange palette and typography, restrained angled framing, minimal texture. Use direct toggles, sliders with numeric values, choices and binding capsules; remove generic previous-field/value buttons and diagnostic summaries. Keep selected category styling distinct from a strong outline/pointer for gamepad focus. Focused content scrolls into view. D-pad/stick navigation cannot reach the underlying menu/dashboard/gameplay. Back returns to the prior screen and restores focus.

Ordinary valid changes apply and auto-save, with slider persistence coalesced at interaction end. No permanent Saved label or global Apply/Cancel footer. Surface save failures inline with retry. Display changes that could hide the UI should have an inline timed Keep/Revert state. Reset affects the current category and offers inline confirmation or undo; no accidental global reset. Short explanations appear only where semantics need them, not blanket instruction banners. Device glyphs are appropriate on the binding pages themselves, without adding global input-dependent footer hints.

## Next deliverable and acceptance

Create mockups before production implementation: (1) standard Settings with Accessibility controls and category rail, (2) Keyboard & Mouse bindings including inline capture/conflict states, (3) Gamepad controls, (4) guided stick calibration. These establish the shared component language; Audio and Display reuse the standard layout. User review of these mockups precedes implementation. Do not add speculative empty categories or unsupported options just to fill space.

Review is complete: source inventory and native observation recorded, expansion structure and binding/calibration contracts proposed. No production code changed; no test run required for this design-only review. Learning: reuse the existing input math/persistence, but do not mistake diagnostic editing controls for a player-facing settings design. Keep this proposal separate from approved screen designs.


## User layout reference — 2026-09-09

User supplied codex-clipboard-4d991fe8-eea4-4d88-9a9a-eb9b8589de80.png specifically for layout, not style. Adopt three columns: categories LEFT (as pictured; the message says right twice), selected category options CENTER, focused-option details RIGHT. This refines the earlier sidebar proposal. Retain the approved PewPew Blitz cream/navy/teal/orange style, typography and restrained texture; do not copy the reference's dark visual theme, settings inventory or footer hints.

Start mockups with approximate usable-width proportions 20% / 48% / 32%, adjusted for readable text. Left category rail and header remain fixed. Center contains vertically scrolling action/value rows with clear focus and direct controls. Right updates from the hovered/focused option and explains its effect; include a default value or meaningful preview where useful, without repeating the current value or adding filler. Use the right panel for live stick/deadzone feedback on calibration screens. Binding capture/conflict handling stays inline. Gamepad left/right adjusts a focused value rather than unexpectedly changing columns; category navigation has a deliberate focus transition, and the informational right panel does not add focus stops unless it contains an actual action. Preserve single contextual Back and auto-save. This is a layout direction, not approval of a generated Settings mockup or implementation.


User requested generation of the first Settings mockup. Generate a preview of the standard three-column screen with Accessibility selected, UI scale, reduced motion and reduced combat effects, contextual reduced-motion explanation on the right. This is visual exploration for review, not production implementation or final asset approval.


Generated first preview using built-in imagegen: /Users/boyd/.codex/generated_images/01a07a6d-94f7-7f21-a36f-405709f6b7cd/exec-42787477-372f-45db-95e9-942d43cb11a3.png. Visually inspected: three-column layout, active category distinct from focused option, direct controls and contextual explanation rendered legibly. Preview only, awaiting user feedback. Generated header wordmark is not the existing approved logo and must not replace the existing logo in implementation.


User approved the Settings mockup layout, then requested removal of the top wordmark: header must contain only SETTINGS, no logo. Implemented in revised preview /Users/boyd/.codex/generated_images/01a07a6d-94f7-7f21-a36f-405709f6b7cd/exec-379dfafa-d9a9-45d9-9161-cb5c2c7dc684.png using built-in imagegen. Visually verified logo removal and preserved three-column layout. This supersedes the prior note about retaining the existing logo on this screen.


Continued user-authorized mockups with Controls / Keyboard & Mouse preview: /Users/boyd/.codex/generated_images/01a07a6d-94f7-7f21-a36f-405709f6b7cd/exec-ad1499d4-cbc0-4fec-9816-ae99f56ea4ad.png (built-in imagegen). Preserves logo-free Settings header and three columns; adds keyboard/gamepad tabs, grouped scrollable binding rows, orange focus and contextual action panel. Preview awaiting feedback, not implementation approval. Illustration currently repeats the focused key; future refinement can replace it with an action illustration if user finds that redundant.


User accepted Keyboard & Mouse mockup and requested continuation. Generated Gamepad overview preview using built-in imagegen: /Users/boyd/.codex/generated_images/01a07a6d-94f7-7f21-a36f-405709f6b7cd/exec-a628a53c-029a-42aa-8fbf-4ae0edcc9025.png. Visually inspected legible bindings, Gamepad active tab, Calibrate sticks navigation focus, contextual stick illustration and retained Settings-only header. Awaiting feedback. Controller labels are illustrative; actual implementation must use device-appropriate readable labels with fallback. Calibration workflow mockup remains next.


User accepted Gamepad overview and requested calibration mockup. Generated preview /Users/boyd/.codex/generated_images/01a07a6d-94f7-7f21-a36f-405709f6b7cd/exec-9282de08-32da-40fb-9045-98ef458192c8.png with built-in imagegen. Shows manual movement/aim deadzones, Measure stick drift action, reset and live input/output diagrams. Awaiting feedback. Values illustrative. Visual review caveats for implementation: render deadzone radii accurately from values (generated circles exaggerate them); use neutral framing for unfocused measurement action so orange focus remains unique. Manual changes auto-save; measured recommendations require explicit acceptance before replacing values. BRL-0050 retains calibration implementation ownership.


User accepted the manual calibration mockup and requested continuation. Generated inline calibration recommendation preview /Users/boyd/.codex/generated_images/01a07a6d-94f7-7f21-a36f-405709f6b7cd/exec-776216c1-3a9e-45b1-8ced-05a31edb5189.png using built-in imagegen. Shows illustrative recommended values, live test preview, Use these values, Measure again and Keep current. No popup; acceptance is specific to measured recommendations, not global Settings saving. Visually reviewed; graph geometry remains illustrative and must be calculated from real input in implementation. Awaiting user feedback on this state.


User accepted measurement-result mockup and requested continuation. Generated inline binding-conflict preview using built-in imagegen: /Users/boyd/.codex/generated_images/01a07a6d-94f7-7f21-a36f-405709f6b7cd/exec-ac96d22a-577d-4b25-81eb-679066bec1b3.png. Visually verified current bindings unchanged in center, explicit Q/E swap preview on right, focused Swap bindings and Cancel without popup. Pending user feedback; equivalent gamepad conflicts reuse this interaction with appropriate button labels.
