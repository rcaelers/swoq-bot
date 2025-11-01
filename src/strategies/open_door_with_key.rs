use tracing::debug;

use crate::goals::Goal;
use crate::strategies::planner::{SelectGoal, StrategyType};
use crate::world_state::WorldState;

pub struct OpenDoorWithKeyStrategy;

impl SelectGoal for OpenDoorWithKeyStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        let mut goals = vec![None; world.players.len()];

        // Iterate over each door color
        for &color in world.doors.colors() {
            let door_positions = match world.doors.get_positions(color) {
                Some(pos) if !pos.is_empty() => pos,
                _ => continue,
            };

            // Check if any door of this color has a reachable empty neighbor from any player
            let mut best_player: Option<(usize, i32)> = None; // (player_index, distance)

            for (player_index, player) in world.players.iter().enumerate() {
                // Skip if player already has a goal
                if player_index < current_goals.len() && current_goals[player_index].is_some() {
                    continue;
                }
                if goals[player_index].is_some() {
                    continue;
                }

                // Skip if player doesn't have the key for this color
                if !world.has_key(player, color) {
                    continue;
                }

                // Check if any door of this color is reachable by this player
                let mut min_distance = i32::MAX;
                for &door_pos in door_positions {
                    // Check if any neighbor of the door is reachable
                    for &neighbor in &door_pos.neighbors() {
                        // Only consider empty tiles (or player position)
                        if neighbor != player.position
                            && !matches!(
                                world.map.get(&neighbor),
                                Some(crate::swoq_interface::Tile::Empty)
                            )
                        {
                            continue;
                        }

                        // Calculate distance to this neighbor
                        let distance = if player.position == neighbor {
                            0 // Already at the door
                        } else {
                            match world.find_path(player.position, neighbor) {
                                Some(path) => path.len() as i32,
                                None => continue, // Can't reach this neighbor
                            }
                        };

                        min_distance = min_distance.min(distance);
                    }
                }

                // If we found at least one reachable door, consider this player
                if min_distance < i32::MAX {
                    debug!(
                        "Player {} can reach {:?} door (has key, distance: {})",
                        player_index + 1,
                        color,
                        min_distance
                    );

                    // Update best player if this one is closer
                    best_player = match best_player {
                        None => Some((player_index, min_distance)),
                        Some((_, best_dist)) if min_distance < best_dist => {
                            Some((player_index, min_distance))
                        }
                        _ => best_player,
                    };
                }
            }

            // Assign the goal to the best player
            if let Some((player_index, _)) = best_player {
                debug!(
                    "[OpenDoorWithKeyStrategy] Player {} assigned to open {:?} door (has key, door reachable)",
                    player_index + 1,
                    color
                );
                goals[player_index] = Some(Goal::OpenDoor(color));
            }
        }

        goals
    }
}
