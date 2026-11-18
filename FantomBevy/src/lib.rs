//! Fantom, Bevy version: the same first fight stage as the Unity project, for comparing the engines on Quest 3.

mod board;
mod game;
mod materials;
mod meshes;
mod phantom;
#[cfg(not(target_os = "android"))]
pub mod preview;
mod tuning;
mod xr_input;

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::view::NoIndirectDrawing;
use bevy_mod_openxr::{add_xr_plugins, init::OxrInitPlugin, types::OxrExtensions};
use bevy_mod_xr::camera::XrCamera;
use bevy_xr_utils::tracking_utils::{TrackingUtilitiesPlugin, suggest_action_bindings};

#[bevy_main]
fn main() {
    let mut app = App::new();
    app.add_plugins(add_xr_plugins(DefaultPlugins).set(OxrInitPlugin {
        exts: {
            let mut exts = OxrExtensions::default();
            // Lets the game ask the headset for 90 Hz.
            exts.raw_mut().fb_display_refresh_rate = true;
            exts
        },
        ..default()
    }))
    .add_plugins(TrackingUtilitiesPlugin)
    .add_systems(bevy_mod_openxr::action_binding::OxrSendActionBindings, suggest_action_bindings)
    .add_plugins((
        materials::FantomMaterialsPlugin,
        xr_input::XrInputPlugin,
        board::BoardPlugin,
        game::GamePlugin,
    ))
    .insert_resource(ClearColor(Color::srgb(0.001, 0.003, 0.012)))
    .add_systems(Update, setup_eye_cameras);

    #[cfg(target_os = "android")]
    android_fix::install(&mut app);

    app.run();
}

/// The headset's two eye cameras are created by the XR module; this sets how they draw.
pub(crate) fn setup_eye_cameras(cams: Query<Entity, Added<XrCamera>>, mut commands: Commands) {
    for cam in &cams {
        commands.entity(cam).insert((
            Msaa::Sample4,
            // Colours go to the screen as the shaders wrote them, like the Unity version (no HDR).
            Tonemapping::None,
            NoIndirectDrawing,
        ));
    }
}

/// On Android the activity always has a window, but in VR the picture goes to the headset through
/// OpenXR. If Bevy also creates a surface for that unused window, the Quest's graphics driver leaks
/// one file handle per frame and the app dies after a few minutes. Dropping the window from the
/// render world prevents the surface from being created. (Same fix as the open pull request 205 in
/// the bevy_oxr repository, applied here because it is not released yet.)
#[cfg(target_os = "android")]
mod android_fix {
    use bevy::prelude::*;
    use bevy::render::{Render, RenderApp, RenderSystems, view::ExtractedWindows};

    pub fn install(app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                Render,
                clear_android_window
                    .before(bevy::render::view::create_surfaces)
                    .before(bevy::render::view::prepare_windows)
                    .in_set(RenderSystems::PrepareViews),
            );
        }
    }

    fn clear_android_window(mut windows: ResMut<ExtractedWindows>) {
        windows.windows.clear();
    }
}
