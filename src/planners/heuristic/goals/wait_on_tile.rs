use tracing::debug;

use crate::planners::heuristic::goals::goal::ExecuteGoal;
use crate::infra::path_to_action;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::swoq_interface::DirectedAction;
use crate::infra::Position;

pub struct WaitOnTileGoal(pub Position);

impl ExecuteGoal for WaitOnTileGoal {
    fn execute(
        &self,
        state: &mut PlannerState,
        player_index: usize,
    ) -> Option<DirectedAction> {
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
                state
                    .world
                    .find_path_for_player(player_index, player_position, tile_position)?;
            state.world.players[player_index].current_path = Some(path.clone());
            path_to_action(player_position, &path)
        }
    }
}
