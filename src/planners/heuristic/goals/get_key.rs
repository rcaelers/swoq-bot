use super::super::pathfinding::find_path_for_player;
use crate::infra::Color;
use crate::infra::path_to_action;
use crate::planners::heuristic::goals::goal::ExecuteGoal;
use crate::planners::heuristic::goals::validate_destination;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::swoq_interface::DirectedAction;

pub struct GetKeyGoal(pub Color);

impl ExecuteGoal for GetKeyGoal {
    fn execute(&self, state: &mut PlannerState, player_index: usize) -> Option<DirectedAction> {
        let player_pos = state.world.players[player_index].position;
        let key_pos = state
            .world
            .closest_key(&state.world.players[player_index], self.0)?;

        validate_destination(state, player_index);

        // Compute new path
        state.world.players[player_index].current_destination = Some(key_pos);
        let path = find_path_for_player(&state.world, player_index, player_pos, key_pos)?;
        state.world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}
