//! Pixel cloud for the phantom's appearance and death.
//!
//! The phantom itself is the animated Quaternius mannequin (see `mannequin.rs`). When it appears
//! or is hit, a cloud of small glowing cubes is built from the mannequin's current pose and the
//! shader in `shaders/phantom.wgsl` assembles or scatters it (from the point of impact).
//!
//! Vertex data is packed into standard attributes:
//!   position = cube centre, normal = face normal,
//!   tangent  = xyz corner offset, w part index (always 0 here),
//!   color    = xyz random vector, w brightness,
//!   uv       = x cube kind (0 body, 2 weak point), y random 0..1.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::meshes::{Rng, face_axes};

pub const DISSOLVE_LIFE: f32 = 0.75;
pub const WAVE_SPEED: f32 = 2.6;
pub const ASSEMBLE_TIME: f32 = 1.05;

pub const KIND_BODY: u32 = 0;
pub const KIND_CORE: u32 = 2;

#[derive(Default)]
pub struct CloudBuilder {
    centers: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    corner_part: Vec<[f32; 4]>,
    rnd_shade: Vec<[f32; 4]>,
    kind_r01: Vec<[f32; 2]>,
    indices: Vec<u32>,
    pub cubes: usize,
}

impl CloudBuilder {
    pub fn cube(&mut self, rng: &mut Rng, center: Vec3, kind: u32, size: f32) {
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
                self.corner_part.push([corner.x, corner.y, corner.z, 0.0]);
                self.rnd_shade.push(rnd);
                self.kind_r01.push(info);
            }
            self.indices
                .extend_from_slice(&[start, start + 1, start + 2, start, start + 2, start + 3]);
        }
        self.cubes += 1;
    }

    /// Fills a capsule (segment `a`-`b`, radius `r`) with cubes on and just under its surface.
    pub fn capsule(&mut self, rng: &mut Rng, a: Vec3, b: Vec3, r: f32, size: f32) {
        let axis = b - a;
        let len = axis.length().max(1e-4);
        let dir = axis / len;
        let side = if dir.y.abs() < 0.9 { dir.cross(Vec3::Y).normalize() } else { dir.cross(Vec3::X).normalize() };
        let up = dir.cross(side);
        let step = size * 1.15;
        let rings = (len / step).ceil().max(1.0) as usize;
        let around = ((std::f32::consts::TAU * r) / step).ceil().max(3.0) as usize;
        for i in 0..=rings {
            let p = a + axis * (i as f32 / rings as f32);
            for j in 0..around {
                let ang = (j as f32 + rng.next() * 0.6) / around as f32 * std::f32::consts::TAU;
                let rr = r * (0.75 + 0.25 * rng.next());
                self.cube(rng, p + (side * ang.cos() + up * ang.sin()) * rr, KIND_BODY, size);
            }
        }
    }

    /// Fills a sphere with cubes on its surface.
    pub fn ball(&mut self, rng: &mut Rng, c: Vec3, r: f32, size: f32) {
        let n = ((4.0 * std::f32::consts::PI * r * r) / (size * size * 1.3)).ceil() as usize;
        for i in 0..n {
            // Even spread over the sphere (golden spiral).
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
            let rad = (1.0 - y * y).sqrt();
            let ang = i as f32 * 2.399_963;
            let rr = r * (0.8 + 0.2 * rng.next());
            self.cube(rng, c + Vec3::new(ang.cos() * rad, y, ang.sin() * rad) * rr, KIND_BODY, size);
        }
    }

    pub fn build(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.centers)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.kind_r01)
            .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, self.corner_part)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.rnd_shade)
            .with_inserted_indices(Indices::U32(self.indices))
    }
}
