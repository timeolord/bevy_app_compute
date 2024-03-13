//! Example showing how to have multiple passes

use bevy::{prelude::*, reflect::TypePath, render::render_resource::ShaderRef};
use bevy_app_compute::prelude::*;

#[derive(TypePath)]
struct FirstPassShader;

impl ComputeShader for FirstPassShader {
    fn shader() -> ShaderRef {
        "shaders/first_pass.wgsl".into()
    }
}

#[derive(TypePath)]
struct SecondPassShader;

impl ComputeShader for SecondPassShader {
    fn shader() -> ShaderRef {
        "shaders/second_pass.wgsl".into()
    }
}

#[derive(Resource)]
struct SimpleComputeWorker;

impl ComputeWorker for SimpleComputeWorker {
    fn build(app: &mut App) -> AppComputeWorker<Self> {
        let worker = AppComputeWorkerBuilder::new(app)
            .add_uniform("value", &3.)
            .add_storage("input", &[1., 2., 3., 4.])
            .add_staging("output", &[0f32; 4])
            .add_pass::<FirstPassShader>([4, 1, 1], &["value", "input", "output"]) // add each item + `value` from `input` to `output`
            .add_pass::<SecondPassShader>([4, 1, 1], &["output"]) // multiply each element of `output` by itself
            .build();

        // [1. + 3., 2. + 3., 3. + 3., 4. + 3.] = [4., 5., 6., 7.]
        // [4. * 4., 5. * 5., 6. * 6., 7. * 7.] = [16., 25., 36., 49.]

        worker
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(AppComputePlugin)
        .add_plugins(AppComputeWorkerPlugin::<SimpleComputeWorker>::default())
        .add_systems(Update, test)
        .run();
}

fn test(compute_worker: Res<AppComputeWorker<SimpleComputeWorker>>) {
    if !compute_worker.ready() {
        return;
    };

    let result: Vec<f32> = compute_worker.read_vec("output");

    println!("got {:?}", result) // [16., 25., 36., 49.]
}
