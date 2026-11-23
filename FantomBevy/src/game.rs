//! Fight with the phantom and the score run.
//! The phantom (the animated Quaternius mannequin) winds up a strike, its blade heating up, then
//! swings. The player blocks by putting the visible shield in the blade's path, or parries with the
//! sword. A blocked phantom staggers and is open for a counter-strike. Any sword hit makes it flinch
//! and fall apart into pixels from the point of impact. A clean swing, the weak point, a counter and
//! a streak without misses score more. Rules from "ТЗ: анимации фантома на манекене Quaternius".

use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;
use bevy_mod_xr::camera::XrCamera;
use bevy_xr_utils::tracking_utils::{XrTrackedLeftGrip, XrTrackedRightGrip};

use crate::board::{Board, BoardText};
use crate::mannequin::{self, Clip, Mannequin, MannequinAssets};
use crate::materials::{GhostMaterial, GlowMaterial, GridMaterial, PhantomMaterial, SolidMaterial};
use crate::meshes::{MeshBuilder, Rng};
use crate::phantom::{self, CloudBuilder};
use crate::tuning as t;
use crate::xr_input::GameInput;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Waiting,
    Ready,
    Assemble,
    Idle,
    Windup,
    Swing,
    Open,
    Recover,
    Dying,
    Dissolve,
    Results,
}

/// One strike of the phantom: which clip, when in the clip the blade moves fastest (measured on
/// the files, see the spec), the clip that brings it back to the stance afterwards, and how close
/// (head to phantom, m at mannequin scale) the phantom floats in during the wind-up so that this
/// strike's blade passes about 0.3 m in front of the player's head, where the shield is held.
/// The distances were measured in the PC preview (log line "closest to body").
#[derive(Clone, Copy)]
struct Strike {
    clip: Clip,
    hit: f32,
    recover: Option<Clip>,
    distance: f32,
}

/// Strike B (Sword_Regular_B) is left out: in that clip the mannequin bends far forward and sweeps
/// beside its own body, so the blade only reaches a standing player if the phantom comes almost
/// face to face. The dash strike gives the horizontal direction instead.
const STRIKES: [Strike; 4] = [
    Strike { clip: Clip::A, hit: 0.43, recover: Some(Clip::ARec), distance: 1.25 },
    Strike { clip: Clip::C, hit: 0.67, recover: None, distance: 1.05 },
    Strike { clip: Clip::Attack, hit: 0.38, recover: None, distance: 1.42 },
    Strike { clip: Clip::Dash, hit: 0.34, recover: None, distance: 1.3 },
];

#[derive(Resource)]
struct Scene {
    phantom: Entity,
    cloud: Entity,
    cloud_mesh: Handle<Mesh>,
    cloud_material: Handle<PhantomMaterial>,
    core: Entity,
    right_hand: Entity,
    left_hand: Entity,
    sword: Entity,
    shield: Entity,
    shield_material: Handle<GlowMaterial>,
    flash: Entity,
    flash_material: Handle<GlowMaterial>,
    dust: Entity,
}

#[derive(Resource)]
struct Game {
    state: State,
    state_time: f32,
    frames_in_state: u32,
    time: f32,
    pause: f32,
    current_windup: f32,
    strike: Strike,
    placed: bool,
    rng: Rng,
    blade_tilt: f32,

    // Phantom.
    root_pos: Vec3,
    root_rot: Quat,
    root_scale: f32,
    visible: bool,
    idle_time: f32,
    heat: f32,
    openness: f32,
    open_glow: f32,
    ghost_flash: f32,
    reaction: Clip,
    recover_clip: Clip,
    recover_from: f32,
    cloud_visible: bool,
    cloud_built: bool,
    cloud_cubes: usize,
    part_a: [Vec4; phantom::PARTS],
    part_b: [Vec4; phantom::PARTS],
    fx: u32, // 0 none, 1 assemble, 2 dissolve
    fx_time: f32,
    fx_duration: f32,
    hit: Vec3,
    push: Vec3,

    // Phantom blade, world space: this frame and last frame.
    blade_base: Vec3,
    blade_tip: Vec3,
    prev_blade_base: Vec3,
    prev_blade_tip: Vec3,
    has_prev_blade: bool,
    closest_to_head: f32,

    // Player's sword and shield this frame.
    head_pos: Vec3,
    sword_base: Vec3,
    sword_tip: Vec3,
    prev_tip: Vec3,
    tip_velocity: Vec3,
    hit_point: Vec3,
    hit_head: bool,
    has_prev_tip: bool,
    swing_path: f32,
    shield_flash: f32,
    flash_opacity: f32,
    flash_on: bool,
    opened_by: u8, // what blocked the last strike: 1 shield, 2 sword
    guard_since: f32,
    windup_started: f32,

    // Run.
    run_left: f32,
    score: i32,
    kills: i32,
    blocks: i32,
    parries: i32,
    misses: i32,
    streak: i32,
    best_streak: i32,
    weak_hits: i32,
    clean_hits: i32,
    last_reaction: i32,
    best_reaction: i32,
    message: String,
    shown_second: i32,

    // Frames per second.
    fps_timer: f32,
    fps_frames: u32,
    fps: u32,
    worst_frame: f32,
    refresh_rate: f32,
}

impl Default for Game {
    fn default() -> Self {
        Self {
            state: State::Waiting,
            state_time: 0.0,
            frames_in_state: 0,
            time: 0.0,
            pause: 0.0,
            current_windup: 0.0,
            strike: STRIKES[0],
            placed: false,
            rng: Rng(0xC0FFEE11),
            blade_tilt: t::AIM_FROM_GRIP - t::BLADE_LEAN_DEFAULT,
            root_pos: Vec3::ZERO,
            root_rot: Quat::IDENTITY,
            root_scale: 1.0,
            visible: false,
            idle_time: 0.0,
            heat: 0.0,
            openness: 0.0,
            open_glow: 0.0,
            ghost_flash: 0.0,
            reaction: Clip::HitChest,
            recover_clip: Clip::Idle,
            recover_from: 0.0,
            cloud_visible: false,
            cloud_built: false,
            cloud_cubes: 0,
            part_a: [Vec4::ZERO; phantom::PARTS],
            part_b: [Vec4::ZERO; phantom::PARTS],
            fx: 0,
            fx_time: 0.0,
            fx_duration: 0.0,
            hit: Vec3::ZERO,
            push: Vec3::ZERO,
            blade_base: Vec3::ZERO,
            blade_tip: Vec3::ZERO,
            prev_blade_base: Vec3::ZERO,
            prev_blade_tip: Vec3::ZERO,
            has_prev_blade: false,
            closest_to_head: f32::MAX,
            head_pos: Vec3::ZERO,
            sword_base: Vec3::ZERO,
            sword_tip: Vec3::ZERO,
            prev_tip: Vec3::ZERO,
            tip_velocity: Vec3::ZERO,
            hit_point: Vec3::ZERO,
            hit_head: false,
            has_prev_tip: false,
            swing_path: 0.0,
            shield_flash: 0.0,
            flash_opacity: 0.0,
            flash_on: false,
            opened_by: 0,
            guard_since: -1.0,
            windup_started: 0.0,
            run_left: 0.0,
            score: 0,
            kills: 0,
            blocks: 0,
            parries: 0,
            misses: 0,
            streak: 0,
            best_streak: 0,
            weak_hits: 0,
            clean_hits: 0,
            last_reaction: -1,
            best_reaction: -1,
            message: String::new(),
            shown_second: -1,
            fps_timer: 0.0,
            fps_frames: 0,
            fps: 0,
            worst_frame: 0.0,
            refresh_rate: 0.0,
        }
    }
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Game>()
            .add_plugins(mannequin::MannequinPlugin)
            .add_systems(Startup, setup_scene)
            .add_systems(Update, game_update);
        #[cfg(feature = "stress3")]
        app.add_systems(Update, drive_extras);
    }
}

