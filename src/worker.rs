use core::panic;
use std::{marker::PhantomData, ops::Deref};

use crate::{
    error::{Error, Result},
    pipeline_cache::AppPipelineCache,
    traits::{ComputeShader, ComputeWorker},
    worker_builder::AppComputeWorkerBuilder,
};
use bevy::{
    prelude::{Res, ResMut, Resource},
    render::{
        render_resource::{
            encase::{internal::WriteInto, StorageBuffer, UniformBuffer},
            Buffer, CachedComputePipelineId, ComputePipeline, ShaderType,
        },
        renderer::{RenderDevice, RenderQueue},
    },
    utils::HashMap,
};
use bytemuck::{bytes_of, cast_slice, from_bytes, AnyBitPattern, NoUninit};

use std::fmt::Debug;
use wgpu::{
    util::BufferInitDescriptor, BindGroupEntry, BufferDescriptor, BufferUsages, CommandEncoder,
    CommandEncoderDescriptor, ComputePassDescriptor,
};

#[derive(PartialEq, Clone, Copy)]
pub enum RunMode {
    Continuous,
    OneShot(bool),
    Immediate,
}

#[derive(PartialEq)]
pub enum WorkerState {
    Created,
    Available,
    Working,
    FinishedWorking,
}

#[derive(Clone, Debug)]
pub(crate) enum Step {
    ComputePass(ComputePass),
    Swap(String, String),
}

#[derive(Clone, Debug)]
pub(crate) struct ComputePass {
    pub(crate) dispatch_size: [u32; 3],
    pub(crate) vars: Vec<String>,
    pub(crate) shader_type_path: String,
}

#[derive(Clone, Debug)]
pub(crate) struct StagingBuffer {
    pub(crate) mapped: bool,
    pub(crate) buffer: Buffer,
}

/// Struct to manage data transfers from/to the GPU
/// it also handles the logic of your compute work.
/// By default, the run mode of the workers is set to continuous,
/// meaning it will run every frames. If you want to run it deterministically
/// use the function `one_shot()` in the builder
#[derive(Resource)]
pub struct AppComputeWorker<W: ComputeWorker> {
    pub(crate) state: WorkerState,
    render_device: RenderDevice,
    render_queue: RenderQueue,
    cached_pipeline_ids: HashMap<String, CachedComputePipelineId>,
    pipelines: HashMap<String, Option<ComputePipeline>>,
    buffers: HashMap<String, Buffer>,
    staging_buffers: HashMap<String, StagingBuffer>,
    steps: Vec<Step>,
    command_encoder: Option<CommandEncoder>,
    run_mode: RunMode,
    wait_mode: bool,
    _phantom: PhantomData<W>,
}

impl<W: ComputeWorker, E: Debug + Copy> From<&AppComputeWorkerBuilder<'_, W, E>>
    for AppComputeWorker<W>
{
    /// Create a new [`AppComputeWorker<W>`].
    fn from(builder: &AppComputeWorkerBuilder<W, E>) -> Self {
        let render_device = builder.app.world.resource::<RenderDevice>().clone();
        let render_queue = builder.app.world.resource::<RenderQueue>().clone();

        let pipelines = builder
            .cached_pipeline_ids
            .iter()
            .map(|(type_path, _)| (type_path.clone(), None))
            .collect();

        let command_encoder =
            Some(render_device.create_command_encoder(&CommandEncoderDescriptor { label: None }));

        Self {
            state: WorkerState::Created,
            render_device,
            render_queue,
            cached_pipeline_ids: builder.cached_pipeline_ids.clone(),
            pipelines,
            buffers: builder.buffers.clone(),
            staging_buffers: builder.staging_buffers.clone(),
            steps: builder.steps.clone(),
            command_encoder,
            run_mode: builder.run_mode,
            wait_mode: builder.wait_mode,
            _phantom: PhantomData,
        }
    }
}

