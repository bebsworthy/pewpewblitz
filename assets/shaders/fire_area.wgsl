#import bevy_pbr::forward_io::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> params: vec4<f32>;

const TAU: f32 = 6.28318530718;

fn hash21(point: vec2<f32>) -> f32 {
    var value = fract(point * vec2<f32>(123.34, 456.21));
    value += dot(value, value + vec2<f32>(45.32));
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
    let point = in.uv * 2.0 - vec2<f32>(1.0, 1.0);
    let radius = length(point);
    if radius > 1.0 {
        discard;
    }

    let time = params.x;
    let boundary = params.y;
    let direction = point / max(radius, 0.001);
    let broad_warp = value_noise(
        direction * 3.4 + vec2<f32>(time * 0.31, -time * 0.23)
    );
    let broad = value_noise(
        direction * 5.8
        + vec2<f32>(broad_warp * 2.1 - time * 0.38, time * 0.19)
    );
    let medium = value_noise(
        direction * 11.5
        + vec2<f32>(broad * 2.7 + time * 0.54, broad_warp * 1.9 - time * 0.61)
    );
    let fine = value_noise(
        direction * 22.0
        + vec2<f32>(medium * 3.2 - time * 1.13, broad * 2.4 + time * 0.87)
    );

    // A broad connected body plus smaller high-energy tongues. Sampling noise in
    // circular coordinates keeps the circumference seamless and avoids a repeating
    // row of geometric teeth.
    let tongue = pow(clamp(medium * 0.78 + fine * 0.22, 0.0, 1.0), 2.35);
    let flame_depth = 0.105 + broad * 0.075 + medium * 0.045 + tongue * 0.075;
    let inner_edge = boundary - flame_depth;
    let body = smoothstep(inner_edge - 0.027, inner_edge + 0.028, radius);
    let body_outer = 1.0 - smoothstep(boundary - 0.006, boundary + 0.014, radius);
    let depth = clamp((radius - inner_edge) / max(flame_depth, 0.001), 0.0, 1.0);

    let flow = value_noise(
        direction * 15.0
        + vec2<f32>((1.0 - depth) * 5.0 - time * 1.28, depth * 7.0 + time * 0.72)
    );
    let porous_body = mix(0.48, 1.0, smoothstep(0.20, 0.76, flow + depth * 0.20));
    let solid_outer = smoothstep(0.58, 0.92, depth);
    let body_alpha = body * body_outer * mix(porous_body, 1.0, solid_outer) * 0.96;

    // The halo is deliberately subordinate to the exact gameplay ring. The effect
    // remains readable if post-processing is disabled.
    let halo_inner = smoothstep(boundary - 0.025, boundary + 0.006, radius);
    let halo_outer = 1.0 - smoothstep(boundary + 0.004, 1.0, radius);
    let halo_alpha = halo_inner * halo_outer * 0.24;
    let alpha = max(body_alpha, halo_alpha);
    if alpha < 0.01 {
        discard;
    }

    let ember = vec3<f32>(0.64, 0.018, 0.002);
    let orange = vec3<f32>(2.55, 0.30, 0.008);
    let hot = vec3<f32>(5.2, 1.42, 0.10);
    var color = mix(ember, orange, smoothstep(0.0, 0.62, depth));
    color = mix(color, hot, smoothstep(0.60, 0.98, depth));
    let filament = smoothstep(0.67, 0.88, flow + fine * 0.24);
    color = mix(color, hot, filament * (1.0 - depth) * 0.46);
    color = mix(color, orange, select(0.0, 0.55, radius > boundary));
    return vec4<f32>(color, alpha);
}
