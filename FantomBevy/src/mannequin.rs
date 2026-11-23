//! The Quaternius mannequin used as the phantom's body (spec: "ТЗ: анимации фантома на манекене
//! Quaternius"). Loads the glTF file, builds the animation graph with the legs masked out, swaps
//! the materials for the glowing ghost material, puts the phantom's sword in its right hand, and
//! plays clips at times chosen by the game, with short cross-fades between them.

use std::collections::HashMap;

use bevy::animation::{AnimationTargetId, graph::AnimationNodeIndex};
use bevy::app::AnimationSystems;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::world_serialization::WorldAssetRoot;

use crate::materials::{GhostMaterial, GlowMaterial};
use crate::meshes::MeshBuilder;

pub const GLB: &str = "characters/Fantom_Base.glb";
/// Height of the mannequin in its own units, m.
pub const HEIGHT: f32 = 1.83;
/// Length of the phantom's blade, m (mannequin scale).
pub const BLADE: f32 = 0.85;
/// Where the blade sits in the right hand (`hand_r` bone space): the fist is around this point,
/// and the blade leaves it towards the thumb, along the bone's +Z.
pub const GRIP_OFFSET: Vec3 = Vec3::new(0.0, 0.09, 0.0);
pub const GRIP_EULER_DEG: [f32; 3] = [30.0, 0.0, 0.0];

/// Rotation of the blade in the hand. On the PC it can be tried out without rebuilding:
/// set FANTOM_GRIP="x,y,z" (degrees, XYZ order) before running the preview.
fn grip_rotation() -> Quat {
    let mut e = GRIP_EULER_DEG;
    #[cfg(not(target_os = "android"))]
    if let Ok(v) = std::env::var("FANTOM_GRIP") {
        for (i, part) in v.split(',').enumerate().take(3) {
            if let Ok(x) = part.trim().parse::<f32>() {
                e[i] = x;
            }
        }
    }
    Quat::from_euler(EulerRot::XYZ, e[0].to_radians(), e[1].to_radians(), e[2].to_radians())
}
/// How long a change of clip blends from the old pose into the new one, s.
const FADE: f32 = 0.15;

const LEG_BONES: [&str; 10] = [
    "thigh_l", "calf_l", "foot_l", "ball_l", "ball_leaf_l", "thigh_r", "calf_r", "foot_r", "ball_r",
    "ball_leaf_r",
];

/// Bones the game reads (hit checks, pixel cloud, weak point, sword).
pub const BONES: [&str; 17] = [
    "root", "pelvis", "spine_01", "spine_03", "neck_01", "Head", "upperarm_l", "lowerarm_l", "hand_l",
    "upperarm_r", "lowerarm_r", "hand_r", "thigh_l", "calf_l", "foot_l", "thigh_r", "calf_r",
];

/// Body parts as capsules between bones: (from, to, radius in m at mannequin scale).
/// `foot_l`/`foot_r` are left out of the list above on purpose: the capsule runs to the calf end.
pub const BODY: [(&str, &str, f32); 11] = [
    ("pelvis", "spine_03", 0.16),
    ("spine_03", "neck_01", 0.15),
    ("upperarm_l", "lowerarm_l", 0.06),
    ("lowerarm_l", "hand_l", 0.05),
    ("upperarm_r", "lowerarm_r", 0.06),
    ("lowerarm_r", "hand_r", 0.05),
    ("thigh_l", "calf_l", 0.08),
    ("calf_l", "foot_l", 0.06),
    ("thigh_r", "calf_r", 0.08),
    ("calf_r", "foot_r", 0.06),
    ("neck_01", "Head", 0.07),
];
/// Head sphere radius, m; its centre is a little above the `Head` bone.
pub const HEAD_RADIUS: f32 = 0.12;
pub const HEAD_LIFT: f32 = 0.08;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Clip {
    Idle,
    A,
    ARec,
    B,
    BRec,
    C,
    Attack,
    Dash,
    ShieldBreak,
    HitHead,
    HitChest,
}

