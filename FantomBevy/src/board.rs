//! Scoreboard above the phantom: score, hint, run time and helper numbers.
//! Text is drawn by Bevy's UI into a texture, and the texture is shown on a panel in the world.
//! The texture is only redrawn when the text changes.

use bevy::camera::RenderTarget;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::text::Font;

const WIDTH: u32 = 1024;
const HEIGHT: u32 = 640;

#[derive(Resource)]
pub struct Board {
    pub panel: Entity,
    camera: Entity,
    lines: [Entity; 6],
    redraw_frames: u32,
}

#[derive(Resource, Default)]
pub struct BoardText {
    pub lines: [String; 6],
}

impl BoardText {
    pub fn set(&mut self, title: &str, big: &str, message: &str, l1: &str, l2: &str, l3: &str) {
        let new = [title, big, message, l1, l2, l3];
        for (old, new) in self.lines.iter_mut().zip(new) {
            if old != new {
                *old = new.to_string();
            }
        }
    }
}

pub struct BoardPlugin;

impl Plugin for BoardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BoardText>()
            .add_systems(Startup, setup_board)
            .add_systems(PostUpdate, update_board);
    }
}

fn setup_board(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut fonts: ResMut<Assets<Font>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let image = images.add(Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None));
    let font = fonts.add(Font::from_bytes(
        include_bytes!("../assets_src/PTSans-Bold.ttf").to_vec(),
    ));

    let camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: -10,
                clear_color: ClearColorConfig::Custom(Color::srgba(0.01, 0.04, 0.13, 0.75)),
                ..default()
            },
            RenderTarget::from(image.clone()),
            Msaa::Off,
        ))
        .id();

    // (vertical centre measured from the middle of the panel, font size, colour) as in the Unity version
    let layout: [(f32, f32, Color); 6] = [
        (258.0, 40.0, Color::srgb(0.5, 0.72, 1.0)),
        (150.0, 150.0, Color::WHITE),
        (22.0, 44.0, Color::srgb(1.0, 0.82, 0.48)),
        (-70.0, 42.0, Color::srgb(0.81, 0.89, 1.0)),
        (-150.0, 38.0, Color::srgb(0.56, 0.7, 0.91)),
        (-235.0, 32.0, Color::srgb(0.45, 0.58, 0.8)),
    ];

    let root = commands
        .spawn((
            Node { width: px(WIDTH as f32), height: px(HEIGHT as f32), ..default() },
            UiTargetCamera(camera),
        ))
        .id();

    let mut lines = [Entity::PLACEHOLDER; 6];
    for (i, (y, size, color)) in layout.into_iter().enumerate() {
        let box_height = size * 1.5;
        let row = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0.0),
                    top: px(HEIGHT as f32 / 2.0 - y - box_height / 2.0),
                    width: px(WIDTH as f32),
                    height: px(box_height),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                ChildOf(root),
            ))
            .id();
        lines[i] = commands
            .spawn((
                Text::new(""),
                TextFont::default().with_font(font.clone()).with_font_size(size),
                TextColor(color),
                TextLayout::new(Justify::Center, LineBreak::NoWrap),
                ChildOf(row),
            ))
            .id();
    }

    let panel = commands
        .spawn((
            Mesh3d(meshes.add(Rectangle::new(2.4, 2.4 * HEIGHT as f32 / WIDTH as f32))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color_texture: Some(image),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                ..default()
            })),
            Transform::default(),
            Visibility::Hidden,
            NoFrustumCulling,
        ))
        .id();

    commands.insert_resource(Board { panel, camera, lines, redraw_frames: 4 });
}

fn update_board(
    text: Res<BoardText>,
    mut board: ResMut<Board>,
    mut texts: Query<&mut Text>,
    mut cameras: Query<&mut Camera>,
) {
    if text.is_changed() {
        for (entity, line) in board.lines.iter().zip(&text.lines) {
            if let Ok(mut t) = texts.get_mut(*entity) {
                if t.0 != *line {
                    t.0 = line.clone();
                }
            }
        }
        // Text layout lands a frame later, so keep the board camera on for a few frames.
        board.redraw_frames = 4;
    }
    if let Ok(mut camera) = cameras.get_mut(board.camera) {
        let active = board.redraw_frames > 0;
        if camera.is_active != active {
            camera.is_active = active;
        }
    }
    board.redraw_frames = board.redraw_frames.saturating_sub(1);
}
