use crate::boulder_tracker::BoulderTracker;
use crate::item_tracker::{ColoredItemTracker, ItemTracker};
use crate::swoq_interface::{Inventory, State, Tile};
use std::collections::{HashMap, HashSet};
use tracing::{debug, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn distance(&self, other: &Pos) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }

    pub fn neighbors(&self) -> [Pos; 4] {
        [
            Pos::new(self.x, self.y - 1), // North
            Pos::new(self.x + 1, self.y), // East
            Pos::new(self.x, self.y + 1), // South
            Pos::new(self.x - 1, self.y), // West
        ]
    }

    pub fn is_adjacent(&self, other: &Pos) -> bool {
        self.distance(other) == 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
}

impl Bounds {
    pub fn new(min_x: i32, max_x: i32, min_y: i32, max_y: i32) -> Self {
        Self {
            min_x,
            max_x,
            min_y,
            max_y,
        }
    }

    pub fn contains(&self, pos: &Pos) -> bool {
        pos.x >= self.min_x && pos.x <= self.max_x && pos.y >= self.min_y && pos.y <= self.max_y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    Red,
    Green,
    Blue,
}

#[derive(Clone)]
pub struct WorldState {
    pub level: i32,
    pub tick: i32,
    pub map_width: i32,
    pub map_height: i32,
    pub visibility_range: i32,

    // Map tiles
    pub map: HashMap<Pos, Tile>,

    // Player 1 state
    pub player_pos: Pos,
    pub player_health: i32,
    pub player_inventory: Inventory,
    pub player_has_sword: bool,

    // Player 2 state (level 12+)
    pub player2_pos: Option<Pos>,
    pub player2_health: Option<i32>,
    pub player2_inventory: Option<Inventory>,
    pub player2_has_sword: Option<bool>,

    // Tracked positions
    pub keys: ColoredItemTracker,
    pub doors: ColoredItemTracker,
    pub enemies: ItemTracker,
    pub boulder_info: BoulderTracker,
    pub swords: ItemTracker,
    pub health: ItemTracker,
    pub pressure_plates: ColoredItemTracker,
    pub exit_pos: Option<Pos>,
    pub boss_position: Option<Pos>,
    pub treasure_position: Option<Pos>,

    pub unexplored_frontier: HashSet<Pos>,

    // Planning state to avoid oscillation
    pub previous_goal: Option<crate::goal::Goal>,
    pub current_destination: Option<Pos>,
    pub current_path: Option<Vec<Pos>>,
}

impl WorldState {
    pub fn new(map_width: i32, map_height: i32, visibility_range: i32) -> Self {
        Self {
            level: 0,
            tick: 0,
            map_width,
            map_height,
            visibility_range,
            map: HashMap::new(),
            player_pos: Pos::new(0, 0),
            player_health: 10,
            player_inventory: Inventory::None,
            player_has_sword: false,
            player2_pos: None,
            player2_health: None,
            player2_inventory: None,
            player2_has_sword: None,
            exit_pos: None,
            keys: ColoredItemTracker::new(),
            doors: ColoredItemTracker::new(),
            enemies: ItemTracker::new(),
            boulder_info: BoulderTracker::new(),
            swords: ItemTracker::new(),
            health: ItemTracker::new(),
            pressure_plates: ColoredItemTracker::new(),
            boss_position: None,
            treasure_position: None,
            unexplored_frontier: HashSet::new(),
            previous_goal: None,
            current_destination: None,
            current_path: None,
        }
    }

