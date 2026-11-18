Reviewed 2026-11-15 against HEAD 3437c0d8: valid; spec refreshed (KTX2 + zstd are already enabled via `bevy/tonemapping_luts`, decals are off on Low so the 8a/Medium is the relevant device tier).

# Outcome

Client textures that can be drawn smaller than their pixel size (first the BRL-23q3 floor decal atlas, `assets/brawler/textures/original/environment/floor-decals.png`, 384x256, 3x2 cells of 128 px) carry mipmaps, so they never shimmer or alias on phones, low resolution scales, or zoomed-out map framing.

# Context (verified)

- Atlas loaded with a linear sampler and no mips: `presentation_3d/environment_assets/runtime.rs:166-172`; path const `presentation_3d/floor_decals.rs:23`; UV layout `floor_decals.rs:417` `atlas_uv`. App default sampler is nearest (`lib.rs:664`, `vfx_lab.rs:728` `ImagePlugin::default_nearest()`).
- Generator `asset_src/environment/floor-decals/build_decals.py` (CELL=128, transparent border per cell, l.14-40).
- Root `Cargo.toml:99-101` enables `bevy/tonemapping_luts`, which in Bevy 0.19.1 pulls `ktx2` + `bevy_image/zstd`, plus `bevy/zstd_rust`: KTX2 with baked mips is loadable with no new feature.
- Quality tiers: `floor_decals: false` on Low (`render_quality.ron:43`), true on High/Medium; the Pixel 8a resolves Auto to Medium, so it does show decals.
- docs/11 floor-decal section: `docs/11-art-and-presentation-direction.md:286-300` (no mip note yet).

# Plan

1. Recommended: emit a KTX2 with a pre-filtered mip chain from `build_decals.py` (premultiplied-aware box filter per cell, stop at the level where a cell is ~8 px, i.e. 5 levels, so cells never bleed), ship `floor-decals.ktx2`, point `FLOOR_DECAL_ATLAS_PATH` at it. Alternative if Python KTX2 writing is awkward: generate mips in code on `AssetEvent::LoadedWithDependencies` (RGBA8 box filter, set `mip_level_count`), same level cap.
2. Sampler: `ImageSamplerDescriptor { mag/min: Linear, mipmap_filter: Linear, .. }` instead of `ImageSampler::linear()`.
3. Document the chosen approach and the level cap in docs/11 next to the decal paragraph.
4. Apply to other textures only on demonstrated shimmer (none known).

# Acceptance

- Focused test: loaded/generated atlas has the expected `mip_level_count` (≥5) and the material sampler uses a linear mip filter.
- Native captures at a small window (e.g. 640x360) with the camera moving: no decal shimmer.
- Pixel 8a device check (Medium tier) when that tier is next measured.
- `just lint`, `just test`.


# Result (2026-11-18)

**Approach: KTX2 with a baked mip chain (recommended path).** `build_decals.py` now writes `assets/brawler/textures/original/environment/floor-decals.ktx2` (RGBA8 sRGB, `VK_FORMAT_R8G8B8A8_SRGB`, straight alpha, Basic DFD, zstd supercompression via Python 3.14+ stdlib `compression.zstd`, 49 KB) and the old `floor-decals.png` is removed. Level 0 is byte-identical to the old PNG. Levels 1-4 are 2x2 box-filtered per sprite cell, colour weighted by alpha in linear light, with transparent texels kept white. The chain is **capped at 5 levels (8 px cells)**, so no level averages two sprites. Rebuilds are byte-identical given the same zstd version (1.5.7). `FLOOR_DECAL_ATLAS_PATH` points at the `.ktx2`. The loader sampler is now an explicit `floor_decal_atlas_sampler()` (mag/min/mipmap Linear). The old `ImageSampler::linear()` already set mipmap Linear, so this is for clarity and the test.

