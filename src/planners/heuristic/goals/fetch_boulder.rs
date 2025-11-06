use tracing::debug;

use crate::infra::Position;
use crate::infra::{path_to_action, use_direction};
use crate::planners::heuristic::goals::goal::ExecuteGoal;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::swoq_interface::DirectedAction;

pub struct FetchBoulderGoal(pub Position);

impl ExecuteGoal for FetchBoulderGoal {
    fn execute(&self, state: &mut PlannerState, player_index: usize) -> Option<DirectedAction> {
        let player = &state.world.players[player_index];
        let player_pos = player.position;
        let boulder_pos = self.0;

        // If we're already adjacent, pick up the boulder
        if player_pos.is_adjacent(&boulder_pos) {
            debug!("Picking up boulder at {:?}", boulder_pos);
            return Some(use_direction(player_pos, boulder_pos));
        }

        // Navigate to an adjacent walkable position next to the boulder
        for adjacent in boulder_pos.neighbors() {
            if state.world.is_walkable(&adjacent, adjacent)
                && let Some(path) =
                    state
                        .world
                        .find_path_for_player(player_index, player_pos, adjacent)
            {
                debug!("Moving to adjacent position {:?} to reach boulder", adjacent);
                state.world.players[player_index].current_destination = Some(adjacent);
                state.world.players[player_index].current_path = Some(path.clone());
                return path_to_action(player_pos, &path);
            }
        }
        debug!("No walkable position adjacent to boulder at {:?}", boulder_pos);
        None
    }
}
