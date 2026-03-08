#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var<uniform> time: f32;

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn star_layer(uv: vec2<f32>, scale: f32, brightness: f32, speed: f32) -> f32 {
    let grid = uv * scale;
    let cell = floor(grid);
    let local = fract(grid) - 0.5;

    var stars = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let offset = vec2<f32>(f32(x), f32(y));
            let neighbor = cell + offset;
            let rnd = hash21(neighbor);
            let pos = vec2<f32>(hash21(neighbor + 100.0) - 0.5, hash21(neighbor + 200.0) - 0.5);
            let d = length(local - offset - pos * 0.7);
            let twinkle = sin(time * speed * (rnd * 3.0 + 1.0) + rnd * 6.28) * 0.5 + 0.5;
            let size = rnd * 0.03 + 0.008;
            stars += smoothstep(size, 0.0, d) * brightness * (0.5 + 0.5 * twinkle);
        }
    }
    return stars;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var uv = in.uv;
    uv.x *= 16.0 / 9.0;

    let drift = vec2<f32>(time * 0.008, time * 0.003);

    var stars = 0.0;
    stars += star_layer(uv + drift, 12.0, 0.6, 0.8);
    stars += star_layer(uv + drift * 0.5, 24.0, 0.35, 1.2);
    stars += star_layer(uv + drift * 0.2, 48.0, 0.15, 1.6);

    let bg_grad = mix(
        vec3<f32>(0.01, 0.01, 0.04),
        vec3<f32>(0.04, 0.02, 0.06),
        uv.y,
    );

    var color = bg_grad + vec3<f32>(0.7, 0.8, 1.0) * stars;
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(0.6));

    return vec4<f32>(color, 1.0);
}
