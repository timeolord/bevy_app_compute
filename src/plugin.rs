use std::marker::PhantomData;

use bevy::{
    prelude::*,
    render::{
        render_resource::{
            CachedPipeline, CachedPipelineState, Pipeline, PipelineCache, PipelineDescriptor,
        },
        MainWorld, RenderApp,
    },
    tasks::ComputeTaskPool,
};

use crate::{
    pipeline_cache::AppPipelineCache,
    traits::ComputeWorker,
    worker::AppComputeWorker,
};

/// The main plugin. Always include it if you want to use `bevy_app_compute`
pub struct AppComputePlugin;

impl Plugin for AppComputePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AppPipelineCache {
            pipeline_cache: vec![],
        });
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .add_systems(ExtractSchedule, update_app_pipeline);
    }
}

fn update_app_pipeline(pipeline_cache: Res<PipelineCache>, mut app_world: ResMut<MainWorld>) {
    let mut app_pipeline_cache = app_world.get_resource_mut::<AppPipelineCache>().unwrap();
    let mut cloned_pipelines = vec![];
    for pipeline in pipeline_cache.pipelines() {
        let cloned_state = match &pipeline.state {
            CachedPipelineState::Ok(x) => Some(CachedPipelineState::Ok(match x {
                Pipeline::RenderPipeline(x) => Pipeline::RenderPipeline(x.clone()),
                Pipeline::ComputePipeline(x) => Pipeline::ComputePipeline(x.clone()),
            })),
            _ => None,
        };
        let cloned_descriptor = match &pipeline.descriptor {
            PipelineDescriptor::RenderPipelineDescriptor(x) => {
                PipelineDescriptor::RenderPipelineDescriptor(x.clone())
            }
            PipelineDescriptor::ComputePipelineDescriptor(x) => {
                PipelineDescriptor::ComputePipelineDescriptor(x.clone())
            }
        };
        let cloned_pipeline = cloned_state.map(|state| CachedPipeline {
            state,
            descriptor: cloned_descriptor,
        });
        cloned_pipelines.push(cloned_pipeline);
    }
    app_pipeline_cache.pipeline_cache = cloned_pipelines;
}

/// Plugin to initialise your [`AppComputeWorker<W>`] structs.
pub struct AppComputeWorkerPlugin<W: ComputeWorker> {
    _phantom: PhantomData<W>,
}

impl<W: ComputeWorker> Default for AppComputeWorkerPlugin<W> {
    fn default() -> Self {
        Self {
            _phantom: Default::default(),
        }
    }
}

impl<W: ComputeWorker> Plugin for AppComputeWorkerPlugin<W> {
    fn build(&self, app: &mut App) {
        // register systems with run_if guards so they only run when worker exists
        app.add_systems(
            Update,
            AppComputeWorker::<W>::extract_pipelines
                .run_if(resource_exists::<AppComputeWorker<W>>),
        )
        .add_systems(
            PostUpdate,
            (AppComputeWorker::<W>::unmap_all, AppComputeWorker::<W>::run)
                .chain()
                .run_if(resource_exists::<AppComputeWorker<W>>),
        );
    }

    fn finish(&self, app: &mut App) {
        // ensure task pools are initialized before we access render resources
        if ComputeTaskPool::try_get().is_none() {
            // this handles the case where plugins are added in an order where task pools aren't ready yet
            ComputeTaskPool::get_or_init(bevy::tasks::TaskPool::new);
        }

        let worker = W::build(app);
        app.insert_resource(worker);
    }
}
