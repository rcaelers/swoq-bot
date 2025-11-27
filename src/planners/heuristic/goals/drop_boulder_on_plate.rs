use tracing::debug;

use crate::infra::Position;
use crate::infra::{path_to_action, use_direction};
use crate::planners::heuristic::goals::goal::ExecuteGoal;
use super::super::pathfinding::find_path_for_player;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::swoq_interface::DirectedAction;

pub struct DropBoulderOnPlateGoal(pub Position);

impl ExecuteGoal for DropBoulderOnPlateGoal {
    fn execute(&self, state: &mut PlannerState, player_index: usize) -> Option<DirectedAction> {
        let player = &state.world.players[player_index];
        let player_pos = player.position;
        let plate_pos = self.0;

        // If we're adjacent to the pressure plate, drop the boulder on it
        if player_pos.is_adjacent(&plate_pos) {
            debug!("Dropping boulder on pressure plate at {:?}", plate_pos);
            return Some(use_direction(player_pos, plate_pos));
        }

        // Navigate to the pressure plate
        state.world.players[player_index].current_destination = Some(plate_pos);
        let path = find_path_for_player(&state.world, player_index, player_pos, plate_pos)?;
        state.world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}
