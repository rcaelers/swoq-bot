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
    pub exit_pos: Option<Pos>,
    pub key_positions: HashMap<Color, Pos>,
    pub door_positions: HashMap<Color, Vec<Pos>>,
    pub enemy_positions: Vec<Pos>,
    pub boulder_positions: Vec<Pos>,
    pub sword_positions: Vec<Pos>,
    pub health_positions: Vec<Pos>,
    pub pressure_plate_positions: HashMap<Color, Vec<Pos>>,
    pub boss_position: Option<Pos>,
    pub treasure_position: Option<Pos>,

    pub unexplored_frontier: HashSet<Pos>,

    // Planning state to avoid oscillation
    pub previous_goal: Option<crate::goal::Goal>,
    pub current_destination: Option<Pos>,
    pub current_path: Option<Vec<Pos>>,

    // Track positions where we've dropped boulders (these are explored)
    pub dropped_boulder_positions: HashSet<Pos>,
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
            key_positions: HashMap::new(),
            door_positions: HashMap::new(),
            enemy_positions: Vec::new(),
            boulder_positions: Vec::new(),
            sword_positions: Vec::new(),
            health_positions: Vec::new(),
            pressure_plate_positions: HashMap::new(),
            boss_position: None,
            treasure_position: None,
            unexplored_frontier: HashSet::new(),
            previous_goal: None,
            current_destination: None,
            current_path: None,
            dropped_boulder_positions: HashSet::new(),
        }
    }

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

    pub fn reset_for_new_level(&mut self) {
        // Clear all map data for the new level
        self.map.clear();
        self.exit_pos = None;
        self.key_positions.clear();
        self.door_positions.clear();
        self.enemy_positions.clear();
        self.boulder_positions.clear();
        self.sword_positions.clear();
        self.health_positions.clear();
        self.pressure_plate_positions.clear();
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
        self.dropped_boulder_positions.clear();
    }

    fn integrate_surroundings(&mut self, surroundings: &[i32], center: Pos) {
        let size = (self.visibility_range * 2 + 1) as usize;

        // Don't clear enemy_positions - enemies don't move when they can't see the player
        // We'll update them below when we see them, or remove them if they're confirmed gone

        // Calculate visibility bounds
        let min_x = center.x - self.visibility_range;
        let max_x = center.x + self.visibility_range;
        let min_y = center.y - self.visibility_range;
        let max_y = center.y + self.visibility_range;

        // Remove Unknown tiles that are now outside our visibility range
        // They should revert to unseen (None/?) since we have no current information
        self.map.retain(|pos, tile| {
            if *tile == Tile::Unknown {
                // Keep Unknown tiles only if they're in our current visibility range
                pos.x >= min_x && pos.x <= max_x && pos.y >= min_y && pos.y <= max_y
            } else {
                // Keep all other tiles (permanent ones should persist)
                true
            }
        });

        // Track which permanent items we can currently see
        let mut seen_keys: HashMap<Color, Pos> = HashMap::new();
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
                // Detect suspicious tile changes
                if let Some(old_tile) = self.map.get(&pos) {
                    match (old_tile, tile) {
                        // Wall becoming empty or other walkable tile
                        (Tile::Wall, Tile::Empty | Tile::Player | Tile::Exit) => {
                            warn!(
                                "SUSPICIOUS: Wall at {:?} changed to {:?} (tick {})",
                                pos, tile, self.tick
                            );
                        }
                        // Any permanent tile changing to something else (except Unknown fog)
                        (Tile::Wall, new_tile)
                            if new_tile != Tile::Wall && new_tile != Tile::Unknown =>
                        {
                            warn!(
                                "SUSPICIOUS: Wall at {:?} changed to {:?} (tick {})",
                                pos, new_tile, self.tick
                            );
                        }
                        // Door consumed (opened)
                        (Tile::DoorRed | Tile::DoorGreen | Tile::DoorBlue, Tile::Empty) => {
                            // This is normal - door was opened
                        }
                        // Key consumed (picked up)
                        (Tile::KeyRed | Tile::KeyGreen | Tile::KeyBlue, Tile::Empty) => {
                            // This is normal - key was picked up
                        }
                        _ => {}
                    }
                }

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
                // If we see a boulder in a location that was empty and adjacent to us, we dropped it
                // Note: if old_tile is None (never seen), the boulder is unexplored (original position)
                if tile == Tile::Boulder && self.player_pos.is_adjacent(&pos) {
                    match self.map.get(&pos) {
                        Some(Tile::Empty)
                        | Some(Tile::Player)
                        | Some(
                            Tile::PressurePlateRed
                            | Tile::PressurePlateGreen
                            | Tile::PressurePlateBlue,
                        ) => {
                            debug!(
                                "Marking boulder position {:?} as explored (we dropped it)",
                                pos
                            );
                            self.dropped_boulder_positions.insert(pos);
                        }
                        None => {
                            // Boulder in a never-seen location - it's unexplored (original position)
                        }
                        _ => {
                            // Boulder replacing something else - likely not a drop
                        }
                    }
                }

                if should_update {
                    self.map.insert(pos, tile);
                }

                // Track special tiles we can see (only if not Unknown)
                if tile != Tile::Unknown {
                    match tile {
                        Tile::Exit => self.exit_pos = Some(pos),
                        Tile::KeyRed => {
                            seen_keys.insert(Color::Red, pos);
                        }
                        Tile::KeyGreen => {
                            seen_keys.insert(Color::Green, pos);
                        }
                        Tile::KeyBlue => {
                            seen_keys.insert(Color::Blue, pos);
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
                        Tile::Boulder => {
                            seen_boulders.push(pos);
                        }
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

        // Update key positions: keep old ones we can't see, update with newly seen ones
        for (color, pos) in seen_keys {
            self.key_positions.insert(color, pos);
        }
        // Remove keys that we saw become Empty (picked up)
        self.key_positions.retain(|_color, pos| {
            if let Some(tile) = self.map.get(pos) {
                matches!(tile, Tile::KeyRed | Tile::KeyGreen | Tile::KeyBlue)
            } else {
                true // Keep if we haven't seen this position
            }
        });

        // Update door positions: merge newly seen doors with previously known ones
        for (color, new_positions) in seen_doors {
            self.door_positions
                .entry(color)
                .or_default()
                .extend(new_positions);
        }
        // Deduplicate and remove opened doors
        for positions in self.door_positions.values_mut() {
            // Remove duplicates manually
            let mut unique_positions: Vec<Pos> = Vec::new();
            for &pos in positions.iter() {
                if !unique_positions.contains(&pos) {
                    unique_positions.push(pos);
                }
            }
            *positions = unique_positions;

            // Remove doors that have been opened
            positions.retain(|pos| {
                if let Some(tile) = self.map.get(pos) {
                    matches!(tile, Tile::DoorRed | Tile::DoorGreen | Tile::DoorBlue)
                } else {
                    true // Keep if we haven't seen this position
                }
            });
        }

        // Update pressure plates similarly
        for (color, new_positions) in seen_pressure_plates {
            debug!("Saw {} new {:?} pressure plates this tick", new_positions.len(), color);
            self.pressure_plate_positions
                .entry(color)
                .or_default()
                .extend(new_positions);
        }

        debug!(
            "Total pressure plates before cleanup: Red={}, Green={}, Blue={}",
            self.pressure_plate_positions
                .get(&Color::Red)
                .map_or(0, |v| v.len()),
            self.pressure_plate_positions
                .get(&Color::Green)
                .map_or(0, |v| v.len()),
            self.pressure_plate_positions
                .get(&Color::Blue)
                .map_or(0, |v| v.len())
        );

        for (color, positions) in self.pressure_plate_positions.iter_mut() {
            debug!("Processing {:?} pressure plates: {} positions", color, positions.len());
            // Remove duplicates manually
            let mut unique_positions: Vec<Pos> = Vec::new();
            for &pos in positions.iter() {
                if !unique_positions.contains(&pos) {
                    unique_positions.push(pos);
                }
            }
            *positions = unique_positions;
            debug!("Processing unique {:?} pressure plates: {} positions", color, positions.len());

            // Remove pressure plates that have been covered (e.g., by boulders)
            let before_count = positions.len();
            positions.retain(|pos| {
                if let Some(tile) = self.map.get(pos) {
                    let is_plate = matches!(
                        tile,
                        Tile::PressurePlateRed | Tile::PressurePlateGreen | Tile::PressurePlateBlue
                    );
                    if !is_plate {
                        debug!(
                            "Removing {:?} pressure plate at {:?} - now shows as {:?}",
                            color, pos, tile
                        );
                    } else {
                        debug!("Keeping {:?} pressure plate at {:?} (still a plate)", color, pos);
                    }
                    is_plate
                } else {
                    debug!("Keeping {:?} pressure plate at {:?} (position not in map)", color, pos);
                    true // Keep if we haven't seen this position
                }
            });
            let after_count = positions.len();
            if before_count != after_count {
                debug!(
                    "Pressure plate count for {:?}: {} -> {} (removed {})",
                    color,
                    before_count,
                    after_count,
                    before_count - after_count
                );
            }
        }

        // Update boulder positions: merge newly seen boulders with previously known ones
        self.boulder_positions.extend(seen_boulders);
        // Remove duplicates
        let mut unique_boulders: Vec<Pos> = Vec::new();
        for &pos in self.boulder_positions.iter() {
            if !unique_boulders.contains(&pos) {
                unique_boulders.push(pos);
            }
        }
        self.boulder_positions = unique_boulders;
        // Remove boulders that have been picked up (turned to Empty)
        self.boulder_positions.retain(|pos| {
            if let Some(tile) = self.map.get(pos) {
                matches!(tile, Tile::Boulder)
            } else {
                true // Keep if we haven't seen this position
            }
        });

        // Update sword positions similarly
        self.sword_positions.extend(seen_swords);
        let mut unique_swords: Vec<Pos> = Vec::new();
        for &pos in self.sword_positions.iter() {
            if !unique_swords.contains(&pos) {
                unique_swords.push(pos);
            }
        }
        self.sword_positions = unique_swords;
        self.sword_positions.retain(|pos| {
            if let Some(tile) = self.map.get(pos) {
                matches!(tile, Tile::Sword)
            } else {
                true
            }
        });

        // Update health positions similarly
        self.health_positions.extend(seen_health);
        let mut unique_health: Vec<Pos> = Vec::new();
        for &pos in self.health_positions.iter() {
            if !unique_health.contains(&pos) {
                unique_health.push(pos);
            }
        }
        self.health_positions = unique_health;
        self.health_positions.retain(|pos| {
            if let Some(tile) = self.map.get(pos) {
                matches!(tile, Tile::Health)
            } else {
                true
            }
        });

        // Update enemy positions: add newly seen enemies, remove enemies we can confirm are gone
        for enemy_pos in seen_enemies {
            if !self.enemy_positions.contains(&enemy_pos) {
                debug!("New enemy spotted at {:?}", enemy_pos);
                self.enemy_positions.push(enemy_pos);
            }
        }
        // Remove enemies that we can now see are not there anymore
        // Only remove if the position is in our current visibility range and the tile is not Enemy
        self.enemy_positions.retain(|pos| {
            // Check if position is in current visibility range
            let in_range = pos.x >= min_x && pos.x <= max_x && pos.y >= min_y && pos.y <= max_y;

            if in_range {
                // We can see this position, so check if enemy is still there
                if let Some(tile) = self.map.get(pos) {
                    let still_enemy = matches!(tile, Tile::Enemy);
                    if !still_enemy {
                        debug!("Enemy at {:?} is gone (now {:?})", pos, tile);
                    }
                    still_enemy
                } else {
                    // Position in range but not in map? Keep enemy for now
                    true
                }
            } else {
                // Out of visibility range - keep the enemy position (enemies don't move when not visible)
                true
            }
        });
    }

    fn update_frontier(&mut self) {
        // Compute reachable frontier positions in a single pass
        // This replaces the old two-step process of:
        // 1. Finding all candidates (neighbors of known tiles that are Unknown/None)
        // 2. Filtering by reachability
        self.unexplored_frontier =
            crate::pathfinding::AStar::compute_reachable_positions(self, self.player_pos);

        debug!("Frontier size: {}", self.unexplored_frontier.len());
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

    pub fn has_key(&self, color: Color) -> bool {
        matches!(
            (self.player_inventory, color),
            (Inventory::KeyRed, Color::Red)
                | (Inventory::KeyGreen, Color::Green)
                | (Inventory::KeyBlue, Color::Blue)
        )
    }

    pub fn sorted_unexplored(&self) -> Vec<Pos> {
        let mut frontier: Vec<Pos> = self.unexplored_frontier.iter().copied().collect();
        frontier.sort_by_key(|pos| self.player_pos.distance(pos));
        frontier
    }

    pub fn closest_enemy(&self) -> Option<Pos> {
        self.enemy_positions
            .iter()
            .min_by_key(|pos| self.player_pos.distance(pos))
            .copied()
    }

    pub fn closest_health(&self) -> Option<Pos> {
        self.health_positions
            .iter()
            .min_by_key(|pos| self.player_pos.distance(pos))
            .copied()
    }

    pub fn doors_without_keys(&self) -> Vec<Color> {
        // Return all door colors we don't have keys for
        self.door_positions
            .keys()
            .filter(|color| !self.has_key(**color))
            .copied()
            .collect()
    }

    pub fn closest_door_of_color(&self, color: Color) -> Option<Pos> {
        // Find the closest door of a specific color
        self.door_positions.get(&color).and_then(|positions| {
            positions
                .iter()
                .min_by_key(|pos| self.player_pos.distance(pos))
                .copied()
        })
    }

    pub fn knows_key_location(&self, color: Color) -> bool {
        self.key_positions.contains_key(&color)
    }

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
                            // Show unexplored boulders in frontier color, explored in dim yellow
                            if self.dropped_boulder_positions.contains(&pos) {
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
