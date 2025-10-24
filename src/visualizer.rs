use crate::swoq_interface::Tile;
use crate::world_state::{Pos, WorldState};
use bevy::asset::AssetPlugin;
use bevy::camera::visibility::ViewVisibility;
use bevy::prelude::*;
use std::env;
use std::path::Path;
use std::sync::{Arc, Mutex, mpsc};

const TILE_SIZE: f32 = 32.0;
const WINDOW_WIDTH: f32 = 1280.0;
const WINDOW_HEIGHT: f32 = 960.0;

#[derive(Resource)]
pub struct GameStateResource {
    pub state: Arc<Mutex<Option<WorldState>>>,
    pub last_tick: i32,
    pub camera_initialized: bool,
}

#[derive(Resource)]
pub struct ReadySignal {
    sender: Option<mpsc::Sender<()>>,
}

#[derive(Resource)]
pub struct TileAssets {
    pub unknown: Handle<Image>,
    pub empty: Handle<Image>,
    pub player: Handle<Image>,
    pub wall: Handle<Image>,
    pub exit: Handle<Image>,
    pub door_red: Handle<Image>,
    pub door_green: Handle<Image>,
    pub door_blue: Handle<Image>,
    pub key_red: Handle<Image>,
    pub key_green: Handle<Image>,
    pub key_blue: Handle<Image>,
    pub boulder: Handle<Image>,
    pub pressure_plate_red: Handle<Image>,
    pub pressure_plate_green: Handle<Image>,
    pub pressure_plate_blue: Handle<Image>,
    pub enemy: Handle<Image>,
    pub sword: Handle<Image>,
    pub health: Handle<Image>,
    pub boss: Handle<Image>,
    pub treasure: Handle<Image>,
}

#[derive(Component)]
struct MapTile {
    #[allow(dead_code)]
    pos: Pos,
}

#[derive(Component)]
struct MapEntity;

pub fn run_visualizer(state: Arc<Mutex<Option<WorldState>>>, ready_tx: mpsc::Sender<()>) {
    let assets_path = env::var("SWOQ_ASSETS_FOLDER").ok();
    let file_path = if let Some(p) = assets_path {
        p
    } else {
        "assets".to_string()
    };

    tracing::info!("Bevy assets root set to: {}", file_path);

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "SWOQ Bot Visualizer".to_string(),
                        resolution: (WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32).into(),
                        ..default()
                    }),
                    ..default()
                })
                // Be explicit about the asset folder to avoid surprises with CWD
                .set(AssetPlugin {
                    file_path,
                    ..default()
                }),
        )
        .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.15)))
        .insert_resource(GameStateResource {
            state,
            last_tick: -1,
            camera_initialized: false,
        })
        .insert_resource(ReadySignal {
            sender: Some(ready_tx),
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (check_assets_loaded, update_map))
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera2d,
        Transform::default(),
        GlobalTransform::default(),
        Visibility::default(),
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Load all tile images
    let tile_assets = TileAssets {
        unknown: asset_server.load("images/tile_unknown.png"),
        empty: asset_server.load("images/tile_empty.png"),
        player: asset_server.load("images/tile_player.png"),
        wall: asset_server.load("images/tile_wall.png"),
        exit: asset_server.load("images/tile_exit.png"),
        door_red: asset_server.load("images/tile_door_red.png"),
        door_green: asset_server.load("images/tile_door_green.png"),
        door_blue: asset_server.load("images/tile_door_blue.png"),
        key_red: asset_server.load("images/tile_key_red.png"),
        key_green: asset_server.load("images/tile_key_green.png"),
        key_blue: asset_server.load("images/tile_key_blue.png"),
        boulder: asset_server.load("images/tile_boulder.png"),
        pressure_plate_red: asset_server.load("images/tile_pressure_plate_red.png"),
        pressure_plate_green: asset_server.load("images/tile_pressure_plate_green.png"),
        pressure_plate_blue: asset_server.load("images/tile_pressure_plate_blue.png"),
        enemy: asset_server.load("images/tile_enemy.png"),
        sword: asset_server.load("images/tile_sword.png"),
        health: asset_server.load("images/tile_health.png"),
        boss: asset_server.load("images/tile_boss.png"),
        treasure: asset_server.load("images/tile_treasure.png"),
    };

    commands.insert_resource(tile_assets);
}

