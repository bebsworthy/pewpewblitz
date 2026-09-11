# Player shell UI toolkit

This document is the implementation contract for the shared native UI used by the player shell.
The [approved shell design](README.md) owns the visual direction and screen compositions; the
[Player UX specification](../../13-player-ux.md) owns flow, authority, and recovery behavior. This
toolkit translates those contracts into reusable Bevy components without creating a second flow
model or a separate UI framework.

## Sources of truth

| Concern | Native owner |
|---|---|
| Palette and display font | [`src/client/ui/theme.rs`](../../../src/client/ui/theme.rs) |
| Surface roles, geometry, and material states | [`src/client/ui/surface.rs`](../../../src/client/ui/surface.rs) |
| Surface shader and antialiasing | [`assets/brawler/shaders/shell_surface.wgsl`](../../../assets/brawler/shaders/shell_surface.wgsl) |
| Action, tab, toggle, slider, and binding controls | [`src/client/ui/controls.rs`](../../../src/client/ui/controls.rs) |
| Shared scroll calculations | [`src/client/ui/scroll.rs`](../../../src/client/ui/scroll.rs) |
| Product-flow action and focus bridge | [`src/client/flow/screens/shared.rs`](../../../src/client/flow/screens/shared.rs) |
| Product-flow focus reveal and scrolling | [`src/client/flow/screens/scroll.rs`](../../../src/client/flow/screens/scroll.rs) |
| Shared connection-screen composition | [`src/client/flow/screens/connection_shell.rs`](../../../src/client/flow/screens/connection_shell.rs) |
| Standalone component review surface | [`src/client/ui/gallery.rs`](../../../src/client/ui/gallery.rs) |

Screen modules still own their content, responsive composition, action meaning, pending/error
copy, and connection to real product state. A visual component never owns navigation, saving,
networking, or gameplay facts.

## Visual tokens

| Token | sRGB | Use |
|---|---|---|
| Cream | `#FAEBCF` | Quiet backgrounds and unselected surfaces |
| Navy | `#081932` | Text, borders, focus geometry, and dark identity surfaces |
| Teal | `#52A39E` | Secondary actions, selected content, and the left background plane |
| Orange | `#FF6606` | Primary actions, Back, mode accents, and the bottom background plane |

Lilita One is the shared display face at
`brawler/ui/fonts/lilita-one-regular.ttf`. Use the theme helper so screens retain the supported
font fallback. Type size may respond to available width and UI scale, but the hierarchy remains
heading, label, value, and supporting copy. Body text must remain readable and should wrap before
shrinking below its useful size.

The shell uses large clean color planes, strong navy outlines, restrained asymmetry, and crisp
offset shadows. Do not add random slashes, rays, halftone, dirt, busy textures, distressed type, or
invented scenery. Icons must communicate a real action or game fact. Use the retained real logo
asset for branding; do not redraw, crop, or substitute it with text.

## Surface roles

Choose a semantic `Surface` role before styling a control. The role defines its normal fill and
geometry; state then modifies that role consistently.

| Role | Intended use |
|---|---|
| `Back` | The single visible return action for a full-screen child; orange with normalized `‹ LABEL` copy |
| `Primary` | The strongest commit or forward action on a screen; orange |
| `Secondary` | A prominent alternate action; teal |
| `Action` | A regular actionable row or button; cream |
| `Card` | A selectable catalog, brawler, map, or game card |
| `Panel` | A grouped information or editor region |
| `Row` | A compact option, result, or list row |
| `Field` | Editable text or value input |
| `Quiet` | Low-emphasis action that remains visually quiet until focused |

Shared button and row chrome has cut top-right and bottom-left corners, slightly slanted side
edges, a substantial navy border, and an offset shadow. The native `UiMaterial` shader derives
edge coverage with screen-space derivatives so diagonal edges remain antialiased. Text and hit
targets stay native Bevy UI; artwork is never stretched to define interaction geometry.

Use internal padding rather than positioning text or trailing controls against the border. Labels
are vertically centered. Toggle tracks, binding badges, values, and other trailing actions must
remain fully inside the usable shape even when the focus frame appears.
Every fill and interaction tint is clipped to the same cut-corner silhouette as its frame; color
must not bleed into the transparent area outside the border.

