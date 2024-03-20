use crate::worker::AppComputeWorker;
use bevy::{
    app::App,
    reflect::TypePath,
    render::render_resource::{BindGroupLayout, ShaderDefVal, ShaderRef},
};
use std::fmt::Debug;
use wgpu::PushConstantRange;

/// Trait to declare [`AppComputeWorker<W>`] structs.
pub trait ComputeWorker: Sized + Send + Sync + 'static {
    type Fields: Debug + Copy;
    fn build(app: &mut App) -> AppComputeWorker<Self>;
}

/// Trait to declare your shaders.
pub trait ComputeShader: TypePath + Send + Sync + 'static {
    /// Implement your [`ShaderRef`]
    ///
    /// Usually, it comes from a path:
    /// ```
    /// fn shader() -> ShaderRef {
    ///     "shaders/my_shader.wgsl".into()
    /// }
    /// ```
    fn shader() -> ShaderRef;

    /// If your shader has dependencies, declare them here.
    /// The dependencies must be written in WGSL.
    fn dependencies() -> Vec<ShaderRef> {
        vec![]
    }

    /// If you don't want to use wgpu's reflection for
    /// your binding layout, you can declare them here.
    fn layouts<'a>() -> &'a [BindGroupLayout] {
        &[]
    }

    fn shader_defs<'a>() -> &'a [ShaderDefVal] {
        &[]
    }
    fn push_constant_ranges<'a>() -> &'a [PushConstantRange] {
        &[]
    }

    /// By default, the shader entry point is `main`.
    /// You can change it from here.
    fn entry_point<'a>() -> &'a str {
        "main"
    }
}
