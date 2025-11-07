use crate::infra::{Color, Position};
use crate::planners::goap::planner_state::PlannerState;
use crate::state::WorldState;
use crate::swoq_interface::{DirectedAction, Inventory};

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct GetKeyAction {
    pub color: Color,
    pub key_pos: Position,
}

impl GOAPActionTrait for GetKeyAction {
    fn precondition(&self, state: &PlannerState, player_index: usize) -> bool {
        let world = &state.world;
        let player = &world.players[player_index];
        player.inventory == Inventory::None
            && world
                .keys
                .get_positions(self.color)
                .is_some_and(|positions| positions.contains(&self.key_pos))
            && world
                .find_path_for_player(player_index, player.position, self.key_pos)
                .is_some()
    }

    fn effect(&self, state: &mut PlannerState, player_index: usize) {
        state.world.players[player_index].inventory = match self.color {
            Color::Red => Inventory::KeyRed,
            Color::Green => Inventory::KeyGreen,
            Color::Blue => Inventory::KeyBlue,
        };
        state.world.players[player_index].position = self.key_pos;
        // Remove key from map (for planning simulation)
        state
            .world
            .map
            .insert(self.key_pos, crate::swoq_interface::Tile::Empty);
        // Remove key from world.keys
        state.world.keys.remove(self.color, self.key_pos);
    }

    fn execute(
        &self,
        world: &WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        execute_move_to(world, player_index, self.key_pos, execution_state)
    }

    fn cost(&self, state: &PlannerState, player_index: usize) -> f32 {
        10.0 + state
            .world
            .path_distance(state.world.players[player_index].position, self.key_pos)
            .unwrap_or(1000) as f32
            * 0.1
    }

    fn duration(&self, state: &PlannerState, player_index: usize) -> u32 {
        // Distance to key + 1 tick to pick it up
        state
            .world
            .path_distance(state.world.players[player_index].position, self.key_pos)
            .unwrap_or(1000) as u32
            + 1
    }

    fn reward(&self, state: &PlannerState, _player_index: usize) -> f32 {
        // High reward if matching door exists and is closed
        if let Some(doors) = state.world.doors.get_positions(self.color)
            && !doors.is_empty()
            && !state.world.is_door_open(self.color)
        {
            return 15.0; // Very high reward for keys that open closed doors
        }
        5.0 // Base reward for collecting keys
    }

    fn name(&self) -> &'static str {
        "GetKey"
    }

    fn generate(state: &PlannerState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;

        // Generate actions for all known keys of all colors
        for color in [Color::Red, Color::Green, Color::Blue] {
            if let Some(key_positions) = world.keys.get_positions(color) {
                for key_pos in key_positions {
                    let action = GetKeyAction {
                        color,
                        key_pos: *key_pos,
                    };
                    // Only include if precondition is satisfied
                    if action.precondition(state, player_index) {
                        actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
                    }
                }
            }
        }

        actions
    }
}
