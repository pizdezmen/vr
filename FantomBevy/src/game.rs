//! Fight with the phantom and the score run. A straight port of `FightGame.cs` from the Unity version.
//! The phantom winds up (its blade heats up) -> the player raises the shield or parries with the sword
//! -> the phantom opens up -> the player strikes. Any sword hit dissolves the phantom, but a clean
//! swing, the weak point, a counter-strike and a streak without misses score more.

use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;
use bevy_mod_xr::camera::XrCamera;
use bevy_xr_utils::tracking_utils::{XrTrackedLeftGrip, XrTrackedRightGrip};

use crate::board::{Board, BoardText};
use crate::materials::{GlowMaterial, GridMaterial, PhantomMaterial, SolidMaterial};
use crate::meshes::{MeshBuilder, Rng};
use crate::phantom::{self, Attack, Pose};
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
    Dissolve,
    Results,
}

#[derive(Resource)]
struct Scene {
    phantom: Entity,
    phantom_material: Handle<PhantomMaterial>,
    right_hand: Entity,
    left_hand: Entity,
    sword: Entity,
    shield: Entity,
    shield_material: Handle<GlowMaterial>,
    flash: Entity,
    flash_material: Handle<GlowMaterial>,
    dust: Entity,
    cubes: usize,
}

#[derive(Resource)]
struct Game {
    state: State,
    state_time: f32,
    time: f32,
    pause: f32,
    current_windup: f32,
    attack: Attack,
    placed: bool,
    rng: Rng,
    blade_tilt: f32,

    // Phantom.
    root_pos: Vec3,
    root_rot: Quat,
    root_scale: f32,
    visible: bool,
    arm: Vec3,
    blade: Vec3,
    hand: Vec3,
    heat: f32,
    openness: f32,
    open_glow: f32,
    fx: u32, // 0 none, 1 assemble, 2 dissolve
    fx_time: f32,
    fx_duration: f32,
    hit: Vec3,
    push: Vec3,

