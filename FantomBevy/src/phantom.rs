//! The phantom: a faceless hooded swordsman made of glowing blue cubes, no legs, floating.
//! The whole figure is one mesh and one draw call; assembly and dissolve run in the shader.
//! Local axes: +Z faces the player, +X is the player's right (the phantom's sword arm is at -X), Y up.
//! The shape is defined in the Unity version's coordinates (X mirrored) and flipped when cubes are placed.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::meshes::{Rng, face_axes};

const CELL: f32 = 0.024; // cube grid step, m
const CUBE: f32 = CELL * 0.86;
pub const ARM_LENGTH: f32 = 0.55;
pub const BLADE_LENGTH: f32 = 0.9;
pub const DISSOLVE_LIFE: f32 = 0.75;
pub const WAVE_SPEED: f32 = 2.6;

pub const SHOULDER: Vec3 = Vec3::new(-0.26, 1.42, 0.0);
/// Weak point: the glowing core on the chest.
pub const CORE: Vec3 = Vec3::new(0.0, 1.25, 0.1);

/// Sword arm pose: direction of the arm from the shoulder and direction of the blade.
#[derive(Clone, Copy)]
pub struct Pose {
    pub arm: Vec3,
    pub blade: Vec3,
}

/// Poses are written in the Unity version's numbers; X is mirrored here.
const fn pose(ax: f32, ay: f32, az: f32, bx: f32, by: f32, bz: f32) -> Pose {
    Pose { arm: Vec3::new(-ax, ay, az), blade: Vec3::new(-bx, by, bz) }
}

impl Pose {
    fn normalized(self) -> Pose {
        Pose { arm: self.arm.normalize(), blade: self.blade.normalize() }
    }
}

#[derive(Clone, Copy)]
pub struct Attack {
    pub wind: Pose,
    pub hit: Pose,
}

pub fn idle() -> Pose {
    pose(0.25, -0.75, 0.6, -0.25, 0.9, 0.35).normalized()
}
pub fn open() -> Pose {
    pose(0.95, 0.1, -0.3, 0.7, -0.5, -0.5).normalized()
}
pub fn attacks() -> [Attack; 3] {
    [
        // top
        Attack {
            wind: pose(0.1, 0.9, -0.3, 0.0, 0.6, -0.8).normalized(),
            hit: pose(-0.3, 0.5, 0.8, -0.1, 0.03, 1.0).normalized(),
        },
        // left
        Attack {
            wind: pose(0.9, 0.25, -0.2, 0.6, 0.5, -0.6).normalized(),
            hit: pose(0.35, -0.2, 0.9, 0.055, -0.23, 0.97).normalized(),
        },
        // right
        Attack {
            wind: pose(-0.75, 0.45, 0.35, -0.7, 0.6, -0.4).normalized(),
            hit: pose(-0.55, -0.2, 0.85, -0.467, -0.193, 0.847).normalized(),
        },
    ]
}

/// Simplified body for sword hit checks: segments with a radius.
pub const CAPSULE_A: [Vec3; 3] =
    [Vec3::new(0.0, 1.58, 0.0), Vec3::new(0.0, 0.95, 0.0), Vec3::new(0.0, 0.42, 0.0)];
pub const CAPSULE_B: [Vec3; 3] =
    [Vec3::new(0.0, 1.68, 0.0), Vec3::new(0.0, 1.4, 0.0), Vec3::new(0.0, 0.85, 0.0)];
pub const CAPSULE_R: [f32; 3] = [0.19, 0.28, 0.26];

/// Mix of two poses, k from 0 to 1. Returns (arm direction, blade direction, hand position).
pub fn blend(from: Pose, to: Pose, k: f32) -> (Vec3, Vec3, Vec3) {
    let mut arm = from.arm.lerp(to.arm, k);
    if arm.length_squared() < 0.01 {
        arm = Vec3::Y;
    }
    let arm = arm.normalize();
    let mut blade = from.blade.lerp(to.blade, k);
    if blade.length_squared() < 0.01 {
        blade = Vec3::Y;
    }
    let blade = blade.normalize();
    (arm, blade, SHOULDER + arm * ARM_LENGTH)
}

/// Part matrices for the shader: arm and blade.
pub fn part_matrices(arm: Vec3, blade: Vec3, hand: Vec3) -> (Mat4, Mat4) {
    (
        Mat4::from_rotation_translation(Quat::from_rotation_arc(Vec3::Z, arm), SHOULDER),
        Mat4::from_rotation_translation(Quat::from_rotation_arc(Vec3::Z, blade), hand),
    )
}

/// The dangerous (far) part of the blade for a pose, in phantom space.
pub fn blade_segment(p: Pose) -> (Vec3, Vec3) {
    let base = SHOULDER + p.arm * ARM_LENGTH;
    let tip = base + p.blade * BLADE_LENGTH;
    (base + p.blade * (BLADE_LENGTH * 0.45), tip)
}

// ---------- Shape (Unity coordinates: +X is the player's left) ----------

