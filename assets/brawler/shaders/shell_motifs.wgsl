#import bevy_ui::ui_vertex_output::UiVertexOutput

// Sparse toy motifs over the shell background (BRL-51zd). Decoration only.
// x: seconds, y: drift amount (0 freezes every motif in place), z: layout (0 Dashboard, 1 edges).
@group(1) @binding(0) var<uniform> motion: vec4<f32>;
// Motif color.
@group(1) @binding(1) var<uniform> tint: vec4<f32>;

// Low contrast on the shell-bg gradient: never competes with type or controls.
const OPACITY: f32 = 0.085;
const COUNT: i32 = 8;

// Each motif: uv x, uv y, kind (0 ring, 1 plus, 2 diamond, 3 sparkle), radius in px at 720 tall.
// Dashboard: clear of the logo, status bar, hero, and the right-hand action column.
const DASHBOARD = array<vec4<f32>, 8>(
    vec4<f32>(0.335, 0.215, 0.0, 15.0),
    vec4<f32>(0.565, 0.235, 1.0, 13.0),
    vec4<f32>(0.585, 0.545, 2.0, 12.0),
    vec4<f32>(0.050, 0.435, 3.0, 16.0),
    vec4<f32>(0.245, 0.610, 0.0, 10.0),
    vec4<f32>(0.505, 0.935, 1.0, 11.0),
    vec4<f32>(0.800, 0.170, 2.0, 10.0),
    vec4<f32>(0.300, 0.890, 3.0, 12.0),
);
// Content-dense screens: outer margins only.
const EDGES = array<vec4<f32>, 8>(
    vec4<f32>(0.030, 0.200, 0.0, 13.0),
    vec4<f32>(0.022, 0.540, 1.0, 11.0),
    vec4<f32>(0.040, 0.880, 3.0, 14.0),
    vec4<f32>(0.970, 0.120, 2.0, 11.0),
    vec4<f32>(0.976, 0.450, 3.0, 13.0),
    vec4<f32>(0.964, 0.800, 0.0, 12.0),
    vec4<f32>(0.500, 0.975, 2.0, 8.0),
    vec4<f32>(0.500, 0.025, 1.0, 8.0),
);

fn rotate(p: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec2<f32>(c * p.x - s * p.y, s * p.x + c * p.y);
}

// Signed distance in pixels, negative inside.
fn motif(kind: i32, p: vec2<f32>, r: f32) -> f32 {
    let q = abs(p);
    if kind == 0 {
        return abs(length(p) - r * 0.78) - r * 0.22;
    }
    if kind == 1 {
        let arm = r * 0.3;
        return min(max(q.x - r, q.y - arm), max(q.y - r, q.x - arm));
    }
    if kind == 2 {
        return (q.x + q.y - r) * 0.7071;
    }
    // Four-point sparkle: two slim diamonds.
    let w = r * 0.32;
    let k = w * r / sqrt(w * w + r * r);
    return min((q.x / w + q.y / r - 1.0) * k, (q.y / w + q.x / r - 1.0) * k);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let size = max(in.size, vec2<f32>(1.0));
    let p = in.uv * size;
    let unit = size.y / 720.0;
    let t = motion.x;
    let drift = motion.y;
    // The same slow offset as the Dashboard tonal planes, plus a gentle per-motif bob.
    let plane = sin(t * 0.18) * 0.004;
    var dashboard = DASHBOARD;
    var edges = EDGES;
    var coverage = 0.0;
    for (var i = 0; i < COUNT; i++) {
        var m = dashboard[i];
        if motion.z > 0.5 {
            m = edges[i];
        }
        let phase = f32(i) * 1.7;
        let bob = sin(t * 0.5 + phase) * 0.006;
        let center = vec2<f32>(m.x, m.y + (plane + bob) * drift) * size;
        let angle = f32(i) * 0.4 + 0.25 * sin(t * 0.3 + phase) * drift;
        let d = motif(i32(m.z), rotate(p - center, angle), m.w * unit);
        coverage = max(coverage, 1.0 - smoothstep(-0.75, 0.75, d));
    }
    return vec4<f32>(tint.rgb, tint.a * OPACITY * coverage);
}