// ---------- Scene ----------

fn sword_local(tilt: f32) -> Quat {
    Quat::from_rotation_x(-(t::AIM_FROM_GRIP - tilt).to_radians())
}

fn hex_point(i: u32) -> Vec3 {
    let angle = i as f32 * std::f32::consts::PI / 3.0 + std::f32::consts::PI / 6.0;
    Vec3::new(angle.cos(), angle.sin(), 0.0)
}

fn ring(b: &mut MeshBuilder, p0: Vec3, p1: Vec3, inner: f32, outer: f32, color: [f32; 4]) {
    let start = b.vertex(p0 * inner, Vec3::Z, color);
    b.vertex(p1 * inner, Vec3::Z, color);
    b.vertex(p1 * outer, Vec3::Z, color);
    b.vertex(p0 * outer, Vec3::Z, color);
    b.quad(start);
}

#[allow(clippy::too_many_arguments)]
fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mannequin_assets: Res<MannequinAssets>,
    mut phantom_materials: ResMut<Assets<PhantomMaterial>>,
    mut grid_materials: ResMut<Assets<GridMaterial>>,
    mut glow_materials: ResMut<Assets<GlowMaterial>>,
    mut solid_materials: ResMut<Assets<SolidMaterial>>,
    mut ghost_materials: ResMut<Assets<GhostMaterial>>,
) {
    // ----- Arena: dark void, glowing floor grid, distant light pillars, drifting pixels. Three draws.
    let mut floor = MeshBuilder::default();
    floor.boxed(Vec3::new(0.0, -0.005, 0.0), Vec3::new(120.0, 0.01, 120.0), [1.0; 4]);
    commands.spawn((
        Mesh3d(meshes.add(floor.build())),
        MeshMaterial3d(grid_materials.add(GridMaterial::default())),
        NoFrustumCulling,
    ));

    let glow = glow_materials.add(GlowMaterial::new(1.0));
    let mut pillars = MeshBuilder::default();
    const PILLARS: u32 = 18;
    for i in 0..PILLARS {
        let angle = i as f32 / PILLARS as f32 * std::f32::consts::TAU + 0.17;
        let radius = 16.0 + (i * 7 % 5) as f32 * 3.0;
        let height = 6.0 + (i * 13 % 7) as f32 * 3.0;
        let width = 0.3 + (i * 5 % 3) as f32 * 0.25;
        let bright = 0.05 + (i * 11 % 5) as f32 * 0.02;
        let color = [0.04 + (i * 3 % 4) as f32 * 0.02, 0.22, 1.0, bright];
        pillars.boxed(
            Vec3::new(angle.cos() * radius, height / 2.0, angle.sin() * radius),
            Vec3::new(width, height, width),
            color,
        );
    }
    commands.spawn((Mesh3d(meshes.add(pillars.build())), MeshMaterial3d(glow.clone()), NoFrustumCulling));

    let mut rng = Rng(11);
    let mut dust_mesh = MeshBuilder::default();
    for _ in 0..350 {
        let angle = rng.next() * std::f32::consts::TAU;
        let radius = 2.5 + rng.next() * 14.0;
        let size = 0.02 + rng.next() * 0.025;
        let p = Vec3::new(angle.cos() * radius, 0.2 + rng.next() * 7.0, angle.sin() * radius);
        dust_mesh.boxed(p, Vec3::splat(size), [0.2, 0.55, 1.0, 0.35]);
    }
    let dust = commands
        .spawn((Mesh3d(meshes.add(dust_mesh.build())), MeshMaterial3d(glow.clone()), NoFrustumCulling))
        .id();

    // ----- Phantom: the animated mannequin, plus the pixel cloud used to assemble and dissolve it.
    let phantom = mannequin::spawn(&mut commands, &mannequin_assets, &mut ghost_materials);
    #[cfg(feature = "stress3")]
    for side in [-1.0f32, 1.0] {
        let extra = mannequin::spawn(&mut commands, &mannequin_assets, &mut ghost_materials);
        commands.entity(extra).insert(Extra(side));
    }

    // The weak point: a small glowing diamond on the chest. Placed each frame in the phantom's space.
    let mut core_mesh = MeshBuilder::default();
    core_mesh.boxed(Vec3::ZERO, Vec3::new(0.07, 0.07, 0.02), [0.6, 1.0, 1.0, 1.0]);
    core_mesh.boxed(Vec3::ZERO, Vec3::new(0.12, 0.12, 0.03), [0.1, 0.6, 1.0, 0.35]);
    let core = commands
        .spawn((
            Mesh3d(meshes.add(core_mesh.build())),
            MeshMaterial3d(glow_materials.add(GlowMaterial::new(1.0))),
            Transform::default(),
            NoFrustumCulling,
            ChildOf(phantom),
        ))
        .id();

    let cloud_mesh = meshes.add(CloudBuilder::default().build());
    let cloud_material = phantom_materials.add(PhantomMaterial::default());
    let cloud = commands
        .spawn((
            Mesh3d(cloud_mesh.clone()),
            MeshMaterial3d(cloud_material.clone()),
            Transform::default(),
            Visibility::Hidden,
            // Cubes are moved by the shader, so the mesh must never be culled by its resting bounds.
            NoFrustumCulling,
        ))
        .id();

    // ----- Hands. In the grip pose +X points right, -Z runs up along the handle (little finger to thumb)
    // and +Y points up and back, so leaning towards where the controller points is a negative turn around X.
    let right_hand = commands.spawn((Transform::default(), Visibility::default(), XrTrackedRightGrip)).id();
    let left_hand = commands.spawn((Transform::default(), Visibility::default(), XrTrackedLeftGrip)).id();

    // The player's sword: warm gold, so it never gets mixed up with the phantom's blue. Blade along -Z.
    let bronze = [0.16, 0.08, 0.02, 1.0];
    let mut solid = MeshBuilder::default();
    solid.boxed(Vec3::new(0.0, 0.0, 0.02), Vec3::new(0.026, 0.026, 0.15), bronze); // handle
    solid.boxed(Vec3::new(0.0, 0.0, -0.07), Vec3::new(0.12, 0.02, 0.02), bronze); // guard
    solid.boxed(
        Vec3::new(0.0, 0.0, -(0.08 + t::PLAYER_BLADE / 2.0)),
        Vec3::new(0.026, 0.007, t::PLAYER_BLADE),
        [1.6, 1.05, 0.35, 1.0],
    ); // blade
    let sword = commands
        .spawn((
            Mesh3d(meshes.add(solid.build())),
            MeshMaterial3d(solid_materials.add(SolidMaterial::default())),
            Transform::from_rotation(sword_local(t::AIM_FROM_GRIP - t::BLADE_LEAN_DEFAULT)),
            Visibility::Hidden,
            NoFrustumCulling,
            ChildOf(right_hand),
        ))
        .id();
    let mut halo = MeshBuilder::default();
    halo.boxed(
        Vec3::new(0.0, 0.0, -(0.08 + t::PLAYER_BLADE / 2.0)),
        Vec3::new(0.042, 0.016, t::PLAYER_BLADE + 0.02),
        [1.0, 0.55, 0.1, 0.3],
    );
    commands.spawn((Mesh3d(meshes.add(halo.build())), MeshMaterial3d(glow.clone()), NoFrustumCulling, ChildOf(sword)));

    // Shield: a golden hexagonal force field with a bright rim and a small ring in the middle.
    // It faces the way the controller points. Its size is exactly the blocking zone.
    let mut b = MeshBuilder::default();
    let field = [1.0, 0.62, 0.15, 0.16];
    let rim = [1.0, 0.8, 0.4, 0.85];
    let r = t::SHIELD_RADIUS;
    let center = b.vertex(Vec3::ZERO, Vec3::Z, field);
    for i in 0..6 {
        let (p0, p1) = (hex_point(i), hex_point(i + 1));
        let a = b.vertex(p0 * r, Vec3::Z, field);
        let c = b.vertex(p1 * r, Vec3::Z, field);
        b.indices.extend_from_slice(&[center, a, c]);
        ring(&mut b, p0, p1, r - 0.014, r, rim);
        ring(&mut b, p0, p1, 0.05, 0.058, rim);
    }
    let shield_material = glow_materials.add(GlowMaterial::new(1.0));
    let aim = Quat::from_rotation_x(-t::AIM_FROM_GRIP.to_radians());
    let shield = commands
        .spawn((
            Mesh3d(meshes.add(b.build())),
            MeshMaterial3d(shield_material.clone()),
            Transform { translation: aim * Vec3::new(0.0, 0.0, -0.1), rotation: aim, scale: Vec3::ONE },
            Visibility::Hidden,
            NoFrustumCulling,
            ChildOf(left_hand),
        ))
        .id();

    // Red flash around the head when the player takes a hit.
    let mut flash_mesh = MeshBuilder::default();
    flash_mesh.boxed(Vec3::ZERO, Vec3::splat(0.6), [1.0, 0.05, 0.08, 1.0]);
    let flash_material = glow_materials.add(GlowMaterial::new(0.0));
    let flash = commands
        .spawn((
            Mesh3d(meshes.add(flash_mesh.build())),
            MeshMaterial3d(flash_material.clone()),
            Visibility::Hidden,
            NoFrustumCulling,
        ))
        .id();

    commands.insert_resource(Scene {
        phantom,
        cloud,
        cloud_mesh,
        cloud_material,
        core,
        right_hand,
        left_hand,
        sword,
        shield,
        shield_material,
        flash,
        flash_material,
        dust,
    });
}

