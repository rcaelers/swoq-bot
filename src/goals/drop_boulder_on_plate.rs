use tracing::debug;

use crate::goals::goal::ExecuteGoal;
use crate::goals::{path_to_action, use_direction};
use crate::swoq_interface::DirectedAction;
use crate::types::Position;
use crate::world_state::WorldState;

pub struct DropBoulderOnPlateGoal(pub Position);

impl ExecuteGoal for DropBoulderOnPlateGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_pos = player.position;
        let plate_pos = self.0;

        // If we're adjacent to the pressure plate, drop the boulder on it
        if player_pos.is_adjacent(&plate_pos) {
            debug!("Dropping boulder on pressure plate at {:?}", plate_pos);
            return Some(use_direction(player_pos, plate_pos));
        }

        // Navigate to the pressure plate
        world.players[player_index].current_destination = Some(plate_pos);
        let path = world.find_path_for_player(player_index, player_pos, plate_pos)?;
        world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}
