use tracing::debug;

use crate::infra::Position;
use crate::infra::path_to_action;
use crate::planners::heuristic::goals::goal::ExecuteGoal;
use super::super::pathfinding::find_path_for_player;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::swoq_interface::DirectedAction;

pub struct PickupHealthGoal(pub Position);

impl ExecuteGoal for PickupHealthGoal {
    fn execute(&self, state: &mut PlannerState, player_index: usize) -> Option<DirectedAction> {
        let player = &state.world.players[player_index];
        let player_pos = player.position;
        let health_pos = self.0;
        debug!("PickupHealth: going to destination {:?}", health_pos);
        state.world.players[player_index].current_destination = Some(health_pos);
        let path = find_path_for_player(&state.world, player_index, player_pos, health_pos)?;
        debug!("PickupHealth: path length={}", path.len());
        state.world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}
