use crate::infra::Position;
use crate::planners::goap::planner_state::PlannerState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct ReachExitAction {
    pub exit_pos: Position,
}

impl GOAPActionTrait for ReachExitAction {
    fn precondition(&self, state: &PlannerState, player_index: usize) -> bool {
        let world = &state.world;
        let player = &world.players[player_index];
        world.exit_position == Some(self.exit_pos)
            && world
                .find_path_for_player(player_index, player.position, self.exit_pos)
                .is_some()
    }

    fn effect(&self, state: &mut PlannerState, player_index: usize) {
        state.world.players[player_index].position = self.exit_pos;
    }

    fn execute(
        &self,
        world: &WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        execute_move_to(world, player_index, self.exit_pos, execution_state)
    }

    fn cost(&self, state: &PlannerState, player_index: usize) -> f32 {
        state
            .world
            .path_distance(state.world.players[player_index].position, self.exit_pos)
            .unwrap_or(1000) as f32
            * 0.1
    }

    fn duration(&self, state: &PlannerState, player_index: usize) -> u32 {
        // Distance to exit
        state
            .world
            .path_distance(state.world.players[player_index].position, self.exit_pos)
            .unwrap_or(1000) as u32
    }

    fn reward(&self, state: &PlannerState, _player_index: usize) -> f32 {
        // Highest reward when enemies are cleared, otherwise low
        if state.world.enemies.is_empty() {
            50.0 // Maximum reward - this is the goal!
        } else {
            5.0 // Low reward if enemies still present
        }
    }

    fn name(&self) -> &'static str {
        "ReachExit"
    }

    fn is_terminal(&self) -> bool {
        true
    }

    fn generate(state: &PlannerState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;

        if let Some(exit_pos) = world.exit_position {
            let action = ReachExitAction { exit_pos };
            if action.precondition(state, player_index) {
                actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
            }
        }

        actions
    }
}
