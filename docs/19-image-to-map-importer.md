# External image-to-map tool specification

## Decision status

`Python tool implemented; engine-limit rework and content adoption remain separate`

This document specifies a small external Python tool that reads a grid-based map image and writes a
PewPew Blitz `MapRecipe` RON file.

The tool has no runtime or build-time integration with the game. It does not link Brawler, start
Bevy, inspect ECS state, call the server, or require a Brawler validation command. Its complete core
workflow is:

```text
map image + recognition profile -> Python analysis -> map recipe .ron
```

The generated RON is ordinary candidate map content. If a developer later adds it to Brawler, the
game's existing loaders, resolver, tests, and playtests validate it in the same way as a manually
written recipe.

## Outcome

Given an image such as:

```text
external_assets/map_images/acid_lakes.webp
```

the operator runs one command:

```bash
pewpew-map-import acid_lakes.webp acid-lakes.ron \
  --recipe-id 20 \
  --revision 1 \
  --mode wipeout \
  --theme-id 2
```

The tool writes `acid-lakes.ron` containing:

- exact detected grid dimensions;
- a configured default surface;
- recognized water, vegetation, walls, barriers, decorations, and spawns;
- stable deterministic placement IDs;
- explicit empty mode anchors unless a supported anchor is selected; and
- comments summarizing approximated or omitted image elements.

The file is produced even when unsupported objects are present. The command reports those omissions
clearly. `--strict` is available when the operator wants unresolved recognition to prevent output.

## Input observations

The images are display exports, not native-scale tile sheets. One visible checker square represents
one logical map cell, regardless of how many output pixels that square occupies. Counting that
pattern gives:

| Image | Pixels | Grid |
|---|---:|---:|
| `acid_lakes.webp` | `525 x 525` | `60 x 60` |
| `core_of_orbit.webp` | `525 x 525` | `60 x 60` |
| `dark_passage.webp` | `525 x 525` | `60 x 60` |
| `double_trouble.webp` | `525 x 525` | `60 x 60` |
| `hyacinth_house.webp` | `345 x 525` | `23 x 35` |

The repeating ground, aligned object baselines, stable colors, and recurring sprite families make a
deterministic Pillow/NumPy implementation sufficient for the first version. Machine learning is not
required.

These filenames, pixel dimensions, and grid dimensions are regression fixtures only. Recognition
must not branch on an image filename, hash, `60 x 60`, `23 x 35`, or any other known sample value.
The same profile must handle different map widths and heights, and grid detection must support other
display scales, including resampling where cell boundaries alternate between adjacent integer pixel
positions.

## Transparent boundary

Transparency around the source maps is a display artefact representing space outside the playable
map. It is not water, void terrain, or a set of authored cells.

The tool therefore:

- derives dimensions from the complete image canvas and detected cell size;
- ignores transparent pixels during recognition;
- does not crop to the nontransparent bounding box;
- does not emit placements for transparent pixels; and
- leaves the surrounding non-walkable boundary to Brawler's existing perimeter when the recipe is
  eventually loaded.

For example, a `525 x 525` square example remains a `60 x 60` recipe even when its outer pixels are
transparent.

## Scope

### Included

- PNG and WebP input.
- Fixed-grid detection with explicit override flags.
- Orthographic sprite recognition where visual height bleeds upward.
- Direct deterministic RON generation.
- Supported-asset mappings stored in a local recognition profile.
- Clear warnings for ambiguous and unsupported objects.
- Optional overlay and JSON report for debugging recognition.
- A strict mode for automated use.

### Excluded

- Bevy, ECS, Avian, Lightyear, or Brawler-library integration.
- A Rust crate or Cargo workspace package.
- A Brawler authoring-contract export or validator service.
- A GUI or player-facing map editor.
- Runtime image loading.
- Automatically implementing unsupported gameplay.
- Automatically modifying Brawler's map index, catalogs, map pools, or admission revisions.

## External Python project

The tool is implemented outside the Brawler repository in the sibling project:

