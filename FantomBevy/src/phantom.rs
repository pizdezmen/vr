//! The phantom: a faceless hooded swordsman made of glowing blue cubes, no legs, floating.
//! The whole figure is one mesh and one draw call; assembly and dissolve run in the shader.
//! Local axes: +Z faces the player, +X is the player's right (the phantom's sword arm is at -X), Y up.
//! The shape is defined in the Unity version's coordinates (X mirrored) and flipped when cubes are placed.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::meshes::{Rng, face_axes};

const CELL: f32 = 0.036; // cube grid step, m
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

fn body_inside(ix: i32, iy: i32, iz: i32) -> bool {
    let (x, y, z) = (ix as f32 * CELL, iy as f32 * CELL, iz as f32 * CELL);
    let ax = x.abs();

    // Dark void where the face would be.
    if ell(x, y, z, [0.0, 1.585, 0.11], [0.085, 0.105, 0.115]) {
        return false;
    }
    // Hood with a sharp crown bent backwards.
    if ell(x, y, z, [0.0, 1.6, -0.01], [0.14, 0.17, 0.15]) {
        return true;
    }
    if ax / 0.07 + (y - 1.77).abs() / 0.11 + (z + 0.07).abs() / 0.1 <= 1.0 {
        return true;
    }
    // Sharp shoulders: a faceted bar and pauldrons raised outwards and up.
    if ax / 0.36 + (y - 1.4).abs() / 0.13 + z.abs() / 0.17 <= 1.0 {
        return true;
    }
    if (0.14..=0.47).contains(&ax) {
        let along = (ax - 0.14) / 0.33;
        let ridge = 1.41 + 0.2 * along;
        if (y - ridge).abs() <= 0.085 * (1.0 - along) + 0.02 && z.abs() <= 0.14 * (1.0 - along) + 0.02 {
            return true;
        }
    }
    // Torso.
    if ell(x, y, z, [0.0, 1.2, 0.0], [0.2, 0.28, 0.125]) {
        return true;
    }
    // Loose sleeve (the phantom's free arm).
    if ell(x, y, z, [-0.29, 1.12, 0.02], [0.065, 0.3, 0.07]) {
        return true;
    }
    // Robe: widens towards the bottom, ragged hem, no legs.
    if y <= 1.05 && y >= 0.36 + hash(ix, iz) * 0.3 {
        let t = (1.05 - y) / 0.7;
        let rx = lerp(0.17, 0.27, t);
        let rz = lerp(0.115, 0.19, t);
        if x * x / (rx * rx) + z * z / (rz * rz) <= 1.0 {
            return true;
        }
    }
    false
}

fn arm_inside(ix: i32, iy: i32, iz: i32) -> bool {
    let (x, y, z) = (ix as f32 * CELL, iy as f32 * CELL, iz as f32 * CELL);
    ell(x, y, z, [0.0, 0.0, ARM_LENGTH / 2.0], [0.062, 0.062, ARM_LENGTH / 2.0 + 0.03])
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
            0.55 + rng.next() * 0.45,
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
                    let near_cavity = ell(x, y, z, [0.0, 1.585, 0.11], [0.13, 0.15, 0.16]);
                    let inner_hood = ell(x, y, z, [0.0, 1.6, -0.01], [0.1, 0.13, 0.11]);
                    if near_cavity && inner_hood {
                        kind = 1;
                    } else if z > 0.06 && x.abs() + (y - CORE.y).abs() <= 0.075 {
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
    voxelize(&mut data, &mut rng, body_inside, 0, [-14, 8, -7], [14, 54, 7]);

    // Pixels falling off the hem.
    for _ in 0..90 {
        let angle = rng.next() * std::f32::consts::TAU;
        let radius = rng.next().sqrt();
        let p = Vec3::new(
            angle.cos() * 0.25 * radius,
            0.36 + rng.next() * 0.2,
            angle.sin() * 0.17 * radius,
        );
        data.cube(&mut rng, p, 0, 3, CUBE);
    }

    voxelize(&mut data, &mut rng, arm_inside, 1, [-3, -3, -1], [3, 3, 17]);

    // Blade and guard.
    let mut z = -0.06;
    while z <= BLADE_LENGTH {
        data.cube(&mut rng, Vec3::new(0.0, 0.0, z), 2, 4, CUBE);
        if z > 0.06 && z < BLADE_LENGTH - 0.05 {
            data.cube(&mut rng, Vec3::new(0.0, CELL * 0.7, z), 2, 4, CUBE);
        }
        z += CELL * 0.75;
    }
    let mut x = -0.1;
    while x <= 0.1 {
        data.cube(&mut rng, Vec3::new(x, 0.0, 0.06), 2, 4, CUBE);
        x += CELL;
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
