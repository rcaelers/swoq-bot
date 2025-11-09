use crate::infra::{Color, Position};
use crate::planners::goap::planner_state::PlannerState;
use crate::state::WorldState;
use crate::swoq_interface::{DirectedAction, Inventory};

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct TouchPlateAction {
    pub plate_pos: Position,
    pub plate_color: Color,
    pub cached_distance: u32, // Cached path distance to plate
}

impl GOAPActionTrait for TouchPlateAction {
    fn precondition(&self, state: &PlannerState, player_index: usize) -> bool {
        let world = &state.world;
        let player = &world.players[player_index];

        // Only when player has nothing else to do:
        // - Empty inventory
        // - No unexplored frontier
        if player.inventory != Inventory::None {
            return false;
        }

        // No unexplored frontier
        if !player.unexplored_frontier.is_empty() {
            return false;
        }

        // Plate must exist
        if let Some(positions) = world.pressure_plates.get_positions(self.plate_color) {
            positions.contains(&self.plate_pos)
        } else {
            false
        }
    }

    fn effect(&self, state: &mut PlannerState, player_index: usize) {
        // Move player to plate position
        state.world.players[player_index].position = self.plate_pos;
        // Track that we touched a plate of this color (only counts once per color)
        state.plates_touched.insert(self.plate_color);
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        let player = &world.players[player_index];
        let player_pos = player.position;

        // If we're already on the plate, wait 2 ticks before completing
        if player_pos == self.plate_pos {
            execution_state.wait_ticks += 1;

            if execution_state.wait_ticks >= 2 {
                // We've waited 2 ticks, now complete
                world.plates_touched.insert(self.plate_color);
                tracing::info!("Recorded plate touch: {:?}", self.plate_color);
                return (DirectedAction::None, ExecutionStatus::Complete);
            } else {
                // Still waiting
                return (DirectedAction::None, ExecutionStatus::InProgress);
            }
        }

        // Navigate to the plate position
        execute_move_to(world, player_index, self.plate_pos, execution_state)
    }

    fn cost(&self, _state: &PlannerState, _player_index: usize) -> f32 {
        // Low cost with small bonus to encourage this when idle
        self.cached_distance as f32 * 0.1
    }

    fn duration(&self, _state: &PlannerState, _player_index: usize) -> u32 {
        // Distance to reach plate + 2 ticks to stand on it
        self.cached_distance + 2
    }

    fn name(&self) -> &'static str {
        "TouchPlate"
    }

    fn generate(state: &PlannerState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;
        let player = &world.players[player_index];

        // Only generate if player has nothing to do
        if player.inventory != Inventory::None {
            return actions;
        }

        // Check if there's unexplored frontier
        if !player.unexplored_frontier.is_empty() {
            return actions;
        }

        // Check if there are any boulders available
        if !world.boulders.get_all_positions().is_empty() {
            return actions;
        }

        // Generate action for each pressure plate color that hasn't been touched yet
        for &color in &[Color::Red, Color::Green, Color::Blue] {
            // Skip if we've already touched this color
            if state.plates_touched.contains(&color) {
                continue;
            }

            if let Some(plate_positions) = world.pressure_plates.get_positions(color) {
                for &plate_pos in plate_positions {
                    // Check if we can reach the plate position directly
                    if let Some(path) =
                        world.find_path_for_player(player_index, player.position, plate_pos)
                    {
                        let distance = path.len() as u32;

                        let action = TouchPlateAction {
                            plate_pos,
                            plate_color: color,
                            cached_distance: distance,
                        };

                        if action.precondition(state, player_index) {
                            actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
                        }
                    }
                }
            }
        }

        actions
    }
}
