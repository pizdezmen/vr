// The whole phantom: every cube lives in one mesh and one draw call.
// Assembly, dissolve and the falling hem pixels are computed on the GPU.
// Vertex data is packed into standard attributes:
//   position = cube centre (in its part's space), normal = face normal,
//   tangent  = xyz corner offset, w part index (0 body, 1 arm, 2 blade),
//   color    = xyz random vector, w brightness,
//   uv       = x cube kind, y random 0..1.
#import bevy_pbr::mesh_functions::{get_world_from_local, mesh_position_local_to_clip}
#import bevy_pbr::mesh_view_bindings::globals

struct PhantomUniform {
    part0: mat4x4<f32>,
    part1: mat4x4<f32>,
    part2: mat4x4<f32>,
    hit: vec4<f32>,   // hit point, phantom space
    push: vec4<f32>,  // sword velocity, phantom space
    fx: vec4<f32>,    // x mode (0 none, 1 assemble, 2 dissolve), y fx time, z heat, w open
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> u: PhantomUniform;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(4) tangent: vec4<f32>,
    @location(5) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

// Unity treats material colours as sRGB and converts them to linear; these are the converted values
// (sRGB originals in the comments), so both versions show the same colours.
const BODY_COLOR = vec3<f32>(0.0031, 0.0732, 1.0);    // 0.04, 0.3, 1
const OPEN_COLOR = vec3<f32>(0.5225, 0.7874, 1.0);    // 0.75, 0.9, 1
const BLADE_COLD = vec3<f32>(0.0331, 0.2633, 1.0);    // 0.2, 0.55, 1
const BLADE_HOT = vec3<f32>(1.0, 0.0399, 0.00155);    // 1, 0.22, 0.02
const CORE_COLOR = vec3<f32>(0.3185, 1.0, 1.0);       // 0.6, 1, 1

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let part = i32(v.tangent.w + 0.5);
    var pm = u.part0;
    if part == 1 {
        pm = u.part1;
    } else if part == 2 {
        pm = u.part2;
    }
    let pm3 = mat3x3<f32>(pm[0].xyz, pm[1].xyz, pm[2].xyz);
    var pos = (pm * vec4<f32>(v.position, 1.0)).xyz;
    let corner = pm3 * v.tangent.xyz;
    let nrm = pm3 * v.normal;
    let rnd = v.color.xyz;
    let shade = v.color.w;
    let kind = v.uv.x;
    let r01 = v.uv.y;

    let time = globals.time;
    let fx_mode = u.fx.x;
    let fx_time = u.fx.y;
    let heat = u.fx.z;
    let open = u.fx.w;

    var scale = 1.0;
    var bright = shade;
    var flash = 0.0;

    // Pixels falling off the hem of the robe.
    if kind > 2.5 && kind < 3.5 {
        let f = fract(time * 0.3 * (0.6 + r01) + r01 * 7.0);
        pos.y -= f * 0.4;
        pos.x += rnd.x * f * 0.05;
        pos.z += rnd.z * f * 0.05;
        scale = (1.0 - f) * 0.85;
    }

    if fx_mode > 0.5 && fx_mode < 1.5 {
        // Assemble from the bottom up.
        let t = fx_time - (pos.y / 1.8 * 0.55 + r01 * 0.1);
        let k = saturate(t / 0.45);
        let rest = (1.0 - k) * (1.0 - k);
        pos += vec3<f32>(rnd.x, -0.2 - abs(rnd.y) * 0.5, rnd.z) * rest;
        scale *= k;
        bright *= 0.6 + (1.0 - k) * 1.2;
    } else if fx_mode > 1.5 {
        // Dissolve as a wave running from the hit point.
        let d = pos - u.hit.xyz;
        let dist = length(d) + 0.001;
        let t = fx_time - (dist / 2.6 + r01 * 0.04);
        if t > 0.0 {
            let k = saturate(t / 0.75);
            let near = saturate(1.0 - dist / 0.5);
            let out_speed = 0.35 + shade * 0.6 + near * 1.6;
            let carry = (0.12 + near * 0.35) * r01;
            let vel = d / dist * out_speed + u.push.xyz * carry
                + vec3<f32>(rnd.x * 0.15, 0.25 + abs(rnd.y) * 0.5, rnd.z * 0.15);
            let drag = 1.0 - exp(-3.0 * t);
            pos += vel * drag / 3.0;
            pos.y += 0.12 * t * t;
            scale *= (1.0 - k) * (1.0 + 0.6 * saturate(1.0 - t / 0.1));
            flash = saturate(1.0 - t / 0.12);
            bright = (bright + flash * 1.2) * (1.0 - k * k);
        }
    }

    // Colour.
    var base = mix(BODY_COLOR, OPEN_COLOR, open);
    base *= 0.75 + 0.45 * saturate((pos.y - 0.3) / 1.5);   // darker towards the hem
    if r01 > 0.94 {
        base = vec3<f32>(0.25, 0.85, 1.0);                  // rare bright sparks
    }
    if kind > 0.5 && kind < 1.5 {
        base *= 0.07;                                       // the void under the hood
    }
    if kind > 1.5 && kind < 2.5 {                           // weak point
        base = CORE_COLOR * (1.4 + 0.4 * sin(time * 6.0));
        var keep = 1.0;
        if fx_mode > 1.5 {
            keep = bright;
        }
        bright = max(bright, 1.0) * keep;
    }
    if kind > 3.5 {
        base = mix(BLADE_COLD, BLADE_HOT, heat) * 1.3;      // blade
    }

    let light = 0.6 + 0.6 * saturate(dot(nrm, normalize(vec3<f32>(-0.25, 0.7, 0.65))));
    let shimmer = 0.9 + 0.1 * sin(time * 3.0 + r01 * 40.0);
    out.color = base * (bright * light * shimmer) + vec3<f32>(flash);

    out.position = mesh_position_local_to_clip(
        get_world_from_local(v.instance_index),
        vec4<f32>(pos + corner * scale, 1.0),
    );
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
