// Floor of the training dimension: a glowing grid that fades with distance, with a travelling pulse.
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

// Linear values of the Unity material colours (sRGB originals in the comments).
const GRID_COLOR = vec3<f32>(0.00155, 0.0243, 1.0);              // 0.02, 0.17, 1
const BASE_COLOR = vec3<f32>(0.0000774, 0.000232, 0.000929);     // 0.001, 0.003, 0.012

fn grid_line(p: vec2<f32>, width: f32) -> f32 {
    let g = abs(fract(p - 0.5) - 0.5) / max(fwidth(p), vec2<f32>(1e-5));
    return 1.0 - min(min(g.x, g.y) / width, 1.0);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let xz = in.world_position.xz;
    let dist = length(xz);
    let fine = grid_line(xz * 2.0, 1.2) * smoothstep(9.0, 2.0, dist);
    let coarse = grid_line(xz * 0.5, 1.6);
    let fade = smoothstep(45.0, 3.0, dist);
    let pulse = 0.5 + 0.5 * sin(dist * 1.2 - globals.time * 1.5);
    let color = GRID_COLOR * (fine * 0.45 + coarse * (0.6 + 0.4 * pulse));
    return vec4<f32>(BASE_COLOR + color * fade, 1.0);
}