fn ell(x: f32, y: f32, z: f32, c: [f32; 3], r: [f32; 3]) -> bool {
    let (dx, dy, dz) = ((x - c[0]) / r[0], (y - c[1]) / r[1], (z - c[2]) / r[2]);
    dx * dx + dy * dy + dz * dz <= 1.0
}

fn hash(a: i32, b: i32) -> f32 {
    let mut h = (a.wrapping_mul(374761393).wrapping_add(b.wrapping_mul(668265263))) as u32;
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Signed "inside" test for a slab bent like a blade: a diamond cross-section around a segment.
fn spike(x: f32, y: f32, z: f32, from: [f32; 3], to: [f32; 3], r0: f32, r1: f32) -> bool {
    let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let len2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
    let k = (((x - from[0]) * d[0] + (y - from[1]) * d[1] + (z - from[2]) * d[2]) / len2).clamp(0.0, 1.0);
    let (px, py, pz) = (from[0] + d[0] * k, from[1] + d[1] * k, from[2] + d[2] * k);
    let r = lerp(r0, r1, k);
    // Diamond (not round) cross-section gives flat facets like the reference.
    (x - px).abs() + (y - py).abs() + (z - pz).abs() <= r
}

fn body_inside(ix: i32, iy: i32, iz: i32) -> bool {
    let (x, y, z) = (ix as f32 * CELL, iy as f32 * CELL, iz as f32 * CELL);
    let ax = x.abs();

    // Dark void where the face would be: a tall opening in the front of the hood.
    if ell(x, y, z, [0.0, 1.575, 0.12], [0.078, 0.125, 0.125]) {
        return false;
    }
    // Hood: a tall shell with a sharp crown leaning back, and a cowl lying on the shoulders.
    if ell(x, y, z, [0.0, 1.61, -0.01], [0.135, 0.175, 0.155]) {
        return true;
    }
    if ax / 0.085 + (y - 1.8).abs() / 0.15 + (z + 0.06).abs() / 0.12 <= 1.0 {
        return true;
    }
    if ell(x, y, z, [0.0, 1.47, -0.02], [0.19, 0.075, 0.15]) {
        return true;
    }
    // Sharp shoulders: two layered plates on each side, raised outwards and up, ending in spikes.
    let sx = if x < 0.0 { -1.0 } else { 1.0 };
    if spike(x, y, z, [sx * 0.13, 1.43, 0.0], [sx * 0.5, 1.66, -0.02], 0.15, 0.012) {
        return true;
    }
    if spike(x, y, z, [sx * 0.16, 1.37, 0.01], [sx * 0.43, 1.33, 0.0], 0.13, 0.02) {
        return true;
    }
    if spike(x, y, z, [sx * 0.2, 1.46, -0.06], [sx * 0.36, 1.74, -0.1], 0.07, 0.008) {
        return true;
    }
    // Torso with a faceted chest ridge.
    if ell(x, y, z, [0.0, 1.2, 0.0], [0.2, 0.3, 0.125]) {
        return true;
    }
    if ax / 0.16 + (y - 1.27).abs() / 0.2 + (z - 0.07).abs() / 0.1 <= 1.0 {
        return true;
    }
    // Free arm: a sleeve hanging down with a bracer fin.
    if ell(x, y, z, [-0.3, 1.13, 0.02], [0.062, 0.3, 0.068]) {
        return true;
    }
    if spike(x, y, z, [-0.33, 0.98, 0.0], [-0.43, 1.27, -0.03], 0.06, 0.008) {
        return true;
    }
    if spike(x, y, z, [-0.3, 0.86, 0.03], [-0.31, 0.7, 0.05], 0.05, 0.006) {
        return true;
    }
    // Robe: narrow at the waist, a little wider below, then breaking into hanging strands
    // that get longer towards the middle and end in a point. No legs.
    if y <= 1.06 {
        let t = ((1.06 - y) / 0.5).clamp(0.0, 1.0);
        let rx = lerp(0.165, 0.25, t);
        let rz = lerp(0.115, 0.175, t);
        let r = (x * x / (rx * rx) + z * z / (rz * rz)).sqrt();
        if r <= 1.0 {
            // Strands are two cells wide; the length is random per strand.
            let strand = hash(ix.div_euclid(2), iz.div_euclid(2));
            let centre = (x * x / (0.25 * 0.25) + z * z / (0.175 * 0.175)).sqrt().min(1.0);
            let bottom = 0.12 + centre.powf(1.4) * 0.42 + strand * 0.3;
            if y >= bottom {
                return true;
            }
        }
    }
    false
}

fn arm_inside(ix: i32, iy: i32, iz: i32) -> bool {
    let (x, y, z) = (ix as f32 * CELL, iy as f32 * CELL, iz as f32 * CELL);
    if ell(x, y, z, [0.0, 0.0, ARM_LENGTH / 2.0], [0.06, 0.06, ARM_LENGTH / 2.0 + 0.03]) {
        return true;
    }
    // Bracer fin on the forearm.
    spike(x, y, z, [0.0, 0.03, 0.3], [0.0, 0.13, 0.52], 0.055, 0.008)
}

#[derive(Default)]
struct VoxelData {
    centers: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    corner_part: Vec<[f32; 4]>,
    rnd_shade: Vec<[f32; 4]>,
    kind_r01: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

impl VoxelData {
    /// `center` is in Unity coordinates; X is mirrored here.
    fn cube(&mut self, rng: &mut Rng, center: Vec3, part: u32, kind: u32, size: f32) {
        let center = Vec3::new(-center.x, center.y, center.z);
        let rnd = [
            rng.next() * 2.0 - 1.0,
            rng.next() * 2.0 - 1.0,
            rng.next() * 2.0 - 1.0,
            rng.next(),
        ];
        let info = [kind as f32, rng.next()];
        for f in 0..6 {
            let (n, u, v) = face_axes(f);
            let start = self.centers.len() as u32;
            for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let corner = (n + u * su + v * sv) * (size * 0.5);
                self.centers.push(center.to_array());
                self.normals.push(n.to_array());
                self.corner_part.push([corner.x, corner.y, corner.z, part as f32]);
                self.rnd_shade.push(rnd);
                self.kind_r01.push(info);
            }
            self.indices
                .extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
        }
    }
}

/// Keeps only the "skin" of the figure: cubes on the surface.
fn voxelize(
    data: &mut VoxelData,
    rng: &mut Rng,
    inside: fn(i32, i32, i32) -> bool,
    part: u32,
    min: [i32; 3],
    max: [i32; 3],
) {
    for ix in min[0]..=max[0] {
        for iy in min[1]..=max[1] {
            for iz in min[2]..=max[2] {
                if !inside(ix, iy, iz) {
                    continue;
                }
                let surface = !inside(ix + 1, iy, iz)
                    || !inside(ix - 1, iy, iz)
                    || !inside(ix, iy + 1, iz)
                    || !inside(ix, iy - 1, iz)
                    || !inside(ix, iy, iz + 1)
                    || !inside(ix, iy, iz - 1);
                if !surface {
                    continue;
                }
                let (x, y, z) = (ix as f32 * CELL, iy as f32 * CELL, iz as f32 * CELL);
                let mut kind = 0;
                if part == 0 {
                    // Walls of the void under the hood: almost black.
                    let near_cavity = ell(x, y, z, [0.0, 1.575, 0.12], [0.12, 0.165, 0.165]);
                    let inner_hood = ell(x, y, z, [0.0, 1.61, -0.01], [0.105, 0.145, 0.125]);
                    if near_cavity && inner_hood {
                        kind = 1;
                    } else if z > 0.1 && x.abs() + (y - CORE.y).abs() <= 0.06 {
                        kind = 2; // weak point: a diamond on the chest
                    }
                }
                data.cube(rng, Vec3::new(x, y, z), part, kind, CUBE);
            }
        }
    }
}

/// Returns the mesh and the number of cubes in it.
pub fn build_mesh() -> (Mesh, usize) {
    let mut data = VoxelData::default();
    let mut rng = Rng(0x9E3779B9);
    let cells = |m: f32| (m / CELL).round() as i32;
    voxelize(
        &mut data,
        &mut rng,
        body_inside,
        0,
        [cells(-0.56), cells(0.08), cells(-0.3)],
        [cells(0.56), cells(2.0), cells(0.3)],
    );

    // Pixels falling off the hem.
    for _ in 0..170 {
        let angle = rng.next() * std::f32::consts::TAU;
        let radius = rng.next().sqrt();
        let p = Vec3::new(
            angle.cos() * 0.25 * radius,
            0.2 + rng.next() * 0.35,
            angle.sin() * 0.17 * radius,
        );
        data.cube(&mut rng, p, 0, 3, CUBE);
    }

    voxelize(
        &mut data,
        &mut rng,
        arm_inside,
        1,
        [cells(-0.1), cells(-0.1), cells(-0.06)],
        [cells(0.1), cells(0.16), cells(0.62)],
    );

    // Sword: a wide blade tapering to a point, a cross-guard and a pommel. Blade cubes are smaller.
    let step = CELL * 0.8;
    let size = step * 0.9;
    let mut z = 0.07;
    while z <= BLADE_LENGTH {
        let along = (z - 0.07) / (BLADE_LENGTH - 0.07);
        let half_width = 0.034 * (1.0 - along * along) + 0.004;
        let mut x = -half_width;
        while x <= half_width + 1e-4 {
            data.cube(&mut rng, Vec3::new(x, 0.0, z), 2, 4, size);
            x += step;
        }
        z += step;
    }
    let mut x = -0.13;
    while x <= 0.13 + 1e-4 {
        data.cube(&mut rng, Vec3::new(x, 0.0, 0.06), 2, 5, size);
        data.cube(&mut rng, Vec3::new(x, step, 0.06), 2, 5, size);
        x += step;
    }
    let mut z = -0.1;
    while z < 0.05 {
        data.cube(&mut rng, Vec3::new(0.0, 0.0, z), 2, 5, size);
        z += step;
    }

    let cubes = data.centers.len() / 24;
    let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, data.centers)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, data.kind_r01)
        .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, data.corner_part)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, data.rnd_shade)
        .with_inserted_indices(Indices::U32(data.indices));
    (mesh, cubes)
}
