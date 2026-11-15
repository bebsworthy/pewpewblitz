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
