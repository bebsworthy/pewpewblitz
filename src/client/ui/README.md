# Player-shell components

This client-only library owns visual presentation across the shell. Screens own content,
layout, actions, navigation, persistence and active-surface boundaries. Components are native
Bevy entities, not a separate widget runtime.

## Production components

- `surface::Surface`: Back, primary, secondary, ordinary action, choice card, overview panel,
  list row, field and quiet action. `SurfaceState` separates selection, focus, disabled and
  inactive state. Owners attach these to native buttons; `SurfacePlugin` owns rendering.
- `controls`: compact Settings action/category rows, switches with knobs, sliders and binding
  capsules. These compose the same surface renderer and state policy. Value conversion,
  binding capture and calibration remain screen-owned.
- `theme`: palette and bundled Lilita One font.
- `scroll`: viewport clamping and focus-reveal geometry, shared by screens and gallery.

The procedural `shell_surface.wgsl` draws frames from final logical dimensions. Action sides
have a pronounced slant; cards/panels use the gentler approved slant; compact rows use a subtle
slant. Outlines and short shadows retain their thickness across sizes. Text and icons remain
upright, and Bevy's rectangular button hit area remains generous around the visual corners.
Back is always orange and gets shared label/arrow composition. Destination wording may vary.

Semantic selection keeps its teal fill. Focus uses an inset contrasting double outline and
triangle marker on every fill, including orange. Disabled overrides interaction, and inactive
owners suppress focus/hover/press. Screens must not choose frame images, focus palettes, border
radii or shadow styles. The old image-button and brawler-card chrome systems are retired.

Surface rendering is installed by the client presentation plugin, separately from headless
Settings behavior. The same plugin runs in the native gallery. Settings controls retain
asset-free native row presentation for isolated behavior tests.

## Consumers and ownership adapters

| Consumers | Component adoption / state owner |
| --- | --- |
| Dashboard and menu | Actions, mode panel; flow action adapter |
| Game selection | Choice cards and confirmation; flow draft adapter |
| Practice maps | Cards, arrows, Start; Practice carousel adapter |
| Brawler browser/details | Back, cards, overview panels, rename, Delete, Select; flow adapter |
| Weapon and parts | Back, equipped-slot and part cards, Nothing; equipment draft adapter |
| Creation / existing ability editor | Shared flow actions; subsequent layout work uses these components |
| Settings and Controls | Back plus rows, categories, switches, sliders, bindings; Settings adapter |
| Server entry, connecting, queue, loading, results and recovery | Shared flow fields/actions; existing screen layout retained |
| Credits and shell overlays | Shared actions; shell focus/layer adapter |

The non-interactive dashboard identity strip, logos, mode badges, map overviews, model renders
and ability pictograms remain content artwork. They are not alternate control renderers.
Historical frame images may remain as source/reference assets but are no longer loaded for
interactive shell chrome. Details frame PNGs were removed; its generator now creates pictograms.

## Native gallery and verification

Run `just ui-gallery`. It needs no server and changes no saved settings. It instantiates
production components, including shell Back/actions/cards/panels and selected+focused,
disabled and inactive examples beside switches/sliders/bindings.

Up/Down, D-pad or left stick moves focus. Left/Right adjusts sliders; Enter/South toggles demo
values. Pointer clicks and slider dragging work; wheel and focus reveal scroll smaller windows.
Columns stack below 1000 logical pixels. Demo action labels do not perform product actions.

BRL-0123 owns adoption and visual acceptance. Verify actual native wide/compact screens against
approved mockups, not only compilation or bounds. Review long labels, tall cards, focus on every
fill, scroll endpoints and inactive overlay isolation. Physical controller checks are reported
separately from synthetic input tests.

For a repeatable compact gallery launch, use
`BRAWLER_UI_GALLERY_COMPACT=1 just ui-gallery` (900×600). The gallery resolves assets
from the repository in the same way as the client, including when launched by a native app wrapper.
