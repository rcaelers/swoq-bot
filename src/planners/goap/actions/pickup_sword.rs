use crate::infra::Position;
use crate::planners::goap::planner_state::PlannerState;
use crate::state::WorldState;
use crate::swoq_interface::DirectedAction;

use super::helpers::execute_move_to;
use super::{ActionExecutionState, ExecutionStatus, GOAPActionTrait};

#[derive(Debug, Clone)]
pub struct PickupSwordAction {
    pub sword_pos: Position,
}

impl GOAPActionTrait for PickupSwordAction {
    fn precondition(&self, state: &PlannerState, player_index: usize) -> bool {
        let world = &state.world;
        let player = &world.players[player_index];
        world.swords.get_positions().contains(&self.sword_pos)
            && !player.has_sword
            && world
                .find_path_for_player(player_index, player.position, self.sword_pos)
                .is_some()
    }

    fn effect(&self, state: &mut PlannerState, player_index: usize) {
        state.world.players[player_index].has_sword = true;
        state.world.players[player_index].position = self.sword_pos;
    }

    fn execute(
        &self,
        world: &WorldState,
        player_index: usize,
        execution_state: &mut ActionExecutionState,
    ) -> (DirectedAction, ExecutionStatus) {
        execute_move_to(world, player_index, self.sword_pos, execution_state)
    }

    fn cost(&self, state: &PlannerState, player_index: usize) -> f32 {
        10.0 + state
            .world
            .path_distance(state.world.players[player_index].position, self.sword_pos)
            .unwrap_or(1000) as f32
            * 0.1
    }

    fn duration(&self, state: &PlannerState, player_index: usize) -> u32 {
        // Distance to sword + 1 tick to pick it up
        state
            .world
            .path_distance(state.world.players[player_index].position, self.sword_pos)
            .unwrap_or(1000) as u32
            + 1
    }

    fn reward(&self, state: &PlannerState, _player_index: usize) -> f32 {
        // High reward if there are enemies present
        if !state.world.enemies.is_empty() {
            18.0 // Very high reward when enemies exist
        } else {
            8.0 // Still good to pick up for potential future enemies
        }
    }

    fn name(&self) -> &'static str {
        "PickupSword"
    }

    fn generate(state: &PlannerState, player_index: usize) -> Vec<Box<dyn GOAPActionTrait>> {
        let mut actions = Vec::new();
        let world = &state.world;

        for sword_pos in world.swords.get_positions() {
            let action = PickupSwordAction {
                sword_pos: *sword_pos,
            };
            if action.precondition(state, player_index) {
                actions.push(Box::new(action) as Box<dyn GOAPActionTrait>);
            }
        }

        actions
    }
}
