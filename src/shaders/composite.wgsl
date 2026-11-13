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
@group(1) @binding(0) var hdr: texture_2d<f32>;
@group(1) @binding(1) var bloom: texture_2d<f32>;
@group(1) @binding(2) var linear: sampler;

const BLOOM: f32 = 0.55;
const KNEE: f32 = 0.6;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var out: VertexOutput;
    out.position = vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(corner.x, 1.0 - corner.y);
    return out;
}

fn shoulder(x: vec3<f32>) -> vec3<f32> {
    let over = max(x - vec3<f32>(KNEE), vec3<f32>(0.0));
    return min(x, vec3<f32>(KNEE)) + (1.0 - KNEE) * over / (over + vec3<f32>(1.0 - KNEE));
}

fn hash(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let base = textureSampleLevel(hdr, linear, in.uv, 0.0).rgb;
    let glow = textureSampleLevel(bloom, linear, in.uv, 0.0).rgb;
    var color = shoulder(base + glow * BLOOM);
    let edge = length((in.uv - 0.5) * vec2<f32>(1.0, 0.8));
    color = color * (1.0 - 0.38 * smoothstep(0.35, 0.85, edge));
    let grain = hash(in.position.xy + fract(globals.eye.w) * 117.0) - 0.5;
    color = color + vec3<f32>(grain * 1.5 / 255.0);
    return vec4<f32>(max(color, vec3<f32>(0.0)), 1.0);
}
