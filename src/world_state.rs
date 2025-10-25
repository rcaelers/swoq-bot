use std::collections::HashMap;

use tracing::{debug, warn};

use crate::boulder_tracker::BoulderTracker;
use crate::item_tracker::{ColoredItemTracker, ItemTracker};
use crate::map::Map;
use crate::player_state::PlayerState;
use crate::swoq_interface::{Inventory, State, Tile};
use crate::types::{Bounds, Color, Position};

struct SurroundingsData<'a> {
    surroundings: &'a [i32],
    center: Position,
    bounds: Bounds,
}

#[derive(Debug, Default)]
struct SeenItems {
    keys: HashMap<Color, Vec<Position>>,
    doors: HashMap<Color, Vec<Position>>,
    pressure_plates: HashMap<Color, Vec<Position>>,
    boulders: Vec<Position>,
    swords: Vec<Position>,
    health: Vec<Position>,
    enemies: Vec<Position>,
}

#[derive(Clone)]
pub struct WorldState {
    pub level: i32,
    pub tick: i32,
    pub visibility_range: i32,

    // Map
    pub map: Map,

    // Player states (1 or 2 players)
    pub players: Vec<PlayerState>,

    // Tracked positions
    pub keys: ColoredItemTracker,
    pub doors: ColoredItemTracker,
    pub enemies: ItemTracker,
    pub boulders: BoulderTracker,
    pub swords: ItemTracker,
    pub health: ItemTracker,
    pub pressure_plates: ColoredItemTracker,
    pub exit_position: Option<Position>,
    pub boss_position: Option<Position>,
    pub treasure_position: Option<Position>,
}

impl WorldState {
    pub fn new(map_width: i32, map_height: i32, visibility_range: i32) -> Self {
        Self {
            level: 0,
            tick: 0,
            visibility_range,
            map: Map::new(map_width, map_height),
            players: vec![PlayerState::new(Position::new(0, 0))],
            exit_position: None,
            keys: ColoredItemTracker::new(),
            doors: ColoredItemTracker::new(),
            enemies: ItemTracker::new(),
            boulders: BoulderTracker::new(),
            swords: ItemTracker::new(),
            health: ItemTracker::new(),
            pressure_plates: ColoredItemTracker::new(),
            boss_position: None,
            treasure_position: None,
        }
    }

    /// Get the first player (the bot's controlled player)
    pub fn player(&self) -> &PlayerState {
        &self.players[0]
    }

    /// Get mutable reference to the first player (the bot's controlled player)
    pub fn player_mut(&mut self) -> &mut PlayerState {
        &mut self.players[0]
    }

