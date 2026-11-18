//! The four materials of the game. Same split as the Unity version: phantom, floor grid, glow, solid.

use bevy::asset::uuid_handle;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError,
};
use bevy::shader::{Shader, ShaderRef};

const PHANTOM_SHADER: Handle<Shader> = uuid_handle!("6f0c2d0e-5a51-4c39-9a1e-0f3b6f1a0001");
const GRID_SHADER: Handle<Shader> = uuid_handle!("6f0c2d0e-5a51-4c39-9a1e-0f3b6f1a0002");
const GLOW_SHADER: Handle<Shader> = uuid_handle!("6f0c2d0e-5a51-4c39-9a1e-0f3b6f1a0003");
const SOLID_SHADER: Handle<Shader> = uuid_handle!("6f0c2d0e-5a51-4c39-9a1e-0f3b6f1a0004");

pub struct FantomMaterialsPlugin;

impl Plugin for FantomMaterialsPlugin {
    fn build(&self, app: &mut App) {
        let mut shaders = app.world_mut().resource_mut::<Assets<Shader>>();
        for (handle, source, name) in [
            (&PHANTOM_SHADER, include_str!("shaders/phantom.wgsl"), "fantom/phantom.wgsl"),
            (&GRID_SHADER, include_str!("shaders/grid.wgsl"), "fantom/grid.wgsl"),
            (&GLOW_SHADER, include_str!("shaders/glow.wgsl"), "fantom/glow.wgsl"),
            (&SOLID_SHADER, include_str!("shaders/solid.wgsl"), "fantom/solid.wgsl"),
        ] {
            let _ = shaders.insert(handle.id(), Shader::from_wgsl(source, name));
        }
        app.add_plugins((
            MaterialPlugin::<PhantomMaterial>::default(),
            MaterialPlugin::<GridMaterial>::default(),
            MaterialPlugin::<GlowMaterial>::default(),
            MaterialPlugin::<SolidMaterial>::default(),
        ));
    }
}

#[derive(ShaderType, Clone, Default)]
pub struct PhantomUniform {
    pub part0: Mat4,
    pub part1: Mat4,
    pub part2: Mat4,
    pub hit: Vec4,
    pub push: Vec4,
    /// x mode (0 none, 1 assemble, 2 dissolve), y fx time, z heat, w open
    pub fx: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
pub struct PhantomMaterial {
    #[uniform(0)]
    pub data: PhantomUniform,
}

impl Material for PhantomMaterial {
    fn vertex_shader() -> ShaderRef {
        PHANTOM_SHADER.into()
    }
    fn fragment_shader() -> ShaderRef {
        PHANTOM_SHADER.into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
pub struct GridMaterial {
    #[uniform(0)]
    pub unused: Vec4,
}

impl Material for GridMaterial {
    fn fragment_shader() -> ShaderRef {
        GRID_SHADER.into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct GlowMaterial {
    /// x: brightness multiplier
    #[uniform(0)]
    pub intensity: Vec4,
}

impl GlowMaterial {
    pub fn new(intensity: f32) -> Self {
        Self { intensity: Vec4::new(intensity, 0.0, 0.0, 0.0) }
    }
}

impl Material for GlowMaterial {
    fn fragment_shader() -> ShaderRef {
        GLOW_SHADER.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Default)]
pub struct SolidMaterial {
    #[uniform(0)]
    pub unused: Vec4,
}

impl Material for SolidMaterial {
    fn fragment_shader() -> ShaderRef {
        SOLID_SHADER.into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
}
