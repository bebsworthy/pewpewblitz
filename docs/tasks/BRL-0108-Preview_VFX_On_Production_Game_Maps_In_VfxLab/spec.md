# Scope

## Product decision

VfxLab must support two complementary viewing contexts:

1. a deliberately neutral diagnostic fixture for isolating shape, animation, layers, and fallback behavior;
2. selectable production-map context using the real client map presentation path.

A production-map preview must not be a screenshot background or a private approximation. It must reuse the built-in map catalog, resolved map asset recipe, theme materials/assets, world-coordinate conventions, camera behavior, and relevant presentation fixtures used by gameplay.

## Required work

- Add a read-only map-context selector covering the built-in production maps and their authored themes.
- Load the selected map through a focused extraction or composition of the existing production client map presentation path without starting networking, admission, matchmaking, authoritative gameplay, bots, or a gameplay worker.
- Spawn VFX at a selectable valid world location using production units and actual authored effect geometry.
- Provide practical placement controls: reset to a representative default, move or point-place the effect, and expose the resolved world position.
- Use production camera conventions and provide the smallest useful camera controls or presets for inspecting both local detail and gameplay-scale readability.
- Allow self, ally, enemy, and neutral semantic contexts through the production VFX resolver. This ticket consumes the integrated ownership-treatment result rather than reintroducing a lab-only overlay.
- Add optional presentation-only fighter, cover, damageable-object, and objective references from production presentation data where needed to judge occlusion and clutter. These remain fixtures and do not simulate gameplay.
- Display the actual recipe/signature, radius or geometry, relationship, map ID/theme, world position, camera preset, variant, seed, and authoritative elapsed/duration values.
- Retain the neutral fixture as a diagnostic option.
- Preserve normal, reduced-effects, and primitive comparison modes.
- Keep VfxLab read-only. Catalog editing, map editing, gameplay simulation, damage previews, and arbitrary asset loading remain outside this ticket.

## Architecture constraints

- Reuse production map and VFX presentation code; do not create a second map renderer, second VFX materializer, or lab-only shader/effect implementation.
- Keep all map/VFX presentation dependencies client-only and absent from the dedicated-server feature graph.
- Map context is visual evidence only. It must not calculate collision, field membership, damage, scoring, visibility authority, or other outcomes.
- Use stable built-in map IDs and existing catalog resolution. Do not branch on individual IDs in a central VfxLab system.
- Respect map framing, world bounds, render-layer, material-cache, asset-lifecycle, and cleanup ownership.
- Any extraction from the production client must remain a focused shared presentation boundary demonstrated by both consumers.

## Acceptance criteria

1. VfxLab can switch between the neutral diagnostic fixture and every supported built-in production map without restarting networking or a game server.
2. A selected VFX scenario renders through the production resolver/materializer at its real authored scale and a visible resolved world location.
3. Production map surfaces, cover, and relevant visual clutter use the same assets/materials and coordinate conventions as gameplay.
4. Fire-field comparison on the map from the reported mismatch matches the production composition for equivalent relationship, radius, variant, seed/tick, placement, and camera inputs.
5. Self, ally, enemy, and neutral relationship variants can be previewed on a real map after BRL-0107.
6. Switching maps, scenarios, and variants cleans up prior presentation-owned entities and retained resources without accumulation.
7. Normal, reduced, and primitive modes remain functional on real-map previews.
8. Evidence capture records enough context to reproduce the preview and supports a paired VfxLab-versus-game comparison.
9. The headless/server configurations remain free of windowing, map-rendering, VFX, and lab dependencies.
10. Native playtesting confirms the tool makes surface-contrast, scale, clutter, and ownership-readability differences understandable before entering a match.

## Verification

- Focused map-selection, placement, cleanup, and presentation-composition tests.
- Lifecycle tests for repeated map/effect switching and app teardown.
- Client, headless, and server feature/build checks.
- Representative native captures on at least one light/warm map and one dark/cool map.
- Paired VfxLab/gameplay capture with matching map, effect, geometry, relationship, seed/tick, and camera inputs.
- Normal/reduced/primitive comparison matrix.
- Record feedback disposition and learn-from-errors notes before completion.

## Sequencing

This ticket follows BRL-0107. It belongs after the existing BRL-0094 implementation stack so it can exercise the completed effect vocabulary and integrated relationship treatment. It remains related to BRL-0096 because it expands the read-only foundation toward the full VFX Workbench.

## Non-goals

- Editing maps, gameplay recipes, weapon balance, or authoritative state.
- Booting a local server or routed match inside VfxLab.
- Replacing native gameplay verification.
- Full VFX catalog authoring, undo/redo, or save workflows.
- A browser-based map or effect editor.