    #[tracing::instrument(level = "trace", skip(self, state))]
    pub fn update(&mut self, state: &State) {
        self.level = state.level;
        self.tick = state.tick;

        let mut all_surroundings = Vec::new();

        // Update player 1
        if let Some(player_state) = &state.player_state
            && let Some(p1) = self.players.get_mut(0)
        {
            Self::update_player_state_fields(p1, player_state);
            let bounds = Bounds::from_center_and_range(p1.position, self.visibility_range);
            all_surroundings.push(SurroundingsData {
                surroundings: &player_state.surroundings,
                center: p1.position,
                bounds,
            });
        }

        // Update player 2 (level 12+)
        if let Some(player2_state) = &state.player2_state {
            if self.players.len() == 1 {
                self.players.push(PlayerState::new(Position::new(0, 0)));
            }
            if let Some(p2) = self.players.get_mut(1) {
                Self::update_player_state_fields(p2, player2_state);
                let bounds = Bounds::from_center_and_range(p2.position, self.visibility_range);
                all_surroundings.push(SurroundingsData {
                    surroundings: &player2_state.surroundings,
                    center: p2.position,
                    bounds,
                });
            }
        }

        self.integrate_surroundings(all_surroundings);
        for player in self.players.iter_mut() {
            player.update_frontier(&self.map);
        }
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub fn reset_for_new_level(&mut self) {
        self.map.clear();
        self.exit_position = None;
        self.keys.clear();
        self.doors.clear();
        self.enemies.clear();
        self.boulders.clear();
        self.swords.clear();
        self.health.clear();
        self.pressure_plates.clear();
        self.boss_position = None;
        self.treasure_position = None;

        // Clear all players and reset to single player
        self.players.truncate(1);
        if let Some(p1) = self.players.get_mut(0) {
            p1.clear();
        }
    }

    fn update_player_state_fields(
        player: &mut PlayerState,
        player_state: &crate::swoq_interface::PlayerState,
    ) {
        if let Some(position) = &player_state.position {
            player.position = Position::new(position.x, position.y);
        }
        player.health = player_state.health.unwrap_or(10);
        player.has_sword = player_state.has_sword.unwrap_or(false);
        player.inventory = player_state
            .inventory
            .and_then(|i| Inventory::try_from(i).ok())
            .unwrap_or(Inventory::None);
    }

    #[tracing::instrument(level = "trace", skip(self, all_surroundings))]
    fn integrate_surroundings(&mut self, all_surroundings: Vec<SurroundingsData>) {
        if all_surroundings.is_empty() {
            return;
        }

        // Remove Unknown tiles that are now outside all players' visibility ranges
        // They should revert to unseen (None/?) since we have no current information
        let combined_bounds: Vec<Bounds> = all_surroundings.iter().map(|s| s.bounds).collect();
        self.map.retain(|pos, tile| {
            if *tile == Tile::Unknown {
                // Keep Unknown tiles only if they're in any player's current visibility range
                combined_bounds.iter().any(|b| b.contains(pos))
            } else {
                // Keep all other tiles (permanent ones should persist)
                true
            }
        });

        // Track which permanent items we can currently see across all surroundings
        let mut seen_items = SeenItems::default();

        // Process each set of surroundings and collect items
        for data in &all_surroundings {
            self.process_surroundings(data, &mut seen_items);
        }

        // Update item trackers with collected items
        // Pass all bounds so items are validated if visible to ANY player
        self.update_item_trackers(&combined_bounds, seen_items);
    }

    fn process_surroundings(&mut self, data: &SurroundingsData, seen_items: &mut SeenItems) {
        let size = (self.visibility_range * 2 + 1) as usize;
        for (idx, &tile_val) in data.surroundings.iter().enumerate() {
            let row = (idx / size) as i32;
            let col = (idx % size) as i32;

            let tile_position = Position::new(
                data.center.x + col - self.visibility_range,
                data.center.y + row - self.visibility_range,
            );

            // Skip out-of-bounds
            if tile_position.x < 0
                || tile_position.x >= self.map.width
                || tile_position.y < 0
                || tile_position.y >= self.map.height
            {
                continue;
            }

            if let Ok(tile) = Tile::try_from(tile_val) {
                // Don't overwrite known permanent tiles with Unknown (fog of war)
                // Only update if it's new info or if we're updating a temporary tile
                let should_update = match (self.map.get(&tile_position), tile) {
                    // Don't replace walls with Unknown
                    (Some(Tile::Wall), Tile::Unknown) => false,
                    // Don't replace Empty with Unknown - we know it's empty
                    (Some(Tile::Empty), Tile::Unknown) => false,
                    // Update Unknown with Unknown (refresh fog of war)
                    (Some(Tile::Unknown), Tile::Unknown) => true,
                    // Update Player position with Unknown (player has moved)
                    (Some(Tile::Player), Tile::Unknown) => true,
                    // Don't replace any other known tile with Unknown
                    (Some(_), Tile::Unknown) => false,
                    // Always update with concrete information
                    _ => true,
                };

                // Check for dropped boulder BEFORE updating the map
                if tile == Tile::Boulder {
                    self.check_dropped_boulder(tile_position);
                }

                if should_update {
                    self.map.insert(tile_position, tile);
                }

                // Track special global tiles (exit, boss, treasure) and items
                if tile != Tile::Unknown {
                    match tile {
                        Tile::Exit => self.exit_position = Some(tile_position),
                        Tile::Boss => self.boss_position = Some(tile_position),
                        Tile::Treasure => self.treasure_position = Some(tile_position),
                        Tile::KeyRed => {
                            seen_items
                                .keys
                                .entry(Color::Red)
                                .or_default()
                                .push(tile_position);
                        }
                        Tile::KeyGreen => {
                            seen_items
                                .keys
                                .entry(Color::Green)
                                .or_default()
                                .push(tile_position);
                        }
                        Tile::KeyBlue => {
                            seen_items
                                .keys
                                .entry(Color::Blue)
                                .or_default()
                                .push(tile_position);
                        }
                        Tile::DoorRed => {
                            seen_items
                                .doors
                                .entry(Color::Red)
                                .or_default()
                                .push(tile_position);
                        }
                        Tile::DoorGreen => {
                            seen_items
                                .doors
                                .entry(Color::Green)
                                .or_default()
                                .push(tile_position);
                        }
                        Tile::DoorBlue => {
                            seen_items
                                .doors
                                .entry(Color::Blue)
                                .or_default()
                                .push(tile_position);
                        }
                        Tile::Enemy => seen_items.enemies.push(tile_position),
                        Tile::Boulder => seen_items.boulders.push(tile_position),
                        Tile::Sword => seen_items.swords.push(tile_position),
                        Tile::Health => seen_items.health.push(tile_position),
                        Tile::PressurePlateRed => {
                            seen_items
                                .pressure_plates
                                .entry(Color::Red)
                                .or_default()
                                .push(tile_position);
                        }
                        Tile::PressurePlateGreen => {
                            seen_items
                                .pressure_plates
                                .entry(Color::Green)
                                .or_default()
                                .push(tile_position);
                        }
                        Tile::PressurePlateBlue => {
                            seen_items
                                .pressure_plates
                                .entry(Color::Blue)
                                .or_default()
                                .push(tile_position);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn update_item_trackers(&mut self, all_bounds: &[Bounds], seen_items: SeenItems) {
        // Update key positions using ColoredItemTracker
        self.keys.update(
            seen_items.keys,
            &self.map,
            |tile| matches!(tile, Tile::KeyRed | Tile::KeyGreen | Tile::KeyBlue),
            all_bounds,
        );

        // Update door positions using ColoredItemTracker
        self.doors.update(
            seen_items.doors,
            &self.map,
            |tile| matches!(tile, Tile::DoorRed | Tile::DoorGreen | Tile::DoorBlue),
            all_bounds,
        );

        // Update pressure plates using ColoredItemTracker
        self.pressure_plates.update(
            seen_items.pressure_plates,
            &self.map,
            |tile| {
                matches!(
                    tile,
                    Tile::PressurePlateRed | Tile::PressurePlateGreen | Tile::PressurePlateBlue
                )
            },
            all_bounds,
        );

        // Update boulder positions
        self.boulders.update(seen_items.boulders, &self.map, |pos| {
            self.players.iter().any(|p| p.position.is_adjacent(pos))
        });

        // Update sword positions using ItemTracker
        self.swords.update(
            seen_items.swords,
            &self.map,
            |tile| matches!(tile, Tile::Sword),
            all_bounds,
        );

        // Update health positions using ItemTracker
        self.health.update(
            seen_items.health,
            &self.map,
            |tile| matches!(tile, Tile::Health),
            all_bounds,
        );

        // Update enemy positions using ItemTracker
        self.enemies.update(
            seen_items.enemies,
            &self.map,
            |tile| matches!(tile, Tile::Enemy),
            all_bounds,
        );
    }

    #[tracing::instrument(level = "trace", skip(self), fields(pos_x = pos.x, pos_y = pos.y))]
    fn check_dropped_boulder(&mut self, pos: Position) {
        // Check for dropped boulder BEFORE updating the map
        // If we see a boulder in a location that was empty and adjacent to us, we dropped it
        // Note: if old_tile is None (never seen), the boulder is unexplored (not moved)
        if self.players.iter().any(|p| p.position.is_adjacent(&pos)) {
            let has_moved = match self.map.get(&pos) {
                Some(Tile::Empty)
                | Some(Tile::Player)
                | Some(
                    Tile::PressurePlateRed | Tile::PressurePlateGreen | Tile::PressurePlateBlue,
                ) => {
                    debug!("Boulder at {:?} is moved (we dropped it)", pos);
                    true // Boulder was dropped by us - has moved
                }
                None => {
                    // Boulder in a never-seen location - it's unexplored (not moved)
                    false
                }
                _ => {
                    // Boulder replacing something else - likely not a drop, assume not moved
                    false
                }
            };

            // Add or update boulder in our tracking
            if !self.boulders.contains(&pos) {
                self.boulders.add_boulder(pos, has_moved);
            }
        }
    }

    /// Compute which pressure plates currently have boulders on them
    pub fn get_boulders_on_plates(&self) -> HashMap<Color, Vec<Position>> {
        let mut result: HashMap<Color, Vec<Position>> = HashMap::new();

        // Check each color's pressure plates to see if a boulder is on any of them
        for &color in &[Color::Red, Color::Green, Color::Blue] {
            if let Some(plate_positions) = self.pressure_plates.get_positions(color) {
                for &plate_pos in plate_positions {
                    // Check if there's a boulder at this plate position
                    if let Some(tile) = self.map.get(&plate_pos)
                        && matches!(tile, Tile::Boulder)
                        && self.boulders.contains(&plate_pos)
                    {
                        result.entry(color).or_default().push(plate_pos);
                    }
                }
            }
        }

        result
    }

    pub fn has_key(&self, player: &PlayerState, color: Color) -> bool {
        // Check if we have the actual key in inventory
        let has_physical_key = matches!(
            (player.inventory, color),
            (Inventory::KeyRed, Color::Red)
                | (Inventory::KeyGreen, Color::Green)
                | (Inventory::KeyBlue, Color::Blue)
        );

        // Check if there's a boulder on a pressure plate of this color
        let boulders_on_plates = self.get_boulders_on_plates();
        let has_boulder_on_plate = boulders_on_plates
            .get(&color)
            .is_some_and(|plates| !plates.is_empty());

        has_physical_key || has_boulder_on_plate
    }

    pub fn closest_enemy(&self, player: &PlayerState) -> Option<Position> {
        self.enemies.closest_to(player.position)
    }

    pub fn closest_health(&self, player: &PlayerState) -> Option<Position> {
        self.health.closest_to(player.position)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    pub fn doors_without_keys(&self, player: &PlayerState) -> Vec<Color> {
        self.doors
            .colors()
            .filter(|color| !self.has_key(player, **color))
            .copied()
            .collect()
    }

    #[allow(dead_code)]
    pub fn closest_door_of_color(&self, player: &PlayerState, color: Color) -> Option<Position> {
        self.doors.closest_to(color, player.position)
    }

    pub fn knows_key_location(&self, color: Color) -> bool {
        self.keys.has_color(color)
    }

    pub fn closest_key(&self, player: &PlayerState, color: Color) -> Option<Position> {
        self.keys.closest_to(color, player.position)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    pub fn draw_ascii_map(&self) -> String {
        let mut output = String::new();

        // ANSI color codes
        const RESET: &str = "\x1b[0m";
        const PLAYER: &str = "\x1b[1;33m"; // Bright yellow
        const PLAYER2: &str = "\x1b[1;36m"; // Bright cyan
        const WALL: &str = "\x1b[90m"; // Dark gray
        const EXIT: &str = "\x1b[1;32m"; // Bright green
        const KEY_RED: &str = "\x1b[91m"; // Bright red
        const KEY_GREEN: &str = "\x1b[92m"; // Bright green
        const KEY_BLUE: &str = "\x1b[94m"; // Bright blue
        const DOOR_RED: &str = "\x1b[31m"; // Red
        const DOOR_GREEN: &str = "\x1b[32m"; // Green
        const DOOR_BLUE: &str = "\x1b[34m"; // Blue
        const ENEMY: &str = "\x1b[1;31m"; // Bright red
        const BOULDER_EXPLORED: &str = "\x1b[33m"; // Yellow (explored/dropped)
        const SWORD: &str = "\x1b[1;37m"; // Bright white
        const HEALTH: &str = "\x1b[1;35m"; // Bright magenta
        const BOSS: &str = "\x1b[1;31m"; // Bright red
        const TREASURE: &str = "\x1b[1;33m"; // Bright yellow
        const FRONTIER: &str = "\x1b[96m"; // Bright cyan
        const UNKNOWN: &str = "\x1b[90m"; // Dark gray

        for y in 0..self.map.height {
            for x in 0..self.map.width {
                let pos = Position::new(x, y);

                // Check if this is the player position
                if self.players.first().is_some_and(|p| pos == p.position) {
                    output.push_str(&format!("{}1{}", PLAYER, RESET));
                } else if self.players.get(1).is_some_and(|p| pos == p.position) {
                    output.push_str(&format!("{}2{}", PLAYER2, RESET));
                } else if self
                    .players
                    .first()
                    .is_some_and(|p| p.unexplored_frontier.contains(&pos))
                {
                    // Show unexplored frontier
                    output.push_str(&format!("{}░{}", FRONTIER, RESET));
                } else {
                    // Render the tile
                    let tile_str = match self.map.get(&pos) {
                        Some(Tile::Unknown) => format!("{}·{}", UNKNOWN, RESET),
                        Some(Tile::Wall) => format!("{}█{}", WALL, RESET),
                        Some(Tile::Empty) => " ".to_string(),
                        Some(Tile::Exit) => format!("{}E{}", EXIT, RESET),
                        Some(Tile::KeyRed) => format!("{}r{}", KEY_RED, RESET),
                        Some(Tile::KeyGreen) => format!("{}g{}", KEY_GREEN, RESET),
                        Some(Tile::KeyBlue) => format!("{}b{}", KEY_BLUE, RESET),
                        Some(Tile::DoorRed) => format!("{}R{}", DOOR_RED, RESET),
                        Some(Tile::DoorGreen) => format!("{}G{}", DOOR_GREEN, RESET),
                        Some(Tile::DoorBlue) => format!("{}B{}", DOOR_BLUE, RESET),
                        Some(Tile::Enemy) => format!("{}e{}", ENEMY, RESET),
                        Some(Tile::Boulder) => {
                            // Show moved boulders in explored color, unmoved boulders in frontier color
                            if self.boulders.has_moved(&pos) {
                                format!("{}o{}", BOULDER_EXPLORED, RESET)
                            } else {
                                format!("{}O{}", FRONTIER, RESET)
                            }
                        }
                        Some(Tile::Sword) => format!("{}s{}", SWORD, RESET),
                        Some(Tile::Health) => format!("{}+{}", HEALTH, RESET),
                        Some(Tile::PressurePlateRed) => format!("{}▫{}", KEY_RED, RESET),
                        Some(Tile::PressurePlateGreen) => format!("{}▪{}", KEY_GREEN, RESET),
                        Some(Tile::PressurePlateBlue) => format!("{}◦{}", KEY_BLUE, RESET),
                        Some(Tile::Boss) => format!("{}X{}", BOSS, RESET),
                        Some(Tile::Treasure) => format!("{}${}", TREASURE, RESET),
                        Some(Tile::Player) => " ".to_string(),
                        None => "?".to_string(), // Never received any data (unexplored)
                    };
                    output.push_str(&tile_str);
                }
            }
            output.push('\n');
        }

        output
    }
}
