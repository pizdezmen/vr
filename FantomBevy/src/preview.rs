//! Look check on the PC without a headset: runs the same scene in a window with a fixed camera,
//! plays a scripted fight (start, wind-up, a sword swing through the phantom) and saves pictures
//! to `Preview/`. Run with `cargo run --release --example preview`.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::WindowResolution;
use bevy_mod_xr::camera::XrCamera;
use bevy_xr_utils::tracking_utils::{XrTrackedLeftGrip, XrTrackedRightGrip};

use crate::xr_input::GameInput;

/// Pictures are taken every SHOT_STEP seconds from SHOT_FROM to SHOT_TO.
const SHOT_FROM: f32 = 1.0;
const SHOT_STEP: f32 = 0.25;
const SHOT_TO: f32 = 10.0;
/// FANTOM_SECONDS=60 keeps the preview running longer (for collecting strike numbers in the log).
fn run_seconds() -> f32 {
    std::env::var("FANTOM_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(SHOT_TO + 1.0)
}
const SWING_AT: f32 = 8.5;

pub fn run() {
    App::new()
        .add_plugins(DefaultPlugins.set(bevy::asset::AssetPlugin {
            file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").to_string(),
            ..default()
        }).set(WindowPlugin {
            primary_window: Some(Window {
                title: "Fantom Bevy preview".into(),
                resolution: WindowResolution::new(1280, 1280),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            crate::materials::FantomMaterialsPlugin,
            crate::board::BoardPlugin,
            crate::game::GamePlugin,
        ))
        .init_resource::<GameInput>()
        .insert_resource(ClearColor(Color::srgb(0.001, 0.003, 0.012)))
        .add_systems(Startup, spawn_camera)
        .add_systems(Update, crate::setup_eye_cameras)
        .add_systems(PreUpdate, drive)
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        XrCamera(0),
        Projection::Perspective(PerspectiveProjection {
            fov: 80f32.to_radians(),
            near: 0.05,
            ..default()
        }),
        // FANTOM_CAM=side looks at the phantom from its left side instead of from the player's eyes.
        if std::env::var("FANTOM_CAM").as_deref() == Ok("side") {
            Transform::from_xyz(2.6, 1.2, -1.4).looking_at(Vec3::new(0.0, 1.0, -1.4), Vec3::Y)
        } else {
            Transform::from_xyz(0.0, 1.62, 0.0).looking_to(Vec3::new(0.0, -0.12, -1.0), Vec3::Y)
        },
    ));
}

fn drive(
    time: Res<Time<Real>>,
    mut input: ResMut<GameInput>,
    mut right: Query<&mut Transform, (With<XrTrackedRightGrip>, Without<XrTrackedLeftGrip>)>,
    mut left: Query<&mut Transform, (With<XrTrackedLeftGrip>, Without<XrTrackedRightGrip>)>,
    mut commands: Commands,
    mut shot: Local<usize>,
    mut exit: MessageWriter<AppExit>,
) {
    let t = time.elapsed_secs();
    input.refresh_rate = 0.0;
    input.trigger_down = (1.5..1.6).contains(&t);

    // Hands held like a player would: shield on the left, sword on the right.
    // The grip's -Z axis runs up along the handle; tip it forward a little.
    let hold = Quat::from_rotation_x(60f32.to_radians());
    if let Ok(mut hand) = left.single_mut() {
        *hand = Transform::from_xyz(-0.28, 1.25, -0.45).with_rotation(hold);
    }
    if let Ok(mut hand) = right.single_mut() {
        // A horizontal swing through the phantom, right to left.
        let k = ((t - SWING_AT) / 0.22).clamp(0.0, 1.0);
        let x = 0.3 - 0.75 * k;
        *hand = Transform::from_xyz(x, 1.25, -0.8 - 0.15 * (k * std::f32::consts::PI).sin())
            .with_rotation(Quat::from_rotation_z(-1.2 * (k - 0.3)) * hold);
    }

    let next = SHOT_FROM + *shot as f32 * SHOT_STEP;
    if next <= SHOT_TO && t >= next {
        let path = format!("Preview/t{:05.2}.png", next);
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        *shot += 1;
    }
    if t > run_seconds() {
        exit.write(AppExit::Success);
    }
}