// ---------- Helpers ----------

/// Distance between two segments (p1-q1 and p2-q2) and the closest point on the first one.
fn segment_distance(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (f32, Vec3) {
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.dot(d1), d2.dot(d2), d2.dot(r));
    let (mut s, mut tt) = (0.0, 0.0);
    if a > 1e-8 && e > 1e-8 {
        let b = d1.dot(d2);
        let c = d1.dot(r);
        let denom = a * e - b * b;
        s = if denom > 1e-8 { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
        tt = (b * s + f) / e;
        if tt < 0.0 {
            tt = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else if tt > 1.0 {
            tt = 1.0;
            s = ((b - c) / a).clamp(0.0, 1.0);
        }
    } else if e > 1e-8 {
        tt = (f / e).clamp(0.0, 1.0);
    } else if a > 1e-8 {
        s = (-d1.dot(r) / a).clamp(0.0, 1.0);
    }
    let c1 = p1 + d1 * s;
    (c1.distance(p2 + d2 * tt), c1)
}

fn point_segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let d = b - a;
    let k = ((p - a).dot(d) / d.length_squared().max(1e-8)).clamp(0.0, 1.0);
    p.distance(a + d * k)
}

/// Does the blade, moving from its last-frame position to this frame's, pass through the shield?
/// The test is against the visible hexagon only (plus the visible half-width of the blade).
fn blade_hits_shield(prev: (Vec3, Vec3), now: (Vec3, Vec3), shield: &GlobalTransform) -> bool {
    let to_local = shield.affine().inverse();
    let (scale, _, _) = shield.to_scale_rotation_translation();
    let r = t::SHIELD_RADIUS;
    let apothem = r * (std::f32::consts::PI / 6.0).cos();
    let margin = t::PHANTOM_BLADE_HALF_WIDTH / scale.x.max(1e-3);
    const STEPS: usize = 6;
    for i in 0..=STEPS {
        let k = i as f32 / STEPS as f32;
        let a = to_local.transform_point3(prev.0.lerp(now.0, k));
        let b = to_local.transform_point3(prev.1.lerp(now.1, k));
        // Where the blade crosses the shield's plane (z = 0 in shield space).
        if (a.z > 0.0) == (b.z > 0.0) {
            continue;
        }
        let p = a + (b - a) * (a.z / (a.z - b.z));
        // Inside the hexagon: its flat sides face 0°, 60° and 120°.
        let inside = (0..3).all(|j| {
            let ang = j as f32 * std::f32::consts::PI / 3.0;
            (p.x * ang.cos() + p.y * ang.sin()).abs() <= apothem + margin
        });
        if inside {
            return true;
        }
    }
    false
}

fn ms(value: i32) -> String {
    if value < 0 { "—".to_string() } else { format!("{value} мс") }
}

/// World positions of the mannequin bones the game needs, read from last frame's pose.
struct Bones<'a, 'w, 's> {
    body: &'a Mannequin,
    globals: &'a Query<'w, 's, &'static GlobalTransform>,
}

