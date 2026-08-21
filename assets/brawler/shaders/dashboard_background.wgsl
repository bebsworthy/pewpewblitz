#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> time_motion: vec4<f32>;
@group(1) @binding(1) var<uniform> palette_dark: vec4<f32>;
@group(1) @binding(2) var<uniform> palette_light: vec4<f32>;
@group(1) @binding(3) var<uniform> glow: vec4<f32>;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let time = time_motion.x * time_motion.y;
    let vertical = smoothstep(0.0, 1.0, 1.0 - uv.y);
    let broad_drift = 0.5 + 0.5 * sin(
        uv.x * 3.2 + uv.y * 1.7 + time * 0.22
    );
    let mix_amount = clamp(vertical * 0.34 + broad_drift * 0.045, 0.0, 0.44);
    var color = mix(palette_dark.rgb, palette_light.rgb, mix_amount);

    let centered = vec2<f32>(
        (uv.x - glow.x) / max(glow.z, 0.001),
        (uv.y - glow.y) / max(glow.w, 0.001)
    );
    let breath = 1.0 + time_motion.y * 0.025 * sin(time * 0.7);
    let halo = exp(-dot(centered, centered) / breath);
    color += vec3<f32>(0.02, 0.42, 0.52) * halo * 0.22;
    return vec4<f32>(color, 1.0);
}