fn check_assets_loaded(
    mut ready_signal: ResMut<ReadySignal>,
    tile_assets: Option<Res<TileAssets>>,
    asset_server: Res<AssetServer>,
) {
    // If we already signaled, do nothing
    if ready_signal.sender.is_none() {
        return;
    }

    // Wait for tile assets resource to exist
    let Some(tile_assets) = tile_assets else {
        return;
    };

    // Check if all assets are loaded
    use bevy::asset::LoadState;
    let all_loaded = [
        &tile_assets.unknown,
        &tile_assets.empty,
        &tile_assets.player,
        &tile_assets.wall,
        &tile_assets.exit,
        &tile_assets.door_red,
        &tile_assets.door_green,
        &tile_assets.door_blue,
        &tile_assets.key_red,
        &tile_assets.key_green,
        &tile_assets.key_blue,
        &tile_assets.boulder,
        &tile_assets.pressure_plate_red,
        &tile_assets.pressure_plate_green,
        &tile_assets.pressure_plate_blue,
        &tile_assets.enemy,
        &tile_assets.sword,
        &tile_assets.health,
        &tile_assets.boss,
        &tile_assets.treasure,
    ]
    .iter()
    .all(|handle| matches!(asset_server.load_state(handle.id()), LoadState::Loaded));

    if all_loaded {
        // Signal that assets are ready and game can start
        if let Some(sender) = ready_signal.sender.take() {
            let _ = sender.send(());
            tracing::info!("All assets loaded, signaling game thread to start...");
        }
    }
}

fn update_map(
    mut commands: Commands,
    mut game_state: ResMut<GameStateResource>,
    tile_assets: Option<Res<TileAssets>>,
    query: Query<Entity, With<MapEntity>>,
    mut camera_query: Query<&mut Transform, With<Camera2d>>,
) {
    // Wait for tile assets to be loaded
    let Some(tile_assets) = tile_assets else {
        return;
    };

    // Skip asset loading check - just try to render
    // (Bevy will use placeholder textures if assets aren't loaded yet)

    // Get the current world state and check if it changed
    let current_tick;
    let world_state_clone;
    {
        let state_guard = game_state.state.lock().unwrap();
        let Some(world_state) = state_guard.as_ref() else {
            return;
        };

        current_tick = world_state.tick;

        // Check if tick has changed
        if current_tick == game_state.last_tick && !query.is_empty() {
            return; // No change, don't re-render
        }

        // Clone the world state so we can release the lock
        world_state_clone = world_state.clone();
    } // Lock is dropped here

    tracing::info!("Rendering tick {} (level {})", current_tick, world_state_clone.level);
    game_state.last_tick = current_tick;

    // Clear existing tiles
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }

    render_world_state(
        &mut commands,
        &world_state_clone,
        &tile_assets,
        &mut camera_query,
        &mut game_state.camera_initialized,
    );
}

fn render_world_state(
    commands: &mut Commands,
    world_state: &WorldState,
    tile_assets: &TileAssets,
    camera_query: &mut Query<&mut Transform, With<Camera2d>>,
    camera_initialized: &mut bool,
) {
    tracing::debug!(
        "Rendering {} tiles for level {} tick {}",
        world_state.map.len(),
        world_state.level,
        world_state.tick
    );

    // Set camera once based on full maze dimensions (not discovered tiles)
    if !*camera_initialized && let Ok(mut camera_transform) = camera_query.single_mut() {
        // Center of the full maze
        let map_center_x = (world_state.map_width as f32 / 2.0) * TILE_SIZE;
        let map_center_y = -((world_state.map_height as f32 / 2.0) * TILE_SIZE);

        // Calculate scale to fit entire maze in window
        let map_width = world_state.map_width as f32 * TILE_SIZE;
        let map_height = world_state.map_height as f32 * TILE_SIZE;

        let scale_x = WINDOW_WIDTH / map_width;
        let scale_y = WINDOW_HEIGHT / map_height;
        let scale = scale_x.min(scale_y) * 0.9; // 0.9 for padding

        camera_transform.translation.x = map_center_x;
        camera_transform.translation.y = map_center_y;
        camera_transform.scale = Vec3::splat(1.0 / scale);

        *camera_initialized = true;
        tracing::info!(
            "Camera initialized: center=({}, {}), scale={}",
            map_center_x,
            map_center_y,
            1.0 / scale
        );
    }

    let center_x = 0.0;
    let center_y = 0.0;

    // Render all known tiles
    for (pos, tile) in &world_state.map {
        let x = center_x + (pos.x as f32 * TILE_SIZE);
        let y = center_y - (pos.y as f32 * TILE_SIZE);

        let texture = get_tile_texture(tile, tile_assets);

        commands.spawn((
            Sprite {
                image: texture,
                custom_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
                ..default()
            },
            Transform::from_xyz(x, y, 0.0),
            GlobalTransform::default(),
            Visibility::default(),
            InheritedVisibility::default(),
            ViewVisibility::default(),
            MapTile { pos: *pos },
            MapEntity,
        ));
    }

    // Render unexplored frontier (positions that can be reached but haven't been explored yet)
    for pos in &world_state.unexplored_frontier {
        let x = center_x + (pos.x as f32 * TILE_SIZE);
        let y = center_y - (pos.y as f32 * TILE_SIZE);

        // Draw frontier as a semi-transparent cyan square
        commands.spawn((
            Sprite {
                color: Color::srgba(0.0, 1.0, 1.0, 0.3), // Bright cyan, 30% opacity
                custom_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
                ..default()
            },
            Transform::from_xyz(x, y, 0.5), // z=0.5 to be above tiles but below player
            GlobalTransform::default(),
            Visibility::default(),
            InheritedVisibility::default(),
            ViewVisibility::default(),
            MapEntity,
        ));
    }

    // Render the player
    let player_x = center_x + (world_state.player_pos.x as f32 * TILE_SIZE);
    let player_y = center_y - (world_state.player_pos.y as f32 * TILE_SIZE);

    commands.spawn((
        Sprite {
            image: tile_assets.player.clone(),
            custom_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
            ..default()
        },
        Transform::from_xyz(player_x, player_y, 1.0),
        GlobalTransform::default(),
        Visibility::default(),
        InheritedVisibility::default(),
        ViewVisibility::default(),
        MapEntity,
    ));

    // Add text overlay with game info
    commands.spawn((
        Text::new(format!(
            "Level: {} | Tick: {} | Health: {:?} | Sword: {} | Inventory: {:?}",
            world_state.level,
            world_state.tick,
            world_state.player_health,
            if world_state.player_has_sword {
                "YES"
            } else {
                "NO"
            },
            world_state.player_inventory
        )),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        Transform::default(),
        GlobalTransform::default(),
        Visibility::default(),
        InheritedVisibility::default(),
        ViewVisibility::default(),
        MapEntity,
    ));
}

