use std::{
    borrow::Cow,
    fs::File,
    hash::{DefaultHasher, Hash, Hasher},
    io::prelude::Read,
    marker::PhantomData,
};

use bevy::{
    asset::{Assets, Handle},
    prelude::{App, AssetServer},
    render::{
        render_resource::{
            encase::{private::WriteInto, StorageBuffer, UniformBuffer},
            Buffer, CachedComputePipelineId, ComputePipelineDescriptor, PipelineCache, Shader,
            ShaderRef, ShaderType,
        },
        renderer::RenderDevice,
        RenderApp,
    },
    utils::HashMap,
};
use std::fmt::Debug;
use wgpu::{util::BufferInitDescriptor, BufferDescriptor, BufferUsages};

use crate::{
    traits::{ComputeShader, ComputeWorker},
    worker::{AppComputeWorker, ComputePass, RunMode, StagingBuffer, Step},
};

/// A builder struct to build [`AppComputeWorker<W>`]
/// from your structs implementing [`ComputeWorker`]
pub struct AppComputeWorkerBuilder<'a, W: ComputeWorker, E: Debug + Copy> {
    pub(crate) app: &'a mut App,
    pub(crate) cached_pipeline_ids: HashMap<String, CachedComputePipelineId>,
    pub(crate) buffers: HashMap<String, Buffer>,
    pub(crate) staging_buffers: HashMap<String, StagingBuffer>,
    pub(crate) steps: Vec<Step>,
    pub(crate) run_mode: RunMode,
    pub(crate) wait_mode: bool,
    _phantom: PhantomData<(W, E)>,
}

impl<'a, W: ComputeWorker, E: Debug + Copy> AppComputeWorkerBuilder<'a, W, E> {
    /// Create a new builder.
    pub fn new(app: &'a mut App) -> Self {
        Self {
            app,
            cached_pipeline_ids: HashMap::default(),
            buffers: HashMap::default(),
            staging_buffers: HashMap::default(),
            steps: vec![],
            run_mode: RunMode::Continuous,
            wait_mode: true,
            _phantom: PhantomData,
        }
    }

    ///Set the wait mode of the worker.
    ///If `wait` is true, the worker will cause the CPU to wait for the GPU to finish before running the next frame.
    ///By default it is set to true.
    ///This is useful if you have a computationally heavy worker, and don't want to block the CPU.
    pub fn set_wait_mode(&mut self, wait: bool) -> &mut Self {
        self.wait_mode = wait;
        self
    }

