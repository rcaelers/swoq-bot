use tracing::debug;

use crate::goals::goal::ExecuteGoal;
use crate::goals::{path_to_action, use_direction};
use crate::swoq_interface::DirectedAction;
use crate::types::Color;
use crate::world_state::WorldState;

pub struct OpenDoorGoal(pub Color);

impl ExecuteGoal for OpenDoorGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        let door_positions = world.doors.get_positions(self.0)?;

        // OpenDoor is only for keys - if we don't have a key, this shouldn't be selected
        if !world.has_key(&world.players[player_index], self.0) {
            debug!("OpenDoor goal but no {:?} key!", self.0);
            return None;
        }

        // Find the closest reachable door by finding the best empty neighbor
        let mut best_target: Option<(crate::types::Position, crate::types::Position, usize)> = None; // (door_pos, neighbor_pos, path_len)

        for &door_pos in door_positions {
            // Check each neighbor of the door
            for neighbor in door_pos.neighbors() {
                // Only consider empty, walkable neighbors (or player position)
                if neighbor != player_pos
                    && !matches!(world.map.get(&neighbor), Some(crate::swoq_interface::Tile::Empty))
                {
                    continue;
                }

                if !world.is_walkable(&neighbor, neighbor) {
                    continue;
                }

                // Try to path to this neighbor
                if let Some(path) = world.find_path(player_pos, neighbor) {
                    let path_len = path.len();
                    if best_target.is_none() || path_len < best_target.unwrap().2 {
                        best_target = Some((door_pos, neighbor, path_len));
                    }
                }
            }
        }

        let (door_pos, target_pos, _) = best_target?;

        // If the door is adjacent to us, use the key on it
        if player_pos.is_adjacent(&door_pos) {
            debug!("Door is adjacent, using key on door at {:?}", door_pos);
            return Some(use_direction(player_pos, door_pos));
        }

        // Navigate to the empty neighbor of the door
        debug!("Navigating to neighbor {:?} of door at {:?}", target_pos, door_pos);
        world.players[player_index].current_destination = Some(target_pos);
        let path = world.find_path_for_player(player_index, player_pos, target_pos)?;
        world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}
