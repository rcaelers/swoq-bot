use crate::planners::heuristic::goals::goal::ExecuteGoal;
use crate::planners::heuristic::goals::path_to_action;
use crate::planners::heuristic::goals::validate_destination;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::swoq_interface::DirectedAction;

pub struct PickupSwordGoal;

impl ExecuteGoal for PickupSwordGoal {
    fn execute(&self, state: &mut PlannerState, player_index: usize) -> Option<DirectedAction> {
        let player_pos = state.world.players[player_index].position;
        let sword_pos = state
            .world
            .closest_sword(&state.world.players[player_index])?;

        validate_destination(state, player_index);

        // Compute new path
        state.world.players[player_index].current_destination = Some(sword_pos);
        let path = state
            .world
            .find_path_for_player(player_index, player_pos, sword_pos)?;
        state.world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}
