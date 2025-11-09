use crate::infra::{Color, Position, use_direction};
use crate::planners::goap::planner_state::PlannerState;
use crate::state::WorldState;
use crate::swoq_interface::{DirectedAction, Inventory};

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct DropBoulderOnPlateAction {
    pub plate_pos: Position,
    pub plate_color: Color,
    pub target_adjacent_pos: Position, // Adjacent position where player will stand to drop on plate
    pub cached_distance: u32,          // Cached path distance to target_adjacent_pos
}

impl GOAPActionTrait for DropBoulderOnPlateAction {
    fn precondition(&self, state: &PlannerState, player_index: usize) -> bool {
        let world = &state.world;
        let player = &world.players[player_index];

        // Player must have boulder and pressure plate must exist
        // Path reachability validated during generation
        player.inventory == Inventory::Boulder
            && world
                .pressure_plates
                .get_positions(self.plate_color)
                .is_some_and(|positions| positions.contains(&self.plate_pos))
    }

    fn effect(&self, state: &mut PlannerState, player_index: usize) {
        // Drop the boulder
        state.world.players[player_index].inventory = Inventory::None;
        // Place boulder on pressure plate
        state
            .world
            .map
            .insert(self.plate_pos, crate::swoq_interface::Tile::Boulder);
        state.world.boulders.add_boulder(self.plate_pos, true); // Mark as moved
        // Move player to the pre-determined adjacent position
        state.world.players[player_index].position = self.target_adjacent_pos;
        // Clear boulder tracking
        state.player_states[player_index].boulder_is_unexplored = None;
    }

    fn execute(
        &self,
        world: &WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        let player = &world.players[player_index];
        let player_pos = player.position;

        // If we're at the target adjacent position, drop the boulder on the plate
        if player_pos == self.target_adjacent_pos {
            let action = use_direction(player_pos, self.plate_pos);
            return (action, ExecutionStatus::Complete);
        }

        // Navigate to the target adjacent position
        execute_move_to(world, player_index, self.target_adjacent_pos, execution_state)
    }

    fn cost(&self, _state: &PlannerState, _player_index: usize) -> f32 {
        1.0 + self.cached_distance as f32 * 0.1
    }

    fn duration(&self, _state: &PlannerState, _player_index: usize) -> u32 {
        self.cached_distance + 1 // +1 for dropping
    }

    fn name(&self) -> &'static str {
        "DropBoulderOnPlate"
    }

    fn generate(state: &PlannerState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;
        let player = &world.players[player_index];

        // Only generate if player has a boulder
        if player.inventory != Inventory::Boulder {
            return actions;
        }

        // Generate actions for all pressure plates
        for &plate_color in &[Color::Red, Color::Green, Color::Blue] {
            if let Some(plate_positions) = world.pressure_plates.get_positions(plate_color) {
                for &plate_pos in plate_positions {
                    // Find the closest reachable adjacent position
                    let mut best_option: Option<(Position, u32)> = None;
                    
                    for &adj in plate_pos.neighbors().iter() {
                        if world.is_walkable(&adj, adj)
                            && let Some(path) = world.find_path_for_player(player_index, player.position, adj) {
                                let distance = path.len() as u32;
                                if best_option.is_none() || distance < best_option.unwrap().1 {
                                    best_option = Some((adj, distance));
                                }
                            }
                    }
                    
                    if let Some((target_adjacent_pos, cached_distance)) = best_option {
                        let action = DropBoulderOnPlateAction {
                            plate_pos,
                            plate_color,
                            target_adjacent_pos,
                            cached_distance,
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
