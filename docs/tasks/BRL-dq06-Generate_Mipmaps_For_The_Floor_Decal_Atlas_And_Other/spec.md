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
