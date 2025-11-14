use crate::infra::Position;
use crate::planners::goap::game_state::GameState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct ReachExitAction {
    pub exit_pos: Position,
    pub cached_distance: u32,
}

impl GOAPActionTrait for ReachExitAction {
    fn precondition(&self, state: &GameState, player_index: usize) -> bool {
        let world = &state.world;
        let player = &world.players[player_index];

        // Player already at exit or has exited
        if player.position == Position::new(-1, -1) || player.position == self.exit_pos {
            return false;
        }

        // Player must have empty inventory and path reachability validated during generation
        if player.inventory != crate::swoq_interface::Inventory::None
            || world.exit_position != Some(self.exit_pos)
        {
            return false;
        }

        // If there are 2 players, both must be able to reach the exit
        if world.players.len() == 2 {
            let other_player_index = 1 - player_index;
            let other_player = &world.players[other_player_index];

            // Check if other player can reach the exit
            if world
                .find_path_for_player(other_player_index, other_player.position, self.exit_pos)
                .is_none()
            {
                return false;
            }
        }

        true
    }

    fn effect(&self, state: &mut GameState, player_index: usize) {
        state.world.players[player_index].position = self.exit_pos;
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        execute_move_to(world, player_index, self.exit_pos, execution_state)
    }

    fn cost(&self, _state: &GameState, _player_index: usize) -> f32 {
        self.cached_distance as f32 * 0.1
    }

    fn duration(&self, _state: &GameState, _player_index: usize) -> u32 {
        self.cached_distance
    }

    fn name(&self) -> &'static str {
        "ReachExit"
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn generate(state: &GameState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;
        let player = &world.players[player_index];

        // Don't generate if player is already at exit position or has exited (-1, -1)
        if player.position == Position::new(-1, -1) {
            return actions;
        }

        if let Some(exit_pos) = world.exit_position {
            // Don't generate if player is already at the exit
            if player.position == exit_pos {
                return actions;
            }

            if let Some(path) = world.find_path_for_player(player_index, player.position, exit_pos)
            {
                let action = ReachExitAction {
                    exit_pos,
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
