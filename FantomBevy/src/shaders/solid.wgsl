// Opaque details with simple faceted shading: the player's sword.
#import bevy_pbr::forward_io::VertexOutput

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
#ifdef VERTEX_COLORS
    let c = in.color.rgb;
#else
    let c = vec3<f32>(1.0);
#endif
    let n = normalize(in.world_normal);
    let light = 0.55 + 0.45 * saturate(dot(n, normalize(vec3<f32>(0.3, 0.8, 0.4))));
    return vec4<f32>(c * light, 1.0);
}
