use bevy::prelude::*;
use bevy_app_compute::prelude::*;

#[derive(TypePath)]
struct SimpleShader;

impl ComputeShader for SimpleShader {
    fn shader() -> ShaderRef {
        "shaders/change_dispatch_size.wgsl".into()
    }
}

#[derive(Debug, Copy, Clone)]
enum ComputeWorkerFields {
    Values,
}

#[derive(Resource)]
struct SimpleComputeWorker;

impl ComputeWorker for SimpleComputeWorker {
    type Fields = ComputeWorkerFields;

    fn build(app: &mut App) -> AppComputeWorker<Self> {
        //You can import the enum variants to avoid writing the full paths
        //use ComputeWorkerFields::*;
        let worker = AppComputeWorkerBuilder::new(app)
            .add_staging(Self::Fields::Values, &[0., 0., 0., 0.])
            .add_pass::<SimpleShader>([1, 1, 1], &[Self::Fields::Values])
            .build();

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

fn test(mut compute_worker: ResMut<AppComputeWorker<SimpleComputeWorker>>) {
    if !compute_worker.ready() {
        return;
    };

    let result: Vec<f32> =
        compute_worker.read_vec(<SimpleComputeWorker as ComputeWorker>::Fields::Values);

    if result[0].round() >= 1.0 {
        compute_worker.set_dispatch_size::<SimpleShader>([4, 1, 1]);
    }

    println!("{:?}", result)
}