impl Bones<'_, '_, '_> {
    fn pos(&self, name: &str) -> Option<Vec3> {
        let e = self.body.bones.get(name)?;
        self.globals.get(*e).ok().map(|g| g.translation())
    }
}

impl Game {
    fn set_state(&mut self, next: State) {
        self.state = next;
        self.state_time = 0.0;
        self.frames_in_state = 0;
    }

    fn windup_time(&self) -> f32 {
        (t::WINDUP_START - t::WINDUP_STEP * self.streak as f32).max(t::WINDUP_MIN)
    }

    fn multiplier(&self) -> f32 {
        (1.0 + t::STREAK_STEP * self.streak as f32).min(t::STREAK_MAX)
    }

    fn forward(&self) -> Vec3 {
        self.root_rot * Vec3::Z
    }

    fn set_open(&mut self, openness: f32) {
        self.openness = openness;
        let flicker = if openness > 0.0 { 0.6 + 0.4 * (self.time * 28.0).sin() } else { 0.0 };
        self.open_glow = openness * flicker * 0.7;
    }

    // ---------- Board ----------

    fn rank(&self) -> &'static str {
        match self.score {
            s if s >= t::RANK_S => "S",
            s if s >= t::RANK_A => "A",
            s if s >= t::RANK_B => "B",
            s if s >= t::RANK_C => "C",
            _ => "D",
        }
    }

    fn say(&mut self, text: String, board: &mut BoardText) {
        self.message = text;
        self.draw_board(board);
    }

    fn draw_board(&self, board: &mut BoardText) {
        let fps_line = format!(
            "Кадров в секунду: {} из {:.0}     Наклон клинка: {}°",
            self.fps,
            self.refresh_rate,
            self.blade_tilt.round() as i32
        );
        if matches!(self.state, State::Ready | State::Waiting) {
            board.set(
                "ТРЕНИРОВКА МЕЧНИКА  (Bevy)",
                "Готов?",
                "Нажми курок, чтобы начать",
                "Щит в левой руке, меч в правой",
                "Подставь щит на пути клинка. Раскалённый клинок = сейчас ударит",
                &format!("Правый стик вверх и вниз меняет наклон клинка.   {fps_line}"),
            );
            return;
        }
        if self.state == State::Results {
            board.set(
                &format!("ЗАБЕГ ОКОНЧЕН. ОЦЕНКА: {}", self.rank()),
                &self.score.to_string(),
                "Нажми курок, чтобы сыграть ещё",
                &format!(
                    "Фантомов: {}     Лучшая серия: {}     Пропущено: {}",
                    self.kills, self.best_streak, self.misses
                ),
                &format!(
                    "Слабых мест: {}     Чистых взмахов: {}     Блоков: {}     Парирований: {}",
                    self.weak_hits, self.clean_hits, self.blocks, self.parries
                ),
                &format!("Лучшая реакция: {}     {fps_line}", ms(self.best_reaction)),
            );
            return;
        }
        let seconds = self.run_left.ceil().max(0.0) as i32;
        board.set(
            "ОЧКИ",
            &self.score.to_string(),
            &self.message,
            &format!(
                "Время: {}:{:02}     Серия: {}  (×{:.1})",
                seconds / 60,
                seconds % 60,
                self.streak,
                self.multiplier()
            ),
            &format!(
                "Фантомов: {}     Блоков: {}     Пропущено: {}",
                self.kills,
                self.blocks + self.parries,
                self.misses
            ),
            &format!(
                "Реакция: {}  (лучшая {})     {fps_line}",
                ms(self.last_reaction),
                ms(self.best_reaction)
            ),
        );
    }

    // ---------- Fight ----------

    /// The phantom turns to the player and keeps its distance.
    fn face_player(&mut self, k: f32) {
        self.face_player_at(k, t::FIGHT_DISTANCE);
    }

    /// Same, keeping `distance` (m at mannequin scale) between the player's head and the phantom.
    fn face_player_at(&mut self, k: f32, distance: f32) {
        let mut away = self.root_pos - self.head_pos;
        away.y = 0.0;
        if away.length_squared() < 0.01 {
            return;
        }
        let mut goal = self.head_pos + away.normalize() * (distance * self.root_scale);
        goal.y = self.root_pos.y;
        self.root_pos = self.root_pos.lerp(goal, k);
        let look = self.head_pos - self.root_pos;
        // +Z of the phantom faces the player.
        self.root_rot = Quat::from_rotation_y(look.x.atan2(look.z));
    }

    fn start_run(&mut self, board: &mut BoardText) {
        self.run_left = t::RUN_SECONDS;
        self.score = 0;
        self.kills = 0;
        self.blocks = 0;
        self.parries = 0;
        self.misses = 0;
        self.streak = 0;
        self.best_streak = 0;
        self.weak_hits = 0;
        self.clean_hits = 0;
        self.last_reaction = -1;
        self.best_reaction = -1;
        self.message = "Фантом идёт!".to_string();
        self.spawn_phantom(board);
    }

    fn finish_run(&mut self, board: &mut BoardText) {
        self.visible = false;
        self.cloud_visible = false;
        self.fx = 0;
        self.set_state(State::Results);
        self.draw_board(board);
    }

    fn spawn_phantom(&mut self, board: &mut BoardText) {
        self.root_pos.y = t::HOVER;
        self.face_player(1.0);
        self.heat = 0.0;
        self.set_open(0.0);
        self.visible = false;
        self.cloud_visible = false;
        self.fx = 0;
        self.set_state(State::Assemble);
        self.draw_board(board);
    }

    fn start_attack(&mut self) {
        #[allow(unused_mut)]
        let mut start = (self.rng.next() * STRIKES.len() as f32) as usize % STRIKES.len();
        // PC preview only: FANTOM_STRIKE=0..3 repeats one strike, for checking it.
        #[cfg(not(target_os = "android"))]
        if let Some(i) = std::env::var("FANTOM_STRIKE").ok().and_then(|v| v.parse::<usize>().ok()) {
            start = i % STRIKES.len();
        }
        self.strike = STRIKES[start];
        self.guard_since = -1.0;
        self.current_windup = self.windup_time();
        self.closest_to_head = f32::MAX;
        self.windup_started = self.time;
        self.set_state(State::Windup);
    }

    /// The strike was stopped by the shield (1) or the sword (2).
    fn resolve_guarded(&mut self, by: u8, board: &mut BoardText, input: &mut GameInput) {
        let since = if self.guard_since >= 0.0 { self.guard_since } else { self.time - self.windup_started };
        let reaction = (since * 1000.0).round() as i32;
        self.last_reaction = reaction;
        self.best_reaction = if self.best_reaction < 0 { reaction } else { self.best_reaction.min(reaction) };
        self.opened_by = by;
        if by == 1 {
            self.blocks += 1;
            self.score += t::SCORE_BLOCK;
            self.shield_flash = 1.0;
            input.pulse(true, 1.0, 0.09);
            self.say(format!("Блок! Бей мечом!   +{}", t::SCORE_BLOCK), board);
        } else {
            self.parries += 1;
            self.score += t::SCORE_PARRY;
            input.pulse(false, 1.0, 0.09);
            self.say(format!("Парирование! Бей!   +{}", t::SCORE_PARRY), board);
        }
        info!("[FantomBevy] strike {:?} guarded by {} (closest to body {:.2} m)", self.strike.clip, by, self.closest_to_head);
        self.set_state(State::Open);
    }

    fn resolve_unguarded(&mut self, at_clip_time: f32, board: &mut BoardText, input: &mut GameInput) {
        self.misses += 1;
        self.streak = 0;
        input.pulse(true, 1.0, 0.2);
        input.pulse(false, 1.0, 0.2);
        self.flash_opacity = 0.55;
        self.flash_on = true;
        self.say("Пропустил удар. Серия сброшена".to_string(), board);
        info!("[FantomBevy] strike {:?} hit the player (closest to body {:.2} m)", self.strike.clip, self.closest_to_head);
        match self.strike.recover {
            Some(clip) => {
                self.recover_clip = clip;
                self.recover_from = 0.0;
            }
            None => {
                self.recover_clip = self.strike.clip;
                self.recover_from = at_clip_time;
            }
        }
        self.set_state(State::Recover);
    }

    /// Did the player's sword touch the phantom's body? Fills `hit_point` and `hit_head`.
    fn check_sword_hit(&mut self, bones: &Bones) -> bool {
        let s = self.root_scale;
        if let (Some(neck), Some(head)) = (bones.pos("neck_01"), bones.pos("Head")) {
            let c = head + (head - neck).normalize_or_zero() * mannequin::HEAD_LIFT * s;
            if point_segment_distance(c, self.sword_base, self.sword_tip) < mannequin::HEAD_RADIUS * s {
                let (_, p) = segment_distance(self.sword_base, self.sword_tip, c, c);
                self.hit_point = p;
                self.hit_head = true;
                return true;
            }
        }
        for (a, b, r) in mannequin::BODY {
            let (Some(pa), Some(pb)) = (bones.pos(a), bones.pos(b)) else { continue };
            let (d, p) = segment_distance(self.sword_base, self.sword_tip, pa, pb);
            if d < r * s {
                self.hit_point = p;
                self.hit_head = a == "neck_01";
                return true;
            }
        }
        false
    }

    fn core_world(&self, bones: &Bones) -> Option<Vec3> {
        bones.pos("spine_03").map(|p| p + self.forward() * 0.13 * self.root_scale)
    }

    fn kill_phantom(&mut self, sword_speed: f32, bones: &Bones, board: &mut BoardText, input: &mut GameInput) {
        // Will the blade pass through the core on the chest if it keeps moving?
        let mut weak = false;
        if let Some(core) = self.core_world(bones) {
            let ahead = core - self.tip_velocity.normalize_or_zero() * 0.12;
            let (d, _) = segment_distance(self.sword_base, self.sword_tip, core, ahead);
            weak = d < t::WEAK_POINT_RANGE;
        }
        let clean = sword_speed >= t::CLEAN_SWING_SPEED && self.swing_path >= t::CLEAN_SWING_PATH;
        let counter = self.state == State::Open;

        let mut points = t::SCORE_KILL;
        let mut text = "Фантом рассыпался!";
        if counter {
            points += if self.opened_by == 2 { t::SCORE_PARRY_COUNTER } else { t::SCORE_COUNTER };
            text = "Ответный удар!";
        }
        if clean {
            points += t::SCORE_CLEAN_SWING;
            self.clean_hits += 1;
            text = "Чистый взмах!";
        }
        if weak {
            points += t::SCORE_WEAK_POINT;
            self.weak_hits += 1;
            text = if clean { "Чистый взмах в слабое место!" } else { "Слабое место!" };
        }
        let points = (points as f32 * self.multiplier()).round() as i32;
        self.score += points;
        self.kills += 1;
        self.streak += 1;
        self.best_streak = self.best_streak.max(self.streak);

        input.pulse(false, if weak || clean { 1.0 } else { 0.7 }, if weak { 0.2 } else { 0.14 });

        // A short flinch (head or chest reaction), then the pixel dissolve from the point of impact.
        self.reaction = if self.hit_head { Clip::HitHead } else { Clip::HitChest };
        self.ghost_flash = 0.6;
        self.heat = 0.0;
        self.set_open(0.0);
        self.say(format!("{text}   +{points}"), board);
        self.set_state(State::Dying);
    }
}

