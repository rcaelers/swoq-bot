use tracing::debug;

use crate::goals::Goal;
use crate::strategies::planner::{SelectGoal, StrategyType};
use crate::types::Color;
use crate::world_state::WorldState;

pub struct DropBoulderOnPlateStrategy;

impl SelectGoal for DropBoulderOnPlateStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        if world.level < 6 {
            return vec![None; world.players.len()];
        }

        let mut goals = vec![None; world.players.len()];

        // Iterate over each color
        for color in [Color::Red, Color::Green, Color::Blue] {
            // Only consider this color if there's a door of the same color
            if !world.doors.has_color(color) {
                continue;
            }

            // Get pressure plates for this color
            let plates = match world.pressure_plates.get_positions(color) {
                Some(plates) if !plates.is_empty() => plates,
                _ => continue,
            };

            debug!("Found {} {:?} pressure plates with matching doors", plates.len(), color);

            let mut best_player: Option<(usize, i32, crate::types::Position)> = None; // (player_index, distance, plate_pos)

            // Find the best player for this color
            for (player_index, player) in world.players.iter().enumerate() {
                // Skip if player already has a goal
                if current_goals[player_index].is_some() {
                    continue;
                }

                // Skip if player already assigned a plate in this iteration
                if goals[player_index].is_some() {
                    continue;
                }

                // Skip if player is not carrying a boulder
                if player.inventory != crate::swoq_interface::Inventory::Boulder {
                    continue;
                }

                // Find the closest reachable plate for this player
                let mut min_distance = i32::MAX;
                let mut closest_plate = None;

                for &plate_pos in plates {
                    if let Some(path_len) = world.path_distance(player.position, plate_pos)
                        && path_len < min_distance
                    {
                        min_distance = path_len;
                        closest_plate = Some(plate_pos);
                    }
                }

                // If we found at least one reachable plate, consider this player
                if let Some(plate_pos) = closest_plate {
                    // Update best player if this one is closer
                    best_player = match best_player {
                        None => Some((player_index, min_distance, plate_pos)),
                        Some((_, best_distance, _)) if min_distance < best_distance => {
                            Some((player_index, min_distance, plate_pos))
                        }
                        _ => best_player,
                    };
                }
            }

            // Assign the goal to the best player
            if let Some((player_index, _, plate_pos)) = best_player {
                debug!(
                    "[DropBoulderOnPlateStrategy] Player {} carrying boulder assigned to {:?} pressure plate at {:?} with matching door",
                    player_index + 1,
                    color,
                    plate_pos
                );
                goals[player_index] = Some(Goal::DropBoulderOnPlate(color, plate_pos));
            }
        }

        goals
    }
}
