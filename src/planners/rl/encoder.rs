//! State encoder for RL - converts WorldState to tensor observations

use crate::infra::{Color, Position};
use crate::state::WorldState;
use crate::swoq_interface::Inventory;

/// Maximum supported map dimensions for encoding
pub const MAX_MAP_SIZE: usize = 64;

/// Configuration for the state encoder
#[derive(Debug, Clone)]
pub struct EncoderConfig {
    /// Maximum number of players to encode
    pub max_players: usize,
    /// Maximum number of enemies to encode
    pub max_enemies: usize,
    /// Maximum number of keys to encode
    pub max_keys: usize,
    /// Maximum number of doors to encode  
    pub max_doors: usize,
    /// Maximum number of swords to encode
    pub max_swords: usize,
    /// Maximum number of health items to encode
    pub max_health: usize,
    /// Maximum number of pressure plates to encode
    pub max_plates: usize,
    /// Maximum number of boulders to encode
    pub max_boulders: usize,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            max_players: 4,
            max_enemies: 16,
            max_keys: 8,
            max_doors: 8,
            max_swords: 8,
            max_health: 8,
            max_plates: 8,
            max_boulders: 8,
        }
    }
}

/// State encoder for converting WorldState to flat feature vectors
#[derive(Debug, Clone)]
pub struct StateEncoder {
    config: EncoderConfig,
}

impl StateEncoder {
    pub fn new(config: EncoderConfig) -> Self {
        Self { config }
    }

    /// Calculate the size of a single player's local observation
    pub fn local_obs_size(&self) -> usize {
        // Player features: position (2), health (1), inventory (4 one-hot), has_key_colors (3)
        let player_size = 2 + 1 + 4 + 3;

        // Enemy features: position (2), health (1), damage (1), valid (1)
        let enemy_size = 5 * self.config.max_enemies;

        // Object features: position (2), valid (1)
        let key_size = 3 * self.config.max_keys;
        let door_size = 4 * self.config.max_doors; // + color (1)
        let sword_size = 3 * self.config.max_swords;
        let health_size = 3 * self.config.max_health;
        let plate_size = 5 * self.config.max_plates; // + color (1), pressed (1)
        let boulder_size = 3 * self.config.max_boulders;

        // Exit features
        let exit_size = 3; // position (2), visible (1)

        // Level info
        let level_size = 1;

        player_size
            + enemy_size
            + key_size
            + door_size
            + sword_size
            + health_size
            + plate_size
            + boulder_size
            + exit_size
            + level_size
    }

    /// Calculate the size of global observation for centralized critic
    pub fn global_obs_size(&self) -> usize {
        // All players' local observations + inter-player features
        let all_players = self.local_obs_size() * self.config.max_players;

        // Additional global features: level progress, team health, exploration %
        let global_features = 10;

        all_players + global_features
    }

