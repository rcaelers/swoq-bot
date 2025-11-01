use tracing::debug;

use crate::goals::Goal;
use crate::strategies::planner::{SelectGoal, StrategyType};
use crate::types::Color;
use crate::world_state::WorldState;

pub struct FallbackPressurePlateStrategy;

impl SelectGoal for FallbackPressurePlateStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    fn try_select_coop(
        &mut self,
        world: &WorldState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        let mut goals = vec![None; world.players.len()];

        // Iterate over each color and assign one player per color
        for color in [Color::Red, Color::Green, Color::Blue] {
            // Get door positions for this color
            let door_positions = match world.doors.get_positions(color) {
                Some(pos) if !pos.is_empty() => pos,
                _ => continue,
            };

            // Get pressure plates for this color
            let plates = match world.pressure_plates.get_positions(color) {
                Some(plates) if !plates.is_empty() => plates,
                _ => continue,
            };

            // Find plates that are within distance 4 of doors of the same color
            let nearby_plates: Vec<crate::types::Position> = plates
                .iter()
                .copied()
                .filter(|plate_pos| {
                    door_positions
                        .iter()
                        .any(|door_pos| plate_pos.distance(door_pos) <= 4)
                })
                .collect();

            if nearby_plates.is_empty() {
                continue;
            }

            let mut best_player: Option<(usize, i32, crate::types::Position)> = None; // (player_index, distance, plate_pos)

            // Find the best player for this color
            for (player_index, player) in world.players.iter().enumerate() {
                // Skip if player already has a goal
                if player_index < current_goals.len() && current_goals[player_index].is_some() {
                    continue;
                }
                if goals[player_index].is_some() {
                    continue;
                }

                // If nothing else to do and area is fully explored, step on any reachable pressure plate
                if !player.unexplored_frontier.is_empty() {
                    continue;
                }

                // Find the closest reachable plate for this player
                let mut min_distance = i32::MAX;
                let mut closest_plate = None;

                for &plate_pos in &nearby_plates {
                    let distance = if player.position.is_adjacent(&plate_pos) {
                        0 // Already adjacent
                    } else {
                        match world.find_path(player.position, plate_pos) {
                            Some(path) => path.len() as i32,
                            None => continue, // Can't reach this plate
                        }
                    };

                    if distance < min_distance {
                        min_distance = distance;
                        closest_plate = Some(plate_pos);
                    }
                }

                // If we found at least one reachable plate, consider this player
                if let Some(plate_pos) = closest_plate {
                    // Update best player if this one is closer
                    best_player = match best_player {
                        None => Some((player_index, min_distance, plate_pos)),
                        Some((_, best_dist, _)) if min_distance < best_dist => {
                            Some((player_index, min_distance, plate_pos))
                        }
                        _ => best_player,
                    };
                }
            }

            // Assign the goal to the best player for this color
            if let Some((player_index, _, plate_pos)) = best_player {
                debug!(
                    "[FallbackPressurePlateStrategy] Player {} assigned to {:?} pressure plate at {:?} (within distance 4 of door) as fallback",
                    player_index + 1,
                    color,
                    plate_pos
                );
                goals[player_index] = Some(Goal::WaitOnTile(color, plate_pos));
            }
        }

        goals
    }
}
