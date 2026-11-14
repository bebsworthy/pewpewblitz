# Outcome

Client textures that can be drawn smaller than their pixel size (first the BRL-0178 floor decal
atlas, `assets/brawler/textures/original/environment/floor-decals.png`) carry mipmaps, so they never
shimmer or alias on phones, low resolution scales, or zoomed-out map framing.

# Context

- Bevy 0.19 loads PNGs without mipmaps; the client's default sampler is nearest. BRL-0178 loads the
  decal atlas with a linear sampler and keeps its art soft, which is enough at today's desktop
  framing (a cell spans roughly 60-140 physical pixels), but there is no mip chain.
- Options: ship KTX2 with baked mips (needs the `ktx2`/`zstd` Bevy features), build the mip chain
  in code once the image loads (box filter on the RGBA8 data, `mip_level_count`), or emit the mips
  from the asset_src script.

# Scope

- Pick one approach for the decal atlas and document it in `docs/11`.
- Apply it to any other client texture only where a minification problem is demonstrated.

# Acceptance criteria

- The decal atlas samples a full mip chain with trilinear filtering.
- Native captures at a small window size (and on the Pixel 8a when that tier is next measured)
  show no shimmer on decals while the camera moves.

# Verification

- A focused test for mip generation or the shipped mip count; native captures; `just test`.