```text
/Users/boyd/wip/pewpew-map-importer/
  .python-version
  README.md
  pyproject.toml
  uv.lock
  src/pewpew_map_importer/
    cli.py
    errors.py
    grid.py
    model.py
    profile.py
    recognition.py
    ron_writer.py
    preview.py
    profiles/
      pewpew-v1.json
  tests/
```

Initial dependencies:

- Python with one pinned supported version;
- Pillow for WebP/PNG decoding and optional overlay output; and
- NumPy for bounded pixel-array analysis.

OpenCV or scikit-image should be added only if a demonstrated recognition problem cannot be solved
clearly with Pillow and NumPy. The tool requires no GPU, network connection, cloud service, or game
installation.

## Recognition profile

`pewpew-v1.json` contains the knowledge needed to turn source sprites into current RON values:

```text
profile version
supported cell-pixel range and grid-origin rules
map recipe schema version
default asset mappings
MapAssetId values
ModeDefinitionId aliases
sprite color/template signatures
logical footprints and rotations
sprite baseline and upward-bleed envelope
confidence thresholds
```

The profile is deliberately simple and manually maintained. If Brawler changes a numeric asset ID
or recipe schema, the profile is updated. A generated RON comment records the profile version used.

The tool does not need to read Brawler catalogs. The generated file remains reviewable text, and any
stale mapping is caught later when a developer attempts to use the recipe in Brawler.

## Recognition algorithm

### 1. Decode and find the grid

1. Decode the image to RGBA8 without resizing.
2. Search a bounded profile-defined range of floating-point display pitches and grid origins.
3. Score candidates using checker-pattern autocorrelation, adjacent-cell color alternation,
   repeated baselines, aligned edges, and whole-canvas coverage. Suppress two-cell and other
   checker harmonics so a repeated pair is not mistaken for one tile.
4. Rasterize fractional boundaries deterministically, then derive grid width and height from the
   winning pitch/origin and image dimensions.
5. If automatic inference is inconclusive, require `--cell-pixels` or `--grid-origin`.

The four square examples currently infer a `60 x 60` logical grid from an approximately `8.5`-pixel
display pitch, producing alternating 8/9-pixel raster cells inside the display border. Hyacinth
House infers `23 x 35` at a 15-pixel display pitch. These measurements describe the supplied
exports only; neither is a game-world scale or a filename-specific rule. Templates and masks are
defined in cell-relative coordinates and sampled between the inferred boundaries.

### 2. Recognize ordinary surfaces

The repeating checkerboard becomes the configured default surface and is not emitted once per cell.
Explicit surface overrides such as water are detected per cell using color coverage and edge
evidence.

Foreground sprites are masked when classifying the surface beneath them. A single center pixel is
not sufficient evidence.

### 3. Recognize projected sprites

Each known sprite class defines two separate shapes:

- logical footprint: cells written to the RON recipe; and
- visual envelope: pixels drawn by the sprite, including height above the footprint.

The recognizer scans screen rows from bottom to top. This follows the image's painter order: sprites
anchored lower on the screen may cover parts of sprites above them. Matching weights pixels around
the owning baseline more heavily than pixels in the upward bleed.

Upward pixels never change logical ownership. For a footprint whose top screen row is `row`:

```text
map_x = screen_column
map_y = grid_height - row - footprint_height
```

### 4. Resolve candidate conflicts

The tool keeps the highest-confidence non-conflicting recognition at each cell, subject to the
known surface, feature, decoration, and marker slots. A candidate becomes unresolved when:

- its confidence is below the profile threshold;
- two candidates have similar scores;
- its footprint conflicts with a stronger candidate; or
- the sprite family has no supported PewPew asset mapping.

By default unresolved objects are omitted from the RON and listed in the summary. Under `--strict`,
any unresolved object prevents the output file from being committed.

## Initial mappings

The initial profile can emit only assets that already have a reasonable current equivalent:

| Image class | RON asset | Treatment |
|---|---|---|
| checker floor | `MapAssetId(5)` / `sand-floor` | default surface |
| water | `MapAssetId(6)` / `water` | explicit surface cells |
| vegetation | `MapAssetId(8)` / `tall-grass` | approximation when presentation differs |
| solid brick wall | `MapAssetId(7)` / `garden-wall` | explicit feature cells |
| exact two-cell barrier | `MapAssetId(9)` / `breakable-barrier` | only for matching `2 x 1` footprints |
| spawn rings | `MapAssetId(20)` / `player-spawn` | team, ordinal, and inward facing inferred from color/edge |
| recognized supported decoration | matching configured ID | only exact family mappings |

Teleporters, healing pads, unsupported crates/chests, hazards, multiple objective zones, and unknown
decorations are skipped and reported. They are not substituted with unrelated gameplay objects.

When Brawler implements another asset family, the Python profile may add the corresponding mapping.
No recognition-code redesign is required.

## RON generation

The writer emits the existing recipe shape directly:

```ron
(
    recipe_id: MapRecipeId(20),
    revision: 1,
    recipe_version: 3,
    mode_definition_id: ModeDefinitionId(2),
    presentation_theme_id: MapPresentationThemeId(2),
    dimensions: (width: 35, height: 35),
    default_surface_asset_id: MapAssetId(5),
    placements: [
        // recognized placements
    ],
    filled_rects: [
        // optional repeated horizontal/rectangular runs
    ],
    mode_anchors: [],
)
```

The writer:

- sorts placements by map row, column, slot, asset, and ID;
- assigns deterministic nonzero placement IDs from slot plus absolute cell index;
- uses `filled_rects` for repeated one-cell assets when it materially reduces file size;
- writes integer cells and quarter turns only;
- writes no image paths, pixel coordinates, confidence values, or unsupported placeholders into the
  recipe;
- includes short leading comments with source filename/hash, tool/profile version, and omission
  counts; and
- writes atomically through a temporary sibling file.

For Wipeout, `mode_anchors` is empty. A supported Hot Zone import requires explicit CLI anchor
options; the tool does not invent an objective from arbitrary blue circles.

## CLI

The common operation is one command:

```text
pewpew-map-import <input-image> <output-ron> [options]
```

Required recipe options:

```text
--recipe-id <u64>
--revision <u32>
--mode <wipeout|hot-zone>
--theme-id <u16>
```

Useful options:

```text
--profile <path-or-name>       default: pewpew-v1
--default-surface-id <u16>    default from profile
--cell-pixels <float>         override inferred display-pixel pitch
--grid-origin <x,y>           override origin; fractional values allowed
--strict                      fail when anything is unresolved
--debug-dir <path>            write overlay.png and report.json
--json                        machine-readable stdout summary
--overwrite                   replace an existing output file
```

Example:

```bash
pewpew-map-import \
  /Users/boyd/wip/brawler/external_assets/map_images/acid_lakes.webp \
  /tmp/acid-lakes.ron \
  --recipe-id 20 \
  --revision 1 \
  --mode wipeout \
  --theme-id 2 \
  --debug-dir /tmp/acid-lakes-debug
```

Human output reports grid size and counts for recognized, approximated, skipped, and unresolved
objects. `--json` emits the same data with stable field names and no progress text or terminal
formatting.

Exit codes:

```text
0  RON written successfully
1  recognition completed but strict-mode unresolved items prevented output
2  invalid arguments, profile, image, or output path
3  image decoding or analysis failure
```

## Optional debug output

`--debug-dir` is diagnostic only and does not introduce an intermediate authoring workflow. It may
contain:

- `overlay.png`: source image with the detected grid, asset labels, and unresolved boxes;
- `semantic.png`: simple colored cells representing emitted RON placements; and
- `report.json`: source hash, grid, counts, confidences, skipped families, and their image cells.

The `.ron` remains the primary output. An operator may edit it manually like any other map recipe.

## Engine changes required for generated maps

The Python tool itself requires no engine changes. Brawler separately needs to accept the map sizes
and densities that the tool can discover.

### Dimensions

Change `MapDimensions::validate` from:

```text
32..=128 wide, 24..=96 high
```

to:

```text
1..=128 wide, 1..=96 high
```