impl Clip {
    pub const ALL: [Clip; 11] = [
        Clip::Idle,
        Clip::A,
        Clip::ARec,
        Clip::B,
        Clip::BRec,
        Clip::C,
        Clip::Attack,
        Clip::Dash,
        Clip::ShieldBreak,
        Clip::HitHead,
        Clip::HitChest,
    ];

    fn name(self) -> &'static str {
        match self {
            Clip::Idle => "Sword_Idle",
            Clip::A => "Sword_Regular_A",
            Clip::ARec => "Sword_Regular_A_Rec",
            Clip::B => "Sword_Regular_B",
            Clip::BRec => "Sword_Regular_B_Rec",
            Clip::C => "Sword_Regular_C",
            Clip::Attack => "Sword_Attack",
            Clip::Dash => "Sword_Dash",
            Clip::ShieldBreak => "Idle_Shield_Break",
            Clip::HitHead => "Hit_Head",
            Clip::HitChest => "Hit_Chest",
        }
    }
}

#[derive(Resource)]
pub struct MannequinAssets {
    gltf: Handle<Gltf>,
    scene: Handle<bevy::world_serialization::WorldAsset>,
}

/// One phantom body. Put on the root entity; the glTF scene is spawned as its child.
#[derive(Component, Default)]
pub struct Mannequin {
    pub ready: bool,
    scene: Option<Entity>,
    player: Option<Entity>,
    nodes: HashMap<Clip, AnimationNodeIndex>,
    lengths: HashMap<Clip, f32>,
    pub bones: HashMap<&'static str, Entity>,
    /// Rest translation of the bones whose sideways movement is locked (no sliding).
    rest: Vec<(Entity, Vec3)>,
    pub blade: Option<Entity>,
    pub blade_hot: Option<Handle<GlowMaterial>>,
    pub blade_cold: Option<Handle<GlowMaterial>>,
    pub ghost: Handle<GhostMaterial>,
    // What to show: set by the game each frame.
    current: Option<(Clip, f32)>,
    previous: Option<(Clip, f32, f32)>, // clip, time, weight
}

impl Mannequin {
    pub fn new(ghost: Handle<GhostMaterial>) -> Self {
        Self { ghost, ..default() }
    }

    pub fn length(&self, clip: Clip) -> f32 {
        self.lengths.get(&clip).copied().unwrap_or(1.0)
    }

    /// Shows `clip` at `time` seconds. A different clip than last frame blends in over `FADE`.
    pub fn show(&mut self, clip: Clip, time: f32) {
        self.show_with_fade(clip, time, true);
    }

    pub fn show_with_fade(&mut self, clip: Clip, time: f32, fade: bool) {
        match self.current {
            Some((c, t)) if c != clip => {
                self.previous = if fade { Some((c, t, 1.0)) } else { None };
            }
            _ => {}
        }
        let len = self.length(clip);
        self.current = Some((clip, time.clamp(0.0, (len - 0.001).max(0.0))));
    }

    pub fn current_clip(&self) -> Option<Clip> {
        self.current.map(|c| c.0)
    }
}

pub struct MannequinPlugin;

impl Plugin for MannequinPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load_assets)
            .add_systems(Update, init_mannequins)
            .add_systems(PostUpdate, apply_clips.before(AnimationSystems))
            .add_systems(
                PostUpdate,
                lock_root_motion.after(AnimationSystems).before(TransformSystems::Propagate),
            );
    }
}

fn load_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(MannequinAssets {
        gltf: asset_server.load(GLB),
        scene: asset_server.load(GltfAssetLabel::Scene(0).from_asset(GLB)),
    });
}

/// Spawns a phantom body (hidden until the game shows it). Returns the root entity.
pub fn spawn(
    commands: &mut Commands,
    assets: &MannequinAssets,
    ghost: &mut Assets<GhostMaterial>,
) -> Entity {
    let material = ghost.add(GhostMaterial::default());
    let root = commands
        .spawn((Transform::default(), Visibility::Hidden, Mannequin::new(material)))
        .id();
    let scene = commands.spawn((WorldAssetRoot(assets.scene.clone()), ChildOf(root))).id();
    commands.entity(root).entry::<Mannequin>().and_modify(move |mut m| m.scene = Some(scene));
    root
}

