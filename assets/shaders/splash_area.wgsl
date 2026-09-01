#import bevy_pbr::forward_io::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> params: vec4<f32>;

const TAU: f32 = 6.28318530718;

fn ring(radius: f32, center: f32, width: f32) -> f32 {
    return 1.0 - smoothstep(width * 0.45, width, abs(radius - center));
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let point = in.uv * 2.0 - vec2<f32>(1.0, 1.0);
    let radius = length(point);
    if radius > 1.0 {
        discard;
    }

    let time = params.x;
    let reduced = params.y;
    let angle = atan2(point.y, point.x);
    let edge_fade = 1.0 - smoothstep(0.94, 1.0, radius);
    let quiet_center = smoothstep(0.12, 0.34, radius);
    let pulse_a = ring(radius, fract(time * 0.20) * 0.78 + 0.12, 0.055);
    let pulse_b = ring(radius, fract(time * 0.20 + 0.5) * 0.78 + 0.12, 0.050);

    var caustic = 0.34;
    var foam = 0.0;
    if reduced < 0.5 {
        let crossing = sin(angle * 7.0 + radius * 22.0 - time * 2.7)
            * sin(angle * 4.0 - radius * 17.0 + time * 1.9);
        caustic = smoothstep(0.34, 0.86, crossing * 0.5 + 0.5);
        let edge_wave = sin(angle * 11.0 - time * 3.1 + sin(angle * 3.0 + time) * 1.4);
        foam = smoothstep(0.55, 0.92, edge_wave * 0.5 + 0.5)
            * smoothstep(0.72, 0.88, radius);
    }

    let ripples = max(pulse_a, pulse_b) * quiet_center;
    let base = 0.045 * edge_fade;
    let activity = ripples * 0.24 + caustic * 0.09 * quiet_center + foam * 0.24;
    let alpha = (base + activity) * edge_fade;
    if alpha < 0.01 {
        discard;
    }

    let deep = vec3<f32>(0.015, 0.18, 0.18);
    let restorative = vec3<f32>(0.05, 1.55, 0.72);
    let impact = vec3<f32>(0.18, 2.35, 3.25);
    var color = mix(deep, restorative, clamp(caustic * 0.55 + ripples * 0.40, 0.0, 1.0));
    color = mix(color, impact, clamp(ripples * 0.62 + foam * 0.58, 0.0, 0.82));
    return vec4<f32>(color, alpha);
}