    /// Encode a single player's local observation
    pub fn encode_local_obs(&self, world: &WorldState, player_index: usize) -> Vec<f32> {
        let mut obs = Vec::with_capacity(self.local_obs_size());

        let player = &world.players[player_index];
        let map_width = world.map.width as f32;
        let map_height = world.map.height as f32;

        // Player position (normalized)
        obs.push(player.position.x as f32 / map_width);
        obs.push(player.position.y as f32 / map_height);

        // Player health (normalized, assuming max health ~10)
        obs.push(player.health as f32 / 10.0);

        // Inventory one-hot: [None, Key, Sword, Boulder]
        obs.push(if player.inventory == Inventory::None {
            1.0
        } else {
            0.0
        });
        obs.push(if Self::inventory_is_key(player.inventory) {
            1.0
        } else {
            0.0
        });
        obs.push(0.0); // No sword inventory in current version
        obs.push(if player.inventory == Inventory::Boulder {
            1.0
        } else {
            0.0
        });

        // Has key colors (using WorldState::has_key)
        obs.push(if world.has_key(player, Color::Red) {
            1.0
        } else {
            0.0
        });
        obs.push(if world.has_key(player, Color::Green) {
            1.0
        } else {
            0.0
        });
        obs.push(if world.has_key(player, Color::Blue) {
            1.0
        } else {
            0.0
        });

        // Enemies (using ItemTracker)
        let mut enemy_count = 0;
        for pos in world.enemies.get_positions() {
            if enemy_count >= self.config.max_enemies {
                break;
            }
            obs.push(pos.x as f32 / map_width);
            obs.push(pos.y as f32 / map_height);
            obs.push(1.0); // health (we don't have per-enemy health in ItemTracker)
            obs.push(1.0); // damage
            obs.push(1.0); // valid flag
            enemy_count += 1;
        }
        while enemy_count < self.config.max_enemies {
            obs.extend_from_slice(&[0.0, 0.0, 0.0, 0.0, 0.0]);
            enemy_count += 1;
        }

        // Keys (using ColoredItemTracker)
        let mut key_count = 0;
        for color in [Color::Red, Color::Green, Color::Blue] {
            if let Some(positions) = world.keys.get_positions(color) {
                for pos in positions {
                    if key_count >= self.config.max_keys {
                        break;
                    }
                    obs.push(pos.x as f32 / map_width);
                    obs.push(pos.y as f32 / map_height);
                    obs.push(1.0); // valid flag
                    key_count += 1;
                }
            }
        }
        while key_count < self.config.max_keys {
            obs.extend_from_slice(&[0.0, 0.0, 0.0]);
            key_count += 1;
        }

        // Doors
        let mut door_count = 0;
        for color in [Color::Red, Color::Green, Color::Blue] {
            if let Some(positions) = world.doors.get_positions(color) {
                for pos in positions {
                    if door_count >= self.config.max_doors {
                        break;
                    }
                    obs.push(pos.x as f32 / map_width);
                    obs.push(pos.y as f32 / map_height);
                    obs.push(Self::encode_color(color));
                    obs.push(1.0); // valid flag
                    door_count += 1;
                }
            }
        }
        while door_count < self.config.max_doors {
            obs.extend_from_slice(&[0.0, 0.0, 0.0, 0.0]);
            door_count += 1;
        }

        // Swords
        let mut sword_count = 0;
        for pos in world.swords.get_positions() {
            if sword_count >= self.config.max_swords {
                break;
            }
            obs.push(pos.x as f32 / map_width);
            obs.push(pos.y as f32 / map_height);
            obs.push(1.0);
            sword_count += 1;
        }
        while sword_count < self.config.max_swords {
            obs.extend_from_slice(&[0.0, 0.0, 0.0]);
            sword_count += 1;
        }

        // Health items
        let mut health_count = 0;
        for pos in world.health.get_positions() {
            if health_count >= self.config.max_health {
                break;
            }
            obs.push(pos.x as f32 / map_width);
            obs.push(pos.y as f32 / map_height);
            obs.push(1.0);
            health_count += 1;
        }
        while health_count < self.config.max_health {
            obs.extend_from_slice(&[0.0, 0.0, 0.0]);
            health_count += 1;
        }

        // Pressure plates
        let mut plate_count = 0;
        for color in [Color::Red, Color::Green, Color::Blue] {
            if let Some(positions) = world.pressure_plates.get_positions(color) {
                for pos in positions {
                    if plate_count >= self.config.max_plates {
                        break;
                    }
                    obs.push(pos.x as f32 / map_width);
                    obs.push(pos.y as f32 / map_height);
                    obs.push(Self::encode_color(color));
                    let pressed = Self::is_plate_pressed(world, pos);
                    obs.push(if pressed { 1.0 } else { 0.0 });
                    obs.push(1.0);
                    plate_count += 1;
                }
            }
        }
        while plate_count < self.config.max_plates {
            obs.extend_from_slice(&[0.0, 0.0, 0.0, 0.0, 0.0]);
            plate_count += 1;
        }

        // Boulders
        let mut boulder_count = 0;
        for pos in world.boulders.get_all_positions() {
            if boulder_count >= self.config.max_boulders {
                break;
            }
            obs.push(pos.x as f32 / map_width);
            obs.push(pos.y as f32 / map_height);
            obs.push(1.0);
            boulder_count += 1;
        }
        while boulder_count < self.config.max_boulders {
            obs.extend_from_slice(&[0.0, 0.0, 0.0]);
            boulder_count += 1;
        }

        // Exit
        if let Some(exit_pos) = world.exit_position {
            obs.push(exit_pos.x as f32 / map_width);
            obs.push(exit_pos.y as f32 / map_height);
            obs.push(1.0); // visible
        } else {
            obs.extend_from_slice(&[0.0, 0.0, 0.0]);
        }

        // Level (normalized, assuming max level ~10)
        obs.push(world.level as f32 / 10.0);

        obs
    }

