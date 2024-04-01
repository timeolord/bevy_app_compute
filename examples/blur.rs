//! A example using the GPU to immediately blur an image

use std::env;

use bevy::{prelude::*, render::render_asset::RenderAssetUsages};
use bevy_app_compute::prelude::*;
use bevy_egui::{
    egui::{self},
    EguiContexts, EguiPlugin,
};
use image::{DynamicImage, RgbaImage};

#[derive(TypePath)]
struct BlurShader;

impl ComputeShader for BlurShader {
    fn shader() -> ShaderRef {
        "shaders/blur.wgsl".into()
    }
}

#[derive(Debug, Copy, Clone)]
enum BlurWorkerFields {
    Image,
    ImageSize,
    BlurSize,
    Result,
}

const WIDTH: u32 = 16;
const HEIGHT: u32 = 16;

#[derive(Resource)]
struct BlurComputeWorker;

impl ComputeWorker for BlurComputeWorker {
    type Fields = BlurWorkerFields;

    fn build(app: &mut App) -> AppComputeWorker<Self> {
        //You can import the enum variants to avoid writing the full paths
        //use BlurWorkerFields::*;
        let image = &[
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 1., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 1., 0., 0., 0., 0.,
            1., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 1., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 1., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 1., 0., 0., 0., 0., 1., 1., 0., 0., 0., 0., 0., 0., 0., 0., 1., 1., 0., 0.,
            0., 0., 0., 1., 1., 0., 0., 0., 0., 0., 0., 1., 1., 0., 0., 0., 0., 0., 0., 0., 1., 1.,
            1., 1., 1., 1., 1., 1., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
            0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0., 0.,
        ];
        let worker = AppComputeWorkerBuilder::new(app)
            .add_staging(Self::Fields::Image, &image)
            .add_staging(Self::Fields::Result, &vec![0.0; (WIDTH * HEIGHT) as usize])
            .add_storage(Self::Fields::ImageSize, &[WIDTH, HEIGHT])
            .add_storage(Self::Fields::BlurSize, &[3u32, 3u32])
            .add_pass::<BlurShader>(
                [1, 1, 1],
                &[
                    Self::Fields::Image,
                    Self::Fields::Result,
                    Self::Fields::ImageSize,
                    Self::Fields::BlurSize,
                ],
            )
            .immediate()
            .build();

        worker
    }
}

fn main() {
    env::set_var("RUST_BACKTRACE", "1");

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .add_plugins(AppComputePlugin)
        .add_plugins(AppComputeWorkerPlugin::<BlurComputeWorker>::default())
        .add_systems(Update, blur)
        .run();
}

fn blur(
    mut compute_worker: ResMut<AppComputeWorker<BlurComputeWorker>>,
    pipeline_cache: Res<AppPipelineCache>,
    mut images: ResMut<Assets<Image>>,
    mut contexts: EguiContexts,
) {
    if !compute_worker.execute_now(pipeline_cache) {
        return;
    }
    let original: Vec<f32> = compute_worker.read_vec(BlurWorkerFields::Image);
    let result: Vec<f32> = compute_worker.read_vec(BlurWorkerFields::Result);

    // Do don't this in production code! This will leak memory!
    let original_handle = contexts.add_image(images.add(convert_to_image(original)));
    let blurred_handle = contexts.add_image(images.add(convert_to_image(result)));

    let ctx = contexts.ctx_mut();

    egui::CentralPanel::default().show(ctx, |ui| {
        egui::Grid::new("images").num_columns(2).show(ui, |ui| {
            let original_image = egui::Image::new(egui::load::SizedTexture::new(
                original_handle,
                egui::vec2(WIDTH as f32, HEIGHT as f32),
            ))
            .fit_to_exact_size([512.0, 512.0].into());
            let blurred_image = egui::Image::new(egui::load::SizedTexture::new(
                blurred_handle,
                egui::vec2(WIDTH as f32, HEIGHT as f32),
            ))
            .fit_to_exact_size([512.0, 512.0].into());
            ui.add(original_image);
            ui.add(blurred_image);
        })
    });
}

fn convert_to_image(image: Vec<f32>) -> Image {
    Image::from_dynamic(
        DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                WIDTH,
                HEIGHT,
                image
                    .iter()
                    .map(|&x| [(x * 255.0) as u8, (x * 255.0) as u8, (x * 255.0) as u8, 255])
                    .flatten()
                    .collect::<Vec<_>>(),
            )
            .unwrap()
            .into(),
        ),
        false,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}
