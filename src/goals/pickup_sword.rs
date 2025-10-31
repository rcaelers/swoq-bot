use crate::goals::goal::ExecuteGoal;
use crate::goals::path_to_action;
use crate::goals::{validate_and_trim_path, validate_destination};
use crate::swoq_interface::DirectedAction;
use crate::world_state::WorldState;

pub struct PickupSwordGoal;

impl ExecuteGoal for PickupSwordGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player_pos = world.players[player_index].position;
        let sword_pos = world.closest_sword(&world.players[player_index])?;

        // Validate destination and trim path
        validate_destination(world, player_index);
        validate_and_trim_path(world, player_index);

        // Check if we can reuse existing path
        if let Some(dest) = world.players[player_index].current_destination
            && dest == sword_pos
            && let Some(ref path) = world.players[player_index].current_path
        {
            return path_to_action(player_pos, path);
        }

        // Compute new path
        world.players[player_index].current_destination = Some(sword_pos);
        let path = world.find_path_for_player(player_index, player_pos, sword_pos)?;
        world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}
