use tracing::debug;

use crate::goals::goal::ExecuteGoal;
use crate::goals::{path_to_action, use_direction};
use crate::swoq_interface::DirectedAction;
use crate::types::Position;
use crate::world_state::WorldState;

pub struct FetchBoulderGoal(pub Position);

impl ExecuteGoal for FetchBoulderGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        let boulder_pos = self.0;

        // If we're already adjacent, pick up the boulder
        if player_pos.is_adjacent(&boulder_pos) {
            debug!("Picking up boulder at {:?}", boulder_pos);
            return Some(use_direction(player_pos, boulder_pos));
        }

        // Navigate to an adjacent walkable position next to the boulder
        for adjacent in boulder_pos.neighbors() {
            if world.map.is_walkable(&adjacent, adjacent)
                && let Some(path) = world.find_path_for_player(player_index, player_pos, adjacent)
            {
                debug!("Moving to adjacent position {:?} to reach boulder", adjacent);
                world.players[player_index].current_destination = Some(adjacent);
                world.players[player_index].current_path = Some(path.clone());
                return path_to_action(player_pos, &path);
            }
        }
        debug!("No walkable position adjacent to boulder at {:?}", boulder_pos);
        None
    }
}