/// End points of the cloud's body parts (see `phantom::PARTS`) in world space, from the pose.
fn part_ends(game: &Game, bones: &Bones) -> Option<[(Vec3, Vec3, f32); phantom::PARTS]> {
    let s = game.root_scale;
    let mut out = [(Vec3::ZERO, Vec3::ZERO, 0.0); phantom::PARTS];
    for (i, (a, b, r)) in mannequin::BODY.iter().enumerate() {
        out[i] = (bones.pos(a)?, bones.pos(b)?, *r);
    }
    let (neck, head) = (bones.pos("neck_01")?, bones.pos("Head")?);
    let up = (head - neck).normalize_or_zero();
    out[phantom::PART_HEAD as usize] = (head, head + up * (2.0 * mannequin::HEAD_LIFT + 0.06) * s, mannequin::HEAD_RADIUS);
    let core = bones.pos("spine_03")? + game.forward() * 0.13 * s;
    out[phantom::PART_CORE as usize] = (core, core + Vec3::Y * 0.02 * s, 0.035);
    Some(out)
}

/// Builds the pixel cloud mesh once, sized from the mannequin's current pose.
fn build_cloud_mesh(game: &mut Game, bones: &Bones, meshes: &mut Assets<Mesh>, handle: &Handle<Mesh>) {
    let Some(parts) = part_ends(game, bones) else { return };
    let mut b = CloudBuilder::default();
    let mut rng = Rng(0x5EED);
    let size = t::PIXEL_SIZE;
    for (i, (a, c, r)) in parts.iter().enumerate() {
        let len = a.distance(*c) / game.root_scale;
        let kind = if i as u32 == phantom::PART_CORE { phantom::KIND_CORE } else { phantom::KIND_BODY };
        let size = if kind == phantom::KIND_CORE { size * 0.8 } else { size };
        b.part(&mut rng, i as u32, kind, len, *r, size);
    }
    game.cloud_cubes = b.cubes;
    let _ = meshes.insert(handle.id(), b.build());
    game.cloud_built = true;
    info!("[FantomBevy] pixel cloud built: {} cubes", game.cloud_cubes);
}