    /// Add a new uniform buffer to the worker, and fill it with `uniform`.
    pub fn add_uniform<T: ShaderType + WriteInto>(&mut self, name: E, uniform: &T) -> &mut Self {
        T::assert_uniform_compat();
        let mut buffer = UniformBuffer::new(Vec::new());
        buffer.write::<T>(uniform).unwrap();

        let render_device = self.app.world.resource::<RenderDevice>();

        self.buffers.insert(
            format!("{name:?}"),
            render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some(&format!("{name:?}")),
                contents: buffer.as_ref(),
                usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
            }),
        );
        self
    }

    /// Add a new storage buffer to the worker, and fill it with `storage`. It will be read only.
    pub fn add_storage<T: ShaderType + WriteInto>(&mut self, name: E, storage: &T) -> &mut Self {
        let mut buffer = StorageBuffer::new(Vec::new());
        buffer.write::<T>(storage).unwrap();

        let render_device = self.app.world.resource::<RenderDevice>();

        self.buffers.insert(
            format!("{name:?}"),
            render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some(&format!("{name:?}")),
                contents: buffer.as_ref(),
                usage: BufferUsages::COPY_DST | BufferUsages::STORAGE,
            }),
        );
        self
    }

    /// Add a new read/write storage buffer to the worker, and fill it with `storage`.
    pub fn add_rw_storage<T: ShaderType + WriteInto>(&mut self, name: E, storage: &T) -> &mut Self {
        let mut buffer = StorageBuffer::new(Vec::new());
        buffer.write::<T>(storage).unwrap();

        let render_device = self.app.world.resource::<RenderDevice>();

        self.buffers.insert(
            format!("{name:?}"),
            render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some(&format!("{name:?}")),
                contents: buffer.as_ref(),
                usage: BufferUsages::COPY_DST | BufferUsages::COPY_SRC | BufferUsages::STORAGE,
            }),
        );
        self
    }

    /// Create two staging buffers, one to read from and one to write to.
    /// Additionally, it will create a read/write storage buffer to access from
    /// your shaders.
    /// The buffer will be filled with `data`
    pub fn add_staging<T: ShaderType + WriteInto>(&mut self, name: E, data: &T) -> &mut Self {
        self.add_rw_storage(name, data);
        let buffer = self.buffers.get(&format!("{name:?}")).unwrap();

        let render_device = self.app.world.resource::<RenderDevice>();

        let staging = StagingBuffer {
            mapped: true,
            buffer: render_device.create_buffer(&BufferDescriptor {
                label: Some(&format!("{name:?}")),
                size: buffer.size(),
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                mapped_at_creation: true,
            }),
        };

        self.staging_buffers.insert(format!("{name:?}"), staging);

        self
    }

    /// Add a new empty uniform buffer to the worker.
    pub fn add_empty_uniform(&mut self, name: E, size: u64) -> &mut Self {
        let render_device = self.app.world.resource::<RenderDevice>();

        self.buffers.insert(
            format!("{name:?}"),
            render_device.create_buffer(&BufferDescriptor {
                label: Some(&format!("{name:?}")),
                size,
                usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
                mapped_at_creation: false,
            }),
        );

        self
    }

    /// Add a new empty storage buffer to the worker. It will be read only.
    pub fn add_empty_storage(&mut self, name: E, size: u64) -> &mut Self {
        let render_device = self.app.world.resource::<RenderDevice>();

        self.buffers.insert(
            format!("{name:?}"),
            render_device.create_buffer(&BufferDescriptor {
                label: Some(&format!("{name:?}")),
                size,
                usage: BufferUsages::COPY_DST | BufferUsages::STORAGE,
                mapped_at_creation: false,
            }),
        );
        self
    }

    /// Add a new empty read/write storage buffer to the worker.
    pub fn add_empty_rw_storage(&mut self, name: E, size: u64) -> &mut Self {
        let render_device = self.app.world.resource::<RenderDevice>();

        self.buffers.insert(
            format!("{name:?}"),
            render_device.create_buffer(&BufferDescriptor {
                label: Some(&format!("{name:?}")),
                size,
                usage: BufferUsages::COPY_DST | BufferUsages::COPY_SRC | BufferUsages::STORAGE,
                mapped_at_creation: false,
            }),
        );
        self
    }

    /// Create two staging buffers, one to read from and one to write to.
    /// Additionally, it will create a read/write storage buffer to access from
    /// your shaders.
    /// The buffer will empty.
    pub fn add_empty_staging(&mut self, name: E, size: u64) -> &mut Self {
        self.add_empty_rw_storage(name, size);

        let buffer = self.buffers.get(&format!("{name:?}")).unwrap();

        let render_device = self.app.world.resource::<RenderDevice>();

        let staging = StagingBuffer {
            mapped: true,
            buffer: render_device.create_buffer(&BufferDescriptor {
                label: Some(&format!("{name:?}")),
                size: buffer.size(),
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                mapped_at_creation: true,
            }),
        };

        self.staging_buffers.insert(format!("{name:?}"), staging);

        self
    }

    /// Add a new compute pass to your worker.
    /// They will run sequentially in the order you insert them.
    pub fn add_pass<S: ComputeShader>(&mut self, dispatch_size: [u32; 3], vars: &[E]) -> &mut Self {
        if !self.cached_pipeline_ids.contains_key(S::type_path()) {
            S::dependencies()
                .into_iter()
                .for_each(|shader| match shader {
                    ShaderRef::Default | ShaderRef::Handle(_) => {}
                    ShaderRef::Path(path) => {
                        let path_string = path.path().to_str().unwrap();

                        let mut current_directory = std::env::current_dir().unwrap();
                        current_directory.push("assets");
                        current_directory.push(path_string);
                        println!(
                            "Loading shader from path: {}",
                            current_directory.to_string_lossy()
                        );

                        if current_directory.extension().unwrap() != "wgsl" {
                            panic!("Only WGSL shaders are supported for now.");
                        }

                        let mut hasher = DefaultHasher::new();
                        path_string.hash(&mut hasher);
                        //Seems sketchy to only use a u64 hash, but hash collisions are already pretty rare, and I don't want to import a whole new library for a 128 bit hash.
                        let hash_bytes = hasher.finish().to_ne_bytes();
                        let hash = u128::from_ne_bytes(
                            [hash_bytes, hash_bytes].concat().try_into().unwrap(),
                        );
                        let handle = Handle::weak_from_u128(hash);

                        let mut shader_string = String::new();
                        let _ = File::open(current_directory)
                            .unwrap()
                            .read_to_string(&mut shader_string);

                        let mut shader_assets = self.app.world.resource_mut::<Assets<Shader>>();
                        //Frankly, this isn't great. It's forces the dependency to be written in WGSL.
                        shader_assets.insert(handle, Shader::from_wgsl(shader_string, path_string));
                    }
                });

            let shader = match S::shader() {
                ShaderRef::Default => None,
                ShaderRef::Handle(handle) => Some(handle),
                ShaderRef::Path(path) => {
                    let asset_server = self.app.world.resource::<AssetServer>();
                    Some(asset_server.load(path))
                }
            }
            .unwrap();

            let pipeline_cache = self
                .app
                .sub_app_mut(RenderApp)
                .world
                .resource::<PipelineCache>();
            let cached_id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: None,
                layout: S::layouts().to_vec(),
                push_constant_ranges: S::push_constant_ranges().to_vec(),
                shader_defs: S::shader_defs().to_vec(),
                entry_point: Cow::Borrowed(S::entry_point()),
                shader,
            });

            self.cached_pipeline_ids
                .insert(S::type_path().to_string(), cached_id);
        }

        self.steps.push(Step::ComputePass(ComputePass {
            dispatch_size,
            vars: vars.iter().map(|a| format!("{a:?}")).collect(),
            shader_type_path: S::type_path().to_string(),
        }));
        self
    }

    pub fn add_swap(&mut self, buffer_a: E, buffer_b: E) -> &mut Self {
        self.steps
            .push(Step::Swap(format!("{buffer_a:?}"), format!("{buffer_b:?}")));
        self
    }

    /// The worker will run every frames.
    /// This is the default mode.
    pub fn continuous(&mut self) -> &mut Self {
        self.run_mode = RunMode::Continuous;
        self
    }

    /// The worker will run when requested.
    pub fn one_shot(&mut self) -> &mut Self {
        self.run_mode = RunMode::OneShot(false);
        self
    }

    /// The worker will run immediately and wait for the GPU to finish.
    pub fn immediate(&mut self) -> &mut Self {
        self.run_mode = RunMode::Immediate;
        self
    }

    /// Build an [`AppComputeWorker<W>`] from this builder.
    pub fn build(&self) -> AppComputeWorker<W> {
        AppComputeWorker::from(self)
    }
}
