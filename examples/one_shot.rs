//! Example showing how to execute compute shaders on demand

use bevy::prelude::*;
use bevy_app_compute::prelude::*;

#[derive(TypePath)]
struct SimpleShader;

impl ComputeShader for SimpleShader {
    fn shader() -> ShaderRef {
        "shaders/simple.wgsl".into()
    }
}

#[derive(Debug, Copy, Clone)]
enum ComputeWorkerFields {
    Uniform,
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
            .add_uniform(Self::Fields::Uniform, &5.)
            .add_staging(Self::Fields::Values, &[1., 2., 3., 4.])
            .add_pass::<SimpleShader>([4, 1, 1], &[Self::Fields::Uniform, Self::Fields::Values])
            .one_shot()
            .build();

        worker
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(AppComputePlugin)
        .add_systems(Startup, setup)
        .add_plugins(AppComputeWorkerPlugin::<SimpleComputeWorker>::default())
        .add_systems(Update, (on_click_compute, read_data))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera3dBundle::default());
}

fn on_click_compute(
    buttons: Res<ButtonInput<MouseButton>>,
    mut compute_worker: ResMut<AppComputeWorker<SimpleComputeWorker>>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }

    compute_worker.execute();
}

fn read_data(mut compute_worker: ResMut<AppComputeWorker<SimpleComputeWorker>>) {
    if !compute_worker.ready() {
        return;
    };

    let result: Vec<f32> =
        compute_worker.read_vec(<SimpleComputeWorker as ComputeWorker>::Fields::Values);

    compute_worker.write_slice(
        <SimpleComputeWorker as ComputeWorker>::Fields::Values,
        &result,
    );

    println!("got {:?}", result)
}
