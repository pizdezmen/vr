// The whole phantom: every cube lives in one mesh and one draw call.
// Assembly, dissolve and the falling hem pixels are computed on the GPU.
// Vertex data is packed into standard attributes:
//   position = cube centre (in its part's space), normal = face normal,
//   tangent  = xyz corner offset, w part index (0 body, 1 arm, 2 blade),
//   color    = xyz random vector, w brightness,
//   uv       = x cube kind (0 body, 1 hood void, 2 weak point, 3 falling pixel, 4 blade, 5 hilt),
//              y random 0..1.
#import bevy_pbr::mesh_functions::{get_world_from_local, mesh_position_local_to_clip}
#import bevy_pbr::mesh_view_bindings::{globals, view}

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
    // How lit this cube face is: 0 deep shadow .. 1 and above = glowing white-cyan.
    @location(0) level: f32,
    // Corner signs (-1..1) across the face; used to brighten the cube's edges.
    @location(1) face: vec3<f32>,
    // x: blade heat 0..1, y: white flash added on top, z: overall multiplier, w: 1 for the weak point
    @location(2) extra: vec4<f32>,
};

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
    var bright = 1.0;
    var flash = 0.0;

    // Pixels falling off the strands of the robe.
    if kind > 2.5 && kind < 3.5 {
        let f = fract(time * 0.3 * (0.6 + r01) + r01 * 7.0);
        pos.y -= f * 0.45;
        pos.x += rnd.x * f * 0.05;
        pos.z += rnd.z * f * 0.05;
        scale = (1.0 - f) * 0.85;
    }

    if fx_mode > 0.5 && fx_mode < 1.5 {
        // Assemble from the bottom up.
        let t = fx_time - (pos.y / 1.9 * 0.55 + r01 * 0.1);
        let k = saturate(t / 0.45);
        let rest = (1.0 - k) * (1.0 - k);
        pos += vec3<f32>(rnd.x, -0.2 - abs(rnd.y) * 0.5, rnd.z) * rest;
        scale *= k;
        bright = 0.7 + (1.0 - k) * 1.5;
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
            bright = (1.0 + flash * 1.2) * (1.0 - k * k);
        }
    }

    let world_from_local = get_world_from_local(v.instance_index);
    let local = pos + corner * scale;
    let world = world_from_local * vec4<f32>(local, 1.0);

    // Light: a key light from above and in front, plus a glow on faces seen at a grazing angle,
    // which is what makes the silhouette read as glowing crystal.
    let wn = normalize((world_from_local * vec4<f32>(nrm, 0.0)).xyz);
    let to_eye = normalize(view.world_position.xyz - world.xyz);
    let key = saturate(dot(nrm, normalize(vec3<f32>(-0.3, 0.75, 0.6))));
    let rim = pow(1.0 - abs(dot(wn, to_eye)), 2.0);
    // Brighter in the chest and hood, fading into the dark towards the ends of the strands.
    let height = smoothstep(0.1, 1.35, pos.y);
    let shimmer = 0.92 + 0.08 * sin(time * 3.0 + r01 * 40.0);

    // Neighbouring cubes differ a lot in brightness (like facets of a crystal catching the light),
    // on top of slow patches that move across the body.
    let facet = shade * shade;
    let blotch = 0.5 + 0.5 * sin(pos.x * 19.0 + pos.y * 13.0 + time * 0.7) * sin(pos.y * 23.0 - pos.z * 17.0);
    var level = (0.05 + 0.55 * key * key + 0.5 * rim)
        * (0.18 + 0.82 * height)
        * (0.3 + 1.1 * facet)
        * (0.65 + 0.5 * blotch)
        * shimmer * 1.75;
    if r01 > 0.955 {
        level += 0.45;                                   // rare bright sparks
    }
    level += open * 0.55;                                // open for a counter-strike: flickers bright
    var mul = bright;
    var weak = 0.0;
    var hot = 0.0;
    if kind > 0.5 && kind < 1.5 {
        level *= 0.05;                                   // the void under the hood
    } else if kind > 1.5 && kind < 2.5 {
        level = 1.0 + 0.2 * sin(time * 6.0);             // weak point
        weak = 1.0;
    } else if kind > 2.5 && kind < 3.5 {
        level = 0.55 + 0.4 * shade;                      // falling pixels glow on their own
    } else if kind > 3.5 && kind < 4.5 {
        level = 0.5 + 0.35 * key + 0.3 * rim + 0.5 * heat;   // blade
        hot = heat;
    } else if kind > 4.5 {
        level = 0.2 + 0.4 * key + 0.3 * rim;             // hilt
    }

    out.level = level;
    out.face = sign(v.tangent.xyz);
    out.extra = vec4<f32>(hot, flash, mul, weak);
    out.position = view.clip_from_world * world;
    return out;
}

// Colour ramps (linear values): deep blue shadow -> saturated blue -> cyan -> near white.
fn cold_ramp(t: f32) -> vec3<f32> {
    var c = mix(vec3<f32>(0.0, 0.004, 0.035), vec3<f32>(0.004, 0.09, 0.85), smoothstep(0.0, 0.45, t));
    c = mix(c, vec3<f32>(0.12, 0.6, 1.0), smoothstep(0.4, 0.8, t));
    c = mix(c, vec3<f32>(0.8, 0.96, 1.0), smoothstep(0.8, 1.15, t));
    return c;
}

fn hot_ramp(t: f32) -> vec3<f32> {
    var c = mix(vec3<f32>(0.08, 0.0, 0.0), vec3<f32>(1.0, 0.06, 0.0), smoothstep(0.0, 0.5, t));
    c = mix(c, vec3<f32>(1.0, 0.45, 0.05), smoothstep(0.5, 0.9, t));
    c = mix(c, vec3<f32>(1.0, 0.9, 0.6), smoothstep(0.9, 1.3, t));
    return c;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // Distance to the nearest edge of the cube face: the largest of the two in-face coordinates
    // (the third one is always 1 because it runs along the face normal).
    let a = abs(in.face);
    let edge = min(max(a.x, a.y), min(max(a.y, a.z), max(a.x, a.z)));
    let edge_glow = smoothstep(0.84, 1.0, edge);
    let t = in.level * (1.0 + 0.45 * edge_glow) + 0.03 * edge_glow;
    var color = mix(cold_ramp(t), hot_ramp(t), in.extra.x);
    color = mix(color, vec3<f32>(0.45, 1.0, 1.0) * t, in.extra.w * 0.6);
    return vec4<f32>(color * in.extra.z + vec3<f32>(in.extra.y), 1.0);
}
