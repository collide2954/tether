struct Globals {
    view_proj: mat4x4<f32>,
    inverse: mat4x4<f32>,
    eye: vec4<f32>,
    ground: vec4<f32>,
    lattice: vec4<f32>,
    viewport: vec4<f32>,
    warps: array<vec4<f32>, 8>,
}

@group(0) @binding(0) var<uniform> globals: Globals;

const CELL: f32 = 0.6;
const JITTER: f32 = 0.38;
const LINE: f32 = 0.008;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    let ndc = uv * 2.0 - 1.0;
    var out: VertexOutput;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.ndc = ndc;
    return out;
}

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn node(cell: vec2<f32>) -> vec2<f32> {
    let jitter = vec2<f32>(hash21(cell), hash21(cell + vec2<f32>(19.19, 7.13))) - 0.5;
    return (cell + 0.5 + jitter * JITTER) * CELL;
}

fn segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

fn lattice(q: vec2<f32>) -> f32 {
    let base = floor(q / CELL);
    var d = 1e9;
    for (var j = -1; j <= 1; j = j + 1) {
        for (var i = -1; i <= 1; i = i + 1) {
            let c = base + vec2<f32>(f32(i), f32(j));
            let a = node(c);
            d = min(d, segment(q, a, node(c + vec2<f32>(1.0, 0.0))));
            d = min(d, segment(q, a, node(c + vec2<f32>(0.0, 1.0))));
            if (hash21(c + vec2<f32>(3.7, 1.3)) > 0.62) {
                d = min(d, segment(q, a, node(c + vec2<f32>(1.0, 1.0))));
            }
        }
    }
    return d;
}

fn warp(p: vec2<f32>) -> vec2<f32> {
    var q = p;
    let count = u32(globals.viewport.z);
    for (var k = 0u; k < 8u; k = k + 1u) {
        if (k < count) {
            let w = globals.warps[k];
            let delta = w.xy - p;
            let fall = exp(-dot(delta, delta) / max(w.w * w.w, 1e-4));
            q = q + delta * w.z * fall;
        }
    }
    return q;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let near = globals.inverse * vec4<f32>(in.ndc, 0.0, 1.0);
    let far = globals.inverse * vec4<f32>(in.ndc, 1.0, 1.0);
    let a = near.xyz / near.w;
    let dir = normalize(far.xyz / far.w - a);
    let hits = dir.z < -1e-4;
    let t = select(0.0, -a.z / dir.z, hits);
    let p = a.xy + dir.xy * t;
    let q = warp(p);
    let aa = length(fwidth(q));
    let line = 1.0 - smoothstep(LINE, LINE + aa, lattice(q));
    let fade = 1.0 - smoothstep(0.02, 0.12, aa / CELL);
    let mist = smoothstep(18.0, 60.0, t);
    let lit = mix(globals.ground.rgb, globals.lattice.rgb, line * fade * (1.0 - mist));
    let color = select(globals.ground.rgb * 0.7, lit * (1.0 - 0.35 * mist), hits);
    return vec4<f32>(color, 1.0);
}
