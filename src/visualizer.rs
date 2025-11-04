use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use std::env;
use std::sync::{Arc, Mutex, mpsc};

use crate::swoq_interface::Tile;
use crate::types::Position;
use crate::world_state::WorldState;

/// Snapshot of game state including statistics
#[derive(Clone)]
pub struct GameStateSnapshot {
    pub world: WorldState,
    pub game_count: i32,
    pub successful_runs: i32,
    pub failed_runs: i32,
}

#[derive(Debug, Clone)]
pub struct LogMessage {
    pub text: String,
    pub color: LogColor,
}

#[derive(Debug, Clone, Copy)]
pub enum LogColor {
    #[allow(dead_code)]
    Cyan,
    #[allow(dead_code)]
    Green,
    #[allow(dead_code)]
    Yellow,
    White,
    Red,
}

impl LogColor {
    pub fn to_bevy_color(self) -> Color {
        match self {
            LogColor::Cyan => Color::srgb(0.4, 0.8, 1.0),
            LogColor::Green => Color::srgb(0.3, 1.0, 0.3),
            LogColor::Yellow => Color::srgb(1.0, 0.8, 0.3),
            LogColor::White => Color::srgb(0.9, 0.9, 0.9),
            LogColor::Red => Color::srgb(1.0, 0.3, 0.3),
        }
    }
}

const TILE_SIZE: f32 = 32.0;
const MAP_WIDTH: f32 = 1196.0;
const LOG_PANE_WIDTH: f32 = 484.0;
const WINDOW_WIDTH: f32 = 1680.0;
const WINDOW_HEIGHT: f32 = 960.0;
const MAX_LOG_ENTRIES: usize = 40;

#[derive(Resource)]
pub struct GameStateResource {
    pub state: Arc<Mutex<Option<GameStateSnapshot>>>,
    pub last_tick: i32,
    pub camera_initialized: bool,
    pub log_background_created: bool,
    pub log_rx: Arc<Mutex<mpsc::Receiver<LogMessage>>>,
    pub log_buffer: Vec<LogMessage>,
}

#[derive(Component)]
struct LogPane;

#[derive(Component)]
struct LogPaneBackground;

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
    pos: Position,
}

#[derive(Component)]
struct MapEntity;

pub fn run_visualizer(
    shared_state: Arc<Mutex<Option<GameStateSnapshot>>>,
    ready_tx: mpsc::Sender<()>,
    log_rx: mpsc::Receiver<LogMessage>,
) {
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
            state: shared_state,
            last_tick: -1,
            camera_initialized: false,
            log_background_created: false,
            log_rx: Arc::new(Mutex::new(log_rx)),
            log_buffer: Vec::new(),
        })
        .insert_resource(ReadySignal {
            sender: Some(ready_tx),
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (check_assets_loaded, update_map, update_log_pane))
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Spawn single camera
    commands.spawn(Camera2d);

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
        let Some(snapshot) = state_guard.as_ref() else {
            return;
        };

        current_tick = snapshot.world.tick;

        // Check if tick has changed
        if current_tick == game_state.last_tick && !query.is_empty() {
            return; // No change, don't re-render
        }

        // Clone the snapshot so we can release the lock
        world_state_clone = snapshot.clone();
    } // Lock is dropped here

    tracing::debug!("Rendering tick {} (level {})", current_tick, world_state_clone.world.level);
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

/// Helper function to spawn a border around a tile
/// Creates 4 thin rectangles forming an outline
fn spawn_tile_border(
    commands: &mut Commands,
    center_x: f32,
    center_y: f32,
    color: Color,
    border_width: f32,
    z: f32,
) {
    let half_tile = TILE_SIZE / 2.0;
    let half_border = border_width / 2.0;

    // Top border
    commands.spawn((
        Sprite {
            color,
            custom_size: Some(Vec2::new(TILE_SIZE + border_width, border_width)),
            ..default()
        },
        Transform::from_xyz(center_x, center_y - half_tile - half_border, z),
        MapEntity,
    ));

    // Bottom border
    commands.spawn((
        Sprite {
            color,
            custom_size: Some(Vec2::new(TILE_SIZE + border_width, border_width)),
            ..default()
        },
        Transform::from_xyz(center_x, center_y + half_tile + half_border, z),
        MapEntity,
    ));

    // Left border
    commands.spawn((
        Sprite {
            color,
            custom_size: Some(Vec2::new(border_width, TILE_SIZE + border_width)),
            ..default()
        },
        Transform::from_xyz(center_x - half_tile - half_border, center_y, z),
        MapEntity,
    ));

    // Right border
    commands.spawn((
        Sprite {
            color,
            custom_size: Some(Vec2::new(border_width, TILE_SIZE + border_width)),
            ..default()
        },
        Transform::from_xyz(center_x + half_tile + half_border, center_y, z),
        MapEntity,
    ));
}

