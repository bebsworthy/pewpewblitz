Reviewed 2026-11-15 against HEAD 3437c0d8: valid; premise confirmed in Bevy 0.19.1 source (custom materials get `TONEMAP_IN_SHADER` but our WGSL never calls `tone_mapping`). Refs added.

# Outcome

Low-tier (non-HDR) arena rendering shows custom effect shaders with the same tonemapper and grading as the rest of the world.

# Context (verified)

- `assets/catalogs/render_quality.ron:39` low tier `hdr: false`; `presentation_3d/render_quality.rs:326-329` removes `Hdr` from the camera. Low is chosen by Auto only for GPUs without compute particles, or forced via Settings > Display > Graphics quality (`settings/persistence.rs:39` `graphics_quality`).
- Bevy 0.19.1 `bevy_pbr/src/render/mesh.rs:479,3486`: when the view is not HDR the mesh pipeline key sets `TONEMAP_IN_SHADER` and pushes the shader def + LUT bindings for every `Material` (custom ones too). `StandardMaterial` / our `toon.wgsl:108` apply it through `main_pass_post_lighting_processing`; `fighter_outline.wgsl` should be checked too.
- The seven custom effect shaders return raw linear colour and import only `bevy_pbr::forward_io::VertexOutput`: `assets/shaders/{fire_area,elemental_support_area,splash_area,sticky_blob,dash_trail,cone_spray,straight_projectile_trail}.wgsl`. Material impls: `presentation_3d/combat/{elemental_style.rs:50,96, splash_style.rs:35, sticky_blob.rs:31, dash_style.rs:34, cone_spray.rs:42, straight_projectile_style.rs:39}`.
- BRL-farz capped non-glow peaks at 2.0, so on Low the 1.0–2.0 range clips per channel and theme grading is skipped. Not yet seen on a device.

# Plan

1. In each of the seven fragment shaders, at the end:
   `#ifdef TONEMAP_IN_SHADER` → `color = tone_mapping(color, view.color_grading);` with `#import bevy_core_pipeline::tonemapping::tone_mapping` and `#import bevy_pbr::mesh_view_bindings::view` inside the ifdef. Alpha-blended shaders: tonemap rgb only, keep alpha.
2. Optionally factor a tiny shared import (`assets/shaders/effect_output.wgsl`, `#define_import_path brawler::effect_output`) so the seven stay identical.
3. Check `fighter_outline.wgsl` for the same gap; fix only if it emits colours that need grading.
4. HDR tiers: shader def absent → unchanged by construction.

# Acceptance

- Desktop captures with Graphics quality forced to Low, before/after: fire, poison, restoration, splash, sticky, dash trail, cone spray, straight trail (VFX lab or Practice; the lab uses the production world look).
- High/Medium captures identical before/after.
- `just lint`, `just test`.
- Pixel 8a check optional (8a resolves Auto to Medium, so force Low in Settings to see it).


# Implementation (2026-11-18)

## Decisions
- Bevy 0.19.1 source verified (`~/.cargo/registry/.../bevy_core_pipeline-0.19.1/src/tonemapping/tonemapping_shared.wgsl:1,376`): `#define_import_path bevy_core_pipeline::tonemapping`, `fn tone_mapping(in: vec4<f32>, in_color_grading: ColorGrading) -> vec4<f32>` grades/tonemaps rgb and returns `vec4(color, in.a)`, so alpha is kept. `view.color_grading` via `bevy_pbr::mesh_view_bindings::view` (same as `pbr_functions.wgsl:1015` and `bevy_sprite_render` sprite shaders). `bevy_pbr-0.19.1/src/render/mesh.rs:477-485,3486` confirmed: non-HDR camera + `Tonemapping` → `TONEMAP_IN_SHADER` + LUT bindings for every material.
- Shared step `assets/shaders/effect_output.wgsl` with `fn effect_output(color: vec4<f32>) -> vec4<f32>`: `tone_mapping(color, view.color_grading)` under `#ifdef TONEMAP_IN_SHADER`, identity otherwise. Imported by **asset path** (`#import "shaders/effect_output.wgsl"::effect_output`) with no `#define_import_path`: Bevy's shader loader (`bevy_shader-0.19.1/src/shader.rs:371-375`) loads asset-path imports as file dependencies automatically, so no Rust-side shader-library registration is needed.
- All seven shaders (fire_area, elemental_support_area, splash_area, sticky_blob, dash_trail, cone_spray ×2 returns, straight_projectile_trail) wrap their fragment returns in `effect_output(...)`. HDR tiers: shader def absent → identity → byte-identical output (verified below).
- Deband dither (`DEBAND_DITHER`) not added: StandardMaterial applies it after tonemapping, but it's a separate polish concern and not part of the clipping/grading gap.
- `fighter_outline.wgsl` (incl. BRL-y3hb's depth-push uniform) left untouched: it emits one flat authored sRGB ink colour (`#0E1638`, well under 1.0), so it never clips, and the same material renders on the always-non-HDR offscreen previews (Dashboard/card), where adding tonemapping would shift the just-tuned preview outline on every tier. Known residual: on Low/preview cameras the outline is not tonemapped while on HDR arena it is (dark navy, small shift). Recorded in docs/11; not filed as a ticket.
- docs/11 world-look paragraph updated (replaces the "clip at 1.0 per channel" limitation).
- Contract test `world_look::tests::custom_effect_shaders_finish_through_the_shared_output_step` asserts the import and that every fragment `return` goes through `effect_output`.

## Verification
- Captures: VFX lab with a **temporary** hook (env `BRAWLER_3EBG_QUALITY=low|high` installing the production `RenderQualityPlugin` + `graphics_quality` override; removed afterwards, `vfx_lab.rs` unchanged). Logs confirm `tier="LOW" hdr=false bloom=false msaa=Off`. Args: `--identity 7 --paused --sample-ticks 40 --window-size 1280x720 --capture-after 30 --exit-after-capture` per fixture (field ×fire/poison/restoration accents, splash, sticky, dash with `--sample-ticks 6` because the wake is gone by 40, cone, straight). Shaders are runtime assets, so before/after used the same binary. Repeat runs are byte-identical (determinism check).
- Paths: `target/3ebg/{before,after}-{low,high}/<effect>/*.png` (+ JSON evidence), side-by-side crops `target/3ebg/compare/{low,high}-<effect>-before-vs-after.png`.
- High before vs after: 0 differing pixels for all eight effects.
- Low before vs after (2560×1440): fire 151,701 px changed (max Δ66/255), poison 187,950 (Δ40), restoration 240,070 (Δ48), splash 227,541 (Δ36), sticky 6,711 (Δ42), dash 51,309 (Δ63), cone 840,765 (Δ43), straight 11,647 (Δ51).
- Visual: Low-before fire ring was a hard-clipped saturated yellow halo/ring; after it rolls off to the same orange rim as High. Restoration arcs and cone filaments were saturated mint/teal; after they are the softer graded greens matching High. Dash wake was saturated cyan with a clipped white core; after it is the graded blue-grey wake with a softer core. Low-after now reads like High (minus bloom/MSAA).
- `just lint` pass; `just test` pass.
