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
        mix(hash(i + vec2<f32>(0.0, 0.0)), hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y,
    );
}

fn fbm(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var value = 0.0;
    var amplitude = 0.5;
    for (var i = 0; i < 5; i++) {
        value += amplitude * noise(p);
        p *= 2.0;
        amplitude *= 0.5;
    }
    return value;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = time * 0.15;
    var uv = in.uv;
    uv.x *= 16.0 / 9.0;

    let n1 = fbm(uv * 3.0 + vec2<f32>(t, t * 0.7));
    let n2 = fbm(uv * 2.0 - vec2<f32>(t * 0.5, t * 1.1));
    let n3 = fbm(uv * 4.0 + vec2<f32>(t * 0.3, -t * 0.6));

    let wave = sin(uv.y * 6.0 + n1 * 4.0 + t) * 0.5 + 0.5;
    let wave2 = sin(uv.y * 8.0 + n2 * 3.0 - t * 1.3) * 0.5 + 0.5;

    var c1 = vec3<f32>(0.05, 0.1, 0.3) * wave;
    var c2 = vec3<f32>(0.1, 0.4, 0.3) * wave2;
    var c3 = vec3<f32>(0.3, 0.1, 0.5) * n3;

    let glow = smoothstep(0.2, 0.8, wave * n1) * 0.6;
    var color = c1 + c2 + c3 + vec3<f32>(0.05, 0.15, 0.2) * glow;

    let vignette = 1.0 - length((in.uv - 0.5) * 1.5);
    color *= smoothstep(0.0, 0.7, vignette);

    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(color, 1.0);
}
