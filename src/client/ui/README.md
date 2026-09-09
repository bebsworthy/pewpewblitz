# Player-shell UI

This client-only module owns shared presentation. Screen modules own actions, selected values,
layout and lifecycle. It is a small Bevy-native toolkit, not another UI runtime.

## Current production primitives

- `theme`: approved cream/ink/teal/orange palette and bundled font with asset-free fallback.
  Dashboard, game selection, Practice maps, equipment and loading use the same source.
- `controls`: native Settings rows, category selection, boolean switches, slider fill and binding display.
  Consumers own values and actions; a private presentation plugin updates the shared visuals.
- `image_button`: normal/focus art and visual state. Both Dashboard and other shell image
  buttons use this policy. Disabled takes precedence over every interaction. An inactive
  surface suppresses focus and pointer feedback. Focus is communicated by focus art; pointer
  press/hover uses the same tint across screens.

Existing screen assets retain their shape, spacing and selected-state treatment. Semantic
selection (the equipped part or current category) is distinct from navigation focus (the control
that will activate). Do not make one boolean stand for both.

## Ownership rules

Pass screen ownership into presentation; never infer it from a button index. Indices are local to
screens and overlays. Keep action processing, overlay gates, persistence and network requests out
of widgets. The existing flow navigation and scroll helpers remain the owners of those behaviors.

Use a shared component when there is an actual repeated use. Add Settings toggles, sliders,
binding fields and category tabs as their production consumers are built. Do not add speculative
variants or replace Bevy's entity/component model with a custom widget tree.

Before adding a local color, font or interaction tint, check this module. Screen-specific layout
measurements stay in their screen until a demonstrated repeated layout calls for shared tokens.

## Review path

BRL-0123 owns consolidation, the native component gallery and screen adoption.
BRL-0122 owns approved Settings mockups. The gallery exercises the same components used in
screens rather than reproducing them in HTML or screenshots. Native visual acceptance remains
necessary for new controls and migrated layouts.

## Native gallery

Run `just ui-gallery` from the repository root. It needs no server and changes no saved settings.
Use Up/Down, D-pad or the left stick to focus the interactive examples; Left/Right adjusts the
slider; Enter/gamepad South toggles the boolean or selected tab. Pointer clicks toggle controls,
and dragging on the slider track adjusts it. The action and binding examples demonstrate their
presentation only; real actions and key capture remain screen-owned. Mouse wheel scrolls smaller
windows, and navigation reveals focused controls using the same scroll geometry as flow screens.

`controls::UiControlsPlugin` updates row chrome, labels and slider fill before Bevy layout.
`spawn_control` creates ordinary Bevy buttons. Consumers set `ControlValue` and `ControlState`
and attach their own action components. `InteractionDisabled` uses the native Bevy disabled
contract. Controls do not save values, capture bindings or send network commands.

The gallery is a component review surface:
binding fields have outlined capsules; boolean controls use a pill with a moving knob; sliders have a larger hit area around their rail and a positioned knob.
It exposes presentation regressions independently of screen behavior. Gallery and flow both reuse `ui::scroll`; focus
ownership and ordering remain in their respective screen systems.

Focused controls also display a marker independent of color. Disabled and inactive surfaces hide
that marker; a selected category keeps its teal fill when focused. Slider fill and knob use the
same normalized value, clamped at the endpoints. Below 1000 logical pixels the gallery stacks
its columns; navigation rechecks visibility when the window size changes.

## Settings consumer

`client::shell::settings_screen` and its Controls pages use these controls throughout Settings.
The screen owns category selection, contextual descriptions, normalized slider conversion and
saving. Left/Right adjusts a focused slider; Up/Down moves focus. Save failures stay inline with
Retry save. Controls owns release-armed binding capture, conflict swaps and selected-device
sampling; `settings::binding_edit` and `settings::calibration` own pure validation and measurement.
Neither behavior lives in shared widgets. Ordinary changes autosave, pointer slider saves are
coalesced until release, fullscreen previews have timed rollback, and measured calibration values
require explicit acceptance. Scoped resets expose Undo reset.

Settings rebuilds restore focus by semantic control ID, never by an entity that was despawned.
Scroll handling is limited to the hovered pane or the pane containing focused descendants.
Narrow layouts move categories above options/details. Native review should include those layouts,
capture cancellation, controller loss and long option lists in addition to the gallery.