fn render_world_state(
    commands: &mut Commands,
    snapshot: &GameStateSnapshot,
    tile_assets: &Res<TileAssets>,
    camera_query: &mut Query<&mut Transform, With<Camera2d>>,
    camera_initialized: &mut bool,
) {
    let world_state = &snapshot.world;
    tracing::debug!(
        "Rendering {} tiles for level {} tick {}",
        world_state.map.len(),
        world_state.level,
        world_state.tick
    );

    // Set camera once based on full maze dimensions (not discovered tiles)
    if !*camera_initialized && let Ok(mut camera_transform) = camera_query.single_mut() {
        // Calculate scale to fit entire maze in the MAP_WIDTH area
        let map_width = world_state.map.width as f32 * TILE_SIZE;
        let map_height = world_state.map.height as f32 * TILE_SIZE;

        let scale_x = MAP_WIDTH / map_width;
        let scale_y = WINDOW_HEIGHT / map_height;
        let scale = scale_x.min(scale_y) * 0.9; // 0.9 for some padding

        camera_transform.translation.x = 0.0;
        camera_transform.translation.y = 0.0;
        camera_transform.scale = Vec3::splat(1.0 / scale);

        *camera_initialized = true;
        tracing::info!("Camera initialized: center=(0, 0), scale={}", 1.0 / scale);
    }

    // Get current camera scale for positioning calculations
    let camera_scale = if let Ok(camera_transform) = camera_query.single() {
        camera_transform.scale.x
    } else {
        1.0
    };

    // Position map at top-left with 20px margin, but below the two header lines (60px)
    // Camera scaling affects world coordinates, so we multiply by camera scale
    // to convert screen pixels to world coordinates
    let center_x = (-(WINDOW_WIDTH / 2.0) + 20.0 + (TILE_SIZE / 2.0)) * camera_scale;
    let center_y = ((WINDOW_HEIGHT / 2.0) - 60.0 - (TILE_SIZE / 2.0)) * camera_scale;

    // Render all known tiles
    for (pos, tile) in world_state.map.iter() {
        let x = center_x + (pos.x as f32 * TILE_SIZE);
        let y = center_y - (pos.y as f32 * TILE_SIZE);

        let texture = get_tile_texture(tile, tile_assets);

        // Check if this is an open door and make it more transparent
        let color = match tile {
            Tile::DoorRed if world_state.is_door_open(crate::types::Color::Red) => {
                Color::srgba(1.0, 1.0, 1.0, 0.4) // 40% opacity for open doors
            }
            Tile::DoorGreen if world_state.is_door_open(crate::types::Color::Green) => {
                Color::srgba(1.0, 1.0, 1.0, 0.4)
            }
            Tile::DoorBlue if world_state.is_door_open(crate::types::Color::Blue) => {
                Color::srgba(1.0, 1.0, 1.0, 0.4)
            }
            _ => Color::srgba(1.0, 1.0, 1.0, 1.0), // Full opacity for all other tiles
        };

        commands.spawn((
            Sprite {
                image: texture,
                color,
                custom_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
                ..default()
            },
            Transform::from_xyz(x, y, 0.0),
            MapTile { pos: *pos },
            MapEntity,
        ));
    }

    // Render unexplored frontier for Player 1 (positions that can be reached but haven't been explored yet)
    for pos in &world_state.players[0].unexplored_frontier {
        let x = center_x + (pos.x as f32 * TILE_SIZE);
        let y = center_y - (pos.y as f32 * TILE_SIZE);

        // Draw Player 1 frontier as a semi-transparent cyan square
        commands.spawn((
            Sprite {
                color: Color::srgba(0.0, 1.0, 1.0, 0.3), // Bright cyan, 30% opacity
                custom_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
                ..default()
            },
            Transform::from_xyz(x, y, 0.5), // z=0.5 to be above tiles but below player
            MapEntity,
        ));
    }

    // Render unexplored frontier for Player 2 if available
    if world_state.players.len() > 1 {
        for pos in &world_state.players[1].unexplored_frontier {
            let x = center_x + (pos.x as f32 * TILE_SIZE);
            let y = center_y - (pos.y as f32 * TILE_SIZE);

            // Draw Player 2 frontier as a semi-transparent magenta square
            commands.spawn((
                Sprite {
                    color: Color::srgba(1.0, 0.0, 1.0, 0.3), // Bright magenta, 30% opacity
                    custom_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
                    ..default()
                },
                Transform::from_xyz(x, y, 0.5), // z=0.5 to be above tiles but below player
                MapEntity,
            ));
        }
    }

    // Render colored borders around boulders on pressure plates
    let boulders_on_plates = world_state.get_boulders_on_plates();
    for (color, boulder_positions) in boulders_on_plates.iter() {
        let border_color = match color {
            crate::types::Color::Red => Color::srgb(1.0, 0.0, 0.0),
            crate::types::Color::Green => Color::srgb(0.0, 1.0, 0.0),
            crate::types::Color::Blue => Color::srgb(0.0, 0.5, 1.0),
        };

        for &boulder_pos in boulder_positions {
            let x = center_x + (boulder_pos.x as f32 * TILE_SIZE);
            let y = center_y - (boulder_pos.y as f32 * TILE_SIZE);

            // Draw colored border around boulder on plate
            spawn_tile_border(commands, x, y, border_color, 3.0, 0.55);
        }
    }

    // Render blue borders around unmoved boulders (that are not on plates)
    for boulder_pos in world_state.boulders.get_original_boulders() {
        // Skip if this boulder is on a plate (already rendered with colored border)
        let is_on_plate = boulders_on_plates
            .values()
            .any(|positions| positions.contains(&boulder_pos));
        if is_on_plate {
            continue;
        }

        let x = center_x + (boulder_pos.x as f32 * TILE_SIZE);
        let y = center_y - (boulder_pos.y as f32 * TILE_SIZE);

        // Draw blue border around unmoved boulder
        spawn_tile_border(commands, x, y, Color::srgb(0.0, 0.8, 1.0), 3.0, 0.55);
    }

    // Render current path for Player 1 (if available)
    if let Some(ref path) = world_state.players[0].current_path {
        for pos in path.iter() {
            let x = center_x + (pos.x as f32 * TILE_SIZE);
            let y = center_y - (pos.y as f32 * TILE_SIZE);

            // Draw Player 1 path as yellow outline border
            spawn_tile_border(commands, x, y, Color::srgb(1.0, 1.0, 0.0), 2.0, 0.6);
        }
    }

    // Render current path for Player 2 (if available)
    if world_state.players.len() > 1
        && let Some(ref path) = world_state.players[1].current_path
    {
        for pos in path.iter() {
            let x = center_x + (pos.x as f32 * TILE_SIZE);
            let y = center_y - (pos.y as f32 * TILE_SIZE);

            // Draw Player 2 path as orange outline border
            spawn_tile_border(commands, x, y, Color::srgb(1.0, 0.6, 0.0), 2.0, 0.6);
        }
    }

    // Render current destination for Player 1 (if available)
    if let Some(dest) = world_state.players[0].current_destination {
        let x = center_x + (dest.x as f32 * TILE_SIZE);
        let y = center_y - (dest.y as f32 * TILE_SIZE);

        // Draw Player 1 destination as bright red outline border (thicker)
        spawn_tile_border(commands, x, y, Color::srgb(1.0, 0.0, 0.0), 3.0, 0.7);
    }

    // Render current destination for Player 2 (if available)
    if world_state.players.len() > 1
        && let Some(dest) = world_state.players[1].current_destination
    {
        let x = center_x + (dest.x as f32 * TILE_SIZE);
        let y = center_y - (dest.y as f32 * TILE_SIZE);

        // Draw Player 2 destination as bright pink outline border (thicker)
        spawn_tile_border(commands, x, y, Color::srgb(1.0, 0.0, 0.6), 3.0, 0.7);
    }

    // Render potential enemy locations with purple outline
    for pos in world_state.potential_enemy_locations.iter() {
        let x = center_x + (pos.x as f32 * TILE_SIZE);
        let y = center_y - (pos.y as f32 * TILE_SIZE);

        // Draw potential enemy location as purple outline border
        spawn_tile_border(commands, x, y, Color::srgb(0.7, 0.0, 1.0), 2.0, 0.65);
    }

    // Render the players
    for player in world_state.players.iter() {
        if !player.is_active {
            continue;
        }

        let player_x = center_x + (player.position.x as f32 * TILE_SIZE);
        let player_y = center_y - (player.position.y as f32 * TILE_SIZE);

        commands.spawn((
            Sprite {
                image: tile_assets.player.clone(),
                custom_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
                ..default()
            },
            Transform::from_xyz(player_x, player_y, 1.0),
            MapEntity,
        ));
    }

    // Add text overlay with game info - Line 1
    let p1 = &world_state.players[0];
    let p1_goal = p1
        .current_goal
        .as_ref()
        .map(format_goal)
        .unwrap_or_else(|| "None".to_string());

    let line1_left = format!(
        "Game:{:<4} Level:{:<4} Tick:{:<6}",
        snapshot.game_count, world_state.level, world_state.tick
    );
    let line1_right = format!("P1  HP:{:<3}", p1.health);

    // Level and Tick - left aligned
    commands.spawn((
        Text::new(line1_left),
        TextFont {
            font_size: 18.0,
            font: default(),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        MapEntity,
    ));

    // Player 1 stats - right aligned
    commands.spawn((
        Text::new(line1_right),
        TextFont {
            font_size: 18.0,
            font: default(),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(LOG_PANE_WIDTH + 320.0),
            ..default()
        },
        MapEntity,
    ));

    // Player 1 inventory icons - positioned after HP
    let icon_start_x = WINDOW_WIDTH - LOG_PANE_WIDTH - 310.0;
    let icon_y = 10.0;
    let icon_size = 20.0;
    let icon_spacing = 24.0;
    let mut icon_offset = 0.0;

    if p1.has_sword {
        commands.spawn((
            ImageNode {
                image: tile_assets.sword.clone(),
                ..default()
            },
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(icon_y),
                left: Val::Px(icon_start_x + icon_offset),
                width: Val::Px(icon_size),
                height: Val::Px(icon_size),
                ..default()
            },
            MapEntity,
        ));
        icon_offset += icon_spacing;
    }

    match p1.inventory {
        crate::swoq_interface::Inventory::KeyRed => {
            commands.spawn((
                ImageNode {
                    image: tile_assets.key_red.clone(),
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(icon_y),
                    left: Val::Px(icon_start_x + icon_offset),
                    width: Val::Px(icon_size),
                    height: Val::Px(icon_size),
                    ..default()
                },
                MapEntity,
            ));
        }
        crate::swoq_interface::Inventory::KeyGreen => {
            commands.spawn((
                ImageNode {
                    image: tile_assets.key_green.clone(),
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(icon_y),
                    left: Val::Px(icon_start_x + icon_offset),
                    width: Val::Px(icon_size),
                    height: Val::Px(icon_size),
                    ..default()
                },
                MapEntity,
            ));
        }
        crate::swoq_interface::Inventory::KeyBlue => {
            commands.spawn((
                ImageNode {
                    image: tile_assets.key_blue.clone(),
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(icon_y),
                    left: Val::Px(icon_start_x + icon_offset),
                    width: Val::Px(icon_size),
                    height: Val::Px(icon_size),
                    ..default()
                },
                MapEntity,
            ));
        }
        crate::swoq_interface::Inventory::Boulder => {
            commands.spawn((
                ImageNode {
                    image: tile_assets.boulder.clone(),
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(icon_y),
                    left: Val::Px(icon_start_x + icon_offset),
                    width: Val::Px(icon_size),
                    height: Val::Px(icon_size),
                    ..default()
                },
                MapEntity,
            ));
        }
        crate::swoq_interface::Inventory::Treasure => {
            commands.spawn((
                ImageNode {
                    image: tile_assets.treasure.clone(),
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(icon_y),
                    left: Val::Px(icon_start_x + icon_offset),
                    width: Val::Px(icon_size),
                    height: Val::Px(icon_size),
                    ..default()
                },
                MapEntity,
            ));
        }
        crate::swoq_interface::Inventory::None => {}
    }

    // Player 1 goal text - positioned after inventory icons
    let p1_goal_text = format!("Goal:{:<20}", p1_goal);
    commands.spawn((
        Text::new(p1_goal_text),
        TextFont {
            font_size: 18.0,
            font: default(),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(icon_start_x + icon_spacing * 2.5),
            ..default()
        },
        MapEntity,
    ));

    // Add Player 2 stats if available (right aligned, line 2)
    if world_state.players.len() > 1 {
        let p2 = &world_state.players[1];
        let p2_goal = p2
            .current_goal
            .as_ref()
            .map(format_goal)
            .unwrap_or_else(|| "None".to_string());

        let p2_text = format!("P2  HP:{:<3}", p2.health);

        commands.spawn((
            Text::new(p2_text),
            TextFont {
                font_size: 18.0,
                font: default(),
                ..default()
            },
            TextColor(Color::srgb(0.4, 0.8, 1.0)), // Cyan color for player 2
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(32.0), // Line 2
                right: Val::Px(LOG_PANE_WIDTH + 320.0),
                ..default()
            },
            MapEntity,
        ));

        // Player 2 inventory icons
        let icon_y2 = 32.0;
        let mut icon_offset2 = 0.0;

        if p2.has_sword {
            commands.spawn((
                ImageNode {
                    image: tile_assets.sword.clone(),
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(icon_y2),
                    left: Val::Px(icon_start_x + icon_offset2),
                    width: Val::Px(icon_size),
                    height: Val::Px(icon_size),
                    ..default()
                },
                MapEntity,
            ));
            icon_offset2 += icon_spacing;
        }

        match p2.inventory {
            crate::swoq_interface::Inventory::KeyRed => {
                commands.spawn((
                    ImageNode {
                        image: tile_assets.key_red.clone(),
                        ..default()
                    },
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(icon_y2),
                        left: Val::Px(icon_start_x + icon_offset2),
                        width: Val::Px(icon_size),
                        height: Val::Px(icon_size),
                        ..default()
                    },
                    MapEntity,
                ));
            }
            crate::swoq_interface::Inventory::KeyGreen => {
                commands.spawn((
                    ImageNode {
                        image: tile_assets.key_green.clone(),
                        ..default()
                    },
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(icon_y2),
                        left: Val::Px(icon_start_x + icon_offset2),
                        width: Val::Px(icon_size),
                        height: Val::Px(icon_size),
                        ..default()
                    },
                    MapEntity,
                ));
            }
            crate::swoq_interface::Inventory::KeyBlue => {
                commands.spawn((
                    ImageNode {
                        image: tile_assets.key_blue.clone(),
                        ..default()
                    },
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(icon_y2),
                        left: Val::Px(icon_start_x + icon_offset2),
                        width: Val::Px(icon_size),
                        height: Val::Px(icon_size),
                        ..default()
                    },
                    MapEntity,
                ));
            }
            crate::swoq_interface::Inventory::Boulder => {
                commands.spawn((
                    ImageNode {
                        image: tile_assets.boulder.clone(),
                        ..default()
                    },
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(icon_y2),
                        left: Val::Px(icon_start_x + icon_offset2),
                        width: Val::Px(icon_size),
                        height: Val::Px(icon_size),
                        ..default()
                    },
                    MapEntity,
                ));
            }
            crate::swoq_interface::Inventory::Treasure => {
                commands.spawn((
                    ImageNode {
                        image: tile_assets.treasure.clone(),
                        ..default()
                    },
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(icon_y2),
                        left: Val::Px(icon_start_x + icon_offset2),
                        width: Val::Px(icon_size),
                        height: Val::Px(icon_size),
                        ..default()
                    },
                    MapEntity,
                ));
            }
            crate::swoq_interface::Inventory::None => {}
        }

        // Player 2 goal text
        let p2_goal_text = format!("Goal:{:<20}", p2_goal);
        commands.spawn((
            Text::new(p2_goal_text),
            TextFont {
                font_size: 18.0,
                font: default(),
                ..default()
            },
            TextColor(Color::srgb(0.4, 0.8, 1.0)), // Cyan color for player 2
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(32.0),
                left: Val::Px(icon_start_x + icon_spacing * 2.5),
                ..default()
            },
            MapEntity,
        ));
    }

    // Add loop statistics on line 3 (left aligned) - always show to maintain consistent layout
    let line3_left = format!(
        "Runs - Success:{:<4} Failed:{:<4}",
        snapshot.successful_runs, snapshot.failed_runs
    );

    commands.spawn((
        Text::new(line3_left),
        TextFont {
            font_size: 18.0,
            font: default(),
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.7, 0.7)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(32.0), // Line 3
            left: Val::Px(10.0),
            ..default()
        },
        MapEntity,
    ));
}

