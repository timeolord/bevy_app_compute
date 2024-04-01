use bevy::{
    prelude::*,
    render::render_resource::{
        CachedComputePipelineId, CachedPipeline, CachedPipelineState, ComputePipeline, Pipeline,
    },
};

#[derive(Resource)]
pub struct AppPipelineCache {
    pub pipeline_cache: Vec<Option<CachedPipeline>>,
}
impl AppPipelineCache {
    #[inline]
    pub fn get_compute_pipeline(&self, id: CachedComputePipelineId) -> Option<&ComputePipeline> {
        self.pipeline_cache
            .get(id.id())
            .map(|x| {
                x.as_ref().map(|x| {
                    if let CachedPipelineState::Ok(Pipeline::ComputePipeline(pipeline)) = &x.state {
                        Some(pipeline)
                    } else {
                        None
                    }
                })
            })
            .flatten()
            .flatten()
    }
}
