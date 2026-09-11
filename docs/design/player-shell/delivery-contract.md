# Output contract for implementation

The [UI toolkit contract](ui-toolkit.md) owns the current native palette, surface roles, controls,
interaction states, and extension rules. This document describes design-artifact handoff rather
than an alternate component system.

## The practical answer

Request **PNG screen references plus written layout/style/motion notes first**. After selecting the style, request separable assets. A PNG can communicate an excellent design; its flat pixels cannot tell the client which button is focused, how text wraps or where an editable field begins. SVG is useful for individual geometric assets, not as a replacement for the application. Blender is useful only for actual 3D content.

An imaging model is not assumed to provide editable layers, valid SVG, font files, accurate measurements, a Figma document or a Blender scene. Ask for those from a capable design/3D tool or reconstruct them after approval. Never accept a bitmap embedded inside an SVG or a renamed file as editable delivery.

## Format choices

| Deliverable | Preferred form | What implementation uses it for |
|---|---|---|
| Approved complete screens | Lossless sRGB PNG, one per state and canvas | Visual target; compare against native captures |
| Shared style board | PNG plus Markdown/JSON values | Palette roles, type scale, component construction, spacing |
| Layout source, if supported | Editable design document with named frames/components and constraints; accompanying PDF/PNG exports | Inspect padding, text frames, alignment, resizing and states without depending on one app |
| Exact UI text | UTF-8 Markdown or JSON keyed by screen/region | Native text, localization-ready layout and honest state copy |
| Geometric icons and brand motifs | True editable SVG with viewBox; provide raster reference | Authoring source; choose existing compatible runtime representation or export PNG after pipeline validation |
| Painterly artwork and textures | Text-free PNG, alpha where needed | Background layers, decorative cutouts or bounded texture assets |
| Typography | Actual licensed OTF/TTF plus license and family/weight names | Native text; the current project already ships Lilita One for display headings |
| Motion study | MP4 preview or timestamped PNG storyboard, plus written timing/behavior | Rebuild transitions and microinteractions in native UI |
| New 3D stage/prop, only if justified | Editable `.blend` plus exported `.glb`, textures and render references | Source editing plus Bevy-oriented runtime asset export |
| Existing brawler preview | Existing game model and animation assets | Live selected fighter; generated illustrations are composition references |

SVG availability does not prove a native SVG loader is installed. Use it as a clean editable source; simple motifs can become native geometry and complex icons can be rasterized to the required densities. Similarly, an editable layout is optional rather than a reason to block work when good PNGs and notes exist.

## Minimum return package

```text
approved-style/
  README.md                    chosen direction/revisions and unresolved decisions
  style-board.png
  style-notes.md               named colors, type choices, dimensions, component rules
  screens/
    s03-dashboard-default-1920x1080.png
    s03-dashboard-empty-1920x1080.png
    s08-weapon-valid-1920x1080.png
    s08-weapon-invalid-1920x1080.png
    s08-weapon-valid-1024x768.png
    s11-loading-sync-1920x1080.png
    ...                        other approved screens and required state variants
  screen-notes.md              exact text, layout regions, behavior, responsive intent
  motion-notes.md              use/revise the proposed timing and reduced-motion table
```

These names illustrate the return format; this directory is not created by the current handoff. Include an approved 1591×720 Dashboard and dense-editor adaptation as well as the reference and compact studies. If the generator cannot produce exact dimensions, document the canvas instead of mislabeling it.

For every screen record:

- screen ID, state, revision, image filename and actual canvas dimensions;
- layout regions with approximate x/y/width/height and anchor/resizing behavior;
- text strings, typography role, wrapping/truncation/scroll intent;
- strongest action, initial/restored focus, selected state and disabled/pending reasons;
- art layers versus native controls, source asset references and any provisional parts;
- what changes at wide/compact sizes and enlarged UI scale;
- unresolved discrepancies between generated image and written product requirements.

Measurements extracted from pixels are estimates until verified in native layout. Capture the visual intent without claiming image-generation text or geometry is exact.

## Preferred separate asset package

Ask for this only after approval to avoid manufacturing assets for discarded styles:

```text
assets/
  backgrounds/                 text-free base artwork and independent foreground accents
  chrome/                      approved bounded icons or decorative accents with a named use
  icons/                       navigation/action/status SVGs and preview PNGs
  typography/                  actual font files and licenses, if newly selected
  models/                      optional .blend source and .glb exports
  motion/                      MP4/storyboard references
  asset-manifest.json          dimensions, roles, provenance and implementation notes
```

