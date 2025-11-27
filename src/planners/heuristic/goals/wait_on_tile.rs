use tracing::debug;

use crate::infra::Position;
use crate::infra::path_to_action;
use crate::planners::heuristic::goals::goal::ExecuteGoal;
use super::super::pathfinding::find_path_for_player;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::swoq_interface::DirectedAction;

pub struct WaitOnTileGoal(pub Position);

impl ExecuteGoal for WaitOnTileGoal {
    fn execute(&self, state: &mut PlannerState, player_index: usize) -> Option<DirectedAction> {
        let player = &state.world.players[player_index];
        let player_position = player.position;
        let tile_position = self.0;

        if player_position == tile_position {
            // Already on the tile - wait (do nothing)
            debug!("Waiting on tile at {:?}", tile_position);
            Some(DirectedAction::None)
        } else {
            // Navigate to the tile using collision-aware pathfinding
            debug!("Navigating to tile at {:?} to wait", tile_position);
            state.world.players[player_index].current_destination = Some(tile_position);
            let path =
                find_path_for_player(&state.world, player_index, player_position, tile_position)?;
            state.world.players[player_index].current_path = Some(path.clone());
            path_to_action(player_position, &path)
        }
    }
}
