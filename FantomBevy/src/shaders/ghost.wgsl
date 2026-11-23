// The living phantom (skinned mannequin): dark blue body glowing at its edges, like a figure of
// light in the dark. Skinning is done by Bevy's own vertex shader; this is only the colour.
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::{globals, view}

struct GhostUniform {
    // x: open for a counter-strike (0..1, flickers), y: white flash when hit, z: brightness, w: unused
    data: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> u: GhostUniform;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let to_eye = normalize(view.world_position.xyz - in.world_position.xyz);
    // Glow where the surface turns away from the eye: the outline of the figure lights up.
    let facing = saturate(abs(dot(n, to_eye)));
    let rim = pow(1.0 - facing, 2.2);
    // A little light from above so the shape of the body still reads.
    let top = saturate(n.y * 0.5 + 0.5);
    // Slow ripple running up the body, so it looks alive rather than plastic.
    let ripple = 0.9 + 0.1 * sin(in.world_position.y * 9.0 - globals.time * 2.5);

    var color = vec3<f32>(0.002, 0.012, 0.09) * (0.45 + 0.9 * top);
    color += vec3<f32>(0.08, 0.42, 1.0) * rim * 1.6 * ripple;
    color += vec3<f32>(0.35, 0.7, 1.0) * u.data.x * (0.35 + rim);
    color = color * u.data.z + vec3<f32>(u.data.y);
    return vec4<f32>(color, 1.0);
}