/// Poses the pixel cloud like the mannequin is posed now: stores the parts' end points in the
/// phantom's own space, and returns where the cloud itself goes.
fn pose_cloud(game: &mut Game, bones: &Bones, root: &GlobalTransform) -> Transform {
    let to_local = root.affine().inverse();
    if let Some(parts) = part_ends(game, bones) {
        for (i, (a, b, _)) in parts.iter().enumerate() {
            game.part_a[i] = to_local.transform_point3(*a).extend(0.0);
            game.part_b[i] = to_local.transform_point3(*b).extend(0.0);
        }
    }
    root.compute_transform()
}

// ---------- Frame ----------

#[allow(clippy::too_many_arguments)]
fn game_update(
    time: Res<Time<Real>>,
    scene: Res<Scene>,
    board: Res<Board>,
    mut game: ResMut<Game>,
    mut input: ResMut<GameInput>,
    mut board_text: ResMut<BoardText>,
    cameras: Query<&Transform, With<XrCamera>>,
    mut transforms: Query<&mut Transform, Without<XrCamera>>,
    globals: Query<&'static GlobalTransform>,
    mut visibility: Query<&mut Visibility>,
    mut bodies: Query<&mut Mannequin>,
    mut phantom_materials: ResMut<Assets<PhantomMaterial>>,
    mut glow_materials: ResMut<Assets<GlowMaterial>>,
    mut ghost_materials: ResMut<Assets<GhostMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let game = &mut *game;
    let board_text = &mut *board_text;
    let raw_dt = time.delta_secs();
    let dt = raw_dt.min(0.05);
    game.time += dt;

    // Drifting pixels.
    if let Ok(mut dust) = transforms.get_mut(scene.dust) {
        dust.rotation = Quat::from_rotation_y((game.time * 0.6).to_radians());
        dust.translation = Vec3::new(0.0, (game.time * 0.2).sin() * 0.2, 0.0);
    }

    // Frames per second: counted once a second, and written to the device log for measurements.
    game.fps_frames += 1;
    game.fps_timer += raw_dt;
    game.worst_frame = game.worst_frame.max(raw_dt);
    let mut second_passed = false;
    if game.fps_timer >= 1.0 {
        game.fps = (game.fps_frames as f32 / game.fps_timer).round() as u32;
        game.refresh_rate = input.refresh_rate;
        info!(
            "[FantomBevy] fps={} refresh={:.0} worst_frame_ms={:.1} state={:?} cubes={}",
            game.fps,
            game.refresh_rate,
            game.worst_frame * 1000.0,
            game.state,
            game.cloud_cubes
        );
        game.fps_frames = 0;
        game.fps_timer = 0.0;
        game.worst_frame = 0.0;
        second_passed = true;
    }

    // Head: the middle point between the two eye cameras.
    let mut head_pos = Vec3::ZERO;
    let mut head_rot = Quat::IDENTITY;
    let mut eyes = 0;
    for cam in &cameras {
        head_pos += cam.translation;
        head_rot = cam.rotation;
        eyes += 1;
    }
    if eyes == 0 {
        return;
    }
    game.head_pos = head_pos / eyes as f32;
    let Ok(mut body) = bodies.get_mut(scene.phantom) else { return };
    if game.head_pos.y < 0.3 || !body.ready {
        return; // the headset is not reporting the head position yet, or the phantom is still loading
    }

    if !game.placed {
        // Put the phantom and the board in front of the player.
        let mut dir = head_rot * Vec3::NEG_Z;
        dir.y = 0.0;
        let dir = if dir.length_squared() < 0.0001 { Vec3::NEG_Z } else { dir.normalize() };
        // Same height as the player: eyes are at about 93% of a person's height.
        let height = game.head_pos.y / 0.93;
        game.root_scale = (height / mannequin::HEIGHT).clamp(0.5, 1.2);
        game.root_pos = game.head_pos + dir * (t::FIGHT_DISTANCE * game.root_scale);
        game.root_pos.y = t::HOVER;
        game.face_player(1.0);

        let mut board_pos = game.head_pos + dir * 4.2;
        board_pos.y = game.head_pos.y + 2.0;
        if let Ok(mut panel) = transforms.get_mut(board.panel) {
            // The panel's front (+Z) faces the player.
            *panel = Transform::from_translation(board_pos).looking_to(board_pos - game.head_pos, Vec3::Y);
        }
        for entity in [board.panel, scene.sword, scene.shield] {
            if let Ok(mut v) = visibility.get_mut(entity) {
                *v = Visibility::Visible;
            }
        }
        game.placed = true;
        game.set_state(State::Ready);
        game.draw_board(board_text);
    }

    // Where the player's sword is now.
    let sword_rot = sword_local(game.blade_tilt);
    if let Ok(hand) = transforms.get(scene.right_hand) {
        game.sword_base = hand.transform_point(sword_rot * Vec3::new(0.0, 0.0, -0.1));
        game.sword_tip = hand.transform_point(sword_rot * Vec3::new(0.0, 0.0, -(0.08 + t::PLAYER_BLADE)));
    }
    if game.has_prev_tip && dt > 0.0 {
        game.tip_velocity = game.tip_velocity.lerp((game.sword_tip - game.prev_tip) / dt, 0.6);
    }
    let sword_speed = game.tip_velocity.length();
    // Length of the current swing: grows while the sword moves fast.
    if sword_speed > 1.0 && game.has_prev_tip {
        game.swing_path += game.sword_tip.distance(game.prev_tip);
    } else {
        game.swing_path = 0.0;
    }
    game.prev_tip = game.sword_tip;
    game.has_prev_tip = true;

    // Where the phantom's blade is now (last frame's pose; the motion between frames is swept).
    if let Some(g) = body.blade.and_then(|b| globals.get(b).ok()) {
        game.prev_blade_base = game.blade_base;
        game.prev_blade_tip = game.blade_tip;
        game.blade_base = g.transform_point(Vec3::Z * 0.12);
        game.blade_tip = g.transform_point(Vec3::Z * (0.1 + mannequin::BLADE));
        if !game.has_prev_blade {
            game.prev_blade_base = game.blade_base;
            game.prev_blade_tip = game.blade_tip;
            game.has_prev_blade = true;
        }
    }

    // Fading flashes.
    if game.flash_on {
        game.flash_opacity -= dt * 1.4;
        if let Some(mut m) = glow_materials.get_mut(&scene.flash_material) {
            m.intensity.x = game.flash_opacity.max(0.0);
        }
        if game.flash_opacity <= 0.0 {
            game.flash_on = false;
        }
        if let Ok(mut flash) = transforms.get_mut(scene.flash) {
            flash.translation = game.head_pos;
        }
        if let Ok(mut v) = visibility.get_mut(scene.flash) {
            *v = if game.flash_on { Visibility::Visible } else { Visibility::Hidden };
        }
    }
    if game.shield_flash > 0.0 {
        game.shield_flash = (game.shield_flash - dt * 4.0).max(0.0);
        if let Some(mut m) = glow_materials.get_mut(&scene.shield_material) {
            m.intensity.x = 1.0 + game.shield_flash * 4.0;
        }
        if let Ok(mut shield) = transforms.get_mut(scene.shield) {
            shield.scale = Vec3::splat(1.0 + game.shield_flash * 0.12);
        }
    }
    game.ghost_flash = (game.ghost_flash - dt * 3.0).max(0.0);

    game.state_time += dt;
    game.frames_in_state += 1;
    let st = game.state_time;
    let idle_len = body.length(Clip::Idle);
    game.idle_time = (game.idle_time + dt) % idle_len;

    if matches!(game.state, State::Ready | State::Results) {
        // Blade tilt adjustment with the right stick.
        if input.stick_y.abs() > 0.5 {
            game.blade_tilt = (game.blade_tilt + input.stick_y * 40.0 * dt).clamp(-10.0, 80.0);
            if let Ok(mut sword) = transforms.get_mut(scene.sword) {
                sword.rotation = sword_local(game.blade_tilt);
            }
        }
        if second_passed {
            game.draw_board(board_text);
        }
        if input.trigger_down && st > 0.5 {
            game.start_run(board_text);
        }
        body.show(Clip::Idle, game.idle_time);
        apply_phantom(game, &scene, &mut body, &mut transforms, &mut visibility, &mut phantom_materials, &mut glow_materials, &mut ghost_materials);
        return;
    }

    // The run is on.
    game.run_left -= dt;
    let second = game.run_left.ceil() as i32;
    if second != game.shown_second {
        game.shown_second = second;
        game.draw_board(board_text);
    }
    if game.run_left <= 0.0 {
        game.finish_run(board_text);
        apply_phantom(game, &scene, &mut body, &mut transforms, &mut visibility, &mut phantom_materials, &mut glow_materials, &mut ghost_materials);
        return;
    }

    let bob = (game.time * 2.2).sin() * 0.015;
    let mut alive = false;
    let root_global = globals.get(scene.phantom).copied().unwrap_or_default();

    match game.state {
        State::Assemble => {
            body.show_with_fade(Clip::Idle, game.idle_time, false);
            // Wait one frame so the pose and the new position have been applied, then build the
            // cloud from it and let the pixels fly together.
            if game.frames_in_state == 2 {
                let bones = Bones { body: &body, globals: &globals };
                if !game.cloud_built {
                    build_cloud_mesh(game, &bones, &mut meshes, &scene.cloud_mesh);
                }
                let tf = pose_cloud(game, &bones, &root_global);
                if let Ok(mut c) = transforms.get_mut(scene.cloud) {
                    *c = tf;
                }
                game.fx = 1;
                game.fx_time = 0.0;
                game.fx_duration = phantom::ASSEMBLE_TIME;
                game.cloud_visible = true;
            }
            if game.fx == 1 {
                game.fx_time += dt;
                if game.fx_time >= game.fx_duration {
                    game.fx = 0;
                    game.cloud_visible = false;
                    game.visible = true;
                    game.pause = game.rng.range(t::PAUSE_MIN, t::PAUSE_MAX);
                    game.set_state(State::Idle);
                }
            }
        }
        State::Idle => {
            alive = true;
            game.face_player((dt * 3.0).min(1.0));
            game.root_pos.y = t::HOVER + bob;
            body.show(Clip::Idle, game.idle_time);
            if st >= game.pause {
                game.start_attack();
            }
        }
        State::Windup => {
            // Wind-up: the start of the strike, slowed down, then held at the top while the blade
            // heats up. This is the hint for the player: where the blade is, the strike comes from.
            alive = true;
            let top = game.strike.hit * t::TOP_FRACTION;
            let k = (st / (game.current_windup * 0.6)).min(1.0);
            let eased = 1.0 - (1.0 - k) * (1.0 - k);
            body.show(game.strike.clip, top * eased);
            // Float in to this strike's distance while winding up.
            let distance = t::FIGHT_DISTANCE + (game.strike.distance - t::FIGHT_DISTANCE) * eased;
            game.face_player_at(0.5, distance);
            game.root_pos.y = t::HOVER + bob;
            game.heat = (st / game.current_windup).min(1.0);
            if st >= game.current_windup {
                game.set_state(State::Swing);
            }
        }
        State::Swing => {
            alive = true;
            let top = game.strike.hit * t::TOP_FRACTION;
            let clip_time = top + st * t::SWING_SPEED;
            body.show(game.strike.clip, clip_time);
            // Closest approach of the blade to the player's body (written to the log for tuning).
            // (the body: from the eyes down to the belly)
            let belly = game.head_pos - Vec3::Y * 0.7 * game.root_scale;
            let (d, _) = segment_distance(game.blade_base, game.blade_tip, game.head_pos, belly);
            game.closest_to_head = game.closest_to_head.min(d);
            let shield_hit = globals
                .get(scene.shield)
                .map(|g| blade_hits_shield((game.prev_blade_base, game.prev_blade_tip), (game.blade_base, game.blade_tip), g))
                .unwrap_or(false);
            let (parry_d, _) = segment_distance(game.sword_base, game.sword_tip, game.blade_base, game.blade_tip);
            if game.frames_in_state > 1 && shield_hit {
                game.resolve_guarded(1, board_text, &mut input);
            } else if game.frames_in_state > 1 && parry_d < t::PARRY_RANGE {
                game.resolve_guarded(2, board_text, &mut input);
            } else if clip_time >= game.strike.hit + t::FOLLOW_THROUGH {
                if game.closest_to_head < t::REACH_TO_HURT {
                    game.resolve_unguarded(clip_time, board_text, &mut input);
                } else {
                    // The blade never came near the player (they stepped back): no penalty.
                    info!("[FantomBevy] strike {:?} missed (closest to body {:.2} m)", game.strike.clip, game.closest_to_head);
                    game.say("Фантом промахнулся".to_string(), board_text);
                    game.recover_clip = game.strike.recover.unwrap_or(game.strike.clip);
                    game.recover_from = if game.strike.recover.is_some() { 0.0 } else { clip_time };
                    game.set_state(State::Recover);
                }
            }
        }
        State::Open => {
            // Thrown back and open: it staggers and flickers bright, a hit now scores more.
            alive = true;
            let len = body.length(Clip::ShieldBreak);
            if st < len {
                body.show(Clip::ShieldBreak, st);
            } else {
                body.show(Clip::Idle, game.idle_time);
            }
            game.heat = (game.heat - dt * 4.0).max(0.0);
            game.set_open(1.0);
            if st >= t::OPEN_TIME {
                game.say("Не успел ответить".to_string(), board_text);
                game.set_open(0.0);
                game.pause = game.rng.range(t::PAUSE_MIN, t::PAUSE_MAX);
                game.set_state(State::Idle);
            }
        }
        State::Recover => {
            // After a strike that landed: play the way back to the stance, then idle.
            alive = true;
            let clip = game.recover_clip;
            let at = game.recover_from + st;
            game.heat = (game.heat - dt * 3.0).max(0.0);
            if at < body.length(clip) {
                body.show(clip, at);
            } else {
                game.heat = 0.0;
                game.pause = game.rng.range(t::PAUSE_MIN, t::PAUSE_MAX);
                game.set_state(State::Idle);
            }
        }
        State::Dying => {
            body.show(game.reaction, st);
            if st >= t::DYING_TIME {
                let bones = Bones { body: &body, globals: &globals };
                let tf = pose_cloud(game, &bones, &root_global);
                if let Ok(mut c) = transforms.get_mut(scene.cloud) {
                    *c = tf;
                }
                let to_local = root_global.affine().inverse();
                game.hit = to_local.transform_point3(game.hit_point);
                game.push = (to_local.transform_vector3(game.tip_velocity)).clamp_length_max(6.0);
                // The cubes farthest from the hit are about one body height away.
                let far = game.hit.distance(Vec3::ZERO).max(game.hit.distance(Vec3::Y * mannequin::HEIGHT)) + 0.6;
                game.fx = 2;
                game.fx_time = 0.0;
                game.fx_duration = far / phantom::WAVE_SPEED + 0.04 + phantom::DISSOLVE_LIFE;
                game.cloud_visible = true;
                game.visible = false;
                game.set_state(State::Dissolve);
            }
        }
        State::Dissolve => {
            body.show(Clip::Idle, game.idle_time);
            game.fx_time += dt;
            if game.fx_time >= game.fx_duration {
                game.fx = 0;
                game.cloud_visible = false;
            }
            if game.fx == 0 && st > 1.3 {
                game.spawn_phantom(board_text);
            }
        }
        _ => {}
    }

    // Track when the player got into guard (for the reaction time on the board).
    if matches!(game.state, State::Windup | State::Swing) {
        let guarding = globals
            .get(scene.shield)
            .map(|g| {
                let local = g.affine().inverse().transform_point3((game.blade_base + game.blade_tip) * 0.5);
                local.z.abs() < 0.4 && local.truncate().length() < 0.5
            })
            .unwrap_or(false);
        if guarding {
            if game.guard_since < 0.0 {
                game.guard_since = game.time - game.windup_started;
            }
        } else {
            game.guard_since = -1.0;
        }
    }

    // Any sword hit on a whole phantom makes it fall apart.
    if alive && game.visible && sword_speed > t::MIN_SWING_SPEED {
        let bones = Bones { body: &body, globals: &globals };
        if game.check_sword_hit(&bones) {
            game.kill_phantom(sword_speed, &bones, board_text, &mut input);
        }
    }

    // Weak point position, in the phantom's own space.
    if let Some(chest) = {
        let bones = Bones { body: &body, globals: &globals };
        bones.pos("spine_03")
    } {
        if let Ok(mut core) = transforms.get_mut(scene.core) {
            let local = root_global.affine().inverse().transform_point3(chest);
            core.translation = local + Vec3::Z * 0.13;
            core.scale = Vec3::splat(1.0 + 0.15 * (game.time * 6.0).sin());
        }
    }

    apply_phantom(game, &scene, &mut body, &mut transforms, &mut visibility, &mut phantom_materials, &mut glow_materials, &mut ghost_materials);
}

