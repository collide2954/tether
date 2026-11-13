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

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) local: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) shape: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) shape: vec4<f32>,
}

@vertex
fn vs_main(v: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = globals.view_proj * vec4<f32>(v.position, 1.0);
    out.local = v.local;
    out.color = v.color;
    out.shape = v.shape;
    return out;
}

fn sd_box(p: vec2<f32>, half: vec2<f32>) -> f32 {
    let q = abs(p) - half;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0);
}

fn sd_triangle(p_in: vec2<f32>, radius: f32) -> f32 {
    let k = 1.7320508;
    let r = radius * 0.8660254;
    var p = p_in;
    p.x = abs(p.x) - r;
    p.y = p.y + r / k;
    if (p.x + k * p.y > 0.0) {
        p = vec2<f32>(p.x - k * p.y, -k * p.x - p.y) / 2.0;
    }
    p.x = p.x - clamp(p.x, -2.0 * r, 0.0);
    return -length(p) * sign(p.y);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let kind = u32(in.shape.x + 0.5);
    var d = 1.0;
    var glow = -1.0;
    switch kind {
        case 0u: {
            d = abs(in.local.y) - in.shape.y;
        }
        case 1u: {
            d = length(in.local) - in.shape.y;
        }
        case 2u: {
            d = abs(length(in.local) - in.shape.y) - 0.5 * in.shape.z;
        }
        case 3u: {
            d = sd_box(in.local, vec2<f32>(in.shape.y));
        }
        case 4u: {
            d = sd_triangle(in.local, in.shape.y);
        }
        case 5u: {
            let t = length(in.local) / in.shape.y;
            glow = exp(-4.0 * t * t) * (1.0 - smoothstep(0.75, 1.0, t));
        }
        case 6u: {
            d = -1.0;
        }
        case 7u: {
            d = sd_box(in.local, in.shape.yz);
        }
        default: {
        }
    }
    let width = max(max(fwidth(d), in.shape.w), 1e-6);
    let coverage = clamp(0.5 - d / width, 0.0, 1.0);
    if (glow >= 0.0) {
        return in.color * glow;
    }
    return in.color * coverage;
}
