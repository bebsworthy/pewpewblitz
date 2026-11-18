// Final colour step shared by the custom effect shaders (BRL-3ebg), imported by asset path.
//
// On an HDR camera the tonemapping post pass grades and tonemaps the whole view, so the colour
// passes through untouched. On a non-HDR camera (the Low quality tier) Bevy's mesh pipeline sets
// `TONEMAP_IN_SHADER` for every material, and the effect must apply the camera's tonemapper and
// colour grading itself, as `StandardMaterial` and `toon.wgsl` do, instead of clipping at 1.0 per
// channel. Bevy's `tone_mapping` grades rgb only and keeps alpha.

#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping::tone_mapping
#import bevy_pbr::mesh_view_bindings::view
#endif

fn effect_output(color: vec4<f32>) -> vec4<f32> {
#ifdef TONEMAP_IN_SHADER
    return tone_mapping(color, view.color_grading);
#else
    return color;
#endif
}
