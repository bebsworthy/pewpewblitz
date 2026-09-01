#import bevy_pbr::forward_io::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> params: vec4<f32>;

@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var<uniform> wake_color: vec4<f32>;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let time = params.x;
    let opacity = params.y;
    let quality = params.z;
    let along = clamp(in.uv.x, 0.0, 1.0);
    let across_signed = in.uv.y * 2.0 - 1.0;
    let across = abs(across_signed);

    let tail_gate = smoothstep(0.0, 0.16, along);
    let head_gate = 1.0 - smoothstep(0.93, 1.0, along);
    let tapered_edge = 1.0 - smoothstep(0.48 + along * 0.28, 0.98, across);
    let central_lane = 1.0 - smoothstep(0.04, 0.24, across);
    let outer_lane = smoothstep(0.20, 0.48, across) * (1.0 - smoothstep(0.48, 0.82, across));

    var filament = 0.78;
    var speed_cut = 0.0;
    if quality < 0.5 {
        let bend = sin(along * 13.0 - time * 11.0) * 0.09;
        let strand = abs(across_signed - bend);
        filament = 1.0 - smoothstep(0.05, 0.22, strand);
        let cut_wave = sin(along * 54.0 - time * 22.0 + across * 7.0) * 0.5 + 0.5;
        speed_cut = smoothstep(0.90, 0.99, cut_wave) * outer_lane;
    }

    let body = tail_gate * head_gate * tapered_edge;
    let alpha = body * opacity * (
        0.16
        + central_lane * 0.54
        + outer_lane * 0.20
        + filament * 0.23
        + speed_cut * 0.42
    );
    if alpha < 0.012 {
        discard;
    }

    let tail_cooling = mix(0.42, 1.0, smoothstep(0.0, 0.72, along));
    let white_hot = vec3<f32>(3.1, 3.55, 3.8);
    let hot_mix = clamp(central_lane * (0.32 + along * 0.50) + speed_cut * 0.35, 0.0, 0.88);
    let color = mix(wake_color.rgb * tail_cooling, white_hot, hot_mix);
    return vec4<f32>(color, alpha);
}