fn get_tile_texture(tile: &Tile, tile_assets: &TileAssets) -> Handle<Image> {
    match tile {
        Tile::Unknown => tile_assets.unknown.clone(),
        Tile::Empty => tile_assets.empty.clone(),
        Tile::Player => tile_assets.player.clone(),
        Tile::Wall => tile_assets.wall.clone(),
        Tile::Exit => tile_assets.exit.clone(),
        Tile::DoorRed => tile_assets.door_red.clone(),
        Tile::DoorGreen => tile_assets.door_green.clone(),
        Tile::DoorBlue => tile_assets.door_blue.clone(),
        Tile::KeyRed => tile_assets.key_red.clone(),
        Tile::KeyGreen => tile_assets.key_green.clone(),
        Tile::KeyBlue => tile_assets.key_blue.clone(),
        Tile::Boulder => tile_assets.boulder.clone(),
        Tile::PressurePlateRed => tile_assets.pressure_plate_red.clone(),
        Tile::PressurePlateGreen => tile_assets.pressure_plate_green.clone(),
        Tile::PressurePlateBlue => tile_assets.pressure_plate_blue.clone(),
        Tile::Enemy => tile_assets.enemy.clone(),
        Tile::Sword => tile_assets.sword.clone(),
        Tile::Health => tile_assets.health.clone(),
        Tile::Boss => tile_assets.boss.clone(),
        Tile::Treasure => tile_assets.treasure.clone(),
    }
}

#[allow(dead_code)]
fn tile_to_color(tile: &Tile) -> Color {
    match tile {
        Tile::Unknown => Color::srgb(0.1, 0.1, 0.1),
        Tile::Empty => Color::srgb(0.2, 0.2, 0.2),
        Tile::Player => Color::srgb(1.0, 1.0, 0.0),
        Tile::Wall => Color::srgb(0.5, 0.5, 0.5),
        Tile::Exit => Color::srgb(0.0, 1.0, 0.0),
        Tile::DoorRed => Color::srgb(0.8, 0.0, 0.0),
        Tile::DoorGreen => Color::srgb(0.0, 0.8, 0.0),
        Tile::DoorBlue => Color::srgb(0.0, 0.0, 0.8),
        Tile::KeyRed => Color::srgb(1.0, 0.3, 0.3),
        Tile::KeyGreen => Color::srgb(0.3, 1.0, 0.3),
        Tile::KeyBlue => Color::srgb(0.3, 0.3, 1.0),
        Tile::Boulder => Color::srgb(0.6, 0.5, 0.4),
        Tile::PressurePlateRed => Color::srgb(0.6, 0.2, 0.2),
        Tile::PressurePlateGreen => Color::srgb(0.2, 0.6, 0.2),
        Tile::PressurePlateBlue => Color::srgb(0.2, 0.2, 0.6),
        Tile::Enemy => Color::srgb(1.0, 0.0, 0.5),
        Tile::Sword => Color::srgb(0.7, 0.7, 0.9),
        Tile::Health => Color::srgb(1.0, 0.5, 0.5),
        Tile::Boss => Color::srgb(0.5, 0.0, 0.5),
        Tile::Treasure => Color::srgb(1.0, 0.8, 0.0),
    }
}
