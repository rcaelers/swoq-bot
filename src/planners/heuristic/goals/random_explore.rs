use tracing::debug;

use crate::infra::Position;
use crate::infra::path_to_action;
use crate::planners::heuristic::goals::goal::ExecuteGoal;
use super::super::pathfinding::find_path_for_player;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::swoq_interface::DirectedAction;

pub struct RandomExploreGoal(pub Position);

impl ExecuteGoal for RandomExploreGoal {
    fn execute(&self, state: &mut PlannerState, player_index: usize) -> Option<DirectedAction> {
        let player = &state.world.players[player_index];
        let player_position = player.position;
        let target_position = self.0;

        debug!("Random exploring to {:?}", target_position);

        // Try to path to the random position
        state.world.players[player_index].current_destination = Some(target_position);
        let path =
            find_path_for_player(&state.world, player_index, player_position, target_position)?;
        state.world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_position, &path)
    }
}
