use crate::goals::goal::ExecuteGoal;
use crate::goals::path_to_action;
use crate::swoq_interface::DirectedAction;
use crate::world_state::WorldState;

pub struct ReachExitGoal;

impl ExecuteGoal for ReachExitGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_position = player.position;
        let exit_position = world.exit_position?;
        world.players[player_index].current_destination = Some(exit_position);
        let path = world.find_path_for_player(player_index, player_position, exit_position)?;
        world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_position, &path)
    }
}
