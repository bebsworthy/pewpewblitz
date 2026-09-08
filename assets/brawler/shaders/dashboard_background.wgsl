#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> time_motion: vec4<f32>;
@group(1) @binding(1) var<uniform> palette_dark: vec4<f32>;
@group(1) @binding(2) var<uniform> palette_light: vec4<f32>;
@group(1) @binding(3) var<uniform> glow: vec4<f32>;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    // Two deliberate graphic planes. Reduced motion fixes their edges in place.
    let drift = sin(time_motion.x * 0.18) * 0.004 * time_motion.y;
    let aa = max(fwidth(uv.y), 0.0001);
    let teal = smoothstep(-aa, aa, uv.y - (0.25 + uv.x * 1.15 + drift));
    // Deliberate junction behind the hero, then a low cream cutout on the right.
    var orange_edge = 1.04 - uv.x * 0.52;
    if uv.x > 0.44 { orange_edge = mix(0.64, 0.835, clamp((uv.x - 0.44) / 0.13, 0.0, 1.0)); }
    if uv.x > 0.57 { orange_edge = mix(0.835, 0.92, clamp((uv.x - 0.57) / 0.37, 0.0, 1.0)); }
    if uv.x > 0.94 { orange_edge = mix(0.92, 0.90, (uv.x - 0.94) / 0.06); }
    let orange = smoothstep(-aa, aa, uv.y - orange_edge - drift);
    var color = mix(palette_dark.rgb, palette_light.rgb, teal);
    color = mix(color, glow.rgb, orange);
    // Broad tonal finish only: no noise, grunge or repeated pattern.
    let ground = (uv - vec2<f32>(0.44, 0.83)) / vec2<f32>(0.12, 0.012);
    color *= 1.0 - 0.16 * exp(-dot(ground, ground));
    color *= 1.0 - 0.035 * length(uv - vec2<f32>(0.45, 0.38));
    return vec4<f32>(color, 1.0);
}
