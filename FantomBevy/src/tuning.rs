//! Feel and score numbers. Same values as `Tuning.cs` in the Unity version, so the two can be compared.

// Player's sword and shield.
pub const PLAYER_BLADE: f32 = 0.62; // blade length, m
/// How far the blade leans forward from the grip axis, degrees. (Unity tilts 35° up from the
/// controller's pointing direction, which is the same blade direction measured from the other side.)
pub const BLADE_LEAN_DEFAULT: f32 = 25.0;
/// Angle between the grip axis and the controller's pointing direction, degrees.
pub const AIM_FROM_GRIP: f32 = 60.0;
pub const SHIELD_RADIUS: f32 = 0.23;
/// Sword closer than this to the phantom's blade = parry: the two visible blades touch.
pub const PARRY_RANGE: f32 = 0.04;
/// Visible half-width of the phantom's blade, m (counts for the shield test; the shield itself has no margin).
pub const PHANTOM_BLADE_HALF_WIDTH: f32 = 0.025;
pub const MIN_SWING_SPEED: f32 = 1.3; // m/s: a slow touch is not a hit

// Phantom.
/// Distance from the player's head to the phantom, in phantom heights relative to the mannequin (×scale), m.
pub const FIGHT_DISTANCE: f32 = 1.5;
/// How high the phantom floats above the floor, m.
pub const HOVER: f32 = 0.04;
pub const WINDUP_START: f32 = 0.9;
pub const WINDUP_MIN: f32 = 0.4;
pub const WINDUP_STEP: f32 = 0.05;
/// Wind-up plays the strike clip up to this share of the way to the hit, then holds there.
pub const TOP_FRACTION: f32 = 0.5;
/// Playback speed of the strike itself after the wind-up (1 = as animated).
pub const SWING_SPEED: f32 = 1.0;
/// How far past the hit moment the blade keeps counting before the strike is decided, s of clip time.
pub const FOLLOW_THROUGH: f32 = 0.05;
/// The phantom flinches this long after a sword hit before it falls apart, s (spec: at most 0.2).
pub const DYING_TIME: f32 = 0.15;
/// Size of a pixel in the appear/dissolve cloud, m.
pub const PIXEL_SIZE: f32 = 0.03;
/// An unblocked strike hurts only if the blade came this close to the player's body, m.
pub const REACH_TO_HURT: f32 = 0.4;
pub const OPEN_TIME: f32 = 1.6;
pub const PAUSE_MIN: f32 = 0.8;
pub const PAUSE_MAX: f32 = 2.0;

// Run and score.
pub const RUN_SECONDS: f32 = 180.0;
pub const SCORE_KILL: i32 = 100;
pub const SCORE_CLEAN_SWING: i32 = 50;
pub const SCORE_WEAK_POINT: i32 = 100;
pub const SCORE_COUNTER: i32 = 50;
pub const SCORE_PARRY_COUNTER: i32 = 100;
pub const SCORE_BLOCK: i32 = 20;
pub const SCORE_PARRY: i32 = 50;
pub const CLEAN_SWING_SPEED: f32 = 3.5;
pub const CLEAN_SWING_PATH: f32 = 0.5;
pub const WEAK_POINT_RANGE: f32 = 0.11;
pub const STREAK_STEP: f32 = 0.1;
pub const STREAK_MAX: f32 = 3.0;
pub const RANK_S: i32 = 6000;
pub const RANK_A: i32 = 4000;
pub const RANK_B: i32 = 2500;
pub const RANK_C: i32 = 1200;


// Picture and headset.
pub const TARGET_REFRESH_RATE: f32 = 90.0;
