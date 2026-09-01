#import bevy_pbr::forward_io::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> params: vec4<f32>;

fn hash21(point: vec2<f32>) -> f32 {
    var value = fract(point * vec2<f32>(127.1, 311.7));
    value += dot(value, value + vec2<f32>(33.33));
    return fract(value.x * value.y);
}

fn value_noise(point: vec2<f32>) -> f32 {
    let cell = floor(point);
    let local = fract(point);
    let curve = local * local * (vec2<f32>(3.0) - 2.0 * local);
    let lower = mix(hash21(cell), hash21(cell + vec2<f32>(1.0, 0.0)), curve.x);
    let upper = mix(
        hash21(cell + vec2<f32>(0.0, 1.0)),
        hash21(cell + vec2<f32>(1.0, 1.0)),
        curve.x
    );
    return mix(lower, upper, curve.y);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let time = params.x;
    let fuse = params.y;
    let reduced = params.z;
    let seed = params.w;
    let uv = in.uv;
    let scroll = vec2<f32>(time * 0.18 + seed * 3.7, -time * 0.11 + seed * 2.3);
    let broad = value_noise(uv * vec2<f32>(5.0, 3.5) + scroll);
    var detail = value_noise(uv * vec2<f32>(11.0, 7.0) - scroll * 1.8 + broad * 1.7);
    if reduced > 0.5 {
        detail = broad;
    }

    let vein_wave = sin((uv.x * 10.0 + uv.y * 7.0 + detail * 3.2) - time * 2.4);
    let veins = smoothstep(0.58, 0.96, vein_wave * 0.5 + 0.5) * smoothstep(0.30, 0.76, broad);
    let heartbeat = pow(max(0.0, sin(time * (3.2 + fuse * 3.8))), 7.0);
    let urgency = smoothstep(0.58, 1.0, fuse);
    let latitude = abs(uv.y * 2.0 - 1.0);
    let rim = smoothstep(0.45, 0.98, latitude);

    let deep = vec3<f32>(0.17, 0.015, 0.31);
    let body = vec3<f32>(0.62, 0.08, 0.92);
    let hot = vec3<f32>(2.4, 0.34, 3.2);
    var color = mix(deep, body, broad * 0.62 + detail * 0.18);
    color = mix(color, hot, veins * (0.36 + urgency * 0.42) + heartbeat * urgency * 0.28);
    let alpha = 0.24 + broad * 0.16 + veins * 0.26 + rim * 0.14;
    return vec4<f32>(color, alpha);
}
