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
    let quality = params.y;
    let radial = in.uv.x;
    let across = in.uv.y * 2.0 - 1.0;
    let point = vec2<f32>(radial * 4.2, across * 2.8) + in.world_position.xz * 0.0025;
    let leading_edge = smoothstep(0.925, 0.992, radial);
    let side_edge = smoothstep(0.91, 0.992, abs(across));
    let boundary = max(leading_edge, side_edge);

    if quality > 1.5 {
        let pulse = sin(radial * 8.0 - time * 1.1) * 0.5 + 0.5;
        let color = mix(vec3<f32>(0.025, 0.22, 0.10), vec3<f32>(0.10, 0.78, 0.35), pulse);
        return vec4<f32>(color, 0.075 + boundary * 0.13);
    }

    let broad_warp = value_noise(point * 0.58 + vec2<f32>(-time * 0.22, time * 0.08));
    let broad = value_noise(
        point * vec2<f32>(0.82, 1.16)
            + vec2<f32>(broad_warp * 1.4 - time * 0.33, time * 0.12)
    );
    let curl = value_noise(
        point * vec2<f32>(1.58, 2.15)
            + vec2<f32>(broad * 2.1 - time * 0.62, broad_warp * 1.5 + time * 0.20)
    );
    var detail = curl;
    if quality < 0.5 {
        let fine = value_noise(
            point * vec2<f32>(3.1, 4.4)
                + vec2<f32>(curl * 2.4 - time * 1.08, broad * 1.8 + time * 0.34)
        );
        detail = mix(curl, fine, 0.30);
    }

    let billow = smoothstep(0.32, 0.76, broad * 0.70 + detail * 0.34);
    let wisp_break = smoothstep(
        0.25,
        0.68,
        value_noise(vec2<f32>(radial * 5.2 - time * 0.72, across * 3.4 + broad * 0.8))
    );
    let wave_a = sin(radial * 7.2 - time * 1.15 + broad * 1.7) * 0.11;
    let wave_b = sin(radial * 5.1 - time * 0.86 + curl * 1.2) * 0.14;
    let wave_c = sin(radial * 8.8 - time * 1.38 + detail * 1.4) * 0.085;
    let stream_a = 1.0 - smoothstep(0.035, 0.105, abs(across - (0.48 + wave_a)));
    let stream_b = 1.0 - smoothstep(0.040, 0.125, abs(across - (0.04 + wave_b)));
    let stream_c = 1.0 - smoothstep(0.035, 0.105, abs(across - (-0.48 + wave_c)));
    let stream_d = 1.0 - smoothstep(0.030, 0.085, abs(across - (-0.22 - wave_a * 0.7)));
    let filament = max(max(stream_a, stream_b), max(stream_c, stream_d))
        * wisp_break
        * smoothstep(0.10, 0.28, radial);
    let source_fade = smoothstep(0.02, 0.20, radial);
    let alpha = source_fade * (0.020 + billow * 0.060 + filament * 0.18) + boundary * 0.16;

    let shadow = vec3<f32>(0.012, 0.105, 0.055);
    let gas = vec3<f32>(0.045, 0.56, 0.30);
    let lit = vec3<f32>(0.26, 1.55, 0.72);
    var color = mix(shadow, gas, billow);
    color = mix(color, lit, filament * 0.82 + boundary * 0.42);
    return vec4<f32>(color, alpha);
}