### Raster assets

- Export sRGB PNG; use real alpha for cutouts, not a painted checkerboard. Retain unpremultiplied source when the tool supports that choice and document the export.
- A full-canvas decorative background may use a 3840×2160 master when the tool can supply genuine detail, with a 1920×1080 review export. Upscaling alone is not new detail.
- Deliver each separable layer on an explicitly documented canvas or with a documented origin/anchor. Preserve overscan where parallax is intended; specify crop-safe regions.
- Remove dynamic text, buttons, values, focus rings and status indicators from background artwork. Provide both the complete reference and clean layers so removals can be checked.
- Keep clean alpha edges with padding around cutouts; check against dark and light backgrounds. Provide an opaque reference composite to catch halos and missing shadows.
- Do not split a flattened generated image into dozens of guessed layers. Start with useful bounded layers: base background, major illustration, foreground accent. Reconstruct UI shapes in code.

### Vector assets

- One icon/motif per SVG, explicit viewBox, consistent optical weight, predictable fill/stroke roles and no external resource links. Label the intended nominal size and padding.
- Use actual paths for icon/wordmark geometry, not raster image wrappers. Keep editable source for any complex artwork.
- Do not bake interface labels into outlines. Native text must remain selectable by the application's data/model and adjustable for size and language. A fixed decorative wordmark may be outlined with its source font/provenance retained.
- Avoid unnecessary SVG filter effects; supply a visual reference if a shadow or effect needs reconstruction. Runtime import/rendering will be verified separately.

### 3D assets

- Reuse existing brawler models first. A new stage pedestal, prop or animated backdrop needs its own demonstrated visual purpose.
- Supply `.blend` with named objects, transforms, units, origin, material setup, packed or relative textures and any animation clips. Export a `.glb` containing the required meshes/materials/animations.
- Include a preview render, camera/framing notes, dimensions, mesh/material counts and intended motion. A screenshot of a Blender scene is not the scene file.
- Keep the runtime design compatible with simple matte materials, restrained lighting and a static fallback. Blender-only procedural materials/compositing are reference until explicitly baked or recreated.
- Do not commission a complete 3D menu room just because the style has depth. Native panels, a simple stage and existing fighter render may reproduce the approved design more clearly.

## Asset manifest record template

```json
{
  "id": "shell.background.arena_show",
  "file": "backgrounds/arena-show-base.png",
  "role": "decorative_background",
  "screens": ["S01", "S03", "S10"],
  "revision": "approved-01",
  "width_px": 3840,
  "height_px": 2160,
  "color_space": "sRGB",
  "alpha": false,
  "anchor": "center",
  "crop_rule": "cover; keep central stage inside safe region",
  "dynamic_text_baked_in": false,
  "source_tool": "REPLACE_WITH_ACTUAL_TOOL",
  "source_file": null,
  "license_or_usage_terms": "RECORD_ACTUAL_TERMS",
  "reference_inputs": [],
  "notes": "Template values; replace with actual inspected asset metadata."
}
```

Record generation/editing provenance and any source-reference restrictions. Approved runtime imports later go through the project's existing asset manifest and client-only catalog conventions. This illustrative record is a handoff format, not a new production schema.

## What remains native

Buttons, panels, labels, statistics, text fields, toggles, sliders, selection/focus, scroll behavior and transient status remain native UI. Real brawlers use the existing 3D preview. Decorative images sit behind/alongside these elements. Motion reads actual flow state; save/network results are never inferred from an animation.

At implementation, map the agreed roles to the existing native toolkit first. Add a focused shared
primitive only when a real second use or concrete ownership problem demonstrates the boundary.
Keep the existing owned screen modules and explicit draft/commit behavior. This brief does not
authorize a custom UI framework, new renderer, or UI dependency.

## Acceptance when the generated style returns

1. Check all screen/state references are readable at their actual dimensions, including the native wide and compact studies.
2. Check required content and actions against the screen map; reject fabricated product features.
3. Reconcile inconsistent colors, typography, corner shapes, shadows and focus/selection before coding.
4. Inspect separate assets: true transparency/vectors, no baked dynamic UI, actual fonts and usable provenance.
5. Implement a small native slice using real models and representative long text; compare native captures to the references.
6. Verify controller/keyboard/pointer flow, drafts, pending/error behavior, resizing, UI scale and reduced motion with the existing verification paths.

The return package can be complete enough to implement even if no SVG or Blender file exists. The most valuable information is an approved coherent visual target plus explicit behavior and separable decorative art.
