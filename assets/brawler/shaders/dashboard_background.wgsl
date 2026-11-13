#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> time_motion: vec4<f32>;
// `shell-bg-top` and `shell-bg-bottom`.
@group(1) @binding(1) var<uniform> palette_dark: vec4<f32>;
@group(1) @binding(2) var<uniform> palette_light: vec4<f32>;
// `ink`, used for the darker tonal plane.
@group(1) @binding(3) var<uniform> glow: vec4<f32>;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    // The shell-bg gradient with two deliberate low-contrast planes. Reduced motion fixes their
    // edges in place.
    let drift = sin(time_motion.x * 0.18) * 0.004 * time_motion.y;
    let aa = max(fwidth(uv.y), 0.0001);
    var color = mix(palette_dark.rgb, palette_light.rgb, uv.y);
    let shade = smoothstep(-aa, aa, uv.y - (0.25 + uv.x * 1.15 + drift));
    color = mix(color, glow.rgb, 0.14 * shade);
    // Deliberate junction behind the hero, then a low lighter strip on the right.
    var lift_edge = 1.04 - uv.x * 0.52;
    if uv.x > 0.44 { lift_edge = mix(0.64, 0.835, clamp((uv.x - 0.44) / 0.13, 0.0, 1.0)); }
    if uv.x > 0.57 { lift_edge = mix(0.835, 0.92, clamp((uv.x - 0.57) / 0.37, 0.0, 1.0)); }
    if uv.x > 0.94 { lift_edge = mix(0.92, 0.90, (uv.x - 0.94) / 0.06); }
    let lift = smoothstep(-aa, aa, uv.y - lift_edge - drift);
    color = mix(color, vec3<f32>(1.0), 0.08 * lift);
    // Broad tonal finish only: a soft contact shadow under the hero, no noise or pattern.
    let ground = (uv - vec2<f32>(0.44, 0.83)) / vec2<f32>(0.12, 0.012);
    color *= 1.0 - 0.22 * exp(-dot(ground, ground));
    return vec4<f32>(color, 1.0);
}