## Interaction states

`SurfaceState` and `ControlState` keep four concerns separate:

| State | Meaning | Required presentation |
|---|---|---|
| Focused | Current keyboard/gamepad navigation target | High-contrast inner frame and pointer; always visible on the active scope |
| Selected | Persistent product choice or active category | Role-appropriate selected fill, normally teal or orange |
| Inactive | Retained surface behind an overlay or outside the active input scope | No focus treatment or pointer |
| Disabled | Action cannot currently run | Muted treatment, no false focus emphasis, and factual reason where needed |

Focus is transient and selected state is persistent. Opening an overlay transfers input and focus
to that overlay; the underlying screen cannot react to the same navigation event. Pointer hover or
press synchronizes navigation focus with the hovered action. Returning restores one valid focus
owner and repairs focus if the previous action disappeared or became disabled.

Do not repeat selection with `Selected` text or a decorative checkmark when the card/category color
already communicates it. Preserve the state in the native component model for accessibility and
input behavior. Critical gameplay information must not rely on color alone.

## Controls

The shared control primitive supports these value families:

| Control value | Presentation and behavior |
|---|---|
| Action | Label plus an optional meaningful trailing action; activates once |
| Tab | Category or subpage choice; selected and focused remain independent |
| Toggle | Visible track and movable knob; the fill alone never represents the boolean value |
| Slider | Track, filled portion, knob, and readable value label |
| Binding | Compact key/button badge contained within the row |

Settings and other dense lists use these primitives rather than recreating their padding, focus,
toggle, slider, or binding styles. New control families belong in the shared module only after a
real screen needs behavior that the existing values cannot express.

## Layout and scrolling

- Wide layouts use clear regions and align content by hierarchy, not by making every screen a
  table. Use rows or grids only when the content is genuinely parallel.
- Compact widths and enlarged UI scale must preserve every action in reading order. Scroll the
  content instead of clipping it or shrinking controls until they become unreadable.
- Shared scroll math handles wheel normalization, clamping, and focus reveal. Keyboard/gamepad
  navigation must reveal the focused item without moving focus to an unrelated control.
- Cards size to their content family. Catalog grids should show useful breadth; detail screens may
  use larger panels. A large empty panel is not a substitute for hierarchy.
- Each full-screen destination has one clear Back route. An action that confirms and returns does
  not need an additional visible Back button when the approved flow deliberately omits it.

The shared abstract shell background uses cream with teal and orange planes. Screen-specific
scenery, doors, staircases, and decorative patterns are not part of that background.
Product controls use input-neutral labels and do not add controller-specific button prompts or
footer legends.

## Assets and degradation

Load retained branding, icons, and fonts through `ClientAssetHandles`. Prefer an existing semantic
asset over a new approximation. Decorative assets may disappear under fallback, but labels,
focus, controls, navigation, saving, recovery, and shutdown must continue to work. The surface
shader is presentation only and cannot become an authority or interaction dependency.

The component gallery is a development review surface and is never installed in the player
application. Run `just ui-gallery` for the wide state matrix and
`BRAWLER_UI_GALLERY_COMPACT=1 just ui-gallery` for its compact layout. It supplements native
screen review; it does not validate product flow or replace screen-level tests.

## Extending the toolkit

When adding or changing a player-shell component:

1. Choose the existing semantic surface role and theme tokens before adding local colors or
   geometry.
2. Model focus, selection, inactive scope, and disabled behavior independently.
3. Reuse the shared control and scroll primitives where their behavior matches.
4. Keep action meaning and product facts in the owning screen or flow module.
5. Extract a new reusable helper only when a second real use or a concrete ownership problem shows
   the boundary. Keep a one-screen composition local.
6. Add a representative state to the component gallery when a new shared primitive or state is
   introduced.
7. Test action dispatch, focus ownership/restoration, disabled repair, and responsive visibility at
   the level that owns those behaviors. Review new visual geometry in the gallery and its actual
   screen.

Before handoff, compare the changed screen with its approved reference, verify long copy and
compact/UI-scale layouts, and exercise pointer, keyboard, and gamepad navigation. A shared
component is complete only when its real screen no longer needs local style overrides to match the
contract.