    // Sword and shield this frame.
    head_pos: Vec3,
    sword_base: Vec3,
    sword_tip: Vec3,
    prev_tip: Vec3,
    tip_velocity: Vec3,
    shield_pos: Vec3,
    danger_base: Vec3,
    danger_tip: Vec3,
    hit_point: Vec3,
    has_prev_tip: bool,
    swing_path: f32,
    shield_flash: f32,
    flash_opacity: f32,
    flash_on: bool,
    opened_by: u8, // what blocked the last attack: 1 shield, 2 sword
    guard_since: f32,

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
        let idle = phantom::idle();
        let (arm, blade, hand) = phantom::blend(idle, idle, 0.0);
        Self {
            state: State::Waiting,
            state_time: 0.0,
            time: 0.0,
            pause: 0.0,
            current_windup: 0.0,
            attack: phantom::attacks()[0],
            placed: false,
            rng: Rng(0xC0FFEE11),
            blade_tilt: t::AIM_FROM_GRIP - t::BLADE_LEAN_DEFAULT,
            root_pos: Vec3::ZERO,
            root_rot: Quat::IDENTITY,
            root_scale: 1.0,
            visible: false,
            arm,
            blade,
            hand,
            heat: 0.0,
            openness: 0.0,
            open_glow: 0.0,
            fx: 0,
            fx_time: 0.0,
            fx_duration: 0.0,
            hit: Vec3::ZERO,
            push: Vec3::ZERO,
            head_pos: Vec3::ZERO,
            sword_base: Vec3::ZERO,
            sword_tip: Vec3::ZERO,
            prev_tip: Vec3::ZERO,
            tip_velocity: Vec3::ZERO,
            shield_pos: Vec3::ZERO,
            danger_base: Vec3::ZERO,
            danger_tip: Vec3::ZERO,
            hit_point: Vec3::ZERO,
            has_prev_tip: false,
            swing_path: 0.0,
            shield_flash: 0.0,
            flash_opacity: 0.0,
            flash_on: false,
            opened_by: 0,
            guard_since: -1.0,
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
            .add_systems(Startup, setup_scene)
            .add_systems(Update, game_update);
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

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut phantom_materials: ResMut<Assets<PhantomMaterial>>,
    mut grid_materials: ResMut<Assets<GridMaterial>>,
    mut glow_materials: ResMut<Assets<GlowMaterial>>,
    mut solid_materials: ResMut<Assets<SolidMaterial>>,
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
    commands.spawn((
        Mesh3d(meshes.add(pillars.build())),
        MeshMaterial3d(glow.clone()),
        NoFrustumCulling,
    ));

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
        .spawn((
            Mesh3d(meshes.add(dust_mesh.build())),
            MeshMaterial3d(glow.clone()),
            NoFrustumCulling,
        ))
        .id();

    // ----- Phantom.
    let (mesh, cubes) = phantom::build_mesh();
    info!("[FantomBevy] Phantom cubes: {cubes}");
    let phantom_material = phantom_materials.add(PhantomMaterial::default());
    let phantom = commands
        .spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(phantom_material.clone()),
            Visibility::Hidden,
            // Cubes are moved by the shader, so the mesh must never be culled by its resting bounds.
            NoFrustumCulling,
        ))
        .id();

    // ----- Hands. In the grip pose +X points right, -Z runs up along the handle (little finger to thumb)
    // and +Y points up and back, so leaning towards where the controller points is a negative turn around X.
    let right_hand = commands.spawn((Transform::default(), Visibility::default(), XrTrackedRightGrip)).id();
    let left_hand = commands.spawn((Transform::default(), Visibility::default(), XrTrackedLeftGrip)).id();

    // Sword: the blade points along -Z.
    let steel = [0.03, 0.06, 0.14, 1.0];
    let mut solid = MeshBuilder::default();
    solid.boxed(Vec3::new(0.0, 0.0, 0.02), Vec3::new(0.026, 0.026, 0.15), steel); // handle
    solid.boxed(Vec3::new(0.0, 0.0, -0.07), Vec3::new(0.12, 0.02, 0.02), steel); // guard
    solid.boxed(
        Vec3::new(0.0, 0.0, -(0.08 + t::PLAYER_BLADE / 2.0)),
        Vec3::new(0.026, 0.007, t::PLAYER_BLADE),
        [0.55, 0.85, 1.6, 1.0],
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
        [0.05, 0.25, 1.0, 0.3],
    );
    commands.spawn((
        Mesh3d(meshes.add(halo.build())),
        MeshMaterial3d(glow.clone()),
        NoFrustumCulling,
        ChildOf(sword),
    ));

    // Shield: a hexagonal force field with a bright rim and a small ring in the middle.
    // It faces the way the controller points.
    let mut b = MeshBuilder::default();
    let field = [0.1, 0.4, 1.0, 0.16];
    let rim = [0.4, 0.7, 1.0, 0.8];
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
        phantom_material,
        right_hand,
        left_hand,
        sword,
        shield,
        shield_material,
        flash,
        flash_material,
        dust,
        cubes,
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

fn ms(value: i32) -> String {
    if value < 0 { "—".to_string() } else { format!("{value} мс") }
}

impl Game {
    fn set_state(&mut self, next: State) {
        self.state = next;
        self.state_time = 0.0;
    }

    fn windup_time(&self) -> f32 {
        (t::WINDUP_START - t::WINDUP_STEP * self.streak as f32).max(t::WINDUP_MIN)
    }

    fn multiplier(&self) -> f32 {
        (1.0 + t::STREAK_STEP * self.streak as f32).min(t::STREAK_MAX)
    }

    fn to_world(&self, p: Vec3) -> Vec3 {
        self.root_pos + self.root_rot * (p * self.root_scale)
    }

    fn to_local(&self, p: Vec3) -> Vec3 {
        self.root_rot.inverse() * (p - self.root_pos) / self.root_scale
    }

    fn vector_to_local(&self, v: Vec3) -> Vec3 {
        self.root_rot.inverse() * v / self.root_scale
    }

