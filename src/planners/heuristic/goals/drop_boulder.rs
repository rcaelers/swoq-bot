use tracing::debug;

use crate::planners::heuristic::goals::goal::ExecuteGoal;
use crate::infra::use_direction;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::swoq_interface::DirectedAction;
use crate::infra::Position;

pub struct DropBoulderGoal;

impl ExecuteGoal for DropBoulderGoal {
    fn execute(
        &self,
        state: &mut PlannerState,
        player_index: usize,
    ) -> Option<DirectedAction> {
        let player = &state.world.players[player_index];
        let player_pos = player.position;
        // Find a safe place to drop the boulder (empty adjacent tile)
        for neighbor in player_pos.neighbors() {
            if matches!(state.world.map.get(&neighbor), Some(crate::swoq_interface::Tile::Empty))
                && neighbor.x >= 0
                && neighbor.x < state.world.map.width
                && neighbor.y >= 0
                && neighbor.y < state.world.map.height
            {
                debug!("Dropping boulder at {:?}", neighbor);
                return Some(use_direction(player_pos, neighbor));
            }
        }
        // Can't drop anywhere, try to move to find a drop location
        debug!("No adjacent empty tile to drop boulder, trying to move");
        // Try to move in any direction
        for direction in [
            DirectedAction::MoveNorth,
            DirectedAction::MoveEast,
            DirectedAction::MoveSouth,
            DirectedAction::MoveWest,
        ] {
            // Check if the direction is walkable
            let next_pos = match direction {
                DirectedAction::MoveNorth => Position::new(
                    state.world.players[player_index].position.x,
                    state.world.players[player_index].position.y - 1,
                ),
                DirectedAction::MoveEast => Position::new(
                    state.world.players[player_index].position.x + 1,
                    state.world.players[player_index].position.y,
                ),
                DirectedAction::MoveSouth => Position::new(
                    state.world.players[player_index].position.x,
                    state.world.players[player_index].position.y + 1,
                ),
                DirectedAction::MoveWest => Position::new(
                    state.world.players[player_index].position.x - 1,
                    state.world.players[player_index].position.y,
                ),
                _ => continue,
            };
            if state.world.is_walkable(&next_pos, None) {
                return Some(direction);
            }
        }
        None
    }
}