/// Sends the phantom's state to its transform and materials.
#[allow(clippy::too_many_arguments)]
fn apply_phantom(
    game: &Game,
    scene: &Scene,
    body: &mut Mannequin,
    transforms: &mut Query<&mut Transform, Without<XrCamera>>,
    visibility: &mut Query<&mut Visibility>,
    cloud_materials: &mut Assets<PhantomMaterial>,
    glow: &mut Assets<GlowMaterial>,
    ghost: &mut Assets<GhostMaterial>,
) {
    for (entity, show) in [(scene.phantom, game.visible), (scene.cloud, game.cloud_visible)] {
        if let Ok(mut v) = visibility.get_mut(entity) {
            let wanted = if show { Visibility::Visible } else { Visibility::Hidden };
            if *v != wanted {
                *v = wanted;
            }
        }
    }
    if let Ok(mut root) = transforms.get_mut(scene.phantom) {
        root.translation = game.root_pos;
        root.rotation = game.root_rot;
        root.scale = Vec3::splat(game.root_scale);
    }
    if game.cloud_visible {
        if let Some(mut m) = cloud_materials.get_mut(&scene.cloud_material) {
            m.data.part_a = game.part_a;
            m.data.part_b = game.part_b;
            m.data.hit = game.hit.extend(0.0);
            m.data.push = game.push.extend(0.0);
            m.data.fx = Vec4::new(game.fx as f32, game.fx_time, 0.0, 0.0);
        }
    }
    if game.visible {
        if let Some(mut m) = ghost.get_mut(&body.ghost) {
            m.data = Vec4::new(game.open_glow, game.ghost_flash, 1.0, 0.0);
        }
        if let Some(h) = body.blade_cold.clone() {
            if let Some(mut m) = glow.get_mut(&h) {
                m.intensity.x = 1.0 - 0.8 * game.heat;
            }
        }
        if let Some(h) = body.blade_hot.clone() {
            if let Some(mut m) = glow.get_mut(&h) {
                m.intensity.x = 1.3 * game.heat;
            }
        }
    }
}

/// Load test (`--features stress3`): two more phantoms striking non-stop beside the main one,
/// to check the frame rate with three animated phantoms, as the spec asks.
#[cfg(feature = "stress3")]
#[derive(Component)]
struct Extra(f32);

#[cfg(feature = "stress3")]
fn drive_extras(
    time: Res<Time>,
    game: Res<Game>,
    mut extras: Query<(&Extra, &mut Mannequin, &mut Transform, &mut Visibility)>,
) {
    if !game.placed {
        return;
    }
    let toward = (game.head_pos - game.root_pos).with_y(0.0).normalize_or_zero();
    let side = Vec3::Y.cross(toward);
    for (extra, mut body, mut tf, mut vis) in &mut extras {
        if !body.ready {
            continue;
        }
        *vis = Visibility::Visible;
        tf.translation = game.root_pos + side * extra.0 * 1.3 * game.root_scale;
        tf.translation.y = t::HOVER;
        tf.rotation = game.root_rot;
        tf.scale = Vec3::splat(game.root_scale);
        let len = body.length(Clip::Attack);
        body.show(Clip::Attack, (time.elapsed_secs() + extra.0) % len);
    }
}
