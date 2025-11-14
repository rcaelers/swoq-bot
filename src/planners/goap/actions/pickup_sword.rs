use crate::infra::Position;
use crate::planners::goap::game_state::GameState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct PickupSwordAction {
    pub sword_pos: Position,
    pub cached_distance: u32,
}

impl GOAPActionTrait for PickupSwordAction {
    fn precondition(&self, state: &GameState, player_index: usize) -> bool {
        let world = &state.world;
        let player = &world.players[player_index];
        // Path reachability validated during generation
        world.swords.get_positions().contains(&self.sword_pos) && !player.has_sword
    }

    fn effect(&self, state: &mut GameState, player_index: usize) {
        state.world.players[player_index].has_sword = true;
        state.world.players[player_index].position = self.sword_pos;
        // Remove sword from tracker and map (for planning simulation)
        state.world.swords.remove(self.sword_pos);
        state
            .world
            .map
            .insert(self.sword_pos, crate::swoq_interface::Tile::Empty);
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        execute_move_to(world, player_index, self.sword_pos, execution_state)
    }

    fn cost(&self, _state: &GameState, _player_index: usize) -> f32 {
        10.0 + self.cached_distance as f32 * 0.1
    }

    fn duration(&self, _state: &GameState, _player_index: usize) -> u32 {
        self.cached_distance + 1 // +1 to pick it up
    }

    fn name(&self) -> &'static str {
        "PickupSword"
    }

    fn generate(state: &GameState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;
        let player = &world.players[player_index];

        for sword_pos in world.swords.get_positions() {
            if let Some(path) = world.find_path_for_player(player_index, player.position, *sword_pos) {
                let action = PickupSwordAction {
                    sword_pos: *sword_pos,
                    cached_distance: path.len() as u32,
                };
                if action.precondition(state, player_index) {
                    actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
                }
            }
        }

        actions
    }
}
