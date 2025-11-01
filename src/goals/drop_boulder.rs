use tracing::debug;

use crate::goals::goal::ExecuteGoal;
use crate::goals::use_direction;
use crate::swoq_interface::DirectedAction;
use crate::types::Position;
use crate::world_state::WorldState;

pub struct DropBoulderGoal;

impl ExecuteGoal for DropBoulderGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        // Find a safe place to drop the boulder (empty adjacent tile)
        for neighbor in player_pos.neighbors() {
            if matches!(world.map.get(&neighbor), Some(crate::swoq_interface::Tile::Empty))
                && neighbor.x >= 0
                && neighbor.x < world.map.width
                && neighbor.y >= 0
                && neighbor.y < world.map.height
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
                    world.players[player_index].position.x,
                    world.players[player_index].position.y - 1,
                ),
                DirectedAction::MoveEast => Position::new(
                    world.players[player_index].position.x + 1,
                    world.players[player_index].position.y,
                ),
                DirectedAction::MoveSouth => Position::new(
                    world.players[player_index].position.x,
                    world.players[player_index].position.y + 1,
                ),
                DirectedAction::MoveWest => Position::new(
                    world.players[player_index].position.x - 1,
                    world.players[player_index].position.y,
                ),
                _ => continue,
            };
            if world.is_walkable(&next_pos, next_pos) {
                return Some(direction);
            }
        }
        None
    }
}
