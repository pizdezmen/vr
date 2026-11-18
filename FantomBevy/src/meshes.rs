//! Builds meshes out of boxes and flat shapes. Used once at start.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

const FACE_NORMALS: [Vec3; 6] = [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z];

/// Two vectors along a face, chosen so that corners (n-u-v, n+u-v, n+u+v, n-u+v) go
/// counter-clockwise when seen from outside.
pub fn face_axes(face: usize) -> (Vec3, Vec3, Vec3) {
    let n = FACE_NORMALS[face];
    let u = if n.y.abs() > 0.5 { Vec3::X } else { Vec3::Y };
    let v = n.cross(u);
    (n, u, v)
}

#[derive(Default)]
pub struct MeshBuilder {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl MeshBuilder {
    pub fn boxed(&mut self, center: Vec3, size: Vec3, color: [f32; 4]) {
        for f in 0..6 {
            let (n, u, v) = face_axes(f);
            let start = self.positions.len() as u32;
            for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let p = center + (n + u * su + v * sv) * 0.5 * size;
                self.positions.push(p.to_array());
                self.normals.push(n.to_array());
                self.colors.push(color);
            }
            self.quad(start);
        }
    }

    /// Turns four already added vertices (counter-clockwise) into two triangles.
    pub fn quad(&mut self, start: u32) {
        self.indices
            .extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
    }

    pub fn vertex(&mut self, p: Vec3, n: Vec3, c: [f32; 4]) -> u32 {
        self.positions.push(p.to_array());
        self.normals.push(n.to_array());
        self.colors.push(c);
        self.positions.len() as u32 - 1
    }

    pub fn build(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
            .with_inserted_indices(Indices::U32(self.indices))
    }
}

/// Small deterministic random generator (no extra dependency needed).
pub struct Rng(pub u32);

impl Rng {
    pub fn next(&mut self) -> f32 {
        // xorshift32
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / 16_777_216.0
    }

    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}