    fn set_pose(&mut self, from: Pose, to: Pose, k: f32) {
        (self.arm, self.blade, self.hand) = phantom::blend(from, to, k);
    }

    fn set_open(&mut self, openness: f32) {
        self.openness = openness;
        let flicker = if openness > 0.0 { 0.6 + 0.4 * (self.time * 28.0).sin() } else { 0.0 };
        self.open_glow = openness * flicker * 0.7;
    }

    fn start_fx(&mut self, mode: u32, duration: f32) {
        self.fx = mode;
        self.fx_time = 0.0;
        self.fx_duration = duration;
        self.visible = true;
    }

    /// Returns true when the effect has finished.
    fn update_fx(&mut self, dt: f32) -> bool {
        if self.fx == 0 {
            return true;
        }
        self.fx_time += dt;
        if self.fx_time < self.fx_duration {
            return false;
        }
        if self.fx == 2 {
            self.visible = false;
        }
        self.fx = 0;
        true
    }

    fn hide_phantom(&mut self) {
        self.fx = 0;
        self.visible = false;
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
                "Раскалённый клинок фантома = сейчас ударит",
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
        let mut away = self.root_pos - self.head_pos;
        away.y = 0.0;
        if away.length_squared() < 0.01 {
            return;
        }
        let mut goal = self.head_pos + away.normalize() * (t::FIGHT_DISTANCE * self.root_scale);
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
        self.hide_phantom();
        self.set_state(State::Results);
        self.draw_board(board);
    }

    fn spawn_phantom(&mut self, board: &mut BoardText) {
        self.root_pos.y = 0.0;
        self.face_player(1.0);
        // The phantom assembles from pixels, bottom up.
        self.heat = 0.0;
        self.set_open(0.0);
        self.set_pose(phantom::idle(), phantom::idle(), 0.0);
        self.start_fx(1, 1.05);
        self.set_state(State::Assemble);
        self.draw_board(board);
    }

    /// What the player is covered by right now: 0 nothing, 1 shield, 2 sword.
    fn guarding(&self) -> u8 {
        let d = self.danger_tip - self.danger_base;
        let k = ((self.shield_pos - self.danger_base).dot(d) / d.length_squared()).clamp(0.0, 1.0);
        if self.shield_pos.distance(self.danger_base + d * k) < t::BLOCK_RANGE {
            return 1;
        }
        let (dist, _) =
            segment_distance(self.sword_base, self.sword_tip, self.danger_base, self.danger_tip);
        if dist < t::PARRY_RANGE { 2 } else { 0 }
    }

    fn start_attack(&mut self) {
        // Pick an attack the player is NOT covered from: standing still behind the shield will not help.
        let attacks = phantom::attacks();
        let start = (self.rng.next() * attacks.len() as f32) as usize % attacks.len();
        for i in 0..attacks.len() {
            self.attack = attacks[(start + i) % attacks.len()];
            let (base, tip) = phantom::blade_segment(self.attack.hit);
            self.danger_base = self.to_world(base);
            self.danger_tip = self.to_world(tip);
            if self.guarding() == 0 {
                break;
            }
        }
        self.guard_since = -1.0;
        self.current_windup = self.windup_time();
        self.set_state(State::Windup);
    }

    /// Remembers the moment the player got into guard (counted from the start of the wind-up).
    fn track_guard(&mut self, since_windup: f32) {
        if self.guarding() != 0 {
            if self.guard_since < 0.0 {
                self.guard_since = since_windup;
            }
        } else {
            self.guard_since = -1.0;
        }
    }

