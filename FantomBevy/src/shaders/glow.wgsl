// Glowing details: shield, blade halo, pillars, dust, hurt flash. Colour is added to the background.
#import bevy_pbr::forward_io::VertexOutput

struct GlowUniform {
    intensity: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> u: GlowUniform;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
#ifdef VERTEX_COLORS
    let c = in.color;
#else
    let c = vec4<f32>(1.0);
#endif
    // Alpha 0 with premultiplied blending = pure additive.
    return vec4<f32>(c.rgb * c.a * u.intensity.x, 0.0);
}