**Loader research (vendored bevy_image 0.19.1 `ktx2.rs`, ktx2 0.5.0).** `R8G8B8A8_SRGB` maps to `Rgba8UnormSrgb` when `is_srgb` is true (the loader default). Uncompressed formats always pass the GPU-support check. Zstandard is decoded by `ruzstd` under `zstd_rust`, which the client enables together with `ktx2` through `bevy/tonemapping_luts`. Supercompression is not required, but it cuts 524 KB to 49 KB. `mip_level_count` = the header `levelCount`, and the data is MipMajor.

**Android.** `packages/brawler-android/android/app/build.gradle` packages the whole repo `assets/` directory (`assets.srcDirs`), and Bevy reads it through `AndroidAssetReader`, so the `.ktx2` ships like any other asset. AAPT may deflate it, which AAsset reads transparently. The Android crate depends on `brawler` with `client` → `bevy-client`, which carries `tonemapping_luts` + `zstd_rust`, so the same loader and decoder are compiled in. The Pixel 8a has no new format requirement because the texture is uncompressed RGBA8. Not run on a device.

**Tests.** `presentation_3d::floor_decals::tests::generated_atlas_ships_mipmapped_with_the_expected_layout` replaces the PNG-header test. It decodes the shipped file through `Image::from_buffer(.., ImageType::Extension("ktx2"), ..)` (the ImageLoader path) and asserts 384x256, `Rgba8UnormSrgb`, `mip_level_count == 5`, the full chain's data length, and a sampler with Linear mip/min/mag filters.

**Verification.**
- `just lint`: pass (`target/dq06/lint.log`; first run failed on rustfmt only, fixed with `cargo fmt`).
- `just test`: pass (`target/dq06/test.log`). Focused decal tests: 9/9 after fmt.
- Native captures: `scripts/v3-render-evidence.sh` with `BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_BIND=127.0.0.1:5037 BRAWLER_RENDER_WINDOW_SIZE=640x360 BRAWLER_RENDER_WARMUP_SECONDS=6 BRAWLER_RENDER_MEASURE_SECONDS=11` on HEAD (before) and with the change (after). The camera follows a move axis of 1,0. Both reports are `result=pass` (`target/dq06/{before,after}/report.txt`, captures beside them), with no asset-load errors. The tier was High on desktop.
- `target/dq06/compare.png`, top two rows: zoomed (3x nearest) crops of the stain and crack decals by the left wall in frames 300 and 480 (top row before, second row after). The decals stay present and consistent, and are slightly softer after (mip ~1 at this framing).
- `compare.png`, bottom: an offline minification model built from the shipped file. Every sprite is tinted over a theme floor at 48, 24 and 16 px cells, each at four sub-pixel offsets (the rows within a size). Left is before (bilinear level 0), right is after (trilinear). Before, the cracks, scuffs and stain rims break into dots that change with the offset (shimmer). After, they stay stable.
- Limitation: the desktop window renders at 2x scale (1280x720 physical), so a decal cell covers ~55 px there and native minification is mild. Three stills 3 s apart cannot show frame-to-frame shimmer. The strong-minification case is shown by the offline model only.
- Known limit: at the 16 px and 8 px levels a sprite's outermost texels carry alpha (up to 95 and 122 of 255), so a decal covering 16 physical px or fewer may pick up a half-texel fringe from its atlas neighbour. That level is not reached at gameplay zoom. This is documented in the README.

**Docs.** The docs/11 floor-decal paragraph records the approach and the 5-level cap. The generator README atlas contract and the license file name the `.ktx2`.

**Owner playtest (not done here):** Pixel 8a, Auto → Medium tier (decals on). Run `just android-build <host:port>` / `just android-install` and play a practice match on a map with wall-adjacent decals (e.g. theme 1 stain/crack near the arena edge). Walk along walls and watch decal edges (crack lines, stain rim, scuff arcs) for crawling or sparkle while the camera moves. Also confirm the decals still appear, so the KTX2 loaded. Known limits: decals look slightly softer than before at distance, which is expected with mips.

Not done: no other textures were changed (none shown to shimmer; Plan step 4).