    fn resolve_impact(&mut self, board: &mut BoardText, input: &mut GameInput) {
        let guard = self.guarding();
        if guard != 0 {
            let since = if self.guard_since >= 0.0 {
                self.guard_since
            } else {
                self.state_time + self.current_windup
            };
            let reaction = (since * 1000.0).round() as i32;
            self.last_reaction = reaction;
            self.best_reaction =
                if self.best_reaction < 0 { reaction } else { self.best_reaction.min(reaction) };
            self.opened_by = guard;
            if guard == 1 {
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
            self.set_state(State::Open);
        } else {
            self.misses += 1;
            self.streak = 0;
            input.pulse(true, 1.0, 0.2);
            input.pulse(false, 1.0, 0.2);
            self.flash_opacity = 0.55;
            self.flash_on = true;
            self.say("Пропустил удар. Серия сброшена".to_string(), board);
            self.set_state(State::Recover);
        }
    }

    /// Did the player's sword touch the phantom's body?
    fn check_sword_hit(&mut self) -> bool {
        let base = self.to_local(self.sword_base);
        let tip = self.to_local(self.sword_tip);
        for i in 0..phantom::CAPSULE_R.len() {
            let (d, c) = segment_distance(base, tip, phantom::CAPSULE_A[i], phantom::CAPSULE_B[i]);
            if d < phantom::CAPSULE_R[i] {
                self.hit_point = c;
                return true;
            }
        }
        false
    }

    fn kill_phantom(&mut self, sword_speed: f32, board: &mut BoardText, input: &mut GameInput) {
        let push = self.vector_to_local(self.tip_velocity).clamp_length_max(6.0);

        // Will the blade pass through the core on the chest if it keeps moving?
        let base = self.to_local(self.sword_base);
        let tip = self.to_local(self.sword_tip);
        let (core_distance, _) =
            segment_distance(base, tip, phantom::CORE, phantom::CORE - push * 0.12);
        let weak = core_distance < t::WEAK_POINT_RANGE;
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

        // Dissolve. The cubes farthest from the hit: the hem, the crown and the tip of the blade.
        self.hit = self.hit_point;
        self.push = push;
        let far = self
            .hit
            .distance(Vec3::ZERO)
            .max(self.hit.distance(Vec3::new(0.0, 1.9, 0.0)))
            .max(self.hit.distance(self.hand) + phantom::BLADE_LENGTH);
        self.start_fx(2, far / phantom::WAVE_SPEED + 0.04 + phantom::DISSOLVE_LIFE);
        self.set_state(State::Dissolve);
        self.say(format!("{text}   +{points}"), board);
    }
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
    mut visibility: Query<&mut Visibility>,
    mut phantom_materials: ResMut<Assets<PhantomMaterial>>,
    mut glow_materials: ResMut<Assets<GlowMaterial>>,
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
            scene.cubes
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
    if game.head_pos.y < 0.3 {
        return; // the headset is not reporting the head position yet
    }

    if !game.placed {
        // Put the phantom and the board in front of the player.
        let mut dir = head_rot * Vec3::NEG_Z;
        dir.y = 0.0;
        let dir = if dir.length_squared() < 0.0001 { Vec3::NEG_Z } else { dir.normalize() };
        let scale = (game.head_pos.y / 1.62).clamp(0.6, 1.25);
        game.root_scale = scale * t::PHANTOM_SCALE;
        game.root_pos = game.head_pos + dir * (t::FIGHT_DISTANCE * game.root_scale);
        game.root_pos.y = 0.0;
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

    // Where the sword and the shield are now.
    let sword_rot = sword_local(game.blade_tilt);
    if let Ok(hand) = transforms.get(scene.right_hand) {
        game.sword_base = hand.transform_point(sword_rot * Vec3::new(0.0, 0.0, -0.1));
        game.sword_tip = hand.transform_point(sword_rot * Vec3::new(0.0, 0.0, -(0.08 + t::PLAYER_BLADE)));
    }
    if let (Ok(hand), Ok(shield)) = (transforms.get(scene.left_hand), transforms.get(scene.shield)) {
        game.shield_pos = hand.transform_point(shield.translation);
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

    game.state_time += dt;
    let st = game.state_time;

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
        apply_phantom(game, &scene, &mut transforms, &mut visibility, &mut phantom_materials);
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
        apply_phantom(game, &scene, &mut transforms, &mut visibility, &mut phantom_materials);
        return;
    }

    let bob = (game.time * 2.2).sin() * 0.015;
    let mut alive = false;

    match game.state {
        State::Assemble => {
            if game.update_fx(dt) {
                game.pause = game.rng.range(t::PAUSE_MIN, t::PAUSE_MAX);
                game.set_state(State::Idle);
            }
        }
        State::Idle => {
            alive = true;
            game.face_player((dt * 3.0).min(1.0));
            game.root_pos.y = bob;
            game.set_pose(phantom::idle(), phantom::idle(), 0.0);
            if st >= game.pause {
                game.start_attack();
            }
        }
        State::Windup => {
            // Wind-up: the arm goes back and the blade heats up. This is the hint for the player.
            alive = true;
            let k = (st / (game.current_windup * 0.55)).min(1.0);
            game.set_pose(phantom::idle(), game.attack.wind, 1.0 - (1.0 - k) * (1.0 - k));
            game.heat = (st / game.current_windup).min(1.0);
            game.track_guard(st);
            if st >= game.current_windup {
                game.set_state(State::Swing);
            }
        }
        State::Swing => {
            alive = true;
            let k = (st / t::SWING_TIME).min(1.0);
            game.set_pose(game.attack.wind, game.attack.hit, k * k);
            game.track_guard(game.current_windup + st);
            if k >= 1.0 {
                game.resolve_impact(board_text, &mut input);
            }
        }
        State::Open => {
            // The phantom is thrown back and open: it flickers bright, a hit now scores more.
            alive = true;
            let k = (st / 0.18).min(1.0);
            game.set_pose(game.attack.hit, phantom::open(), 1.0 - (1.0 - k) * (1.0 - k));
            game.heat = (1.0 - st * 4.0).max(0.0);
            game.set_open(1.0);
            if st >= t::OPEN_TIME {
                game.say("Не успел ответить".to_string(), board_text);
                game.set_state(State::Recover);
            }
        }
        State::Recover => {
            alive = true;
            let k = (st / 0.6).min(1.0);
            let from = if game.openness > 0.0 { phantom::open() } else { game.attack.hit };
            game.set_pose(from, phantom::idle(), k);
            game.heat = (game.heat - dt * 3.0).max(0.0);
            if k >= 1.0 {
                game.set_open(0.0);
                game.heat = 0.0;
                game.pause = game.rng.range(t::PAUSE_MIN, t::PAUSE_MAX);
                game.set_state(State::Idle);
            }
        }
        State::Dissolve => {
            if game.update_fx(dt) && st > 1.3 {
                game.spawn_phantom(board_text);
            }
        }
        _ => {}
    }

    // Any sword hit on a whole phantom dissolves it.
    if alive
        && game.state != State::Dissolve
        && sword_speed > t::MIN_SWING_SPEED
        && game.check_sword_hit()
    {
        game.kill_phantom(sword_speed, board_text, &mut input);
    }

    apply_phantom(game, &scene, &mut transforms, &mut visibility, &mut phantom_materials);
}

/// Sends the phantom's state to its transform and material.
fn apply_phantom(
    game: &Game,
    scene: &Scene,
    transforms: &mut Query<&mut Transform, Without<XrCamera>>,
    visibility: &mut Query<&mut Visibility>,
    materials: &mut Assets<PhantomMaterial>,
) {
    if let Ok(mut v) = visibility.get_mut(scene.phantom) {
        let wanted = if game.visible { Visibility::Visible } else { Visibility::Hidden };
        if *v != wanted {
            *v = wanted;
        }
    }
    if !game.visible {
        return;
    }
    if let Ok(mut root) = transforms.get_mut(scene.phantom) {
        root.translation = game.root_pos;
        root.rotation = game.root_rot;
        root.scale = Vec3::splat(game.root_scale);
    }
    if let Some(mut m) = materials.get_mut(&scene.phantom_material) {
        let (arm, blade) = phantom::part_matrices(game.arm, game.blade, game.hand);
        m.data.part0 = Mat4::IDENTITY;
        m.data.part1 = arm;
        m.data.part2 = blade;
        m.data.hit = game.hit.extend(0.0);
        m.data.push = game.push.extend(0.0);
        m.data.fx = Vec4::new(game.fx as f32, game.fx_time, game.heat, game.open_glow);
    }
}
