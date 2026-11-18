//! Look check on the PC without a headset: runs the same scene in a window with a fixed camera,
//! plays a scripted fight (start, wind-up, a sword swing through the phantom) and saves pictures
//! to `Preview/`. Run with `cargo run --release --example preview`.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::WindowResolution;
use bevy_mod_xr::camera::XrCamera;
use bevy_xr_utils::tracking_utils::{XrTrackedLeftGrip, XrTrackedRightGrip};

use crate::xr_input::GameInput;

/// (time in seconds, file name)
const SHOTS: [(f32, &str); 6] = [
    (1.2, "1_ready"),
    (2.1, "2_assemble"),
    (3.2, "3_idle"),
    (4.6, "4_windup"),
    (6.12, "5_dissolve_start"),
    (6.45, "6_dissolve"),
];
const SWING_AT: f32 = 6.0;

pub fn run() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
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
        Transform::from_xyz(0.0, 1.62, 0.0).looking_to(Vec3::new(0.0, -0.12, -1.0), Vec3::Y),
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

    if *shot < SHOTS.len() && t >= SHOTS[*shot].0 {
        let path = format!("Preview/{}.png", SHOTS[*shot].1);
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        *shot += 1;
    }
    if t > SHOTS[SHOTS.len() - 1].0 + 1.0 {
        exit.write(AppExit::Success);
    }
}
