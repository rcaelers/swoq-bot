use tracing::debug;

use crate::planners::heuristic::goals::goal::ExecuteGoal;
use crate::infra::path_to_action;
use crate::planners::heuristic::planner_state::PlannerState;
use crate::swoq_interface::DirectedAction;
use crate::infra::Position;

pub struct PassThroughDoorGoal {
    pub door_pos: Position,
    pub target_pos: Position,
}

impl PassThroughDoorGoal {
    pub fn new(door_pos: Position, target_pos: Position) -> Self {
        Self {
            door_pos,
            target_pos,
        }
    }
}

impl ExecuteGoal for PassThroughDoorGoal {
    fn execute(
        &self,
        state: &mut PlannerState,
        player_index: usize,
    ) -> Option<DirectedAction> {
        let player = &state.world.players[player_index];
        let player_pos = player.position;
        let neighbor_pos = self.door_pos; // Position adjacent to door
        let target_pos = self.target_pos; // Position beyond door

        // Calculate door position from neighbor and target
        // Door is between neighbor and target
        let dx = target_pos.x - neighbor_pos.x;
        let dy = target_pos.y - neighbor_pos.y;
        let door_pos = Position {
            x: neighbor_pos.x + dx / 2,
            y: neighbor_pos.y + dy / 2,
        };

        // Check if the door is currently closed (not walkable)
        let door_tile = state.world.map.get(&door_pos);
        let door_closed = !matches!(door_tile, Some(crate::swoq_interface::Tile::Empty));

        // If we've reached the target position, we're done
        if player_pos == target_pos {
            debug!("Reached target position {:?} beyond door at {:?}", target_pos, door_pos);
            return Some(DirectedAction::None);
        }

        // If we're on the door position, take one more step toward target
        if player_pos == door_pos {
            debug!("On door at {:?}, moving to target {:?}", door_pos, target_pos);
            let dx = target_pos.x - player_pos.x;
            let dy = target_pos.y - player_pos.y;
            return Some(if dy < 0 {
                DirectedAction::MoveNorth
            } else if dy > 0 {
                DirectedAction::MoveSouth
            } else if dx > 0 {
                DirectedAction::MoveEast
            } else {
                DirectedAction::MoveWest
            });
        }

        // If we're at the neighbor position (adjacent to door)
        if player_pos == neighbor_pos {
            if door_closed {
                debug!("At neighbor {:?}, door at {:?} is closed, waiting", neighbor_pos, door_pos);
                return Some(DirectedAction::None);
            } else {
                debug!(
                    "At neighbor {:?}, door at {:?} is open, stepping onto it",
                    neighbor_pos, door_pos
                );
                let dx = door_pos.x - player_pos.x;
                let dy = door_pos.y - player_pos.y;
                return Some(if dy < 0 {
                    DirectedAction::MoveNorth
                } else if dy > 0 {
                    DirectedAction::MoveSouth
                } else if dx > 0 {
                    DirectedAction::MoveEast
                } else {
                    DirectedAction::MoveWest
                });
            }
        }

        // Otherwise, navigate to the neighbor position first
        if let Some(path) = state
            .world
            .find_path_for_player(player_index, player_pos, neighbor_pos)
        {
            debug!("Navigating to neighbor {:?} before door at {:?}", neighbor_pos, door_pos);
            state.world.players[player_index].current_destination = Some(neighbor_pos);
            state.world.players[player_index].current_path = Some(path.clone());
            return path_to_action(player_pos, &path);
        }

        debug!("Cannot find path to neighbor {:?} for door at {:?}", neighbor_pos, door_pos);
        None
    }
}
