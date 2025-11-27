use tracing::debug;

use super::super::pathfinding::find_path_for_player;
use crate::infra::path_to_action;
use crate::planners::heuristic::goals::goal::ExecuteGoal;
use crate::planners::heuristic::goals::{try_keep_destination, validate_destination};
use crate::planners::heuristic::planner_state::PlannerState;
use crate::swoq_interface::DirectedAction;

pub struct ExploreGoal;

impl ExecuteGoal for ExploreGoal {
    fn execute(&self, state: &mut PlannerState, player_index: usize) -> Option<DirectedAction> {
        let player_pos = state.world.players[player_index].position;

        // Step 1: Validate destination
        validate_destination(state, player_index);

        // Step 2: Try to reuse existing destination
        if try_keep_destination(state, player_index) {
            return path_to_action(
                player_pos,
                state.world.players[player_index].current_path.as_ref()?,
            );
        }

        // Step 3: Search for new frontier destination
        let sorted_frontier = &state.world.players[player_index].sorted_unexplored();
        debug!("Searching for new frontier destination from {} tiles", sorted_frontier.len());
        let mut attempts = 0;
        for (i, target) in sorted_frontier.iter().enumerate() {
            if i < 5 {
                debug!(
                    "  Trying frontier #{}: {:?}, distance={}",
                    i,
                    target,
                    player_pos.distance(target)
                );
            }
            attempts += 1;
            if let Some(path) =
                find_path_for_player(&state.world, player_index, player_pos, *target)
            {
                debug!(
                    "New frontier destination: {:?}, path length={} (tried {} tiles)",
                    target,
                    path.len(),
                    attempts
                );
                state.world.players[player_index].current_destination = Some(*target);
                state.world.players[player_index].current_path = Some(path.clone());
                return path_to_action(player_pos, &path);
            }
        }
        debug!(
            "No reachable frontier tiles found out of {} candidates (tried {} tiles)",
            sorted_frontier.len(),
            attempts
        );
        state.world.players[player_index].current_destination = None;
        state.world.players[player_index].current_path = None;

        None
    }
}
