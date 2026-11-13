struct Globals {
    clip: vec4<f32>,
    ground: vec4<f32>,
    viewport: vec2<f32>,
    pixel: f32,
    time: f32,
}

@group(0) @binding(0) var<uniform> globals: Globals;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    let clip = uv * 2.0 - 1.0;
    var out: VertexOutput;
    out.position = vec4<f32>(clip, 0.0, 1.0);
    out.world = (clip - globals.clip.zw) / globals.clip.xy;
    return out;
}

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var q = p;
    for (var i = 0; i < 4; i = i + 1) {
        sum = sum + amp * value_noise(q);
        q = q * 2.03 + vec2<f32>(17.1, 9.2);
        amp = amp * 0.5;
    }
    return sum;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let w = in.world;
    let tooth = hash21(floor(in.position.xy)) - 0.5;
    let fiber = value_noise(w * vec2<f32>(40.0, 7.0)) - 0.5;
    let cloud = fbm(w * 0.35) - 0.5;
    var color = globals.ground.rgb * (1.0 + 0.05 * cloud + 0.02 * fiber + 0.025 * tooth);
    let v = length(w / vec2<f32>(10.0, 6.0));
    color = color * (1.0 - 0.09 * smoothstep(0.55, 1.3, v));
    return vec4<f32>(color, 1.0);
}
