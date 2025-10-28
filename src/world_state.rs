use std::collections::{HashMap, HashSet};

use tracing::{debug, warn};

use crate::boulder_tracker::BoulderTracker;
use crate::item_tracker::{ColoredItemTracker, ItemTracker};
use crate::map::Map;
use crate::pathfinding::AStar;
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

    // Loop mode statistics
    pub successful_runs: i32,
    pub failed_runs: i32,
    pub game_count: i32,

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

    // Potential enemy locations (positions where enemies became Unknown)
    pub potential_enemy_locations: HashSet<Position>,
}

impl WorldState {
    pub fn new(map_width: i32, map_height: i32, visibility_range: i32) -> Self {
        Self {
            level: 0,
            tick: 0,
            visibility_range,
            successful_runs: 0,
            failed_runs: 0,
            game_count: 0,
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
            potential_enemy_locations: HashSet::new(),
        }
    }

    #[tracing::instrument(level = "trace", skip(self, state))]
    pub fn update(&mut self, state: &State) {
        self.level = state.level;
        self.tick = state.tick;

        let mut all_surroundings = Vec::new();

        // Update player 1
        if let Some(p1) = self.players.get_mut(0) {
            if let Some(player_state) = &state.player_state {
                Self::update_player_state_fields(p1, player_state);
                let bounds = Bounds::from_center_and_range(p1.position, self.visibility_range);
                all_surroundings.push(SurroundingsData {
                    surroundings: &player_state.surroundings,
                    center: p1.position,
                    bounds,
                });
            } else {
                warn!("Player 1 state is None - player has exited!");
                p1.is_active = false;
            }
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
        } else if self.players.len() > 1 {
            // Player 2 state is None, mark as inactive
            if let Some(p2) = self.players.get_mut(1) {
                warn!("Player 2 state is None - player has exited!");
                p2.is_active = false;
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
        self.potential_enemy_locations.clear();

        // Note: successful_runs and failed_runs are NOT cleared
        // to preserve statistics across levels and game restarts

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
        debug!("Player location: {:?}", player_state.position);
        // Check if player has exited (position is -1, -1)
        let has_exited = player_state
            .position
            .as_ref()
            .is_some_and(|pos| pos.x == -1 && pos.y == -1);

        if has_exited {
            if player.is_active {
                debug!("Player has exited");
            }
            player.is_active = false;
        } else {
            player.is_active = true;
        }

        // Update player fields
        if let Some(position) = &player_state.position {
            player.position = Position::new(position.x, position.y);
        } else {
            warn!("Player state position is None!");
        }
        player.health = player_state.health.unwrap_or(5);
        if player_state.health.is_none() {
            debug!("Player health is None, using default 5");
        }
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
            } else if *tile == Tile::Enemy {
                // Remove enemy tiles that are no longer in any player's visibility range
                // When enemies move out of sight, we should remove them from the map
                let is_visible = combined_bounds.iter().any(|b| b.contains(pos));
                if !is_visible {
                    debug!("Enemy at {:?} is no longer visible, removing from map and adding to potential locations", pos);
                    self.potential_enemy_locations.insert(*pos);
                }
                is_visible
            } else {
                // Keep all other tiles (permanent ones should persist)
                true
            }
        });

        // Merge all surroundings into a single HashMap
        let merged_surroundings = self.merge_surroundings(&all_surroundings);

        // Track which permanent items we can currently see
        let mut seen_items = SeenItems::default();

        // Process merged surroundings once
        self.process_surroundings(&merged_surroundings, &mut seen_items);

