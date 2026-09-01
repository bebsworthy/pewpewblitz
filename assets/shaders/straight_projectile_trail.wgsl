#import bevy_pbr::forward_io::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> params: vec4<f32>;

@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var<uniform> trail_color: vec4<f32>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let time = params.x;
    let quality = params.y;
    let along = clamp(in.uv.x, 0.0, 1.0);
    let across = abs(in.uv.y * 2.0 - 1.0);
    let edge = 1.0 - smoothstep(0.34, 1.0, across);
    let head = smoothstep(0.08, 0.98, along);
    let tail_fade = smoothstep(0.0, 0.22, along);

    var filament = 0.72;
    if quality < 0.5 {
        let fast = sin(along * 27.0 - time * 16.0 + across * 5.0) * 0.5 + 0.5;
        let slow = sin(along * 11.0 - time * 8.0 - across * 3.0) * 0.5 + 0.5;
        filament = mix(0.56, 1.0, fast * 0.62 + slow * 0.38);
    }

    let hot_center = 1.0 - smoothstep(0.0, 0.32, across);
    let alpha = tail_fade * edge * head * (0.18 + filament * 0.42 + hot_center * 0.28);
    if alpha < 0.01 {
        discard;
    }
    let white_hot = vec3<f32>(2.8, 3.2, 3.4);
    let color = mix(trail_color.rgb * (0.42 + filament * 0.58), white_hot, hot_center * head * 0.62);
    return vec4<f32>(color, alpha);
}