#[allow(clippy::too_many_arguments)]
fn init_mannequins(
    mut commands: Commands,
    assets: Option<Res<MannequinAssets>>,
    asset_server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    clips: Res<Assets<AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut glow: ResMut<Assets<GlowMaterial>>,
    mut bodies: Query<(Entity, &mut Mannequin)>,
    children: Query<&Children>,
    names: Query<&Name>,
    targets: Query<&AnimationTargetId>,
    players: Query<(), With<AnimationPlayer>>,
    std_mats: Query<(), With<MeshMaterial3d<StandardMaterial>>>,
    transforms: Query<&Transform>,
) {
    let Some(assets) = assets else { return };
    for (root, mut body) in &mut bodies {
        if body.ready {
            continue;
        }
        if !asset_server.is_loaded_with_dependencies(&assets.gltf) {
            continue;
        }
        let Some(gltf) = gltfs.get(&assets.gltf) else { continue };
        let Some(scene) = body.scene else { continue };
        // Wait until the scene has been spawned under the root.
        let player = children.iter_descendants(scene).find(|e| players.contains(*e));
        let Some(player) = player else { continue };

        // Animation graph: every clip under one blend node that masks out the legs.
        let mut graph = AnimationGraph::new();
        let blend = graph.add_blend_with_mask(1, 1.0, graph.root);
        let mut nodes = HashMap::new();
        let mut lengths = HashMap::new();
        for clip in Clip::ALL {
            let Some(handle) = gltf.named_animations.get(clip.name()) else {
                error!("[FantomBevy] clip {} missing in {}", clip.name(), GLB);
                continue;
            };
            nodes.insert(clip, graph.add_clip(handle.clone(), 1.0, blend));
            lengths.insert(clip, clips.get(handle).map(|c| c.duration()).unwrap_or(1.0));
        }

        let mut bones = HashMap::new();
        let mut rest = Vec::new();
        let mut meshes_to_swap = Vec::new();
        for e in children.iter_descendants(scene) {
            if std_mats.contains(e) {
                meshes_to_swap.push(e);
            }
            let Ok(name) = names.get(e) else { continue };
            let name = name.as_str();
            if LEG_BONES.contains(&name) {
                if let Ok(id) = targets.get(e) {
                    graph.add_target_to_mask_group(*id, 0);
                }
            }
            if let Some(known) = BONES.iter().chain(["foot_l", "foot_r"].iter()).find(|b| **b == name) {
                bones.insert(*known, e);
            }
            if name == "root" || name == "pelvis" {
                if let Ok(t) = transforms.get(e) {
                    rest.push((e, t.translation));
                }
            }
        }

        // The phantom glows: swap the file's plain materials for the ghost material.
        for e in meshes_to_swap {
            commands
                .entity(e)
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert((MeshMaterial3d(body.ghost.clone()), NoFrustumCulling));
        }

        // The phantom's sword: a glowing blade in the right hand. Two copies of the same shape,
        // one cold blue and one hot red, cross-fade as the blade heats up before a strike.
        if let Some(&hand) = bones.get("hand_r") {
            let blade_mesh = |color: [f32; 4], halo: [f32; 4]| {
                let mut b = MeshBuilder::default();
                let z0 = 0.1;
                b.boxed(Vec3::new(0.0, 0.0, z0 + BLADE * 0.5), Vec3::new(0.05, 0.012, BLADE), color);
                b.boxed(Vec3::new(0.0, 0.0, z0 + BLADE * 0.5), Vec3::new(0.09, 0.03, BLADE + 0.04), halo);
                b.boxed(Vec3::new(0.0, 0.0, z0 - 0.01), Vec3::new(0.16, 0.03, 0.025), color); // guard
                b.boxed(Vec3::new(0.0, 0.0, -0.01), Vec3::new(0.03, 0.03, 0.2), [color[0], color[1], color[2], color[3] * 0.5]); // grip
                b.build()
            };
            let cold = glow.add(GlowMaterial::new(1.0));
            let hot = glow.add(GlowMaterial::new(0.0));
            let blade = commands
                .spawn((
                    Transform::from_translation(GRIP_OFFSET).with_rotation(grip_rotation()),
                    Visibility::Inherited,
                    ChildOf(hand),
                ))
                .id();
            commands.spawn((
                Mesh3d(meshes.add(blade_mesh([0.35, 0.75, 1.0, 1.0], [0.05, 0.3, 1.0, 0.35]))),
                MeshMaterial3d(cold.clone()),
                NoFrustumCulling,
                ChildOf(blade),
            ));
            commands.spawn((
                Mesh3d(meshes.add(blade_mesh([1.0, 0.35, 0.05, 1.0], [1.0, 0.1, 0.0, 0.4]))),
                MeshMaterial3d(hot.clone()),
                NoFrustumCulling,
                ChildOf(blade),
            ));
            body.blade = Some(blade);
            body.blade_cold = Some(cold);
            body.blade_hot = Some(hot);
        } else {
            error!("[FantomBevy] hand_r bone not found");
        }

        let graph = graphs.add(graph);
        commands.entity(player).insert(AnimationGraphHandle(graph));
        body.player = Some(player);
        body.nodes = nodes;
        body.lengths = lengths;
        body.bones = bones;
        body.rest = rest;
        body.ready = true;
        info!(
            "[FantomBevy] mannequin ready (root {root:?}), clips: {:?}",
            body.lengths.iter().map(|(c, l)| format!("{c:?}={l:.2}")).collect::<Vec<_>>()
        );
    }
}

