use crate::goals::goal::ExecuteGoal;
use crate::goals::path_to_action;
use crate::goals::validate_destination;
use crate::swoq_interface::DirectedAction;
use crate::types::Color;
use crate::world_state::WorldState;

pub struct GetKeyGoal(pub Color);

impl ExecuteGoal for GetKeyGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player_pos = world.players[player_index].position;
        let key_pos = world.closest_key(&world.players[player_index], self.0)?;

        validate_destination(world, player_index);

        // Compute new path
        world.players[player_index].current_destination = Some(key_pos);
        let path = world.find_path_for_player(player_index, player_pos, key_pos)?;
        world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_pos, &path)
    }
}
