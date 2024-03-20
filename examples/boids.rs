//! Example showing how to calculate boids data from compute shaders
//! For now they are stupid and just fly straight, need to fix this later on.
//! Reimplementation of https://github.com/gfx-rs/wgpu-rs/blob/master/examples/boids/main.rs

use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};

use bevy::{
    core::Pod,
    prelude::*,
    sprite::{MaterialMesh2dBundle, Mesh2dHandle},
    window::PrimaryWindow,
};

use bevy_app_compute::prelude::*;
use bytemuck::Zeroable;

use rand::distributions::{Distribution, Uniform};

// Debug mode
//const NUM_BOIDS: u32 = 500;

// Release mode
const NUM_BOIDS: u32 = 2_000;

#[derive(ShaderType, Pod, Zeroable, Clone, Copy)]
#[repr(C)]
struct Params {
    speed: f32,
    rule_1_distance: f32,
    rule_2_distance: f32,
    rule_3_distance: f32,
    rule_1_scale: f32,
    rule_2_scale: f32,
    rule_3_scale: f32,
}

#[derive(ShaderType, Pod, Zeroable, Clone, Copy)]
#[repr(C)]
struct Boid {
    pos: Vec2,
    vel: Vec2,
}

#[derive(TypePath)]
struct BoidsShader;

impl ComputeShader for BoidsShader {
    fn shader() -> ShaderRef {
        "shaders/boids.wgsl".into()
    }
}

#[derive(Debug, Copy, Clone)]
enum BoidsWorkerFields {
    Parameters,
    DeltaTime,
    Source,
    Destination,
}

struct BoidWorker;

impl ComputeWorker for BoidWorker {
    type Fields = BoidsWorkerFields;
    fn build(app: &mut App) -> AppComputeWorker<Self> {
        let params = Params {
            speed: 0.5,
            rule_1_distance: 0.2,
            rule_2_distance: 0.025,
            rule_3_distance: 0.01,
            rule_1_scale: 0.08,
            rule_2_scale: 0.02,
            rule_3_scale: 0.01,
        };

        let mut initial_boids_data = Vec::with_capacity(NUM_BOIDS as usize);
        let mut rng = rand::thread_rng();
        let unif = Uniform::new_inclusive(-1., 1.);

        for _ in 0..NUM_BOIDS {
            initial_boids_data.push(Boid {
                pos: Vec2::new(unif.sample(&mut rng), unif.sample(&mut rng)),
                vel: Vec2::new(
                    unif.sample(&mut rng) * params.speed,
                    unif.sample(&mut rng) * params.speed,
                ),
            });
        }

        //You can import the enum variants to avoid writing the full paths
        use BoidsWorkerFields::*;
        AppComputeWorkerBuilder::new(app)
            .add_uniform(Parameters, &params)
            .add_uniform(DeltaTime, &0.004f32)
            .add_staging(Source, &initial_boids_data)
            .add_staging(Destination, &initial_boids_data)
            .add_pass::<BoidsShader>(
                [NUM_BOIDS, 1, 1],
                &[Parameters, DeltaTime, Source, Destination],
            )
            .add_swap(Source, Destination)
            .build()
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(LogDiagnosticsPlugin::default())
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(AppComputePlugin)
        .add_plugins(AppComputeWorkerPlugin::<BoidWorker>::default())
        .insert_resource(ClearColor(Color::DARK_GRAY))
        .add_systems(Startup, setup)
        .add_systems(Update, move_entities)
        .run()
}

#[derive(Component)]
struct BoidEntity(pub usize);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2dBundle::default());

    let boid_mesh = meshes.add(RegularPolygon::new(1., 3));
    let boid_material = materials.add(Color::ANTIQUE_WHITE);

    // First boid in red, so we can follow it easily
    commands.spawn((
        BoidEntity(0),
        MaterialMesh2dBundle {
            mesh: Mesh2dHandle(boid_mesh.clone()),
            material: materials.add(Color::ORANGE_RED),
            ..Default::default()
        },
    ));

    for i in 1..NUM_BOIDS {
        commands.spawn((
            BoidEntity(i as usize),
            MaterialMesh2dBundle {
                mesh: Mesh2dHandle(boid_mesh.clone()),
                material: boid_material.clone(),
                ..Default::default()
            },
        ));
    }
}

fn move_entities(
    time: Res<Time>,
    mut worker: ResMut<AppComputeWorker<BoidWorker>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut q_boid: Query<(&mut Transform, &BoidEntity), With<BoidEntity>>,
) {
    if !worker.ready() {
        return;
    }

    let window = q_window.single();

    let boids = worker.read_vec::<Boid>(<BoidWorker as ComputeWorker>::Fields::Destination);

    worker.write(
        <BoidWorker as ComputeWorker>::Fields::DeltaTime,
        &time.delta_seconds(),
    );

    q_boid
        .par_iter_mut()
        .for_each(|(mut transform, boid_entity)| {
            let world_pos = Vec2::new(
                (window.width() / 2.) * (boids[boid_entity.0].pos.x),
                (window.height() / 2.) * (boids[boid_entity.0].pos.y),
            );

            transform.translation = world_pos.extend(0.);
            transform.look_to(Vec3::Z, boids[boid_entity.0].vel.extend(0.));
        });
}