    /// Encode global observation for centralized critic
    pub fn encode_global_obs(&self, world: &WorldState) -> Vec<f32> {
        let mut obs = Vec::with_capacity(self.global_obs_size());

        // Encode all players' local observations
        for player_idx in 0..self.config.max_players {
            if player_idx < world.players.len() {
                obs.extend(self.encode_local_obs(world, player_idx));
            } else {
                // Pad with zeros for missing players
                obs.extend(vec![0.0; self.local_obs_size()]);
            }
        }

        // Global features

        // Team total health
        let total_health: i32 = world.players.iter().map(|p| p.health).sum();
        obs.push(total_health as f32 / (world.players.len() as f32 * 10.0));

        // Number of alive players
        let alive_players = world.players.iter().filter(|p| p.health > 0).count();
        obs.push(alive_players as f32 / self.config.max_players as f32);

        // Exploration progress (rough estimate based on seen tiles)
        let explored_ratio = Self::estimate_exploration(world);
        obs.push(explored_ratio);

        // Number of keys collected by team
        let team_keys: usize = world
            .players
            .iter()
            .map(|p| {
                [Color::Red, Color::Green, Color::Blue]
                    .iter()
                    .filter(|&&c| world.has_key(p, c))
                    .count()
            })
            .sum();
        obs.push(team_keys as f32 / 9.0); // Max 3 colors * 3 keys each

        // Number of enemies remaining
        obs.push(world.enemies.get_positions().len() as f32 / 16.0);

        // Exit visible
        obs.push(if world.exit_position.is_some() {
            1.0
        } else {
            0.0
        });

        // Level
        obs.push(world.level as f32 / 10.0);

        // Padding to reach global_obs_size
        while obs.len() < self.global_obs_size() {
            obs.push(0.0);
        }

        obs
    }

    /// Helper: Check if inventory is a key
    fn inventory_is_key(inv: Inventory) -> bool {
        matches!(inv, Inventory::KeyRed | Inventory::KeyGreen | Inventory::KeyBlue)
    }

    /// Encode color as a normalized float
    fn encode_color(color: Color) -> f32 {
        match color {
            Color::Red => 0.0,
            Color::Green => 0.5,
            Color::Blue => 1.0,
        }
    }

    /// Check if a pressure plate is pressed
    fn is_plate_pressed(world: &WorldState, pos: &Position) -> bool {
        // Check if any player is standing on it
        for player in &world.players {
            if &player.position == pos {
                return true;
            }
        }
        // Check if boulder is on it
        world.boulders.contains(pos)
    }

    /// Estimate exploration progress
    fn estimate_exploration(world: &WorldState) -> f32 {
        let total_cells = (world.map.width * world.map.height) as f32;
        let explored = world.map.len() as f32; // Number of known tiles
        explored / total_cells
    }
}

/// Batch of observations for training
#[derive(Debug, Clone)]
pub struct ObservationBatch {
    /// Local observations [batch_size, num_players, local_obs_size]
    pub local_obs: Vec<Vec<Vec<f32>>>,
    /// Global observations [batch_size, global_obs_size]
    pub global_obs: Vec<Vec<f32>>,
    /// Action masks [batch_size, num_players, max_actions]
    pub action_masks: Vec<Vec<Vec<f32>>>,
}

impl ObservationBatch {
    pub fn new() -> Self {
        Self {
            local_obs: Vec::new(),
            global_obs: Vec::new(),
            action_masks: Vec::new(),
        }
    }

    pub fn add(&mut self, local: Vec<Vec<f32>>, global: Vec<f32>, masks: Vec<Vec<f32>>) {
        self.local_obs.push(local);
        self.global_obs.push(global);
        self.action_masks.push(masks);
    }

    pub fn len(&self) -> usize {
        self.local_obs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.local_obs.is_empty()
    }
}

impl Default for ObservationBatch {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_config_defaults() {
        let config = EncoderConfig::default();
        assert_eq!(config.max_players, 4);
        assert_eq!(config.max_enemies, 16);
    }

    #[test]
    fn test_encoder_obs_sizes() {
        let encoder = StateEncoder::new(EncoderConfig::default());

        // Verify sizes are consistent
        let local_size = encoder.local_obs_size();
        let global_size = encoder.global_obs_size();

        assert!(local_size > 0);
        assert!(global_size > local_size);
    }
}
