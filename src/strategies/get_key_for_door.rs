use tracing::debug;

use crate::goals::Goal;
use crate::strategies::planner::{SelectGoal, StrategyType};
use crate::types::Color;
use crate::world_state::WorldState;

pub struct GetKeyForDoorStrategy;

impl SelectGoal for GetKeyForDoorStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        let mut goals = vec![None; world.players.len()];

        // Track which key colors have been assigned to prevent conflicts
        let mut assigned_key_colors = std::collections::HashSet::new();

        // In 2-player mode, check which door colors have reachable pressure plates
        let mut doors_with_plates = std::collections::HashSet::new();
        if world.is_two_player_mode() {
            for &color in &[Color::Red, Color::Green, Color::Blue] {
                if let Some(plates) = world.pressure_plates.get_positions(color) {
                    // Check if any player can reach any plate of this color
                    let can_reach_plate = world.players.iter().any(|player| {
                        plates.iter().any(|&plate_pos| {
                            world.map.find_path(player.position, plate_pos).is_some()
                        })
                    });
                    if can_reach_plate {
                        doors_with_plates.insert(color);
                        debug!("In 2-player mode: {:?} door has reachable pressure plate", color);
                    }
                }
            }
        }

        for (player_index, player) in world.players.iter().enumerate() {
            // Skip if player already has a goal
            if player_index < current_goals.len() && current_goals[player_index].is_some() {
                continue;
            }

            for color in world.doors_without_keys(player) {
                // Skip if this key color is already assigned to another player
                if assigned_key_colors.contains(&color) {
                    continue;
                }

                debug!("Player {} checking door without key: {:?}", player_index + 1, color);

                // If we know where the key is and can reach it, go get it
                debug!(
                    "Checking if we know key location for {:?}: {}",
                    color,
                    world.knows_key_location(color)
                );
                if world.knows_key_location(color) {
                    if let Some(key_pos) = world.closest_key(player, color) {
                        debug!("Closest key for {:?} is at {:?}", color, key_pos);
                        
                        // In 2-player mode with reachable pressure plate, treat matching doors as walkable
                        let can_reach = if world.is_two_player_mode() && doors_with_plates.contains(&color) {
                            world.map.find_path_with_custom_walkability(player.position, key_pos, |pos, goal, _tick| {
                                // Check if this door matches our target color and we have a reachable plate
                                let is_matching_door = matches!((world.map.get(pos), color), 
                                        (Some(crate::swoq_interface::Tile::DoorRed), Color::Red) | 
                                        (Some(crate::swoq_interface::Tile::DoorGreen), Color::Green) | 
                                        (Some(crate::swoq_interface::Tile::DoorBlue), Color::Blue));
                                if is_matching_door {
                                    debug!("Treating {:?} door at {:?} as walkable (plate reachable in 2P mode)", color, pos);
                                    true
                                } else {
                                    world.map.is_walkable(pos, goal)
                                }
                            }).is_some()
                        } else {
                            world.map.find_path(player.position, key_pos).is_some()
                        };
                        
                        if can_reach {
                            debug!(
                                "[GetKeyForDoorStrategy] Player {} assigned to get {:?} key (reachable)",
                                player_index + 1,
                                color
                            );
                            goals[player_index] = Some(Goal::GetKey(color));
                            assigned_key_colors.insert(color);
                            break;
                        } else {
                            debug!("Key at {:?} is not reachable", key_pos);
                        }
                    } else {
                        debug!("No keys found for {:?}!", color);
                    }
                }
            }
        }

        goals
    }
}