This allows `hyacinth_house.webp` to remain `23 x 35` without padding. Spawn clearance,
reachability, mode topology, camera behavior, and normal map validation still determine whether a
particular small map is playable.

### Placement density

Remove the independent `placements.len() > 512` rejection. Bound work instead by:

- checked expansion no greater than the map's cell/slot capacity;
- existing source-RON and resolved-snapshot byte ceilings;
- existing slot-conflict and footprint validation;
- measured dynamic-state, collider, generated-mesh, render, CPU, and memory limits; and
- early rejection before allocating structurally impossible expansions.

These are engine changes to support dense authored recipes generally. They are not Python
integration points.

## Verification

### Python tool

- Infer the correct grids for the five reference examples without filename, hash, or known-size
  branches.
- Infer unrelated synthetic rectangular grids such as `17 x 29` and `41 x 24`.
- Infer multiple supported cell-pixel sizes and nonzero grid origins, with explicit overrides
  producing the same result.
- Reject images whose grid evidence is ambiguous instead of selecting a familiar sample size.
- Ignore transparent boundary pixels without cropping dimensions.
- Correctly convert screen rows to map `y` coordinates.
- Recognize a sprite whose visual pixels bleed into the cell above without moving its footprint.
- Resolve bottom-to-top sprite occlusion deterministically.
- Emit byte-identical RON for repeated identical runs.
- Emit syntactically correct current-schema RON for supported detections.
- Report every unsupported or ambiguous object; strict mode rejects them.
- Refuse accidental overwrite without `--overwrite`.
- Install and run from the locked Python project without Brawler present.

### Brawler engine rework

- Existing built-ins retain their fingerprints and behavior after the dimension-limit change.
- A legal `23 x 35` recipe resolves without padding.
- A legal recipe with more than 512 placements resolves within byte/performance limits.
- Narrow-map perimeter, camera, cursor projection, spawn clearance, and presentation pass native
  checks.
- Dense-map resolution, collider generation, snapshot size, render cost, and memory stay bounded.

### First imported map

After choosing to add a generated RON to Brawler, run the ordinary map/content test, routed match,
native presentation, and user-playtest process. This is normal content adoption, not part of the
external tool's execution.

## Delivery

### Step 1 — Python proof

- Create the external locked Python project.
- Detect the grids and transparent boundaries in all five images.
- Prove general grid inference with synthetic maps using unrelated dimensions, cell sizes, and
  origins.
- Recognize ground, water, vegetation, walls, and spawns.
- Generate one RON plus optional debug overlay for each image.
- Review the five outputs and adjust the recognition profile.

Status: complete on 2026-08-24. The locked Python 3.14 project, wheel, generalized synthetic tests,
five source-image regressions, generated RON files, and debug overlays passed. Brawler's actual Rust
RON deserializer parsed every generated recipe. The generated outputs remain candidate content and
are not added to Brawler automatically.

### Step 2 — Engine limits

- Remove the artificial minimum dimensions and 512-placement cap.
- Add narrow and dense fixtures plus measured safety gates.
- Do not add image-related code or dependencies to Brawler.

Status: not part of the external-tool implementation. No Brawler engine code changed.

### Step 3 — Content adoption

- Select one generated RON.
- Manually resolve or accept unsupported omissions.
- Add it through Brawler's normal map-content and playtest workflow.

Status: not started; this requires choosing and manually reviewing a generated map.

## Acceptance criteria

- The external Python command reads one image and directly writes one `.ron` file.
- It runs without Brawler, Rust, Cargo, Bevy, a server, or a network connection.
- Grid dimensions are derived from image evidence, never from known filenames, hashes, or sample
  dimensions.
- All five example grids and unrelated synthetic grids are detected exactly.
- Transparent border pixels do not become placements or change dimensions.
- Orthographic height does not shift logical cell ownership.
- Unsupported items are omitted and clearly reported.
- Repeated runs produce identical RON.
- Hyacinth House can be emitted as `23 x 35` without padding.
- Brawler can separately accept a legal narrow recipe and a legal recipe exceeding 512 placements
  after the engine-limit rework.
