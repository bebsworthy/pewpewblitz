// Shared stylized toon shading for fighters and the world (BRL-0150).
//
// Keeps Bevy's StandardMaterial inputs (textures, vertex colours, alpha, normal maps, emissive) and
// replaces PBR direct lighting with a two-band ramp, a hue-shifted shadow band, Bevy's diffuse
// ambient term, and a lit-side rim. Tuning comes from `assets/catalogs/toon_shading.ron`.

#import bevy_pbr::{
    ambient,
    forward_io::{VertexOutput, FragmentOutput},
    mesh_functions,
    mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT,
    mesh_view_bindings as view_bindings,
    mesh_view_types::DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, main_pass_post_lighting_processing},
    pbr_types::{PbrInput, STANDARD_MATERIAL_FLAGS_UNLIT_BIT},
    shadows,
}

struct ToonSettings {
    // rgb: shadow band multiplier on albedo.
    shadow_tint: vec4<f32>,
    // x: N·L threshold, y: band softness, z: flat light level, w: N·L gradient in the lit band.
    ramp: vec4<f32>,
    // x/y: rim start/end on 1 − N·V; z/w: start/end on how far the normal turns left or right on
    // screen, so the rim stays on side silhouettes instead of faces tilted toward the high camera.
    rim: vec4<f32>,
    // rgb: rim colour, a: rim strength. Indexed by the mesh tag: 0 world, 1 local, 2 ally, 3 enemy.
    rim_colors: array<vec4<f32>, 4>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> toon: ToonSettings;

const INV_PI: f32 = 0.31830988618;
const WORLD_TAG: u32 = 0u;

// Light before exposure, matching the units of Bevy's PBR direct + indirect terms.
fn toon_lighting(in: PbrInput, tag: u32) -> vec3<f32> {
    let albedo = in.material.base_color.rgb;
    let threshold = toon.ramp.x;
    let softness = toon.ramp.y;
    let flat_level = toon.ramp.z;
    let gradient = toon.ramp.w;
    let view_z = dot(vec4<f32>(
        view_bindings::view.view_from_world[0].z,
        view_bindings::view.view_from_world[1].z,
        view_bindings::view.view_from_world[2].z,
        view_bindings::view.view_from_world[3].z
    ), in.world_position);
    // Fighters (tagged) ignore received shadow maps so faces and silhouettes stay readable.
    let receives_shadows = tag == WORLD_TAG && (in.flags & MESH_FLAGS_SHADOW_RECEIVER_BIT) != 0u;

    var direct = vec3<f32>(0.0);
    var key = vec3<f32>(0.0);
    var lit = 0.0;
    let n_directional_lights = view_bindings::lights.n_directional_lights;
    for (var i: u32 = 0u; i < n_directional_lights; i = i + 1u) {
        let light = &view_bindings::lights.directional_lights[i];
        let n_dot_l = dot(in.N, (*light).direction_to_light);
        var shadow = 1.0;
        if receives_shadows && ((*light).flags & DIRECTIONAL_LIGHT_FLAGS_SHADOWS_ENABLED_BIT) != 0u {
            shadow = shadows::fetch_directional_shadow(
                i, in.world_position, in.world_normal, view_z, in.frag_coord.xy
            );
        }
        let band = smoothstep(threshold - softness, threshold + softness, n_dot_l) * shadow;
        let lit_level = mix(flat_level, saturate(n_dot_l), gradient);
        let ramp = mix(toon.shadow_tint.rgb * flat_level, vec3<f32>(lit_level), band);
        let radiance = (*light).color.rgb * INV_PI;
        direct += albedo * radiance * ramp;
        key += radiance;
        lit = max(lit, band);
    }

    let n_dot_v = max(dot(in.N, in.V), 0.0001);
    let indirect = ambient::ambient_light(
        in.world_position, in.N, in.V, n_dot_v, albedo, vec3<f32>(0.0), 1.0, in.diffuse_occlusion
    );

    // Mix (not add) toward the rim colour at full key light: never brighter than lit white.
    let rim = toon.rim_colors[min(tag, 3u)];
    let view_normal = (view_bindings::view.view_from_world * vec4<f32>(in.N, 0.0)).xyz;
    let sideways = abs(view_normal.x) / max(length(view_normal), 0.0001);
    let rim_mask = smoothstep(toon.rim.x, toon.rim.y, 1.0 - n_dot_v)
        * smoothstep(toon.rim.z, toon.rim.w, sideways) * lit * rim.a;
    return mix(direct + indirect, rim.rgb * key, rim_mask);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    if (pbr_input.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        let tag = mesh_functions::get_tag(in.instance_index);
        let exposure = view_bindings::view.exposure;
        let alpha = pbr_input.material.base_color.a;
        let emissive = pbr_input.material.emissive;
        let emissive_light = emissive.rgb * alpha * mix(1.0, exposure, emissive.a);
        out.color = vec4<f32>(exposure * toon_lighting(pbr_input, tag) + emissive_light, alpha);
    } else {
        out.color = pbr_input.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
