//! ReachExit action - move to the exit to complete the level

use crate::infra::Position;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ActionType, ExecutionStatus, RLActionTrait};

#[derive(Debug, Clone)]
pub struct ReachExitAction {
    pub exit_pos: Position,
    pub cached_distance: u32,
}

impl RLActionTrait for ReachExitAction {
    fn precondition(&self, world: &WorldState, player_index: usize) -> bool {
        let player = &world.players[player_index];

        // Player already at exit or has exited
        if player.position == Position::new(-1, -1) || player.position == self.exit_pos {
            return false;
        }

        // Player must have empty inventory
        if player.inventory != crate::swoq_interface::Inventory::None {
            return false;
        }

        // Exit must exist
        if world.exit_position != Some(self.exit_pos) {
            return false;
        }

        // If there are 2 players, both must be able to reach the exit
        if world.players.len() == 2 {
            let other_player_index = 1 - player_index;
            let other_player = &world.players[other_player_index];

            if world
                .find_path(other_player.position, self.exit_pos)
                .is_none()
            {
                return false;
            }
        }

        world.find_path(player.position, self.exit_pos).is_some()
    }

    fn prepare(&mut self, world: &mut WorldState, player_index: usize) -> Option<Position> {
        let player = &world.players[player_index];

        if world.players.len() == 2 {
            let other_player_index = 1 - player_index;
            let other_player = &world.players[other_player_index];

            if other_player.is_active {
                world.find_path(other_player.position, self.exit_pos)?;
            }
        }

        if world.find_path(player.position, self.exit_pos).is_some() {
            Some(self.exit_pos)
        } else {
            None
        }
    }

    fn execute(
        &self,
        world: &mut WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        execute_move_to(world, player_index, self.exit_pos, execution_state)
    }

    fn name(&self) -> String {
        "ReachExit".to_string()
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn action_type_index(&self) -> usize {
        ActionType::ReachExit as usize
    }

    fn target_position(&self) -> Option<Position> {
        Some(self.exit_pos)
    }

    fn generate(world: &WorldState, player_index: usize) -> Vec<Box<dyn RLActionTrait>> {
        let mut actions = Vec::new();
        let player = &world.players[player_index];

        // Don't generate if player is already at exit position or has exited
        if player.position == Position::new(-1, -1) {
            return actions;
        }

        if let Some(exit_pos) = world.exit_position {
            if player.position == exit_pos {
                return actions;
            }

            if let Some(path) = world.find_path(player.position, exit_pos) {
                let action = ReachExitAction {
                    exit_pos,
                    cached_distance: path.len() as u32,
                };
                if action.precondition(world, player_index) {
                    actions.push(Box::new(action) as Box<dyn RLActionTrait>);
                }
            }
        }

        actions
    }
}