/// Sends the chosen clip and time to the animation player. Time is set directly every frame,
/// so the game has full control (slow wind-ups, pauses, speed changes).
fn apply_clips(time: Res<Time>, mut bodies: Query<&mut Mannequin>, mut players: Query<&mut AnimationPlayer>) {
    let dt = time.delta_secs();
    for mut body in &mut bodies {
        let Some(player_entity) = body.player else { continue };
        let Ok(mut player) = players.get_mut(player_entity) else { continue };
        let Some((clip, t)) = body.current else { continue };

        if let Some((_, _, w)) = body.previous.as_mut() {
            *w -= dt / FADE;
        }
        if body.previous.is_some_and(|p| p.2 <= 0.0 || p.0 == clip) {
            body.previous = None;
        }
        let w_prev = body.previous.map(|p| p.2).unwrap_or(0.0);

        let wanted: Vec<AnimationNodeIndex> =
            [Some(clip), body.previous.map(|p| p.0)].into_iter().flatten().filter_map(|c| body.nodes.get(&c).copied()).collect();
        let playing: Vec<AnimationNodeIndex> = player.playing_animations().map(|(n, _)| *n).collect();
        for node in playing {
            if !wanted.contains(&node) {
                player.stop(node);
            }
        }
        if let Some(&node) = body.nodes.get(&clip) {
            player.play(node).set_seek_time(t).set_speed(0.0).set_weight(1.0 - w_prev);
        }
        if let Some((pc, pt, pw)) = body.previous {
            if let Some(&node) = body.nodes.get(&pc) {
                player.play(node).set_seek_time(pt).set_speed(0.0).set_weight(pw);
            }
        }
    }
}

/// Pelvis height in the sword stance (Sword_Idle), in the pelvis' parent space (Z is up there).
const STANCE_PELVIS_HEIGHT: f32 = 0.742;

/// The clips step, lunge and crouch (strike B drops the pelvis by 0.3 m). The phantom floats,
/// so its body stays over the same spot at the stance height; only the upper body moves.
fn lock_root_motion(bodies: Query<&Mannequin>, mut transforms: Query<&mut Transform>) {
    for body in &bodies {
        for (e, rest) in &body.rest {
            if let Ok(mut t) = transforms.get_mut(*e) {
                let height = if t.translation.z > 0.3 { STANCE_PELVIS_HEIGHT } else { rest.z };
                let wanted = Vec3::new(rest.x, rest.y, height);
                if t.translation != wanted {
                    t.translation = wanted;
                }
            }
        }
    }
}
