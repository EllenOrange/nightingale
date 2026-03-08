#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(2) @binding(0) var<uniform> time: f32;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let t = time * 0.4;
    var uv = in.uv * 2.0 - 1.0;
    uv.x *= 16.0 / 9.0;

    var color = vec3<f32>(0.0);

    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        let wave_y = sin(uv.x * (2.0 + fi * 0.5) + t * (0.8 + fi * 0.15) + fi * 1.2) * 0.15;
        let dist = abs(uv.y - wave_y - (fi - 2.5) * 0.2);
        let intensity = 0.012 / dist;

        let hue = fi / 6.0 + t * 0.05;
        let r = 0.5 + 0.5 * cos(6.28318 * (hue + 0.0));
        let g = 0.5 + 0.5 * cos(6.28318 * (hue + 0.333));
        let b = 0.5 + 0.5 * cos(6.28318 * (hue + 0.667));

        color += vec3<f32>(r, g, b) * intensity * 0.4;
    }

    let bg = vec3<f32>(0.02, 0.02, 0.06);
    color += bg;

    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(color, 1.0);
}