fn update_log_pane(
    mut commands: Commands,
    mut game_state: ResMut<GameStateResource>,
    log_pane_query: Query<Entity, With<LogPane>>,
    log_background_query: Query<Entity, With<LogPaneBackground>>,
) {
    // Create log pane background if it doesn't exist (persists across map updates)
    if log_background_query.is_empty() {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(MAP_WIDTH),
                top: Val::Px(0.0),
                width: Val::Px(LOG_PANE_WIDTH),
                height: Val::Px(WINDOW_HEIGHT),
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
            LogPaneBackground,
        ));
        game_state.log_background_created = true;
    }

    // Read all available log messages from channel and add to buffer
    let mut new_messages = Vec::new();
    if let Ok(rx) = game_state.log_rx.lock() {
        while let Ok(msg) = rx.try_recv() {
            new_messages.push(msg);
        }
    } // Lock is released here

    // Only update if we have new messages
    if new_messages.is_empty() {
        return;
    }

    // Add new messages to buffer
    game_state.log_buffer.extend(new_messages);

    // Trim buffer if it gets too large
    if game_state.log_buffer.len() > MAX_LOG_ENTRIES * 2 {
        game_state.log_buffer.drain(0..MAX_LOG_ENTRIES);
    }

    // Clear existing log pane to rebuild with updated messages
    for entity in log_pane_query.iter() {
        commands.entity(entity).despawn();
    }

    // Get last MAX_LOG_ENTRIES from buffer
    let start_idx = if game_state.log_buffer.len() > MAX_LOG_ENTRIES {
        game_state.log_buffer.len() - MAX_LOG_ENTRIES
    } else {
        0
    };
    let log_messages: Vec<LogMessage> = game_state.log_buffer[start_idx..].to_vec();

    // Create log pane container
    let container = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(MAP_WIDTH + 10.0),
                top: Val::Px(10.0),
                width: Val::Px(LOG_PANE_WIDTH - 20.0),
                height: Val::Px(WINDOW_HEIGHT - 20.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            LogPane,
        ))
        .id();

    // Add log entries as children
    for log_message in log_messages.iter().rev() {
        let text_entity = commands
            .spawn((
                Text::new(log_message.text.clone()),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(log_message.color.to_bevy_color()),
                Node {
                    margin: UiRect::bottom(Val::Px(2.0)),
                    max_width: Val::Px(LOG_PANE_WIDTH - 30.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
            ))
            .id();

        commands.entity(container).add_child(text_entity);
    }
}

fn format_goal(goal: &crate::goals::Goal) -> String {
    use crate::goals::Goal;
    match goal {
        Goal::Explore => "Explore".to_string(),
        Goal::GetKey(color) => format!("GetKey({:?})", color),
        Goal::OpenDoor(color) => format!("OpenDoor({:?})", color),
        Goal::WaitOnTile(color, _pos) => format!("WaitOnTile({:?})", color),
        Goal::PassThroughDoor(color, _door_pos, _target_pos) => format!("PassDoor({:?})", color),
        Goal::PickupSword => "PickupSword".to_string(),
        Goal::PickupHealth(_pos) => "PickupHealth".to_string(),
        Goal::AvoidEnemy(_pos) => "AvoidEnemy".to_string(),
        Goal::KillEnemy(_pos) => "KillEnemy".to_string(),
        Goal::FetchBoulder(_pos) => "FetchBoulder".to_string(),
        Goal::DropBoulder => "DropBoulder".to_string(),
        Goal::DropBoulderOnPlate(color, _pos) => format!("DropOnPlate({:?})", color),
        Goal::ReachExit => "ReachExit".to_string(),
        Goal::RandomExplore(_pos) => "RandomExplore".to_string(),
    }
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
