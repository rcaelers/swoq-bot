use crate::planners::heuristic::goals::goal::ExecuteGoal;
use crate::planners::heuristic::goals::path_to_action;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::swoq_interface::DirectedAction;

pub struct ReachExitGoal;

impl ExecuteGoal for ReachExitGoal {
    fn execute(
        &self,
        state: &mut PlannerState,
        player_index: usize,
    ) -> Option<DirectedAction> {
        let player = &state.world.players[player_index];
        let player_position = player.position;
        let exit_position = state.world.exit_position?;
        state.world.players[player_index].current_destination = Some(exit_position);
        let path =
            state
                .world
                .find_path_for_player(player_index, player_position, exit_position)?;
        state.world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_position, &path)
    }
}
