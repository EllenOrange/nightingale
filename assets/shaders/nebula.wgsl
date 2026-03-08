#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var<uniform> time: f32;

fn hash(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y,
    );
}

fn fbm(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var value = 0.0;
    var amp = 0.5;
    for (var i = 0; i < 6; i++) {
        value += amp * noise(p);
        p *= 2.2;
        amp *= 0.45;
    }
    return value;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = time * 0.06;
    var uv = in.uv;
    uv.x *= 16.0 / 9.0;

    let n1 = fbm(uv * 2.5 + vec2<f32>(t, t * 0.4));
    let n2 = fbm(uv * 3.0 + vec2<f32>(-t * 0.3, t * 0.7) + n1 * 0.5);
    let n3 = fbm(uv * 1.8 - vec2<f32>(t * 0.2, -t * 0.5));

    let purple = vec3<f32>(0.12, 0.03, 0.18) * n1 * 1.8;
    let teal = vec3<f32>(0.02, 0.08, 0.12) * n2 * 1.5;
    let dust = vec3<f32>(0.08, 0.04, 0.02) * n3 * 0.8;

    var color = purple + teal + dust;

    let glow = smoothstep(0.45, 0.75, n1 * n2) * 0.3;
    color += vec3<f32>(0.08, 0.04, 0.15) * glow;

    let vignette = 1.0 - length((in.uv - 0.5) * 1.6);
    color *= smoothstep(0.0, 0.6, vignette);

    color = clamp(color, vec3<f32>(0.0), vec3<f32>(0.45));

    return vec4<f32>(color, 1.0);
}