impl<W: ComputeWorker> AppComputeWorker<W> {
    pub fn run_mode(&self) -> RunMode {
        self.run_mode
    }
    pub fn set_dispatch_size<S: ComputeShader>(&mut self, dispatch_size: [u32; 3]) {
        let shader_index = self
            .steps
            .iter()
            .position(|step| match step {
                Step::ComputePass(compute_pass) => {
                    compute_pass.shader_type_path == S::type_path().to_string()
                }
                Step::Swap(_, _) => false,
            })
            .expect(&format!("Shader {} not found", S::type_path()));

        match &self.steps[shader_index] {
            Step::ComputePass(compute_pass) => {
                let mut new_compute_pass = compute_pass.clone();
                new_compute_pass.dispatch_size = dispatch_size;
                self.steps[shader_index] = Step::ComputePass(new_compute_pass);
            }
            Step::Swap(_, _) => panic!("Invalid step"),
        }
    }

    /// Add a new uniform buffer to the worker, and fill it with `uniform`. Will replace the old buffer if it exists.
    pub fn add_uniform<T: ShaderType + WriteInto, E: Debug + Copy>(
        &mut self,
        render_device: &RenderDevice,
        name: E,
        uniform: &T,
    ) -> &mut Self {
        T::assert_uniform_compat();
        let mut buffer = UniformBuffer::new(Vec::new());
        buffer.write::<T>(uniform).unwrap();

        let old_buffer = self.buffers.insert(
            format!("{name:?}"),
            render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some(&format!("{name:?}")),
                contents: buffer.as_ref(),
                usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
            }),
        );
        if let Some(old_buffer) = old_buffer {
            old_buffer.destroy();
        }
        self
    }

    /// Add a new storage buffer to the worker, and fill it with `storage`. It will be read only. Will replace the old buffer if it exists.
    pub fn add_storage<T: ShaderType + WriteInto, E: Debug + Copy>(
        &mut self,
        render_device: &RenderDevice,
        name: E,
        storage: &T,
    ) -> &mut Self {
        let mut buffer = StorageBuffer::new(Vec::new());
        buffer.write::<T>(storage).unwrap();

        let old_buffer = self.buffers.insert(
            format!("{name:?}"),
            render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some(&format!("{name:?}")),
                contents: buffer.as_ref(),
                usage: BufferUsages::COPY_DST | BufferUsages::STORAGE,
            }),
        );
        if let Some(old_buffer) = old_buffer {
            old_buffer.destroy();
        }
        self
    }

    /// Add a new read/write storage buffer to the worker, and fill it with `storage`. Will replace the old buffer if it exists.
    pub fn add_rw_storage<T: ShaderType + WriteInto, E: Debug + Copy>(
        &mut self,
        render_device: &RenderDevice,
        name: E,
        storage: &T,
    ) -> &mut Self {
        let mut buffer = StorageBuffer::new(Vec::new());
        buffer.write::<T>(storage).unwrap();

        let old_buffer = self.buffers.insert(
            format!("{name:?}"),
            render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some(&format!("{name:?}")),
                contents: buffer.as_ref(),
                usage: BufferUsages::COPY_DST | BufferUsages::COPY_SRC | BufferUsages::STORAGE,
            }),
        );
        if let Some(old_buffer) = old_buffer {
            old_buffer.destroy();
        }
        self
    }

    /// Create two staging buffers, one to read from and one to write to.
    /// Additionally, it will create a read/write storage buffer to access from
    /// your shaders.
    /// The buffer will be filled with `data`
    /// Will replace the old buffer if it exists.
    pub fn add_staging<T: ShaderType + WriteInto, E: Debug + Copy>(
        &mut self,
        render_device: &RenderDevice,
        name: E,
        data: &T,
    ) -> &mut Self {
        self.add_rw_storage(render_device, name, data);
        let buffer = self.buffers.get(&format!("{name:?}")).unwrap();

        let staging = StagingBuffer {
            mapped: true,
            buffer: render_device.create_buffer(&BufferDescriptor {
                label: Some(&format!("{name:?}")),
                size: buffer.size(),
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                mapped_at_creation: true,
            }),
        };

        let old_buffer = self.staging_buffers.insert(format!("{name:?}"), staging);
        if let Some(old_buffer) = old_buffer {
            old_buffer.buffer.destroy();
        }
        self
    }

    #[inline]
    fn dispatch(&mut self, index: usize) -> Result<()> {
        let compute_pass = match &self.steps[index] {
            Step::ComputePass(compute_pass) => compute_pass,
            Step::Swap(_, _) => return Err(Error::InvalidStep(format!("{:?}", self.steps[index]))),
        };

        let mut entries = vec![];
        for (index, var) in compute_pass.vars.iter().enumerate() {
            let Some(buffer) = self.buffers.get(var) else {
                return Err(Error::BufferNotFound(var.to_owned()));
            };

            let entry = BindGroupEntry {
                binding: index as u32,
                resource: buffer.as_entire_binding(),
            };

            entries.push(entry);
        }

        let Some(maybe_pipeline) = self.pipelines.get(&compute_pass.shader_type_path) else {
            return Err(Error::PipelinesEmpty);
        };

        let Some(pipeline) = maybe_pipeline else {
            return Err(Error::PipelineNotReady);
        };

        let bind_group_layout = pipeline.get_bind_group_layout(0);
        let bind_group =
            self.render_device
                .create_bind_group(None, &bind_group_layout.into(), &entries);

        let Some(encoder) = &mut self.command_encoder else {
            return Err(Error::EncoderIsNone);
        };
        {
            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            cpass.set_pipeline(pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.dispatch_workgroups(
                compute_pass.dispatch_size[0],
                compute_pass.dispatch_size[1],
                compute_pass.dispatch_size[2],
            )
        }

        Ok(())
    }

    #[inline]
    fn swap(&mut self, index: usize) -> Result<()> {
        let (buf_a_name, buf_b_name) = match &self.steps[index] {
            Step::ComputePass(_) => {
                return Err(Error::InvalidStep(format!("{:?}", self.steps[index])))
            }
            Step::Swap(a, b) => (a.as_str(), b.as_str()),
        };

        if !self.buffers.contains_key(buf_a_name) {
            return Err(Error::BufferNotFound(buf_a_name.to_owned()));
        }

        if !self.buffers.contains_key(buf_b_name) {
            return Err(Error::BufferNotFound(buf_b_name.to_owned()));
        }

        let [buffer_a, buffer_b] = self.buffers.get_many_mut([buf_a_name, buf_b_name]).unwrap();
        std::mem::swap(buffer_a, buffer_b);

        Ok(())
    }

    #[inline]
    fn read_staging_buffers(&mut self) -> Result<&mut Self> {
        for (name, staging_buffer) in &self.staging_buffers {
            let Some(encoder) = &mut self.command_encoder else {
                return Err(Error::EncoderIsNone);
            };
            let Some(buffer) = self.buffers.get(name) else {
                return Err(Error::BufferNotFound(name.to_owned()));
            };

            encoder.copy_buffer_to_buffer(
                buffer,
                0,
                &staging_buffer.buffer,
                0,
                staging_buffer.buffer.size(),
            );
        }
        Ok(self)
    }

    #[inline]
    fn map_staging_buffers(&mut self) -> &mut Self {
        for (_, staging_buffer) in self.staging_buffers.iter_mut() {
            let read_buffer_slice = staging_buffer.buffer.slice(..);

            read_buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                let err = result.err();
                if err.is_some() {
                    let some_err = err.unwrap();
                    panic!("{}", some_err.to_string());
                }
            });

            staging_buffer.mapped = true;
        }
        self
    }

    /// Read data from `target` staging buffer, return raw bytes
    #[inline]
    pub fn try_read_raw<'a>(
        &'a self,
        target: W::Fields,
    ) -> Result<(impl Deref<Target = [u8]> + 'a)> {
        let Some(staging_buffer) = &self.staging_buffers.get(&format!("{target:?}")) else {
            return Err(Error::StagingBufferNotFound(format!("{target:?}")));
        };

        let result = staging_buffer.buffer.slice(..).get_mapped_range();

        Ok(result)
    }

    /// Read data from `target` staging buffer, return raw bytes
    /// Panics on error.
    #[inline]
    pub fn read_raw<'a>(&'a self, target: W::Fields) -> (impl Deref<Target = [u8]> + 'a) {
        self.try_read_raw(target).unwrap()
    }

    /// Try Read data from `target` staging buffer, return a single `B: Pod`
    #[inline]
    pub fn try_read<B: AnyBitPattern>(&self, target: W::Fields) -> Result<B> {
        let result = *from_bytes::<B>(&self.try_read_raw(target)?);
        Ok(result)
    }

    /// Try Read data from `target` staging buffer, return a single `B: Pod`
    /// In case of error, this function will panic.
    #[inline]
    pub fn read<B: AnyBitPattern>(&self, target: W::Fields) -> B {
        self.try_read(target).unwrap()
    }

    /// Try Read data from `target` staging buffer, return a vector of `B: Pod`
    #[inline]
    pub fn try_read_vec<B: AnyBitPattern>(&self, target: W::Fields) -> Result<Vec<B>> {
        let bytes = self.try_read_raw(target)?;
        Ok(cast_slice::<u8, B>(&bytes).to_vec())
    }

    /// Try Read data from `target` staging buffer, return a vector of `B: Pod`
    /// In case of error, this function will panic.
    #[inline]
    pub fn read_vec<B: AnyBitPattern>(&self, target: W::Fields) -> Vec<B> {
        self.try_read_vec(target).unwrap()
    }

    /// Write data to `target` buffer.
    #[inline]
    pub fn try_write<T: NoUninit>(&mut self, target: W::Fields, data: &T) -> Result<()> {
        let Some(buffer) = &self.buffers.get(&format!("{target:?}")) else {
            return Err(Error::BufferNotFound(format!("{target:?}")));
        };

        let bytes = bytes_of(data);

        self.render_queue.write_buffer(buffer, 0, bytes);

        Ok(())
    }

    /// Write data to `target` buffer.
    /// In case of error, this function will panic.
    #[inline]
    pub fn write<T: NoUninit>(&mut self, target: W::Fields, data: &T) {
        self.try_write(target, data).unwrap()
    }

    /// Write data to `target` buffer.
    #[inline]
    pub fn try_write_slice<T: NoUninit>(&mut self, target: W::Fields, data: &[T]) -> Result<()> {
        let Some(buffer) = &self.buffers.get(&format!("{target:?}")) else {
            return Err(Error::BufferNotFound(format!("{target:?}")));
        };

        let bytes = cast_slice(data);

        self.render_queue.write_buffer(buffer, 0, bytes);

        Ok(())
    }

    /// Write data to `target` buffer.
    /// In case of error, this function will panic.
    #[inline]
    pub fn write_slice<T: NoUninit>(&mut self, target: W::Fields, data: &[T]) {
        self.try_write_slice(target, data).unwrap()
    }

    fn submit(&mut self) -> &mut Self {
        let encoder = self.command_encoder.take().unwrap();
        self.render_queue.submit(Some(encoder.finish()));
        self.state = WorkerState::Working;
        self
    }

    #[inline]
    fn poll(&self) -> bool {
        let maintain = if self.wait_mode || self.run_mode == RunMode::Immediate {
            wgpu::MaintainBase::Wait
        } else {
            wgpu::MaintainBase::Poll
        };
        match self.render_device.wgpu_device().poll(maintain) {
            wgpu::MaintainResult::SubmissionQueueEmpty => true,
            wgpu::MaintainResult::Ok => false,
        }
    }

    /// Check if the worker is ready to be read from.
    #[inline]
    pub fn ready(&self) -> bool {
        self.state == WorkerState::FinishedWorking
    }

    /// Tell the worker to execute the compute shader at the end of the current frame
    #[inline]
    pub fn execute(&mut self) {
        match self.run_mode {
            RunMode::Continuous => {}
            RunMode::OneShot(_) => self.run_mode = RunMode::OneShot(true),
            RunMode::Immediate => {
                panic!("Immediate mode is not supported in execute(), please use execute_now() instead");
            }
        }
    }

    ///Execute the compute shader immediately and wait for the result. This will return false if the worker is not ready to execute, e.g the pipeline is not ready. This will only happen before the first time the ExtractSchedule is run.
    pub fn execute_now(&mut self, pipeline_cache: Res<AppPipelineCache>) -> bool {
        match self.run_mode {
            RunMode::Continuous | RunMode::OneShot(_) => {
                panic!("Continuous and OneShot modes are not supported in execute_now(), please use execute() instead");
            }
            RunMode::Immediate => {
                self.extract_pipelines_aux(&pipeline_cache);
                self.unmap_all_aux();
                self.poll();
                self.run_immediate()
            }
        }
    }

    #[inline]
    fn ready_to_execute(&self) -> bool {
        (self.state != WorkerState::Working) && (self.run_mode != RunMode::OneShot(false))
    }

    pub(crate) fn run(mut worker: ResMut<Self>) {
        worker.run_aux();
    }
    fn run_immediate(&mut self) -> bool {
        // Workaround for interior mutability
        for i in 0..self.steps.len() {
            let result = match self.steps[i] {
                Step::ComputePass(_) => self.dispatch(i),
                Step::Swap(_, _) => self.swap(i),
            };

            if let Err(err) = result {
                match err {
                    Error::PipelineNotReady => return false,
                    _ => panic!("{:?}", err),
                }
            }
        }

        self.read_staging_buffers().unwrap();
        self.submit();
        self.map_staging_buffers();

        if self.poll() {
            self.command_encoder = Some(
                self.render_device
                    .create_command_encoder(&CommandEncoderDescriptor { label: None }),
            );
        }
        true
    }
    fn run_aux(&mut self) {
        if self.ready() {
            self.state = WorkerState::Available;
        }

        if self.ready_to_execute() {
            // Workaround for interior mutability
            for i in 0..self.steps.len() {
                let result = match self.steps[i] {
                    Step::ComputePass(_) => self.dispatch(i),
                    Step::Swap(_, _) => self.swap(i),
                };

                if let Err(err) = result {
                    match err {
                        Error::PipelineNotReady => return,
                        _ => panic!("{:?}", err),
                    }
                }
            }

            self.read_staging_buffers().unwrap();
            self.submit();
            self.map_staging_buffers();
        }

        if self.run_mode != RunMode::OneShot(false) && self.poll() {
            self.state = WorkerState::FinishedWorking;
            self.command_encoder = Some(
                self.render_device
                    .create_command_encoder(&CommandEncoderDescriptor { label: None }),
            );

            match self.run_mode {
                RunMode::Continuous | RunMode::Immediate => {}
                RunMode::OneShot(_) => self.run_mode = RunMode::OneShot(false),
            };
        }
    }

    pub(crate) fn unmap_all(mut worker: ResMut<Self>) {
        worker.unmap_all_aux();
    }

    fn unmap_all_aux(&mut self) {
        if self.ready_to_execute() || self.run_mode == RunMode::Immediate {
            for (_, staging_buffer) in &mut self.staging_buffers {
                if staging_buffer.mapped {
                    staging_buffer.buffer.unmap();
                    staging_buffer.mapped = false;
                }
            }
        }
    }

    pub(crate) fn extract_pipelines(
        mut worker: ResMut<Self>,
        pipeline_cache: Res<AppPipelineCache>,
    ) {
        worker.extract_pipelines_aux(&pipeline_cache);
    }

    fn extract_pipelines_aux(&mut self, pipeline_cache: &AppPipelineCache) {
        for (type_path, cached_id) in &self.cached_pipeline_ids.clone() {
            let Some(pipeline) = self.pipelines.get(type_path) else {
                continue;
            };

            if pipeline.is_some() {
                continue;
            };

            let cached_id = *cached_id;

            self.pipelines.insert(
                type_path.clone(),
                pipeline_cache.get_compute_pipeline(cached_id).cloned(),
            );
        }
    }
}
