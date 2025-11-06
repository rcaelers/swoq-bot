use tracing::debug;

use crate::planners::heuristic::goals::goal::ExecuteGoal;
use crate::planners::heuristic::goals::path_to_action;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::swoq_interface::DirectedAction;
use crate::infra::Position;

pub struct PickupHealthGoal(pub Position);

impl ExecuteGoal for PickupHealthGoal {
    fn execute(&self, state: &mut PlannerState, player_index: usize) -> Option<DirectedAction> {
        let player = &state.world.players[player_index];
        let player_pos = player.position;
        let health_pos = self.0;
        debug!("PickupHealth: going to destination {:?}", health_pos);
        state.world.players[player_index].current_destination = Some(health_pos);
        let path = state
            .world
            .find_path_for_player(player_index, player_pos, health_pos)?;
        debug!("PickupHealth: path length={}", path.len());
        state.world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}
