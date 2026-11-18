//! Controller buttons, vibration and the display refresh rate, straight through OpenXR.
//! (Grip poses come from `bevy_xr_utils::tracking_utils`.)

use std::time::Duration;

use bevy::prelude::*;
use bevy_mod_openxr::{
    action_binding::{OxrSendActionBindings, OxrSuggestActionBinding},
    action_set_attaching::OxrAttachActionSet,
    action_set_syncing::{OxrActionSetSyncSet, OxrSyncActionSet},
    openxr_session_available, openxr_session_running,
    resources::OxrInstance,
    session::OxrSession,
};
use bevy_mod_xr::session::XrSessionCreated;
use openxr::{Haptic, HapticVibration, Path, Vector2f};

use crate::tuning;

const PROFILE: &str = "/interaction_profiles/oculus/touch_controller";

#[derive(Resource)]
pub struct GameActions {
    set: openxr::ActionSet,
    trigger_left: openxr::Action<f32>,
    trigger_right: openxr::Action<f32>,
    stick_right: openxr::Action<Vector2f>,
    haptic_left: openxr::Action<Haptic>,
    haptic_right: openxr::Action<Haptic>,
}

/// What the game reads every frame.
#[derive(Resource, Default)]
pub struct GameInput {
    /// A trigger was pressed this frame (either hand).
    pub trigger_down: bool,
    pub stick_y: f32,
    /// Refresh rate the headset reports, Hz (0 if unknown).
    pub refresh_rate: f32,
    trigger_held: bool,
    /// Vibration requests: (left hand?, strength 0..1, seconds).
    pulses: Vec<(bool, f32, f32)>,
}

impl GameInput {
    pub fn pulse(&mut self, left: bool, strength: f32, seconds: f32) {
        self.pulses.push((left, strength, seconds));
    }
}

pub struct XrInputPlugin;

impl Plugin for XrInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameInput>()
            .add_systems(Startup, create_actions.run_if(openxr_session_available))
            .add_systems(OxrSendActionBindings, suggest_bindings)
            .add_systems(XrSessionCreated, (attach_set, request_refresh_rate))
            .add_systems(
                PreUpdate,
                sync_actions.before(OxrActionSetSyncSet).run_if(openxr_session_running),
            )
            .add_systems(
                PreUpdate,
                read_input.after(OxrActionSetSyncSet).run_if(openxr_session_running),
            )
            .add_systems(PostUpdate, send_haptics.run_if(openxr_session_running));
    }
}

fn create_actions(instance: Res<OxrInstance>, mut cmds: Commands) {
    let set = instance.create_action_set("fantom", "Fantom", 0).unwrap();
    let actions = GameActions {
        trigger_left: set.create_action("trigger_left", "Left trigger", &[]).unwrap(),
        trigger_right: set.create_action("trigger_right", "Right trigger", &[]).unwrap(),
        stick_right: set.create_action("stick_right", "Right thumbstick", &[]).unwrap(),
        haptic_left: set.create_action("haptic_left", "Left vibration", &[]).unwrap(),
        haptic_right: set.create_action("haptic_right", "Right vibration", &[]).unwrap(),
        set,
    };
    cmds.insert_resource(actions);
}

fn suggest_bindings(
    actions: Option<Res<GameActions>>,
    mut bindings: MessageWriter<OxrSuggestActionBinding>,
) {
    let Some(actions) = actions else { return };
    let mut bind = |action: openxr::sys::Action, path: &'static str| {
        bindings.write(OxrSuggestActionBinding {
            action,
            interaction_profile: PROFILE.into(),
            bindings: vec![path.into()],
        });
    };
    bind(actions.trigger_left.as_raw(), "/user/hand/left/input/trigger/value");
    bind(actions.trigger_right.as_raw(), "/user/hand/right/input/trigger/value");
    bind(actions.stick_right.as_raw(), "/user/hand/right/input/thumbstick");
    bind(actions.haptic_left.as_raw(), "/user/hand/left/output/haptic");
    bind(actions.haptic_right.as_raw(), "/user/hand/right/output/haptic");
}

fn attach_set(actions: Option<Res<GameActions>>, mut attach: MessageWriter<OxrAttachActionSet>) {
    if let Some(actions) = actions {
        attach.write(OxrAttachActionSet(actions.set.clone()));
    }
}

fn sync_actions(actions: Option<Res<GameActions>>, mut sync: MessageWriter<OxrSyncActionSet>) {
    if let Some(actions) = actions {
        sync.write(OxrSyncActionSet(actions.set.clone()));
    }
}

/// Ask the headset for 90 Hz, like the Unity version does.
fn request_refresh_rate(session: Res<OxrSession>, mut input: ResMut<GameInput>) {
    match session.enumerate_display_refresh_rates() {
        Ok(rates) => {
            info!("[FantomBevy] refresh rates available: {rates:?}");
            if let Some(rate) = rates.iter().find(|r| (**r - tuning::TARGET_REFRESH_RATE).abs() < 1.0) {
                match session.request_display_refresh_rate(*rate) {
                    Ok(()) => info!("[FantomBevy] requested {rate} Hz"),
                    Err(e) => warn!("[FantomBevy] refresh rate request failed: {e}"),
                }
            }
        }
        Err(e) => warn!("[FantomBevy] cannot list refresh rates: {e}"),
    }
    input.refresh_rate = 0.0;
}

fn read_input(
    actions: Option<Res<GameActions>>,
    session: Res<OxrSession>,
    mut input: ResMut<GameInput>,
    mut frame: Local<u32>,
) {
    let Some(actions) = actions else { return };
    let left = actions.trigger_left.state(&session, Path::NULL).map(|s| s.current_state).unwrap_or(0.0);
    let right = actions.trigger_right.state(&session, Path::NULL).map(|s| s.current_state).unwrap_or(0.0);
    let held = left > 0.6 || right > 0.6;
    input.trigger_down = held && !input.trigger_held;
    input.trigger_held = held;
    input.stick_y = actions
        .stick_right
        .state(&session, Path::NULL)
        .map(|s| s.current_state.y)
        .unwrap_or(0.0);

    // The actual rate can change a moment after the request, so re-read it now and then.
    *frame += 1;
    if *frame % 120 == 1 {
        input.refresh_rate = session.get_display_refresh_rate().unwrap_or(0.0);
    }
}

fn send_haptics(actions: Option<Res<GameActions>>, session: Res<OxrSession>, mut input: ResMut<GameInput>) {
    let Some(actions) = actions else {
        input.pulses.clear();
        return;
    };
    for (left, strength, seconds) in input.pulses.drain(..) {
        let action = if left { &actions.haptic_left } else { &actions.haptic_right };
        let event = HapticVibration::new()
            .amplitude(strength)
            .frequency(openxr::FREQUENCY_UNSPECIFIED)
            .duration(openxr::Duration::from_nanos(
                Duration::from_secs_f32(seconds).as_nanos() as i64
            ));
        let _ = action.apply_feedback(&session, Path::NULL, &event);
    }
}
