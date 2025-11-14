use crate::infra::{Color, Position};
use crate::planners::goap::game_state::GameState;
use crate::state::WorldState;
use crate::swoq_interface::{DirectedAction, Inventory};

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct GetKeyAction {
    pub color: Color,
    pub key_pos: Position,
    pub cached_distance: u32,
}

impl GOAPActionTrait for GetKeyAction {
    fn precondition(&self, state: &GameState, player_index: usize) -> bool {
        let world = &state.world;
        let player = &world.players[player_index];
        // Path reachability validated during generation
        player.inventory == Inventory::None
            && world
                .keys
                .get_positions(self.color)
                .is_some_and(|positions| positions.contains(&self.key_pos))
    }

    fn effect(&self, state: &mut GameState, player_index: usize) {
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
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        execute_move_to(world, player_index, self.key_pos, execution_state)
    }

    fn cost(&self, _state: &GameState, _player_index: usize) -> f32 {
        10.0 + self.cached_distance as f32 * 0.1
    }

    fn duration(&self, _state: &GameState, _player_index: usize) -> u32 {
        self.cached_distance + 1 // +1 to pick it up
    }

    fn name(&self) -> &'static str {
        "GetKey"
    }

    fn generate(state: &GameState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;
        let player = &world.players[player_index];

        // Generate actions for all known keys of all colors
        for color in [Color::Red, Color::Green, Color::Blue] {
            if let Some(key_positions) = world.keys.get_positions(color) {
                for key_pos in key_positions {
                    // Check path and cache distance
                    if let Some(path) =
                        world.find_path_for_player(player_index, player.position, *key_pos)
                    {
                        let action = GetKeyAction {
                            color,
                            key_pos: *key_pos,
                            cached_distance: path.len() as u32,
                        };
                        // Check other preconditions
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
