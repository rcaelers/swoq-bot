use tracing::debug;

use crate::infra::Color;
use crate::planners::heuristic::goals::Goal;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::planners::heuristic::strategies::planner::{SelectGoal, StrategyType};

pub struct UsePressurePlateForDoorStrategy;

impl SelectGoal for UsePressurePlateForDoorStrategy {
    fn strategy_type(&self) -> StrategyType {
        StrategyType::Coop
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, state, current_goals),
        fields(strategy = "UsePressurePlateForDoorStrategy")
    )]
    fn try_select_coop(
        &mut self,
        state: &PlannerState,
        current_goals: &[Option<Goal>],
    ) -> Vec<Option<Goal>> {
        debug!("UsePressurePlateForDoorStrategy");
        let mut goals = vec![None; state.world.players.len()];

        // Iterate over each color
        for color in [Color::Red, Color::Green, Color::Blue] {
            // Get door positions for this color
            let door_positions = match state.world.doors.get_positions(color) {
                Some(pos) if !pos.is_empty() => pos,
                _ => continue,
            };

            // Get pressure plates for this color
            let plates = match state.world.pressure_plates.get_positions(color) {
                Some(plates) if !plates.is_empty() => plates,
                _ => continue,
            };

            // Find plates that are adjacent to doors of the same color
            let adjacent_plates: Vec<crate::infra::Position> = plates
                .iter()
                .copied()
                .filter(|plate_pos| {
                    plate_pos
                        .neighbors()
                        .iter()
                        .any(|neighbor| door_positions.contains(neighbor))
                })
                .collect();

            if adjacent_plates.is_empty() {
                continue;
            }

            debug!("Found {} {:?} pressure plates adjacent to doors", adjacent_plates.len(), color);

            let mut best_player: Option<(usize, i32, crate::infra::Position)> = None; // (player_index, distance, plate_pos)

            // Find the best player for this color
            for (player_index, player) in state.world.players.iter().enumerate() {
                // Skip if player already has a goal
                if player_index < current_goals.len() && current_goals[player_index].is_some() {
                    continue;
                }
                if goals[player_index].is_some() {
                    continue;
                }

                // Skip if player has a key for this color (prefer keys over plates)
                if state.world.has_key(player, color) {
                    continue;
                }

                // Find the closest reachable plate for this player
                let mut min_distance = i32::MAX;
                let mut closest_plate = None;

                for &plate_pos in &adjacent_plates {
                    let distance = if player.position.is_adjacent(&plate_pos) {
                        0 // Already adjacent
                    } else {
                        match state.world.find_path(player.position, plate_pos) {
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

            // Assign the goal to the best player
            if let Some((player_index, _, plate_pos)) = best_player {
                debug!(
                    "[UsePressurePlateForDoorStrategy] Player {} assigned to wait on {:?} pressure plate at {:?}",
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
