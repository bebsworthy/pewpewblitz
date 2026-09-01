#import bevy_pbr::forward_io::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> params: vec4<f32>;

const TAU: f32 = 6.28318530718;

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

fn ring_mask(radius: f32, center: f32, width: f32) -> f32 {
    return 1.0 - smoothstep(width * 0.45, width, abs(radius - center));
}

fn poison(point: vec2<f32>, radius: f32, time: f32) -> vec4<f32> {
    let warped = point * 3.2
        + vec2<f32>(
            value_noise(point * 2.1 + vec2<f32>(time * 0.13, -time * 0.09)),
            value_noise(point * 2.4 + vec2<f32>(-time * 0.08, time * 0.12))
        ) * 1.35;
    let broad = value_noise(warped + vec2<f32>(time * 0.18, -time * 0.14));
    let cells = value_noise(warped * 2.35 + vec2<f32>(-time * 0.32, time * 0.21));
    let veins = smoothstep(0.56, 0.78, broad * 0.68 + cells * 0.42);

    let angle = atan2(point.y, point.x);
    let bubble_wave = sin(angle * 9.0 + time * 1.7 + broad * 4.0)
        * sin(radius * 38.0 - time * 2.2 + cells * 5.0);
    let bubbles = smoothstep(0.68, 0.93, bubble_wave * 0.5 + 0.5);
    let edge = smoothstep(0.63, 0.88, radius + (broad - 0.5) * 0.16)
        * (1.0 - smoothstep(0.94, 1.0, radius));
    let pool = (1.0 - smoothstep(0.88, 1.0, radius)) * (0.055 + veins * 0.10);
    let activity = max(pool, edge * (0.22 + bubbles * 0.32));
    let alpha = activity * (1.0 - smoothstep(0.985, 1.0, radius));
    if alpha < 0.01 {
        discard;
    }

    let murk = vec3<f32>(0.035, 0.20, 0.012);
    let acid = vec3<f32>(0.30, 1.55, 0.025);
    let foam = vec3<f32>(1.05, 3.25, 0.11);
    var color = mix(murk, acid, veins * 0.72 + edge * 0.28);
    color = mix(color, foam, bubbles * edge * 0.58);
    return vec4<f32>(color, alpha);
}

fn restoration(point: vec2<f32>, radius: f32, time: f32) -> vec4<f32> {
    let angle = atan2(point.y, point.x);
    let slow = time * 0.38;
    let breath = sin(time * 1.25) * 0.5 + 0.5;

    let outer_dash = smoothstep(0.18, 0.52, sin(angle * 8.0 - slow * 2.0) * 0.5 + 0.5);
    let middle_dash = smoothstep(0.20, 0.58, sin(angle * 6.0 + slow * 1.35) * 0.5 + 0.5);
    let inner_dash = smoothstep(0.28, 0.68, sin(angle * 4.0 - slow) * 0.5 + 0.5);
    let arcs = ring_mask(radius, 0.79, 0.055) * outer_dash
        + ring_mask(radius, 0.58, 0.045) * middle_dash
        + ring_mask(radius, 0.36, 0.038) * inner_dash;

    // Four restrained radial strokes form a vitality-cross motif without turning
    // the affected area into a flat icon or hiding fighters standing over it.
    let cross_x = (1.0 - smoothstep(0.018, 0.055, abs(point.x)))
        * smoothstep(0.10, 0.22, abs(point.y))
        * (1.0 - smoothstep(0.31, 0.43, abs(point.y)));
    let cross_y = (1.0 - smoothstep(0.018, 0.055, abs(point.y)))
        * smoothstep(0.10, 0.22, abs(point.x))
        * (1.0 - smoothstep(0.31, 0.43, abs(point.x)));
    let cross = max(cross_x, cross_y) * (0.72 + breath * 0.28);
    let base = (1.0 - smoothstep(0.91, 1.0, radius)) * 0.045;
    let alpha = (base + min(arcs, 1.0) * 0.30 + cross * 0.34)
        * (1.0 - smoothstep(0.985, 1.0, radius));
    if alpha < 0.01 {
        discard;
    }

    let calm = vec3<f32>(0.01, 0.22, 0.12);
    let life = vec3<f32>(0.06, 1.75, 0.72);
    let bright = vec3<f32>(0.48, 3.20, 1.62);
    var color = mix(calm, life, clamp(arcs * 0.8 + cross * 0.55, 0.0, 1.0));
    color = mix(color, bright, clamp(cross * 0.55 + arcs * breath * 0.18, 0.0, 0.7));
    return vec4<f32>(color, alpha);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let point = in.uv * 2.0 - vec2<f32>(1.0, 1.0);
    let radius = length(point);
    if radius > 1.0 {
        discard;
    }
    if params.y < 0.5 {
        return poison(point, radius, params.x);
    }
    return restoration(point, radius, params.x);
}
