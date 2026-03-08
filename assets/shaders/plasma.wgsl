#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var<uniform> time: f32;

fn palette(t: f32, a: vec3<f32>, b: vec3<f32>, c: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    return a + b * cos(6.28318 * (c * t + d));
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = time * 0.1;
    var uv = in.uv * 2.0 - 1.0;
    uv.x *= 16.0 / 9.0;

    var color = vec3<f32>(0.0);
    var uv0 = uv;

    for (var i = 0; i < 3; i++) {
        uv = fract(uv * 1.2) - 0.5;

        var d = length(uv) * exp(-length(uv0));
        let col = palette(
            length(uv0) + f32(i) * 0.4 + t,
            vec3<f32>(0.03, 0.02, 0.06),
            vec3<f32>(0.1, 0.1, 0.18),
            vec3<f32>(1.0, 1.0, 1.0),
            vec3<f32>(0.263, 0.416, 0.557),
        );

        d = sin(d * 5.0 + t) / 5.0;
        d = abs(d);
        d = pow(0.006 / d, 1.05);

        color += col * d;
    }

    color = clamp(color * 0.45, vec3<f32>(0.0), vec3<f32>(0.5));

    return vec4<f32>(color, 1.0);
}