        // Update item trackers with collected items
        // Pass all bounds so items are validated if visible to ANY player
        self.update_item_trackers(&combined_bounds, seen_items);
    }

    #[tracing::instrument(level = "trace", skip(self, all_surroundings))]
    fn merge_surroundings(&self, all_surroundings: &[SurroundingsData]) -> HashMap<Position, Tile> {
        let mut merged_surroundings: HashMap<Position, Tile> = HashMap::new();

        for data in all_surroundings {
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
                    // If position already exists, prefer non-Unknown tiles
                    match merged_surroundings.get(&tile_position) {
                        Some(Tile::Unknown) if tile != Tile::Unknown => {
                            // Replace Unknown with concrete tile
                            merged_surroundings.insert(tile_position, tile);
                        }
                        None => {
                            // New position
                            merged_surroundings.insert(tile_position, tile);
                        }
                        _ => {
                            // Keep existing tile (either both Unknown or existing is concrete)
                        }
                    }
                }
            }
        }

        merged_surroundings
    }

    #[tracing::instrument(level = "trace", skip(self, merged_surroundings, seen_items))]
    fn process_surroundings(
        &mut self,
        merged_surroundings: &HashMap<Position, Tile>,
        seen_items: &mut SeenItems,
    ) {
        for (&tile_position, &tile) in merged_surroundings.iter() {
            // Special case: if we had an enemy here and now see Unknown, replace with Empty
            // This means both players see Unknown (or one player sees it), so enemy has moved
            if tile == Tile::Unknown && matches!(self.map.get(&tile_position), Some(Tile::Enemy)) {
                debug!(
                    "Enemy at {:?} is now unknown, marking as empty and adding to potential locations",
                    tile_position
                );
                self.map.insert(tile_position, Tile::Empty);
                // Add to potential enemy locations - the enemy may have moved nearby
                self.potential_enemy_locations.insert(tile_position);
                continue;
            }

            // If we see a known tile at a potential enemy location, remove it from potential list
            if tile != Tile::Unknown
                && tile != Tile::Enemy
                && self.potential_enemy_locations.remove(&tile_position)
            {
                debug!(
                    "Position {:?} is now known as {:?}, removing from potential enemy locations",
                    tile_position, tile
                );
            }

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
                // Update Enemy position with Unknown (enemy has moved) - enemies are temporary
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
        // Special case: pressure plates are still there even if a player is standing on them
        let player_positions: Vec<Position> = self.players.iter().map(|p| p.position).collect();
        self.pressure_plates.update_with_positions(
            seen_items.pressure_plates,
            &self.map,
            |tile, pos| {
                matches!(
                    tile,
                    Tile::PressurePlateRed | Tile::PressurePlateGreen | Tile::PressurePlateBlue
                ) || (matches!(tile, Tile::Player) && player_positions.contains(pos))
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
        matches!(
            (player.inventory, color),
            (Inventory::KeyRed, Color::Red)
                | (Inventory::KeyGreen, Color::Green)
                | (Inventory::KeyBlue, Color::Blue)
        )
    }

    pub fn has_door_been_opened(&self, color: Color) -> bool {
        // Check if there's a boulder on a pressure plate of this color
        let boulders_on_plates = self.get_boulders_on_plates();
        boulders_on_plates
            .get(&color)
            .is_some_and(|plates| !plates.is_empty())
    }

    pub fn closest_enemy(&self, player: &PlayerState) -> Option<Position> {
        self.enemies.closest_to(player.position)
    }

    pub fn closest_potential_enemy(&self, player: &PlayerState) -> Option<Position> {
        self.potential_enemy_locations
            .iter()
            .min_by_key(|pos| player.position.distance(pos))
            .copied()
    }

    pub fn closest_health(&self, player: &PlayerState) -> Option<Position> {
        self.health.closest_to(player.position)
    }

    pub fn closest_sword(&self, player: &PlayerState) -> Option<Position> {
        self.swords.closest_to(player.position)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    pub fn doors_without_keys(&self, player: &PlayerState) -> Vec<Color> {
        self.doors
            .colors()
            .filter(|color| !self.has_key(player, **color) && !self.has_door_been_opened(**color))
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

    /// Get the actual path distance between two positions, returns None if unreachable
    pub fn path_distance(&self, from: Position, to: Position) -> Option<i32> {
        self.map.find_path(from, to).map(|path| path.len() as i32)
    }

    /// Get the path distance to an enemy position.
    /// Only calculates actual path if Manhattan distance < 5, otherwise returns Manhattan distance.
    pub fn path_distance_to_enemy(&self, from: Position, enemy_pos: Position) -> i32 {
        let manhattan_dist = from.distance(&enemy_pos);

        // Only use expensive pathfinding for nearby enemies
        if manhattan_dist < 6 {
            self.path_distance(from, enemy_pos).unwrap_or(i32::MAX)
        } else {
            manhattan_dist
        }
    }

    #[allow(dead_code)]
    pub fn is_walkable_avoiding_enemies(&self, pos: &Position, goal: Position) -> bool {
        // First check basic walkability
        if !self.map.is_walkable(pos, goal) {
            return false;
        }

        // Don't walk on tiles adjacent to enemies (unless it's the goal)
        if *pos != goal {
            for enemy_pos in self.enemies.get_positions() {
                if pos.is_adjacent(enemy_pos) {
                    return false;
                }
            }
        }

        true
    }

    #[allow(dead_code)]
    pub fn find_path_avoiding_enemies(
        &self,
        start: Position,
        goal: Position,
    ) -> Option<Vec<Position>> {
        AStar::find_path(&self.map, start, goal, |pos, goal| {
            self.is_walkable_avoiding_enemies(pos, goal)
        })
    }

    #[tracing::instrument(level = "trace", skip(self))]
    pub fn draw_ascii_map(&self) -> String {
        let mut output = String::new();

        for y in 0..self.map.height {
            for x in 0..self.map.width {
                let pos = Position::new(x, y);

                // Check if this is the player position
                if self.players.first().is_some_and(|p| pos == p.position) {
                    output.push_str("\x1b[1;33m1\x1b[0m"); // Bright yellow
                } else if self.players.get(1).is_some_and(|p| pos == p.position) {
                    output.push_str("\x1b[1;36m2\x1b[0m"); // Bright cyan
                } else {
                    // Render the tile
                    let tile = self.map.get(&pos).copied();
                    output.push_str(&Self::format_tile(tile, &pos, &self.boulders));
                }
            }
            output.push('\n');
        }

        output
    }

    pub fn draw_surroundings(
        &self,
        surroundings: &[i32],
        center: Position,
        player_num: usize,
    ) -> String {
        let mut output = String::new();
        let size = (self.visibility_range * 2 + 1) as usize;

        output.push_str(&format!(
            "Player {} surroundings (center: {}, {}):\n",
            player_num, center.x, center.y
        ));

        for row in 0..size {
            for col in 0..size {
                let idx = row * size + col;
                let is_center =
                    row == self.visibility_range as usize && col == self.visibility_range as usize;

                if is_center {
                    let player_char = if player_num == 1 { "1" } else { "2" };
                    let player_color = if player_num == 1 {
                        "\x1b[1;33m"
                    } else {
                        "\x1b[1;36m"
                    };
                    output.push_str(&format!("{}{}\x1b[0m", player_color, player_char));
                } else if let Ok(tile) = Tile::try_from(surroundings[idx]) {
                    let tile_position = Position::new(
                        center.x + col as i32 - self.visibility_range,
                        center.y + row as i32 - self.visibility_range,
                    );
                    output.push_str(&Self::format_tile(Some(tile), &tile_position, &self.boulders));
                } else {
                    output.push('?');
                }
            }
            output.push('\n');
        }

        output
    }

    fn format_tile(tile: Option<Tile>, pos: &Position, boulders: &BoulderTracker) -> String {
        const RESET: &str = "\x1b[0m";
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

        match tile {
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
                if boulders.has_moved(pos) {
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
        }
    }

    /// Check if we're in 2-player mode
    pub fn is_two_player_mode(&self) -> bool {
        self.players.len() == 2
    }

    /// Check if any player still has unexplored frontier tiles
    pub fn any_player_has_frontier(&self) -> bool {
        self.players
            .iter()
            .any(|p| !p.unexplored_frontier.is_empty())
    }
}