    #[tracing::instrument(level = "trace", skip(self, state))]
    pub fn update(&mut self, state: &State) {
        self.level = state.level;
        self.tick = state.tick;

        // Update player 1
        if let Some(player_state) = &state.player_state {
            if let Some(position) = &player_state.position {
                self.player_pos = Pos::new(position.x, position.y);
            }
            self.player_health = player_state.health.unwrap_or(10);
            self.player_has_sword = player_state.has_sword.unwrap_or(false);
            self.player_inventory = player_state
                .inventory
                .and_then(|i| Inventory::try_from(i).ok())
                .unwrap_or(Inventory::None);

            // Update map from surroundings
            self.integrate_surroundings(&player_state.surroundings, self.player_pos);
        }

        // Update player 2 (level 12+)
        if let Some(player2_state) = &state.player2_state {
            if let Some(position) = &player2_state.position {
                self.player2_pos = Some(Pos::new(position.x, position.y));
            }
            self.player2_health = player2_state.health;
            self.player2_has_sword = player2_state.has_sword;
            self.player2_inventory = player2_state
                .inventory
                .and_then(|i| Inventory::try_from(i).ok());
        }

        self.update_frontier();
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub fn reset_for_new_level(&mut self) {
        // Clear all map data for the new level
        self.map.clear();
        self.exit_pos = None;
        self.keys.clear();
        self.doors.clear();
        self.enemies.clear();
        self.boulder_info.clear();
        self.swords.clear();
        self.health.clear();
        self.pressure_plates.clear();
        self.boss_position = None;
        self.treasure_position = None;
        self.unexplored_frontier.clear();

        // Reset player state will be updated from the new state
        self.player_inventory = Inventory::None;
        self.player_has_sword = false;
        self.player2_pos = None;
        self.player2_health = None;
        self.player2_inventory = None;
        self.player2_has_sword = None;

        // Clear planning state
        self.previous_goal = None;
        self.current_destination = None;
        self.current_path = None;
    }

    #[tracing::instrument(level = "trace", skip(self, surroundings), fields(center_x = center.x, center_y = center.y, surroundings_len = surroundings.len()))]
    fn integrate_surroundings(&mut self, surroundings: &[i32], center: Pos) {
        let size = (self.visibility_range * 2 + 1) as usize;

        // Calculate visibility bounds
        let bounds = Bounds::new(
            center.x - self.visibility_range,
            center.x + self.visibility_range,
            center.y - self.visibility_range,
            center.y + self.visibility_range,
        );

        // Remove Unknown tiles that are now outside our visibility range
        // They should revert to unseen (None/?) since we have no current information
        self.map.retain(|pos, tile| {
            if *tile == Tile::Unknown {
                // Keep Unknown tiles only if they're in our current visibility range
                bounds.contains(pos)
            } else {
                // Keep all other tiles (permanent ones should persist)
                true
            }
        });

        // Track which permanent items we can currently see
        let mut seen_keys: HashMap<Color, Vec<Pos>> = HashMap::new();
        let mut seen_doors: HashMap<Color, Vec<Pos>> = HashMap::new();
        let mut seen_pressure_plates: HashMap<Color, Vec<Pos>> = HashMap::new();
        let mut seen_boulders: Vec<Pos> = Vec::new();
        let mut seen_swords: Vec<Pos> = Vec::new();
        let mut seen_health: Vec<Pos> = Vec::new();
        let mut seen_enemies: Vec<Pos> = Vec::new();

        for (idx, &tile_val) in surroundings.iter().enumerate() {
            let row = (idx / size) as i32;
            let col = (idx % size) as i32;

            let pos = Pos::new(
                center.x + col - self.visibility_range,
                center.y + row - self.visibility_range,
            );

            // Skip out-of-bounds
            if pos.x < 0 || pos.x >= self.map_width || pos.y < 0 || pos.y >= self.map_height {
                continue;
            }

            if let Ok(tile) = Tile::try_from(tile_val) {
                // Don't overwrite known permanent tiles with Unknown (fog of war)
                // Only update if it's new info or if we're updating a temporary tile
                let should_update = match (self.map.get(&pos), tile) {
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
                    self.check_dropped_boulder(pos);
                }

                if should_update {
                    self.map.insert(pos, tile);
                }

                // Track special tiles we can see (only if not Unknown)
                if tile != Tile::Unknown {
                    match tile {
                        Tile::Exit => self.exit_pos = Some(pos),
                        Tile::KeyRed => {
                            seen_keys.entry(Color::Red).or_default().push(pos);
                        }
                        Tile::KeyGreen => {
                            seen_keys.entry(Color::Green).or_default().push(pos);
                        }
                        Tile::KeyBlue => {
                            seen_keys.entry(Color::Blue).or_default().push(pos);
                        }
                        Tile::DoorRed => {
                            seen_doors.entry(Color::Red).or_default().push(pos);
                        }
                        Tile::DoorGreen => {
                            seen_doors.entry(Color::Green).or_default().push(pos);
                        }
                        Tile::DoorBlue => {
                            seen_doors.entry(Color::Blue).or_default().push(pos);
                        }
                        Tile::Enemy => seen_enemies.push(pos),
                        Tile::Boulder => seen_boulders.push(pos),
                        Tile::Sword => seen_swords.push(pos),
                        Tile::Health => seen_health.push(pos),
                        Tile::PressurePlateRed => {
                            seen_pressure_plates
                                .entry(Color::Red)
                                .or_default()
                                .push(pos);
                        }
                        Tile::PressurePlateGreen => {
                            seen_pressure_plates
                                .entry(Color::Green)
                                .or_default()
                                .push(pos);
                        }
                        Tile::PressurePlateBlue => {
                            seen_pressure_plates
                                .entry(Color::Blue)
                                .or_default()
                                .push(pos);
                        }
                        Tile::Boss => self.boss_position = Some(pos),
                        Tile::Treasure => self.treasure_position = Some(pos),
                        _ => {}
                    }
                }
            }
        }

        // Update key positions using ColoredItemTracker
        self.keys.update(
            seen_keys,
            &self.map,
            |tile| matches!(tile, Tile::KeyRed | Tile::KeyGreen | Tile::KeyBlue),
            &bounds,
        );

        // Update door positions using ColoredItemTracker
        self.doors.update(
            seen_doors,
            &self.map,
            |tile| matches!(tile, Tile::DoorRed | Tile::DoorGreen | Tile::DoorBlue),
            &bounds,
        );

        // Update pressure plates using ColoredItemTracker
        self.pressure_plates.update(
            seen_pressure_plates,
            &self.map,
            |tile| {
                matches!(
                    tile,
                    Tile::PressurePlateRed | Tile::PressurePlateGreen | Tile::PressurePlateBlue
                )
            },
            &bounds,
        );

        // Update boulder positions
        self.boulder_info
            .update(seen_boulders, &self.map, |pos| self.player_pos.is_adjacent(pos));

        // Update sword positions using ItemTracker
        self.swords
            .update(seen_swords, &self.map, |tile| matches!(tile, Tile::Sword), &bounds);

        // Update health positions using ItemTracker
        self.health
            .update(seen_health, &self.map, |tile| matches!(tile, Tile::Health), &bounds);

        // Update enemy positions using ItemTracker
        self.enemies
            .update(seen_enemies, &self.map, |tile| matches!(tile, Tile::Enemy), &bounds);
    }

    #[tracing::instrument(level = "trace", skip(self))]
    fn update_frontier(&mut self) {
        // Compute reachable frontier positions in a single pass
        // This replaces the old two-step process of:
        // 1. Finding all candidates (neighbors of known tiles that are Unknown/None)
        // 2. Filtering by reachability
        self.unexplored_frontier =
            crate::pathfinding::AStar::compute_reachable_positions(self, self.player_pos);

        tracing::trace!(frontier_size = self.unexplored_frontier.len(), "Frontier updated");
    }

    #[tracing::instrument(level = "trace", skip(self), fields(pos_x = pos.x, pos_y = pos.y))]
    fn check_dropped_boulder(&mut self, pos: Pos) {
        // Check for dropped boulder BEFORE updating the map
        // If we see a boulder in a location that was empty and adjacent to us, we dropped it
        // Note: if old_tile is None (never seen), the boulder is unexplored (not moved)
        if self.player_pos.is_adjacent(&pos) {
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
            if !self.boulder_info.contains(&pos) {
                self.boulder_info.add_boulder(pos, has_moved);
            }
        }
    }

    pub fn is_walkable(&self, pos: &Pos, can_open_doors: bool, avoid_keys: bool) -> bool {
        match self.map.get(pos) {
            Some(
                Tile::Empty
                | Tile::Exit
                | Tile::Player
                | Tile::Sword
                | Tile::Health
                | Tile::PressurePlateRed
                | Tile::PressurePlateGreen
                | Tile::PressurePlateBlue
                | Tile::Treasure
                | Tile::Unknown, // Fog of war - assume walkable
            ) => true,
            // Keys: avoid during exploration unless specifically going to get one
            Some(Tile::KeyRed | Tile::KeyGreen | Tile::KeyBlue) => {
                if avoid_keys {
                    false
                } else {
                    self.player_inventory == Inventory::None
                }
            }
            Some(Tile::DoorRed) => can_open_doors && self.has_key(Color::Red),
            Some(Tile::DoorGreen) => can_open_doors && self.has_key(Color::Green),
            Some(Tile::DoorBlue) => can_open_doors && self.has_key(Color::Blue),
            None => true, // Never seen tiles - assume walkable
            _ => false,
        }
    }

    pub fn is_walkable_with_goal(&self, pos: &Pos, can_open_doors: bool, goal: Pos) -> bool {
        match self.map.get(pos) {
            Some(
                Tile::Empty
                | Tile::Exit
                | Tile::Player
                | Tile::Sword
                | Tile::Health
                | Tile::PressurePlateRed
                | Tile::PressurePlateGreen
                | Tile::PressurePlateBlue
                | Tile::Treasure
                | Tile::Unknown, // Fog of war - assume walkable
            ) => true,
            // Keys: always avoid unless it's the destination
            Some(Tile::KeyRed | Tile::KeyGreen | Tile::KeyBlue) => {
                // Allow walking on the destination key, avoid all others
                *pos == goal
            }
            Some(Tile::DoorRed) => can_open_doors && self.has_key(Color::Red),
            Some(Tile::DoorGreen) => can_open_doors && self.has_key(Color::Green),
            Some(Tile::DoorBlue) => can_open_doors && self.has_key(Color::Blue),
            None => true, // Never seen tiles - assume walkable
            _ => false,
        }
    }

    /// Compute which pressure plates currently have boulders on them
    fn get_boulders_on_plates(&self) -> HashMap<Color, Vec<Pos>> {
        let mut result: HashMap<Color, Vec<Pos>> = HashMap::new();

        // Check each color's pressure plates to see if a boulder is on any of them
        for &color in &[Color::Red, Color::Green, Color::Blue] {
            if let Some(plate_positions) = self.pressure_plates.get_positions(color) {
                for &plate_pos in plate_positions {
                    // Check if there's a boulder at this plate position
                    if let Some(tile) = self.map.get(&plate_pos)
                        && matches!(tile, Tile::Boulder)
                        && self.boulder_info.contains(&plate_pos)
                    {
                        result.entry(color).or_default().push(plate_pos);
                    }
                }
            }
        }

        result
    }

    pub fn has_key(&self, color: Color) -> bool {
        // Check if we have the actual key in inventory
        let has_physical_key = matches!(
            (self.player_inventory, color),
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

    #[tracing::instrument(level = "trace", skip(self))]
    pub fn sorted_unexplored(&self) -> Vec<Pos> {
        let mut frontier: Vec<Pos> = self.unexplored_frontier.iter().copied().collect();
        frontier.sort_by_key(|pos| self.player_pos.distance(pos));
        frontier
    }

    pub fn closest_enemy(&self) -> Option<Pos> {
        self.enemies.closest_to(self.player_pos)
    }

    pub fn closest_health(&self) -> Option<Pos> {
        self.health.closest_to(self.player_pos)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    pub fn doors_without_keys(&self) -> Vec<Color> {
        self.doors
            .colors()
            .filter(|color| !self.has_key(**color))
            .copied()
            .collect()
    }

    pub fn closest_door_of_color(&self, color: Color) -> Option<Pos> {
        self.doors.closest_to(color, self.player_pos)
    }

    pub fn knows_key_location(&self, color: Color) -> bool {
        self.keys.has_color(color)
    }

    pub fn closest_key(&self, color: Color) -> Option<Pos> {
        self.keys.closest_to(color, self.player_pos)
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

        for y in 0..self.map_height {
            for x in 0..self.map_width {
                let pos = Pos::new(x, y);

                // Check if this is the player position
                if pos == self.player_pos {
                    output.push_str(&format!("{}@{}", PLAYER, RESET));
                } else if Some(pos) == self.player2_pos {
                    output.push_str(&format!("{}2{}", PLAYER2, RESET));
                } else if self.unexplored_frontier.contains(&pos) {
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
                            if self.boulder_info.has_moved(&pos) {
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
