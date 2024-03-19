//! Example showing how to have multiple passes

use bevy::{prelude::*, reflect::TypePath};
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

#[derive(Debug, Copy, Clone)]
enum ComputeWorkerFields {
    Value,
    Input,
    Output,
}

impl ComputeWorker for SimpleComputeWorker {
    type Fields = ComputeWorkerFields;
    fn build(app: &mut App) -> AppComputeWorker<Self> {
        //You can import the enum variants to avoid writing the full paths
        use ComputeWorkerFields::*;
        let worker = AppComputeWorkerBuilder::new(app)
            .add_uniform(Value, &3.)
            .add_storage(Input, &[1., 2., 3., 4.])
            .add_staging(Output, &[0f32; 4])
            .add_pass::<FirstPassShader>([4, 1, 1], &[Value, Input, Output]) // add each item + `value` from `input` to `output`
            .add_pass::<SecondPassShader>([4, 1, 1], &[Output]) // multiply each element of `output` by itself
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

    let result: Vec<f32> =
        compute_worker.read_vec(<SimpleComputeWorker as ComputeWorker>::Fields::Output);

    println!("got {:?}", result) // [16., 25., 36., 49.]
}
