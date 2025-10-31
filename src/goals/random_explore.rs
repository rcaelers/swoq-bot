use tracing::debug;

use crate::goals::goal::ExecuteGoal;
use crate::goals::path_to_action;
use crate::swoq_interface::DirectedAction;
use crate::types::Position;
use crate::world_state::WorldState;

pub struct RandomExploreGoal(pub Position);

impl ExecuteGoal for RandomExploreGoal {
    fn execute(&self, world: &mut WorldState, player_index: usize) -> Option<DirectedAction> {
        let player = &world.players[player_index];
        let player_position = player.position;
        let target_position = self.0;

        debug!("Random exploring to {:?}", target_position);

        // Try to path to the random position
        world.players[player_index].current_destination = Some(target_position);
        let path = world.find_path_for_player(player_index, player_position, target_position)?;
        world.players[player_index].current_path = Some(path.clone());
        path_to_action(player_position, &path)
    }
}
